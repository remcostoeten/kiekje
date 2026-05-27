package binds

import "testing"

func TestFormatDisplay(t *testing.T) {
	got := FormatDisplay("ALT SHIFT, R, exec, kiekje --capture")
	if got != "⇧⌥R" {
		t.Fatalf("got %q, want ⇧⌥R", got)
	}
}

func TestFormatTrayDisplay(t *testing.T) {
	got := FormatTrayDisplay("ALT SHIFT, R, exec, kiekje --capture")
	if got != "Alt+Shift+R" {
		t.Fatalf("got %q, want Alt+Shift+R", got)
	}
}

func TestTrayLabel(t *testing.T) {
	got := TrayLabel("Capture region", "Alt+Shift+R")
	if got != "Capture region  ·  Alt+Shift+R" {
		t.Fatalf("got %q", got)
	}
	if TrayLabel("Open", "") != "Open" {
		t.Fatal("expected title only when shortcut empty")
	}
}
