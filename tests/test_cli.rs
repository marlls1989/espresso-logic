//! Integration tests for the `espresso` CLI binary.
//!
//! These drive the *built* binary (`CARGO_BIN_EXE_espresso`) end-to-end via `std::process::Command`,
//! covering behaviours the shell regression harness does not: the `-O`/`-x`/`-s` flags, error exit
//! codes, and the exact (`-D exact`) path. The whole file is gated on the `cli` feature, since the
//! binary is `required-features = ["cli"]`.
#![cfg(feature = "cli")]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const ESPRESSO: &str = env!("CARGO_BIN_EXE_espresso");

/// Write `content` to a uniquely-named temp PLA file and return its path. The name is keyed on the
/// test-supplied `tag` so concurrently-running tests never collide.
fn temp_pla(tag: &str, content: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("espresso_cli_{}_{}.pla", std::process::id(), tag));
    fs::write(&path, content).expect("write temp PLA");
    path
}

/// f(a,b) = !a  (covers minterms 00, 01) — reduces from two cubes to the single cube `0-`.
const REDUCIBLE: &str = ".i 2\n.o 1\n00 1\n01 1\n.e\n";

#[test]
fn writes_output_file_with_out_flag() {
    let input = temp_pla("out_in", REDUCIBLE);
    let out = std::env::temp_dir().join(format!("espresso_cli_{}_out.pla", std::process::id()));

    let status = Command::new(ESPRESSO)
        .arg("-O")
        .arg(&out)
        .arg(&input)
        .status()
        .expect("run espresso");
    assert!(status.success(), "exit: {status:?}");

    let written = fs::read_to_string(&out).expect("read -O output");
    // The written file is a well-formed PLA and reflects the minimised result (one cube).
    assert!(written.contains(".i 2"), "missing .i in:\n{written}");
    assert!(written.contains(".o 1"), "missing .o in:\n{written}");
    assert!(
        written.contains("0-") || written.contains("0 1"),
        "unexpected:\n{written}"
    );

    let _ = fs::remove_file(&input);
    let _ = fs::remove_file(&out);
}

#[test]
fn no_output_flag_suppresses_solution() {
    let input = temp_pla("noout", REDUCIBLE);
    let output = Command::new(ESPRESSO)
        .arg("-x")
        .arg(&input)
        .output()
        .expect("run espresso");
    assert!(output.status.success());
    // `-x` suppresses the solution: stdout carries no cube rows.
    assert!(
        output.stdout.is_empty(),
        "stdout not suppressed: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let _ = fs::remove_file(&input);
}

#[test]
fn summary_flag_prints_to_stderr() {
    let input = temp_pla("summary", REDUCIBLE);
    let output = Command::new(ESPRESSO)
        .arg("-s")
        .arg(&input)
        .output()
        .expect("run espresso");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // The summary reports cube/dimension counts on stderr.
    assert!(stderr.contains("inputs"), "no summary on stderr:\n{stderr}");
    let _ = fs::remove_file(&input);
}

#[test]
fn missing_input_file_exits_nonzero() {
    let status = Command::new(ESPRESSO)
        .arg("/no/such/espresso_input.pla")
        .status()
        .expect("run espresso");
    assert!(!status.success(), "expected non-zero exit for missing file");
}

#[test]
fn exact_command_runs_and_minimises() {
    let input = temp_pla("exact", REDUCIBLE);
    let output = Command::new(ESPRESSO)
        .arg("-D")
        .arg("exact")
        .arg(&input)
        .output()
        .expect("run espresso");
    assert!(output.status.success(), "exact exit: {:?}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Exact minimisation of !a yields the single cube `0-` (one product term).
    assert!(
        stdout.contains("0-"),
        "exact output missing `0-`:\n{stdout}"
    );
    let _ = fs::remove_file(&input);
}

#[test]
fn exact_flag_is_alias_for_exact_command() {
    // `-e` must select the exact algorithm (equivalent to `-D exact`), not merely toggle fast mode.
    let input = temp_pla("exact_flag", REDUCIBLE);
    let via_flag = Command::new(ESPRESSO)
        .arg("-e")
        .arg(&input)
        .output()
        .expect("run espresso -e");
    let via_command = Command::new(ESPRESSO)
        .args(["-D", "exact"])
        .arg(&input)
        .output()
        .expect("run espresso -D exact");
    assert!(via_flag.status.success() && via_command.status.success());
    assert_eq!(
        via_flag.stdout, via_command.stdout,
        "`-e` and `-D exact` should produce identical output"
    );
    let _ = fs::remove_file(&input);
}
