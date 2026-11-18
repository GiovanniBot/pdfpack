#!/usr/bin/env bash
# Monta o PDFPack.app (Intel e Apple Silicon no mesmo executável) e o compacta em
# dist/PDFPack-macos.zip. Roda em um Mac com o Rust instalado; é o que o GitHub Actions chama.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n 's/^version = "\(.*\)"/\1/p' apps/pdfpack/Cargo.toml | head -1)
app="dist/PDFPack.app"

rustup target add aarch64-apple-darwin x86_64-apple-darwin
# Roda em qualquer Mac com o macOS 11 ou mais novo, não só no sistema de quem compilou.
export MACOSX_DEPLOYMENT_TARGET=11.0
cargo build --release --locked -p pdfpack --target aarch64-apple-darwin
cargo build --release --locked -p pdfpack --target x86_64-apple-darwin

rm -rf dist
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
lipo -create -output "$app/Contents/MacOS/pdfpack" \
    target/aarch64-apple-darwin/release/pdfpack target/x86_64-apple-darwin/release/pdfpack

# O ícone do aplicativo, nos tamanhos que o macOS pede, a partir do logo do repositório.
icons="dist/PDFPack.iconset"
mkdir -p "$icons"
for size in 16 32 128 256; do
    sips -z "$size" "$size" apps/pdfpack/assets/logo.png --out "$icons/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" apps/pdfpack/assets/logo.png --out "$icons/icon_${size}x${size}@2x.png" >/dev/null
done
sips -z 512 512 apps/pdfpack/assets/logo.png --out "$icons/icon_512x512.png" >/dev/null
iconutil -c icns "$icons" -o "$app/Contents/Resources/PDFPack.icns"
rm -rf "$icons"

cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>PDFPack</string>
    <key>CFBundleDisplayName</key><string>PDFPack</string>
    <key>CFBundleIdentifier</key><string>io.github.giovannibot.pdfpack</string>
    <key>CFBundleExecutable</key><string>pdfpack</string>
    <key>CFBundleIconFile</key><string>PDFPack</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF

# Assinatura local ("ad hoc"): o Apple Silicon não abre um executável sem nenhuma. Não é a
# assinatura de desenvolvedor da Apple, então o macOS ainda pede confirmação na primeira abertura.
codesign --force --deep --sign - "$app"
codesign --verify --deep --strict "$app"

built=$("$app/Contents/MacOS/pdfpack" --version)
[ "$built" = "$version" ] || { echo "O executável diz $built, mas a versão é $version." >&2; exit 1; }

ditto -c -k --keepParent "$app" dist/PDFPack-macos.zip
echo "Pronto: dist/PDFPack-macos.zip ($version)"
