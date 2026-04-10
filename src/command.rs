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
  cmd.replace("npm:", "npm run ")
}
fn pnpm_expander(cmd: &str) -> String {
  cmd.replace("pnpm:", "pnpm run ")
}
fn yarn_expander(cmd: &str) -> String {
  cmd.replace("yarn:", "yarn run ")
}
fn bun_expander(cmd: &str) -> String {
  cmd.replace("bun:", "bun run ")
}
fn node_expander(cmd: &str) -> String {
  cmd.replace("node:", "node --run ")
}
fn deno_expander(cmd: &str) -> String {
  cmd.replace("deno:", "deno task ")
}

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

  // if not, check for a npm command.

  let prefixes = ["pnpm:", "yarn:", "bun:", "node:", "deno:", "npm:"];
  let default_name = format!("{}", index);

  let backup_name: String = prefixes
    .iter()
    .find(|p| raw_cmd.contains(**p))
    .and_then(|p| raw_cmd.split(p).nth(1))
    .unwrap_or(&default_name)
    .to_string();

  backup_name
}

fn truncate(s: &str, max_chars: usize) -> &str {
  match s.char_indices().nth(max_chars) {
    None => s,
    Some((idx, _)) => &s[..idx],
  }
}
