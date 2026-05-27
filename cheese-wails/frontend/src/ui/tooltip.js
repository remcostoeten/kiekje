export function initTooltip(dom) {
  let tooltipTimeout = null;

  function showTooltip(btn) {
    const tip = btn.dataset.tip;
    const kbd = btn.dataset.kbd;
    if (!tip) return;
    dom.tooltipEl.innerHTML = `<span class="tt-label">${tip}</span>${kbd ? `<kbd class="tt-kbd">${kbd}</kbd>` : ''}`;
    const barRect = document.getElementById('bar').getBoundingClientRect();
    const btnRect = btn.getBoundingClientRect();
    dom.tooltipEl.style.top = `${btnRect.top - barRect.top - 8}px`;
    const tooltipWidth = dom.tooltipEl.offsetWidth;
    const center = (btnRect.left + btnRect.width / 2) - barRect.left;
    dom.tooltipEl.style.left = `${Math.max(4, Math.min(center - tooltipWidth / 2, barRect.width - tooltipWidth - 4))}px`;
    dom.tooltipEl.classList.remove('hidden');
    dom.tooltipEl.style.transformOrigin = `${Math.min(tooltipWidth / 2, center)}px bottom`;
    requestAnimationFrame(() => dom.tooltipEl.classList.add('is-open'));
  }

  function hideTooltip() {
    dom.tooltipEl.classList.remove('is-open');
    clearTimeout(tooltipTimeout);
    tooltipTimeout = setTimeout(() => dom.tooltipEl.classList.add('hidden'), 150);
  }

  document.querySelectorAll('#bar .btn[data-tip]').forEach((btn) => {
    btn.addEventListener('mouseenter', () => {
      clearTimeout(tooltipTimeout);
      showTooltip(btn);
    });
    btn.addEventListener('mouseleave', hideTooltip);
  });
}
