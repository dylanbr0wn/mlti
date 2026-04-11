# Stdin Forwarding Design

**Issue:** #23 — Implement `--handle-input` / `-i` and `--default-input-target`
**Date:** 2026-04-11
**Approach:** Shared `Arc<InputRouter>` with per-task registration

## Overview

Read from stdin and forward input lines to child processes. Users target specific processes by prefixing input with the process name or index, separated by `:`. Unprefixed input goes to a configurable default target.

```bash
mlti -i -n server,worker "node server.js" "node worker.js"
# Type "server:restart" → sends "restart" to the server process
# Type "hello"          → sends "hello" to process 0 (default target)
```

## CLI Flags

Two new flags on the `Commands` struct (via `argh`):

| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--handle-input` | `-i` | `bool` | `false` | Enable stdin forwarding to child processes |
| `--default-input-target` | — | `Option<String>` | `None` | Which process receives unprefixed input (name or index) |

**Behavior rules:**

- When `--handle-input` is off and `--default-input-target` is absent, stdin remains `Stdio::null()` for all children (current behavior, unchanged).
- `--default-input-target` implicitly enables input handling — no need to also pass `-i`.
- When input handling is on but `--default-input-target` is not specified, the default target is index `0` (first process), matching concurrently's behavior.

**Config changes:** `MltiConfig` gains `handle_input: bool` and `default_input_target: Option<String>`. During construction, if `default_input_target.is_some()`, `handle_input` is forced to `true`.

## InputRouter

New module: `src/input_router.rs`.

### Struct

```rust
pub struct InputRouter {
    handles: tokio::sync::Mutex<HashMap<usize, ChildStdin>>,
    names: Vec<String>,
    num_processes: usize,
    default_target: usize,
    message_tx: Sender<Message>,
}
```

Uses `tokio::sync::Mutex` (not `std::sync`) because the lock is held across `.await` points during writes. The entire `InputRouter` is shared via `Arc<InputRouter>`, so no inner `Arc` on `handles` is needed.

### API

- **`InputRouter::new(names, default_target, message_tx)`** — constructs the router.
- **`router.register(index, child_stdin)`** — inserts a stdin handle into the map. Called by each `Task` after spawning.
- **`router.deregister(index)`** — removes and drops the handle, closing the pipe. Called by each `Task` on process exit.
- **`router.route(line)`** — async. Parses the input line, resolves the target, writes to the correct `ChildStdin`.

### Routing Logic (`route()`)

1. Check if the line contains a `:` — split on the **first** colon only.
2. Left side is the candidate target specifier, right side is the input payload.
3. Resolve the target: try name match first (scan `self.names`), then parse as `usize` and check it's within `0..self.num_processes`.
4. If the candidate doesn't resolve to a known name or valid process index, treat the **entire original line** as unprefixed input and send to `self.default_target`. This avoids URLs like `http://localhost:3000` being misrouted, and prevents `99:hello` from being treated as a targeted input when there are only 2 processes.
5. If no `:` in the line, send to `self.default_target`.
6. If the resolved target exists in `handles`, write the input line + `\n` to its `ChildStdin`. Print feedback: `[mlti] -> server: restart`.
7. If the target is unknown or already exited, print: `[mlti] Unknown target "foo", input discarded`.

## Stdin Reader Task

A single tokio task spawned in `main.rs` when `handle_input` is true.

- Uses `tokio::io::BufReader::new(tokio::io::stdin())` with `AsyncBufReadExt::lines()`.
- Each line is passed to `router.route(line).await`.
- On EOF (`Ok(None)`), the task exits naturally.
- On shutdown (all processes exited), the task is aborted via its `JoinHandle`.

No OS thread needed — fully async via `tokio::io::stdin()`.

## Task Integration

### `Process::run()` Changes

- Accepts `handle_input: bool` (from `MltiConfig`).
- When `true`: adds `cmd.stdin(Stdio::piped())` before spawning.
- When `false`: unchanged (`Stdio::null()`).

### `Task` Struct Changes

- Receives `Option<Arc<InputRouter>>` — `Some` when input handling is enabled, `None` otherwise.
- After successful spawn in `Task::start()`:
  1. If router is `Some`, call `child.stdin.take()` to extract the `ChildStdin`.
  2. Call `router.register(self.process.index, stdin_handle)`.
  3. Proceed with stdout/stderr reading as normal.
- On process exit (after the `select!` loop and exit code capture):
  1. Call `router.deregister(self.process.index)`.

### Restart Behavior

When `--restart-tries` > 0, the existing retry loop naturally handles re-registration:
- Deregister at the top of each retry iteration (no-op on first iteration).
- Register the new `ChildStdin` after a successful re-spawn.

## Wiring in `main.rs`

### Setup Phase (after CLI parsing, before task dispatch)

1. Resolve `default_input_target`:
   - If provided: try name match in `arg_parser.names` first, then parse as `usize`. If neither resolves, print error and exit.
   - If not provided: default to `0`.
2. If `handle_input` is true: construct `Arc::new(InputRouter::new(...))`.
3. If false: `input_router = None`.

### Task Creation Loop

Pass `input_router.clone()` (`Option<Arc<InputRouter>>`) to each `Task::new()`.

### Stdin Reader Spawn

If `input_router` is `Some`, spawn the stdin reader task and hold its `JoinHandle`.

### Shutdown

When all processes have exited (shutdown signal received), abort the stdin reader `JoinHandle`. Existing exit logic proceeds unchanged.

## Error Handling & Edge Cases

| Scenario | Behavior |
|----------|----------|
| Write to `ChildStdin` fails (process died mid-write) | Catch error, deregister handle, print warning: `[mlti] Failed to send input to "server" (process exited)` |
| Empty input line | Forward as-is (newline only). Some programs use blank lines. |
| Ambiguous prefix (`http://localhost:3000`) | Target `http` doesn't resolve as name or index → treat entire line as unprefixed, send to default target |
| Multiple colons (`server:key:value`) | Split on first `:` only → target `server`, input `key:value` |
| All processes exited, user types input | Print: `[mlti] No running processes, input discarded` |
| `--default-input-target` specifies unknown name/index | Print error at startup and exit (fail fast) |

## What Doesn't Change

- **`Scheduler`** — no changes. Tasks flow through it as before; the router reference is carried by `Task`.
- **`Messenger`** — no changes. The router sends its feedback/warning messages through the existing `message_tx` channel as `SenderType::Main`.
- **`Message` / `MessageType`** — no new variants needed.
- **Output handling** — stdout/stderr capture unchanged.

## Testing Strategy

- Unit tests for `InputRouter`: routing logic, name resolution, index fallback, ambiguous prefix handling, unknown target warnings.
- Unit tests for `route()` parsing: first-colon split, no-colon lines, edge cases.
- Integration test: spawn mlti with `-i`, write to its stdin programmatically, verify child processes receive the input.
- Integration test: verify `--default-input-target` resolution (by name and by index).
- Integration test: verify feedback messages appear in output.
