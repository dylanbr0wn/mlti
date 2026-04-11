# Wildcard Matching for Package Manager Shortcuts

**Issue:** #24 (parent: #5)
**Date:** 2026-04-11

## Overview

Expand wildcard patterns in package manager shortcuts (e.g., `npm:build:*`) by reading script names from `package.json`, glob-matching against them, and spawning one process per match. Supports exclusion patterns (`*(!fix)`) and trailing arguments.

## Scope

### In scope
- Wildcard expansion for `npm:`, `pnpm:`, `yarn:`, `bun:` prefixes
- Glob matching against `package.json` scripts
- Exclusion syntax: `*(!pattern)`
- Trailing arguments appended to each expanded command
- Auto-naming from the matched wildcard portion
- `--manifest-path` flag for specifying a custom `package.json` location
- Warn-and-skip when a pattern matches nothing

### Out of scope (deferred)
- `deno:` support (`deno.json` / `deno.jsonc` manifest format)
- Directory tree walking to find `package.json`

## Architecture

### Approach: Pre-processor before CommandParser

A new `src/command_expander.rs` module runs before `CommandParser::new()`. It takes the raw process list and names list, detects wildcard patterns, reads `package.json`, expands wildcards into concrete command strings, and returns expanded vectors. `CommandParser` and everything downstream sees only normal command strings — no changes needed.

### Public API

```rust
pub fn expand_commands(
    processes: Vec<String>,
    names: Vec<String>,
    manifest_path: Option<String>,
) -> Result<(Vec<String>, Vec<String>)>
```

### Integration point in main.rs

Called after `argh::from_env()` but before `CommandParser::new()`:

```rust
let commands: Commands = argh::from_env();
let (processes, names) = expand_commands(
    commands.processes,
    parse_names(commands.names, commands.names_seperator),
    commands.manifest_path,
)?;
// Pass expanded processes and names into CommandParser
```

`CommandParser::new()` signature adjusts to accept pre-parsed `Vec<String>` for both processes and names instead of raw `Option<String>` for names.

## Pattern Parsing

A wildcard command string like `npm:build:* --verbose` is parsed into three parts:

1. **Manager prefix** — `npm:` (determines the runner command)
2. **Glob pattern** — `build:*` (matched against script names)
3. **Trailing args** — `--verbose` (appended to each expanded command)

Parsing: split on the first space to separate the shortcut from trailing args, then strip the manager prefix to get the raw glob pattern.

### Manager prefix mapping

| Prefix | Expanded command |
|--------|-----------------|
| `npm:` | `npm run <script>` |
| `pnpm:` | `pnpm run <script>` |
| `yarn:` | `yarn run <script>` |
| `bun:` | `bun run <script>` |

### Wildcard detection

A command is a wildcard pattern if it starts with a known manager prefix AND the script portion contains `*`.

## Glob Matching

Use the `globset` crate to match patterns against script names from `package.json`.

### Exclusion syntax

`*(!fix)` is not standard glob syntax. Handled by:

1. Detect the `(!...)` suffix in the pattern
2. Strip it to get the base glob (e.g., `lint:*`)
3. Construct the exclusion glob by replacing `*(!...)` in the original pattern with `{exclusion_content}*` — so `lint:*(!fix)` produces exclusion glob `lint:fix*`
4. A script matches if: base glob matches AND exclusion glob does NOT match

This means `lint:*(!fix)` matches `lint:js` and `lint:ts` but excludes `lint:fix`, `lint:fix:js`, and `lint:fix:ts` — the exclusion content acts as a prefix filter on the wildcard portion.

### Auto-naming

For pattern `build:*` matching script `build:client`, the auto-name is `client` — the portion matched by `*`. Derived by stripping the literal prefix of the glob pattern (everything before the first `*`) from the matched script name.

## Name Resolution

Names are resolved per-position in this order:

1. Explicit `--names` value at that position (if provided)
2. Auto-generated name from the wildcard match portion
3. Index number (existing fallback for non-wildcard commands)

### Example

```bash
mlti -n server "node server.js" "npm:build:*"
```

With `package.json` scripts `build:client` and `build:server`:

- Expanded processes: `["node server.js", "npm run build:client", "npm run build:server"]`
- Expanded names: `["server", "client", "server"]`
  - Position 0: from `--names`
  - Positions 1-2: auto-generated from wildcard match

When a wildcard pattern at position N expands to M commands, it consumes name positions N through N+M-1 from `--names`. Positions without an explicit name get auto-names.

## Manifest File Reading

### Locating the manifest

- Default: `package.json` in the current working directory
- `--manifest-path <path>`: use the specified path exactly
- No directory walking

### Parsing

Use `serde_json` to deserialize only the `scripts` field:

```rust
#[derive(Deserialize)]
struct PackageJson {
    #[serde(default)]
    scripts: HashMap<String, String>,
}
```

Read lazily: only when at least one wildcard pattern is detected. Read once and reuse for multiple wildcard commands.

### New CLI flag

`--manifest-path` added to the `Commands` struct. Optional, no short flag.

## Error Handling

| Condition | Behavior |
|-----------|----------|
| No manifest file found (wildcards present) | Hard error, exit with message |
| Invalid JSON in manifest | Hard error, exit with message |
| No `scripts` key in manifest | Warn, skip the pattern, continue |
| Wildcard matches zero scripts | Warn, skip the pattern, continue |
| Invalid glob pattern (e.g., unclosed bracket) | Hard error, exit with message |

## New Dependencies

- `serde` (with derive feature) — deserialization framework
- `serde_json` — JSON parsing for `package.json`
- `globset` — glob pattern matching

## Testing Strategy

### Unit tests (in `command_expander.rs`)

Core logic takes a `HashMap<String, String>` (scripts) and a pattern, returns matches. No filesystem needed.

- Basic glob: `build:*` matches `build:client`, `build:server`, not `test:unit`
- Exclusion: `lint:*(!fix)` matches `lint:js`, `lint:ts`, not `lint:fix`
- Trailing args: `build:* --verbose` appends `--verbose` to each expanded command
- Name generation: correct auto-names from matched portions
- Name precedence: explicit `--names` override auto-names positionally
- No matches: returns empty expansion for that pattern
- All four manager prefixes expand correctly
- Non-wildcard commands pass through unchanged
- Mixed wildcard and non-wildcard commands in the same invocation

### Integration test

Write a temporary `package.json`, run the full `expand_commands` function, verify output vectors.
