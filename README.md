# PDFPack

Aplicativo de desktop para Windows que faz duas coisas: **comprime PDFs** e os **organiza em pastas
que cabem em um email**. Feito em Rust, sobre o motor do [PDFCraft](https://github.com/storytold/pdfcraft).

## O que faz

- **Comprimir** — quatro níveis (Mínima, Média, Alta e Extrema). O nível Média equivale à
  otimização do PDFCraft com imagens a 100 ppi, JPEG de qualidade mínima e CCITT grupo 4.
- **Organizar para email** — você informa o tamanho máximo de um email (25 MB por padrão) e os
  PDFs são distribuídos, na ordem da lista, em pastas `Email 1`, `Email 2`… Um PDF maior que o
  limite é dividido em partes.
- **Salvar** (Ctrl+S) — grava as pastas com os PDFs já comprimidos e organizados. O arquivo de
  origem nunca é sobrescrito.
- Visualizador com minimapa de páginas, ordenação por numeração, nome, tamanho ou data,
  arrastar e soltar para reordenar, desfazer e refazer (Ctrl+Z / Ctrl+Shift+Z).

## Instalar

Baixe `pdfpack.exe` na [última release](https://github.com/GiovanniBot/pdfpack/releases/latest) e
abra. Não há instalador: o executável roda de qualquer pasta em que você possa gravar.

## Atualizações

Ao abrir, o PDFPack consulta as releases deste repositório. Quando a primeira versão nova chega,
ele pergunta se deve **atualizar sempre sozinho** ou **perguntar a cada atualização**; a escolha
pode ser mudada no botão de informações (ⓘ) da barra superior. A atualização baixa o novo
executável do GitHub por HTTPS, confere o SHA-256 publicado na release, troca o arquivo no lugar e
passa a valer na próxima abertura.

## Compilar

Precisa do Rust 1.95 ou mais novo.

```
cargo build --release -p pdfpack
```

O executável sai em `target/release/pdfpack.exe`.

## Publicar uma versão

1. Aumente `version` em `apps/pdfpack/Cargo.toml`.
2. Faça o commit e envie.
3. Rode `scripts/release.ps1` (precisa do [GitHub CLI](https://cli.github.com/) autenticado).

O script compila, cria a tag `vX.Y.Z` e publica a release com `pdfpack.exe` anexado. É esse anexo
que os aplicativos instalados baixam.

## Estrutura

- `apps/pdfpack` — o aplicativo (interface em egui).
- `crates/` — o motor herdado do PDFCraft: leitura e escrita de PDF, otimizador, divisão de
  páginas e renderizador.
- `vendor/` — dependências de terceiros com correções do PDFCraft.

## Licença e créditos

MIT ou Apache-2.0, à sua escolha, como o PDFCraft. O motor de PDF é do projeto
[PDFCraft](https://github.com/storytold/pdfcraft) e seus contribuidores; veja `NOTICE`,
`ATTRIBUTION.md` e `licenses/`. Ícones [Lucide](https://lucide.dev) (ISC/MIT). Fonte Inter (OFL).
