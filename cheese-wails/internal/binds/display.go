package binds

import (
	"encoding/json"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

var modDisplay = map[string]string{
	"SUPER": "⌘",
	"SHIFT": "⇧",
	"CTRL":  "⌃",
	"ALT":   "⌥",
}

var modDisplayOrder = []string{"SHIFT", "SUPER", "CTRL", "ALT"}

var modTrayOrder = []string{"CTRL", "ALT", "SHIFT", "SUPER"}

var modTrayNames = map[string]string{
	"SUPER": "Super",
	"SHIFT": "Shift",
	"CTRL":  "Ctrl",
	"ALT":   "Alt",
}

type appState struct {
	Binds map[string]string `json:"binds"`
}

func statePath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".config", "kiekje", "state.json"), nil
}

func loadState() (appState, error) {
	path, err := statePath()
	if err != nil {
		return appState{}, err
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		return appState{}, err
	}
	var state appState
	if err := json.Unmarshal(raw, &state); err != nil {
		return appState{}, err
	}
	return state, nil
}

// DisplayForAction reads ~/.config/kiekje/state.json and formats the bind for in-app UI.
func DisplayForAction(action string) string {
	state, err := loadState()
	if err != nil || state.Binds == nil {
		return ""
	}
	return FormatDisplay(state.Binds[action])
}

// TrayDisplayForAction formats a bind for the system tray (plain text, e.g. Alt+Shift+R).
func TrayDisplayForAction(action string) string {
	state, err := loadState()
	if err != nil || state.Binds == nil {
		return ""
	}
	return FormatTrayDisplay(state.Binds[action])
}

// TrayDisplays returns tray-formatted shortcuts for all configured binds.
func TrayDisplays() map[string]string {
	state, err := loadState()
	if err != nil || state.Binds == nil {
		return nil
	}
	out := make(map[string]string, len(state.Binds))
	for action, line := range state.Binds {
		if shortcut := FormatTrayDisplay(line); shortcut != "" {
			out[action] = shortcut
		}
	}
	return out
}

// FormatDisplay turns a Hyprland-style bind line into compact symbols (e.g. ⌥⇧R).
func FormatDisplay(line string) string {
	parsed := parseBindLine(line)
	if parsed == nil {
		return ""
	}
	var mods strings.Builder
	for _, mod := range modDisplayOrder {
		if parsed.modifiers[mod] {
			mods.WriteString(modDisplay[mod])
		}
	}
	key := parsed.key
	if len(key) == 1 {
		key = strings.ToUpper(key)
	}
	return mods.String() + key
}

// FormatTrayDisplay turns a bind line into readable text for GTK tray menus.
func FormatTrayDisplay(line string) string {
	parsed := parseBindLine(line)
	if parsed == nil {
		return ""
	}
	parts := make([]string, 0, 4)
	for _, mod := range modTrayOrder {
		if parsed.modifiers[mod] {
			parts = append(parts, modTrayNames[mod])
		}
	}
	key := parsed.key
	if len(key) == 1 {
		key = strings.ToUpper(key)
	} else {
		key = strings.ToLower(key)
		if len(key) > 0 {
			key = strings.ToUpper(key[:1]) + key[1:]
		}
	}
	parts = append(parts, key)
	return strings.Join(parts, "+")
}

// TrayLabel appends a readable shortcut suffix for tray menu items.
func TrayLabel(title, shortcut string) string {
	shortcut = strings.TrimSpace(shortcut)
	if shortcut == "" || shortcut == "-" {
		return title
	}
	return title + "  ·  " + shortcut
}

type parsedBind struct {
	modifiers map[string]bool
	key       string
}

func parseBindLine(line string) *parsedBind {
	line = strings.TrimSpace(line)
	if line == "" {
		return nil
	}
	line = regexp.MustCompile(`(?i)^bind\w*\s*=\s*`).ReplaceAllString(line, "")
	parts := strings.Split(line, ",")
	trimmed := make([]string, 0, len(parts))
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part != "" {
			trimmed = append(trimmed, part)
		}
	}
	if len(trimmed) < 2 {
		return nil
	}
	mods := make(map[string]bool)
	for _, mod := range strings.Fields(strings.ToUpper(trimmed[0])) {
		mods[mod] = true
	}
	key := strings.ToUpper(trimmed[1])
	if key == "" {
		return nil
	}
	return &parsedBind{modifiers: mods, key: key}
}
