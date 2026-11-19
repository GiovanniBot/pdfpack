// A seção de doação: três valores fixos, cada um com o seu Pix (QR code e copia e cola).
// Os códigos vêm prontos de doacao-dados.js, gerado por scripts/doacao.py.
(() => {
  const data = window.PDFPACK_DOACAO;
  const tiers = document.querySelector('#tiers');
  const dialog = document.querySelector('#pix-dialog');
  if (!data || !tiers || !dialog) return;

  // Uma pilha de moedas, desenhada aqui: uma, duas ou três.
  const coins = count => {
    let body = '';
    for (let i = 0; i < count; i++) {
      const y = 44 - i * 11;
      body += `<path d="M8 ${y}v7c0 3.9 7.2 7 16 7s16-3.1 16-7v-7" fill="#e9b800" stroke="#19241f" stroke-width="2"/>` +
        `<ellipse cx="24" cy="${y}" rx="16" ry="7" fill="#ffda24" stroke="#19241f" stroke-width="2"/>`;
    }
    const top = 44 - (count - 1) * 11;
    body += `<ellipse cx="24" cy="${top}" rx="9" ry="3.4" fill="none" stroke="#19241f" stroke-width="1.4"/>`;
    return `<svg viewBox="0 0 48 64" width="60" height="80" aria-hidden="true">${body}</svg>`;
  };

  const qr = rows => {
    const size = rows.length + 8;
    let path = '';
    rows.forEach((row, y) => {
      for (let x = 0; x < row.length; x++) {
        if (row[x] !== '1') continue;
        let end = x;
        while (row[end + 1] === '1') end++;
        path += `M${x + 4} ${y + 4}h${end - x + 1}v1h-${end - x + 1}z`;
        x = end;
      }
    });
    return `<svg viewBox="0 0 ${size} ${size}" shape-rendering="crispEdges"><rect width="${size}" height="${size}" fill="#fff"/><path d="${path}" fill="#000"/></svg>`;
  };

  const status = dialog.querySelector('#pix-status');
  const code = dialog.querySelector('#pix-code');
  const open = tier => {
    dialog.querySelector('#pix-name').textContent = tier.nome;
    dialog.querySelector('#pix-price').textContent = tier.preco;
    dialog.querySelector('#pix-receiver').textContent = data.recebedor;
    dialog.querySelector('#pix-qr').innerHTML = qr(tier.qr);
    code.value = tier.codigo;
    status.textContent = 'Obrigado por apoiar o PDFPack.';
    if (typeof dialog.showModal === 'function') dialog.showModal(); else dialog.setAttribute('open', '');
  };

  data.valores.forEach(tier => {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'tier';
    button.innerHTML = coins(tier.moedas) + '<strong></strong><span></span>';
    button.querySelector('strong').textContent = tier.nome;
    button.querySelector('span').textContent = tier.preco;
    button.addEventListener('click', () => open(tier));
    tiers.append(button);
  });

  dialog.querySelector('#pix-copy').addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(code.value);
      status.textContent = 'Código copiado. Cole no Pix copia e cola do seu banco.';
    } catch {
      code.select();
      status.textContent = 'Selecione o código acima e copie.';
    }
  });
  code.addEventListener('focus', () => code.select());
  dialog.addEventListener('click', event => {
    if (event.target !== dialog) return;
    const b = dialog.getBoundingClientRect();
    if (event.clientX < b.left || event.clientX > b.right || event.clientY < b.top || event.clientY > b.bottom) dialog.close();
  });
})();
