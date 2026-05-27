export const EDITOR_TEMPLATE = `
  <div class="editor" id="editor">
    <div class="stage">
      <canvas id="canvas" class="idle"></canvas>
    </div>

    <div class="capture-overlay hidden" id="capture-overlay" aria-hidden="true">
      <canvas id="capture-canvas"></canvas>
      <div class="capture-help">
        <span>Drag a region</span>
        <kbd>Esc</kbd>
        <span>cancel</span>
      </div>
    </div>

    <div class="bar" id="bar">
      <button class="btn primary" id="capture" type="button" data-tip="Capture" data-kbd="C">
        <i class="ti ti-camera" aria-hidden="true"></i>
        <span>Capture</span>
      </button>
      <div class="sep"></div>
      <button class="btn on" data-tool="select" type="button" data-tip="Select" data-kbd="V">
        <i class="ti ti-pointer" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="rect" type="button" data-tip="Rectangle" data-kbd="R">
        <i class="ti ti-square" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="arrow" type="button" data-tip="Arrow" data-kbd="A">
        <i class="ti ti-arrow-up-right" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="pen" type="button" data-tip="Draw" data-kbd="P">
        <i class="ti ti-pencil" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="highlight" type="button" data-tip="Highlight" data-kbd="H">
        <i class="ti ti-highlight" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="ellipse" type="button" data-tip="Ellipse" data-kbd="O">
        <i class="ti ti-circle" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="step" type="button" data-tip="Numbered step" data-kbd="N">
        <i class="ti ti-list-numbers" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="text" type="button" data-tip="Text" data-kbd="T">
        <i class="ti ti-letter-t" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="blur" type="button" data-tip="Blur redact" data-kbd="B">
        <i class="ti ti-blur" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="crop" type="button" data-tip="Crop image" data-kbd="G">
        <i class="ti ti-crop" aria-hidden="true"></i>
      </button>
      <div class="color-wrap" id="color-wrap">
        <button class="btn" id="color-trigger" type="button" data-tip="Stroke color">
          <span class="color-trigger-swatch" id="color-swatch"></span>
        </button>
        <div class="color-popper hidden" id="color-popper"></div>
      </div>
      <label class="stroke-wrap" id="stroke-wrap" data-tip="Stroke width">
        <i class="ti ti-line" aria-hidden="true"></i>
        <input type="range" id="stroke-width" min="1" max="12" value="3" aria-label="Stroke width" />
      </label>
      <div class="sep"></div>
      <button class="btn" id="undo" type="button" data-tip="Undo" data-kbd="Ctrl Z">
        <i class="ti ti-arrow-back-up" aria-hidden="true"></i>
      </button>
      <button class="btn" id="redo" type="button" data-tip="Redo" data-kbd="Ctrl Shift Z">
        <i class="ti ti-arrow-forward-up" aria-hidden="true"></i>
      </button>
      <button class="btn done" id="save" type="button" data-tip="Save" data-kbd="Ctrl S">
        <i class="ti ti-check" aria-hidden="true"></i>
      </button>
      <button class="btn" id="copy" type="button" data-tip="Copy to clipboard" data-kbd="Ctrl C">
        <i class="ti ti-copy" aria-hidden="true"></i>
      </button>
      <div class="sep"></div>
      <div class="color-picker" id="color-picker"></div>
      <div class="menu-wrap">
        <button class="btn" id="menu-toggle" type="button" data-tip="Settings" aria-expanded="false">
          <i class="ti ti-dots-vertical" aria-hidden="true"></i>
        </button>
        <div class="menu hidden" id="menu">
          <div class="menu-section">
            <div class="menu-label">Output</div>
            <div class="path-row">
              <div class="path-text" id="save-dir"></div>
              <button class="path-icon" id="choose-save-dir" type="button" title="Choose folder">
                <i class="ti ti-folder" aria-hidden="true"></i>
              </button>
              <button class="path-icon" id="open-save-dir" type="button" title="Open folder">
                <i class="ti ti-external-link" aria-hidden="true"></i>
              </button>
            </div>
          </div>
          <div class="menu-section">
            <div class="menu-item">
              <span>Copy after capture</span>
              <div class="tog" id="tog-copy" role="switch" tabindex="0"></div>
            </div>
            <div class="menu-item">
              <span>Clipboard only</span>
              <div class="tog" id="tog-clipboard-only" role="switch" tabindex="0"></div>
            </div>
            <div class="menu-item">
              <span>Close after capture</span>
              <div class="tog" id="tog-close" role="switch" tabindex="0"></div>
            </div>
            <div class="menu-item">
              <span>Close after save</span>
              <div class="tog" id="tog-close-save" role="switch" tabindex="0"></div>
            </div>
          </div>
          <div class="menu-section">
            <div class="menu-label">Shortcuts</div>
            <div class="menu-item">
              <span>Capture region</span>
              <span><span class="kbd" id="bind-capture">-</span>
              <button type="button" data-record="capture">Rec</button></span>
            </div>
            <div class="menu-item">
              <span>Capture window</span>
              <span><span class="kbd" id="bind-captureWindow">-</span>
              <button type="button" data-record="captureWindow">Rec</button></span>
            </div>
            <div class="menu-item">
              <span>Save</span>
              <span><span class="kbd" id="bind-save">-</span>
              <button type="button" data-record="save">Rec</button></span>
            </div>
            <div class="menu-item">
              <span>Undo</span>
              <span><span class="kbd" id="bind-undo">-</span>
              <button type="button" data-record="undo">Rec</button></span>
            </div>
          </div>
          <div class="menu-section">
            <button class="menu-action" id="reset-binds" type="button">Reset defaults</button>
          </div>
        </div>
      </div>
      <div class="tooltip" id="tooltip" aria-hidden="true"></div>
    </div>
  </div>
`;

export function mountTemplate(root) {
  root.innerHTML = EDITOR_TEMPLATE;
  return {
    editor: document.getElementById('editor'),
    canvas: document.getElementById('canvas'),
    captureOverlay: document.getElementById('capture-overlay'),
    captureCanvas: document.getElementById('capture-canvas'),
    captureBtn: document.getElementById('capture'),
    menu: document.getElementById('menu'),
    menuToggle: document.getElementById('menu-toggle'),
    saveDirEl: document.getElementById('save-dir'),
    tooltipEl: document.getElementById('tooltip'),
    colorPicker: document.getElementById('color-picker'),
    colorPopper: document.getElementById('color-popper'),
    colorSwatch: document.getElementById('color-swatch'),
    colorWrap: document.getElementById('color-wrap'),
    colorTrigger: document.getElementById('color-trigger'),
    strokeWidth: document.getElementById('stroke-width'),
    toggles: {
      copy: document.getElementById('tog-copy'),
      clipboardOnly: document.getElementById('tog-clipboard-only'),
      close: document.getElementById('tog-close'),
      closeSave: document.getElementById('tog-close-save'),
    },
    bindEls: {
      capture: document.getElementById('bind-capture'),
      captureWindow: document.getElementById('bind-captureWindow'),
      save: document.getElementById('bind-save'),
      undo: document.getElementById('bind-undo'),
    },
  };
}
