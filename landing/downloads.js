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
  download.textContent = 'Baixar para ' + option.querySelector('strong').textContent;
}

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
