//! **A `.cvstate` document, read and checked** — everything the State view draws.
//!
//! ⚠ **The editor computes none of this.** The un-softlockable check over a state graph is the solver's
//! own analysis ([`cv_core::state`]), and *a check is not a view*: the editor draws the result. It lived
//! in an editor crate for three milestones because the editor was its first caller, and being the first
//! caller does not make you the owner.
//!
//! # Positions come from the document
//!
//! ▶ **Each `Begin State` carries `Pos=(x,y)`**, so drawing needs no layout algorithm and no layout
//! *decision* — the author placed the boxes, and the view honours that. Which is why the State view can
//! be drawn while panel arrangement is still waiting on mockups: node placement is authored data, not a
//! design question.

use cv_core::state::{Finding, StateGraph};
use cv_cvb::parse::Block;
use cv_cvb::value::Value;

use crate::content::ContentError;

/// One state's authored position, as the document placed it.
///
/// ⚠ **Named rather than a bare tuple.** `(String, f64, f64)` reads identically whichever way round
/// x and y go, which is the kind of mistake that draws a plausible wrong picture.
struct Placed {
    name: String,
    x: f64,
    y: f64,
}

/// Read a `.cvstate` document into the core's graph, keeping each state's authored position.
///
/// Returns the graph and the positions side by side: [`StateGraph`] is the analysis model and has no
/// business carrying pixels.
fn read(text: &str) -> Result<(StateGraph, Vec<Placed>), ContentError> {
    let root: Block = cv_cvb::parse(text).map_err(|e| ContentError::Malformed {
        rel: "<state graph>".into(),
        detail: e.to_string(),
    })?;

    let text_of = |v: Option<&Value>| -> String {
        match v {
            Some(Value::Quoted(s)) | Some(Value::Ident(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        }
    };

    let variable = text_of(root.header_get("Variable")).replace('"', "");
    let mut graph = StateGraph::new(if variable.is_empty() {
        text_of(root.header_get("Path"))
    } else {
        variable
    });
    let mut positions = Vec::new();

    for state in root.blocks("State") {
        let name = text_of(state.header_get("Name"));
        let initial = matches!(state.get("Initial"), Some(Value::Ident(s)) if s == "true");
        graph = graph.state(&name, initial);
        let (x, y) = match state.header_get("Pos") {
            Some(Value::Tuple(items)) if items.len() >= 2 => {
                (number(&items[0].1), number(&items[1].1))
            }
            _ => (0.0, 0.0),
        };
        positions.push(Placed { name, x, y });
    }

    for t in root.blocks("Transition") {
        let from = text_of(t.header_get("From"));
        let to = text_of(t.header_get("To"));
        // ⚠ **A gate is what the transition costs**, and its shape is a rule expression. The check only
        // needs *whether* something is required and what it is named, so the unlock names are lifted
        // out rather than the rule being re-implemented here.
        let requires = unlocks(t);
        let via = text_of(t.get("Via"));
        let borrowed: Vec<&str> = requires.iter().map(String::as_str).collect();
        graph = graph.transition(&from, &to, &borrowed, &via);
    }

    Ok((graph, positions))
}

fn number(v: &Value) -> f64 {
    match v {
        Value::Number { value, .. } => *value,
        Value::Ident(s) | Value::Quoted(s) => s.parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Every unlock a transition's gate names.
///
/// ⚠ **Names, not a re-implemented rule engine.** The state check asks *"can this be taken freely"*,
/// and a gate that names anything answers no. Evaluating the rule properly is the solver's job, and
/// duplicating it here would be a second opinion that drifts.
fn unlocks(t: &Block) -> Vec<String> {
    let mut out = Vec::new();
    let mut scan = |text: &str| {
        for part in text.split('#').skip(1) {
            let name: String = part
                .trim_start_matches('"')
                .chars()
                .take_while(|c| *c != '"')
                .collect();
            if !name.is_empty() {
                out.push(name);
            }
        }
    };
    if let Some(gate) = t.get("Gate") {
        scan(&gate.to_string());
    }
    if let Some(gate) = t.header_get("Gate") {
        scan(&gate.to_string());
    }
    out.sort();
    out.dedup();
    out
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Check a `.cvstate` document and return everything the view needs, as JSON.
///
/// ⚠ **One call, not two.** A view that fetched the graph and then the findings could draw a graph the
/// findings do not describe — a race nobody would reproduce and everybody would blame on the drawing.
pub fn check(text: &str) -> Result<String, ContentError> {
    let (graph, positions) = read(text)?;
    let findings = graph.check();
    let degrees = graph.out_degree();

    let states: Vec<String> = positions
        .iter()
        .map(|Placed { name, x, y }| {
            let initial = graph.states.iter().any(|s| &s.name == name && s.initial);
            format!(
                "{{\"name\":\"{}\",\"x\":{x},\"y\":{y},\"initial\":{initial},\"outDegree\":{}}}",
                esc(name),
                degrees.get(name.as_str()).copied().unwrap_or(0)
            )
        })
        .collect();

    let transitions: Vec<String> = graph
        .transitions
        .iter()
        .map(|t| {
            let requires: Vec<String> = t
                .requires
                .iter()
                .map(|r| format!("\"{}\"", esc(r)))
                .collect();
            format!(
                "{{\"from\":\"{}\",\"to\":\"{}\",\"via\":\"{}\",\"requires\":[{}]}}",
                esc(&t.from),
                esc(&t.to),
                esc(&t.via),
                requires.join(",")
            )
        })
        .collect();

    let faults: Vec<String> = findings
        .iter()
        .map(|f| {
            format!(
                "{{\"kind\":\"{}\",\"blocks\":{},\"state\":\"{}\",\"message\":\"{}\"}}",
                kind_of(f),
                f.blocks(),
                esc(state_of(f)),
                esc(&f.to_string())
            )
        })
        .collect();

    Ok(format!(
        "{{\"variable\":\"{}\",\"satisfiesP15\":{},\"states\":[{}],\"transitions\":[{}],\"findings\":[{}]}}",
        esc(&graph.variable),
        graph.satisfies_p15(),
        states.join(","),
        transitions.join(","),
        faults.join(",")
    ))
}

/// ⚠ **Four faults, named apart.** Telling a developer *"nobody can get here"* when the truth is
/// *"nobody can leave"* sends them to the wrong end of the graph.
fn kind_of(f: &Finding) -> &'static str {
    match f {
        Finding::Inaccessible { .. } => "inaccessible",
        Finding::DeadEnd { .. } => "dead-end",
        Finding::ExitGated { .. } => "exit-gated",
        Finding::InitialUnclear { .. } => "initial-unclear",
        Finding::UnknownState { .. } => "unknown-state",
    }
}

/// Which state a finding is about, so the view can draw it on that box.
fn state_of(f: &Finding) -> &str {
    match f {
        Finding::Inaccessible { state }
        | Finding::DeadEnd { state }
        | Finding::ExitGated { state, .. } => state,
        Finding::UnknownState { missing, .. } => missing,
        Finding::InitialUnclear { .. } => "",
    }
}
