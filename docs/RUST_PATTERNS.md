# Rust Patterns & Concepts Used in MLTI

A quick reference for the Rust-specific patterns in this codebase — useful for refreshing your memory.

---

## Async / Tokio

**`#[tokio::main]`** — The entry point macro that sets up the Tokio multi-threaded runtime. Used in `main.rs`.

**`tokio::spawn`** — Spawns a future as an independent task on the runtime. Used for the output messenger and the scheduler. Each spawned task runs concurrently.

**`tokio::select!`** — Races multiple futures; the first one to complete "wins" and the others are cancelled. Used in `scheduler.rs` to wait for either a task completion or a kill signal:
```rust
tokio::select! {
    _ = join_set.join_next() => { /* a task finished */ }
    _ = self.kill_all_rx.recv_async() => { /* abort everything */ }
}
```

**`JoinSet`** — A Tokio utility for managing a dynamic set of spawned tasks. The scheduler uses it to spawn each task and then wait for completions one at a time.

**`tokio::process::Command`** — Async version of `std::process::Command`. Spawns child processes without blocking the runtime.

**`AsyncBufReadExt::lines()`** — Async line-by-line reader. Wraps a child's stdout to stream lines as they arrive.

---

## Channels (`flume`)

The project uses **flume** instead of Tokio's built-in channels. Flume provides MPMC (multi-producer, multi-consumer) channels with both sync and async APIs.

```rust
let (tx, rx) = flume::unbounded::<Message>();
// Async send/recv
tx.send_async(msg).await;
let msg = rx.recv_async().await;
// Sync send (used in the Ctrl+C handler, which isn't async)
tx.send(msg);
```

The `Ctrl+C` handler uses the **sync** `.send()` because `ctrlc::set_handler` takes a non-async closure — this is why flume is useful here (Tokio channels don't offer sync send).

---

## Ownership & Borrowing

**`Clone` on `MltiConfig`** — The config is `#[derive(Clone)]` so it can be cheaply shared across tasks. Each `Task` gets its own owned copy.

**`Sender<T>.clone()`** — Channel senders are cloned to give multiple producers their own handle. The underlying channel is shared.

**`Arc<RwLock<i32>>`** — The scheduler's `running_processes` counter is wrapped in `Arc<RwLock<>>` for shared mutable state across async tasks. `Arc` provides shared ownership; `RwLock` provides interior mutability with read/write locking.

**`Option<T>` and `.take()`** — `child.stdout.take()` moves stdout out of the `Child`, leaving `None` behind. This is a common pattern to move a resource out of a struct that you still need to hold onto.

---

## Error Handling

**`anyhow::Result`** — Used throughout for ergonomic error handling. `anyhow` lets you use `?` without defining custom error types.

**`.expect("...")`** — Used on `Result`/`Option` where failure is considered unrecoverable (e.g., channel send failures). Panics with the given message.

**`.ok()`** — Converts `Result<T, E>` to `Option<T>`, discarding the error. Used where failure is acceptable (e.g., `kill_all.send(()).ok()` — if the receiver is already dropped, that's fine).

**`.unwrap_or_default()`** — Returns the value or a default. Used on `reader.next_line().await.unwrap_or_default()` — if reading fails, treat it as `None` (end of stream).

---

## Enums & Pattern Matching

`MessageType` is an enum with 7 variants used as a discriminator for the message protocol:
```rust
match message.type_ {
    MessageType::Text => { /* print it */ }
    MessageType::Kill => { /* stop listening */ }
    MessageType::KillAll => { /* shut everything down */ }
    // ...
}
```

The handler closures return `0` (continue) or `1` (break) — a simple protocol for the `Messenger::listen` loop.

---

## Closures as Arguments

`Messenger::listen` takes a closure parameter:
```rust
pub async fn listen<F>(&mut self, handler: F)
where
    F: Fn(Message, bool, bool) -> usize,
```
This lets `main.rs` define different behavior for the output messenger vs. the shutdown messenger while reusing the same listen loop.

---

## String Processing

**`split_whitespace()`** — Used to tokenize commands into program + args.

**`char_indices().nth(n)`** — Used in `truncate()` for Unicode-safe string truncation (slicing by character count, not byte count).

**Template replacement** — The prefix system uses simple `String::replace` with `{key}` patterns:
```rust
prefix.replace(&format!("{{{}}}", key), &value)
```

---

## Styling (`owo-colors`)

`owo-colors` provides zero-allocation terminal coloring:
```rust
let style = Style::new().truecolor(r, g, b);
format!("{}", text.style(style))
```
Each process gets a random RGB color for its prefix, making it easy to visually distinguish outputs.

---

## Crate Highlights

| Crate | Purpose |
|-------|---------|
| `argh` | Derive-based CLI argument parser (lightweight alternative to `clap`) |
| `tokio` | Async runtime — process spawning, I/O, timers, task management |
| `flume` | MPMC channels with both sync and async APIs |
| `anyhow` | Ergonomic error handling |
| `owo-colors` | Terminal color/style formatting |
| `chrono` + `chrono-humanize` | Timestamp formatting and human-readable durations |
| `num_cpus` | Detects CPU count for `--max-processes` percentage mode |
| `rand` | Random color generation |
| `ctrlc` | Cross-platform Ctrl+C signal handling |
