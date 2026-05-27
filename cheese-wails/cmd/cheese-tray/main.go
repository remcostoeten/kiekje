package main

import (
	"flag"
	"os"
	"os/exec"
	"path/filepath"
	"sync"
	"syscall"
	"time"

	"github.com/getlantern/systray"
)

var (
	trayCmdMu   sync.Mutex
	lastTrayCmd time.Time
)

func main() {
	appPath := flag.String("app", "cheese-wails", "path to the Cheese Wails binary")
	iconPath := flag.String("icon", "", "path to the tray icon")
	flag.Parse()

	lockFile, err := os.OpenFile(filepath.Join(os.TempDir(), "cheese-tray.lock"), os.O_CREATE|os.O_RDWR, 0o600)
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
		systray.SetTitle("Cheese")
		systray.SetTooltip("Cheese capture")

		open := systray.AddMenuItem("Open Cheese", "Show the capture window")
		capture := systray.AddMenuItem("Capture region", "Start region capture")
		chooseSaveDir := systray.AddMenuItem("Set save folder", "Choose where screenshots are saved")
		openSaveDir := systray.AddMenuItem("Open save folder", "Open the screenshot folder")
		openLastImage := systray.AddMenuItem("Open last image", "Open the most recently saved image")
		hide := systray.AddMenuItem("Hide", "Hide the capture window")
		systray.AddSeparator()
		quit := systray.AddMenuItem("Quit", "Quit Cheese")

		go runOnClick(open, *appPath, "--show")
		go runOnClick(capture, *appPath, "--capture")
		go runOnClick(chooseSaveDir, *appPath, "--choose-save-dir")
		go runOnClick(openSaveDir, *appPath, "--open-save-dir")
		go runOnClick(openLastImage, *appPath, "--open-last-image")
		go runOnClick(hide, *appPath, "--hide")
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
