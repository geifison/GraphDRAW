//! `graphdraw convert` (GD-EXP-06) e conversão em lote (GD-CDR-07).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_graphdraw"))
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn run(args: &[&std::ffi::OsStr]) -> Output {
    bin().args(args).output().unwrap()
}

fn os<S: AsRef<std::ffi::OsStr> + ?Sized>(s: &S) -> &std::ffi::OsStr {
    s.as_ref()
}

fn bridge_available() -> bool {
    let found = graphdraw_core::cdr::bridge::locate().is_some();
    if !found && std::env::var_os("GRAPHDRAW_REQUIRE_BRIDGE").is_some() {
        panic!("gd-cdr-bridge não encontrado e GRAPHDRAW_REQUIRE_BRIDGE está definida");
    }
    found
}

#[test]
fn convert_gdr_to_each_format_by_extension() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixtures().join("gdr/basic-shapes.gdr");
    for ext in ["svg", "pdf", "png", "gdr"] {
        let out = dir.path().join(format!("saida.{ext}"));
        let o = run(&[
            os("convert"),
            input.as_os_str(),
            out.as_os_str(),
            os("--dpi"),
            os("50"),
        ]);
        assert!(
            o.status.success(),
            "{ext}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        let bytes = std::fs::read(&out).unwrap();
        match ext {
            "svg" => assert!(bytes.starts_with(b"<?xml")),
            "pdf" => assert!(bytes.starts_with(b"%PDF-")),
            "png" => assert!(bytes.starts_with(b"\x89PNG")),
            _ => assert!(graphdraw_core::gdr::sniff(&bytes)),
        }
    }
}

#[test]
fn page_dpi_and_text_to_curves_flags() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixtures().join("gdr/multipage.gdr");
    let out = dir.path().join("p.png");
    let o = run(&[
        os("convert"),
        input.as_os_str(),
        out.as_os_str(),
        os("--page"),
        os("3"),
        os("--dpi"),
        os("254"),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let img = image::open(&out).unwrap();
    assert_eq!(
        (img.width(), img.height()),
        (900, 500),
        "cartão 90×50 mm a 254 ppp"
    );

    // Sem --page: uma saída por página.
    let o = run(&[
        os("convert"),
        input.as_os_str(),
        dir.path().join("todas.svg").as_os_str(),
    ]);
    assert!(o.status.success());
    for n in 1..=3 {
        assert!(dir.path().join(format!("todas-{n}.svg")).is_file());
    }

    let text = fixtures().join("gdr/text.gdr");
    let curves = dir.path().join("curvas.svg");
    let o = run(&[
        os("convert"),
        text.as_os_str(),
        curves.as_os_str(),
        os("--text-to-curves"),
    ]);
    assert!(o.status.success());
    assert!(!std::fs::read_to_string(&curves).unwrap().contains("<text"));
}

#[test]
fn errors_have_clear_messages_and_exit_codes() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixtures().join("gdr/colors.gdr");
    let o = run(&[
        os("convert"),
        input.as_os_str(),
        dir.path().join("x.docx").as_os_str(),
    ]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("formato de saída desconhecido"));

    let o = run(&[
        os("convert"),
        input.as_os_str(),
        dir.path().join("x.pdf").as_os_str(),
        os("--page"),
        os("9"),
    ]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("fora do documento"));

    let o = run(&[
        os("convert"),
        fixtures().join("gdr/future-version.gdr").as_os_str(),
        dir.path().join("x.pdf").as_os_str(),
    ]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("versão não suportada"));

    let junk = dir.path().join("lixo.bin");
    std::fs::write(&junk, b"nada a ver").unwrap();
    let o = run(&[
        os("convert"),
        junk.as_os_str(),
        dir.path().join("x.pdf").as_os_str(),
    ]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("não reconhecido"));
}

#[test]
fn gd_cdr_07_batch_folder_conversion_with_report() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("acervo");
    std::fs::create_dir(&src).unwrap();
    for n in ["colors.gdr", "gradients.gdr", "future-version.gdr"] {
        std::fs::copy(fixtures().join("gdr").join(n), src.join(n)).unwrap();
    }
    let with_cdr = bridge_available();
    if with_cdr {
        std::fs::copy(
            fixtures().join("cdr/fdo65220-2.cdr"),
            src.join("fdo65220-2.cdr"),
        )
        .unwrap();
        std::fs::copy(
            fixtures().join("cdr/EDB-35217-2.cdr"),
            src.join("EDB-35217-2.cdr"),
        )
        .unwrap();
    }
    std::fs::write(src.join("leiame.txt"), b"ignorado").unwrap();
    let out = dir.path().join("pdfs");
    let report = dir.path().join("relatorio.json");

    // Sem --format no modo lote: erro.
    let o = run(&[os("convert"), src.as_os_str(), out.as_os_str()]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("--format"));

    let o = run(&[
        os("convert"),
        src.as_os_str(),
        out.as_os_str(),
        os("--format"),
        os("pdf"),
        os("--report"),
        report.as_os_str(),
    ]);
    // Um arquivo com versão futura (e um .cdr não suportado) falham; o resto converte.
    assert!(!o.status.success());
    assert!(out.join("colors.pdf").is_file());
    assert!(out.join("gradients.pdf").is_file());
    assert!(!out.join("future-version.pdf").exists());
    let r: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    let entries = r.as_array().unwrap();
    assert_eq!(entries.len(), if with_cdr { 5 } else { 3 });
    if with_cdr {
        assert!(out.join("fdo65220-2.pdf").is_file());
        let cdr = entries
            .iter()
            .find(|e| e["input"].as_str().unwrap().ends_with("fdo65220-2.cdr"))
            .unwrap();
        assert_eq!(cdr["ok"], true);
        assert_eq!(cdr["report"]["sourceFormat"], "cdr");
    }
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert!(
        stdout.contains(&format!("{} convertido(s)", if with_cdr { 3 } else { 2 })),
        "{stdout}"
    );
}

#[test]
fn gd_exp_06_cdr_to_svg_pdf_png() {
    if !bridge_available() {
        eprintln!("AVISO: gd-cdr-bridge ausente; teste pulado");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = fixtures().join("cdr/fdo63782-1.cmx");
    for ext in ["svg", "pdf", "png", "gdr"] {
        let out = dir.path().join(format!("c.{ext}"));
        let o = run(&[
            os("convert"),
            input.as_os_str(),
            out.as_os_str(),
            os("--dpi"),
            os("96"),
        ]);
        assert!(
            o.status.success(),
            "{ext}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(std::fs::metadata(&out).unwrap().len() > 500);
    }
    let o = run(&[os("info"), input.as_os_str()]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("116 objeto(s)"));
}
