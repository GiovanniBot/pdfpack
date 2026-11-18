//! Instalação sem instalador: o próprio executável se copia para a pasta de programas do usuário,
//! cria os atalhos e se registra em "Aplicativos instalados". Nada pede senha de administrador,
//! e a pasta continua gravável, que é o que a atualização automática precisa.

use std::path::{Path, PathBuf};
use std::process::Command;

const EXE: &str = "pdfpack.exe";

/// Cria o atalho do menu Iniciar (e o da área de trabalho, se pedido) e registra a desinstalação.
/// Recebe os caminhos por variáveis de ambiente, para não depender de aspas na linha de comando.
const INSTALL: &str = r#"
$ErrorActionPreference = 'Stop'
$shell = New-Object -ComObject WScript.Shell
$places = @('Programs')
if ($env:PDFPACK_DESKTOP -eq '1') { $places += 'Desktop' }
foreach ($place in $places) {
    $link = $shell.CreateShortcut((Join-Path ([Environment]::GetFolderPath($place)) 'PDFPack.lnk'))
    $link.TargetPath = $env:PDFPACK_EXE
    $link.WorkingDirectory = $env:PDFPACK_DIR
    $link.Description = 'Comprime PDFs e organiza anexos por email'
    $link.Save()
}
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\PDFPack'
New-Item -Path $key -Force | Out-Null
$quote = [char]34
Set-ItemProperty -LiteralPath $key -Name DisplayName -Value 'PDFPack'
Set-ItemProperty -LiteralPath $key -Name Publisher -Value 'PDFPack'
Set-ItemProperty -LiteralPath $key -Name DisplayIcon -Value $env:PDFPACK_EXE
Set-ItemProperty -LiteralPath $key -Name InstallLocation -Value $env:PDFPACK_DIR
Set-ItemProperty -LiteralPath $key -Name UninstallString -Value ($quote + $env:PDFPACK_EXE + $quote + ' --uninstall')
Set-ItemProperty -LiteralPath $key -Name QuietUninstallString -Value ($quote + $env:PDFPACK_EXE + $quote + ' --uninstall --quiet')
Set-ItemProperty -LiteralPath $key -Name NoModify -Value 1 -Type DWord
Set-ItemProperty -LiteralPath $key -Name NoRepair -Value 1 -Type DWord
"#;

/// Desfaz a instalação. Roda depois que o aplicativo fecha, porque um executável aberto não pode
/// ser apagado. Só apaga os arquivos que o PDFPack criou, pelo nome: nunca uma pasta com o que
/// houver dentro. Um atalho só sai se apontar para este executável.
const UNINSTALL: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$shell = New-Object -ComObject WScript.Shell
foreach ($place in 'Programs', 'Desktop') {
    $path = Join-Path ([Environment]::GetFolderPath($place)) 'PDFPack.lnk'
    if ((Test-Path -LiteralPath $path) -and ($shell.CreateShortcut($path).TargetPath -eq $env:PDFPACK_EXE)) {
        Remove-Item -LiteralPath $path -Force
    }
}
Remove-Item -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\PDFPack' -Force
for ($i = 0; $i -lt 20 -and (Test-Path -LiteralPath $env:PDFPACK_EXE); $i++) {
    Start-Sleep -Milliseconds 700
    Remove-Item -LiteralPath $env:PDFPACK_EXE -Force
}
foreach ($name in 'pdfpack.old', 'pdfpack.new') {
    Remove-Item -LiteralPath (Join-Path $env:PDFPACK_DIR $name) -Force
}
try { [IO.Directory]::Delete($env:PDFPACK_DIR) } catch {}
if ($env:PDFPACK_PREFS) {
    Remove-Item -LiteralPath (Join-Path $env:PDFPACK_PREFS 'preferencias.json') -Force
    try { [IO.Directory]::Delete($env:PDFPACK_PREFS) } catch {}
}
"#;

/// A pasta em que o PDFPack se instala: `%LOCALAPPDATA%\Programs\PDFPack`.
pub fn dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Programs").join("PDFPack"))
}

fn same(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()),
    }
}

/// Se este executável é o instalado (e não, por exemplo, o que está na pasta Downloads).
pub fn is_installed() -> bool {
    let (Ok(exe), Some(dir)) = (std::env::current_exe(), dir()) else { return false };
    same(&exe, &dir.join(EXE))
}

fn powershell(script: &str, vars: &[(&str, &std::ffi::OsStr)]) -> Command {
    let mut command = Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script]);
    for (name, value) in vars {
        command.env(name, value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: sem uma janela de console piscando na frente do usuário.
        command.creation_flags(0x0800_0000);
    }
    command
}

/// Copia este executável para a pasta de instalação, cria os atalhos e registra a desinstalação.
/// Devolve o caminho do executável instalado.
pub fn install(desktop: bool) -> Result<PathBuf, String> {
    let from = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = dir().ok_or("não foi possível achar a pasta de programas do usuário")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let to = dir.join(EXE);
    if !same(&from, &to) {
        std::fs::copy(&from, &to).map_err(|e| format!("não foi possível copiar o PDFPack para {} ({e}). Se o PDFPack instalado estiver aberto, feche-o e tente de novo.", dir.display()))?;
    }
    let vars = [("PDFPACK_EXE", to.as_os_str()), ("PDFPACK_DIR", dir.as_os_str()), ("PDFPACK_DESKTOP", std::ffi::OsStr::new(if desktop { "1" } else { "0" }))];
    let output = powershell(INSTALL, &vars).output().map_err(|e| format!("o PDFPack foi copiado, mas os atalhos não puderam ser criados ({e})"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.lines().find(|l| !l.trim().is_empty()).unwrap_or("erro desconhecido").trim().to_owned();
        return Err(format!("o PDFPack foi copiado para {}, mas os atalhos não puderam ser criados: {detail}", dir.display()));
    }
    Ok(to)
}

/// Agenda a remoção dos atalhos, do registro, do executável instalado e das preferências. Quem
/// chama fecha o aplicativo em seguida: a remoção espera por isso.
pub fn uninstall() -> Result<(), String> {
    if !is_installed() {
        return Err("Este PDFPack não está instalado: ele está rodando direto do arquivo baixado. Para removê-lo, basta apagar o arquivo.".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = dir().ok_or("não foi possível achar a pasta de instalação")?;
    let prefs = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("PDFPack")).unwrap_or_default();
    let vars = [("PDFPACK_EXE", exe.as_os_str()), ("PDFPACK_DIR", dir.as_os_str()), ("PDFPACK_PREFS", prefs.as_os_str())];
    powershell(UNINSTALL, &vars).spawn().map(|_| ()).map_err(|e| format!("não foi possível desinstalar ({e})"))
}

/// Abre o executável instalado; quem chama fecha este.
pub fn launch(exe: &Path) -> Result<(), String> {
    Command::new(exe).spawn().map(|_| ()).map_err(|e| e.to_string())
}
