# MLTI vs Concurrently — Feature Gap Analysis

A comparison of [concurrently](https://github.com/open-cli-tools/concurrently) (the Node.js original) against MLTI (this Rust port), based on concurrently's current documentation.

---

## ✅ Features MLTI Already Has

| Feature | Concurrently | MLTI | Notes |
|---------|:---:|:---:|-------|
| Run multiple commands in parallel | ✅ | ✅ | Core functionality |
| `--kill-others` / `-k` | ✅ | ✅ | Kill all when one exits |
| `--kill-others-on-fail` | ✅ | ✅ | Kill all when one exits non-zero |
| `--raw` / `-r` | ✅ | ✅ | Raw output, no prefixes/colors |
| `--no-color` | ✅ | ✅ | Disable ANSI colors |
| `--group` / `-g` | ✅ | ✅ | Buffer output, print per-process sequentially |
| `--max-processes` / `-m` | ✅ | ✅ | Limit concurrency; MLTI also supports `%` of CPUs |
| `--names` / `-n` | ✅ | ✅ | Custom process names |
| `--names-separator` | ✅ | ✅ | Custom delimiter for names |
| `--prefix` / `-p` | ✅ | ✅ | Prefix template with `{index}`, `{command}`, `{name}`, `{pid}`, `{time}`, `{none}` |
| `--prefix-length` / `-l` | ✅ | ✅ | Truncate long prefixes |
| `--timestamp-format` / `-t` | ✅ | ✅ | Custom time format in prefix |
| `--restart-tries` | ✅ | ✅ | Retry failed process spawns |
| `--restart-after` | ✅ | ✅ | Delay between retries (ms) |
| `npm:` shortcut | ✅ | ✅ | `npm:foo` → `npm run foo` |
| `pnpm:` shortcut | ✅ | ✅ | `pnpm:foo` → `pnpm foo` (note: concurrently expands to `pnpm run`) |
| Random prefix colors | ✅ | ✅ | MLTI uses random RGB; concurrently uses `auto` |

---

## ❌ Features MLTI Is Missing

### Output Control

| Feature | Difficulty | Description |
|---------|:---:|-------------|
| **`--hide`** | 🟢 Easy | Hide output from specific processes by index/name. Concurrently accepts a comma-separated list of indices. Would need a filter check in `Messenger` before printing. |
| **`--timings`** | 🟢 Easy | Print timing info (start time, end time, duration) for each process after it exits. The data is mostly already available — just needs formatting and a `CloseEvent`-style summary. |
| **`--pad-prefix`** | 🟢 Easy | Pad all prefixes to the same length so output columns align. Just need to find the max prefix width and left-pad shorter ones. |
| **stderr capture** | 🟡 Medium | MLTI only captures `stdout`. Concurrently captures both `stdout` and `stderr`. The `Process` struct needs `Stdio::piped()` on stderr and a second reader in `Task`. |

### Success Conditions

| Feature | Difficulty | Description |
|---------|:---:|-------------|
| **`--success`** | 🟡 Medium | Controls the exit code of the `mlti` process itself. Values: `all` (default — all must succeed), `first`, `last`, `command-{name}`, `command-{index}`, `!command-{name}`, `!command-{index}`. Currently MLTI always exits 0. Would need exit code tracking per task and a final evaluation step. |

### Termination Control

| Feature | Difficulty | Description |
|---------|:---:|-------------|
| **`--kill-signal`** | 🟢 Easy | Choose which signal to send when killing processes (`SIGTERM` vs `SIGKILL`). Currently MLTI just drops/aborts. On Unix, would use `nix::sys::signal` or `libc`. |
| **`--kill-timeout`** | 🟡 Medium | After sending the kill signal, wait N ms and then force-kill with `SIGKILL`. Requires a timed escalation mechanism. |

### Restart Enhancements

| Feature | Difficulty | Description |
|---------|:---:|-------------|
| **Exponential backoff** (`--restart-after exponential`) | 🟢 Easy | `2^N` second delay between retries instead of fixed. Just a calculation change in the retry loop. |
| **Restart on non-zero exit** (not just spawn failure) | 🟡 Medium | Concurrently restarts commands that *exit* with non-zero, not just ones that fail to spawn. MLTI's current restart logic only handles spawn failures. The `Task::start` loop needs restructuring. |

### Input Handling

| Feature | Difficulty | Description |
|---------|:---:|-------------|
| **`--handle-input` / `-i`** | 🔴 Hard | Read from stdin and forward to a child process. Support targeting by index (`0:input`) or name (`server:input`). Requires stdin multiplexing and writing to child stdin handles. |
| **`--default-input-target`** | 🔴 Hard | Set which process receives input by default. Depends on `--handle-input`. |

### Command Shortcuts

| Feature | Difficulty | Description |
|---------|:---:|-------------|
| **`yarn:` shortcut** | 🟢 Easy | `yarn:foo` → `yarn run foo`. Already partially referenced in `get_name()` but not in `expand()`. |
| **`bun:` shortcut** | 🟢 Easy | `bun:foo` → `bun run foo`. |
| **`node:` shortcut** | 🟢 Easy | `node:foo` → `node --run foo`. |
| **`deno:` shortcut** | 🟢 Easy | `deno:foo` → `deno task foo`. |
| **Wildcard matching** (`npm:build:*`) | 🔴 Hard | Reads `package.json` scripts, globs against them, and spawns matching commands. Requires filesystem + JSON parsing + glob matching. |
| **Wildcard exclusion** (`npm:lint:*(!fix)`) | 🔴 Hard | Filter out matches from wildcard expansion. |

### Prefix Colors

| Feature | Difficulty | Description |
|---------|:---:|-------------|
| **`--prefix-colors` / `-c`** | 🟡 Medium | Explicit per-process colors instead of random. Supports named colors (`red`, `blue`), hex (`#23de43`), modifiers (`.bold`, `.dim`, `.italic`), background colors (`bgRed`), and `auto`. |

### Passthrough Arguments

| Feature | Difficulty | Description |
|---------|:---:|-------------|
| **`--passthrough-arguments` / `-P`** | 🟡 Medium | Everything after `--` is captured and substituted into commands via `{1}`, `{@}`, `{*}` placeholders. Useful for wrapper scripts. Requires argument parsing changes and template expansion. |

### Configuration

| Feature | Difficulty | Description |
|---------|:---:|-------------|
| **Environment variable config** | 🟢 Easy | `CONCURRENTLY_KILL_OTHERS=true` sets defaults. Read `MLTI_*` env vars at startup and merge with CLI args. |
| **Per-command `cwd`** | 🟡 Medium | Each command can have its own working directory. Would need a syntax or config file. |
| **Per-command `env`** | 🟡 Medium | Each command can have custom environment variables. |

---

## 🐛 Bugs / Inconsistencies to Fix

| Issue | Description |
|-------|-------------|
| **`pnpm:` expansion** | MLTI expands `pnpm:foo` → `pnpm foo`. Concurrently expands it to `pnpm run foo`. Should add `run`. |
| **`restart_tries` vs `restart_after` confusion** | In `Task::start`, the retry loop initializes `restart_attemps = self.mlti_config.restart_after - 1` but it should be using `restart_tries`. `restart_after` is the *delay*, not the count. This is a bug. |
| **No stderr** | Child stderr is not piped at all, so error output from subprocesses is lost. |
| **Exit code always 0** | MLTI always returns `Ok(())` regardless of child exit codes. |
| **No exit code reporting** | Concurrently prints `command exited with code N` for each process. MLTI only prints "Done!". |

---

## 📋 Suggested Priority Order

If you want to chip away at parity, here's a reasonable order that balances impact and difficulty:

1. **Fix the bugs above** — `restart_tries`/`restart_after` mixup, stderr capture, pnpm expansion
2. **Exit code reporting** — print exit codes like concurrently does
3. **`--success` flag** — makes MLTI usable in CI/scripts
4. **`--hide`** — simple filter, high utility
5. **`--timings`** — easy win, nice UX
6. **Missing shortcuts** — `yarn:`, `bun:`, `node:`, `deno:`
7. **`--prefix-colors`** — replace random colors with user-controlled ones
8. **`--kill-signal` / `--kill-timeout`** — proper process termination
9. **`--passthrough-arguments`** — useful for npm script wrappers
10. **`--handle-input`** — hardest feature, but completes the picture
