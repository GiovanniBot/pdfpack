param([ValidateRange(1,65535)][int]$Port = 4173, [string]$VideoPath)

$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
$output = Join-Path $project 'dist/landing'
$media = Join-Path $output 'media'
New-Item -ItemType Directory -Force -Path $media | Out-Null

if (-not $VideoPath) {
    $VideoPath = Join-Path $project 'video/pdfpack-apresentacao.mp4'
}
Copy-Item -LiteralPath $VideoPath -Destination (Join-Path $media 'pdfpack-apresentacao.mp4')
foreach ($pageFile in 'index.html','styles.css','downloads.js','doacao.js','doacao-dados.js') {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $pageFile) -Destination $output
}
$assets = @(
    'apps/pdfpack/assets/banner.png', 'apps/pdfpack/assets/icon.png',
    'apps/pdfpack/screenshots/emails.png',
    'assets/fonts/Inter-Regular.ttf', 'assets/fonts/Inter-SemiBold.ttf',
    'assets/icons/monitor.svg', 'assets/icons/shield-check.svg', 'assets/icons/code-xml.svg',
    'assets/icons/clock.svg', 'assets/icons/sliders-horizontal.svg',
    'assets/icons/folder-open.svg', 'assets/icons/files.svg'
)
foreach ($asset in $assets) {
    Copy-Item -LiteralPath (Join-Path $project $asset) -Destination $media
}
Push-Location $PSScriptRoot
try {
    & npx.cmd --yes vite@8.3.7 preview --outDir $output --host 127.0.0.1 --port $Port --strictPort
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
