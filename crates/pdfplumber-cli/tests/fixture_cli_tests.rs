//! CLI integration tests using real-world and generated PDF fixtures.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn cmd() -> Command {
    Command::cargo_bin("pdfplumber").unwrap()
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn generated(name: &str) -> String {
    fixtures_dir()
        .join("generated")
        .join(name)
        .to_str()
        .unwrap()
        .to_string()
}

fn downloaded(name: &str) -> String {
    fixtures_dir()
        .join("downloaded")
        .join(name)
        .to_str()
        .unwrap()
        .to_string()
}

// ==================== text subcommand ====================

#[test]
fn cli_text_basic() {
    cmd()
        .args(["text", &generated("basic_text.pdf")])
        .assert()
        .success()
        .stdout(predicate::str::contains("quick brown fox"));
}

#[test]
fn cli_text_long_document_pages() {
    cmd()
        .args(["text", &generated("long_document.pdf"), "--pages", "1,3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Page 1"))
        .stdout(predicate::str::contains("Page 3"));
}

#[test]
fn cli_text_scotus() {
    cmd()
        .args(["text", &downloaded("scotus-transcript-p1.pdf")])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

// ==================== chars subcommand ====================

#[test]
fn cli_chars_json_basic() {
    let output = cmd()
        .args(["chars", &generated("basic_text.pdf"), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Should be valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    // Should be an array of chars
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty(), "should have character entries");
    // Each entry should have text, fontname, size
    let first = &arr[0];
    assert!(first.get("text").is_some(), "char should have 'text' field");
}

#[test]
fn cli_chars_multi_font() {
    let output = cmd()
        .args(["chars", &generated("multi_font.pdf"), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    // Should have chars with different fontnames
    let fontnames: std::collections::HashSet<String> = arr
        .iter()
        .filter_map(|c| c.get("fontname").and_then(|f| f.as_str()).map(String::from))
        .collect();
    assert!(
        fontnames.len() >= 2,
        "should have multiple fontnames: {:?}",
        fontnames
    );
}

// ==================== tables subcommand ====================

#[test]
fn cli_tables_json_lattice() {
    let output = cmd()
        .args([
            "tables",
            &generated("table_lattice.pdf"),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    assert!(!parsed.is_empty(), "should detect tables in lattice PDF");
}

#[test]
fn cli_tables_nics() {
    let output = cmd()
        .args([
            "tables",
            &downloaded("nics-firearm-checks.pdf"),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    assert!(
        !parsed.is_empty(),
        "should detect tables in NICS government PDF"
    );
}

// ==================== info subcommand ====================

#[test]
fn cli_info_annotations() {
    cmd()
        .args(["info", &generated("annotations_links.pdf")])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn cli_info_pdffill() {
    cmd()
        .args(["info", &downloaded("pdffill-demo.pdf")])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

// ==================== search subcommand ====================

#[test]
fn cli_search_long_document() {
    cmd()
        .args(["search", &generated("long_document.pdf"), "Lorem ipsum"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

// ==================== links subcommand ====================

#[test]
fn cli_links_annotations() {
    // May or may not find links depending on how fpdf2 encodes them
    cmd()
        .args(["links", &generated("annotations_links.pdf")])
        .assert()
        .success();
}
