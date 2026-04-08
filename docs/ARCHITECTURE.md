# MLTI Architecture Overview

MLTI is a concurrent process runner for the command line — a Rust port of Node's [concurrently](https://github.com/open-cli-tools/concurrently). You give it multiple shell commands, and it runs them in parallel, with colorized, prefixed output.

```
mlti "echo hello" "echo world" "sleep 2 && echo done"
```

## High-Level Flow

```
┌──────────┐     ┌───────────┐     ┌───────────────┐     ┌───────────┐
│  main.rs │────▶│ Scheduler │────▶│  Task (×N)    │────▶│  Process  │
│ (CLI +   │     │           │     │  (per command) │     │  (spawn)  │
│  wiring) │     └─────┬─────┘     └───────┬───────┘     └───────────┘
└────┬─────┘           │                   │
     │                 │                   │
     │           kill_all channel    message channels
     │                 │                   │
     ▼                 ▼                   ▼
┌──────────────────────────────────────────────┐
│              Messenger (×2)                  │
│  - one for output (text/errors)              │
│  - one for shutdown signals                  │
└──────────────────────────────────────────────┘
```

## Module Responsibilities

| File | Struct/Role | What it does |
|------|-------------|-------------|
| `main.rs` | `Commands`, `MltiConfig`, `CommandParser` | Parses CLI args (via `argh`), builds config, wires everything together, runs the event loop |
| `scheduler.rs` | `Scheduler` | Controls how many processes run concurrently (respects `--max-processes`). Receives `Task`s on a channel, spawns them in a `JoinSet`, and tracks completion. |
| `task.rs` | `Task` | Owns one `Process`. Starts it, reads its stdout line-by-line, sends each line as a `Message`. Handles restart logic and kill-others behavior. |
| `command.rs` | `Process` | Parses a raw command string, expands npm/pnpm shortcuts, builds and spawns a `tokio::process::Command`. Also computes the display name/prefix. |
| `message.rs` | `Message`, `MessageType`, `MessageSender` | Data types for the internal message-passing protocol. Messages carry a type (Text, Error, Kill, KillAll, etc.), a name, data, a color style, and sender metadata. |
| `messenger.rs` | `Messenger` | Receives `Message`s on a `flume` channel and prints them. Supports `--group` mode (buffers output per-process, flushes at the end) and `--raw` mode (only process stdout, no decoration). |

## Channel Topology

MLTI uses **`flume`** (an MPMC channel library) for all inter-task communication. There are three logical channels:

1. **`message_tx` / message channel** — Tasks send their stdout lines and status updates here. The output `Messenger` listens and prints them.

2. **`shutdown_tx` / shutdown channel** — Carries control signals (`KillAll`, `KillOthers`, `KillAllOnError`, `Complete`). The shutdown `Messenger` in `main` listens here and orchestrates graceful termination.

3. **`task_queue` (tasks_tx/tasks_rx)** — `main` sends `Task` structs to the `Scheduler`, which pulls them off and spawns them when capacity allows.

4. **`kill_all` (kill_all_tx/kill_all_rx)** — A simple `()` signal to tell the `Scheduler` to abort all running tasks immediately.

## Async Runtime

The project uses **Tokio** as the async runtime (`#[tokio::main]`). Key async patterns:

- `JoinSet` in the scheduler to manage a dynamic set of spawned task futures
- `tokio::select!` to race between "a task completed" and "kill signal received"
- `tokio::process::Command` for non-blocking child process spawning
- `AsyncBufReadExt::lines()` for streaming stdout line-by-line

## Configuration & CLI Flags

All CLI args are parsed by `argh` into the `Commands` struct, then normalized into `MltiConfig` by `CommandParser`. Notable flags:

| Flag | Effect |
|------|--------|
| `-k` / `--kill-others` | If any process exits, kill all others |
| `--kill-others-on-fail` | Kill all if a process exits with non-zero |
| `-r` / `--raw` | Print only raw process output (no prefixes/colors) |
| `--no-color` | Disable ANSI color output |
| `-g` / `--group` | Buffer output per-process, print sequentially at end |
| `-m` / `--max-processes` | Limit concurrent processes (supports `%` of CPU count) |
| `-n` / `--names` | Custom names for processes |
| `-p` / `--prefix` | Prefix template (`{index}`, `{command}`, `{name}`, `{pid}`, `{time}`) |
| `--restart-tries` / `--restart-after` | Retry failed process starts |
| `-t` / `--timestamp-format` | `chrono` format string for `{time}` prefix |
