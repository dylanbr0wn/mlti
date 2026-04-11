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
