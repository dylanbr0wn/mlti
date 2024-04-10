package main

import (
	"encoding/json"
	"os"
	"regexp"
	"strings"
)

var (
	nodeFile   = "package.json"
	npmPrefix  = "npm run"
	pnpmPrefix = "pnpm"
	yarnPrefix = "yarn"
)

type Command struct {
	id          int
	Name        string
	Args        []string
	DisplayName string
}

type PackageJson struct {
	Scripts map[string]string `json:"scripts"`
}

func ParsePackageJson() ([]string, error) {
	// read package.json file
	data, err := os.ReadFile(nodeFile)
	if err != nil {
		return nil, err
	}

	// parse package.json file
	var packageJson PackageJson
	err = json.Unmarshal(data, &packageJson)
	if err != nil {
		return nil, err
	}

	var scripts []string
	for script := range packageJson.Scripts {
		scripts = append(scripts, script)
	}

	return scripts, nil
}

func FilterScripts(scripts []string, scriptRegex string) ([]string, error) {
	var filtered []string
	for _, script := range scripts {
		if scriptRegex != "" {
			rex, err := regexp.Compile(scriptRegex)
			if err != nil {
				return nil, err
			}
			if rex.MatchString(script) {
				filtered = append(filtered, script)
			}
		} else {
			filtered = append(filtered, script)
		}
	}
	return filtered, nil
}

func ExpandShorthandCommand(command string) string {
	if command == "npm" {
		return npmPrefix
	} else if command == "pnpm" {
		return pnpmPrefix
	} else if command == "yarn" {
		return yarnPrefix
	}
	return command
}

func LoadCommands() []*Command {
	var commands []*Command
	inputArg := GetArgs()
	var nodeScripts []string
	index := 0
	for _, arg := range inputArg {
		parts := strings.Split(arg, " ")

		if len(parts) == 0 {
			continue
		} else if len(parts) == 1 {
			// need to test if we need to expand the command
			part := parts[0]
			subparts := strings.SplitN(part, ":", 2)
			if len(subparts) == 2 {
				prefix := subparts[0]
				script := subparts[1]

				expandedCommand := ExpandShorthandCommand(prefix)

				if nodeScripts == nil {
					nodeScripts, _ = ParsePackageJson()
				}

				matchedScripts, _ := FilterScripts(nodeScripts, script)
				for _, matchedScript := range matchedScripts {
					fullCommandParts := strings.Split(expandedCommand+" "+matchedScript, " ")
					commands = append(commands, &Command{
						id:          index,
						Name:        fullCommandParts[0],
						Args:        fullCommandParts[1:],
						DisplayName: fullCommandParts[0],
					})
					index++
				}
			}
		} else {
			commands = append(commands, &Command{
				id:          index,
				Name:        parts[0],
				Args:        parts[1:],
				DisplayName: parts[0],
			})
			index++
		}
	}
	return commands
}
