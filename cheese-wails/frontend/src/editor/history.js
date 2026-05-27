const MAX_HISTORY = 40;

function cloneAnnotations(annotations) {
  return structuredClone(annotations);
}

export function createEditorHistory({ state, getImageData, putImageData, redraw }) {
  function createSnapshot() {
    return {
      annotations: cloneAnnotations(state.annotations),
      image: getImageData(),
    };
  }

  function applySnapshot(snap) {
    state.annotations = cloneAnnotations(snap.annotations);
    putImageData(snap.image);
    state.editor.selectedIndices = [];
    state.editor.hoveredIndex = -1;
    state.current = null;
    state.dragStart = null;
    redraw();
    updateButtons();
  }

  function resetHistory() {
    state.editor.history.past = [];
    state.editor.history.future = [];
    updateButtons();
  }

  function recordBeforeChange() {
    state.editor.history.past.push(createSnapshot());
    if (state.editor.history.past.length > MAX_HISTORY) {
      state.editor.history.past.shift();
    }
    state.editor.history.future = [];
    updateButtons();
  }

  function undo() {
    const { past, future } = state.editor.history;
    if (past.length === 0) return;
    future.push(createSnapshot());
    applySnapshot(past.pop());
  }

  function redo() {
    const { past, future } = state.editor.history;
    if (future.length === 0) return;
    past.push(createSnapshot());
    applySnapshot(future.pop());
  }

  function updateButtons() {
    const undoBtn = document.getElementById('undo');
    const redoBtn = document.getElementById('redo');
    if (undoBtn) undoBtn.disabled = state.editor.history.past.length === 0;
    if (redoBtn) redoBtn.disabled = state.editor.history.future.length === 0;
  }

  return {
    resetHistory,
    recordBeforeChange,
    undo,
    redo,
    updateButtons,
  };
}
