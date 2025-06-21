package main

import (
	"fmt"
	"os"
	"os/exec"
	"strings"
)

func main() {
	input := "../../data/fonts/Virgil.woff2"
	if len(os.Args) > 1 {
		input = os.Args[1]
	}

	output := strings.TrimSuffix(input, ".woff2") + ".ttf"
	if len(os.Args) > 2 {
		output = os.Args[2]
	}

	if _, err := os.Stat(input); os.IsNotExist(err) {
		fmt.Printf("Error: %s not found\n", input)
		os.Exit(1)
	}

	if tryCommand("woff2_decompress", input) {
		expected := strings.TrimSuffix(input, ".woff2") + ".ttf"
		if expected != output {
			os.Rename(expected, output)
		}
	} else if tryPython(input, output) {
		// success
	} else {
		fmt.Println("Install: brew install woff2 or pip install fonttools")
		os.Exit(1)
	}

	fmt.Printf("Converted: %s\n", output)
}

func tryCommand(cmd string, args ...string) bool {
	if _, err := exec.LookPath(cmd); err != nil {
		return false
	}
	return exec.Command(cmd, args...).Run() == nil
}

func tryPython(input, output string) bool {
	script := fmt.Sprintf("from fontTools.ttLib import TTFont; TTFont('%s').save('%s')", input, output)
	return tryCommand("python3", "-c", script)
}
