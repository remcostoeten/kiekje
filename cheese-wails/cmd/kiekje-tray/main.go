package main

import (
	"flag"
	"os"
	"os/exec"
	"path/filepath"
	"sync"
	"syscall"
	"time"

	"kiekje/internal/binds"

	"github.com/getlantern/systray"
)

var (
	trayCmdMu   sync.Mutex
	lastTrayCmd time.Time
)

type trayMenuEntry struct {
	title      string
	tooltip    string
	args       []string
	bindAction string
}

func main() {
	appPath := flag.String("app", "kiekje", "path to the Kiekje binary")
	iconPath := flag.String("icon", "", "path to the tray icon")
	flag.Parse()

	lockFile, err := os.OpenFile(filepath.Join(os.TempDir(), "kiekje-tray.lock"), os.O_CREATE|os.O_RDWR, 0o600)
	if err != nil {
		os.Exit(1)
	}
	if err := syscall.Flock(int(lockFile.Fd()), syscall.LOCK_EX|syscall.LOCK_NB); err != nil {
		os.Exit(0)
	}

	systray.Run(func() {
		if *iconPath != "" {
			if icon, err := os.ReadFile(*iconPath); err == nil {
				systray.SetIcon(icon)
			}
		}
		systray.SetTitle("Kiekje")
		systray.SetTooltip("Kiekje capture")

		shortcuts := binds.TrayDisplays()

		entries := []trayMenuEntry{
			{title: "Open Kiekje", tooltip: "Show the capture window", args: []string{"--show"}},
			{title: "Capture region", tooltip: "Start region capture", args: []string{"--capture"}, bindAction: "capture"},
			{title: "Capture window", tooltip: "Click a window to capture it", args: []string{"--capture-window"}, bindAction: "captureWindow"},
			{title: "Set save folder", tooltip: "Choose where screenshots are saved", args: []string{"--choose-save-dir"}},
			{title: "Open save folder", tooltip: "Open the screenshot folder", args: []string{"--open-save-dir"}},
			{title: "Open last image", tooltip: "Open the most recently saved image", args: []string{"--open-last-image"}},
			{title: "Hide", tooltip: "Hide the capture window", args: []string{"--hide"}},
			{title: "Settings", tooltip: "Open the settings menu", args: []string{"--settings"}},
		}

		for _, entry := range entries {
			label := entry.title
			if entry.bindAction != "" {
				label = binds.TrayLabel(label, shortcuts[entry.bindAction])
			}
			item := systray.AddMenuItem(label, entry.tooltip)
			go runOnClick(item, *appPath, entry.args...)
		}

		systray.AddSeparator()
		quit := systray.AddMenuItem("Quit", "Quit Kiekje")

		go func() {
			for range quit.ClickedCh {
				_ = quitApp(*appPath)
				systray.Quit()
			}
		}()
	}, func() {})
}

func runOnClick(item *systray.MenuItem, appPath string, args ...string) {
	for range item.ClickedCh {
		trayCmdMu.Lock()
		now := time.Now()
		if now.Sub(lastTrayCmd) < 400*time.Millisecond {
			trayCmdMu.Unlock()
			continue
		}
		lastTrayCmd = now
		trayCmdMu.Unlock()
		_ = startCommand(appPath, args...)
	}
}

func startCommand(appPath string, args ...string) error {
	appPath = resolveAppPath(appPath)
	return exec.Command(appPath, args...).Start()
}

func resolveAppPath(appPath string) string {
	if !filepath.IsAbs(appPath) {
		if resolved, err := exec.LookPath(appPath); err == nil {
			appPath = resolved
		}
	}
	return appPath
}

func quitApp(appPath string) error {
	appPath = resolveAppPath(appPath)
	_ = exec.Command(appPath, "--quit").Run()
	return exec.Command("pkill", "-fx", appPath).Run()
}
