# Compila o PDFPack e publica a release da versão que está em apps/pdfpack/Cargo.toml.
# Os aplicativos instalados baixam o pdfpack.exe anexado à release mais nova.
param([string]$Notes = "")

$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

$version = (Select-String -Path apps/pdfpack/Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$tag = "v$version"
if (git status --porcelain) { throw "Há mudanças sem commit: faça o commit antes de publicar." }
gh release view $tag *> $null
if ($LASTEXITCODE -eq 0) { throw "A release $tag já existe: aumente a versão em apps/pdfpack/Cargo.toml." }

cargo build --release -p pdfpack
if ($LASTEXITCODE -ne 0) { throw "A compilação falhou." }
$exe = "target/release/pdfpack.exe"
$built = (& $exe --version | Out-String).Trim()
if ($built -ne $version) { throw "O executável diz $built, mas a versão é $version." }

git push origin HEAD
if (-not $Notes) { $Notes = "PDFPack $version" }
gh release create $tag $exe --title "PDFPack $version" --notes $Notes --target (git rev-parse HEAD)
if ($LASTEXITCODE -ne 0) { throw "Não foi possível criar a release." }
Write-Host "Publicado: $tag"
