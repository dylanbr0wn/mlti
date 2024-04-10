package main

import (
	"context"
	"os"
	"os/signal"
	"syscall"
)

var max_processes = NewFlag(
	Name("max-processes"),
	Description("Maximum number of processes to run at once."),
	Short("m"),
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
	Short("s"),
).String()

var raw = NewFlag(
	Name("raw"),
	Description("Print the raw output of the commands."),
	Short("r"),
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
	Short("p"),
).String()

var prefix_colors = NewFlag(
	Name("prefix-colors"),
	Description("Colors for the prefixes."),
	Short("c"),
).String()

var TimestampFormat = NewFlag(
	Name("timestamp-format"),
	Description("Format for the timestamps."),
	Default("yyyy-MM-dd HH:mm:ss.SSS"),
).String()

var kill_others = NewFlag(
	Name("kill-others"),
	Description("Kill other processes when one ends."),
	Default(false),
	Short("k"),
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

func main() {
	ParseFlags()
	commands := LoadCommands()
	config := LoadConfig(commands)

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	scheduler := NewScheduler(
		WithCommands(commands),
		WithMaxProcesses(config.MaxProcesses),
	)

	go scheduler.Start(ctx)

	printer := NewPrinter(ctx, PrinterConfig{
		raw:          config.Raw,
		group:        config.Group,
		timings:      config.Timings,
		timingFormat: config.TimestampFormat,
		color:        !config.NoColor,
		styles:       GenerateCommandStyles(commands, config.Hide),
	})

	for _, command := range commands {
		task := NewTask(ctx, command, config, printer)
		scheduler.Schedule(task)
	}

	// wg.Wait()
	exitCodes := make([]int, 0, len(commands))
	completed := 0
outer:
	for completed < len(commands) {
		select {
		case <-ctx.Done():
			break outer
		case t := <-scheduler.Completed():
			exitCodes = append(exitCodes, t.ExitCode())
			if config.KillOthers || (!t.Success() && config.KillOthersOnFail) {
				// cancel all other tasks
				stop()
			}
			completed++
		}
	}
	//decide exit code
	if len(config.SuccessTerms) == 1 {
		switch config.SuccessTerms[0] {
		case "all":
			os.Exit(anyCodeNotZero(exitCodes))
		case "last":
			os.Exit(exitCodes[len(exitCodes)-1])
		case "first":
			os.Exit(exitCodes[0])
		default:
			os.Exit(anyCodeNotZero(exitCodes))
		}
	}
}

func anyCodeNotZero(codes []int) int {
	for _, code := range codes {
		if code != 0 {
			return code
		}
	}
	return 0
}
