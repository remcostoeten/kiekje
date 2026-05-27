package main

import "testing"

func TestChooseSaveDirTakesPriorityOverCapture(t *testing.T) {
	args := []string{"--capture", "--choose-save-dir"}
	if primarySecondInstanceAction(args) != "--choose-save-dir" {
		t.Fatalf("expected choose-save-dir priority, got %q", primarySecondInstanceAction(args))
	}
}

func TestCaptureWhenOnlyCaptureArg(t *testing.T) {
	args := []string{"--capture"}
	if primarySecondInstanceAction(args) != "--capture" {
		t.Fatalf("expected capture, got %q", primarySecondInstanceAction(args))
	}
}

func primarySecondInstanceAction(args []string) string {
	for _, arg := range args {
		if trimArg(arg) == "--choose-save-dir" {
			return "--choose-save-dir"
		}
	}
	for _, arg := range args {
		switch trimArg(arg) {
		case "--capture":
			return "--capture"
		case "--show":
			return "--show"
		}
	}
	return ""
}

func trimArg(arg string) string {
	for len(arg) > 0 && (arg[0] == ' ' || arg[0] == '\t') {
		arg = arg[1:]
	}
	for len(arg) > 0 && (arg[len(arg)-1] == ' ' || arg[len(arg)-1] == '\t') {
		arg = arg[:len(arg)-1]
	}
	return arg
}
