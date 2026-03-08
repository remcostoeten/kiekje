package main

import (
	"encoding/json"
	"errors"
	"fmt"
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
	settingsMenu
	inputDelayMenu
	inputModeMenu
)

type model struct {
	current      menu
	cursor       int
	settings     settings
	status       string
	input        string
	appBin       string
	configPath   string
	mainItems    []string
	captureItems []string
	width        int
	height       int
}

var (
	titleStyle  = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("12"))
	headerStyle = lipgloss.NewStyle().Bold(true)
	normalStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("252"))
	okStyle     = lipgloss.NewStyle().Foreground(lipgloss.Color("10"))
	errStyle    = lipgloss.NewStyle().Foreground(lipgloss.Color("9"))
	hintStyle   = lipgloss.NewStyle().Foreground(lipgloss.Color("8"))
	cursorStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("11")).Bold(true)
)

func main() {
	cfgPath := resolveConfigPath()
	cfg, err := loadOrCreateSettings(cfgPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to load settings: %v\n", err)
		os.Exit(1)
	}

	m := model{
		current:    mainMenu,
		settings:   cfg,
		configPath: cfgPath,
		mainItems: []string{
			"Capture",
			"Settings",
			"Build capture-app",
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
		fmt.Fprintf(os.Stderr, "screeny-tui failed: %v\n", err)
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
	case tea.KeyMsg:
		key := msg.String()
		switch m.current {
		case mainMenu:
			return updateMainMenu(m, key)
		case captureMenu:
			return updateCaptureMenu(m, key)
		case settingsMenu:
			return updateSettingsMenu(m, key)
		case inputDelayMenu:
			return updateDelayInput(m, key)
		case inputModeMenu:
			return updateModeInput(m, key)
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
			m.current = settingsMenu
			m.cursor = 0
		case 2:
			if err := buildCaptureApp(); err != nil {
				m.status = "build failed: " + err.Error()
			} else {
				m.status = "capture-app build finished"
			}
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
			m.status = runCaptureWithRecovery(modeRegion, &m.settings, m.configPath)
		case 1:
			m.status = runCaptureWithRecovery(modeFullscreen, &m.settings, m.configPath)
		case 2:
			m.status = runCaptureWithRecovery(modeWindow, &m.settings, m.configPath)
		case 3:
			m.current = mainMenu
			m.cursor = 0
		}
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

func (m model) View() string {
	var b strings.Builder
	b.WriteString(titleStyle.Render("screeny-tui"))
	b.WriteString("\n")
	b.WriteString(hintStyle.Render("j/k or arrows to move, Enter to select, b/Esc back, q quit"))
	b.WriteString("\n\n")

	switch m.current {
	case mainMenu:
		b.WriteString(headerStyle.Render("Main Menu"))
		b.WriteString("\n")
		b.WriteString(renderMenu(m.mainItems, m.cursor))
	case captureMenu:
		b.WriteString(headerStyle.Render("Capture"))
		b.WriteString("\n")
		b.WriteString(renderMenu(m.captureItems, m.cursor))
	case settingsMenu:
		b.WriteString(headerStyle.Render("Settings"))
		b.WriteString("\n")
		b.WriteString(renderMenu(settingsItems(m.settings), m.cursor))
	case inputDelayMenu:
		b.WriteString(headerStyle.Render("Set Delay (ms)"))
		b.WriteString("\n")
		b.WriteString(normalStyle.Render("Type number and press Enter. b/Esc to cancel."))
		b.WriteString("\n\n")
		b.WriteString("> " + m.input)
	case inputModeMenu:
		b.WriteString(headerStyle.Render("Set Default Mode"))
		b.WriteString("\n")
		b.WriteString(normalStyle.Render("Type: region | fullscreen | window, then Enter."))
		b.WriteString("\n\n")
		b.WriteString("> " + m.input)
	}

	if m.status != "" {
		b.WriteString("\n\n")
		if strings.HasPrefix(strings.ToLower(m.status), "error") || strings.Contains(strings.ToLower(m.status), "failed") {
			b.WriteString(errStyle.Render(m.status))
		} else {
			b.WriteString(okStyle.Render(m.status))
		}
	}

	return b.String()
}

func renderMenu(items []string, cursor int) string {
	var b strings.Builder
	for i, item := range items {
		prefix := "  "
		if i == cursor {
			prefix = cursorStyle.Render("> ")
		}
		line := fmt.Sprintf("%s%s", prefix, item)
		if i == cursor {
			b.WriteString(cursorStyle.Render(line))
		} else {
			b.WriteString(normalStyle.Render(line))
		}
		b.WriteString("\n")
	}
	return b.String()
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

func onOff(v bool) string {
	if v {
		return "ON"
	}
	return "OFF"
}

func runCapture(mode captureMode) string {
	bin, err := resolveAppBin()
	if err != nil {
		return "error: " + err.Error()
	}

	cmd := exec.Command(bin, string(mode))
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Stdin = os.Stdin
	if err := cmd.Run(); err != nil {
		return "capture failed: " + err.Error()
	}
	return fmt.Sprintf("capture done (%s)", mode)
}

type dependencyFailure struct {
	tools    map[string]bool
	installs []string
}

func runCaptureWithRecovery(mode captureMode, s *settings, configPath string) string {
	currentMode := mode

	for attempts := 0; attempts < 3; attempts++ {
		output, err := runCaptureCommand(currentMode)
		if err == nil {
			return fmt.Sprintf("capture done (%s)", currentMode)
		}

		failure := parseDependencyFailure(output)
		if failure == nil {
			return "capture failed: " + strings.TrimSpace(output)
		}

		if failure.tools["grim"] {
			if len(failure.installs) > 0 {
				return "missing grim: install required first (" + failure.installs[0] + ")"
			}
			return "missing grim: install required first"
		}

		recovered := false
		actions := []string{}

		if failure.tools["wl-copy"] && s.CopyToClipboard {
			s.CopyToClipboard = false
			if saveErr := saveSettings(configPath, *s); saveErr == nil {
				actions = append(actions, "disabled clipboard")
				recovered = true
			}
		}

		if currentMode == modeWindow && failure.tools["hyprctl"] {
			currentMode = modeFullscreen
			actions = append(actions, "fallback mode=fullscreen")
			recovered = true
		} else if currentMode == modeRegion && failure.tools["slurp"] {
			currentMode = modeFullscreen
			actions = append(actions, "fallback mode=fullscreen")
			recovered = true
		}

		if recovered {
			continue
		}

		if len(failure.installs) > 0 {
			return "missing dependencies: run install and retry (" + strings.Join(failure.installs, " | ") + ")"
		}

		return "missing dependencies: run `capture-app --doctor`"
	}

	return "capture aborted after recovery attempts"
}

func runCaptureCommand(mode captureMode) (string, error) {
	bin, err := resolveAppBin()
	if err != nil {
		return "", err
	}

	cmd := exec.Command(bin, string(mode))
	out, err := cmd.CombinedOutput()
	return string(out), err
}

func parseDependencyFailure(output string) *dependencyFailure {
	if !strings.Contains(output, "Code: SCREENY-E001") {
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

	if p, err := exec.LookPath("capture-app"); err == nil {
		return p, nil
	}

	candidates := []string{"./target/release/capture-app", "./target/debug/capture-app"}
	for _, c := range candidates {
		if isExecutable(c) {
			return c, nil
		}
	}

	if err := buildCaptureApp(); err == nil {
		if isExecutable("./target/release/capture-app") {
			return "./target/release/capture-app", nil
		}
	}

	return "", errors.New("capture-app not found; build it with `cargo build --release` or set APP_BIN")
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
		return filepath.Join(xdg, "screeny", "config.json")
	}
	home := os.Getenv("HOME")
	if home == "" {
		home = "."
	}
	return filepath.Join(home, ".config", "screeny", "config.json")
}

func loadOrCreateSettings(path string) (settings, error) {
	if _, err := os.Stat(path); errors.Is(err, os.ErrNotExist) {
		d := defaultSettings()
		if err := saveSettings(path, d); err != nil {
			return settings{}, err
		}
		return d, nil
	}

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
		FilenameTemplate:   "screeny-{timestamp}-{mode}.png",
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
