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
	details   string
}

func NewReport(id int, content string) Report {
	return Report{
		commandId: id,
		details:   content,
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

func (p *Printer) Send(report Report) {
	p.queueChan <- report
}

func (p *Printer) format(report Report) string {
	if style, ok := p.config.styles[report.commandId]; ok {
		// check if its hidden
		if style.show {

			prefix := style.displayName
			if p.config.timings {
				prefix = fmt.Sprintf("[%s] %s", time.Now().Format(p.config.timingFormat), prefix)
			}

			styledPrefix := style.style.Bold(true).Render(prefix)
			styledDetails := style.style.Render(strings.TrimSpace(report.details))

			return fmt.Sprintf("%s: %s\n", styledPrefix, styledDetails)
		}
	}
	return "\n"
}

func (p *Printer) Print(ctx context.Context) {
	for {
		select {
		case <-ctx.Done():
			return
		case report := <-p.queueChan:
			p.print(report)
		}
	}
}

func (p *Printer) print(report Report) {
	if content := p.format(report); content != "" {
		fmt.Print(content)
	}
}

func (p *Printer) Group(ctx context.Context) {
outer:
	for {
		select {
		case <-ctx.Done():
			break outer
		case report := <-p.queueChan:
			p.group(report)
		}
	}
	for _, g := range p.groupedReports {
		for _, r := range g {
			fmt.Print(r)
		}
	}
}

func (p *Printer) group(report Report) {
	if content := p.format(report); content != "" {
		if _, ok := p.groupedReports[report.commandId]; !ok {
			p.groupedReports[report.commandId] = append(make([]string, 256), content)
		} else {
			p.groupedReports[report.commandId] = append(p.groupedReports[report.commandId], content)
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
