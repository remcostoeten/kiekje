package main

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"

	"kiekje/internal/hyprland"

	"github.com/wailsapp/wails/v2/pkg/runtime"
)

type App struct {
	ctx                 context.Context
	ctxMu               sync.RWMutex
	captureMu           sync.Mutex
	capturing           bool
	settingsMu          sync.Mutex
	choosingSaveDir     bool
	lastChooseSaveDirAt time.Time
	slurpMu             sync.Mutex
	slurpCmd            *exec.Cmd
}

type CaptureResult struct {
	Path string `json:"path"`
	Data string `json:"data"`
}

type WindowGeometry struct {
	X int `json:"x"`
	Y int `json:"y"`
	W int `json:"w"`
	H int `json:"h"`
}

type ipcRect struct {
	X      int `json:"x"`
	Y      int `json:"y"`
	Width  int `json:"width"`
	Height int `json:"height"`
}

type hyprClient struct {
	PID            int    `json:"pid"`
	Mapped         bool   `json:"mapped"`
	Hidden         bool   `json:"hidden"`
	Visible        bool   `json:"visible"`
	AcceptsInput   bool   `json:"acceptsInput"`
	Title          string `json:"title"`
	Class          string `json:"class"`
	At             []int  `json:"at"`
	Size           []int  `json:"size"`
	FocusHistoryID int    `json:"focusHistoryID"`
}

type swayNode struct {
	PID            int               `json:"pid"`
	Type           string            `json:"type"`
	Name           string            `json:"name"`
	AppID          string            `json:"app_id"`
	Rect           ipcRect           `json:"rect"`
	WindowRect     ipcRect           `json:"window_rect"`
	Focused        bool              `json:"focused"`
	Nodes          []swayNode        `json:"nodes"`
	FloatingNodes  []swayNode        `json:"floating_nodes"`
	WindowPropsRaw map[string]string `json:"window_properties"`
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

type AppState struct {
	OutputPath           string            `json:"outputPath"`
	SaveDir              string            `json:"saveDir"`
	LastSavedPath        string            `json:"lastSavedPath"`
	CopyAfterCapture     bool              `json:"copyAfterCapture"`
	CloseAfterCapture    bool              `json:"closeAfterCapture"`
	CloseAfterSave       bool              `json:"closeAfterSave"`
	ClipboardOnlyCapture bool              `json:"clipboardOnlyCapture"`
	Binds                map[string]string `json:"binds"`
}

type ShortcutBinding struct {
	Action  string `json:"action"`
	Key     string `json:"key"`
	Command string `json:"command"`
}

func NewApp() *App {
	return &App{}
}

func (a *App) stateDir() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".config", "kiekje"), nil
}

func (a *App) statePath() (string, error) {
	dir, err := a.stateDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "state.json"), nil
}

func (a *App) loadState() (AppState, error) {
	path, err := a.statePath()
	if err != nil {
		return AppState{}, err
	}

	raw, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			state, err := a.defaultState()
			if err != nil {
				return AppState{}, err
			}
			_ = a.saveState(state)
			return state, nil
		}
		return AppState{}, err
	}

	var state AppState
	if err := json.Unmarshal(raw, &state); err != nil {
		return AppState{}, err
	}
	changed := false
	if state.Binds == nil {
		state.Binds = defaultBinds()
		changed = true
	}
	if _, ok := state.Binds["recapture"]; ok {
		delete(state.Binds, "recapture")
		changed = true
	}
	if _, ok := state.Binds["captureWindow"]; !ok {
		state.Binds["captureWindow"] = defaultBinds()["captureWindow"]
		changed = true
	}
	if state.SaveDir == "" {
		state.SaveDir = defaultSaveDir()
		changed = true
	}
	if state.OutputPath == "" {
		state.OutputPath = nextOutputPath(state.SaveDir)
		changed = true
	}
	if changed {
		_ = a.saveState(state)
	}
	return state, nil
}

func (a *App) defaultState() (AppState, error) {
	saveDir := defaultSaveDir()
	return AppState{
		OutputPath: nextOutputPath(saveDir),
		SaveDir:    saveDir,
		Binds:      defaultBinds(),
	}, nil
}

func (a *App) saveState(state AppState) error {
	dir, err := a.stateDir()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}

	path, err := a.statePath()
	if err != nil {
		return err
	}

	payload, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, payload, 0o644)
}

func defaultBinds() map[string]string {
	return map[string]string{
		"capture":       "SUPER SHIFT, C, exec, kiekje --capture",
		"captureWindow": "SUPER SHIFT, W, exec, kiekje --capture-window",
		"save":          "CTRL, S, save",
		"undo":          "CTRL, Z, undo",
	}
}

func defaultSaveDir() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return os.TempDir()
	}
	return filepath.Join(home, "Pictures", "Kiekje")
}

func nextOutputPath(saveDir string) string {
	if strings.TrimSpace(saveDir) == "" {
		saveDir = defaultSaveDir()
	}
	return filepath.Join(saveDir, fmt.Sprintf("kiekje-%s.png", time.Now().Format("20060102-150405")))
}

func (a *App) startup(ctx context.Context) {
	a.ctxMu.Lock()
	a.ctx = ctx
	a.ctxMu.Unlock()
	runtime.WindowSetSize(ctx, 1366, 900)
	runtime.WindowCenter(ctx)
	a.startTraySidecar()
	_ = a.InstallAutostart()
	_ = a.InstallGlobalShortcuts()
	if len(os.Args) > 1 {
		go func(args []string) {
			time.Sleep(250 * time.Millisecond)
			a.HandleSecondInstance(args)
		}(append([]string(nil), os.Args[1:]...))
	} else {
		runtime.WindowHide(ctx)
	}
}

func (a *App) context() context.Context {
	a.ctxMu.RLock()
	defer a.ctxMu.RUnlock()
	return a.ctx
}

func (a *App) ShowWindow() {
	ctx := a.context()
	if ctx == nil {
		return
	}
	runtime.WindowSetAlwaysOnTop(ctx, true)
	runtime.WindowShow(ctx)
	runtime.WindowCenter(ctx)
	go func() {
		// Hyprland needs the surface mapped before focus/raise dispatch.
		time.Sleep(50 * time.Millisecond)
		a.raiseKiekjeWindow()
	}()
}

func (a *App) raiseKiekjeWindow() {
	if _, err := exec.LookPath("hyprctl"); err != nil {
		return
	}
	_ = exec.Command("hyprctl", "dispatch", "focuswindow", "title:Kiekje").Run()
	_ = exec.Command("hyprctl", "dispatch", "alterzorder", "top").Run()
}

func (a *App) OpenSettings() {
	a.ShowWindow()
	if ctx := a.context(); ctx != nil {
		runtime.EventsEmit(ctx, "kiekje:open-settings")
	}
}

func (a *App) HideWindow() {
	if ctx := a.context(); ctx != nil {
		runtime.WindowHide(ctx)
	}
}

func (a *App) QuitApp() {
	if ctx := a.context(); ctx != nil {
		runtime.Quit(ctx)
	}
}

func (a *App) RequestCapture() {
	if a.shouldDeferCapture() {
		return
	}
	go a.triggerCapture(false)
}

func (a *App) RequestCaptureWindow() {
	if a.shouldDeferCapture() {
		return
	}
	go a.triggerCapture(true)
}

// triggerCapture runs slurp+grim from Go so Hyprland/tray shortcuts work when the
// webview is hidden on another workspace (EventsEmit may not run there).
func (a *App) triggerCapture(windowPickOnly bool) {
	res, err := a.captureWithSlurp(windowPickOnly)
	a.deliverCaptureResult(res, err)
}

func (a *App) deliverCaptureResult(res CaptureResult, err error) {
	if err != nil {
		return
	}
	state, loadErr := a.loadState()
	if loadErr != nil {
		return
	}
	ctx := a.context()
	if ctx == nil {
		return
	}

	if state.ClipboardOnlyCapture {
		_ = a.CopyImageToClipboard(res.Path)
		a.ShowCaptureSuccessToast()
		a.HideWindow()
		return
	}

	if state.CopyAfterCapture {
		_ = a.CopyImageToClipboard(res.Path)
	}
	if state.CloseAfterCapture {
		_ = a.SaveImage(res.Data, state.OutputPath)
		a.HideWindow()
		return
	}

	runtime.EventsEmit(ctx, "kiekje:capture-result", res)
}

func (a *App) FinishCapture() {
	a.captureMu.Lock()
	a.capturing = false
	a.captureMu.Unlock()
}

func (a *App) isChoosingSaveDir() bool {
	a.settingsMu.Lock()
	defer a.settingsMu.Unlock()
	return a.choosingSaveDir
}

func (a *App) shouldDeferCapture() bool {
	a.settingsMu.Lock()
	defer a.settingsMu.Unlock()
	if a.choosingSaveDir {
		return true
	}
	return !a.lastChooseSaveDirAt.IsZero() && time.Since(a.lastChooseSaveDirAt) < 3*time.Second
}

func (a *App) CancelCapture() {
	a.slurpMu.Lock()
	cmd := a.slurpCmd
	a.slurpCmd = nil
	a.slurpMu.Unlock()
	if cmd != nil && cmd.Process != nil {
		_ = cmd.Process.Kill()
	}
	a.FinishCapture()
	if ctx := a.context(); ctx != nil {
		runtime.EventsEmit(ctx, "kiekje:cancel-capture")
	}
}

func (a *App) beginChooseSaveDir() {
	a.settingsMu.Lock()
	a.choosingSaveDir = true
	a.lastChooseSaveDirAt = time.Now()
	a.settingsMu.Unlock()
	a.CancelCapture()
	a.HideWindow()
	if ctx := a.context(); ctx != nil {
		runtime.EventsEmit(ctx, "kiekje:choose-save-dir")
	}
}

func (a *App) endChooseSaveDir() {
	a.settingsMu.Lock()
	a.choosingSaveDir = false
	a.settingsMu.Unlock()
	a.HideWindow()
}

func (a *App) HandleSecondInstance(args []string) {
	for _, arg := range args {
		if strings.TrimSpace(arg) == "--choose-save-dir" {
			a.beginChooseSaveDir()
			go func() {
				defer a.endChooseSaveDir()
				_, _ = a.ChooseSaveDir()
			}()
			return
		}
	}
	for _, arg := range args {
		switch strings.TrimSpace(arg) {
		case "--capture":
			if a.shouldDeferCapture() {
				return
			}
			go a.triggerCapture(false)
			return
		case "--capture-window":
			if a.shouldDeferCapture() {
				return
			}
			go a.triggerCapture(true)
			return
		case "--show":
			a.ShowWindow()
			return
		case "--settings":
			a.OpenSettings()
			return
		case "--hide":
			a.HideWindow()
			return
		case "--open-save-dir":
			_ = a.OpenSaveDir()
			return
		case "--open-last-image":
			_ = a.OpenLastImage()
			return
		case "--quit":
			a.QuitApp()
			return
		case "--sync-hyprland":
			_ = a.SyncHyprlandConfig()
			return
		}
	}
	a.ShowWindow()
}

func (a *App) startTraySidecar() {
	exe, err := os.Executable()
	if err != nil {
		return
	}
	dir := filepath.Dir(exe)
	trayPath := filepath.Join(dir, "kiekje-tray")
	if _, err := os.Stat(trayPath); err != nil {
		return
	}

	iconPath := filepath.Join(filepath.Dir(dir), "appicon.png")
	if _, err := os.Stat(iconPath); err != nil {
		if home, homeErr := os.UserHomeDir(); homeErr == nil {
			iconPath = filepath.Join(home, ".local", "share", "icons", "hicolor", "256x256", "apps", "kiekje.png")
		}
	}
	args := []string{"--app", exe}
	if _, err := os.Stat(iconPath); err == nil {
		args = append(args, "--icon", iconPath)
	}

	cmd := exec.Command(trayPath, args...)
	cmd.Stdout = nil
	cmd.Stderr = nil
	_ = cmd.Start()
}

func (a *App) InstallAutostart() error {
	home, err := os.UserHomeDir()
	if err != nil {
		return err
	}
	exe, err := os.Executable()
	if err != nil {
		return err
	}

	autostartDir := filepath.Join(home, ".config", "autostart")
	if err := os.MkdirAll(autostartDir, 0o755); err != nil {
		return err
	}

	desktop := fmt.Sprintf(`[Desktop Entry]
Type=Application
Name=Kiekje
Comment=Kiekje capture tray
Exec=%q
Terminal=false
X-GNOME-Autostart-enabled=true
`, exe)

	return os.WriteFile(filepath.Join(autostartDir, "kiekje.desktop"), []byte(desktop), 0o644)
}

func (a *App) InstallGlobalShortcuts() error {
	state, err := a.loadState()
	if err != nil {
		return err
	}
	return a.writeGlobalShortcuts(state)
}

func (a *App) SyncHyprlandConfig() error {
	state, err := a.loadState()
	if err != nil {
		return err
	}
	return a.writeGlobalShortcuts(state)
}

func (a *App) writeGlobalShortcuts(state AppState) error {
	exe, err := os.Executable()
	if err != nil {
		return err
	}
	return hyprland.Sync(globalBindLines(state.Binds, exe))
}

func globalBindLines(binds map[string]string, exe string) string {
	defaults := defaultBinds()
	lines := []string{
		managedGlobalBindLine(binds["capture"], defaults["capture"], exe, "--capture"),
		managedGlobalBindLine(binds["captureWindow"], defaults["captureWindow"], exe, "--capture-window"),
	}
	out := make([]string, 0, len(lines))
	for _, line := range lines {
		if line != "" {
			out = append(out, line)
		}
	}
	return strings.Join(out, "\n")
}

func managedGlobalBindLine(bindLine, fallback, exe, flag string) string {
	modifiers, key := bindCombo(bindLine)
	if modifiers == "" || key == "" {
		modifiers, key = bindCombo(fallback)
	}
	if modifiers == "" || key == "" {
		return ""
	}
	return fmt.Sprintf("bind = %s, %s, exec, %q %s", modifiers, key, exe, flag)
}

func bindCombo(bindLine string) (string, string) {
	raw := strings.TrimSpace(bindLine)
	raw = regexp.MustCompile(`(?i)^bind\w*\s*=\s*`).ReplaceAllString(raw, "")
	parts := splitBindLine(raw)
	if len(parts) < 2 {
		return "", ""
	}
	modifiers := strings.TrimSpace(parts[0])
	key := strings.TrimSpace(parts[1])
	if modifiers == "" || key == "" || isModifierKey(key) {
		return "", ""
	}
	return modifiers, key
}

func isModifierKey(key string) bool {
	switch strings.ToUpper(strings.TrimSpace(key)) {
	case "SUPER", "CTRL", "CONTROL", "ALT", "SHIFT", "META":
		return true
	default:
		return false
	}
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

func parseSlurpGeometry(raw string) (int, int, int, int, error) {
	geom := strings.TrimSpace(raw)
	if geom == "" {
		return 0, 0, 0, 0, fmt.Errorf("no region selected")
	}

	space := strings.IndexByte(geom, ' ')
	if space < 0 {
		return 0, 0, 0, 0, fmt.Errorf("invalid slurp geometry: %q", raw)
	}

	origin := geom[:space]
	sizePart := strings.Fields(geom[space+1:])
	if len(sizePart) == 0 {
		return 0, 0, 0, 0, fmt.Errorf("invalid slurp geometry: %q", raw)
	}
	size := sizePart[0]
	comma := strings.IndexByte(origin, ',')
	xSep := strings.IndexByte(size, 'x')
	if comma < 0 || xSep < 0 {
		return 0, 0, 0, 0, fmt.Errorf("invalid slurp geometry: %q", raw)
	}

	x, err := strconv.Atoi(origin[:comma])
	if err != nil {
		return 0, 0, 0, 0, fmt.Errorf("invalid slurp geometry: %q", raw)
	}
	y, err := strconv.Atoi(origin[comma+1:])
	if err != nil {
		return 0, 0, 0, 0, fmt.Errorf("invalid slurp geometry: %q", raw)
	}
	w, err := strconv.Atoi(size[:xSep])
	if err != nil {
		return 0, 0, 0, 0, fmt.Errorf("invalid slurp geometry: %q", raw)
	}
	h, err := strconv.Atoi(size[xSep+1:])
	if err != nil {
		return 0, 0, 0, 0, fmt.Errorf("invalid slurp geometry: %q", raw)
	}
	if w <= 0 || h <= 0 {
		return 0, 0, 0, 0, fmt.Errorf("no region selected")
	}

	return x, y, w, h, nil
}

func slurpBoxLabel(parts ...string) string {
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		part = strings.ReplaceAll(part, "\n", " ")
		part = strings.ReplaceAll(part, "\r", " ")
		if len(part) > 48 {
			part = part[:48]
		}
		return part
	}
	return "window"
}

func formatSlurpBox(x, y, w, h int, label string) string {
	return fmt.Sprintf("%d,%d %dx%d %s", x, y, w, h, slurpBoxLabel(label))
}

func isKiekjeWindowLabel(label string) bool {
	label = strings.ToLower(strings.TrimSpace(label))
	return label == "kiekje" || strings.Contains(label, "kiekje") || label == "cheese" || strings.Contains(label, "cheese")
}

func (a *App) slurpWindowBoxes() string {
	if boxes, err := a.hyprlandSlurpBoxes(); err == nil && len(boxes) > 0 {
		return strings.Join(boxes, "\n")
	}
	if boxes, err := a.swaySlurpBoxes(); err == nil && len(boxes) > 0 {
		return strings.Join(boxes, "\n")
	}
	return ""
}

func (a *App) hyprlandSlurpBoxes() ([]string, error) {
	if _, err := exec.LookPath("hyprctl"); err != nil {
		return nil, err
	}

	rawClients, err := exec.Command("hyprctl", "-j", "clients").Output()
	if err != nil {
		return nil, err
	}

	var clients []hyprClient
	if err := json.Unmarshal(rawClients, &clients); err != nil {
		return nil, err
	}

	pid := os.Getpid()
	lines := make([]string, 0, len(clients))
	for _, client := range clients {
		if client.PID == pid || !client.Mapped || client.Hidden || !client.Visible {
			continue
		}
		if len(client.At) < 2 || len(client.Size) < 2 {
			continue
		}
		w, h := client.Size[0], client.Size[1]
		if w <= 0 || h <= 0 {
			continue
		}
		label := slurpBoxLabel(client.Title, client.Class)
		if isKiekjeWindowLabel(label) || isKiekjeWindowLabel(client.Title) || isKiekjeWindowLabel(client.Class) {
			continue
		}
		lines = append(lines, formatSlurpBox(client.At[0], client.At[1], w, h, label))
	}

	if len(lines) == 0 {
		return nil, fmt.Errorf("no hyprland windows available")
	}
	return lines, nil
}

func appendSwaySlurpBoxes(node swayNode, pid int, out *[]string) {
	for i := range node.FloatingNodes {
		appendSwaySlurpBoxes(node.FloatingNodes[i], pid, out)
	}
	for i := range node.Nodes {
		appendSwaySlurpBoxes(node.Nodes[i], pid, out)
	}

	if node.PID == 0 || node.PID == pid {
		return
	}
	if isKiekjeWindowLabel(node.windowLabel()) {
		return
	}
	if !node.isWindowLike() {
		return
	}

	rect := node.WindowRect
	if rect.Width <= 0 || rect.Height <= 0 {
		rect = node.Rect
	}
	if rect.Width <= 0 || rect.Height <= 0 {
		return
	}

	*out = append(*out, formatSlurpBox(rect.X, rect.Y, rect.Width, rect.Height, node.windowLabel()))
}

func (a *App) swaySlurpBoxes() ([]string, error) {
	if _, err := exec.LookPath("swaymsg"); err != nil {
		return nil, err
	}

	rawTree, err := exec.Command("swaymsg", "-t", "get_tree").Output()
	if err != nil {
		return nil, err
	}

	var root swayNode
	if err := json.Unmarshal(rawTree, &root); err != nil {
		return nil, err
	}

	lines := make([]string, 0, 16)
	appendSwaySlurpBoxes(root, os.Getpid(), &lines)
	if len(lines) == 0 {
		return nil, fmt.Errorf("no sway windows available")
	}
	return lines, nil
}

func (a *App) prepareForSlurp() {
	ctx := a.context()
	if ctx != nil {
		runtime.WindowSetAlwaysOnTop(ctx, false)
		runtime.WindowUnfullscreen(ctx)
		runtime.WindowSetBackgroundColour(ctx, 0, 0, 0, 1)
	}
	a.HideWindow()
	// Let Hyprland drop the pinned float before slurp owns the layer-shell surface.
	time.Sleep(250 * time.Millisecond)
}

func (a *App) restoreAfterSlurp() {
	ctx := a.context()
	if ctx != nil {
		runtime.WindowSetAlwaysOnTop(ctx, true)
	}
}

func (a *App) runSlurp(windowPickOnly bool) (string, error) {
	args := []string{
		"slurp",
		"-d",
		"-F", "monospace",
		"-w", "2",
		"-b", "00000000",
		"-s", "ededed28",
		"-c", "ededede6",
	}

	boxes := a.slurpWindowBoxes()
	if boxes != "" {
		args = append(args, "-B", "ededed55")
		if windowPickOnly {
			args = append(args, "-r")
		}
	}

	cmd := exec.Command(args[0], args[1:]...)
	if boxes != "" {
		cmd.Stdin = strings.NewReader(boxes + "\n")
	}

	a.slurpMu.Lock()
	a.slurpCmd = cmd
	a.slurpMu.Unlock()
	defer func() {
		a.slurpMu.Lock()
		a.slurpCmd = nil
		a.slurpMu.Unlock()
	}()

	out, err := cmd.Output()
	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok && exitErr.ExitCode() == 1 {
			return "", fmt.Errorf("capture cancelled")
		}
		return "", err
	}

	geom := strings.TrimSpace(string(out))
	if geom == "" {
		return "", fmt.Errorf("no region selected")
	}
	return geom, nil
}

// CaptureRegion hides Kiekje and uses slurp + grim so selection covers the full desktop.
func (a *App) CaptureRegion() (CaptureResult, error) {
	return a.captureWithSlurp(false)
}

// CaptureWindow hides Kiekje and uses slurp window boxes only (click a window to capture it).
func (a *App) CaptureWindow() (CaptureResult, error) {
	return a.captureWithSlurp(true)
}

func (a *App) captureWithSlurp(windowPickOnly bool) (CaptureResult, error) {
	if a.shouldDeferCapture() {
		return CaptureResult{}, fmt.Errorf("capture cancelled for save folder selection")
	}

	a.captureMu.Lock()
	if a.capturing {
		a.captureMu.Unlock()
		return CaptureResult{}, fmt.Errorf("capture already in progress")
	}
	a.capturing = true
	a.captureMu.Unlock()
	defer a.FinishCapture()

	a.prepareForSlurp()
	defer a.restoreAfterSlurp()

	geom, err := a.runSlurp(windowPickOnly)
	if err != nil {
		return CaptureResult{}, err
	}

	x, y, w, h, err := parseSlurpGeometry(geom)
	if err != nil {
		return CaptureResult{}, err
	}

	return a.CaptureRegionAt(x, y, w, h)
}

func (a *App) CaptureRegionAt(x int, y int, width int, height int) (CaptureResult, error) {
	if a.shouldDeferCapture() {
		return CaptureResult{}, fmt.Errorf("capture cancelled for save folder selection")
	}
	if width <= 0 || height <= 0 {
		return CaptureResult{}, fmt.Errorf("no region selected")
	}

	tmpDir := os.TempDir()
	tmpFile, err := os.CreateTemp(tmpDir, "kiekje-*.png")
	if err != nil {
		return CaptureResult{}, err
	}
	path := tmpFile.Name()
	tmpFile.Close()

	geom := fmt.Sprintf("%d,%d %dx%d", x, y, width, height)
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

func (r ipcRect) contains(x, y int) bool {
	return x >= r.X && y >= r.Y && x < r.X+r.Width && y < r.Y+r.Height
}

func (n swayNode) windowLabel() string {
	if n.Name != "" {
		return n.Name
	}
	if n.WindowPropsRaw != nil {
		if title := n.WindowPropsRaw["title"]; title != "" {
			return title
		}
		if class := n.WindowPropsRaw["class"]; class != "" {
			return class
		}
	}
	if n.AppID != "" {
		return n.AppID
	}
	return ""
}

func (n swayNode) isWindowLike() bool {
	if n.Type == "root" || n.Type == "output" || n.Type == "workspace" || n.Type == "dockarea" {
		return false
	}
	return (n.WindowRect.Width > 0 && n.WindowRect.Height > 0) || len(n.Nodes) == 0 && len(n.FloatingNodes) == 0
}

func (a *App) GetWindowGeometryAtPoint(x int, y int) (WindowGeometry, error) {
	if geom, err := a.windowGeometryFromHyprlandAtPoint(x, y); err == nil {
		return geom, nil
	}
	if geom, err := a.windowGeometryFromSwayAtPoint(x, y); err == nil {
		return geom, nil
	}
	return WindowGeometry{}, fmt.Errorf("no window found under point")
}

func (a *App) GetWindowGeometryAtCursor() (WindowGeometry, error) {
	rawPos, err := exec.Command("hyprctl", "cursorpos").Output()
	if err == nil {
		var cursorX, cursorY int
		if _, scanErr := fmt.Sscanf(strings.TrimSpace(string(rawPos)), "%d, %d", &cursorX, &cursorY); scanErr == nil {
			return a.GetWindowGeometryAtPoint(cursorX, cursorY)
		}
	}
	return WindowGeometry{}, fmt.Errorf("unable to resolve cursor position")
}

func (a *App) windowGeometryFromHyprlandAtPoint(x, y int) (WindowGeometry, error) {
	if _, err := exec.LookPath("hyprctl"); err != nil {
		return WindowGeometry{}, err
	}

	rawClients, err := exec.Command("hyprctl", "-j", "clients").Output()
	if err != nil {
		return WindowGeometry{}, err
	}

	var clients []hyprClient
	if err := json.Unmarshal(rawClients, &clients); err != nil {
		return WindowGeometry{}, err
	}

	pid := os.Getpid()
	matches := make([]hyprClient, 0, 4)
	for _, client := range clients {
		if client.PID == pid || !client.Mapped || client.Hidden || !client.Visible || len(client.At) < 2 || len(client.Size) < 2 {
			continue
		}
		left, top := client.At[0], client.At[1]
		right := left + client.Size[0]
		bottom := top + client.Size[1]
		if x >= left && x < right && y >= top && y < bottom {
			matches = append(matches, client)
		}
	}

	if len(matches) == 0 {
		return WindowGeometry{}, fmt.Errorf("no hyprland window found at point")
	}

	sort.Slice(matches, func(i, j int) bool {
		return matches[i].FocusHistoryID < matches[j].FocusHistoryID
	})

	match := matches[0]
	return WindowGeometry{
		X: match.At[0],
		Y: match.At[1],
		W: match.Size[0],
		H: match.Size[1],
	}, nil
}

func (a *App) windowGeometryFromSwayAtPoint(x, y int) (WindowGeometry, error) {
	if _, err := exec.LookPath("swaymsg"); err != nil {
		return WindowGeometry{}, err
	}

	rawTree, err := exec.Command("swaymsg", "-t", "get_tree").Output()
	if err != nil {
		return WindowGeometry{}, err
	}

	var root swayNode
	if err := json.Unmarshal(rawTree, &root); err != nil {
		return WindowGeometry{}, err
	}

	pid := os.Getpid()
	if node := findWindowNode(root, x, y, pid); node != nil {
		return WindowGeometry{
			X: node.Rect.X,
			Y: node.Rect.Y,
			W: node.Rect.Width,
			H: node.Rect.Height,
		}, nil
	}

	return WindowGeometry{}, fmt.Errorf("no sway window found at point")
}

func findWindowNode(node swayNode, x, y int, pid int) *swayNode {
	if !node.Rect.contains(x, y) {
		return nil
	}

	for i := range node.FloatingNodes {
		if found := findWindowNode(node.FloatingNodes[i], x, y, pid); found != nil {
			return found
		}
	}
	for i := range node.Nodes {
		if found := findWindowNode(node.Nodes[i], x, y, pid); found != nil {
			return found
		}
	}

	if node.PID == pid {
		return nil
	}

	if isKiekjeWindowLabel(node.windowLabel()) {
		return nil
	}

	if node.isWindowLike() {
		return &node
	}
	return nil
}

func (a *App) SaveImage(base64Data string, outputPath string) error {
	state, err := a.loadState()
	if err != nil {
		return err
	}
	outputPath = strings.TrimSpace(outputPath)
	if outputPath == "" {
		outputPath = nextOutputPath(state.SaveDir)
	}
	if info, err := os.Stat(outputPath); err == nil && info.IsDir() {
		outputPath = nextOutputPath(outputPath)
	}

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

	if err := os.WriteFile(outputPath, bytes, 0o644); err != nil {
		return err
	}

	state.OutputPath = nextOutputPath(state.SaveDir)
	state.LastSavedPath = outputPath
	return a.saveState(state)
}

func (a *App) LoadAppState() (AppState, error) {
	return a.loadState()
}

func (a *App) UpdateBind(action string, bindLine string) (AppState, error) {
	state, err := a.loadState()
	if err != nil {
		return AppState{}, err
	}
	if state.Binds == nil {
		state.Binds = defaultBinds()
	}
	state.Binds[action] = strings.TrimSpace(bindLine)
	if err := a.saveState(state); err != nil {
		return AppState{}, err
	}
	if action == "capture" || action == "captureWindow" {
		if err := a.writeGlobalShortcuts(state); err != nil {
			return AppState{}, err
		}
	}
	return state, nil
}

func (a *App) UpdateSettings(saveDir string, copyAfterCapture bool, closeAfterCapture bool, closeAfterSave bool, clipboardOnlyCapture bool) (AppState, error) {
	state, err := a.loadState()
	if err != nil {
		return AppState{}, err
	}
	saveDir = strings.TrimSpace(saveDir)
	if saveDir != "" {
		state.SaveDir = saveDir
		state.OutputPath = nextOutputPath(saveDir)
	}
	state.CopyAfterCapture = copyAfterCapture
	state.CloseAfterCapture = closeAfterCapture
	state.CloseAfterSave = closeAfterSave
	state.ClipboardOnlyCapture = clipboardOnlyCapture
	if err := a.saveState(state); err != nil {
		return AppState{}, err
	}
	return state, nil
}

func (a *App) ChooseSaveDir() (AppState, error) {
	a.HideWindow()
	state, err := a.loadState()
	if err != nil {
		return AppState{}, err
	}
	ctx := a.context()
	if ctx == nil {
		return AppState{}, fmt.Errorf("app is not ready")
	}
	_ = os.MkdirAll(state.SaveDir, 0o755)
	dir, err := runtime.OpenDirectoryDialog(ctx, runtime.OpenDialogOptions{
		Title:            "Choose save folder",
		DefaultDirectory: state.SaveDir,
	})
	if err != nil {
		return AppState{}, err
	}
	if strings.TrimSpace(dir) == "" {
		a.HideWindow()
		return state, nil
	}
	state.SaveDir = dir
	state.OutputPath = nextOutputPath(dir)
	if err := a.saveState(state); err != nil {
		return AppState{}, err
	}
	a.HideWindow()
	return state, nil
}

func (a *App) OpenSaveDir() error {
	state, err := a.loadState()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(state.SaveDir, 0o755); err != nil {
		return err
	}
	return exec.Command("xdg-open", state.SaveDir).Start()
}

func (a *App) OpenLastImage() error {
	state, err := a.loadState()
	if err != nil {
		return err
	}
	if strings.TrimSpace(state.LastSavedPath) == "" {
		return a.OpenSaveDir()
	}
	return exec.Command("xdg-open", state.LastSavedPath).Start()
}

func (a *App) CopyImageToClipboard(path string) error {
	path = strings.TrimSpace(path)
	if path == "" {
		return fmt.Errorf("missing image path")
	}
	cmd := exec.Command("wl-copy", "--type", "image/png")
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()
	cmd.Stdin = file
	return cmd.Run()
}

func (a *App) ShowCaptureSuccessToast() {
	if notifySend, err := exec.LookPath("notify-send"); err == nil {
		go func() {
			_ = exec.Command(
				notifySend,
				"--app-name=Kiekje",
				"--urgency=low",
				"--expire-time=1400",
				"--hint=string:desktop-entry:kiekje",
				"Captured",
				"Copied to clipboard",
			).Run()
		}()
	}
}

func (a *App) ResetBinds() (AppState, error) {
	state, err := a.loadState()
	if err != nil {
		return AppState{}, err
	}
	state.Binds = defaultBinds()
	if err := a.saveState(state); err != nil {
		return AppState{}, err
	}
	if err := a.writeGlobalShortcuts(state); err != nil {
		return AppState{}, err
	}
	return state, nil
}
