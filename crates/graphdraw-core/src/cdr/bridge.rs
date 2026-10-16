//! Execução do `gd-cdr-bridge` (processo separado sobre a libcdr).
//! Isolar a libcdr num processo garante que arquivo malformado nunca
//! derrube o GraphDraw (GD-CDR-06).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::Error;

#[cfg(windows)]
const BRIDGE_EXE: &str = "gd-cdr-bridge.exe";
#[cfg(not(windows))]
const BRIDGE_EXE: &str = "gd-cdr-bridge";

/// Tempo máximo de importação de um arquivo.
pub const TIMEOUT: Duration = Duration::from_secs(300);

/// Procura o executável da ponte:
/// 1. variável `GRAPHDRAW_CDR_BRIDGE`;
/// 2. ao lado do executável atual (ou em `cdr-bridge/`);
/// 3. em `../cdr-bridge/` relativo ao executável (layout de `target/` no desenvolvimento);
/// 4. no `PATH`.
pub fn locate() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GRAPHDRAW_CDR_BRIDGE") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(Path::to_path_buf);
        // Sobe alguns níveis (target/debug/deps → target).
        for _ in 0..3 {
            let Some(d) = dir else { break };
            for cand in [d.join(BRIDGE_EXE), d.join("cdr-bridge").join(BRIDGE_EXE)] {
                if cand.is_file() {
                    return Some(cand);
                }
            }
            dir = d.parent().map(Path::to_path_buf);
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(BRIDGE_EXE))
        .find(|p| p.is_file())
}

/// Resultado bruto da ponte.
pub struct BridgeOutput {
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub timed_out: bool,
}

pub fn run(bytes: &[u8]) -> Result<BridgeOutput, Error> {
    let exe = locate().ok_or_else(|| {
        Error::Cdr(
            "componente de importação .cdr (gd-cdr-bridge) não encontrado; reinstale o GraphDraw \
             ou defina GRAPHDRAW_CDR_BRIDGE"
                .into(),
        )
    })?;
    let mut cmd = Command::new(&exe);
    cmd.arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::Cdr(format!("não foi possível iniciar {}: {e}", exe.display())))?;

    let mut stdin = child.stdin.take().unwrap();
    let input = bytes.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&input);
    });
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let mut stderr = child.stderr.take().unwrap();
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });

    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait()? {
            Some(st) => break Some(st),
            None if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                timed_out = true;
                break child.wait().ok();
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    let _ = writer.join();
    let stdout = reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();
    Ok(BridgeOutput {
        exit_code: status.and_then(|s| s.code()),
        stdout,
        stderr,
        timed_out,
    })
}
