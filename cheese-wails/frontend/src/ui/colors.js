export const COLORS = ['#ff6b6b', '#ffa94d', '#ffd43b', '#69db7c', '#4dabf7', '#9775fa', '#f783ac', '#ededed'];

export function renderSwatchButtons(activeColor, className, activeClass) {
  return COLORS.map((c) =>
    `<button class="${className}${c === activeColor ? ` ${activeClass}` : ''}" data-color="${c}" style="background:${c}" title="${c}"></button>`,
  ).join('');
}

export function initColors({ dom, state, onColorApplied }) {
  function selectToolbarColor(color, swatch) {
    state.editor.activeColor = color;
    dom.colorSwatch.style.background = color;
    dom.colorPopper.querySelectorAll('.clr').forEach((b) => {
      b.classList.toggle('clr-on', b.dataset.color === color);
    });
    dom.colorPicker.querySelectorAll('.cp-swatch').forEach((b) => {
      b.classList.toggle('cp-active', b.dataset.color === color);
    });
    if (swatch) {
      swatch.classList.remove('clr-pop');
      void swatch.offsetWidth;
      swatch.classList.add('clr-pop');
    }
  }

  function buildToolbarColors() {
    dom.colorPopper.innerHTML = renderSwatchButtons(state.editor.activeColor, 'clr', 'clr-on');
    dom.colorPopper.addEventListener('click', (e) => {
      const swatch = e.target.closest('[data-color]');
      if (!swatch) return;
      selectToolbarColor(swatch.dataset.color, swatch);
      dom.colorPopper.classList.add('hidden');
    });
    dom.colorSwatch.style.background = state.editor.activeColor;
  }

  function closeContextPicker() {
    dom.colorPicker.classList.remove('is-open');
  }

  function openContextPicker(clientX, clientY, currentColor) {
    dom.colorPicker.innerHTML = renderSwatchButtons(currentColor, 'cp-swatch', 'cp-active');
    const editorRect = dom.editor.getBoundingClientRect();
    const pickerWidth = 232;
    const pickerHeight = 42;
    const margin = 8;
    const x = Math.max(margin, Math.min(clientX - editorRect.left, editorRect.width - pickerWidth - margin));
    const y = Math.max(margin, Math.min(clientY - editorRect.top, editorRect.height - pickerHeight - margin));
    dom.colorPicker.style.left = `${x}px`;
    dom.colorPicker.style.top = `${y}px`;
    closeContextPicker();
    requestAnimationFrame(() => {
      dom.colorPicker.classList.add('is-open');
    });
  }

  dom.colorTrigger.addEventListener('click', (e) => {
    e.stopPropagation();
    dom.colorPopper.classList.toggle('hidden');
  });

  document.addEventListener('click', (e) => {
    if (!dom.colorWrap.contains(e.target)) {
      dom.colorPopper.classList.add('hidden');
    }
    if (!dom.colorPicker.contains(e.target)) {
      closeContextPicker();
    }
  });

  dom.colorPicker.addEventListener('click', (e) => {
    const swatch = e.target.closest('[data-color]');
    if (!swatch) return;
    const color = swatch.dataset.color;
    selectToolbarColor(color, dom.colorPopper.querySelector(`.clr[data-color="${color}"]`));
    onColorApplied(color);
    closeContextPicker();
  });

  buildToolbarColors();

  return {
    selectToolbarColor,
    openContextPicker,
    closeContextPicker,
  };
}
