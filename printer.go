package main

import (
	"context"
	"fmt"
	"io"
	"slices"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
)

type Report struct {
	commandId int
	content   string
}

func NewReport(id int, content string) Report {
	return Report{
		commandId: id,
		content:   content,
	}
}

type PrinterConfig struct {
	raw          bool
	timingFormat string
	timings      bool
	group        bool
	color        bool
	styles       map[int]CommandStyle
}

type Printer struct {
	config         PrinterConfig
	groupedReports map[int][]string
	io.Writer
	queueChan chan Report
}

func NewPrinter(ctx context.Context, config PrinterConfig) *Printer {
	printer := &Printer{
		config:         config,
		queueChan:      make(chan Report, 256),
		groupedReports: make(map[int][]string),
	}
	if config.group {
		go printer.Group(ctx)
	} else {
		go printer.Print(ctx)
	}
	return printer
}

func (r *Printer) Send(report Report) {
	r.queueChan <- report
}

func (r *Printer) Print(ctx context.Context) {
	for {
		select {
		case <-ctx.Done():
			return
		case report := <-r.queueChan:
			r.print(report)
		}
	}
}

func (r *Printer) Group(ctx context.Context) {
outer:
	for {
		select {
		case <-ctx.Done():
			break outer
		case report := <-r.queueChan:
			r.group(report)
		}
	}
	for _, g := range r.groupedReports {
		for _, r := range g {
			fmt.Print(r)
		}
	}
}

func (r *Printer) group(report Report) {
	if style, ok := r.config.styles[report.commandId]; ok {
		// check if its hidden
		if style.show {

			prefix := style.displayName
			if r.config.timings {
				prefix = fmt.Sprintf("[%s] %s", time.Now().Format(r.config.timingFormat), prefix)
			}

			fmt.Sprintf("%s: %s", style.style.Bold(true).Render(prefix), style.style.Render(strings.TrimSpace(report.content)))
			r.pushToGroup(report.commandId)
		}
	}
}

func (r *Printer) pushToGroup(id int, content string) {
	if _, ok := r.groupedReports[id]; !ok {
		r.groupedReports[id] = make([]string, 256)
		r.groupedReports[id] = append(r.groupedReports[id], content)
	} else {
		r.groupedReports[id] = append(r.groupedReports[id], content)
	}
}

func (r *Printer) print(report Report) {
	if style, ok := r.config.styles[report.commandId]; ok {
		// check if its hidden
		if style.show {
			fmt.Printf("%s: %s\n", style.style.Bold(true).Render(style.displayName), style.style.Render(strings.TrimSpace(report.content)))
		}
	}
}

type CommandStyle struct {
	show        bool
	style       lipgloss.Style
	displayName string
}

func GenerateCommandStyles(commands []*Command, hidden []string) map[int]CommandStyle {
	colorFactory := NewColorFactory(Pastel)
	styles := make(map[int]CommandStyle)
	for _, command := range commands {
		if slices.Contains(hidden, fmt.Sprintf("%d", command.id)) || slices.Contains(hidden, command.DisplayName) {
			styles[command.id] = CommandStyle{
				show: false,
			}
		} else {
			styles[command.id] = CommandStyle{
				show:        true,
				style:       lipgloss.NewStyle().Foreground(lipgloss.Color(colorFactory.Generate())),
				displayName: command.DisplayName,
			}
		}
	}
	return styles
}
