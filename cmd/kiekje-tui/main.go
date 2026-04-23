package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

type captureMode string

const (
	modeRegion     captureMode = "region"
	modeFullscreen captureMode = "fullscreen"
	modeWindow     captureMode = "window"
)

type settings struct {
	DelayMS            uint64      `json:"delay_ms"`
	DefaultSavePath    string      `json:"default_save_location"`
	CopyToClipboard    bool        `json:"copy_to_clipboard"`
	CloseAfterCopy     bool        `json:"close_after_copy"`
	OpenAfterSave      bool        `json:"open_after_save"`
	OpenEditor         bool        `json:"open_editor"`
	DefaultCaptureMode captureMode `json:"default_capture_mode"`
	AutoSave           bool        `json:"auto_save"`
	TrayAutostart      bool        `json:"tray_autostart"`
	ShortcutRegion     string      `json:"shortcut_region"`
	ShortcutFullscreen string      `json:"shortcut_fullscreen"`
	ShortcutWindow     string      `json:"shortcut_window"`
	FilenameTemplate   string      `json:"filename_template"`
}

type menu int

const (
	mainMenu menu = iota
	captureMenu
	devMenu
	featureMenu
	settingsMenu
	inputDelayMenu
	inputModeMenu
	inputTagMenu
	inputFeatureMenu
	inputReleaseTagMenu
)

type model struct {
	current      menu
	cursor       int
	settings     settings
	features     []featureItem
	status       string
	input        string
	appBin       string
	configPath   string
	repoRoot     string
	mainItems    []string
	captureItems []string
	width        int
	height       int
}

type captureFinishedMsg struct {
	status string
}

type commandFinishedMsg struct {
	status string
}

type featureItem struct {
	Text    string `json:"text"`
	Checked bool   `json:"checked"`
}

var (
	titleStyle        = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("12"))
	headerStyle       = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("15"))
	subtitleStyle     = lipgloss.NewStyle().Foreground(lipgloss.Color("8"))
	normalStyle       = lipgloss.NewStyle().Foreground(lipgloss.Color("252"))
	selectedStyle     = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("11"))
	selectedMarkerSty = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("11"))
	okStyle           = lipgloss.NewStyle().Foreground(lipgloss.Color("10"))
	errStyle          = lipgloss.NewStyle().Foreground(lipgloss.Color("9"))
	hintStyle         = lipgloss.NewStyle().Foreground(lipgloss.Color("8"))
	panelStyle        = lipgloss.NewStyle().Padding(0, 1)
	dividerStyle      = lipgloss.NewStyle().Foreground(lipgloss.Color("238"))
	inputStyle        = lipgloss.NewStyle().Foreground(lipgloss.Color("15")).Bold(true)
)

func main() {
	cfgPath := resolveConfigPath()
	cfg, err := loadOrCreateSettings(cfgPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to load settings: %v\n", err)
		os.Exit(1)
	}
	repoRoot, err := resolveRepoRoot()
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to resolve repo root: %v\n", err)
		os.Exit(1)
	}
	features, err := loadFeatureList(repoRoot)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to load feature list: %v\n", err)
		os.Exit(1)
	}

	m := model{
		current:    mainMenu,
		settings:   cfg,
		features:   features,
		configPath: cfgPath,
		repoRoot:   repoRoot,
		mainItems: []string{
			"Capture",
			"Dev / Release",
			"Settings",
			"Show settings",
			"Quit",
		},
		captureItems: []string{
			"Region",
			"Fullscreen",
			"Window (active Hyprland window)",
			"Back",
		},
	}

	p := tea.NewProgram(m, tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		fmt.Fprintf(os.Stderr, "kiekje-tui failed: %v\n", err)
		os.Exit(1)
	}
}

func (m model) Init() tea.Cmd { return nil }

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil
	case captureFinishedMsg:
		m.status = msg.status
		return m, nil
	case commandFinishedMsg:
		m.status = msg.status
		return m, nil
	case tea.KeyMsg:
		key := msg.String()
		switch m.current {
		case mainMenu:
			return updateMainMenu(m, key)
		case captureMenu:
			return updateCaptureMenu(m, key)
		case devMenu:
			return updateDevMenu(m, key)
		case featureMenu:
			return updateFeatureMenu(m, key)
		case settingsMenu:
			return updateSettingsMenu(m, key)
		case inputDelayMenu:
			return updateDelayInput(m, key)
		case inputModeMenu:
			return updateModeInput(m, key)
		case inputTagMenu:
			return updateTagInput(m, key)
		case inputFeatureMenu:
			return updateFeatureInput(m, key)
		case inputReleaseTagMenu:
			return updateReleaseTagInput(m, key)
		}
	}
	return m, nil
}

func updateMainMenu(m model, key string) (tea.Model, tea.Cmd) {
	switch key {
	case "q", "ctrl+c":
		return m, tea.Quit
	case "up", "k":
		if m.cursor > 0 {
			m.cursor--
		}
	case "down", "j":
		if m.cursor < len(m.mainItems)-1 {
			m.cursor++
		}
	case "enter":
		switch m.cursor {
		case 0:
			m.current = captureMenu
			m.cursor = 0
		case 1:
			m.current = devMenu
			m.cursor = 0
		case 2:
			m.current = settingsMenu
			m.cursor = 0
		case 3:
			m.status = formatSettings(m.settings)
		case 4:
			return m, tea.Quit
		}
	}
	return m, nil
}

func updateCaptureMenu(m model, key string) (tea.Model, tea.Cmd) {
	switch key {
	case "q", "ctrl+c":
		return m, tea.Quit
	case "esc", "b":
		m.current = mainMenu
		m.cursor = 0
		return m, nil
	case "up", "k":
		if m.cursor > 0 {
			m.cursor--
		}
	case "down", "j":
		if m.cursor < len(m.captureItems)-1 {
			m.cursor++
		}
	case "enter":
		switch m.cursor {
		case 0:
			m.status = "Running region capture..."
			return m, captureCmd(modeRegion)
		case 1:
			m.status = "Running fullscreen capture..."
			return m, captureCmd(modeFullscreen)
		case 2:
			m.status = "Running window capture..."
			return m, captureCmd(modeWindow)
		case 3:
			m.current = mainMenu
			m.cursor = 0
		}
	}
	return m, nil
}

func updateDevMenu(m model, key string) (tea.Model, tea.Cmd) {
	items := devItems()
	switch key {
	case "q", "ctrl+c":
		return m, tea.Quit
	case "esc", "b":
		m.current = mainMenu
		m.cursor = 0
		return m, nil
	case "up", "k":
		if m.cursor > 0 {
			m.cursor--
		}
	case "down", "j":
		if m.cursor < len(items)-1 {
			m.cursor++
		}
	case "enter":
		switch m.cursor {
		case 0:
			m.status = "Running cargo build..."
			return m, runRepoCommandCmd(m.repoRoot, "cargo", "build")
		case 1:
			m.status = "Running cargo build --release..."
			return m, runRepoCommandCmd(m.repoRoot, "cargo", "build", "--release")
		case 2:
			m.status = "Running cargo test..."
			return m, runRepoCommandCmd(m.repoRoot, "cargo", "test")
		case 3:
			m.status = "Running cargo clippy..."
			return m, runRepoCommandCmd(
				m.repoRoot,
				"cargo",
				"clippy",
				"--all-targets",
				"--all-features",
				"--",
				"-D",
				"warnings",
			)
		case 4:
			m.status = "Running cargo run -- --doctor..."
			return m, runRepoCommandCmd(m.repoRoot, "cargo", "run", "--", "--doctor")
		case 5:
			m.status = "Running go test ./..."
			return m, runRepoCommandCmd(filepath.Join(m.repoRoot, "cmd", "kiekje-tui"), "go", "test", "./...")
		case 6:
			m.status = "Running go vet ./..."
			return m, runRepoCommandCmd(filepath.Join(m.repoRoot, "cmd", "kiekje-tui"), "go", "vet", "./...")
		case 7:
			m.status = "Running git status --short..."
			return m, runRepoCommandCmd(m.repoRoot, "git", "status", "--short")
		case 8:
			m.current = inputTagMenu
			m.input = ""
			return m, nil
		case 9:
			m.status = "Running git push..."
			return m, runRepoCommandCmd(m.repoRoot, "git", "push")
		case 10:
			m.status = "Running git push --tags..."
			return m, runRepoCommandCmd(m.repoRoot, "git", "push", "--tags")
		case 11:
			m.status = "Packaging release bundle..."
			return m, runRepoCommandCmd(m.repoRoot, "scripts/package-release.sh")
		case 12:
			m.current = inputReleaseTagMenu
			m.input = ""
			return m, nil
		case 13:
			m.current = featureMenu
			m.cursor = 0
			return m, nil
		case 14:
			m.current = mainMenu
			m.cursor = 0
			return m, nil
		}
	}
	return m, nil
}

func updateFeatureMenu(m model, key string) (tea.Model, tea.Cmd) {
	switch key {
	case "q", "ctrl+c":
		return m, tea.Quit
	case "esc", "b":
		m.current = devMenu
		m.cursor = 0
		return m, nil
	case "up", "k":
		if m.cursor > 0 {
			m.cursor--
		}
	case "down", "j":
		if m.cursor < len(featureMenuItems(m.features))-1 {
			m.cursor++
		}
	case "space", "enter":
		if len(m.features) == 0 {
			if m.cursor == 0 {
				m.current = inputFeatureMenu
				m.input = ""
			} else {
				m.current = devMenu
				m.cursor = 0
			}
			return m, nil
		}

		switch {
		case m.cursor < len(m.features):
			m.features[m.cursor].Checked = !m.features[m.cursor].Checked
			if err := saveFeatureList(m.repoRoot, m.features); err != nil {
				m.status = "failed saving feature list: " + err.Error()
			} else {
				m.status = "feature updated"
			}
			return m, nil
		case m.cursor == len(m.features):
			m.current = inputFeatureMenu
			m.input = ""
			return m, nil
		default:
			m.current = devMenu
			m.cursor = 0
			return m, nil
		}
	case "d", "backspace":
		if len(m.features) == 0 || m.cursor >= len(m.features) {
			return m, nil
		}
		removed := m.features[m.cursor].Text
		m.features = append(m.features[:m.cursor], m.features[m.cursor+1:]...)
		if m.cursor >= len(featureMenuItems(m.features)) && m.cursor > 0 {
			m.cursor--
		}
		if err := saveFeatureList(m.repoRoot, m.features); err != nil {
			m.status = "failed saving feature list: " + err.Error()
		} else {
			m.status = "removed feature: " + removed
		}
		return m, nil
	}
	return m, nil
}

func updateSettingsMenu(m model, key string) (tea.Model, tea.Cmd) {
	items := settingsItems(m.settings)
	switch key {
	case "q", "ctrl+c":
		return m, tea.Quit
	case "esc", "b":
		m.current = mainMenu
		m.cursor = 0
		return m, nil
	case "up", "k":
		if m.cursor > 0 {
			m.cursor--
		}
	case "down", "j":
		if m.cursor < len(items)-1 {
			m.cursor++
		}
	case "enter":
		switch m.cursor {
		case 0:
			m.settings.CopyToClipboard = !m.settings.CopyToClipboard
			m.status = saveSettingsAndStatus(m)
		case 1:
			m.settings.OpenEditor = !m.settings.OpenEditor
			m.status = saveSettingsAndStatus(m)
		case 2:
			m.settings.AutoSave = !m.settings.AutoSave
			m.status = saveSettingsAndStatus(m)
		case 3:
			m.current = inputDelayMenu
			m.input = strconv.FormatUint(m.settings.DelayMS, 10)
		case 4:
			m.current = inputModeMenu
			m.input = string(m.settings.DefaultCaptureMode)
		case 5:
			m.status = formatSettings(m.settings)
		case 6:
			m.current = mainMenu
			m.cursor = 0
		}
	}
	return m, nil
}

func updateDelayInput(m model, key string) (tea.Model, tea.Cmd) {
	switch key {
	case "q", "ctrl+c":
		return m, tea.Quit
	case "esc", "b":
		m.current = settingsMenu
		m.input = ""
		m.cursor = 0
		return m, nil
	case "enter":
		if m.input == "" {
			m.status = "delay cannot be empty"
			m.current = settingsMenu
			return m, nil
		}
		v, err := strconv.ParseUint(m.input, 10, 64)
		if err != nil {
			m.status = "invalid delay value"
		} else {
			m.settings.DelayMS = v
			m.status = saveSettingsAndStatus(m)
		}
		m.current = settingsMenu
		m.input = ""
		m.cursor = 0
		return m, nil
	case "backspace":
		if len(m.input) > 0 {
			m.input = m.input[:len(m.input)-1]
		}
	default:
		if key >= "0" && key <= "9" {
			m.input += key
		}
	}
	return m, nil
}

func updateModeInput(m model, key string) (tea.Model, tea.Cmd) {
	switch key {
	case "q", "ctrl+c":
		return m, tea.Quit
	case "esc", "b":
		m.current = settingsMenu
		m.input = ""
		m.cursor = 0
		return m, nil
	case "enter":
		mode := strings.ToLower(strings.TrimSpace(m.input))
		switch mode {
		case "r", "region":
			m.settings.DefaultCaptureMode = modeRegion
		case "f", "fullscreen":
			m.settings.DefaultCaptureMode = modeFullscreen
		case "w", "window":
			m.settings.DefaultCaptureMode = modeWindow
		default:
			m.status = "invalid mode, use region/fullscreen/window"
			m.current = settingsMenu
			m.input = ""
			m.cursor = 0
			return m, nil
		}
		m.status = saveSettingsAndStatus(m)
		m.current = settingsMenu
		m.input = ""
		m.cursor = 0
		return m, nil
	case "backspace":
		if len(m.input) > 0 {
			m.input = m.input[:len(m.input)-1]
		}
	default:
		if len(key) == 1 {
			m.input += key
		}
	}
	return m, nil
}

func updateTagInput(m model, key string) (tea.Model, tea.Cmd) {
	switch key {
	case "q", "ctrl+c":
		return m, tea.Quit
	case "esc", "b":
		m.current = devMenu
		m.input = ""
		m.cursor = 0
		return m, nil
	case "enter":
		tag := strings.TrimSpace(m.input)
		if tag == "" {
			m.status = "tag cannot be empty"
			m.current = devMenu
			m.input = ""
			m.cursor = 0
			return m, nil
		}
		m.status = "Creating git tag " + tag + "..."
		m.current = devMenu
		m.input = ""
		m.cursor = 0
		return m, runRepoCommandCmd(m.repoRoot, "git", "tag", "-a", tag, "-m", tag)
	case "backspace":
		if len(m.input) > 0 {
			m.input = m.input[:len(m.input)-1]
		}
	default:
		if len(key) == 1 {
			m.input += key
		}
	}
	return m, nil
}

func updateFeatureInput(m model, key string) (tea.Model, tea.Cmd) {
	switch key {
	case "q", "ctrl+c":
		return m, tea.Quit
	case "esc", "b":
		m.current = featureMenu
		m.input = ""
		m.cursor = 0
		return m, nil
	case "enter":
		text := strings.TrimSpace(m.input)
		if text == "" {
			m.status = "feature text cannot be empty"
			m.current = featureMenu
			m.input = ""
			m.cursor = 0
			return m, nil
		}
		m.features = append(m.features, featureItem{Text: text})
		if err := saveFeatureList(m.repoRoot, m.features); err != nil {
			m.status = "failed saving feature list: " + err.Error()
		} else {
			m.status = "added feature: " + text
		}
		m.current = featureMenu
		m.input = ""
		m.cursor = 0
		return m, nil
	case "backspace":
		if len(m.input) > 0 {
			m.input = m.input[:len(m.input)-1]
		}
	default:
		if len(key) == 1 {
			m.input += key
		}
	}
	return m, nil
}

func updateReleaseTagInput(m model, key string) (tea.Model, tea.Cmd) {
	switch key {
	case "q", "ctrl+c":
		return m, tea.Quit
	case "esc", "b":
		m.current = devMenu
		m.input = ""
		m.cursor = 0
		return m, nil
	case "enter":
		tag := strings.TrimSpace(m.input)
		if tag == "" {
			m.status = "release tag cannot be empty"
			m.current = devMenu
			m.input = ""
			m.cursor = 0
			return m, nil
		}
		m.status = "Creating GitHub release " + tag + "..."
		m.current = devMenu
		m.input = ""
		m.cursor = 0
		return m, createReleaseCmd(m.repoRoot, tag, m.features)
	case "backspace":
		if len(m.input) > 0 {
			m.input = m.input[:len(m.input)-1]
		}
	default:
		if len(key) == 1 {
			m.input += key
		}
	}
	return m, nil
}

func (m model) View() string {
	switch m.current {
	case mainMenu:
		return renderPage("Main Menu", "j/k or arrows to move, Enter to select, b/Esc back, q quit", renderMenu(m.mainItems, m.cursor), m.status)
	case captureMenu:
		return renderPage("Capture", "j/k or arrows to move, Enter to run a capture, b/Esc back, q quit", renderMenu(m.captureItems, m.cursor), m.status)
	case devMenu:
		return renderPage("Dev / Release", "j/k or arrows to move, Enter to run a task, b/Esc back, q quit", renderMenu(devItems(), m.cursor), m.status)
	case featureMenu:
		return renderPage("Feature List", "Enter/Space toggles, d deletes, Add feature opens input, b back", renderMenu(featureMenuItems(m.features), m.cursor), m.status)
	case settingsMenu:
		return renderPage("Settings", "j/k or arrows to move, Enter to change a setting, b/Esc back, q quit", renderMenu(settingsItems(m.settings), m.cursor), m.status)
	case inputDelayMenu:
		return renderInputPage("Set Delay (ms)", "Type a number and press Enter. b/Esc/q cancels.", m.input, m.status)
	case inputModeMenu:
		return renderInputPage("Set Default Mode", "Type region, fullscreen, or window, then Enter. b/Esc/q cancels.", m.input, m.status)
	case inputTagMenu:
		return renderInputPage("Create Git Tag", "Type a tag like v0.0.1 and press Enter. b/Esc/q cancels.", m.input, m.status)
	case inputFeatureMenu:
		return renderInputPage("Add Feature", "Type a feature or release-note bullet and press Enter. b/Esc/q cancels.", m.input, m.status)
	case inputReleaseTagMenu:
		return renderInputPage("Create GitHub Release", "Type a tag like v0.0.1. Notes come from the feature list. b/Esc/q cancels.", m.input, m.status)
	}
	return renderPage("Main Menu", "j/k or arrows to move, Enter to select, b/Esc back, q quit", renderMenu(m.mainItems, m.cursor), m.status)
}

func renderMenu(items []string, cursor int) string {
	var b strings.Builder
	if len(items) == 0 {
		b.WriteString(subtitleStyle.Render("(empty)"))
		b.WriteString("\n")
		return b.String()
	}
	for i, item := range items {
		prefix := "  "
		itemStyle := normalStyle
		if i == cursor {
			prefix = selectedMarkerSty.Render("> ")
			itemStyle = selectedStyle
		}
		line := prefix + itemStyle.Render(item)
		b.WriteString(line)
		b.WriteString("\n")
	}
	return b.String()
}

func renderPage(title, subtitle, body, status string) string {
	var b strings.Builder
	b.WriteString(titleStyle.Render("kiekje-tui"))
	b.WriteString("\n")
	b.WriteString(headerStyle.Render(title))
	if subtitle != "" {
		b.WriteString("\n")
		b.WriteString(subtitleStyle.Render(subtitle))
	}
	b.WriteString("\n")
	b.WriteString(dividerStyle.Render(strings.Repeat("-", 44)))
	b.WriteString("\n\n")
	b.WriteString(panelStyle.Render(body))
	if status != "" {
		b.WriteString("\n\n")
		b.WriteString(renderStatus(status))
	}
	return b.String()
}

func renderInputPage(title, subtitle, value, status string) string {
	return renderPage(title, subtitle, renderField("> "+value)+"\n", status)
}

func renderField(text string) string {
	return inputStyle.Render(text)
}

func renderStatus(status string) string {
	lowered := strings.ToLower(status)
	if strings.HasPrefix(lowered, "error") || strings.Contains(lowered, "failed") {
		return errStyle.Render(status)
	}
	return okStyle.Render(status)
}

func settingsItems(s settings) []string {
	return []string{
		fmt.Sprintf("Toggle clipboard copy     [%s]", onOff(s.CopyToClipboard)),
		fmt.Sprintf("Toggle open editor        [%s]", onOff(s.OpenEditor)),
		fmt.Sprintf("Toggle auto-save          [%s]", onOff(s.AutoSave)),
		fmt.Sprintf("Set delay (ms)            [%d]", s.DelayMS),
		fmt.Sprintf("Set default mode          [%s]", s.DefaultCaptureMode),
		"Show settings",
		"Back",
	}
}

func devItems() []string {
	return []string{
		"Cargo build",
		"Cargo build --release",
		"Cargo test",
		"Cargo clippy -D warnings",
		"Run doctor",
		"Go test ./...",
		"Go vet ./...",
		"Git status --short",
		"Create git tag",
		"Git push",
		"Git push --tags",
		"Package release bundle",
		"Create GitHub release",
		"Feature list",
		"Back",
	}
}

func featureMenuItems(features []featureItem) []string {
	items := make([]string, 0, len(features)+2)
	for _, feature := range features {
		marker := "[ ]"
		if feature.Checked {
			marker = "[x]"
		}
		items = append(items, fmt.Sprintf("%s %s", marker, feature.Text))
	}
	items = append(items, "Add feature")
	items = append(items, "Back")
	return items
}

func onOff(v bool) string {
	if v {
		return "ON"
	}
	return "OFF"
}

func captureCmd(mode captureMode) tea.Cmd {
	bin, err := resolveAppBin()
	if err != nil {
		return func() tea.Msg {
			return captureFinishedMsg{status: "error: " + err.Error()}
		}
	}

	cmd := exec.Command(bin, string(mode))
	var stderr bytes.Buffer
	cmd.Stdout = os.Stdout
	cmd.Stderr = io.MultiWriter(os.Stderr, &stderr)
	cmd.Stdin = os.Stdin
	return tea.ExecProcess(cmd, func(err error) tea.Msg {
		if err == nil {
			return captureFinishedMsg{status: fmt.Sprintf("capture done (%s)", mode)}
		}
		return captureFinishedMsg{status: captureFailureStatus(mode, stderr.String(), err)}
	})
}

func runRepoCommandCmd(dir string, name string, args ...string) tea.Cmd {
	cmd := exec.Command(name, args...)
	cmd.Dir = dir
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Stdin = os.Stdin
	return tea.ExecProcess(cmd, func(err error) tea.Msg {
		commandLine := strings.Join(append([]string{name}, args...), " ")
		if err != nil {
			return commandFinishedMsg{status: commandLine + " failed: " + err.Error()}
		}
		return commandFinishedMsg{status: commandLine + " finished"}
	})
}

func createReleaseCmd(repoRoot, tag string, features []featureItem) tea.Cmd {
	return func() tea.Msg {
		body, err := renderReleaseNotes(tag, features)
		if err != nil {
			return commandFinishedMsg{status: "release notes failed: " + err.Error()}
		}

		tmp, err := os.CreateTemp("", "kiekje-release-notes-*.md")
		if err != nil {
			return commandFinishedMsg{status: "release notes temp file failed: " + err.Error()}
		}
		tmpPath := tmp.Name()
		defer os.Remove(tmpPath)

		if _, err := tmp.WriteString(body); err != nil {
			tmp.Close()
			return commandFinishedMsg{status: "release notes write failed: " + err.Error()}
		}
		if err := tmp.Close(); err != nil {
			return commandFinishedMsg{status: "release notes close failed: " + err.Error()}
		}

		cmd := exec.Command("gh", "release", "create", tag, "--title", tag, "--notes-file", tmpPath)
		cmd.Dir = repoRoot
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		cmd.Stdin = os.Stdin
		if err := cmd.Run(); err != nil {
			return commandFinishedMsg{status: "gh release create failed: " + err.Error()}
		}
		return commandFinishedMsg{status: "gh release create " + tag + " finished"}
	}
}

func renderReleaseNotes(tag string, features []featureItem) (string, error) {
	var b strings.Builder
	b.WriteString("# " + tag + "\n\n")
	if len(features) == 0 {
		b.WriteString("- Initial release\n")
		return b.String(), nil
	}

	b.WriteString("## Highlights\n\n")
	for _, feature := range features {
		prefix := "- "
		if feature.Checked {
			prefix = "- [x] "
		}
		b.WriteString(prefix + feature.Text + "\n")
	}
	return b.String(), nil
}

type dependencyFailure struct {
	tools    map[string]bool
	installs []string
}

func captureFailureStatus(mode captureMode, output string, runErr error) string {
	if strings.Contains(output, "Code: KIEKJE-E002") || strings.Contains(output, "Code: SCREENY-E002") {
		return fmt.Sprintf("capture canceled (%s)", mode)
	}

	if failure := parseDependencyFailure(output); failure != nil {
		if failure.tools["grim"] {
			if len(failure.installs) > 0 {
				return "missing grim: install required first (" + failure.installs[0] + ")"
			}
			return "missing grim: install required first"
		}
		if failure.tools["hyprctl"] && mode == modeWindow {
			return "window capture requires hyprctl; use fullscreen/region or install Hyprland tools"
		}
		if failure.tools["wl-copy"] {
			return "clipboard copy failed because wl-copy is missing; disable clipboard copy or install wl-clipboard"
		}
		if len(failure.installs) > 0 {
			return "missing dependencies: run install and retry (" + strings.Join(failure.installs, " | ") + ")"
		}
		return "missing dependencies: run `kiekje --doctor`"
	}

	output = strings.TrimSpace(output)
	if output != "" {
		return "capture failed: " + output
	}
	return "capture failed: " + runErr.Error()
}

func parseDependencyFailure(output string) *dependencyFailure {
	if !strings.Contains(output, "Code: KIEKJE-E001") && !strings.Contains(output, "Code: SCREENY-E001") {
		return nil
	}

	failure := &dependencyFailure{
		tools:    map[string]bool{},
		installs: []string{},
	}

	lines := strings.Split(output, "\n")
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "- ") {
			name := strings.TrimPrefix(trimmed, "- ")
			if idx := strings.Index(name, " "); idx > 0 {
				name = name[:idx]
			}
			failure.tools[name] = true
		}
		if strings.HasPrefix(trimmed, "Install: ") {
			cmd := strings.TrimPrefix(trimmed, "Install: ")
			if cmd != "" {
				failure.installs = append(failure.installs, cmd)
			}
		}
	}

	return failure
}

func buildCaptureApp() error {
	if _, err := exec.LookPath("cargo"); err != nil {
		return errors.New("cargo not found in PATH")
	}
	cmd := exec.Command("cargo", "build", "--release")
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Stdin = os.Stdin
	return cmd.Run()
}

func resolveAppBin() (string, error) {
	if envBin := strings.TrimSpace(os.Getenv("APP_BIN")); envBin != "" {
		if p, err := exec.LookPath(envBin); err == nil {
			return p, nil
		}
		if isExecutable(envBin) {
			return envBin, nil
		}
	}

	if p, err := exec.LookPath("kiekje"); err == nil {
		return p, nil
	}
	if p, err := exec.LookPath("capture-app"); err == nil {
		return p, nil
	}

	candidates := []string{
		"./target/release/kiekje",
		"./target/debug/kiekje",
		"./target/release/capture-app",
		"./target/debug/capture-app",
	}
	for _, c := range candidates {
		if isExecutable(c) {
			return c, nil
		}
	}

	if err := buildCaptureApp(); err == nil {
		if isExecutable("./target/release/kiekje") {
			return "./target/release/kiekje", nil
		}
		if isExecutable("./target/release/capture-app") {
			return "./target/release/capture-app", nil
		}
	}

	return "", errors.New("kiekje not found; build it with `cargo build --release` or set APP_BIN")
}

func isExecutable(path string) bool {
	info, err := os.Stat(path)
	if err != nil {
		return false
	}
	if info.IsDir() {
		return false
	}
	return info.Mode()&0111 != 0
}

func saveSettingsAndStatus(m model) string {
	if err := saveSettings(m.configPath, m.settings); err != nil {
		return "failed saving settings: " + err.Error()
	}
	return "settings saved"
}

func resolveConfigPath() string {
	if xdg := os.Getenv("XDG_CONFIG_HOME"); xdg != "" {
		return filepath.Join(xdg, "kiekje", "config.json")
	}
	home := os.Getenv("HOME")
	if home == "" {
		home = "."
	}
	return filepath.Join(home, ".config", "kiekje", "config.json")
}

func resolveLegacyConfigPath() string {
	if xdg := os.Getenv("XDG_CONFIG_HOME"); xdg != "" {
		return filepath.Join(xdg, "screeny", "config.json")
	}
	home := os.Getenv("HOME")
	if home == "" {
		home = "."
	}
	return filepath.Join(home, ".config", "screeny", "config.json")
}

func resolveRepoRoot() (string, error) {
	if root := strings.TrimSpace(os.Getenv("KIEKJE_REPO_ROOT")); root != "" && looksLikeRepoRoot(root) {
		return root, nil
	}
	if root := strings.TrimSpace(os.Getenv("SCREENY_REPO_ROOT")); root != "" && looksLikeRepoRoot(root) {
		return root, nil
	}

	if cwd, err := os.Getwd(); err == nil {
		if root, ok := findRepoRoot(cwd); ok {
			return root, nil
		}
	}

	if exe, err := os.Executable(); err == nil {
		if root, ok := findRepoRoot(filepath.Dir(exe)); ok {
			return root, nil
		}
	}

	return "", errors.New("could not find repository root with Cargo.toml")
}

func findRepoRoot(start string) (string, bool) {
	dir := start
	for {
		if looksLikeRepoRoot(dir) {
			return dir, true
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return "", false
		}
		dir = parent
	}
}

func looksLikeRepoRoot(dir string) bool {
	if _, err := os.Stat(filepath.Join(dir, "Cargo.toml")); err != nil {
		return false
	}
	if _, err := os.Stat(filepath.Join(dir, "cmd", "kiekje-tui", "go.mod")); err != nil {
		return false
	}
	return true
}

func featureListPath(repoRoot string) string {
	return filepath.Join(repoRoot, ".kiekje-tui-features.json")
}

func legacyFeatureListPath(repoRoot string) string {
	return filepath.Join(repoRoot, ".screeny-tui-features.json")
}

func loadFeatureList(repoRoot string) ([]featureItem, error) {
	path := featureListPath(repoRoot)
	b, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		legacyPath := legacyFeatureListPath(repoRoot)
		if legacy, legacyErr := os.ReadFile(legacyPath); legacyErr == nil {
			var features []featureItem
			if err := json.Unmarshal(legacy, &features); err != nil {
				return nil, err
			}
			return features, nil
		} else if !errors.Is(legacyErr, os.ErrNotExist) {
			return nil, legacyErr
		}

		features := []featureItem{
			{Text: "Wayland screenshot capture"},
			{Text: "GTK annotation editor"},
			{Text: "Tray controls and delay presets"},
		}
		if err := saveFeatureList(repoRoot, features); err != nil {
			return nil, err
		}
		return features, nil
	}
	if err != nil {
		return nil, err
	}

	var features []featureItem
	if err := json.Unmarshal(b, &features); err != nil {
		return nil, err
	}
	return features, nil
}

func saveFeatureList(repoRoot string, features []featureItem) error {
	path := featureListPath(repoRoot)
	body, err := json.MarshalIndent(features, "", "  ")
	if err != nil {
		return err
	}
	body = append(body, '\n')
	return os.WriteFile(path, body, 0o644)
}

func loadOrCreateSettings(path string) (settings, error) {
	if _, err := os.Stat(path); errors.Is(err, os.ErrNotExist) {
		legacyPath := resolveLegacyConfigPath()
		if legacyPath != path {
			if legacy, legacyErr := loadSettingsFromPath(legacyPath); legacyErr == nil {
				return legacy, nil
			} else if !errors.Is(legacyErr, os.ErrNotExist) {
				return settings{}, legacyErr
			}
		}

		d := defaultSettings()
		if err := saveSettings(path, d); err != nil {
			return settings{}, err
		}
		return d, nil
	}

	return loadSettingsFromPath(path)
}

func loadSettingsFromPath(path string) (settings, error) {
	b, err := os.ReadFile(path)
	if err != nil {
		return settings{}, err
	}
	var s settings
	if err := json.Unmarshal(b, &s); err != nil {
		return settings{}, err
	}
	if s.DefaultCaptureMode == "" {
		s.DefaultCaptureMode = modeRegion
	}
	if s.DefaultSavePath == "" {
		s.DefaultSavePath = defaultSettings().DefaultSavePath
	}
	if s.ShortcutRegion == "" {
		s.ShortcutRegion = defaultSettings().ShortcutRegion
	}
	if s.ShortcutFullscreen == "" {
		s.ShortcutFullscreen = defaultSettings().ShortcutFullscreen
	}
	if s.ShortcutWindow == "" {
		s.ShortcutWindow = defaultSettings().ShortcutWindow
	}
	if s.FilenameTemplate == "" {
		s.FilenameTemplate = defaultSettings().FilenameTemplate
	}
	return s, nil
}

func saveSettings(path string, s settings) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	b, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return err
	}
	b = append(b, '\n')
	return os.WriteFile(path, b, 0o644)
}

func defaultSettings() settings {
	home := os.Getenv("HOME")
	if home == "" {
		home = "."
	}
	return settings{
		DelayMS:            0,
		DefaultSavePath:    filepath.Join(home, "Pictures", "Screenshots"),
		CopyToClipboard:    true,
		CloseAfterCopy:     false,
		OpenAfterSave:      false,
		OpenEditor:         true,
		DefaultCaptureMode: modeRegion,
		AutoSave:           false,
		TrayAutostart:      false,
		ShortcutRegion:     "SUPER SHIFT, S",
		ShortcutFullscreen: "SUPER SHIFT, F",
		ShortcutWindow:     "SUPER SHIFT, W",
		FilenameTemplate:   "kiekje-{timestamp}-{mode}.png",
	}
}

func formatSettings(s settings) string {
	return fmt.Sprintf(
		"delay_ms=%d | clipboard=%t | editor=%t | auto_save=%t | mode=%s",
		s.DelayMS,
		s.CopyToClipboard,
		s.OpenEditor,
		s.AutoSave,
		s.DefaultCaptureMode,
	)
}
