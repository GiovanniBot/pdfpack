"""Gera os códigos Pix da doação (QR code e "copia e cola") para o site e para o aplicativo.

    python scripts/doacao.py

Lê `scripts/doacao.json` (a chave Pix, o nome e a cidade de quem recebe, e os valores) e escreve:

  landing/doacao-dados.js              o que a página mostra
  apps/pdfpack/src/donate_data.rs      o que o aplicativo mostra

Os códigos são Pix estáticos (BR Code, o padrão do Banco Central): o valor já vai preenchido e o
mesmo código serve para quantas doações vierem. Nada aqui fala com banco ou com a internet.

Precisa do pacote `segno` (BSD-3-Clause), só para desenhar o QR code: `pip install segno`.
"""
import json
import pathlib
import sys
import unicodedata

import segno

ROOT = pathlib.Path(__file__).resolve().parent.parent


def field(tag: str, value: str) -> str:
    return f'{tag}{len(value):02d}{value}'


def crc16(data: str) -> str:
    """CRC-16/CCITT-FALSE, como o BR Code pede."""
    crc = 0xFFFF
    for byte in data.encode('ascii'):
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021 if crc & 0x8000 else crc << 1) & 0xFFFF
    return f'{crc:04X}'


def plain(text: str, limit: int) -> str:
    """Sem acentos e em maiúsculas, que é o que todos os bancos leem."""
    text = unicodedata.normalize('NFKD', text).encode('ascii', 'ignore').decode()
    return ' '.join(text.upper().split())[:limit]


def brcode(key: str, name: str, city: str, amount: str, txid: str) -> str:
    account = field('00', 'br.gov.bcb.pix') + field('01', key)
    body = (field('00', '01') + field('26', account) + field('52', '0000') + field('53', '986') + field('54', amount)
            + field('58', 'BR') + field('59', plain(name, 25)) + field('60', plain(city, 15)) + field('62', field('05', txid)) + '6304')
    return body + crc16(body)


def main() -> int:
    cfg = json.loads((ROOT / 'scripts/doacao.json').read_text(encoding='utf-8'))
    key, name, city = cfg['chave'].strip(), cfg['nome'].strip(), cfg['cidade'].strip()
    if not (key and name and city):
        print('Preencha "chave", "nome" e "cidade" em scripts/doacao.json.')
        return 1
    if not cfg['txid'].isalnum() or len(cfg['txid']) > 25:
        print('"txid" aceita só letras e números, até 25.')
        return 1
    tiers = []
    for i, t in enumerate(cfg['valores']):
        amount = f"{float(t['valor']):.2f}"
        code = brcode(key, name, city, amount, cfg['txid'])
        qr = segno.make(code, error='m', micro=False)
        rows = [list(r) for r in qr.matrix]
        if len(rows) > 64:
            print('O código ficou grande demais para o QR do aplicativo: use uma chave mais curta.')
            return 1
        price = 'R$ ' + amount.replace('.', ',')
        tiers.append({'nome': t['nome'], 'preco': price, 'moedas': i + 1, 'codigo': code, 'linhas': rows})

    web = [{'nome': t['nome'], 'preco': t['preco'], 'moedas': t['moedas'], 'codigo': t['codigo'],
            'qr': [''.join('1' if m else '0' for m in r) for r in t['linhas']]} for t in tiers]
    js = ('// Gerado por scripts/doacao.py a partir de scripts/doacao.json. Não edite à mão.\n'
          f'window.PDFPACK_DOACAO = {json.dumps({"recebedor": plain(name, 25), "valores": web}, ensure_ascii=False, indent=1)};\n')
    (ROOT / 'landing/doacao-dados.js').write_text(js, encoding='utf-8', newline='\n')

    rs = ['//! Gerado por scripts/doacao.py a partir de scripts/doacao.json. Não edite à mão.', '',
          'use crate::donate::Tier;', '', '/// O nome de quem recebe, como o banco mostra antes de confirmar.',
          f'pub const RECEIVER: &str = {json.dumps(plain(name, 25))};', '', 'pub const TIERS: &[Tier] = &[']
    for t in tiers:
        masks = ', '.join(f'0x{sum(1 << x for x, m in enumerate(r) if m):x}' for r in t['linhas'])
        rs += ['    Tier {', f'        name: {json.dumps(t["nome"], ensure_ascii=False)},', f'        price: {json.dumps(t["preco"])},',
               f'        coins: {t["moedas"]},', f'        code: {json.dumps(t["codigo"])},', f'        size: {len(t["linhas"])},',
               f'        rows: &[{masks}],', '    },']
    rs += ['];', '']
    (ROOT / 'apps/pdfpack/src/donate_data.rs').write_text('\n'.join(rs), encoding='utf-8', newline='\n')
    for t in tiers:
        print(f"{t['nome']:<10} {t['preco']:<9} QR {len(t['linhas'])}x{len(t['linhas'])}  {t['codigo']}")
    return 0


if __name__ == '__main__':
    sys.exit(main())
