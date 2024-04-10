package main

import (
	"context"
	"fmt"
	"os/exec"
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
	if err := cmd.Run(); err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			t.exitCode = exitErr.ExitCode()
		}
		t.err = fmt.Errorf("Error running command %s: %w", t.cmd.Name, err)
		fmt.Printf("Error running command %s: %v\n", t.cmd.Name, err)
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
