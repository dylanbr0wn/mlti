package main

import (
	"context"
	"fmt"
	"os/exec"
	"time"
)

type Task struct {
	id       string
	cmd      *Command
	config   *Config
	exitCode int
	err      error
	printer  *Printer
}

func NewTask(ctx context.Context, cmd *Command, config *Config, printer *Printer) *Task {

	return &Task{
		id:      cmd.DisplayName,
		cmd:     cmd,
		config:  config,
		printer: printer,
	}
}

func (t *Task) Run(ctx context.Context) {

	cmd := exec.CommandContext(ctx, t.cmd.Name, t.cmd.Args...)
	cmd.Stdout = t
	cmd.Stderr = t
	for i := 0; i < t.config.RestartTries; i++ {
		if err := cmd.Run(); err != nil {
			if exitErr, ok := err.(*exec.ExitError); ok {
				t.exitCode = exitErr.ExitCode()
			}
			t.err = fmt.Errorf("Error running command %s: %w", t.cmd.Name, err)
			fmt.Print(t.err.Error())
			// want to delay, if there is a delay, every time before the last time
			if t.config.RestartAfter > 0 && i < t.config.RestartTries-1 {
				// sleep for a while before restarting
				time.Sleep(time.Duration(t.config.RestartAfter) * time.Second)
				fmt.Printf("Restarting %s\n", t.cmd.DisplayName)
			}
		} else {
			break
		}
	}
}

func (t *Task) ExitCode() int {
	return t.exitCode
}

func (t *Task) Error() error {
	return t.err
}

func (t *Task) Success() bool {
	return t.exitCode == 0
}

func (t *Task) Write(b []byte) (int, error) {
	t.printer.Send(NewReport(t.cmd.id, string(b)))
	return len(b), nil
}
