package main

import (
	"context"
	"encoding/base64"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strings"

	"github.com/wailsapp/wails/v2/pkg/runtime"
)

type App struct {
	ctx context.Context
}

type CaptureResult struct {
	Path string `json:"path"`
	Data string `json:"data"`
}

type ConfigFile struct {
	Path    string `json:"path"`
	Name    string `json:"name"`
	Preview string `json:"preview"`
}

type Shortcut struct {
	Source    string `json:"source"`
	Line      int    `json:"line"`
	Modifiers string `json:"modifiers"`
	Key       string `json:"key"`
	Action    string `json:"action"`
	Command   string `json:"command"`
}

type WaybarInfo struct {
	Height    string `json:"height"`
	MarginTop string `json:"marginTop"`
}

type HyprlandSnapshot struct {
	Settings  map[string]string `json:"settings"`
	Shortcuts []Shortcut        `json:"shortcuts"`
	Files     []ConfigFile      `json:"files"`
	Waybar    WaybarInfo        `json:"waybar"`
}

func NewApp() *App {
	return &App{}
}

func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
	runtime.WindowSetSize(ctx, 1366, 900)
	runtime.WindowCenter(ctx)
	runtime.WindowHide(ctx)
}

func (a *App) GetHyprlandSnapshot() (HyprlandSnapshot, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return HyprlandSnapshot{}, err
	}

	paths := []string{
		filepath.Join(home, ".config/dotfiles/configs/hypr/hyprland.conf"),
		filepath.Join(home, ".config/dotfiles/configs/hypr/themes/theme.conf"),
		filepath.Join(home, ".config/dotfiles/configs/hypr/windowrules.conf"),
		filepath.Join(home, ".config/dotfiles/configs/hypr/keybindings.conf"),
		filepath.Join(home, ".config/dotfiles/configs/waybar/config"),
		filepath.Join(home, ".config/dotfiles/configs/waybar/config.jsonc"),
		filepath.Join(home, ".config/dotfiles/configs/waybar/user-style.css"),
	}

	files := make([]ConfigFile, 0, len(paths))
	settings := map[string]string{}
	shortcuts := []Shortcut{}

	bindRe := regexp.MustCompile(`^\s*(bindd?|bindm)\s*=\s*(.+)$`)
	generalRe := regexp.MustCompile(`^\s*(gaps_in|gaps_out|border_size|layout|rounding|exclusive|position|height|margin-top|margin-bottom|margin-left|margin-right)\s*=\s*(.+)$`)

	for _, path := range paths {
		raw, readErr := os.ReadFile(path)
		if readErr != nil {
			continue
		}

		lines := strings.Split(string(raw), "\n")
		preview := strings.Join(lines[:min(12, len(lines))], "\n")
		files = append(files, ConfigFile{
			Path:    path,
			Name:    filepath.Base(path),
			Preview: preview,
		})

		for i, line := range lines {
			if m := generalRe.FindStringSubmatch(line); len(m) == 3 {
				settings[m[1]] = strings.TrimSpace(m[2])
			}

			if m := bindRe.FindStringSubmatch(line); len(m) == 3 {
				fields := splitBindLine(m[2])
				if len(fields) >= 2 {
					shortcuts = append(shortcuts, Shortcut{
						Source:    filepath.Base(path),
						Line:      i + 1,
						Modifiers: fields[0],
						Key:       fields[1],
						Action:    fieldAt(fields, 2),
						Command:   strings.Join(fields[3:], ", "),
					})
				}
			}
		}
	}

	sort.SliceStable(shortcuts, func(i, j int) bool {
		if shortcuts[i].Source == shortcuts[j].Source {
			return shortcuts[i].Line < shortcuts[j].Line
		}
		return shortcuts[i].Source < shortcuts[j].Source
	})

	return HyprlandSnapshot{
		Settings:  settings,
		Shortcuts: shortcuts,
		Files:     files,
		Waybar: WaybarInfo{
			Height:    settings["height"],
			MarginTop: settings["margin-top"],
		},
	}, nil
}

func splitBindLine(raw string) []string {
	parts := strings.Split(raw, ",")
	out := make([]string, 0, len(parts))
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part != "" {
			out = append(out, part)
		}
	}
	return out
}

func fieldAt(fields []string, idx int) string {
	if idx >= 0 && idx < len(fields) {
		return fields[idx]
	}
	return ""
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func (a *App) CaptureRegion() (CaptureResult, error) {
	geometry, err := exec.Command("slurp", "-b", "000000AA", "-s", "ff550066", "-c", "ffffffcc", "-B", "111111cc").Output()
	if err != nil {
		return CaptureResult{}, err
	}

	geom := strings.TrimSpace(string(geometry))
	if geom == "" {
		return CaptureResult{}, fmt.Errorf("no region selected")
	}

	tmpDir := os.TempDir()
	tmpFile, err := os.CreateTemp(tmpDir, "cheese-*.png")
	if err != nil {
		return CaptureResult{}, err
	}
	path := tmpFile.Name()
	tmpFile.Close()

	if err := exec.Command("grim", "-g", geom, path).Run(); err != nil {
		return CaptureResult{}, err
	}

	bytes, err := os.ReadFile(path)
	if err != nil {
		return CaptureResult{}, err
	}

	return CaptureResult{
		Path: path,
		Data: base64.StdEncoding.EncodeToString(bytes),
	}, nil
}

func (a *App) SaveImage(base64Data string, outputPath string) error {
	raw := base64Data
	if idx := strings.Index(raw, ","); idx >= 0 && strings.HasPrefix(raw, "data:image/") {
		raw = raw[idx+1:]
	}

	bytes, err := base64.StdEncoding.DecodeString(raw)
	if err != nil {
		return err
	}

	if err := os.MkdirAll(filepath.Dir(outputPath), 0o755); err != nil {
		return err
	}

	return os.WriteFile(outputPath, bytes, 0o644)
}
