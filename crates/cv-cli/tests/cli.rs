//! **M15a's green condition** — `check`, `generate` and `determinism` run against a project skeleton,
//! and the exit codes are meaningful enough for CI to branch on.
//!
//! ⚠ **These spawn the real binary**, because an exit code is the one part of a CLI that unit tests
//! cannot check: `run::check` returns an `Exit`, and whether `main` turns that into a process status is
//! a separate question with its own way of going wrong.
//!
//! ⚠ **The fixture is a neutral project**, not any particular game's. A tool's tests naming a specific
//! title would put that title into a public repository, and the CLI has no opinion about which project
//! it is pointed at.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A valid schematic — one hook, one array, one return.
const SCHEMATIC: &str = "\
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

/// A minimal project skeleton on disk.
fn skeleton(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cv-cli-e2e-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("content/schematics")).unwrap();
    std::fs::write(
        dir.join("game.cvproj"),
        r#"{"cyclevania":"0.2.0","worldScale":1.0,"paths":{"contentRoot":"content"}}"#,
    )
    .unwrap();
    std::fs::write(dir.join("content/schematics/hookshot.cvs"), SCHEMATIC).unwrap();
    dir
}

/// The `cv` binary cargo just built.
fn cv() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cv"))
}

fn run(args: &[&str]) -> Output {
    Command::new(cv())
        .args(args)
        .output()
        .expect("the cv binary runs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

fn project(dir: &Path) -> String {
    dir.join("game.cvproj").display().to_string()
}

#[test]
fn check_runs_against_a_project_skeleton_and_exits_zero() {
    let dir = skeleton("check");
    let out = run(&["check", &project(&dir)]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    assert!(stdout(&out).contains("schematics: 1"));
}

#[test]
fn generate_runs_and_reports_a_recipe_and_a_roll() {
    let dir = skeleton("generate");
    let out = run(&["generate", &project(&dir), "--seed", "world-42"]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("seed: world-42"));
    assert!(text.contains("fingerprint: "));
    assert!(text.contains("world: "));
}

#[test]
fn determinism_runs_the_soak_and_exits_zero() {
    let dir = skeleton("determinism");
    let out = run(&["determinism", &project(&dir), "--seeds", "24"]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    assert!(stdout(&out).contains("seeds: 24"));
}

#[test]
fn trace_runs_and_names_the_seed() {
    let dir = skeleton("trace");
    let out = run(&["trace", &project(&dir), "--seed", "the-flooded-wing"]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    assert!(stdout(&out).contains("the-flooded-wing"));
}

#[test]
fn invalid_content_exits_two_and_a_missing_project_exits_one() {
    // ⚠ The two codes exist to say whose fault it is: the project's, or the person running the tool's.
    let dir = skeleton("codes");
    std::fs::write(
        dir.join("content/schematics/broken.cvs"),
        SCHEMATIC.replace("\"grants\"", "\"grnts\""),
    )
    .unwrap();
    let invalid = run(&["check", &project(&dir)]);
    assert_eq!(code(&invalid), 2, "{}", stdout(&invalid));
    assert!(stdout(&invalid).contains("grnts"));

    let missing = run(&["check", "/definitely/not/here/game.cvproj"]);
    assert_eq!(code(&missing), 1, "{}", stdout(&missing));
}

#[test]
fn generate_on_invalid_content_exits_two_rather_than_three() {
    // ⚠ The project is at fault, not the generator — and a CI job branching on `3` must not catch this.
    let dir = skeleton("gen-invalid");
    std::fs::write(
        dir.join("content/schematics/broken.cvs"),
        SCHEMATIC.replace("Op=array.make", "Op=array.mak"),
    )
    .unwrap();
    let out = run(&["generate", &project(&dir), "--seed", "s"]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
}

#[test]
fn json_output_is_one_object_a_machine_can_read() {
    let dir = skeleton("json");
    let out = run(&["check", &project(&dir), "--json"]);
    assert_eq!(code(&out), 0);
    let text = stdout(&out);
    let parsed = cv_assets::json::parse(text.trim()).expect("valid JSON");
    assert_eq!(
        parsed
            .get("command")
            .and_then(cv_assets::json::Json::as_str),
        Some("check")
    );
    assert_eq!(
        parsed.get("code").and_then(cv_assets::json::Json::as_f64),
        Some(0.0)
    );
}

#[test]
fn json_output_survives_a_failure_so_a_redirect_still_captures_the_answer() {
    // ⚠ A tool that split findings onto stderr would make `cv check --json > out.json` produce a file
    // missing exactly the part a reader wanted.
    let dir = skeleton("json-fail");
    std::fs::write(
        dir.join("content/schematics/broken.cvs"),
        SCHEMATIC.replace("\"grants\"", "\"grnts\""),
    )
    .unwrap();
    let out = run(&["check", &project(&dir), "--json"]);
    assert_eq!(code(&out), 2);

    let text = stdout(&out);
    let parsed = cv_assets::json::parse(text.trim()).expect("still valid JSON on a failure");
    let findings = parsed
        .get("findings")
        .and_then(cv_assets::json::Json::as_array)
        .expect("findings are in the object");
    assert!(!findings.is_empty(), "the findings came with it");
}

#[test]
fn the_same_seed_generates_the_same_world_across_two_processes() {
    // ⚠ Two processes rather than two calls: a generator whose determinism depended on warm state
    // would pass an in-process check and fail here.
    let dir = skeleton("cross-process");
    let a = run(&["generate", &project(&dir), "--seed", "world-42", "--json"]);
    let b = run(&["generate", &project(&dir), "--seed", "world-42", "--json"]);
    assert_eq!(stdout(&a), stdout(&b));
}

#[test]
fn no_subcommand_is_help_rather_than_an_error() {
    let out = run(&[]);
    assert_eq!(code(&out), 0);
    assert!(stdout(&out).contains("--help"));
}
