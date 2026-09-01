//! The **notification bridge** — how generation tells a host what it is doing.
//!
//! # One-way, and enforced by the type system
//!
//! Determinism forbids a host influencing generation mid-run: if a script could call *into* host code,
//! nondeterministic host state would leak in and replay would break. So the boundary is strictly
//! outbound. [`EventLog::emit`] returns `()` — there is no return value, no `Result`, no handle a
//! caller could read an answer from. The absence is the design: a channel that cannot carry anything
//! back cannot be misused to carry something back.
//!
//! A host still influences generation, of course — through **dials, assets and the seed, set before
//! the run** ([`crate::fingerprint`]). What it cannot do is answer a question halfway through.
//!
//! # Batched, not per-event
//!
//! A generation run emits tens of thousands of events. Crossing the FFI/WASM boundary once per event
//! would cost more than generating the world. Events accumulate in a log and flush in batches, so a
//! 10 000-event run costs a handful of crossings rather than 10 000.
//!
//! # Deterministic order
//!
//! Events are appended in emission order and drained in that order, so two runs of the same recipe and
//! seed produce the same event stream. That matters more than it sounds: the trace is how a dev
//! understands *why* a world came out the way it did, and a trace that reshuffles between runs cannot
//! be diffed against another.
//!
//! # What this is at M06
//!
//! The spine only. Routing named events to host callbacks (`cv.on("placed", handler)`) lands with the
//! bindings at M21; this is the core-side half it will sit on.

use crate::descriptor::ScopeRef;
use crate::fingerprint::Fingerprint;
use crate::object::ObjectId;
use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use std::fmt;

/// Something worth telling the host about.
///
/// Every variant is a statement of fact about work already done — never a question.
#[derive(Clone, Debug, PartialEq)]
pub enum GenEvent {
    /// A run began.
    Started { fingerprint: Fingerprint, seed: u64 },
    /// Progress through a pipeline layer, as a fraction in `[0, 1]`. The coarse signal a loading
    /// screen subscribes to — the near-universal need for a procgen game.
    LayerProgress { layer: u8, fraction: f64 },
    /// A scope advanced along the lazy-generation lifecycle.
    ScopeAdvanced {
        scope: ScopeRef,
        state: crate::node::NodeState,
    },
    /// Content was placed. A host may use this to pre-warm the corresponding asset.
    Placed {
        instance: ObjectId,
        content: ObjectId,
        scope: ScopeRef,
    },
    /// A candidate was rejected, with the reason — the backbone of the "watch it think" trace.
    Rejected {
        content: ObjectId,
        scope: ScopeRef,
        reason: String,
    },
    /// **Content sent the host a message.** `SendMessage(text, channel, debug_only)`.
    ///
    /// ⚠ **Observational only, and one-way by construction.** A host reply that changed the solve
    /// would kill replayability; cancellation aborts rather than alters, which is why it is the only
    /// permitted influence.
    ///
    /// ⚠ `debug_only` messages are **stripped entirely at cook**, not merely suppressed — see
    /// [`EventLog::cook`]. Suppression leaves the text in the shipped build, which is how a debug
    /// string ends up in a player's log.
    Message {
        text: String,
        channel: String,
        debug_only: bool,
    },
    /// The run finished.
    Finished { instances: u32, meshes: u32 },
}

impl GenEvent {
    /// The name this event is routed under (`cv.on(name, handler)`).
    pub fn name(&self) -> &str {
        match self {
            GenEvent::Started { .. } => "started",
            GenEvent::LayerProgress { .. } => "progress",
            GenEvent::ScopeAdvanced { .. } => "scope",
            GenEvent::Placed { .. } => "placed",
            GenEvent::Rejected { .. } => "rejected",
            // A message routes under its **channel**, so a host subscribes to `"door_opened"` rather
            // than to `"message"` and then re-dispatching.
            GenEvent::Message { channel, .. } => channel,
            GenEvent::Finished { .. } => "finished",
        }
    }

    /// Is this a high-volume event? Hosts commonly subscribe to the coarse ones only.
    pub fn is_verbose(&self) -> bool {
        matches!(
            self,
            GenEvent::Rejected { .. } | GenEvent::ScopeAdvanced { .. }
        )
    }

    fn tag(&self) -> u8 {
        match self {
            GenEvent::Started { .. } => 0,
            GenEvent::LayerProgress { .. } => 1,
            GenEvent::ScopeAdvanced { .. } => 2,
            GenEvent::Placed { .. } => 3,
            GenEvent::Rejected { .. } => 4,
            GenEvent::Message { .. } => 5,
            GenEvent::Finished { .. } => 6,
        }
    }
}

impl fmt::Display for GenEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenEvent::Started { fingerprint, seed } => {
                write!(f, "started recipe {fingerprint} seed {seed}")
            }
            GenEvent::LayerProgress { layer, fraction } => {
                write!(f, "L{layer} {:.0}%", fraction * 100.0)
            }
            GenEvent::ScopeAdvanced { scope, state } => write!(f, "{scope} → {state}"),
            GenEvent::Placed { content, scope, .. } => write!(f, "placed {content} in {scope}"),
            GenEvent::Rejected {
                content,
                scope,
                reason,
            } => {
                write!(f, "rejected {content} in {scope}: {reason}")
            }
            GenEvent::Message {
                text,
                channel,
                debug_only,
            } => {
                let mark = if *debug_only { " [debug]" } else { "" };
                write!(f, "{channel}{mark}: {text}")
            }
            GenEvent::Finished { instances, meshes } => {
                write!(f, "finished — {instances} instances, {meshes} meshes")
            }
        }
    }
}

/// Which events a consumer wants. Verbose events are opt-in because they dominate the volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Verbosity {
    /// Drop everything — zero cost when nobody is listening.
    Silent,
    /// Lifecycle and progress only: what a loading screen needs.
    #[default]
    Coarse,
    /// Everything, including per-candidate rejections — the full "watch it think" trace.
    Verbose,
}

/// Accumulates events for batched delivery.
///
/// Note the signature of [`EventLog::emit`]: it takes an event and returns nothing. That is the whole
/// host boundary, and it is one-way by construction rather than by convention.
#[derive(Clone, Debug, Default)]
pub struct EventLog {
    events: Vec<GenEvent>,
    verbosity: Verbosity,
    /// Counts events dropped by verbosity, so a trace can say "12 340 rejections suppressed" rather
    /// than quietly appearing to have had none.
    suppressed: u64,
    /// ⚠ **A cooked build never constructs a `debug_only` message at all.** Not a verbosity level:
    /// verbosity is a *listener's* choice and can be turned back up, while cook is a property of the
    /// build and must not be.
    cooked: bool,
}

impl EventLog {
    /// A log at the default (coarse) verbosity.
    pub fn new() -> Self {
        EventLog::default()
    }

    /// A log at a chosen verbosity.
    pub fn with_verbosity(verbosity: Verbosity) -> Self {
        EventLog {
            events: Vec::new(),
            verbosity,
            suppressed: 0,
            cooked: false,
        }
    }

    /// A log for a **cooked build**, where `debug_only` messages do not exist.
    ///
    /// ⚠ **Stripped, not suppressed.** A suppressed message still carries its text through the build;
    /// a stripped one is never constructed. That difference is the whole reason `debug_only` defaults
    /// to `true` — a developer who forgets to think about it ships nothing rather than everything.
    pub fn cooked() -> Self {
        EventLog {
            events: Vec::new(),
            verbosity: Verbosity::Coarse,
            suppressed: 0,
            cooked: true,
        }
    }

    /// Is this log for a cooked build?
    pub fn is_cooked(&self) -> bool {
        self.cooked
    }

    /// The current verbosity.
    pub fn verbosity(&self) -> Verbosity {
        self.verbosity
    }

    /// Emit an event.
    ///
    /// Returns nothing, deliberately — see the module docs. There is no path for a host to answer.
    pub fn emit(&mut self, event: GenEvent) {
        // ⚠ **Before verbosity, and not counted as suppressed.** A cooked build has no debug messages
        // to have dropped, so reporting "1 suppressed" would be telling a player's log that something
        // was withheld from it.
        if self.cooked
            && matches!(
                event,
                GenEvent::Message {
                    debug_only: true,
                    ..
                }
            )
        {
            return;
        }
        match self.verbosity {
            Verbosity::Silent => {
                self.suppressed += 1;
                return;
            }
            Verbosity::Coarse if event.is_verbose() => {
                self.suppressed += 1;
                return;
            }
            _ => {}
        }
        self.events.push(event);
    }

    /// How many events are queued.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Is the queue empty?
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// How many events were dropped by the verbosity setting.
    pub fn suppressed(&self) -> u64 {
        self.suppressed
    }

    /// The queued events, in emission order.
    pub fn events(&self) -> &[GenEvent] {
        &self.events
    }

    /// Take the queued batch, leaving the log empty — one boundary crossing per flush.
    pub fn drain(&mut self) -> Vec<GenEvent> {
        std::mem::take(&mut self.events)
    }

    /// Should a batch be flushed yet?
    ///
    /// Lets a caller flush on a size threshold without reaching into the queue.
    pub fn should_flush(&self, batch_size: usize) -> bool {
        self.events.len() >= batch_size
    }
}

impl Serialize for GenEvent {
    fn serialize(&self, w: &mut Writer) {
        w.u8(self.tag());
        match self {
            GenEvent::Started { fingerprint, seed } => {
                w.write(fingerprint);
                w.u64(*seed);
            }
            GenEvent::LayerProgress { layer, fraction } => {
                w.u8(*layer);
                w.f64(*fraction);
            }
            GenEvent::ScopeAdvanced { scope, state } => {
                w.write(scope);
                w.write(state);
            }
            GenEvent::Placed {
                instance,
                content,
                scope,
            } => {
                w.write(instance);
                w.write(content);
                w.write(scope);
            }
            GenEvent::Rejected {
                content,
                scope,
                reason,
            } => {
                w.write(content);
                w.write(scope);
                w.str(reason);
            }
            GenEvent::Message {
                text,
                channel,
                debug_only,
            } => {
                w.str(text);
                w.str(channel);
                w.u8(u8::from(*debug_only));
            }
            GenEvent::Finished { instances, meshes } => {
                w.u32(*instances);
                w.u32(*meshes);
            }
        }
    }
}

impl Deserialize for GenEvent {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(match r.u8()? {
            0 => GenEvent::Started {
                fingerprint: r.read()?,
                seed: r.u64()?,
            },
            1 => GenEvent::LayerProgress {
                layer: r.u8()?,
                fraction: r.f64()?,
            },
            2 => GenEvent::ScopeAdvanced {
                scope: r.read()?,
                state: r.read()?,
            },
            3 => GenEvent::Placed {
                instance: r.read()?,
                content: r.read()?,
                scope: r.read()?,
            },
            4 => GenEvent::Rejected {
                content: r.read()?,
                scope: r.read()?,
                reason: r.str()?,
            },
            5 => GenEvent::Message {
                text: r.str()?,
                channel: r.str()?,
                debug_only: r.u8()? != 0,
            },
            6 => GenEvent::Finished {
                instances: r.u32()?,
                meshes: r.u32()?,
            },
            _ => return Err(SerError::InvalidValue("unknown GenEvent tag")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeState;
    use crate::serialize::{from_bytes, to_bytes};

    fn sample() -> Vec<GenEvent> {
        vec![
            GenEvent::Started {
                fingerprint: Fingerprint::from_raw(0xABC),
                seed: 42,
            },
            GenEvent::LayerProgress {
                layer: 2,
                fraction: 0.5,
            },
            GenEvent::Placed {
                instance: ObjectId::derived("instance", "door_1"),
                content: ObjectId::derived("actor", "door"),
                scope: ScopeRef(3),
            },
            GenEvent::Rejected {
                content: ObjectId::derived("actor", "statue"),
                scope: ScopeRef(3),
                reason: "footprint exceeds the space".into(),
            },
            GenEvent::ScopeAdvanced {
                scope: ScopeRef(3),
                state: NodeState::Realized,
            },
            GenEvent::Message {
                text: "by key_bronze".into(),
                channel: "door_opened".into(),
                debug_only: false,
            },
            GenEvent::Finished {
                instances: 12,
                meshes: 30,
            },
        ]
    }

    #[test]
    fn emission_order_is_preserved_exactly() {
        let mut log = EventLog::with_verbosity(Verbosity::Verbose);
        for e in sample() {
            log.emit(e);
        }
        assert_eq!(log.drain(), sample());
        assert!(log.is_empty(), "draining takes the batch");
    }

    #[test]
    fn the_same_run_produces_the_same_stream() {
        // A trace that reshuffles between runs cannot be diffed, which is most of its value.
        let run = || {
            let mut log = EventLog::with_verbosity(Verbosity::Verbose);
            for e in sample() {
                log.emit(e);
            }
            to_bytes(&log.drain())
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn verbosity_suppresses_the_high_volume_events_and_says_so() {
        let mut coarse = EventLog::with_verbosity(Verbosity::Coarse);
        for e in sample() {
            coarse.emit(e);
        }
        // The two verbose events are dropped, and the drop is *counted* rather than hidden.
        assert_eq!(coarse.len(), 5);
        assert_eq!(coarse.suppressed(), 2);
        assert!(!coarse.events().iter().any(|e| e.is_verbose()));

        let mut silent = EventLog::with_verbosity(Verbosity::Silent);
        for e in sample() {
            silent.emit(e);
        }
        assert!(silent.is_empty());
        assert_eq!(silent.suppressed(), 7);
    }

    #[test]
    fn batching_flushes_in_chunks() {
        let mut log = EventLog::with_verbosity(Verbosity::Verbose);
        assert!(!log.should_flush(4));
        for _ in 0..4 {
            log.emit(GenEvent::LayerProgress {
                layer: 1,
                fraction: 0.1,
            });
        }
        assert!(
            log.should_flush(4),
            "a full batch is ready to cross the boundary once"
        );
        assert_eq!(log.drain().len(), 4);
        assert!(!log.should_flush(4));
    }

    #[test]
    fn events_route_by_name_and_scripts_use_their_own() {
        let events = sample();
        let names: Vec<&str> = events.iter().map(|e| e.name()).collect();
        assert_eq!(
            names,
            vec![
                "started",
                "progress",
                "placed",
                "rejected",
                "scope",
                "door_opened",
                "finished"
            ]
        );
        // A message routes under its channel, so a host subscribes directly to it.
        assert_eq!(
            GenEvent::Message {
                text: String::new(),
                channel: "boss_placed".into(),
                debug_only: false,
            }
            .name(),
            "boss_placed"
        );
    }

    #[test]
    fn events_round_trip() {
        let events = sample();
        assert_eq!(
            from_bytes::<Vec<GenEvent>>(&to_bytes(&events)).unwrap(),
            events
        );
    }

    #[test]
    fn events_read_well_in_a_trace() {
        assert_eq!(
            GenEvent::LayerProgress {
                layer: 3,
                fraction: 0.25
            }
            .to_string(),
            "L3 25%"
        );
        assert_eq!(
            GenEvent::Finished {
                instances: 12,
                meshes: 30
            }
            .to_string(),
            "finished — 12 instances, 30 meshes"
        );
        assert!(GenEvent::Rejected {
            content: ObjectId::from_raw(1),
            scope: ScopeRef(2),
            reason: "too big".into()
        }
        .to_string()
        .contains("too big"));
    }
}
