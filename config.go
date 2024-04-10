package main

import (
	"fmt"
	"strconv"
	"strings"
)

type Config struct {
	MaxProcesses     int      `json:"max-processes"`
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

func LoadConfig(commands []*Command) *Config {
	names := names.Get()
	name_separator := name_separator.Get()
	namesSlice := []string{}
	if strings.Contains(names, name_separator) {
		namesSlice = strings.Split(names, name_separator)
		for i, name := range namesSlice {
			println("me")
			commands[i].DisplayName = name
		}
	}

	return &Config{
		MaxProcesses:     calculateMaxProcesses(max_processes.Get(), len(commands)),
		Names:            namesSlice,
		NameSeparator:    name_separator,
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
func calculateMaxProcesses(max string, total int) int {
	if max == "" || max == "0" {
		return total
	}
	if strings.HasSuffix(max, "%") {
		percent, err := strconv.Atoi(strings.TrimSuffix(max, "%"))
		if err != nil {
			fmt.Printf("Error parsing max-processes: %v\n", err)
			return 0
		}
		return int(float64(total) * (float64(percent) / 100))
	} else {
		maxProcesses, err := strconv.Atoi(max)
		if err != nil {
			fmt.Printf("Error parsing max-processes: %v\n", err)
			return 0
		}
		return maxProcesses
	}
}
