# Stdin Forwarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Forward stdin input to child processes with prefix-based routing (`name:input` or `index:input`), controlled by `--handle-input` / `-i` and `--default-input-target` flags.

**Architecture:** A new `InputRouter` module owns a `tokio::sync::Mutex<HashMap<usize, ChildStdin>>`. Each `Task` registers its child's stdin handle after spawning and deregisters on exit. A dedicated tokio task reads lines from `tokio::io::stdin()` and delegates to the router for parsing and delivery.

**Tech Stack:** Rust, tokio (async stdin, Mutex), argh (CLI flags), flume (message channels), owo-colors (output formatting)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/input_router.rs` | Create | `InputRouter` struct: owns stdin handles, parses input lines, resolves targets, writes to child stdin |
| `src/main.rs` | Modify | Add CLI flags, `MltiConfig` fields, `InputRouter` construction, stdin reader task spawn, shutdown abort |
| `src/command.rs` | Modify | Accept `handle_input: bool` in `Process::run()`, conditionally pipe stdin |
| `src/task.rs` | Modify | Accept `Option<Arc<InputRouter>>`, register/deregister stdin handles around process lifecycle |

---

### Task 1: Add CLI Flags and Config Fields

**Files:**
- Modify: `src/main.rs:34-101` (Commands struct), `src/main.rs:103-116` (MltiConfig struct), `src/main.rs:126-146` (CommandParser::new)

- [ ] **Step 1: Add `handle_input` and `default_input_target` to the `Commands` struct**

In `src/main.rs`, add two new fields to the `Commands` struct, after the `success` field (line 100):

```rust
  /// enable stdin forwarding to child processes
  #[argh(switch, short = 'i')]
  handle_input: bool,

  /// set which process receives input by default (name or index). Implies --handle-input.
  #[argh(option)]
  default_input_target: Option<String>,
```

- [ ] **Step 2: Add `handle_input` to `MltiConfig`**

In `src/main.rs`, add a new field to the `MltiConfig` struct (after `timestamp_format` at line 115):

```rust
  pub handle_input: bool,
```

- [ ] **Step 3: Wire the new fields in `CommandParser::new()`**

In `src/main.rs` inside `CommandParser::new()`, update the `MltiConfig` construction (lines 132-144). After the `timestamp_format` field, add:

```rust
        handle_input: commands.handle_input || commands.default_input_target.is_some(),
```

Also store `default_input_target` on `CommandParser`. Add a new field to the `CommandParser` struct (line 118-123):

```rust
pub struct CommandParser {
  pub names: Vec<String>,
  pub processes: Vec<String>,
  pub mlti_config: MltiConfig,
  pub default_input_target: Option<String>,
  success_condition: SuccessCondition,
}
```

And in `CommandParser::new()`, set it:

```rust
    Ok(Self {
      names: parse_names(commands.names, commands.names_seperator),
      processes: commands.processes,
      default_input_target: commands.default_input_target,
      success_condition,
      mlti_config: MltiConfig {
        group: commands.group,
        kill_others: commands.kill_others,
        kill_others_on_fail: commands.kill_others_on_fail,
        restart_tries: commands.restart_tries,
        restart_after: commands.restart_after,
        prefix: commands.prefix,
        prefix_length: commands.prefix_length,
        max_processes: parse_max_processes(commands.max_processes),
        raw: commands.raw,
        no_color: commands.no_color,
        timestamp_format: commands.timestamp_format,
        handle_input: commands.handle_input || commands.default_input_target.is_some(),
      },
    })
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build 2>&1`
Expected: Compiles successfully with no errors.

- [ ] **Step 5: Run existing tests**

Run: `cargo test 2>&1`
Expected: All existing tests pass (the new fields have no effect yet).

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: add --handle-input and --default-input-target CLI flags"
```

---

### Task 2: Implement the `InputRouter` — Target Resolution Logic

**Files:**
- Create: `src/input_router.rs`

- [ ] **Step 1: Write tests for target resolution and line parsing**

Create `src/input_router.rs` with the struct, constructor, a `resolve_target` helper, and tests:

```rust
use std::collections::HashMap;

use flume::Sender;
use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdin;
use tokio::sync::Mutex;

use crate::message::{build_message_sender, Message, MessageType, SenderType};

pub struct InputRouter {
    handles: Mutex<HashMap<usize, ChildStdin>>,
    names: Vec<String>,
    num_processes: usize,
    default_target: usize,
    message_tx: Sender<Message>,
}

/// The result of parsing an input line.
struct ParsedInput {
    /// The resolved process index to send to.
    target: usize,
    /// The payload to write to the child's stdin.
    payload: String,
    /// Display name for feedback messages.
    target_name: String,
}

impl InputRouter {
    pub fn new(
        names: Vec<String>,
        num_processes: usize,
        default_target: usize,
        message_tx: Sender<Message>,
    ) -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            names,
            num_processes,
            default_target,
            message_tx,
        }
    }

    /// Try to resolve a candidate string to a process index.
    /// Returns Some(index) if the candidate matches a name or is a valid index
    /// within 0..num_processes. Returns None otherwise.
    fn resolve_target(&self, candidate: &str) -> Option<usize> {
        // Try name match first
        if let Some(idx) = self.names.iter().position(|n| n == candidate) {
            return Some(idx);
        }
        // Fall back to index
        if let Ok(idx) = candidate.parse::<usize>() {
            if idx < self.num_processes {
                return Some(idx);
            }
        }
        None
    }

    /// Parse an input line into a target index and payload.
    fn parse_line(&self, line: &str) -> ParsedInput {
        if let Some(colon_pos) = line.find(':') {
            let candidate = &line[..colon_pos];
            if let Some(idx) = self.resolve_target(candidate) {
                let payload = line[colon_pos + 1..].to_string();
                let target_name = self.display_name(idx);
                return ParsedInput {
                    target: idx,
                    payload,
                    target_name,
                };
            }
        }
        // No colon or unresolved prefix — send whole line to default
        let target_name = self.display_name(self.default_target);
        ParsedInput {
            target: self.default_target,
            payload: line.to_string(),
            target_name,
        }
    }

    /// Get a display name for a process index.
    fn display_name(&self, index: usize) -> String {
        self.names
            .get(index)
            .cloned()
            .unwrap_or_else(|| index.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_router(names: Vec<&str>, num_processes: usize, default_target: usize) -> InputRouter {
        let (tx, _rx) = flume::unbounded();
        InputRouter::new(
            names.into_iter().map(String::from).collect(),
            num_processes,
            default_target,
            tx,
        )
    }

    // -- resolve_target --

    #[test]
    fn resolve_by_name() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        assert_eq!(router.resolve_target("server"), Some(0));
        assert_eq!(router.resolve_target("worker"), Some(1));
    }

    #[test]
    fn resolve_by_index() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        assert_eq!(router.resolve_target("0"), Some(0));
        assert_eq!(router.resolve_target("1"), Some(1));
    }

    #[test]
    fn resolve_name_takes_priority_over_index() {
        // Process named "1" should match by name (index 0), not by index 1
        let router = make_router(vec!["1", "worker"], 2, 0);
        assert_eq!(router.resolve_target("1"), Some(0));
    }

    #[test]
    fn resolve_unknown_name_returns_none() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        assert_eq!(router.resolve_target("unknown"), None);
    }

    #[test]
    fn resolve_out_of_range_index_returns_none() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        assert_eq!(router.resolve_target("99"), None);
    }

    // -- parse_line --

    #[test]
    fn parse_line_with_valid_name_prefix() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        let parsed = router.parse_line("server:restart");
        assert_eq!(parsed.target, 0);
        assert_eq!(parsed.payload, "restart");
        assert_eq!(parsed.target_name, "server");
    }

    #[test]
    fn parse_line_with_valid_index_prefix() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        let parsed = router.parse_line("1:hello");
        assert_eq!(parsed.target, 1);
        assert_eq!(parsed.payload, "hello");
        assert_eq!(parsed.target_name, "worker");
    }

    #[test]
    fn parse_line_no_colon_goes_to_default() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        let parsed = router.parse_line("hello world");
        assert_eq!(parsed.target, 0);
        assert_eq!(parsed.payload, "hello world");
    }

    #[test]
    fn parse_line_unresolved_prefix_goes_to_default() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        let parsed = router.parse_line("http://localhost:3000");
        assert_eq!(parsed.target, 0);
        assert_eq!(parsed.payload, "http://localhost:3000");
    }

    #[test]
    fn parse_line_multiple_colons_splits_on_first() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        let parsed = router.parse_line("server:key:value");
        assert_eq!(parsed.target, 0);
        assert_eq!(parsed.payload, "key:value");
    }

    #[test]
    fn parse_line_out_of_range_index_goes_to_default() {
        let router = make_router(vec!["server", "worker"], 2, 1);
        let parsed = router.parse_line("99:hello");
        assert_eq!(parsed.target, 1);
        assert_eq!(parsed.payload, "99:hello");
    }

    #[test]
    fn parse_line_empty_line_goes_to_default() {
        let router = make_router(vec!["server"], 1, 0);
        let parsed = router.parse_line("");
        assert_eq!(parsed.target, 0);
        assert_eq!(parsed.payload, "");
    }

    #[test]
    fn parse_line_with_non_default_target() {
        let router = make_router(vec!["server", "worker"], 2, 1);
        let parsed = router.parse_line("hello");
        assert_eq!(parsed.target, 1);
        assert_eq!(parsed.payload, "hello");
    }

    #[test]
    fn display_name_with_named_process() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        assert_eq!(router.display_name(0), "server");
        assert_eq!(router.display_name(1), "worker");
    }

    #[test]
    fn display_name_falls_back_to_index() {
        let router = make_router(vec![], 2, 0);
        assert_eq!(router.display_name(0), "0");
        assert_eq!(router.display_name(1), "1");
    }
}
```

- [ ] **Step 2: Register the module in `main.rs`**

In `src/main.rs`, add after line 16 (`mod task;`):

```rust
mod input_router;
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test input_router 2>&1`
Expected: All 13 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/input_router.rs src/main.rs
git commit -m "feat: add InputRouter with target resolution and line parsing"
```

---

### Task 3: Implement `InputRouter::route()` and `register`/`deregister`

**Files:**
- Modify: `src/input_router.rs`

- [ ] **Step 1: Add `register`, `deregister`, and `route` methods**

In `src/input_router.rs`, add these methods to the `impl InputRouter` block, after the `display_name` method:

```rust
    pub async fn register(&self, index: usize, stdin: ChildStdin) {
        self.handles.lock().await.insert(index, stdin);
    }

    pub async fn deregister(&self, index: usize) {
        self.handles.lock().await.remove(&index);
    }

    pub async fn route(&self, line: &str) {
        let parsed = self.parse_line(line);
        let mut handles = self.handles.lock().await;

        if handles.is_empty() {
            self.send_message("[mlti] No running processes, input discarded".to_string());
            return;
        }

        match handles.get_mut(&parsed.target) {
            Some(stdin) => {
                let data = format!("{}\n", parsed.payload);
                if let Err(_) = stdin.write_all(data.as_bytes()).await {
                    // Process died between lookup and write
                    drop(handles);
                    self.deregister(parsed.target).await;
                    self.send_message(format!(
                        "[mlti] Failed to send input to \"{}\" (process exited)",
                        parsed.target_name
                    ));
                    return;
                }
                if let Err(_) = stdin.flush().await {
                    drop(handles);
                    self.deregister(parsed.target).await;
                    self.send_message(format!(
                        "[mlti] Failed to send input to \"{}\" (process exited)",
                        parsed.target_name
                    ));
                    return;
                }
                self.send_message(format!(
                    "[mlti] -> {}: {}",
                    parsed.target_name, parsed.payload
                ));
            }
            None => {
                self.send_message(format!(
                    "[mlti] Unknown target \"{}\", input discarded",
                    parsed.target_name
                ));
            }
        }
    }

    fn send_message(&self, data: String) {
        self.message_tx
            .send(Message::new(
                MessageType::Text,
                Some("".to_string()),
                Some(data),
                None,
                build_message_sender(SenderType::Main, None, None),
            ))
            .expect("Could not send message on channel.");
    }
```

- [ ] **Step 2: Add a test for `register` and `deregister`**

Add this test to the `mod tests` block in `src/input_router.rs`:

```rust
    #[tokio::test]
    async fn register_and_deregister() {
        let router = make_router(vec!["server"], 1, 0);
        // Initially empty
        assert!(router.handles.lock().await.is_empty());

        // We can't easily create a real ChildStdin in a test, but we can
        // verify the map operations work by checking the handles map size
        // after deregister of a non-existent key (no panic).
        router.deregister(0).await;
        assert!(router.handles.lock().await.is_empty());
    }
```

- [ ] **Step 3: Run all tests**

Run: `cargo test input_router 2>&1`
Expected: All tests pass (14 total).

- [ ] **Step 4: Commit**

```bash
git add src/input_router.rs
git commit -m "feat: add InputRouter route, register, and deregister methods"
```

---

### Task 4: Modify `Process::run()` to Conditionally Pipe Stdin

**Files:**
- Modify: `src/command.rs:44-51`

- [ ] **Step 1: Add `handle_input` parameter to `Process::run()`**

In `src/command.rs`, change the `run` method signature and body (lines 44-51):

```rust
  pub fn run(&self, handle_input: bool) -> Result<Child, std::io::Error> {
    let mut cmd = tokio::process::Command::new(self.cmd.clone());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    if handle_input {
      cmd.stdin(Stdio::piped());
    }
    cmd.args(self.args.clone());

    cmd.spawn()
  }
```

- [ ] **Step 2: Update the call site in `task.rs`**

In `src/task.rs`, line 58, change:

```rust
      let attempt_child = self.process.run();
```

to:

```rust
      let attempt_child = self.process.run(self.mlti_config.handle_input);
```

- [ ] **Step 3: Verify it compiles and tests pass**

Run: `cargo test 2>&1`
Expected: All tests pass. No behavior change — `handle_input` defaults to `false`.

- [ ] **Step 4: Commit**

```bash
git add src/command.rs src/task.rs
git commit -m "feat: conditionally pipe stdin when --handle-input is set"
```

---

### Task 5: Integrate `InputRouter` into `Task` (Register/Deregister)

**Files:**
- Modify: `src/task.rs:13-35` (struct + constructor), `src/task.rs:52-223` (start method)

- [ ] **Step 1: Add `InputRouter` to the `Task` struct**

In `src/task.rs`, add the import at the top (after line 9):

```rust
use crate::input_router::InputRouter;
use std::sync::Arc;
```

Update the `Task` struct (lines 13-19) to add the router field:

```rust
pub(crate) struct Task {
  process: Process,
  message_tx: Sender<Message>,
  shutdown_tx: Sender<Message>,
  mlti_config: MltiConfig,
  input_router: Option<Arc<InputRouter>>,
  exit_code: Option<i32>,
}
```

Update `Task::new()` (lines 22-35) to accept and store the router:

```rust
  pub fn new(
    process: Process,
    message_tx: Sender<Message>,
    shutdown_tx: Sender<Message>,
    mlti_config: MltiConfig,
    input_router: Option<Arc<InputRouter>>,
  ) -> Self {
    Self {
      process,
      message_tx,
      shutdown_tx,
      mlti_config,
      input_router,
      exit_code: None,
    }
  }
```

- [ ] **Step 2: Register stdin after spawn in `Task::start()`**

In `src/task.rs`, after the `stderr` handle is taken (after line 122), add the stdin registration:

```rust
    // Register stdin with the input router if enabled
    if let Some(ref router) = self.input_router {
      if let Some(stdin) = child.stdin.take() {
        router.register(self.process.index, stdin).await;
      }
    }
```

- [ ] **Step 3: Deregister on process exit**

In `src/task.rs`, after `self.exit_code = Some(code);` (after line 193), add:

```rust
    // Deregister stdin from the input router
    if let Some(ref router) = self.input_router {
      router.deregister(self.process.index).await;
    }
```

- [ ] **Step 4: Update the call site in `main.rs`**

In `src/main.rs`, update the `Task::new()` call (lines 404-409). For now, pass `None` as the router — we'll wire it in Task 7:

```rust
    task_queue
      .send_async(Task::new(
        my_cmd,
        message_tx.clone(),
        shutdown_tx.clone(),
        mlti_config.to_owned(),
        None, // input_router — wired in Task 7
      ))
      .await
      .expect("Could not send task on channel.");
```

- [ ] **Step 5: Verify it compiles and tests pass**

Run: `cargo test 2>&1`
Expected: All tests pass. No behavior change yet — router is always `None`.

- [ ] **Step 6: Commit**

```bash
git add src/task.rs src/main.rs
git commit -m "feat: integrate InputRouter into Task for stdin registration"
```

---

### Task 6: Handle Restart Re-registration

**Files:**
- Modify: `src/task.rs:52-99` (the retry loop in `start()`)

- [ ] **Step 1: Add deregister at the top of the retry loop**

In `src/task.rs`, at the very beginning of the `loop` body (after `loop {` on line 57), add:

```rust
      // Deregister any previous stdin handle (no-op on first iteration)
      if let Some(ref router) = self.input_router {
        router.deregister(self.process.index).await;
      }
```

- [ ] **Step 2: Move stdin registration inside the retry loop after successful spawn**

In the retry loop, when a spawn succeeds (the `Ok(c)` arm, lines 60-63), register stdin before breaking:

Change:
```rust
        Ok(c) => {
          child = Some(c);
          break;
        }
```

To:
```rust
        Ok(mut c) => {
          // Register stdin with the input router if enabled
          if let Some(ref router) = self.input_router {
            if let Some(stdin) = c.stdin.take() {
              router.register(self.process.index, stdin).await;
            }
          }
          child = Some(c);
          break;
        }
```

- [ ] **Step 3: Remove the duplicate registration after the retry loop**

Remove the stdin registration block that was added in Task 5, Step 2 (the one after stderr is taken, around line 122-126). The registration now happens inside the retry loop instead, which correctly handles both first-spawn and restart cases.

- [ ] **Step 4: Verify it compiles and tests pass**

Run: `cargo test 2>&1`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/task.rs
git commit -m "feat: handle stdin re-registration on process restart"
```

---

### Task 7: Wire Everything in `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports**

In `src/main.rs`, add at the top (after the existing `use` statements, around line 10):

```rust
use std::sync::Arc;
use crate::input_router::InputRouter;
```

- [ ] **Step 2: Add the `resolve_default_target` helper function**

Add this function before `main()` (e.g., after `parse_max_processes` around line 271):

```rust
fn resolve_default_target(
    target: &Option<String>,
    names: &[String],
    num_processes: usize,
) -> usize {
    match target {
        None => 0,
        Some(t) => {
            // Try name match first
            if let Some(idx) = names.iter().position(|n| n == t) {
                return idx;
            }
            // Fall back to index
            if let Ok(idx) = t.parse::<usize>() {
                if idx < num_processes {
                    return idx;
                }
            }
            eprintln!(
                "Error: --default-input-target \"{}\" does not match any process name or index",
                t
            );
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 3: Construct the `InputRouter` in `main()`**

In `src/main.rs`, after the scheduler is created and before the task loop (after line 381, before line 387), add:

```rust
  let input_router: Option<Arc<InputRouter>> = if mlti_config.handle_input {
    let default_target = resolve_default_target(
      &arg_parser.default_input_target,
      &arg_parser.names,
      arg_parser.len(),
    );
    Some(Arc::new(InputRouter::new(
      arg_parser.names.clone(),
      arg_parser.len(),
      default_target,
      message_tx.clone(),
    )))
  } else {
    None
  };
```

- [ ] **Step 4: Pass the router to each `Task`**

Update the `Task::new()` call in the loop (replacing the `None` from Task 5):

```rust
    task_queue
      .send_async(Task::new(
        my_cmd,
        message_tx.clone(),
        shutdown_tx.clone(),
        mlti_config.to_owned(),
        input_router.clone(),
      ))
      .await
      .expect("Could not send task on channel.");
```

- [ ] **Step 5: Spawn the stdin reader task**

After the task dispatch loop (after line 412), add the stdin reader:

```rust
  let stdin_reader_handle = if let Some(ref router) = input_router {
    let router = router.clone();
    Some(tokio::spawn(async move {
      let stdin = tokio::io::stdin();
      let mut reader = tokio::io::BufReader::new(stdin).lines();
      while let Ok(Some(line)) = reader.next_line().await {
        router.route(&line).await;
      }
    }))
  } else {
    None
  };
```

- [ ] **Step 6: Abort the stdin reader on shutdown**

In the shutdown messenger's `listen` closure, we need access to the stdin reader handle to abort it. Since the closure already runs to completion before `messenger_handle.await`, add the abort right after the `shutdown_messenger.listen(...)` call returns (after line 513, before `messenger_handle.await`):

```rust
  // Abort the stdin reader task if it's running
  if let Some(handle) = stdin_reader_handle {
    handle.abort();
  }
```

- [ ] **Step 7: Add the `AsyncBufReadExt` import**

In `src/main.rs`, add at the top with the other imports:

```rust
use tokio::io::AsyncBufReadExt;
```

- [ ] **Step 8: Verify it compiles**

Run: `cargo build 2>&1`
Expected: Compiles successfully.

- [ ] **Step 9: Run all tests**

Run: `cargo test 2>&1`
Expected: All tests pass.

- [ ] **Step 10: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire InputRouter, stdin reader task, and shutdown in main"
```

---

### Task 8: Integration Test — Basic Stdin Forwarding

**Files:**
- Create: `tests/stdin_forwarding.rs`

- [ ] **Step 1: Add `wait-timeout` dev dependency**

In `Cargo.toml`, add a `[dev-dependencies]` section at the end:

```toml
[dev-dependencies]
wait-timeout = "0.2"
```

- [ ] **Step 2: Write the integration tests**

Create `tests/stdin_forwarding.rs`:

```rust
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
```

- [ ] **Step 3: Run the integration tests**

Run: `cargo test --test stdin_forwarding 2>&1`
Expected: All 4 tests pass. Note: these tests need the binary built, so the first run may take longer.

- [ ] **Step 5: Commit**

```bash
git add tests/stdin_forwarding.rs Cargo.toml
git commit -m "test: add integration tests for stdin forwarding"
```

---

### Task 9: Integration Test — Edge Cases

**Files:**
- Modify: `tests/stdin_forwarding.rs`

- [ ] **Step 1: Add edge case tests**

Append these tests to `tests/stdin_forwarding.rs`:

```rust
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
```

- [ ] **Step 2: Run all integration tests**

Run: `cargo test --test stdin_forwarding 2>&1`
Expected: All 9 tests pass.

- [ ] **Step 3: Run the full test suite**

Run: `cargo test 2>&1`
Expected: All unit tests and integration tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/stdin_forwarding.rs
git commit -m "test: add edge case integration tests for stdin forwarding"
```

---

### Task 10: Update FEATURE_GAP.md

**Files:**
- Modify: `docs/FEATURE_GAP.md`

- [ ] **Step 1: Read the current FEATURE_GAP.md**

Read `docs/FEATURE_GAP.md` to find the `--handle-input` and `--default-input-target` entries.

- [ ] **Step 2: Update the entries to mark them as implemented**

Find the rows for `--handle-input` and `--default-input-target` and update their status from unimplemented to implemented (follow whatever convention the file uses for marking features as done).

- [ ] **Step 3: Commit**

```bash
git add docs/FEATURE_GAP.md
git commit -m "docs: mark --handle-input and --default-input-target as implemented"
```
