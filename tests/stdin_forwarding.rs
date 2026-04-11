use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

/// Helper: spawn mlti with the given args, write lines to its stdin,
/// wait for it to exit, and return stdout+stderr as a single string.
fn run_mlti(args: &[&str], stdin_lines: &[&str], timeout: Duration) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mlti"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn mlti");

    // Give mlti and its child processes a moment to start and register
    // their stdin handles with the input router before we send input.
    std::thread::sleep(Duration::from_millis(500));

    if let Some(mut stdin) = child.stdin.take() {
        for line in stdin_lines {
            writeln!(stdin, "{}", line).expect("failed to write to stdin");
        }
        // stdin is dropped here, closing the pipe
    }

    let status = child
        .wait_timeout(timeout)
        .expect("failed to wait on mlti");

    if status.is_none() {
        child.kill().expect("failed to kill mlti");
    }

    let output = child.wait_with_output().expect("failed to get output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("{}{}", stdout, stderr)
}

#[test]
fn stdin_forwarding_default_target() {
    // Use `cat` which echoes stdin back to stdout
    let output = run_mlti(
        &["-i", "cat"],
        &["hello world"],
        Duration::from_secs(5),
    );
    assert!(
        output.contains("hello world"),
        "Expected child to echo input. Got: {}",
        output
    );
}

#[test]
fn stdin_forwarding_by_name() {
    let output = run_mlti(
        &["-i", "-n", "mycat", "cat"],
        &["mycat:test message"],
        Duration::from_secs(5),
    );
    assert!(
        output.contains("test message"),
        "Expected child to receive targeted input. Got: {}",
        output
    );
    assert!(
        output.contains("[mlti] -> mycat: test message"),
        "Expected routing feedback. Got: {}",
        output
    );
}

#[test]
fn stdin_forwarding_unknown_target_warning() {
    let output = run_mlti(
        &["-i", "-n", "mycat", "cat"],
        &["unknown:data"],
        Duration::from_secs(5),
    );
    // "unknown" doesn't match any process — entire line goes to default
    assert!(
        output.contains("unknown:data"),
        "Expected full line sent to default. Got: {}",
        output
    );
}

#[test]
fn default_input_target_implies_handle_input() {
    let output = run_mlti(
        &["--default-input-target", "0", "cat"],
        &["hello"],
        Duration::from_secs(5),
    );
    assert!(
        output.contains("hello"),
        "Expected --default-input-target to imply --handle-input. Got: {}",
        output
    );
}

#[test]
fn stdin_forwarding_url_not_misrouted() {
    // A URL like "http://localhost:3000" should not be parsed as target "http"
    let output = run_mlti(
        &["-i", "-n", "mycat", "cat"],
        &["http://localhost:3000"],
        Duration::from_secs(5),
    );
    // The full URL should arrive at the default target (cat echoes it back)
    assert!(
        output.contains("http://localhost:3000"),
        "Expected URL to be sent as-is to default target. Got: {}",
        output
    );
}

#[test]
fn stdin_forwarding_multiple_colons() {
    // "mycat:key:value" should split as target="mycat", payload="key:value"
    let output = run_mlti(
        &["-i", "-n", "mycat", "cat"],
        &["mycat:key:value"],
        Duration::from_secs(5),
    );
    assert!(
        output.contains("key:value"),
        "Expected payload with colon to be forwarded. Got: {}",
        output
    );
}

#[test]
fn stdin_forwarding_empty_line() {
    let output = run_mlti(
        &["-i", "cat"],
        &[""],
        Duration::from_secs(5),
    );
    // cat should output an empty line — we just verify no crash
    // The output should contain the routing feedback or at minimum not panic
    assert!(
        !output.contains("panic"),
        "Empty line should not cause a panic. Got: {}",
        output
    );
}

#[test]
fn stdin_forwarding_by_index() {
    let output = run_mlti(
        &["-i", "-n", "first,second", "cat", "cat"],
        &["1:targeted"],
        Duration::from_secs(5),
    );
    assert!(
        output.contains("targeted"),
        "Expected index-targeted input to reach child. Got: {}",
        output
    );
    assert!(
        output.contains("[mlti] -> second: targeted"),
        "Expected feedback with resolved name. Got: {}",
        output
    );
}

#[test]
fn default_input_target_by_name() {
    let output = run_mlti(
        &["--default-input-target", "second", "-n", "first,second", "cat", "cat"],
        &["hello"],
        Duration::from_secs(5),
    );
    assert!(
        output.contains("[mlti] -> second: hello"),
        "Expected unprefixed input routed to 'second'. Got: {}",
        output
    );
}
