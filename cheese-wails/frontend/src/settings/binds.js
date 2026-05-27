const MOD_DISPLAY = { SUPER: '⌘', SHIFT: '⇧', CTRL: '⌃', ALT: '⌥' };
const MOD_DISPLAY_ORDER = ['SHIFT', 'SUPER', 'CTRL', 'ALT'];
const MOD_RECORD_ORDER = ['SUPER', 'CTRL', 'ALT', 'SHIFT'];

const MOD_READABLE = { SUPER: 'Super', SHIFT: 'Shift', CTRL: 'Ctrl', ALT: 'Alt' };
const MOD_READABLE_ORDER = ['CTRL', 'ALT', 'SHIFT', 'SUPER'];

const BIND_COMMANDS = {
  capture: 'exec, kiekje --capture',
  captureWindow: 'exec, kiekje --capture-window',
  save: 'save',
  undo: 'undo',
};

export function parseBindLine(line) {
  if (!line) return null;
  const raw = line.replace(/^bind\d*\s*=\s*/i, '');
  const parts = raw.split(',').map((p) => p.trim()).filter(Boolean);
  if (parts.length < 2) return null;
  const modifiers = new Set(parts[0].split(/\s+/).map((m) => m.toUpperCase()).filter(Boolean));
  return { modifiers, key: parts[1].toUpperCase() };
}

export function formatBindDisplay(line) {
  const parsed = parseBindLine(line);
  if (!parsed) return '-';
  const mods = MOD_DISPLAY_ORDER
    .filter((m) => parsed.modifiers.has(m))
    .map((m) => MOD_DISPLAY[m] || m)
    .join('');
  const key = parsed.key.length === 1 ? parsed.key.toUpperCase() : parsed.key;
  return `${mods}${key}`;
}

/** Plain-text shortcut for tray menus and readable labels (e.g. Alt+Shift+R). */
export function formatBindReadable(line) {
  const parsed = parseBindLine(line);
  if (!parsed) return '';
  const parts = MOD_READABLE_ORDER
    .filter((m) => parsed.modifiers.has(m))
    .map((m) => MOD_READABLE[m] || m);
  const key = parsed.key.length === 1 ? parsed.key.toUpperCase() : parsed.key;
  parts.push(key);
  return parts.join('+');
}

export function matchBind(line, evt) {
  const parsed = parseBindLine(line);
  if (!parsed) return false;
  const mods = new Set();
  if (evt.metaKey) mods.add('SUPER');
  if (evt.ctrlKey) mods.add('CTRL');
  if (evt.altKey) mods.add('ALT');
  if (evt.shiftKey) mods.add('SHIFT');
  if (mods.size !== parsed.modifiers.size) return false;
  for (const mod of parsed.modifiers) {
    if (!mods.has(mod)) return false;
  }
  return evt.key.toUpperCase() === parsed.key;
}

export function formatRecordingPreview(recordingMods, recordingKey) {
  const mods = MOD_DISPLAY_ORDER
    .filter((m) => recordingMods.has(m))
    .map((m) => MOD_DISPLAY[m] || m)
    .join('');
  const key = recordingKey ? recordingKey.toUpperCase() : '';
  return mods + key || '…';
}

export function formatRecorderLine(recordingAction, recordingMods, recordingKey) {
  const mods = MOD_RECORD_ORDER.filter((m) => recordingMods.has(m)).join(' ');
  return `bind = ${mods}${mods ? ', ' : ''}${recordingKey}, ${BIND_COMMANDS[recordingAction] || ''}`;
}
