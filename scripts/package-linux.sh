#!/usr/bin/env bash
# Compila o PDFPack para Linux (x86_64) e monta dist/pdfpack-linux-x86_64.tar.gz, com o
# executável, o ícone e a entrada de menu. É o que o GitHub Actions chama.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n 's/^version = "\(.*\)"/\1/p' apps/pdfpack/Cargo.toml | head -1)
target_dir=${CARGO_TARGET_DIR:-target}
dir="dist/pdfpack-linux-x86_64"

cargo build --release --locked -p pdfpack
built=$("$target_dir/release/pdfpack" --version)
[ "$built" = "$version" ] || { echo "O executável diz $built, mas a versão é $version." >&2; exit 1; }

rm -rf dist
mkdir -p "$dir"
cp "$target_dir/release/pdfpack" "$dir/pdfpack"
cp apps/pdfpack/assets/icon.png "$dir/pdfpack.png"
cp apps/pdfpack/assets/linux/pdfpack.desktop apps/pdfpack/assets/linux/instalar.sh apps/pdfpack/assets/linux/LEIA-ME.txt "$dir/"
# Os avisos de licença das bibliotecas e arquivos embutidos, que as licenças pedem junto do programa.
cp apps/pdfpack/LICENCAS-DE-TERCEIROS.txt "$dir/"
chmod +x "$dir/pdfpack" "$dir/instalar.sh"
tar -C dist -czf dist/pdfpack-linux-x86_64.tar.gz pdfpack-linux-x86_64
echo "Pronto: dist/pdfpack-linux-x86_64.tar.gz ($version)"
