import sys
from pathlib import Path
from playwright.sync_api import sync_playwright

root = Path(__file__).resolve().parent
with sync_playwright() as p:
    browser = p.chromium.launch(args=["--allow-file-access-from-files"])
    page = browser.new_page(viewport={"width": 1280, "height": 640}, device_scale_factor=2)
    errors = []
    page.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
    page.goto((root / "social-preview.html").as_uri())
    page.evaluate("document.fonts.ready")
    page.screenshot(path=str(root / "social-preview.png"))
    browser.close()
if errors:
    sys.exit("\n".join(errors))
