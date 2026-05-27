package hyprland

import (
	"os"
	"strings"
	"testing"
)

func TestEnsureSourceLineAddsManagedSource(t *testing.T) {
	input := "monitor=,preferred\n"
	got, changed := EnsureSourceLine(input)
	if !changed {
		t.Fatal("expected changed=true")
	}
	if !strings.Contains(got, SourceLine()) {
		t.Fatalf("expected source line in %q", got)
	}
}

func TestEnsureSourceLineKeepsExistingKiekjeSource(t *testing.T) {
	input := "monitor=,preferred\n" + SourceLine() + "\n"
	got, changed := EnsureSourceLine(input)
	if changed {
		t.Fatal("expected changed=false when kiekje source already present")
	}
	if got != input {
		t.Fatalf("expected unchanged content, got %q", got)
	}
}

func TestEnsureSourceLineMigratesLegacyCheeseSource(t *testing.T) {
	input := "monitor=,preferred\nsource = ./cheese-bindings.conf\n"
	got, changed := EnsureSourceLine(input)
	if !changed {
		t.Fatal("expected changed=true")
	}
	if strings.Contains(got, "cheese-bindings.conf") {
		t.Fatalf("expected legacy source removed, got %q", got)
	}
	if !strings.Contains(got, SourceLine()) {
		t.Fatalf("expected kiekje source added, got %q", got)
	}
}

func TestBuildBindingsContentIncludesWindowRules(t *testing.T) {
	got := BuildBindingsContent("bind = CTRL, C, exec, \"kiekje\" --capture")
	if !strings.Contains(got, ManagedByComment) {
		t.Fatal("expected managed header")
	}
	if !strings.Contains(got, "match:title ^(Kiekje)$") {
		t.Fatal("expected window rules")
	}
	if !strings.Contains(got, "--capture") {
		t.Fatal("expected bind line")
	}
}

func TestSyncCreatesConfigAndSourceLine(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("HYPRLAND_INSTANCE_SIGNATURE", "")

	if err := Sync(`bind = CTRL, C, exec, "/usr/bin/kiekje" --capture`); err != nil {
		t.Fatal(err)
	}

	mainPath := home + "/.config/hypr/hyprland.conf"
	raw, err := os.ReadFile(mainPath)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(raw), SourceLine()) {
		t.Fatalf("expected source line in %q", string(raw))
	}

	bindsPath := home + "/.config/hypr/" + BindingsFileName
	binds, err := os.ReadFile(bindsPath)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(binds), "--capture") {
		t.Fatalf("expected bind line in %q", string(binds))
	}
}
