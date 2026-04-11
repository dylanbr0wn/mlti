# Wildcard Matching Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand wildcard patterns in package manager shortcuts (`npm:build:*`) by glob-matching against `package.json` scripts, supporting exclusion patterns and trailing arguments.

**Architecture:** A new `src/command_expander.rs` module pre-processes command strings before `CommandParser::new()`. It detects wildcard patterns, reads `package.json`, expands matches into concrete commands with auto-generated names, and returns expanded vectors. Everything downstream sees only normal command strings.

**Tech Stack:** Rust, `globset` (glob matching), `serde`/`serde_json` (manifest parsing), `tempfile` (dev, for tests)

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `Cargo.toml` | Modify | Add serde, serde_json, globset, tempfile (dev) |
| `src/command_expander.rs` | Create | Wildcard detection, pattern parsing, glob matching, manifest reading, command expansion |
| `src/main.rs` | Modify | Add `--manifest-path` CLI flag, register command_expander module, wire up expansion before CommandParser, refactor CommandParser::new to accept pre-expanded data, update names type to `Vec<Option<String>>` |

---

### Task 1: Add Dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add serde, serde_json, globset as dependencies and tempfile as dev-dependency**

In `Cargo.toml`, add to `[dependencies]`:

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
globset = "0.4"
```

And add a new section:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: add serde, serde_json, globset deps for wildcard expansion (#24)"
```

---

### Task 2: Pattern Parsing

**Files:**
- Create: `src/command_expander.rs`
- Modify: `src/main.rs` (register module only)

- [ ] **Step 1: Create command_expander.rs with WildcardPattern struct and write failing tests**

Create `src/command_expander.rs`:

```rust
use std::collections::HashMap;

use anyhow::{Context, Result};
use globset::GlobBuilder;
use serde::Deserialize;

/// Known package manager prefixes and their `run` commands.
const MANAGERS: &[(&str, &str)] = &[
    ("npm:", "npm run "),
    ("pnpm:", "pnpm run "),
    ("yarn:", "yarn run "),
    ("bun:", "bun run "),
];

#[cfg_attr(test, derive(Debug, PartialEq))]
struct WildcardPattern {
    runner_prefix: String,
    glob_pattern: String,
    exclusion: Option<String>,
    trailing_args: String,
}

/// Parse a command string into a WildcardPattern if it contains a wildcard.
/// Returns None for non-wildcard commands.
fn parse_wildcard(cmd: &str) -> Option<WildcardPattern> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wildcard_npm_basic() {
        let result = parse_wildcard("npm:build:*");
        assert_eq!(
            result,
            Some(WildcardPattern {
                runner_prefix: "npm run ".to_string(),
                glob_pattern: "build:*".to_string(),
                exclusion: None,
                trailing_args: "".to_string(),
            })
        );
    }

    #[test]
    fn parse_wildcard_pnpm() {
        let result = parse_wildcard("pnpm:test:*");
        assert_eq!(
            result,
            Some(WildcardPattern {
                runner_prefix: "pnpm run ".to_string(),
                glob_pattern: "test:*".to_string(),
                exclusion: None,
                trailing_args: "".to_string(),
            })
        );
    }

    #[test]
    fn parse_wildcard_yarn() {
        let result = parse_wildcard("yarn:lint:*");
        assert_eq!(
            result,
            Some(WildcardPattern {
                runner_prefix: "yarn run ".to_string(),
                glob_pattern: "lint:*".to_string(),
                exclusion: None,
                trailing_args: "".to_string(),
            })
        );
    }

    #[test]
    fn parse_wildcard_bun() {
        let result = parse_wildcard("bun:build:*");
        assert_eq!(
            result,
            Some(WildcardPattern {
                runner_prefix: "bun run ".to_string(),
                glob_pattern: "build:*".to_string(),
                exclusion: None,
                trailing_args: "".to_string(),
            })
        );
    }

    #[test]
    fn parse_wildcard_with_exclusion() {
        let result = parse_wildcard("npm:lint:*(!fix)");
        assert_eq!(
            result,
            Some(WildcardPattern {
                runner_prefix: "npm run ".to_string(),
                glob_pattern: "lint:*".to_string(),
                exclusion: Some("fix".to_string()),
                trailing_args: "".to_string(),
            })
        );
    }

    #[test]
    fn parse_wildcard_with_trailing_args() {
        let result = parse_wildcard("npm:build:* --verbose");
        assert_eq!(
            result,
            Some(WildcardPattern {
                runner_prefix: "npm run ".to_string(),
                glob_pattern: "build:*".to_string(),
                exclusion: None,
                trailing_args: " --verbose".to_string(),
            })
        );
    }

    #[test]
    fn parse_wildcard_exclusion_and_trailing_args() {
        let result = parse_wildcard("npm:lint:*(!fix) --quiet");
        assert_eq!(
            result,
            Some(WildcardPattern {
                runner_prefix: "npm run ".to_string(),
                glob_pattern: "lint:*".to_string(),
                exclusion: Some("fix".to_string()),
                trailing_args: " --quiet".to_string(),
            })
        );
    }

    #[test]
    fn parse_wildcard_non_wildcard_shortcut_returns_none() {
        assert_eq!(parse_wildcard("npm:build"), None);
    }

    #[test]
    fn parse_wildcard_plain_command_returns_none() {
        assert_eq!(parse_wildcard("echo hello"), None);
    }

    #[test]
    fn parse_wildcard_unknown_prefix_returns_none() {
        assert_eq!(parse_wildcard("deno:build:*"), None);
    }
}
```

- [ ] **Step 2: Register the module in main.rs**

In `src/main.rs`, add after the existing `mod` declarations (after `mod task;`):

```rust
mod command_expander;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test command_expander -- --nocapture`
Expected: FAIL — `parse_wildcard` calls `todo!()`

- [ ] **Step 4: Implement parse_wildcard**

Replace the `todo!()` body of `parse_wildcard` in `src/command_expander.rs`:

```rust
fn parse_wildcard(cmd: &str) -> Option<WildcardPattern> {
    let (prefix, runner) = MANAGERS.iter().find(|(p, _)| cmd.starts_with(p))?;

    let rest = &cmd[prefix.len()..];

    // Split on first space to separate script pattern from trailing args
    let (script_part, trailing) = match rest.find(' ') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };

    if !script_part.contains('*') {
        return None;
    }

    // Check for exclusion syntax: *(!pattern)
    let (glob_pattern, exclusion) = if let Some(excl_start) = script_part.find("*(!") {
        if script_part[excl_start..].ends_with(')') {
            let excl_content = &script_part[excl_start + 3..script_part.len() - 1];
            let base = format!("{}*", &script_part[..excl_start]);
            (base, Some(excl_content.to_string()))
        } else {
            (script_part.to_string(), None)
        }
    } else {
        (script_part.to_string(), None)
    };

    Some(WildcardPattern {
        runner_prefix: runner.to_string(),
        glob_pattern,
        exclusion,
        trailing_args: trailing.to_string(),
    })
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test command_expander -- --nocapture`
Expected: all 10 tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/command_expander.rs src/main.rs
git commit -m "feat: add wildcard pattern parsing for package manager shortcuts (#24)"
```

---

### Task 3: Glob Matching with Auto-Naming

**Files:**
- Modify: `src/command_expander.rs`

- [ ] **Step 1: Write failing tests for match_scripts**

Add the `ExpandedMatch` struct and `match_scripts` stub above the `#[cfg(test)]` block in `src/command_expander.rs`:

```rust
#[cfg_attr(test, derive(Debug, PartialEq))]
struct ExpandedMatch {
    command: String,
    auto_name: String,
}

/// Match a wildcard pattern against script names. Returns expanded commands
/// and auto-generated names, sorted by command for deterministic output.
fn match_scripts(
    pattern: &WildcardPattern,
    scripts: &HashMap<String, String>,
) -> Result<Vec<ExpandedMatch>> {
    todo!()
}
```

Add these tests inside the existing `mod tests` block:

```rust
    fn test_scripts() -> HashMap<String, String> {
        HashMap::from([
            ("build:client".into(), "webpack --config client.js".into()),
            ("build:server".into(), "webpack --config server.js".into()),
            ("test:unit".into(), "jest".into()),
            ("lint:js".into(), "eslint .".into()),
            ("lint:ts".into(), "tsc --noEmit".into()),
            ("lint:fix".into(), "eslint --fix .".into()),
            ("lint:fix:js".into(), "eslint --fix --ext .js .".into()),
        ])
    }

    #[test]
    fn match_scripts_basic_glob() {
        let pattern = parse_wildcard("npm:build:*").unwrap();
        let matches = match_scripts(&pattern, &test_scripts()).unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].command, "npm run build:client");
        assert_eq!(matches[0].auto_name, "client");
        assert_eq!(matches[1].command, "npm run build:server");
        assert_eq!(matches[1].auto_name, "server");
    }

    #[test]
    fn match_scripts_with_exclusion() {
        let pattern = parse_wildcard("npm:lint:*(!fix)").unwrap();
        let matches = match_scripts(&pattern, &test_scripts()).unwrap();
        assert_eq!(matches.len(), 2);
        let names: Vec<&str> = matches.iter().map(|m| m.auto_name.as_str()).collect();
        assert!(names.contains(&"js"));
        assert!(names.contains(&"ts"));
    }

    #[test]
    fn match_scripts_no_matches() {
        let pattern = parse_wildcard("npm:deploy:*").unwrap();
        let matches = match_scripts(&pattern, &test_scripts()).unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn match_scripts_with_trailing_args() {
        let pattern = parse_wildcard("npm:build:* --verbose").unwrap();
        let matches = match_scripts(&pattern, &test_scripts()).unwrap();
        assert_eq!(matches[0].command, "npm run build:client --verbose");
        assert_eq!(matches[1].command, "npm run build:server --verbose");
    }

    #[test]
    fn match_scripts_star_alone_matches_all() {
        let pattern = parse_wildcard("npm:*").unwrap();
        let matches = match_scripts(&pattern, &test_scripts()).unwrap();
        assert_eq!(matches.len(), 7);
        // auto_name for bare * is the full script name
        assert_eq!(matches[0].auto_name, "build:client");
    }

    #[test]
    fn match_scripts_auto_name_strips_prefix() {
        let pattern = parse_wildcard("pnpm:test:*").unwrap();
        let matches = match_scripts(&pattern, &test_scripts()).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].auto_name, "unit");
        assert_eq!(matches[0].command, "pnpm run test:unit");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test command_expander -- --nocapture`
Expected: FAIL — `match_scripts` calls `todo!()`

- [ ] **Step 3: Implement match_scripts**

Replace the `todo!()` body of `match_scripts`:

```rust
fn match_scripts(
    pattern: &WildcardPattern,
    scripts: &HashMap<String, String>,
) -> Result<Vec<ExpandedMatch>> {
    let glob = GlobBuilder::new(&pattern.glob_pattern)
        .literal_separator(false)
        .build()
        .context(format!(
            "Invalid glob pattern: {}",
            pattern.glob_pattern
        ))?
        .compile_matcher();

    let exclusion_matcher = if let Some(ref excl) = pattern.exclusion {
        let literal_prefix = pattern
            .glob_pattern
            .split('*')
            .next()
            .unwrap_or("");
        let excl_glob = format!("{}{}*", literal_prefix, excl);
        Some(
            GlobBuilder::new(&excl_glob)
                .literal_separator(false)
                .build()
                .context(format!("Invalid exclusion pattern: {}", excl_glob))?
                .compile_matcher(),
        )
    } else {
        None
    };

    let name_prefix = pattern
        .glob_pattern
        .split('*')
        .next()
        .unwrap_or("");

    let mut matches: Vec<ExpandedMatch> = scripts
        .keys()
        .filter(|name| {
            if !glob.is_match(name.as_str()) {
                return false;
            }
            if let Some(ref excl) = exclusion_matcher {
                if excl.is_match(name.as_str()) {
                    return false;
                }
            }
            true
        })
        .map(|name| {
            let auto_name = name
                .strip_prefix(name_prefix)
                .unwrap_or(name)
                .to_string();
            let command = format!(
                "{}{}{}",
                pattern.runner_prefix, name, pattern.trailing_args
            );
            ExpandedMatch { command, auto_name }
        })
        .collect();

    matches.sort_by(|a, b| a.command.cmp(&b.command));

    Ok(matches)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test command_expander -- --nocapture`
Expected: all 16 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/command_expander.rs
git commit -m "feat: add glob matching with exclusion and auto-naming (#24)"
```

---

### Task 4: Manifest Reading

**Files:**
- Modify: `src/command_expander.rs`

- [ ] **Step 1: Write failing tests for read_manifest**

Add the `PackageJson` struct and `read_manifest` stub above `#[cfg(test)]` in `src/command_expander.rs`:

```rust
#[derive(Deserialize)]
struct PackageJson {
    #[serde(default)]
    scripts: HashMap<String, String>,
}

/// Read and parse the scripts field from a package.json manifest file.
fn read_manifest(manifest_path: Option<&str>) -> Result<HashMap<String, String>> {
    todo!()
}
```

Add an `fs` import at the top of the file:

```rust
use std::fs;
```

Add these tests inside `mod tests`:

```rust
    #[test]
    fn read_manifest_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package.json");
        fs::write(
            &path,
            r#"{"scripts": {"build": "tsc", "test": "jest"}}"#,
        )
        .unwrap();
        let scripts = read_manifest(Some(path.to_str().unwrap())).unwrap();
        assert_eq!(scripts.len(), 2);
        assert_eq!(scripts["build"], "tsc");
        assert_eq!(scripts["test"], "jest");
    }

    #[test]
    fn read_manifest_missing_file() {
        let result = read_manifest(Some("/nonexistent/path/package.json"));
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Could not read manifest"));
    }

    #[test]
    fn read_manifest_no_scripts_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package.json");
        fs::write(&path, r#"{"name": "test", "version": "1.0.0"}"#).unwrap();
        let scripts = read_manifest(Some(path.to_str().unwrap())).unwrap();
        assert!(scripts.is_empty());
    }

    #[test]
    fn read_manifest_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package.json");
        fs::write(&path, "this is not json {{{").unwrap();
        let result = read_manifest(Some(path.to_str().unwrap()));
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Invalid JSON"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test command_expander::tests::read_manifest -- --nocapture`
Expected: FAIL — `read_manifest` calls `todo!()`

- [ ] **Step 3: Implement read_manifest**

Replace the `todo!()` body:

```rust
fn read_manifest(manifest_path: Option<&str>) -> Result<HashMap<String, String>> {
    let path = manifest_path.unwrap_or("package.json");
    let content = fs::read_to_string(path)
        .context(format!("Could not read manifest file: {}", path))?;
    let pkg: PackageJson = serde_json::from_str(&content)
        .context(format!("Invalid JSON in manifest file: {}", path))?;
    Ok(pkg.scripts)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test command_expander -- --nocapture`
Expected: all 20 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/command_expander.rs
git commit -m "feat: add package.json manifest reading for wildcard expansion (#24)"
```

---

### Task 5: expand_commands Top-Level Function

**Files:**
- Modify: `src/command_expander.rs`

- [ ] **Step 1: Write failing tests for expand_commands**

Add the public `expand_commands` stub above `#[cfg(test)]`:

```rust
/// Expand wildcard patterns in process commands.
/// Returns expanded (processes, names) vectors.
/// Names are `Some(name)` for explicitly named or auto-named positions,
/// `None` for positions with no name.
pub fn expand_commands(
    processes: Vec<String>,
    names: Vec<String>,
    manifest_path: Option<String>,
) -> Result<(Vec<String>, Vec<Option<String>>)> {
    todo!()
}
```

Add these tests inside `mod tests`:

```rust
    fn write_test_manifest(dir: &tempfile::TempDir) -> String {
        let path = dir.path().join("package.json");
        fs::write(
            &path,
            r#"{
                "scripts": {
                    "build:client": "webpack client",
                    "build:server": "webpack server",
                    "test:unit": "jest",
                    "lint:js": "eslint .",
                    "lint:ts": "tsc --noEmit",
                    "lint:fix": "eslint --fix ."
                }
            }"#,
        )
        .unwrap();
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn expand_no_wildcards_passthrough() {
        let processes = vec!["echo hello".into(), "echo world".into()];
        let names = vec!["a".into(), "b".into()];
        let (procs, nms) = expand_commands(processes.clone(), names, None).unwrap();
        assert_eq!(procs, processes);
        assert_eq!(nms, vec![Some("a".into()), Some("b".into())]);
    }

    #[test]
    fn expand_no_wildcards_fewer_names() {
        let processes = vec!["echo a".into(), "echo b".into(), "echo c".into()];
        let names = vec!["first".into()];
        let (procs, nms) = expand_commands(processes.clone(), names, None).unwrap();
        assert_eq!(procs, processes);
        assert_eq!(nms, vec![Some("first".into()), None, None]);
    }

    #[test]
    fn expand_wildcard_basic() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = write_test_manifest(&dir);
        let processes = vec!["npm:build:*".into()];
        let names = vec![];
        let (procs, nms) =
            expand_commands(processes, names, Some(manifest)).unwrap();
        assert_eq!(procs, vec![
            "npm run build:client".to_string(),
            "npm run build:server".to_string(),
        ]);
        assert_eq!(nms, vec![
            Some("client".into()),
            Some("server".into()),
        ]);
    }

    #[test]
    fn expand_mixed_wildcard_and_plain() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = write_test_manifest(&dir);
        let processes = vec![
            "node server.js".into(),
            "npm:build:*".into(),
        ];
        let names = vec!["server".into()];
        let (procs, nms) =
            expand_commands(processes, names, Some(manifest)).unwrap();
        assert_eq!(procs, vec![
            "node server.js",
            "npm run build:client",
            "npm run build:server",
        ]);
        assert_eq!(nms, vec![
            Some("server".into()),
            Some("client".into()),
            Some("server".into()),
        ]);
    }

    #[test]
    fn expand_explicit_names_override_auto() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = write_test_manifest(&dir);
        let processes = vec!["npm:build:*".into()];
        let names = vec!["custom1".into(), "custom2".into()];
        let (procs, nms) =
            expand_commands(processes, names, Some(manifest)).unwrap();
        assert_eq!(procs.len(), 2);
        assert_eq!(nms, vec![
            Some("custom1".into()),
            Some("custom2".into()),
        ]);
    }

    #[test]
    fn expand_wildcard_no_matches_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = write_test_manifest(&dir);
        let processes = vec![
            "echo hello".into(),
            "npm:deploy:*".into(),
        ];
        let names = vec!["greeter".into()];
        let (procs, nms) =
            expand_commands(processes, names, Some(manifest)).unwrap();
        // deploy:* matched nothing, so only the echo command remains
        assert_eq!(procs, vec!["echo hello"]);
        assert_eq!(nms, vec![Some("greeter".into())]);
    }

    #[test]
    fn expand_wildcard_with_trailing_args() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = write_test_manifest(&dir);
        let processes = vec!["npm:build:* --watch".into()];
        let names = vec![];
        let (procs, _) =
            expand_commands(processes, names, Some(manifest)).unwrap();
        assert_eq!(procs, vec![
            "npm run build:client --watch",
            "npm run build:server --watch",
        ]);
    }

    #[test]
    fn expand_plain_after_wildcard_no_name() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = write_test_manifest(&dir);
        let processes = vec![
            "npm:build:*".into(),
            "echo done".into(),
        ];
        let names = vec![];
        let (procs, nms) =
            expand_commands(processes, names, Some(manifest)).unwrap();
        assert_eq!(procs, vec![
            "npm run build:client",
            "npm run build:server",
            "echo done",
        ]);
        assert_eq!(nms, vec![
            Some("client".into()),
            Some("server".into()),
            None,
        ]);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test command_expander::tests::expand -- --nocapture`
Expected: FAIL — `expand_commands` calls `todo!()`

- [ ] **Step 3: Implement expand_commands**

Replace the `todo!()` body:

```rust
pub fn expand_commands(
    processes: Vec<String>,
    names: Vec<String>,
    manifest_path: Option<String>,
) -> Result<(Vec<String>, Vec<Option<String>>)> {
    let has_wildcards = processes.iter().any(|p| parse_wildcard(p).is_some());

    let scripts = if has_wildcards {
        Some(read_manifest(manifest_path.as_deref())?)
    } else {
        None
    };

    let mut expanded_processes = Vec::new();
    let mut expanded_names: Vec<Option<String>> = Vec::new();
    let mut name_idx = 0;

    for process in &processes {
        if let Some(pattern) = parse_wildcard(process) {
            let scripts = scripts.as_ref().unwrap();
            let matches = match_scripts(&pattern, scripts)?;

            if matches.is_empty() {
                eprintln!(
                    "Warning: pattern '{}' matched no scripts, skipping",
                    process
                );
                if name_idx < names.len() {
                    name_idx += 1;
                }
                continue;
            }

            for m in &matches {
                let name = if name_idx < names.len() {
                    Some(names[name_idx].clone())
                } else {
                    Some(m.auto_name.clone())
                };
                expanded_processes.push(m.command.clone());
                expanded_names.push(name);
                name_idx += 1;
            }
        } else {
            expanded_processes.push(process.clone());
            let name = if name_idx < names.len() {
                Some(names[name_idx].clone())
            } else {
                None
            };
            expanded_names.push(name);
            name_idx += 1;
        }
    }

    Ok((expanded_processes, expanded_names))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test command_expander -- --nocapture`
Expected: all 28 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/command_expander.rs
git commit -m "feat: add expand_commands top-level function (#24)"
```

---

### Task 6: CLI Flag and main.rs Integration

**Files:**
- Modify: `src/main.rs`

This task wires everything up: adds the `--manifest-path` flag, changes `CommandParser.names` to `Vec<Option<String>>`, refactors `CommandParser::new` to accept pre-expanded data, updates `SuccessCondition::evaluate` for the new names type, and calls `expand_commands` in main.

- [ ] **Step 1: Add `--manifest-path` to Commands struct**

In `src/main.rs`, inside the `Commands` struct, add after the `success` field:

```rust
  /// path to package.json for wildcard expansion
  #[argh(option)]
  manifest_path: Option<String>,
```

- [ ] **Step 2: Change `CommandParser.names` type to `Vec<Option<String>>`**

In `src/main.rs`, change the `CommandParser` struct:

```rust
pub struct CommandParser {
  pub names: Vec<Option<String>>,
  pub processes: Vec<String>,
  pub mlti_config: MltiConfig,
  success_condition: SuccessCondition,
}
```

- [ ] **Step 3: Refactor `CommandParser::new` to accept pre-expanded data**

Replace the existing `CommandParser::new` method:

```rust
  pub fn new(
    processes: Vec<String>,
    names: Vec<Option<String>>,
    success: &str,
    mlti_config: MltiConfig,
  ) -> Result<Self, String> {
    let success_condition = SuccessCondition::parse(success)?;
    Ok(Self {
      names,
      processes,
      success_condition,
      mlti_config,
    })
  }
```

- [ ] **Step 4: Update `SuccessCondition::evaluate` for `Option<String>` names**

Change the signature and body of `SuccessCondition::evaluate`:

```rust
  fn evaluate(&self, exit_codes: &[(usize, i32)], names: &[Option<String>]) -> i32 {
    if exit_codes.is_empty() {
      return 1;
    }
    match self {
      Self::All => first_nonzero(exit_codes, None),
      Self::First => code_at(exit_codes.first(), 1),
      Self::Last => code_at(exit_codes.last(), 1),
      Self::CommandIndex(idx) => {
        code_at(exit_codes.iter().find(|(i, _)| i == idx), 1)
      }
      Self::CommandName(name) => {
        match names.iter().position(|n| n.as_deref() == Some(name.as_str())) {
          Some(idx) => code_at(exit_codes.iter().find(|(i, _)| *i == idx), 1),
          None => 1,
        }
      }
      Self::NotCommandIndex(idx) => first_nonzero(exit_codes, Some(*idx)),
      Self::NotCommandName(name) => {
        match names.iter().position(|n| n.as_deref() == Some(name.as_str())) {
          Some(idx) => first_nonzero(exit_codes, Some(idx)),
          None => 1,
        }
      }
    }
  }
```

- [ ] **Step 5: Update the `main` function to wire up expansion**

Replace the beginning of the `main` function (from `let commands: Commands = argh::from_env();` through the `CommandParser` creation and `mlti_config` line) with:

```rust
  let commands: Commands = argh::from_env();
  let red_style = Style::new().red();
  let bold_green_style = Style::new().bold().green();

  let parsed_names = parse_names(
    commands.names.clone(),
    commands.names_seperator.clone(),
  );
  let (processes, names) = command_expander::expand_commands(
    commands.processes.clone(),
    parsed_names,
    commands.manifest_path.clone(),
  )
  .unwrap_or_else(|e| {
    eprintln!("{}", e);
    std::process::exit(1);
  });

  let mlti_config = MltiConfig {
    group: commands.group,
    kill_others: commands.kill_others,
    kill_others_on_fail: commands.kill_others_on_fail,
    restart_tries: commands.restart_tries,
    restart_after: commands.restart_after,
    prefix: commands.prefix.clone(),
    prefix_length: commands.prefix_length,
    max_processes: parse_max_processes(commands.max_processes.clone()),
    raw: commands.raw,
    no_color: commands.no_color,
    timestamp_format: commands.timestamp_format.clone(),
  };

  let arg_parser = CommandParser::new(
    processes,
    names,
    &commands.success,
    mlti_config.clone(),
  )
  .unwrap_or_else(|e| {
    eprintln!("{}", e);
    std::process::exit(1);
  });
  let mlti_config = arg_parser.get_mlti_config();
```

- [ ] **Step 6: Update the process name lookup in the main loop**

In the main loop (the `for i in 0..arg_parser.len()` block), change the name lookup from:

```rust
    let name = arg_parser.names.get(i).map(|name| name.to_string());
```

to:

```rust
    let name = arg_parser.names.get(i).cloned().flatten();
```

- [ ] **Step 7: Update tests for `Option<String>` names**

In the `#[cfg(test)] mod tests` block in `src/main.rs`, update every test that passes a `names` slice to `evaluate`. Change `Vec<String>` to `Vec<Option<String>>`:

Replace:
```rust
  fn evaluate_command_name_resolves_via_names() {
    let names = vec!["build".to_string(), "serve".to_string(), "test".to_string()];
```
with:
```rust
  fn evaluate_command_name_resolves_via_names() {
    let names: Vec<Option<String>> = vec![Some("build".into()), Some("serve".into()), Some("test".into())];
```

Replace:
```rust
  fn evaluate_command_name_unknown_returns_one() {
    let names = vec!["build".to_string(), "serve".to_string()];
```
with:
```rust
  fn evaluate_command_name_unknown_returns_one() {
    let names: Vec<Option<String>> = vec![Some("build".into()), Some("serve".into())];
```

Replace:
```rust
  fn evaluate_not_command_name_resolves_and_excludes() {
    let names = vec!["build".to_string(), "flaky".to_string(), "test".to_string()];
```
with:
```rust
  fn evaluate_not_command_name_resolves_and_excludes() {
    let names: Vec<Option<String>> = vec![Some("build".into()), Some("flaky".into()), Some("test".into())];
```

Replace:
```rust
  fn evaluate_not_command_name_unknown_returns_one() {
    let names = vec!["build".to_string(), "serve".to_string()];
```
with:
```rust
  fn evaluate_not_command_name_unknown_returns_one() {
    let names: Vec<Option<String>> = vec![Some("build".into()), Some("serve".into())];
```

For all tests that pass `&[]` as names (like `evaluate_all_returns_zero_when_all_succeed`), no change needed — `&[]` is compatible with both `&[String]` and `&[Option<String>]`.

- [ ] **Step 8: Run all tests**

Run: `cargo test`
Expected: all tests PASS (both main.rs tests and command_expander tests)

- [ ] **Step 9: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire up wildcard expansion with --manifest-path flag (#24)"
```

---

### Task 7: Final Verification

**Files:** None (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: no warnings or errors

- [ ] **Step 3: Run rustfmt**

Run: `cargo fmt -- --check`
Expected: no formatting issues (run `cargo fmt` to fix if needed)

- [ ] **Step 4: Verify the binary runs with --help**

Run: `cargo run -- --help`
Expected: help output includes `--manifest-path` option

- [ ] **Step 5: Commit any fixes from clippy/fmt**

If clippy or fmt required changes:
```bash
git add -A
git commit -m "chore: fix clippy warnings and formatting (#24)"
```
