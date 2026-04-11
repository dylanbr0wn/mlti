use std::process::{self, Stdio};

use anyhow::Result;
use tokio::process::Child;

pub(crate) struct Process {
  pub name: String,
  pub args: Vec<String>,
  pub cmd: String,
  pub raw_cmd: String,
  pub index: usize,
  pub color: (u8, u8, u8),
}

impl Process {
  pub fn new(
    raw_cmd: String,
    name: Option<String>,
    index: usize,
    prefix: Option<String>,
    length: i16,
    color: (u8, u8, u8),
    timestamp_format: String,
  ) -> Self {
    let parsed_cmd = parse(&raw_cmd).unwrap();

    let mut args = parsed_cmd.split_whitespace();

    let cmd_string = args.next().unwrap_or("");

    let args = args.map(|x| x.to_string()).collect::<Vec<String>>();

    let name = get_name(&raw_cmd, name, index, prefix, length, timestamp_format);

    Self {
      color,
      index,
      name,
      args,
      cmd: cmd_string.to_string(),
      raw_cmd: raw_cmd.clone(),
    }
  }
  pub fn run(&self) -> Result<Child, std::io::Error> {
    let mut cmd = tokio::process::Command::new(self.cmd.clone());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.args(self.args.clone());

    cmd.spawn()
  }
}

fn npm_expander(cmd: &str) -> String {
  match cmd.strip_prefix("npm:") {
    Some(rest) => format!("npm run {rest}"),
    None => cmd.to_string(),
  }
}
fn pnpm_expander(cmd: &str) -> String {
  match cmd.strip_prefix("pnpm:") {
    Some(rest) => format!("pnpm run {rest}"),
    None => cmd.to_string(),
  }
}
fn yarn_expander(cmd: &str) -> String {
  match cmd.strip_prefix("yarn:") {
    Some(rest) => format!("yarn run {rest}"),
    None => cmd.to_string(),
  }
}
fn bun_expander(cmd: &str) -> String {
  match cmd.strip_prefix("bun:") {
    Some(rest) => format!("bun run {rest}"),
    None => cmd.to_string(),
  }
}
fn node_expander(cmd: &str) -> String {
  match cmd.strip_prefix("node:") {
    Some(rest) => format!("node --run {rest}"),
    None => cmd.to_string(),
  }
}
fn deno_expander(cmd: &str) -> String {
  match cmd.strip_prefix("deno:") {
    Some(rest) => format!("deno task {rest}"),
    None => cmd.to_string(),
  }
}

// Expansion order: with strip_prefix, each expander only matches at the start
// of the string, so order no longer affects correctness (e.g., "pnpm:" won't
// false-match "npm:"). Only one expander can ever match a given command.
pub fn expand(cmd: &str) -> String {
  let cmd = pnpm_expander(cmd);
  let cmd = yarn_expander(&cmd);
  let cmd = bun_expander(&cmd);
  let cmd = node_expander(&cmd);
  let cmd = deno_expander(&cmd);
  npm_expander(&cmd)
}

pub fn parse(raw_cmd: &str) -> Result<String> {
  let parts = expand(raw_cmd);
  Ok(parts)
}

fn replace_prefix(prefix: String, key: String, value: String) -> String {
  if prefix == key {
    value
  } else {
    let format_str = format!("{{{}}}", key);
    prefix.replace(&format_str, &value)
  }
}

fn get_name(
  raw_cmd: &str,
  name: Option<String>,
  index: usize,
  prefix: Option<String>,
  length: i16,
  timestamp_format: String,
) -> String {
  // if Prefix template parse it

  if let Some(prefix) = prefix {
    let mut prefix = prefix;
    let replace_list = vec![
      ("index", index.to_string()),
      ("command", raw_cmd.to_string()),
      ("name", (&raw_cmd).to_string()),
      ("pid", process::id().to_string()),
      (
        "time",
        chrono::Local::now().format(&timestamp_format).to_string(),
      ),
      ("none", "".to_string()),
    ];

    for (key, value) in replace_list {
      prefix = replace_prefix(prefix, key.to_string(), value);
    }

    let prefix = truncate(&prefix, length.try_into().unwrap());
    return prefix.to_string();
  }

  // if explicitly named, use that

  if let Some(name) = name {
    return name;
  }

  // if not, extract the task name from a package-manager prefix (e.g. "yarn:test" → "test").
  // Uses strip_prefix so only start-of-string prefixes match, consistent with expand().

  let prefixes = ["pnpm:", "yarn:", "bun:", "node:", "deno:", "npm:"];
  let default_name = format!("{}", index);

  prefixes
    .iter()
    .find_map(|p| raw_cmd.strip_prefix(p))
    .unwrap_or(&default_name)
    .to_string()
}

fn truncate(s: &str, max_chars: usize) -> &str {
  match s.char_indices().nth(max_chars) {
    None => s,
    Some((idx, _)) => &s[..idx],
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // ── expand() ──────────────────────────────────────────────

  #[test]
  fn expand_npm() {
    assert_eq!(expand("npm:test"), "npm run test");
  }

  #[test]
  fn expand_pnpm() {
    assert_eq!(expand("pnpm:build"), "pnpm run build");
  }

  #[test]
  fn expand_yarn() {
    assert_eq!(expand("yarn:test"), "yarn run test");
  }

  #[test]
  fn expand_bun() {
    assert_eq!(expand("bun:dev"), "bun run dev");
  }

  #[test]
  fn expand_node() {
    assert_eq!(expand("node:test"), "node --run test");
  }

  #[test]
  fn expand_deno() {
    assert_eq!(expand("deno:serve"), "deno task serve");
  }

  #[test]
  fn expand_no_prefix_passthrough() {
    assert_eq!(expand("echo hello"), "echo hello");
  }

  #[test]
  fn expand_no_false_node_in_middle() {
    // "node:" appearing inside a string must not be expanded
    assert_eq!(
      expand("node -e \"require('node:fs')\""),
      "node -e \"require('node:fs')\""
    );
  }

  #[test]
  fn expand_pnpm_does_not_trigger_npm() {
    // "pnpm:" contains "npm:" as a substring — strip_prefix prevents false match
    assert_eq!(expand("pnpm:test"), "pnpm run test");
  }

  // ── get_name() ────────────────────────────────────────────

  #[test]
  fn get_name_from_npm_prefix() {
    assert_eq!(
      get_name("npm:test", None, 0, None, 20, String::new()),
      "test"
    );
  }

  #[test]
  fn get_name_from_yarn_prefix() {
    assert_eq!(
      get_name("yarn:lint", None, 1, None, 20, String::new()),
      "lint"
    );
  }

  #[test]
  fn get_name_from_node_prefix() {
    assert_eq!(
      get_name("node:test", None, 0, None, 20, String::new()),
      "test"
    );
  }

  #[test]
  fn get_name_from_deno_prefix() {
    assert_eq!(
      get_name("deno:serve", None, 0, None, 20, String::new()),
      "serve"
    );
  }

  #[test]
  fn get_name_explicit_name_wins() {
    assert_eq!(
      get_name(
        "npm:test",
        Some("my-name".to_string()),
        0,
        None,
        20,
        String::new()
      ),
      "my-name"
    );
  }

  #[test]
  fn get_name_falls_back_to_index() {
    assert_eq!(
      get_name("echo hello", None, 3, None, 20, String::new()),
      "3"
    );
  }
}
