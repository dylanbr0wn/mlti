use std::i32::MAX;

use clap::{arg, command, value_parser, ArgAction, ArgMatches};

#[derive(Clone)]
pub struct CommandArgs {
  pub kill_others: bool,
  pub kill_others_on_fail: bool,
  pub restart_tries: i64,
  pub restart_after: i64,
  pub prefix: Option<String>,
  pub prefix_length: i16,
  pub max_processes: i32,
  pub raw: bool,
  pub no_color: bool,
}

pub struct CommandParser {
  pub matches: ArgMatches,
  pub names: Vec<String>,
  pub processes: Vec<String>,
  pub command_args: CommandArgs,
}

impl CommandParser {
  pub fn new() -> Self {
    let matches = command!() // requires `cargo` feature
            .arg(
                arg!(-n --names <NAMES> "Names of processes")
                    .id("names")
                    .help("Names of processes"),
            )
            .arg(
                arg!(--"name-separator" <seperator> "Name seperator character")
                    .id("name-separator"),
            )
            .arg(
                arg!( -k --"kill-others" "Kill other processes if one exits.")
                    .id("kill-others")
                    .value_parser(value_parser!(bool)),
            )
            .arg(
                arg!(--"kill-others-on-fail" "Kill other processes if one exits with a non-zero exit code.")
                    .id("kill-others-on-fail")
                    .value_parser(value_parser!(bool)),
            )
            .arg(
                arg!(--"restart-tries" <attempts> "How many times a process will attempt to restart.")
                    .id("restart-tries")
                    .value_parser(value_parser!(i64)),
            )
             .arg(
                arg!(--"restart-after" <delay> "Amount of time to delay between restart attempts.")
                    .id("restart-after")
                    .value_parser(value_parser!(i64)),
            )
            .arg(
                arg!(-p --prefix <pre> "Prefixed used in logging for each process.")
                    .id("prefix")
                    .value_parser(value_parser!(String)),
            ).arg(
                arg!(-l --"prefix-length" <pre> "Max number of characters of prefix that are shown.")
                    .id("prefix")
                    .value_parser(value_parser!(i16)),
            )
            .arg(
                arg!(-m --"max-processes" <pre> "How many process should run at once.")
                    .id("max-processes")
                    .value_parser(value_parser!(String)),
            )
            .arg(
                arg!(-r --raw "Print raw output of process only.")
                    .id("raw")
                    .value_parser(value_parser!(bool)),
            )
            .arg(
                arg!(--"no-color" "Disable color output.")
                    .id("no-color")
                    .value_parser(value_parser!(bool)),
            )
            .arg(arg!([processes] "List of prcoess to run concurrently").action(ArgAction::Append))
            .get_matches();

    let names = parse_names(matches.clone());
    let processes = parse_processes(matches.clone());

    let kill_others = matches.get_flag("kill-others");
    let kill_others_on_fail = matches.get_flag("kill-others-on-fail");

    let restart_after = matches
      .get_one::<i64>("restart-after")
      .unwrap_or(&0)
      .to_owned();
    let restart_tries = matches
      .get_one::<i64>("restart-tries")
      .unwrap_or(&0)
      .to_owned();

    let prefix = matches.get_one::<String>("prefix").map(|x| x.to_owned());

    let prefix_length = matches
      .get_one::<i16>("prefix-length")
      .unwrap_or(&10)
      .to_owned();

    let max_processes = matches
      .get_one::<String>("max-processes")
      .map(|x| x.to_owned());

    let max_processes = parse_max_processes(max_processes);

    let raw = matches.get_flag("raw");
    let no_color = matches.get_flag("no-color");

    Self {
      matches,
      names,
      processes,
      command_args: CommandArgs {
        kill_others,
        kill_others_on_fail,
        restart_tries,
        restart_after,
        prefix,
        prefix_length,
        max_processes,
        raw,
        no_color,
      },
    }
  }

  pub fn len(&self) -> usize {
    self.processes.len()
  }
  pub fn get_command_args(&self) -> CommandArgs {
    self.command_args.clone()
  }
}

pub fn parse_names(matches: ArgMatches) -> Vec<String> {
  let seperator = match matches.get_one::<String>("name-separator") {
    Some(seperator) => seperator,
    None => ",",
  };

  let names = match matches.get_one::<String>("names") {
    Some(names) => names.split(seperator).map(|x| x.to_string()).collect(),
    None => vec![],
  };
  names
}

pub fn parse_processes(matches: ArgMatches) -> Vec<String> {
  let processes = matches
    .get_many::<String>("processes")
    .unwrap_or_default()
    .map(|v| v.to_owned())
    .collect::<Vec<_>>();
  processes
}
pub fn parse_max_processes(max_processes: Option<String>) -> i32 {

  match max_processes {
    Some(max) => {
      if max.contains('%') {
        let percentage = str::parse::<i32>(&max.replace('%', ""))
          .expect("Could not parse percentage");
        let cpus = num_cpus::get();

        (cpus as f32 * (percentage as f32 / 100.0)) as i32
      } else {
        str::parse::<i32>(&max).expect("Could not parse max processes")
      }
    }
    None => MAX, // fuck it why not
  }
}
