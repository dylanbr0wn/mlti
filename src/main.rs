use anyhow::Result;
use owo_colors::Style;
use rand::Rng;

mod command_parser;
mod logger;
mod runner;

pub fn parse_names(names: &Option<String>, seperator: &String) -> Vec<String> {
  let names = match names {
    Some(names) => names.split(seperator).map(|x| x.to_string()).collect(),
    None => vec![],
  };
  names
}

#[tokio::main]
async fn main() -> Result<()> {
  let red_style = Style::new().red();
  let bold_green_style = Style::new().bold().green();

  let cmds = command_parser::CommandParser::new();

  let runner = runner::Runner::new(cmds.parse()?);

  runner.run().await?;

  Ok(())
}

fn parse_hidden(hide: &Option<String>) -> Vec<String> {
  match hide {
    Some(h) => h.split(",").map(|x| x.to_string()).collect(),
    None => vec![],
  }
}
