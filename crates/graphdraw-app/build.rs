//! Ícones do programa (1) e do arquivo `.gdr` (2) dentro do `.exe` no
//! Windows, compilados com o `windres` do MinGW (assets/brand). Sem
//! `windres` no PATH, o programa sai sem ícone embutido e o build avisa.

fn main() {
    println!("cargo:rerun-if-changed=assets/brand/app.ico");
    println!("cargo:rerun-if-changed=assets/brand/gdr.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/brand");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let path = |name: &str| dir.join(name).to_string_lossy().replace('\\', "/");
    let rc = out.join("graphdraw.rc");
    std::fs::write(
        &rc,
        format!(
            "1 ICON \"{}\"\n2 ICON \"{}\"\n",
            path("app.ico"),
            path("gdr.ico")
        ),
    )
    .unwrap();
    let obj = out.join("graphdraw-res.o");
    let ok = std::process::Command::new("windres")
        .arg("-i")
        .arg(&rc)
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&obj)
        .status()
        .is_ok_and(|s| s.success());
    if ok {
        println!("cargo:rustc-link-arg-bins={}", obj.display());
    } else {
        println!("cargo:warning=windres não encontrado: o .exe sai sem ícone embutido");
    }
}
