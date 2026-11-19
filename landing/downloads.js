function desktopSystem() {
  const hints = navigator.userAgentData;
  const agent = navigator.userAgent;
  const platform = hints?.platform || navigator.platform || agent;

  // iPadOS can identify itself as a Mac in desktop browsing mode.
  if (hints?.mobile || /Android|iPhone|iPad|iPod|CrOS|Windows Phone/i.test(agent) ||
      (/Mac/i.test(platform) && navigator.maxTouchPoints > 1)) return null;
  if (/^Win|Windows NT/i.test(platform)) return 'windows';
  if (/Mac/i.test(platform)) return 'macos';
  if (/Linux/i.test(platform) && !/arm|aarch64|i[3-6]86/i.test(navigator.platform + ' ' + agent)) return 'linux';
  return null;
}

const dialog = document.querySelector('#download-dialog');
const picker = document.querySelector('#system-picker');
const system = desktopSystem();
if (system) {
  const option = dialog.querySelector('[data-system="' + system + '"]');
  const download = document.querySelector('#device-download');
  download.href = option.href;
  download.target = '_blank';
  download.rel = 'noopener';
  download.dataset.system = system;
  download.textContent = 'Baixar para ' + option.querySelector('strong').textContent;
}

// Downloads open in a new tab, which the browser closes as soon as the file
// starts, so the visitor never leaves this page even where a browser would
// otherwise show GitHub. Each place confirms the download where it was
// clicked: the main button says so on itself, and the system picker (opened
// only from its own footer button) says so inside the dialog.
const status = document.querySelector('#download-status');
const heroButton = document.querySelector('#device-download');
let heroLabel = heroButton.textContent;
let heroTimer = 0;
document.addEventListener('click', event => {
  const link = event.target.closest('a[data-system]');
  if (!link) return;
  if (link === heroButton) {
    clearTimeout(heroTimer);
    heroButton.textContent = 'Download iniciado ✓';
    heroTimer = setTimeout(() => { heroButton.textContent = heroLabel; }, 4000);
    return;
  }
  const option = dialog.querySelector('[data-system="' + link.dataset.system + '"]');
  if (!option) return;
  status.textContent = 'O download do PDFPack para ' + option.querySelector('strong').textContent +
    ' começou. Confira a pasta Downloads.';
  status.hidden = false;
  dialog.querySelectorAll('[data-system]').forEach(a => a.classList.toggle('chosen', a === option));
});
picker.addEventListener('submit', () => {
  status.hidden = true;
  dialog.querySelectorAll('.chosen').forEach(a => a.classList.remove('chosen'));
});

if (typeof dialog.showModal === 'function') {
  picker.querySelector('button').setAttribute('aria-haspopup', 'dialog');
  picker.addEventListener('submit', event => {
    event.preventDefault();
    dialog.showModal();
  });
  dialog.addEventListener('click', event => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right ||
        event.clientY < bounds.top || event.clientY > bounds.bottom) dialog.close();
  });
}
