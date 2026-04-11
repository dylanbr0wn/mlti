use std::collections::HashMap;
use std::fs;

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
  let glob = GlobBuilder::new(&pattern.glob_pattern)
    .literal_separator(false)
    .build()
    .context(format!("Invalid glob pattern: {}", pattern.glob_pattern))?
    .compile_matcher();

  let exclusion_matcher = if let Some(ref excl) = pattern.exclusion {
    let literal_prefix = pattern.glob_pattern.split('*').next().unwrap_or("");
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

  let name_prefix = pattern.glob_pattern.split('*').next().unwrap_or("");

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
      let auto_name = name.strip_prefix(name_prefix).unwrap_or(name).to_string();
      let command =
        format!("{}{}{}", pattern.runner_prefix, name, pattern.trailing_args);
      ExpandedMatch { command, auto_name }
    })
    .collect();

  matches.sort_by(|a, b| a.command.cmp(&b.command));

  Ok(matches)
}

/// Parse a command string into a WildcardPattern if it contains a wildcard.
/// Returns None for non-wildcard commands.
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

#[derive(Deserialize)]
struct PackageJson {
  #[serde(default)]
  scripts: HashMap<String, String>,
}

/// Expand wildcard patterns in process commands.
/// Returns expanded (processes, names) vectors.
/// Names are `Some(name)` for explicitly named or auto-named positions,
/// `None` for positions with no name.
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

/// Read and parse the scripts field from a package.json manifest file.
fn read_manifest(manifest_path: Option<&str>) -> Result<HashMap<String, String>> {
  let path = manifest_path.unwrap_or("package.json");
  let content = fs::read_to_string(path)
    .context(format!("Could not read manifest file: {}", path))?;
  let pkg: PackageJson = serde_json::from_str(&content)
    .context(format!("Invalid JSON in manifest file: {}", path))?;
  Ok(pkg.scripts)
}

#[cfg(test)]
mod tests {
  use super::*;

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
    let (procs, nms) = expand_commands(processes, names, Some(manifest)).unwrap();
    assert_eq!(
      procs,
      vec![
        "npm run build:client".to_string(),
        "npm run build:server".to_string(),
      ]
    );
    assert_eq!(nms, vec![Some("client".into()), Some("server".into()),]);
  }

  #[test]
  fn expand_mixed_wildcard_and_plain() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = write_test_manifest(&dir);
    let processes = vec!["node server.js".into(), "npm:build:*".into()];
    let names = vec!["server".into()];
    let (procs, nms) = expand_commands(processes, names, Some(manifest)).unwrap();
    assert_eq!(
      procs,
      vec![
        "node server.js",
        "npm run build:client",
        "npm run build:server",
      ]
    );
    assert_eq!(
      nms,
      vec![
        Some("server".into()),
        Some("client".into()),
        Some("server".into()),
      ]
    );
  }

  #[test]
  fn expand_explicit_names_override_auto() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = write_test_manifest(&dir);
    let processes = vec!["npm:build:*".into()];
    let names = vec!["custom1".into(), "custom2".into()];
    let (procs, nms) = expand_commands(processes, names, Some(manifest)).unwrap();
    assert_eq!(procs.len(), 2);
    assert_eq!(nms, vec![Some("custom1".into()), Some("custom2".into()),]);
  }

  #[test]
  fn expand_wildcard_no_matches_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = write_test_manifest(&dir);
    let processes = vec!["echo hello".into(), "npm:deploy:*".into()];
    let names = vec!["greeter".into()];
    let (procs, nms) = expand_commands(processes, names, Some(manifest)).unwrap();
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
    let (procs, _) = expand_commands(processes, names, Some(manifest)).unwrap();
    assert_eq!(
      procs,
      vec![
        "npm run build:client --watch",
        "npm run build:server --watch",
      ]
    );
  }

  #[test]
  fn expand_plain_after_wildcard_no_name() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = write_test_manifest(&dir);
    let processes = vec!["npm:build:*".into(), "echo done".into()];
    let names = vec![];
    let (procs, nms) = expand_commands(processes, names, Some(manifest)).unwrap();
    assert_eq!(
      procs,
      vec!["npm run build:client", "npm run build:server", "echo done",]
    );
    assert_eq!(
      nms,
      vec![Some("client".into()), Some("server".into()), None,]
    );
  }

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

  #[test]
  fn read_manifest_valid() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("package.json");
    fs::write(&path, r#"{"scripts": {"build": "tsc", "test": "jest"}}"#).unwrap();
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
}
