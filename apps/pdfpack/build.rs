//! Põe o ícone no executável do Windows.

fn main() {
    println!("cargo:rerun-if-changed=assets/pdfpack.ico");
    println!("cargo:rerun-if-changed=assets/pdfpack.rc");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // Sem o compilador de recursos o aplicativo ainda compila, só fica sem o ícone no arquivo.
    if let Err(error) = icon() {
        println!("cargo:warning=ícone do executável não incluído: {error}");
    }
}

#[cfg(windows)]
fn icon() -> Result<(), String> {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu") {
        return winresource::WindowsResource::new().set_icon("assets/pdfpack.ico").compile().map_err(|e| e.to_string());
    }
    // O windres passa os caminhos sem aspas ao pré-processador e se perde em pastas com espaço
    // no nome. Rodando dentro de `assets`, com nomes relativos, só a saída leva o caminho inteiro,
    // e essa ele mesmo abre.
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").map_err(|e| e.to_string())?).join("pdfpack-icon.o");
    let status = std::process::Command::new("windres")
        .current_dir("assets")
        .args(["--input", "pdfpack.rc", "--output-format=coff", "--output"])
        .arg(&out)
        .status()
        .map_err(|e| format!("windres: {e}"))?;
    if !status.success() {
        return Err("o windres falhou".into());
    }
    println!("cargo:rustc-link-arg-bins={}", out.display());
    Ok(())
}

#[cfg(not(windows))]
fn icon() -> Result<(), String> {
    Err("só é feito ao compilar no Windows".into())
}
