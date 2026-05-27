package main

import (
	"embed"
	"os"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	linuxoptions "github.com/wailsapp/wails/v2/pkg/options/linux"
)

//go:embed all:frontend/dist
var assets embed.FS

func init() {
	// WebKitGTK DMABUF + translucent surfaces crash on several Linux/Wayland setups (Wails #2977).
	if os.Getenv("WEBKIT_DISABLE_DMABUF_RENDERER") == "" {
		_ = os.Setenv("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
	}
}

func main() {
	app := NewApp()

	err := wails.Run(&options.App{
		Title:       "Cheese",
		Width:       1366,
		Height:      900,
		Frameless:   true,
		AlwaysOnTop: true,
		Linux: &linuxoptions.Options{
			WindowIsTranslucent: true,
			// Required when Linux options are set; zero value enables GPU accel and can crash WebKitWebProcess.
			WebviewGpuPolicy: linuxoptions.WebviewGpuPolicyNever,
		},
		SingleInstanceLock: &options.SingleInstanceLock{
			UniqueId: "cheese-wails",
			OnSecondInstanceLaunch: func(secondInstanceData options.SecondInstanceData) {
				app.HandleSecondInstance(secondInstanceData.Args)
			},
		},
		StartHidden:   true,
		DisableResize: false,
		MinWidth:      1180,
		MinHeight:     760,
		AssetServer: &assetserver.Options{
			Assets: assets,
		},
		BackgroundColour: &options.RGBA{R: 0, G: 0, B: 0, A: 0},
		OnStartup:        app.startup,
		Bind: []interface{}{
			app,
		},
	})

	if err != nil {
		println("Error:", err.Error())
	}
}
