use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("pdfplumber").unwrap()
}

#[test]
fn help_flag_prints_usage_with_subcommands() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("text"))
        .stdout(predicate::str::contains("chars"))
        .stdout(predicate::str::contains("words"))
        .stdout(predicate::str::contains("tables"));
}

#[test]
fn text_subcommand_help() {
    cmd()
        .args(["text", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("FILE"))
        .stdout(predicate::str::contains("--pages"))
        .stdout(predicate::str::contains("--format"));
}

#[test]
fn chars_subcommand_help() {
    cmd()
        .args(["chars", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("FILE"))
        .stdout(predicate::str::contains("--pages"))
        .stdout(predicate::str::contains("--format"));
}

#[test]
fn words_subcommand_help() {
    cmd()
        .args(["words", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("FILE"))
        .stdout(predicate::str::contains("--pages"))
        .stdout(predicate::str::contains("--format"));
}

#[test]
fn tables_subcommand_help() {
    cmd()
        .args(["tables", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("FILE"))
        .stdout(predicate::str::contains("--pages"))
        .stdout(predicate::str::contains("--format"));
}

#[test]
fn no_args_shows_help() {
    // Running with no subcommand should show usage / error
    cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn text_requires_file_argument() {
    cmd()
        .arg("text")
        .assert()
        .failure()
        .stderr(predicate::str::contains("FILE"));
}
