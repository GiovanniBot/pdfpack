#!/bin/sh
# Instala o PDFPack só para o usuário atual, sem senha de administrador: o executável em
# ~/.local/bin e a entrada no menu de aplicativos. Para remover: ./instalar.sh --remover
set -eu
here=$(cd "$(dirname "$0")" && pwd)
data=${XDG_DATA_HOME:-$HOME/.local/share}
bin="$HOME/.local/bin/pdfpack"
entry="$data/applications/pdfpack.desktop"
icon="$data/icons/hicolor/256x256/apps/pdfpack.png"

if [ "${1:-}" = "--remover" ]; then
    rm -f "$bin" "$entry" "$icon"
    echo "PDFPack removido."
    exit 0
fi

mkdir -p "$(dirname "$bin")" "$(dirname "$entry")" "$(dirname "$icon")"
cp "$here/pdfpack" "$bin"
chmod +x "$bin"
cp "$here/pdfpack.png" "$icon"
# O caminho inteiro do executável, para o menu funcionar mesmo sem ~/.local/bin no PATH.
sed "s|^Exec=pdfpack|Exec=\"$bin\"|" "$here/pdfpack.desktop" > "$entry"
echo "PDFPack instalado. Procure por PDFPack no menu de aplicativos, ou rode: $bin"
