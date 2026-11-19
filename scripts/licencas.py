"""Gera apps/pdfpack/LICENCAS-DE-TERCEIROS.txt: os avisos de licença de tudo o que vai dentro do
PDFPack (as bibliotecas Rust e os arquivos embutidos, como fontes e ícones).

As licenças MIT, Apache, BSD e parecidas pedem que o texto da licença e os créditos de autoria
acompanhem o programa. O aplicativo mostra este arquivo em Sobre ▸ Licenças de terceiros, e os
pacotes de macOS e Linux o levam junto.

Uso:
    python scripts/licencas.py           # regrava o arquivo
    python scripts/licencas.py --check   # só confere se o arquivo está em dia (falha se não)

Rode depois de mudar dependências (Cargo.toml ou Cargo.lock). O scripts/release.ps1 confere.
"""

import hashlib
import json
import os
import subprocess
import sys
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "apps" / "pdfpack" / "LICENCAS-DE-TERCEIROS.txt"

# Os sistemas para os quais o PDFPack é publicado. Só entra o que vai em algum deles.
TARGETS = [
    "x86_64-pc-windows-gnu",
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
]

LICENSE_NAMES = ("license", "licence", "copying", "notice", "unlicense", "ofl", "ufl")
SKIP_DIRS = {"tests", "test", "benches", "examples", "target", ".git", "fuzz", "docs"}

# Arquivos embutidos no executável que não vêm de uma biblioteca: o que os cobre.
ASSETS = [
    (
        "Fontes Inter (assets/fonts/Inter-*.ttf), usadas na interface",
        "OFL-1.1",
        ["assets/fonts/OFL-Inter.txt"],
    ),
    (
        "Ícones Lucide (assets/icons/*.svg), usados na interface",
        "ISC AND MIT",
        ["assets/icons/LICENSE-lucide.txt"],
    ),
]

# Pacotes publicados sem o arquivo de licença: o mesmo texto vem de um pacote irmão, do mesmo
# autor e com a mesma licença (alloc-stdlib está no mesmo repositório de alloc-no-stdlib; a
# Boost Software License não tem linha de copyright, e error-code é do mesmo autor de
# clipboard-win).
SAME_AS = {
    "alloc-stdlib": "alloc-no-stdlib",
    "clipboard-win": "error-code",
}

# Pacotes cuja licença declarada é permissiva mas que trazem arquivos sob LGPL. O texto da LGPL
# vai junto, como ela pede; ela permite uso comercial, e o código do PDFPack é aberto.
LGPL_PARTS = {
    "wayland-protocols-plasma": (
        "No Linux, o código gerado do protocolo de desfoque do KDE (blur.xml, que a biblioteca"
        " de janelas winit usa no Wayland) está sob a LGPL-2.1-or-later. O código-fonte"
        " está em https://crates.io/crates/wayland-protocols-plasma e o do PDFPack em"
        " https://github.com/GiovanniBot/pdfpack."
    ),
}

SPDX_FALLBACK = {
    "MIT": ["LICENSE-MIT"],
    "Apache-2.0": ["LICENSE-APACHE"],
}


def metadata(target):
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target],
        cwd=ROOT,
        check=True,
        capture_output=True,
    ).stdout
    return json.loads(out)


def shipped():
    """Os pacotes que entram no executável do PDFPack em algum dos sistemas publicados."""
    packages = {}
    for target in TARGETS:
        m = metadata(target)
        pk = {p["id"]: p for p in m["packages"]}
        nodes = {n["id"]: n for n in m["resolve"]["nodes"]}
        root = next(p["id"] for p in m["packages"] if p["name"] == "pdfpack")
        seen, stack = set(), [root]
        while stack:
            i = stack.pop()
            if i in seen:
                continue
            seen.add(i)
            for d in nodes[i]["deps"]:
                # Só as dependências normais: as de compilação e de teste não vão no executável.
                if any(k["kind"] is None for k in d["dep_kinds"]):
                    stack.append(d["pkg"])
        for i in seen:
            p = pk[i]
            if p["name"] != "pdfpack":
                packages[(p["name"], p["version"])] = p
    return [packages[k] for k in sorted(packages)]


def is_license(name):
    low = name.lower()
    if low.endswith((".rs", ".toml", ".orig", ".json", ".py", ".sh")):
        return False
    return low.startswith(LICENSE_NAMES) or low.endswith(("-license.txt", "_license.txt", "license.md"))


def is_copyleft(text):
    """GPL e LGPL só aparecem como alternativa ("Apache-2.0 OR GPL-2.0-only"), e o PDFPack usa a
    outra opção; o texto delas não se aplica e só confundiria quem lê."""
    head = text.lstrip()[:200].upper()
    return head.startswith(("GNU GENERAL PUBLIC LICENSE", "GNU LESSER GENERAL PUBLIC LICENSE", "GNU LIBRARY GENERAL"))


def license_files(p):
    """Os arquivos de licença do pacote: na pasta dele, nas subpastas e, para pacotes de um
    repositório com vários pacotes, na raiz do repositório."""
    base = Path(p["manifest_path"]).parent
    found = []
    for dirpath, dirnames, filenames in os.walk(base):
        dirnames[:] = sorted(d for d in dirnames if d.lower() not in SKIP_DIRS)
        for f in sorted(filenames):
            if is_license(f):
                found.append(Path(dirpath) / f)
    if p.get("license_file"):
        lf = base / p["license_file"]
        if lf.is_file() and lf not in found:
            found.insert(0, lf)
    if not found:
        # Pacotes de um repositório git ou do próprio projeto: a licença fica numa pasta acima.
        up = base.parent
        for _ in range(4):
            files = [up / f for f in sorted(os.listdir(up)) if is_license(f) and (up / f).is_file()]
            if files:
                found = files
                break
            if (up / ".git").exists() or up == ROOT or up.parent == up:
                break
            up = up.parent
    return found


def read(path):
    text = path.read_text(encoding="utf-8", errors="replace").replace("\r\n", "\n").strip("\n﻿")
    return text


def build():
    blocks = {}  # hash -> (titulo do arquivo, texto, [pacotes])
    order = []
    missing = []

    def add(owner, path, text):
        key = hashlib.sha256(text.encode()).hexdigest()
        if key not in blocks:
            blocks[key] = (path.name, text, [])
            order.append(key)
        blocks[key][2].append(owner)

    packages = shipped()
    by_name = {p["name"]: p for p in packages}
    lines = []
    for p in packages:
        lic = p["license"] or "veja o texto da licença abaixo"
        authors = ", ".join(a.split("<")[0].strip() for a in p.get("authors") or [] if a.strip())
        owner = f'{p["name"]} {p["version"]}'
        lines.append(f"  {owner}  —  {lic}" + (f"  —  {authors}" if authors else ""))
        files = license_files(p)
        if not files and p["name"] in SAME_AS and SAME_AS[p["name"]] in by_name:
            sibling = by_name[SAME_AS[p["name"]]]
            if sibling["license"] == p["license"]:
                files = license_files(sibling)
        if not files:
            for spdx, names in SPDX_FALLBACK.items():
                if spdx in (p["license"] or ""):
                    files = [ROOT / n for n in names]
                    break
        if not files:
            missing.append(owner)
            continue
        for f in files:
            text = read(f)
            if is_copyleft(text) and p["name"] not in LGPL_PARTS:
                if " OR " not in (p["license"] or ""):
                    sys.exit(f"{owner} traz uma licença GPL/LGPL sem alternativa: revise antes de publicar")
                continue
            add(owner, f, text)

    for title, lic, paths in ASSETS:
        lines.append(f"  {title}  —  {lic}")
        for rel in paths:
            add(title, ROOT / rel, read(ROOT / rel))

    if missing:
        sys.exit("Sem arquivo de licença: " + ", ".join(missing))

    own = read(ROOT / "LICENSE-MIT")
    out = [
        "PDFPack — licenças de terceiros",
        "=" * 31,
        "",
        "O PDFPack é software livre, sob a licença MIT ou Apache-2.0, à sua escolha. Foi feito",
        "sobre o motor do PDFCraft, dos autores abaixo:",
        "",
        own,
        "",
        "O PDFPack usa as bibliotecas e os arquivos listados a seguir. Quando um item oferece",
        "mais de uma licença (por exemplo \"MIT OR Apache-2.0\"), o PDFPack o usa sob uma das",
        "licenças permissivas oferecidas (MIT, Apache-2.0, BSD ou Zlib), nunca sob GPL ou LGPL.",
        "",
        "Avisos exigidos:",
        "  - Este software é baseado em parte no trabalho do Independent JPEG Group",
        "    (biblioteca jpeg-encoder, licença IJG).",
        "  - A lista de autoridades certificadoras usada para checar atualizações vem do",
        "    pacote webpki-roots (dados da Mozilla), sob a licença CDLA-Permissive-2.0,",
        "    cujo texto está abaixo.",
        *(textwrap.fill(f"- {name}: {note}", 88, initial_indent="  ", subsequent_indent="    ",
                       break_on_hyphens=False)
          for name, note in LGPL_PARTS.items()),
        "",
        f"Bibliotecas e arquivos ({len(lines)}):",
        "",
        *lines,
        "",
        "Textos das licenças",
        "===================",
    ]
    for key in order:
        name, text, owners = blocks[key]
        out += [
            "",
            "-" * 78,
            textwrap.fill(f"{name} — de: " + ", ".join(sorted(set(owners))), 78, break_on_hyphens=False),
            "-" * 78,
            "",
            text,
        ]
    return "\n".join(out) + "\n"


def main():
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8", errors="replace")
    text = build()
    if "--check" in sys.argv:
        current = OUT.read_text(encoding="utf-8").replace("\r\n", "\n") if OUT.exists() else ""
        if current != text:
            sys.exit(f"{OUT.relative_to(ROOT)} está desatualizado: rode python scripts/licencas.py")
        print("Licenças de terceiros em dia.")
        return
    OUT.write_text(text, encoding="utf-8", newline="\n")
    print(f"Gravado {OUT.relative_to(ROOT)} ({len(text) // 1024} KB)")


if __name__ == "__main__":
    main()
