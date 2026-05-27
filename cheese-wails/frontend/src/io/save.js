import { LoadAppState, SaveImage } from '../../wailsjs/go/main/App';

export function createSaveIO({ dom, state, settings }) {
  async function saveImageData(data, outputPath) {
    await SaveImage(data, outputPath);
    const loaded = await LoadAppState();
    settings.renderSettings(loaded);
    return loaded;
  }

  async function saveCanvas() {
    const blob = await new Promise((r) => dom.canvas.toBlob(r, 'image/png'));
    const dataUrl = await new Promise((resolve) => {
      const reader = new FileReader();
      reader.onloadend = () => resolve(reader.result);
      reader.readAsDataURL(blob);
    });
    return saveImageData(dataUrl, state.settings.outputPath || '');
  }

  function closeAfterSave(setIdle) {
    state.image = new Image();
    state.annotations = [];
    state.current = null;
    state.dragStart = null;
    state.penPoints = [];
    state.editor.selectedIndices = [];
    state.editor.hoveredIndex = -1;
    setIdle(true);
    window.runtime.WindowHide();
  }

  return { saveImageData, saveCanvas, closeAfterSave };
}
