const MOD_DISPLAY = { SUPER: '⌘', SHIFT: '⇧', CTRL: '⌃', ALT: '⌥' };
const MOD_DISPLAY_ORDER = ['SHIFT', 'SUPER', 'CTRL', 'ALT'];
const MOD_RECORD_ORDER = ['SUPER', 'CTRL', 'ALT', 'SHIFT'];

const BIND_COMMANDS = {
  capture: 'exec, cheese-wails --capture',
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
