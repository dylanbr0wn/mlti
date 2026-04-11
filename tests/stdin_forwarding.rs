use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// A child process whose stdin is piped and whose stdout/stderr are
/// drained by background threads into a shared buffer. We control
/// startup-ordering via a `MLTI_READY` marker emitted by each wrapped
/// child, so tests never depend on wall-clock sleeps.
const READY_MARKER: &str = "MLTI_READY";

/// Write a tiny shell script to $CARGO_TARGET_TMPDIR and return its
/// absolute path. mlti's own command parser splits on whitespace
/// without respecting quotes (see `src/command.rs::parse`), so we
/// can't pass `sh -c 'echo MLTI_READY; exec cat'` as a single
/// positional — the shell would see mangled argv. Using a concrete
/// file path sidesteps the parser entirely.
fn write_helper_script(name: &str, body: &str) -> String {
  // $CARGO_TARGET_TMPDIR is guaranteed writable for integration
  // tests; fall back to `std::env::temp_dir()` for older toolchains.
  let dir = option_env!("CARGO_TARGET_TMPDIR")
    .map(std::path::PathBuf::from)
    .unwrap_or_else(std::env::temp_dir);
  std::fs::create_dir_all(&dir).expect("create target tmpdir");
  let path = dir.join(name);
  std::fs::write(&path, body).expect("write helper script");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).expect("chmod helper script");
  }
  path.to_string_lossy().into_owned()
}

/// Path to a helper script that prints the ready marker and then
/// exec's cat. Initialized once per test binary.
fn ready_cat_path() -> &'static str {
  static PATH: OnceLock<String> = OnceLock::new();
  PATH
    .get_or_init(|| {
      write_helper_script(
        "mlti_ready_cat.sh",
        &format!("#!/bin/sh\necho {}\nexec cat\n", READY_MARKER),
      )
    })
    .as_str()
}

/// Path to a helper script that prints the ready marker and then
/// exits cleanly — used to verify that routing keeps working after
/// one task has deregistered its handle.
fn ready_exit_path() -> &'static str {
  static PATH: OnceLock<String> = OnceLock::new();
  PATH
    .get_or_init(|| {
      write_helper_script(
        "mlti_ready_exit.sh",
        &format!("#!/bin/sh\necho {}\nexit 0\n", READY_MARKER),
      )
    })
    .as_str()
}

/// Helper: spawn mlti with the given args, wait until `num_ready`
/// ready markers appear on its output (one per wrapped child), then
/// write `stdin_lines` and wait for mlti to exit (or time out).
///
/// Returns all captured stdout+stderr as a single string.
fn run_mlti(
  args: &[&str],
  num_ready: usize,
  stdin_lines: &[&str],
  timeout: Duration,
) -> String {
  let mut child = Command::new(env!("CARGO_BIN_EXE_mlti"))
    .args(args)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("failed to spawn mlti");

  let stdout = child.stdout.take().expect("stdout piped");
  let stderr = child.stderr.take().expect("stderr piped");

  // Collected output lives behind an Arc<Mutex<_>> so the reader
  // threads and the main test thread can both touch it.
  let collected: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
  let (line_tx, line_rx) = mpsc::channel::<String>();

  let stdout_handle = {
    let tx = line_tx.clone();
    let col = collected.clone();
    std::thread::spawn(move || {
      for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        col.lock().unwrap().push(line.clone());
        let _ = tx.send(line);
      }
    })
  };
  let stderr_handle = {
    let tx = line_tx;
    let col = collected.clone();
    std::thread::spawn(move || {
      for line in BufReader::new(stderr).lines().map_while(Result::ok) {
        col.lock().unwrap().push(line.clone());
        let _ = tx.send(line);
      }
    })
  };

  // Wait for the expected number of ready markers.
  let start = Instant::now();
  let mut seen = 0usize;
  while seen < num_ready {
    let remaining = match timeout.checked_sub(start.elapsed()) {
      Some(r) if !r.is_zero() => r,
      _ => {
        let _ = child.kill();
        let _ = child.wait();
        let _ = stdout_handle.join();
        let _ = stderr_handle.join();
        let out = collected.lock().unwrap().join("\n");
        panic!(
          "timed out waiting for {} {} marker(s) (saw {}). Output:\n{}",
          num_ready, READY_MARKER, seen, out
        );
      }
    };
    match line_rx.recv_timeout(remaining) {
      Ok(line) if line.contains(READY_MARKER) => seen += 1,
      Ok(_) => continue,
      Err(_) => {
        // Both reader threads have closed their senders → mlti died
        // before we saw enough markers. Surface whatever we captured.
        let _ = child.kill();
        let _ = child.wait();
        let _ = stdout_handle.join();
        let _ = stderr_handle.join();
        let out = collected.lock().unwrap().join("\n");
        panic!(
          "mlti exited before emitting {} ready marker(s) (saw {}). Output:\n{}",
          num_ready, seen, out
        );
      }
    }
  }

  // All children are registered; send input and close stdin so mlti
  // and its children unwind cleanly.
  if let Some(mut stdin) = child.stdin.take() {
    for line in stdin_lines {
      writeln!(stdin, "{}", line).expect("failed to write to stdin");
    }
    // stdin dropped here → EOF cascade
  }

  // Wait for mlti to exit (or deadline).
  let deadline = start + timeout;
  loop {
    match child.try_wait() {
      Ok(Some(_)) => break,
      Ok(None) => {
        if Instant::now() >= deadline {
          let _ = child.kill();
          break;
        }
        std::thread::sleep(Duration::from_millis(25));
      }
      Err(_) => break,
    }
  }
  let _ = child.wait();

  // Join reader threads so the collected buffer is complete before
  // we read it. The threads exit naturally once their pipes EOF.
  let _ = stdout_handle.join();
  let _ = stderr_handle.join();

  let guard = collected.lock().unwrap();
  guard.join("\n")
}

#[test]
fn stdin_forwarding_default_target() {
  let output = run_mlti(
    &["-i", ready_cat_path()],
    1,
    &["hello world"],
    Duration::from_secs(10),
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
    &["-i", "-n", "mycat", ready_cat_path()],
    1,
    &["mycat:test message"],
    Duration::from_secs(10),
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
fn stdin_forwarding_unknown_target_fallthrough() {
  // "unknown" doesn't match any process → entire line routed to default
  let output = run_mlti(
    &["-i", "-n", "mycat", ready_cat_path()],
    1,
    &["unknown:data"],
    Duration::from_secs(10),
  );
  assert!(
    output.contains("unknown:data"),
    "Expected full line sent to default. Got: {}",
    output
  );
}

#[test]
fn default_input_target_implies_handle_input() {
  let output = run_mlti(
    &["--default-input-target", "0", ready_cat_path()],
    1,
    &["hello"],
    Duration::from_secs(10),
  );
  assert!(
    output.contains("hello"),
    "Expected --default-input-target to imply --handle-input. Got: {}",
    output
  );
}

#[test]
fn stdin_forwarding_url_not_misrouted() {
  // A URL like "http://localhost:3000" must not be parsed as target "http".
  let output = run_mlti(
    &["-i", "-n", "mycat", ready_cat_path()],
    1,
    &["http://localhost:3000"],
    Duration::from_secs(10),
  );
  assert!(
    output.contains("http://localhost:3000"),
    "Expected URL to be sent as-is to default target. Got: {}",
    output
  );
}

#[test]
fn stdin_forwarding_multiple_colons() {
  // "mycat:key:value" → target="mycat", payload="key:value"
  let output = run_mlti(
    &["-i", "-n", "mycat", ready_cat_path()],
    1,
    &["mycat:key:value"],
    Duration::from_secs(10),
  );
  assert!(
    output.contains("key:value"),
    "Expected payload with colon to be forwarded. Got: {}",
    output
  );
  // And the payload should *not* be prefixed by "mycat:" — the router
  // strips that off before writing to the child.
  assert!(
    !output.contains("mycat:key:value"),
    "Target prefix leaked into child payload. Got: {}",
    output
  );
}

#[test]
fn stdin_forwarding_empty_line() {
  let output = run_mlti(
    &["-i", "-n", "mycat", ready_cat_path()],
    1,
    &[""],
    Duration::from_secs(10),
  );
  // Empty lines must route to the default target (not panic, not
  // discard) so downstream tools that consume blank lines still work.
  assert!(
    output.contains("[mlti] -> mycat:"),
    "Expected routing feedback for empty line. Got: {}",
    output
  );
  assert!(
    !output.contains("panic"),
    "Empty line must not cause a panic. Got: {}",
    output
  );
}

#[test]
fn stdin_forwarding_by_index() {
  let output = run_mlti(
    &[
      "-i",
      "-n",
      "first,second",
      ready_cat_path(),
      ready_cat_path(),
    ],
    2,
    &["1:targeted"],
    Duration::from_secs(10),
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
    &[
      "--default-input-target",
      "second",
      "-n",
      "first,second",
      ready_cat_path(),
      ready_cat_path(),
    ],
    2,
    &["hello"],
    Duration::from_secs(10),
  );
  assert!(
    output.contains("[mlti] -> second: hello"),
    "Expected unprefixed input routed to 'second'. Got: {}",
    output
  );
}

#[test]
fn stdin_forwarding_routes_only_to_surviving_process() {
  // One child exits immediately after emitting the ready marker; the
  // other stays alive. We wait for both markers so we know both tasks
  // have reached the register→stdout-reader phase, then confirm that
  // targeting the surviving process still works, even though the
  // other task has already deregistered its handle.
  let output = run_mlti(
    &[
      "-i",
      "-n",
      "dying,alive",
      ready_exit_path(),
      ready_cat_path(),
    ],
    2,
    &["alive:still here"],
    Duration::from_secs(10),
  );
  assert!(
    output.contains("[mlti] -> alive: still here"),
    "Expected routing to the surviving process. Got: {}",
    output
  );
  assert!(
    output.contains("still here"),
    "Expected surviving child to echo payload. Got: {}",
    output
  );
}
