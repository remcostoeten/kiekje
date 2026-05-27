import { canvasPointToEditor } from '../utils/geometry.js';

export function createInlineTextController({ dom, state, redraw }) {
  function commitInlineText() {
    if (!state.editor.inlineTextInput) return;
    const input = state.editor.inlineTextInput;
    const text = input.value.trim();
    const p = { x: Number(input.dataset.canvasX), y: Number(input.dataset.canvasY) };
    input.remove();
    state.editor.inlineTextInput = null;
    if (text) {
      state.annotations.push({
        kind: 'text',
        x: p.x,
        y: p.y,
        w: 0,
        h: 0,
        color: state.editor.activeColor,
        text,
      });
      redraw();
    }
  }

  function cancelInlineText() {
    if (!state.editor.inlineTextInput) return;
    state.editor.inlineTextInput.remove();
    state.editor.inlineTextInput = null;
    redraw();
  }

  function openInlineText(p) {
    cancelInlineText();
    const editorPoint = canvasPointToEditor(dom.canvas, dom.editor, p);
    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'inline-text-input';
    input.dataset.canvasX = String(p.x);
    input.dataset.canvasY = String(p.y);
    input.style.left = `${editorPoint.x}px`;
    input.style.top = `${editorPoint.y}px`;
    input.style.color = state.editor.activeColor;
    dom.editor.appendChild(input);
    state.editor.inlineTextInput = input;
    input.addEventListener('pointerdown', (evt) => evt.stopPropagation());
    input.addEventListener('click', (evt) => evt.stopPropagation());
    requestAnimationFrame(() => {
      input.focus();
      input.setSelectionRange(input.value.length, input.value.length);
    });
    input.addEventListener('keydown', (evt) => {
      if (evt.key === 'Enter') {
        evt.preventDefault();
        commitInlineText();
      } else if (evt.key === 'Escape') {
        evt.preventDefault();
        cancelInlineText();
      }
    });
    input.addEventListener('blur', commitInlineText);
  }

  return { commitInlineText, cancelInlineText, openInlineText };
}
