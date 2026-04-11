use std::process::{self, Stdio};

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
    // Commands are already expanded by command_expander before reaching here.
    let mut args = raw_cmd.split_whitespace();
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

  // if explicitly named, use that (includes auto-names from command_expander)

  if let Some(name) = name {
    return name;
  }

  // Fallback to index-based name for plain commands with no explicit name
  format!("{}", index)
}

fn truncate(s: &str, max_chars: usize) -> &str {
  match s.char_indices().nth(max_chars) {
    None => s,
    Some((idx, _)) => &s[..idx],
  }
}
