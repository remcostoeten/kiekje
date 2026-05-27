package main

import "testing"

func TestSlurpBoxLabelUsesFirstNonEmpty(t *testing.T) {
	if got := slurpBoxLabel("", "  Firefox  "); got != "Firefox" {
		t.Fatalf("expected trimmed title, got %q", got)
	}
}

func TestSlurpBoxLabelStripsNewlines(t *testing.T) {
	if got := slurpBoxLabel("line1\nline2"); got != "line1 line2" {
		t.Fatalf("expected newline replaced, got %q", got)
	}
}

func TestFormatSlurpBox(t *testing.T) {
	got := formatSlurpBox(10, 20, 300, 200, "Terminal")
	want := "10,20 300x200 Terminal"
	if got != want {
		t.Fatalf("got %q want %q", got, want)
	}
}

func TestParseSlurpGeometryIgnoresLabel(t *testing.T) {
	x, y, w, h, err := parseSlurpGeometry("100,200 640x480 Terminal")
	if err != nil {
		t.Fatal(err)
	}
	if x != 100 || y != 200 || w != 640 || h != 480 {
		t.Fatalf("unexpected geometry: %d,%d %dx%d", x, y, w, h)
	}
}

func TestIsCheeseWindowLabel(t *testing.T) {
	if !isCheeseWindowLabel("Cheese") {
		t.Fatal("expected Cheese to match")
	}
	if isCheeseWindowLabel("Firefox") {
		t.Fatal("expected Firefox not to match")
	}
}
