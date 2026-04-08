# MLTI Data Flow — A Walkthrough

This traces what happens when you run:

```bash
mlti -k "echo hello" "sleep 1 && echo world"
```

## 1. Startup & Parsing (`main.rs`)

1. `argh` parses CLI args into `Commands { kill_others: true, processes: ["echo hello", "sleep 1 && echo world"], ... }`
2. `CommandParser::new()` normalizes this into `MltiConfig` + a list of process strings and names
3. Two `Messenger` instances are created:
   - **`messenger`** — for printing process output (text, errors)
   - **`shutdown_messenger`** — for handling control signals (kill, complete)
4. A `Scheduler` is created with `max_processes = i32::MAX` (no limit specified)
5. A `Ctrl+C` handler is registered that sends `KillAll` on the shutdown channel

## 2. Task Creation Loop (`main.rs`)

For each command string:
1. A random RGB color is generated (`rand::thread_rng()`)
2. A `Process` is created — the raw command is parsed/expanded, a display name is computed
3. A `Task` wrapping that `Process` is sent to the `Scheduler` via `task_queue`

## 3. Scheduling (`scheduler.rs`)

The `Scheduler::run()` loop:
1. Reads tasks from the channel
2. Checks if it can run more (respects `max_processes` limit)
3. Spawns each task into a `JoinSet` (Tokio's managed set of futures)
4. Uses `tokio::select!` to wait for either:
   - A task completing → decrements counter, checks if all done
   - A kill signal → calls `join_set.shutdown().await` to abort everything

## 4. Task Execution (`task.rs`)

Each `Task::start()`:
1. Calls `Process::run()` to spawn the child process
2. If spawn fails, retries based on `restart_tries` / `restart_after` config
3. Takes the child's stdout handle and wraps it in a `BufReader::lines()`
4. Reads each line and sends it as a `Message { type_: Text, ... }` on the message channel
5. Awaits the child's exit status
6. Sends a "Done!" message
7. Since `-k` is set: sends `KillOthers` on the shutdown channel

## 5. Message Printing (`messenger.rs`)

The output `Messenger` runs in its own `tokio::spawn`'d task:
- Receives `Message`s and calls the handler closure
- For `Text`/`Error`: calls `print_message()` which formats as `[name]: data` with colors
- For `Kill`: breaks the listen loop

## 6. Shutdown Sequence

When the first process (`echo hello`) finishes and `kill_others` is true:
1. `Task` sends `KillOthers` → shutdown channel
2. `shutdown_messenger` in `main` receives it:
   - Prints "Kill others flag present, stopping other processes."
   - Sends `Kill` → message channel (tells output messenger to stop)
   - Sends `()` → kill_all channel (tells scheduler to abort)
3. Scheduler receives kill signal, shuts down the `JoinSet` (kills remaining children)
4. Output messenger receives `Kill`, breaks its loop
5. `main` awaits both handles, prints "Goodbye! 👋"

## Message Type Summary

```
MessageType::Text          — a line of stdout from a process
MessageType::Error         — an error from a process or task
MessageType::Kill          — tells a Messenger to stop listening
MessageType::KillAll       — Ctrl+C was pressed; kill everything
MessageType::KillOthers    — a process exited and --kill-others is set
MessageType::KillAllOnError— a process failed and --kill-others-on-fail is set
MessageType::Complete      — all tasks finished naturally
```

## Group Mode (`--group`)

When `--group` is enabled, the output `Messenger` doesn't print immediately. Instead:
- Each message is pushed into a per-process `VecDeque`
- When `Kill` is received, it calls `flush()` which drains each queue in order
- This means process 0's output prints first (in full), then process 1's, etc.
