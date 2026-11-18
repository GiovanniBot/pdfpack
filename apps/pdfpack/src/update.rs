//! Atualizações: o aplicativo procura a release mais nova no GitHub, baixa o executável e o troca
//! no lugar, sem instalador. A versão nova vale a partir da próxima vez que o aplicativo abre.

use std::io::{Read, Write};
use std::path::PathBuf;

use sha2::{Digest, Sha256};

/// O repositório cujas releases são as atualizações.
pub const REPO: &str = "GiovanniBot/pdfpack";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// O nome do executável anexado a cada release.
const ASSET: &str = "pdfpack.exe";
const AGENT: &str = concat!("PDFPack/", env!("CARGO_PKG_VERSION"));
/// Nenhum executável do PDFPack chega perto disso; um download maior está errado.
const MAX_BYTES: u64 = 300 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    url: String,
    size: u64,
    sha256: Option<String>,
}

#[derive(serde::Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(serde::Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

fn numbers(version: &str) -> Vec<u64> {
    version.trim_start_matches('v').split(['.', '-', '+']).map_while(|p| p.parse().ok()).collect()
}

/// Se `candidate` é uma versão mais nova que `current` ("0.10.0" é mais nova que "0.9.3").
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let (a, b) = (numbers(candidate), numbers(current));
    !a.is_empty() && a > b
}

/// A release mais nova, se for mais nova que esta e tiver o executável anexado.
pub fn check() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut response = ureq::get(&url).header("User-Agent", AGENT).header("Accept", "application/vnd.github+json").call().map_err(|e| e.to_string())?;
    let text = response.body_mut().read_to_string().map_err(|e| e.to_string())?;
    let api: ApiRelease = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !is_newer(&api.tag_name, VERSION) {
        return Ok(None);
    }
    let Some(asset) = api.assets.into_iter().find(|a| a.name.eq_ignore_ascii_case(ASSET)) else { return Ok(None) };
    // Só baixa do próprio GitHub, por HTTPS.
    if !asset.browser_download_url.starts_with("https://github.com/") || asset.size == 0 || asset.size > MAX_BYTES {
        return Ok(None);
    }
    Ok(Some(Release {
        version: api.tag_name.trim_start_matches('v').to_owned(),
        notes: api.body.unwrap_or_default().trim().to_owned(),
        url: asset.browser_download_url,
        size: asset.size,
        sha256: asset.digest.and_then(|d| d.strip_prefix("sha256:").map(str::to_lowercase)),
    }))
}

fn exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

/// Apaga o executável antigo que a última atualização deixou ao lado do novo.
pub fn clean_up() {
    if let Ok(exe) = exe() {
        let _ = std::fs::remove_file(exe.with_extension("old"));
        let _ = std::fs::remove_file(exe.with_extension("new"));
    }
}

/// Baixa `release` e troca o executável. `progress` recebe de 0 a 1 e devolve `false` para parar.
/// O executável em uso é renomeado (o Windows deixa) e o novo assume o nome dele.
pub fn install(release: &Release, progress: &mut dyn FnMut(f32) -> bool) -> Result<(), String> {
    let exe = exe()?;
    let (new, old) = (exe.with_extension("new"), exe.with_extension("old"));
    let result = download(release, &new, progress).and_then(|()| {
        let _ = std::fs::remove_file(&old);
        std::fs::rename(&exe, &old).map_err(|e| format!("não foi possível trocar o executável ({e}); a pasta do aplicativo permite gravação?"))?;
        std::fs::rename(&new, &exe).map_err(|e| {
            // Sem o novo no lugar, o antigo volta.
            let _ = std::fs::rename(&old, &exe);
            e.to_string()
        })
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&new);
    }
    result
}

fn download(release: &Release, to: &std::path::Path, progress: &mut dyn FnMut(f32) -> bool) -> Result<(), String> {
    let response = ureq::get(&release.url).header("User-Agent", AGENT).call().map_err(|e| e.to_string())?;
    let mut reader = response.into_body().into_reader();
    let mut file = std::fs::File::create(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        done += n as u64;
        if done > MAX_BYTES {
            return Err("o download é maior do que deveria".into());
        }
        hash.update(&buffer[..n]);
        file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        if !progress(done as f32 / release.size.max(1) as f32) {
            return Err("cancelado".into());
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    if done != release.size {
        return Err("o download veio incompleto".into());
    }
    // O GitHub publica o SHA-256 de cada anexo: o arquivo baixado tem de ser exatamente aquele.
    let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if release.sha256.as_ref().is_some_and(|want| *want != got) {
        return Err("o arquivo baixado não confere com o publicado".into());
    }
    let mut head = [0u8; 2];
    let is_exe = std::fs::File::open(to).and_then(|mut f| f.read_exact(&mut head)).is_ok() && &head == b"MZ";
    if !is_exe {
        return Err("o arquivo baixado não é um executável".into());
    }
    Ok(())
}

/// Abre a versão recém-instalada; quem chama fecha esta.
pub fn restart() -> Result<(), String> {
    std::process::Command::new(exe()?).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("v0.10.0", "0.9.3"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("v0.0.9", "0.1.0"));
        assert!(!is_newer("nightly", "0.1.0"));
    }
}
