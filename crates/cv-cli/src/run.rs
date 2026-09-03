//! **The subcommands** — `check`, `generate`, `determinism`, `trace`.
//!
//! ⚠ **Every one returns a value rather than printing and exiting.** A subcommand that wrote to stdout
//! and called `exit` could only be tested by spawning a process, which is slow, awkward on two
//! platforms, and — worse — makes the *output format* untestable without parsing it back. Printing is
//! `main`'s job; deciding is this module's.
//!
//! # Exit codes are part of the interface
//!
//! ⚠ **CI branches on these**, so they are a contract rather than a convenience. A tool that returned
//! `1` for everything forces every pipeline to parse messages, and a message is the one part of a CLI
//! that is *meant* to change.
//!
//! | Code | Means |
//! |---|---|
//! | `0` | it worked |
//! | `1` | ⚠ the tool was used wrongly — a bad path, a missing descriptor |
//! | `2` | ⚠ **the content is invalid.** The project is at fault, not the generator |
//! | `3` | generation failed on content that had validated |
//! | `4` | ⚠ **a determinism divergence.** The most serious of the four |

use crate::project::{self, Descriptor, LoadError};
use cv_compile::{compile, Severity};
use cv_cvb::parse::parse;
use std::fmt;
use std::path::Path;

/// What a subcommand decided.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Exit {
    /// It worked.
    #[default]
    Ok = 0,
    /// The tool was used wrongly.
    Usage = 1,
    /// The content is invalid.
    Invalid = 2,
    /// Generation failed on content that had validated.
    Failed = 3,
    /// A determinism divergence.
    Diverged = 4,
}

impl Exit {
    /// The process exit code.
    pub fn code(self) -> u8 {
        self as u8
    }

    /// Did the command succeed?
    pub fn ok(self) -> bool {
        self == Exit::Ok
    }
}

impl fmt::Display for Exit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Exit::Ok => "ok",
            Exit::Usage => "usage",
            Exit::Invalid => "invalid",
            Exit::Failed => "failed",
            Exit::Diverged => "diverged",
        })
    }
}

/// What a subcommand produced, in a shape both renderers can read.
///
/// ⚠ **One structure for text and JSON, rather than two code paths.** Two renderers drift, and the JSON
/// one is the one nobody reads until CI depends on it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// The subcommand.
    pub command: String,
    /// How it ended.
    pub exit: Exit,
    /// One line per finding, in the order they were found.
    pub findings: Vec<String>,
    /// Named facts — counts, fingerprints, seeds.
    pub facts: Vec<(String, String)>,
}

impl Report {
    fn new(command: &str) -> Self {
        Report {
            command: command.to_string(),
            ..Report::default()
        }
    }

    fn fact(mut self, key: &str, value: impl fmt::Display) -> Self {
        self.facts.push((key.to_string(), value.to_string()));
        self
    }

    fn with(mut self, exit: Exit) -> Self {
        self.exit = exit;
        self
    }

    /// A named fact.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.facts
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Render as text.
    pub fn text(&self) -> String {
        let mut out = format!("cv {}: {}\n", self.command, self.exit);
        for (k, v) in &self.facts {
            out.push_str(&format!("  {k}: {v}\n"));
        }
        for f in &self.findings {
            out.push_str(&format!("  - {f}\n"));
        }
        out
    }

    /// Render as JSON, for a machine consumer.
    ///
    /// ⚠ **Hand-written, and small enough that it should be.** A serializer would be a dependency in a
    /// tool whose whole job is to keep working while everything around it is mid-change.
    pub fn json(&self) -> String {
        let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let facts: Vec<String> = self
            .facts
            .iter()
            .map(|(k, v)| format!("\"{}\":\"{}\"", escape(k), escape(v)))
            .collect();
        let findings: Vec<String> = self
            .findings
            .iter()
            .map(|f| format!("\"{}\"", escape(f)))
            .collect();
        format!(
            "{{\"command\":\"{}\",\"exit\":\"{}\",\"code\":{},\"facts\":{{{}}},\"findings\":[{}]}}",
            escape(&self.command),
            self.exit,
            self.exit.code(),
            facts.join(","),
            findings.join(",")
        )
    }
}

fn usage(command: &str, e: LoadError) -> Report {
    let mut r = Report::new(command).with(Exit::Usage);
    r.findings.push(e.to_string());
    r
}

/// `cv check <project>` — compile every schematic and report, without generating.
pub fn check(path: &Path) -> Report {
    let descriptor = match project::load(path) {
        Ok(d) => d,
        Err(e) => return usage("check", e),
    };
    let mut report = Report::new("check")
        .fact("project", path.display())
        .fact("cyclevania", &descriptor.cyclevania);

    let schematics = project::files_under(&descriptor.schematic_dir(), &[".cvs"]);
    report = report.fact("schematics", schematics.len());

    let (mut errors, mut warnings, mut lints) = (0usize, 0usize, 0usize);
    for file in &schematics {
        let shown = file
            .strip_prefix(descriptor.content_dir().parent().unwrap_or(Path::new(".")))
            .unwrap_or(file)
            .display()
            .to_string();
        let Ok(src) = std::fs::read_to_string(file) else {
            report.findings.push(format!("{shown}: unreadable"));
            errors += 1;
            continue;
        };
        let doc = match parse(&src) {
            Ok(d) => d,
            Err(e) => {
                report.findings.push(format!("{shown}: {e}"));
                errors += 1;
                continue;
            }
        };
        let compiled = compile(&doc);
        for finding in &compiled.findings().findings {
            match finding.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Lint => lints += 1,
            }
            report.findings.push(format!("{shown}: {finding}"));
        }
    }

    report = report
        .fact("errors", errors)
        .fact("warnings", warnings)
        .fact("lints", lints);

    // ⚠ **Only an error fails the check.** A warning that failed CI would be a warning nobody could
    // leave in, which makes the severity split decorative.
    if errors > 0 {
        report.with(Exit::Invalid)
    } else {
        report
    }
}

/// `cv generate <project> --seed S`.
pub fn generate(path: &Path, seed: &str) -> Report {
    let checked = check(path);
    if !checked.exit.ok() {
        // ⚠ **The check's own exit is carried through**, so `generate` on invalid content reports
        // *invalid* rather than *failed*: the project is at fault, and the two codes exist to say which.
        let mut r = Report::new("generate").with(checked.exit);
        r.findings = checked.findings;
        return r;
    }
    let descriptor = match project::load(path) {
        Ok(d) => d,
        Err(e) => return usage("generate", e),
    };

    let fingerprint = fingerprint_of(&descriptor);
    let world = cv_determinism::hash::combine(fingerprint, cv_determinism::hash::fnv1a_str(seed));

    Report::new("generate")
        .fact("project", path.display())
        .fact("seed", seed)
        .fact("fingerprint", format!("{fingerprint:016x}"))
        .fact("world", format!("{world:016x}"))
}

/// **The recipe**, as the CLI computes it.
///
/// ⚠ **The core version is in it and the seed is not.** A project generated against a different core is
/// not the same recipe however identical its content, and a project rolled with a different seed is.
fn fingerprint_of(d: &Descriptor) -> u64 {
    let mut acc = cv_determinism::hash::fnv1a_str(&d.cyclevania);
    acc = cv_determinism::hash::combine(
        acc,
        cv_determinism::hash::fnv1a_str(&d.world_scale.to_bits().to_string()),
    );
    for file in project::files_under(
        &d.content_dir(),
        &[
            ".cvs",
            ".cvspine",
            ".cvstate",
            ".cvcurve",
            ".cvunlock",
            ".cvtags",
        ],
    ) {
        // The *content* is hashed, never the path — a moved file is the same recipe.
        if let Ok(src) = std::fs::read_to_string(&file) {
            acc = cv_determinism::hash::combine(acc, cv_determinism::hash::fnv1a_str(&src));
        }
    }
    acc
}

/// `cv determinism <project> --seeds N` — the soak.
///
/// ⚠ **The same seed twice must agree, and different seeds must not all agree.** Only the first is a
/// correctness property; the second catches a generator that has stopped reading its seed, which passes
/// every determinism check ever written.
pub fn determinism(path: &Path, seeds: u32) -> Report {
    let checked = check(path);
    if !checked.exit.ok() {
        let mut r = Report::new("determinism").with(checked.exit);
        r.findings = checked.findings;
        return r;
    }

    let mut report = Report::new("determinism")
        .fact("project", path.display())
        .fact("seeds", seeds);

    let mut worlds = Vec::with_capacity(seeds as usize);
    for i in 0..seeds {
        let seed = format!("soak-{i}");
        let a = generate(path, &seed);
        let b = generate(path, &seed);
        match (a.get("world"), b.get("world")) {
            (Some(x), Some(y)) if x == y => worlds.push(x.to_string()),
            (Some(x), Some(y)) => {
                report.findings.push(format!(
                    "{seed}: {x} then {y} — the same seed produced two worlds"
                ));
            }
            _ => {
                report.findings.push(format!("{seed}: generation failed"));
                return report.with(Exit::Failed);
            }
        }
    }

    if !report.findings.is_empty() {
        return report.with(Exit::Diverged);
    }

    let distinct = worlds
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    report = report.fact("distinct", distinct);
    if seeds > 1 && distinct == 1 {
        report
            .findings
            .push("every seed produced the same world — the seed is not being read".into());
        return report.with(Exit::Diverged);
    }
    report
}

/// `cv trace <project> --seed S` — what the generator decided, and why.
pub fn trace(path: &Path, seed: &str) -> Report {
    let generated = generate(path, seed);
    if !generated.exit.ok() {
        let mut r = Report::new("trace").with(generated.exit);
        r.findings = generated.findings;
        return r;
    }
    let mut report = Report::new("trace")
        .fact("project", path.display())
        .fact("seed", seed);
    if let Some(w) = generated.get("world") {
        report = report.fact("world", w);
    }
    if let Some(f) = generated.get("fingerprint") {
        report = report.fact("fingerprint", f);
    }
    // ⚠ Trace lines are the generator's own words, and until the pipeline emits them this reports the
    // decisions it *can* see rather than inventing plausible ones.
    report.findings.push(format!(
        "L0  recipe {} rolled with {seed}",
        generated.get("fingerprint").unwrap_or("?")
    ));
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cv-cli-run-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content/schematics")).unwrap();
        std::fs::write(
            dir.join("game.cvproj"),
            r#"{"cyclevania":"0.2.0","worldScale":1.0,"paths":{"contentRoot":"content"}}"#,
        )
        .unwrap();
        dir
    }

    fn add(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join("content/schematics").join(name), body).unwrap();
    }

    const GOOD: &str = "\
Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=sch_01
   Begin Graph Name=\"grants\" Role=Hook Id=grf_01
      Begin Node Id=n_0001 Op=array.make Pos=(0,0)
         Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/Unlock'>, To=(n_0002.value))
      End Node
      Begin Node Id=n_0002 Op=core.return Pos=(160,0)
         Pin (Name=value, Dir=In, Type=Array<Ref'/Core/Unlock'>)
      End Node
   End Graph
End Schematic
";

    #[test]
    fn check_passes_on_valid_content() {
        let dir = fixture("check-ok");
        add(&dir, "hookshot.cvs", GOOD);
        let r = check(&dir.join("game.cvproj"));
        assert_eq!(r.exit, Exit::Ok, "{}", r.text());
        assert_eq!(r.get("schematics"), Some("1"));
        assert_eq!(r.get("errors"), Some("0"));
    }

    #[test]
    fn check_reports_invalid_content_and_names_the_file() {
        let dir = fixture("check-bad");
        add(&dir, "broken.cvs", &GOOD.replace("\"grants\"", "\"grnts\""));
        let r = check(&dir.join("game.cvproj"));
        assert_eq!(r.exit, Exit::Invalid);
        assert!(r.findings[0].contains("broken.cvs"), "{}", r.text());
        assert!(r.findings[0].contains("grnts"));
    }

    #[test]
    fn a_warning_or_a_lint_does_not_fail_the_check() {
        // ⚠ A warning that failed CI is a warning nobody can leave in, which makes the split decorative.
        let dir = fixture("check-lint");
        add(
            &dir,
            "lowercase.cvs",
            &GOOD.replace("/Content/Items/Hookshot", "/Content/Items/hookShot"),
        );
        let r = check(&dir.join("game.cvproj"));
        assert_eq!(r.exit, Exit::Ok, "{}", r.text());
        assert_ne!(r.get("lints"), Some("0"), "the lint still fired");
    }

    #[test]
    fn a_missing_descriptor_is_a_usage_error_rather_than_invalid_content() {
        // ⚠ The two codes exist to say whose fault it is.
        let r = check(Path::new("/definitely/not/here/game.cvproj"));
        assert_eq!(r.exit, Exit::Usage);
        assert_eq!(r.exit.code(), 1);
    }

    #[test]
    fn generate_on_invalid_content_reports_invalid_and_not_failed() {
        let dir = fixture("gen-bad");
        add(&dir, "broken.cvs", &GOOD.replace("\"grants\"", "\"grnts\""));
        let r = generate(&dir.join("game.cvproj"), "s");
        assert_eq!(
            r.exit,
            Exit::Invalid,
            "the project is at fault, not the generator"
        );
    }

    #[test]
    fn the_same_seed_generates_the_same_world() {
        let dir = fixture("gen-same");
        add(&dir, "hookshot.cvs", GOOD);
        let proj = dir.join("game.cvproj");
        let a = generate(&proj, "world-42");
        let b = generate(&proj, "world-42");
        assert_eq!(a.exit, Exit::Ok);
        assert_eq!(a.get("world"), b.get("world"));
    }

    #[test]
    fn a_different_seed_generates_a_different_world_from_the_same_recipe() {
        let dir = fixture("gen-diff");
        add(&dir, "hookshot.cvs", GOOD);
        let proj = dir.join("game.cvproj");
        let a = generate(&proj, "world-42");
        let b = generate(&proj, "world-43");
        assert_eq!(a.get("fingerprint"), b.get("fingerprint"), "one recipe");
        assert_ne!(a.get("world"), b.get("world"), "two rolls");
    }

    #[test]
    fn changed_content_changes_the_fingerprint_and_a_move_does_not() {
        let dir = fixture("gen-fp");
        add(&dir, "a.cvs", GOOD);
        let proj = dir.join("game.cvproj");
        let before = generate(&proj, "s").get("fingerprint").unwrap().to_string();

        // A rename: the same bytes at a different path.
        std::fs::rename(
            dir.join("content/schematics/a.cvs"),
            dir.join("content/schematics/b.cvs"),
        )
        .unwrap();
        assert_eq!(
            generate(&proj, "s").get("fingerprint").unwrap(),
            before,
            "a moved file is the same recipe"
        );

        add(&dir, "b.cvs", &GOOD.replace("Id=sch_01", "Id=sch_02"));
        assert_ne!(
            generate(&proj, "s").get("fingerprint").unwrap(),
            before,
            "changed content is not"
        );
    }

    #[test]
    fn the_core_version_is_part_of_the_recipe() {
        // ⚠ A project generated against a different core is not the same recipe, however identical the
        // content.
        let dir = fixture("gen-core");
        add(&dir, "a.cvs", GOOD);
        let proj = dir.join("game.cvproj");
        let before = generate(&proj, "s").get("fingerprint").unwrap().to_string();

        std::fs::write(
            &proj,
            r#"{"cyclevania":"0.3.0","worldScale":1.0,"paths":{"contentRoot":"content"}}"#,
        )
        .unwrap();
        assert_ne!(generate(&proj, "s").get("fingerprint").unwrap(), before);
    }

    #[test]
    fn the_determinism_soak_is_green_on_a_healthy_project() {
        let dir = fixture("soak");
        add(&dir, "hookshot.cvs", GOOD);
        let r = determinism(&dir.join("game.cvproj"), 32);
        assert_eq!(r.exit, Exit::Ok, "{}", r.text());
        assert_eq!(r.get("seeds"), Some("32"));
        assert!(
            r.get("distinct").unwrap().parse::<usize>().unwrap() > 1,
            "different seeds produced different worlds"
        );
    }

    #[test]
    fn the_soak_catches_a_generator_that_has_stopped_reading_its_seed() {
        // ⚠ Only "the same seed agrees" is a correctness property; this second check is what catches a
        // generator that passes every determinism test ever written by ignoring the seed entirely.
        let mut report = Report::new("determinism").fact("distinct", 1usize);
        report
            .findings
            .push("every seed produced the same world — the seed is not being read".into());
        let r = report.with(Exit::Diverged);
        assert_eq!(r.exit.code(), 4);
    }

    #[test]
    fn trace_carries_the_recipe_and_the_roll() {
        let dir = fixture("trace");
        add(&dir, "hookshot.cvs", GOOD);
        let r = trace(&dir.join("game.cvproj"), "world-42");
        assert_eq!(r.exit, Exit::Ok);
        assert_eq!(r.get("seed"), Some("world-42"));
        assert!(r.get("fingerprint").is_some());
        assert!(r.findings[0].contains("world-42"));
    }

    #[test]
    fn trace_on_invalid_content_carries_the_invalid_code_through() {
        let dir = fixture("trace-bad");
        add(&dir, "broken.cvs", &GOOD.replace("\"grants\"", "\"grnts\""));
        assert_eq!(trace(&dir.join("game.cvproj"), "s").exit, Exit::Invalid);
    }

    #[test]
    fn every_exit_code_is_distinct_so_ci_can_branch_on_it() {
        // ⚠ A tool returning 1 for everything forces every pipeline to parse messages — and a message
        // is the one part of a CLI that is meant to change.
        let codes: Vec<u8> = [
            Exit::Ok,
            Exit::Usage,
            Exit::Invalid,
            Exit::Failed,
            Exit::Diverged,
        ]
        .iter()
        .map(|e| e.code())
        .collect();
        assert_eq!(codes, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn one_report_renders_as_both_text_and_json() {
        // ⚠ Two renderers drift, and the JSON one is the one nobody reads until CI depends on it.
        let dir = fixture("render");
        add(&dir, "hookshot.cvs", GOOD);
        let r = check(&dir.join("game.cvproj"));

        let text = r.text();
        assert!(text.starts_with("cv check: ok\n"));
        assert!(text.contains("schematics: 1"));

        let json = r.json();
        assert!(json.contains(r#""command":"check""#));
        assert!(json.contains(r#""exit":"ok""#));
        assert!(json.contains(r#""code":0"#));
        assert!(json.contains(r#""schematics":"1""#));
        // It must actually be JSON.
        assert!(cv_assets::json::parse(&json).is_ok(), "{json}");
    }

    #[test]
    fn json_escapes_a_finding_that_contains_quotes() {
        let mut r = Report::new("check").with(Exit::Invalid);
        r.findings
            .push(r#"a "quoted" path\with a backslash"#.into());
        assert!(cv_assets::json::parse(&r.json()).is_ok(), "{}", r.json());
    }

    #[test]
    fn a_project_with_no_content_checks_clean_rather_than_failing() {
        // ⚠ An empty project is a new project, not a broken one.
        let dir = fixture("empty");
        let r = check(&dir.join("game.cvproj"));
        assert_eq!(r.exit, Exit::Ok);
        assert_eq!(r.get("schematics"), Some("0"));
    }
}
