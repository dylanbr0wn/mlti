package main

import "strings"

type Config struct {
	MaxProcesses     string   `json:"max-processes"`
	Names            []string `json:"names"`
	NameSeparator    string   `json:"name-separator"`
	SuccessTerms     []string `json:"success-terms"`
	Raw              bool     `json:"raw"`
	NoColor          bool     `json:"no-color"`
	Hide             []string `json:"hide"`
	Group            bool     `json:"group"`
	Timings          bool     `json:"timings"`
	PassThrough      bool     `json:"pass-through"`
	Prefix           string   `json:"prefix"`
	PrefixColors     []string `json:"prefix-colors"`
	PrefixLength     int      `json:"prefix-length"`
	TimestampFormat  string   `json:"timestamp-format"`
	KillOthers       bool     `json:"kill-others"`
	KillOthersOnFail bool     `json:"kill-others-on-fail"`
	KillSignal       string   `json:"kill-signal"`
	RestartTries     int      `json:"restart-tries"`
	RestartAfter     int      `json:"restart-after"`
}

var max_processes = NewFlag(
	Name("max-processes"),
	Description("Maximum number of processes to run at once."),
).String()

var names = NewFlag(
	Name("names"),
	Description("Names of the processes to run."),
	Short("n"),
	Default("lol"),
).String()

var name_separator = NewFlag(
	Name("name-separator"),
	Description("Separator for the names of the processes to run."),
	Default(","),
).String()

var success_terms = NewFlag(
	Name("success-terms"),
	Description("Which command(s) must exit with code 0 in order for mlti to exit with code 0 too."),
	Default("all"),
).String()

var raw = NewFlag(
	Name("raw"),
	Description("Print the raw output of the commands."),
).Bool()

var no_color = NewFlag(
	Name("no-color"),
	Description("Disable color output."),
).Bool()

var hide = NewFlag(
	Name("hide"),
	Description("Hide the output of the commands."),
).String()

var group = NewFlag(
	Name("group"),
	Description("Group the output of the commands."),
	Short("g"),
).Bool()

var timings = NewFlag(
	Name("timings"),
	Description("Show the timings of the commands."),
).Bool()

var pass_through = NewFlag(
	Name("pass-through"),
	Description("Pass the output of the commands through."),
).Bool()

var prefix = NewFlag(
	Name("prefix"),
	Description("Prefix the output of the commands."),
	Default("index"),
).String()

var prefix_colors = NewFlag(
	Name("prefix-colors"),
	Description("Colors for the prefixes."),
).String()

var TimestampFormat = NewFlag(
	Name("timestamp-format"),
	Description("Format for the timestamps."),
	Default("yyyy-MM-dd HH:mm:ss.SSS"),
).String()

var kill_others = NewFlag(
	Name("kill-others"),
	Description("Kill other processes when one fails."),
	Default(false),
).Bool()

var kill_others_on_fail = NewFlag(
	Name("kill-others-on-fail"),
	Description("Kill other processes when one fails."),
).Bool()

var kill_signal = NewFlag(
	Name("kill-signal"),
	Description("Signal to send to the processes."),
	Default("SIGTERM"),
).String()

var restart_tries = NewFlag(
	Name("restart-tries"),
	Description("Number of times to restart a process."),
	Default(10),
).Int()

var restart_after = NewFlag(
	Name("restart-after"),
	Description("Number of seconds to wait before restarting a process."),
	Default(0),
).Int()

func LoadConfigFromFlags() *Config {
	ParseFlags()
	return &Config{
		MaxProcesses:     max_processes.Get(),
		Names:            strings.Split(names.Get(), name_separator.Get()),
		NameSeparator:    name_separator.Get(),
		SuccessTerms:     strings.Split(success_terms.Get(), ","),
		Raw:              raw.Get(),
		NoColor:          no_color.Get(),
		Hide:             strings.Split(hide.Get(), ","),
		Group:            group.Get(),
		Timings:          timings.Get(),
		PassThrough:      pass_through.Get(),
		Prefix:           prefix.Get(),
		PrefixColors:     strings.Split(prefix_colors.Get(), ","),
		TimestampFormat:  TimestampFormat.Get(),
		KillOthers:       kill_others.Get(),
		KillOthersOnFail: kill_others_on_fail.Get(),
		KillSignal:       kill_signal.Get(),
		RestartTries:     restart_tries.Get(),
		RestartAfter:     restart_after.Get(),
	}
}
