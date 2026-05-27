import {
  LoadAppState,
  UpdateSettings,
  ChooseSaveDir,
  OpenSaveDir,
  ResetBinds,
  UpdateBind,
} from '../../wailsjs/go/main/App';
import { shortenPath } from '../utils/path.js';
import {
  formatBindDisplay,
  formatRecordingPreview,
  formatRecorderLine,
} from './binds.js';

function setToggle(el, on) {
  el.classList.toggle('on', on);
}

function getToggle(el) {
  return el.classList.contains('on');
}

export function createSettingsMenu({ dom, state }) {
  function positionMenu() {
    const gap = 8;
    const margin = 8;
    const wasHidden = dom.menu.classList.contains('hidden');

    if (wasHidden) {
      dom.menu.classList.remove('hidden');
      dom.menu.style.visibility = 'hidden';
    }

    dom.menu.classList.remove('menu-above', 'menu-below', 'menu-align-left', 'menu-align-right');
    dom.menu.classList.add('menu-above', 'menu-align-right');

    const toggleRect = dom.menuToggle.getBoundingClientRect();
    const menuRect = dom.menu.getBoundingClientRect();
    const menuHeight = menuRect.height;
    const menuWidth = menuRect.width;

    const spaceAbove = toggleRect.top - margin;
    const spaceBelow = window.innerHeight - toggleRect.bottom - margin;
    const fitsAbove = spaceAbove >= menuHeight + gap;
    const fitsBelow = spaceBelow >= menuHeight + gap;

    let vertical = 'menu-below';
    if (fitsAbove && (!fitsBelow || spaceAbove >= spaceBelow)) {
      vertical = 'menu-above';
    }

    const overflowRight = toggleRect.right - menuWidth < margin;
    const overflowLeft = toggleRect.left + menuWidth > window.innerWidth - margin;

    let horizontal = 'menu-align-right';
    if (overflowRight && !overflowLeft) {
      horizontal = 'menu-align-left';
    } else if (overflowLeft && !overflowRight) {
      horizontal = 'menu-align-right';
    } else if (overflowRight && overflowLeft) {
      horizontal =
        toggleRect.left + menuWidth / 2 <= window.innerWidth / 2
          ? 'menu-align-left'
          : 'menu-align-right';
    }

    dom.menu.classList.remove('menu-above', 'menu-below', 'menu-align-left', 'menu-align-right');
    dom.menu.classList.add(vertical, horizontal);

    if (wasHidden) {
      dom.menu.classList.add('hidden');
      dom.menu.style.visibility = '';
    }
  }

  function setMenuOpen(open) {
    if (open) positionMenu();
    dom.menu.classList.toggle('hidden', !open);
    dom.menuToggle.classList.toggle('on', open);
    dom.menuToggle.setAttribute('aria-expanded', String(open));
  }

  function renderBinds(binds = {}) {
    state.settings.binds = binds;
    for (const [action, el] of Object.entries(dom.bindEls)) {
      el.textContent = formatBindDisplay(binds[action]);
      el.classList.remove('recording');
    }
  }

  function renderSettings(nextState = {}) {
    state.settings = { ...state.settings, ...nextState };
    dom.saveDirEl.textContent = shortenPath(nextState.saveDir || '');
    dom.saveDirEl.title = nextState.saveDir || '';
    setToggle(dom.toggles.copy, Boolean(nextState.copyAfterCapture));
    setToggle(dom.toggles.clipboardOnly, Boolean(nextState.clipboardOnlyCapture));
    setToggle(dom.toggles.close, Boolean(nextState.closeAfterCapture));
    setToggle(dom.toggles.closeSave, Boolean(nextState.closeAfterSave));
  }

  async function persistSettings() {
    const updated = await UpdateSettings(
      state.settings.saveDir || '',
      getToggle(dom.toggles.copy),
      getToggle(dom.toggles.close),
      getToggle(dom.toggles.closeSave),
      getToggle(dom.toggles.clipboardOnly),
    );
    renderSettings(updated);
  }

  function wireToggle(el) {
    const flip = () => {
      setToggle(el, !getToggle(el));
      persistSettings();
    };
    el.addEventListener('click', flip);
    el.addEventListener('keydown', (e) => {
      if (e.key === ' ' || e.key === 'Enter') {
        e.preventDefault();
        flip();
      }
    });
  }

  function setRecording(action) {
    state.recording.action = action;
    state.recording.mods = new Set();
    state.recording.key = '';
    document.querySelectorAll('[data-record]').forEach((btn) => {
      btn.classList.toggle('recording', btn.dataset.record === action);
    });
    dom.bindEls[action].textContent = '…';
  }

  async function persistRecording() {
    const { action, mods, key } = state.recording;
    if (!action || !key) return;
    const updated = await UpdateBind(action, formatRecorderLine(action, mods, key));
    state.recording.action = null;
    document.querySelectorAll('[data-record]').forEach((b) => b.classList.remove('recording'));
    renderBinds(updated.binds || {});
  }

  function cancelRecording() {
    state.recording.action = null;
    document.querySelectorAll('[data-record]').forEach((b) => b.classList.remove('recording'));
    renderBinds(state.settings.binds || {});
  }

  function handleRecordingKeydown(evt) {
    const { recording } = state;
    if (!recording.action) return false;
    evt.preventDefault();
    if (evt.key === 'Escape') {
      cancelRecording();
      return true;
    }
    if (evt.key === 'Enter') {
      if (recording.key) persistRecording();
      return true;
    }
    if (evt.key === 'Backspace') {
      recording.key = '';
      dom.bindEls[recording.action].textContent = '…';
      return true;
    }
    if (['Meta', 'Control', 'Alt', 'Shift'].includes(evt.key)) {
      if (evt.key === 'Meta') recording.mods.add('SUPER');
      if (evt.key === 'Control') recording.mods.add('CTRL');
      if (evt.key === 'Alt') recording.mods.add('ALT');
      if (evt.key === 'Shift') recording.mods.add('SHIFT');
      dom.bindEls[recording.action].textContent = formatRecordingPreview(recording.mods, recording.key);
      return true;
    }
    recording.key = evt.key.length === 1 ? evt.key.toUpperCase() : evt.key.toUpperCase();
    dom.bindEls[recording.action].textContent = formatRecordingPreview(recording.mods, recording.key);
    return true;
  }

  async function loadState() {
    const loaded = await LoadAppState();
    state.settings = loaded;
    renderBinds(loaded.binds || {});
    renderSettings(loaded);
  }

  wireToggle(dom.toggles.copy);
  wireToggle(dom.toggles.clipboardOnly);
  wireToggle(dom.toggles.close);
  wireToggle(dom.toggles.closeSave);

  dom.menuToggle.onclick = (e) => {
    e.stopPropagation();
    setMenuOpen(dom.menu.classList.contains('hidden'));
  };

  document.addEventListener('click', () => setMenuOpen(false));
  dom.menu.addEventListener('click', (e) => e.stopPropagation());

  document.getElementById('choose-save-dir').onclick = async () => {
    setMenuOpen(false);
    renderSettings(await ChooseSaveDir());
  };
  document.getElementById('open-save-dir').onclick = () => OpenSaveDir();
  document.querySelectorAll('[data-record]').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      setRecording(btn.dataset.record);
    });
  });
  document.getElementById('reset-binds').onclick = async () => {
    renderBinds((await ResetBinds()).binds || {});
  };

  return {
    setMenuOpen,
    positionMenu,
    renderBinds,
    renderSettings,
    loadState,
    handleRecordingKeydown,
    cancelRecording,
  };
}
