//! **Memoization via recorded reads** — the single most valuable optimization for a solver that
//! re-evaluates constantly.
//!
//! ⚠ **A cache with an unspecified key is a correctness bug waiting for the first person to add a
//! field.** *"Cached by the context they read"* named no key, so this is what a key is:
//!
//! ```text
//! key = (hook, self, sorted[ (channel, subject) -> observed value ])
//! ```
//!
//! # Recorded, never declared
//!
//! ⚠ **A hook that *declares* what it reads is a hook whose declaration drifts from its body** the
//! first time somebody edits one and not the other — and the failure is **silent**, because a stale key
//! returns a plausible answer. The VM watches the reads instead, so the key cannot disagree with the
//! computation that produced it.
//!
//! # The channel list is closed, and an unattributable read is uncacheable
//!
//! ⚠ **A read the VM cannot attribute makes the entry unkeyable**, so the result is returned and **not
//! cached**. That is the conservative direction: a cache that guessed would return a right answer at
//! the wrong time, which is worse than having no cache.
//!
//! # Per read, not per scope
//!
//! ⚠ **The tempting coarse key is *"the scope this ran in"*, and it is wrong in both directions.** It
//! invalidates on changes the hook never looked at — losing the optimization it exists to provide — *and*
//! it misses a change to something read from a **different** scope, which `ctx.instances_of(AREA)` does
//! routinely.
//!
//! # The cache is deletable without changing the output
//!
//! ⚠ **Verified rather than asserted.** [`Memo::disabled`] is what a CI pass with caches off runs
//! against, and a world that differed would mean the key was wrong — which is the only way to find that
//! out.

use std::collections::BTreeMap;
use std::fmt;

/// What kind of context read an entry depends on.
///
/// ⚠ **Closed.** A seventh channel is a design change, not an implementation detail, because every one
/// of these is a thing that can change between two evaluations of the same hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Channel {
    /// Whether an unlock is held.
    Held,
    /// Something about a scope — its occupants, its bounds, its neighbours.
    Scope,
    /// A dial's resolved value.
    Dial,
    /// A state variable's setting.
    State,
    /// ⚠ **The fidelity rung.** A channel and not an ambient: the same question at a coarser rung has a
    /// different answer, so a cache that ignored it would serve an L2c answer to an L3 question.
    Tolerance,
    /// A project setting.
    Settings,
}

impl Channel {
    /// All six.
    pub const ALL: [Channel; 6] = [
        Channel::Held,
        Channel::Scope,
        Channel::Dial,
        Channel::State,
        Channel::Tolerance,
        Channel::Settings,
    ];
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Channel::Held => "held",
            Channel::Scope => "scope",
            Channel::Dial => "dial",
            Channel::State => "state",
            Channel::Tolerance => "tolerance",
            Channel::Settings => "settings",
        })
    }
}

/// One thing a hook read, and what it saw.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Read {
    /// Which channel.
    pub channel: Channel,
    /// What was asked about — an unlock id, a scope handle, a dial's qualified name.
    pub subject: String,
    /// What came back, in a form two evaluations can compare.
    pub observed: String,
}

/// What a hook read while it ran.
///
/// ⚠ **Sorted on completion**, so two evaluations that read the same things in a different order
/// produce the same key. Iteration order escaping into a cache key is the same defect as iteration
/// order escaping into a world.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recording {
    reads: Vec<Read>,
    unattributed: usize,
}

impl Recording {
    /// A fresh recording.
    pub fn new() -> Self {
        Recording::default()
    }

    /// Note a read.
    pub fn record(
        &mut self,
        channel: Channel,
        subject: impl Into<String>,
        observed: impl Into<String>,
    ) {
        self.reads.push(Read {
            channel,
            subject: subject.into(),
            observed: observed.into(),
        });
    }

    /// ⚠ **Note a read the VM could not attribute to a channel.**
    ///
    /// One of these makes the whole entry unkeyable. It is not an error — the hook still runs and
    /// still returns — it simply cannot be cached, which is the only safe answer.
    pub fn record_unattributed(&mut self) {
        self.unattributed += 1;
    }

    /// May a result with this recording be cached at all?
    pub fn is_keyable(&self) -> bool {
        self.unattributed == 0
    }

    /// How many reads.
    pub fn len(&self) -> usize {
        self.reads.len()
    }

    /// Nothing read — a hook that depends on nothing.
    pub fn is_empty(&self) -> bool {
        self.reads.is_empty()
    }

    /// The reads, sorted and de-duplicated.
    pub fn into_key(mut self, hook: impl Into<String>, subject: impl Into<String>) -> Option<Key> {
        if !self.is_keyable() {
            return None;
        }
        self.reads.sort();
        self.reads.dedup();
        Some(Key {
            hook: hook.into(),
            subject: subject.into(),
            reads: self.reads,
        })
    }
}

/// What a cached result is filed under.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    /// Which hook.
    pub hook: String,
    /// Whose — the object the hook ran on.
    pub subject: String,
    /// Every context read it performed, sorted.
    pub reads: Vec<Read>,
}

impl Key {
    /// Does every recorded read still see what it saw?
    ///
    /// ⚠ **This is the invalidation, and it is a *comparison* rather than a subscription.** A hook that
    /// registered listeners would have to unregister them, and a missed unregistration is a leak that
    /// looks like a correctness bug much later. Re-reading is cheap because the reads are few.
    pub fn still_holds(&self, current: &dyn Fn(Channel, &str) -> Option<String>) -> bool {
        self.reads
            .iter()
            .all(|r| current(r.channel, &r.subject).as_deref() == Some(r.observed.as_str()))
    }
}

/// Why a lookup did not hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Miss {
    /// Nothing under that key.
    Absent,
    /// The cache is off.
    Disabled,
    /// Something the hook read has changed.
    Stale,
}

/// The memo cache.
///
/// ⚠ **Only pure hooks reach it.** Purity is a property of the *manifest entry*, so the compiler knows
/// it without inspecting a body — and a hook the manifest does not mark pure is simply never offered
/// here.
#[derive(Clone, Debug, Default)]
pub struct Memo<V: Clone> {
    entries: BTreeMap<Key, V>,
    enabled: bool,
    hits: usize,
    misses: usize,
    refused: usize,
}

impl<V: Clone> Memo<V> {
    /// A cache that caches.
    pub fn new() -> Self {
        Memo {
            entries: BTreeMap::new(),
            enabled: true,
            hits: 0,
            misses: 0,
            refused: 0,
        }
    }

    /// ⚠ **A cache that never caches — what the caches-off CI pass runs against.**
    ///
    /// A world that differed from the cached run would mean the key was wrong, and there is no other
    /// way to find that out.
    pub fn disabled() -> Self {
        Memo {
            enabled: false,
            ..Memo::new()
        }
    }

    /// Is it on?
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Look a result up, re-checking the reads it was filed under.
    pub fn get(
        &mut self,
        key: &Key,
        current: &dyn Fn(Channel, &str) -> Option<String>,
    ) -> Result<V, Miss> {
        if !self.enabled {
            self.misses += 1;
            return Err(Miss::Disabled);
        }
        let Some(v) = self.entries.get(key) else {
            self.misses += 1;
            return Err(Miss::Absent);
        };
        if !key.still_holds(current) {
            self.misses += 1;
            return Err(Miss::Stale);
        }
        self.hits += 1;
        Ok(v.clone())
    }

    /// File a result, if it is keyable at all.
    ///
    /// ⚠ **Returns whether it was stored**, because *"we ran and could not cache"* is a fact worth
    /// counting: a project whose hooks are mostly unkeyable has a performance problem the profiler
    /// cannot see.
    pub fn put(&mut self, key: Option<Key>, value: V) -> bool {
        let Some(key) = key else {
            self.refused += 1;
            return false;
        };
        if !self.enabled {
            return false;
        }
        self.entries.insert(key, value);
        true
    }

    /// How many lookups hit.
    pub fn hits(&self) -> usize {
        self.hits
    }

    /// How many missed.
    pub fn misses(&self) -> usize {
        self.misses
    }

    /// How many results could not be keyed at all.
    pub fn unkeyable(&self) -> usize {
        self.refused
    }

    /// How many entries are held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Nothing cached.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drop everything.
    ///
    /// ⚠ **Always safe, by construction.** If clearing the cache could change an output, the key was
    /// wrong — which is the property the caches-off pass exists to check.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(pairs: &[(Channel, &str, &str)]) -> impl Fn(Channel, &str) -> Option<String> + use<> {
        let map: BTreeMap<(Channel, String), String> = pairs
            .iter()
            .map(|(c, s, v)| ((*c, (*s).to_string()), (*v).to_string()))
            .collect();
        move |c, s| map.get(&(c, s.to_string())).cloned()
    }

    fn recording(pairs: &[(Channel, &str, &str)]) -> Recording {
        let mut r = Recording::new();
        for (c, s, v) in pairs {
            r.record(*c, *s, *v);
        }
        r
    }

    #[test]
    fn a_result_is_reused_while_everything_it_read_still_holds() {
        let reads = [(Channel::Held, "Missiles", "true")];
        let mut memo = Memo::new();
        let key = recording(&reads).into_key("requires", "hookshot").unwrap();
        assert!(memo.put(Some(key.clone()), 42));
        assert_eq!(memo.get(&key, &world(&reads)), Ok(42));
        assert_eq!(memo.hits(), 1);
    }

    #[test]
    fn a_changed_read_invalidates_the_entry() {
        let mut memo = Memo::new();
        let key = recording(&[(Channel::Held, "Missiles", "true")])
            .into_key("requires", "hookshot")
            .unwrap();
        memo.put(Some(key.clone()), 42);
        let after = world(&[(Channel::Held, "Missiles", "false")]);
        assert_eq!(memo.get(&key, &after), Err(Miss::Stale));
    }

    #[test]
    fn a_read_the_vm_cannot_attribute_makes_the_result_uncacheable() {
        // ⚠ The conservative direction: a cache that guessed would return a right answer at the wrong
        // time, which is worse than no cache.
        let mut r = recording(&[(Channel::Held, "Missiles", "true")]);
        r.record_unattributed();
        assert!(!r.is_keyable());
        assert_eq!(r.clone().into_key("requires", "x"), None);

        let mut memo: Memo<i32> = Memo::new();
        assert!(!memo.put(r.into_key("requires", "x"), 42));
        assert_eq!(memo.unkeyable(), 1);
        assert!(memo.is_empty());
    }

    #[test]
    fn the_key_is_order_independent() {
        // ⚠ Iteration order escaping into a cache key is the same defect as escaping into a world.
        let a = recording(&[
            (Channel::Held, "Missiles", "true"),
            (Channel::Dial, "Hookshot.length", "30.0"),
        ])
        .into_key("requires", "h")
        .unwrap();
        let b = recording(&[
            (Channel::Dial, "Hookshot.length", "30.0"),
            (Channel::Held, "Missiles", "true"),
        ])
        .into_key("requires", "h")
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn tolerance_is_part_of_the_key_so_a_coarse_answer_is_not_served_to_a_fine_question() {
        // ⚠ The same question at a coarser rung has a different answer — that is the fidelity ladder.
        let coarse = recording(&[(Channel::Tolerance, "rung", "L2c")])
            .into_key("standable", "floor_1")
            .unwrap();
        let fine = recording(&[(Channel::Tolerance, "rung", "L3")])
            .into_key("standable", "floor_1")
            .unwrap();
        assert_ne!(coarse, fine);

        let mut memo = Memo::new();
        memo.put(Some(coarse), "AMBIGUOUS");
        assert_eq!(
            memo.get(&fine, &world(&[(Channel::Tolerance, "rung", "L3")])),
            Err(Miss::Absent),
            "an L2c answer must not be served to an L3 question"
        );
    }

    #[test]
    fn an_ambiguous_result_is_cached_like_any_other() {
        // ⚠ It is a real answer at that rung, and special-casing it would recompute the most expensive
        // answers most often.
        let reads = [(Channel::Tolerance, "rung", "L2c")];
        let mut memo = Memo::new();
        let key = recording(&reads).into_key("standable", "floor_1").unwrap();
        memo.put(Some(key.clone()), "AMBIGUOUS");
        assert_eq!(memo.get(&key, &world(&reads)), Ok("AMBIGUOUS"));
    }

    #[test]
    fn a_disabled_cache_never_hits_and_never_stores() {
        // ⚠ What the caches-off CI pass runs against.
        let reads = [(Channel::Held, "Missiles", "true")];
        let mut memo = Memo::disabled();
        let key = recording(&reads).into_key("requires", "h").unwrap();
        assert!(!memo.put(Some(key.clone()), 42));
        assert_eq!(memo.get(&key, &world(&reads)), Err(Miss::Disabled));
        assert!(memo.is_empty());
        assert!(!memo.is_enabled());
    }

    #[test]
    fn clearing_is_always_safe_and_leaves_only_recomputation() {
        let reads = [(Channel::Held, "Missiles", "true")];
        let mut memo = Memo::new();
        let key = recording(&reads).into_key("requires", "h").unwrap();
        memo.put(Some(key.clone()), 42);
        assert_eq!(memo.len(), 1);
        memo.clear();
        assert_eq!(memo.get(&key, &world(&reads)), Err(Miss::Absent));
    }

    #[test]
    fn a_read_from_another_scope_is_keyed_and_a_scope_level_key_would_have_missed_it() {
        // ⚠ `ctx.instances_of(AREA)` reads outside the hook's own scope routinely, which is half of why
        // the coarse key is wrong.
        let mut memo = Memo::new();
        let key = recording(&[
            (Channel::Scope, "area_1", "3 instances"),
            (Channel::Scope, "space_7", "1 instance"),
        ])
        .into_key("requires", "hookshot")
        .unwrap();
        memo.put(Some(key.clone()), 1);

        let elsewhere = world(&[
            (Channel::Scope, "area_1", "3 instances"),
            (Channel::Scope, "space_7", "2 instances"),
        ]);
        assert_eq!(
            memo.get(&key, &elsewhere),
            Err(Miss::Stale),
            "a change in a scope the hook read must invalidate, wherever that scope is"
        );
    }

    #[test]
    fn a_hook_that_reads_nothing_is_cached_forever() {
        // A constant hook is the easiest possible case and must not be a special one.
        let mut memo = Memo::new();
        let key = Recording::new()
            .into_key("classification", "hookshot")
            .unwrap();
        assert!(key.reads.is_empty());
        memo.put(Some(key.clone()), "PROGRESSION");
        assert_eq!(memo.get(&key, &world(&[])), Ok("PROGRESSION"));
    }

    #[test]
    fn two_hooks_on_the_same_subject_do_not_share_an_entry() {
        let reads = [(Channel::Held, "Missiles", "true")];
        let a = recording(&reads).into_key("requires", "hookshot").unwrap();
        let b = recording(&reads).into_key("grants", "hookshot").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn the_same_hook_on_two_subjects_does_not_share_an_entry() {
        let reads = [(Channel::Held, "Missiles", "true")];
        let a = recording(&reads).into_key("requires", "hookshot").unwrap();
        let b = recording(&reads).into_key("requires", "grapple").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn the_channel_list_is_closed() {
        // ⚠ A seventh channel is a design change, not an implementation detail.
        assert_eq!(Channel::ALL.len(), 6);
        let mut seen = std::collections::BTreeSet::new();
        for c in Channel::ALL {
            assert!(seen.insert(c.to_string()));
        }
    }
}
