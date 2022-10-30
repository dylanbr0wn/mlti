use clap::{arg, command, value_parser, ArgAction, ArgMatches};

pub struct ArgParser {
  pub matches: ArgMatches,
  pub names: Vec<String>,
  pub processes: Vec<String>,
  pub kill_others: bool,
  pub kill_others_on_fail: bool,
  pub restart_tries: i64,
  pub restart_after: i64,
  pub prefix: Option<String>,
  pub prefix_length: i16,
}

impl ArgParser {
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
                arg!(--"kill-others" "Kill other processes if one exits.")
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

    Self {
      matches,
      names,
      processes,
      kill_others,
      kill_others_on_fail,
      restart_tries,
      restart_after,
      prefix,
      prefix_length,
    }
  }

  pub fn len(&self) -> usize {
    self.processes.len()
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
