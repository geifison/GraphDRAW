//! `graphdraw` — linha de comando do GraphDraw.
//!
//! `graphdraw convert <entrada> <saida> [--page N] [--dpi N] [--text-to-curves]`
//! (GD-EXP-06). Entrada pode ser uma pasta: converte todos os `.cdr`, `.cmx`
//! e `.gdr` dela para a pasta de saída (GD-CDR-07).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use graphdraw_core::export::{self, ExportFormat, ExportOptions};
use graphdraw_core::report::Severity;
use graphdraw_core::{ImportReport, open_path};

#[derive(Parser)]
#[command(
    name = "graphdraw",
    version,
    about = "GraphDraw — editor de ilustração vetorial (linha de comando)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Converte .gdr/.cdr/.cmx para SVG, PDF, PNG ou .gdr.
    Convert(ConvertArgs),
    /// Mostra páginas e o relatório de importação de um arquivo.
    Info {
        /// Arquivo a inspecionar.
        input: PathBuf,
    },
    /// Mede a fidelidade de importação sobre um corpus privado (GD-FID-30):
    /// `<corpus>/casos/NNNN/{original.cdr, ref-curvas.pdf, meta.json}`.
    /// Requer o Poppler (pdftoppm, pdftotext) no PATH ou em GRAPHDRAW_POPPLER.
    Fidelity {
        /// Pasta do corpus (nunca dentro do repositório, GD-FID-40).
        corpus: PathBuf,
        /// Resolução da rasterização de comparação.
        #[arg(long, value_name = "N", default_value_t = 150)]
        dpi: u32,
        /// Nome da execução em `resultados/` (padrão: data e hora UTC).
        #[arg(long, value_name = "NOME")]
        run: Option<String>,
    },
}

#[derive(clap::Args)]
struct ConvertArgs {
    /// Arquivo de entrada, ou pasta para conversão em lote.
    input: PathBuf,
    /// Arquivo de saída (formato pela extensão), ou pasta no modo lote.
    output: PathBuf,
    /// Página ou intervalo (base 1): `2`, `1-3`, `1,4-`. Padrão: todas.
    #[arg(long, value_name = "N")]
    page: Option<String>,
    /// Resolução da exportação PNG.
    #[arg(long, value_name = "N", default_value_t = 300.0)]
    dpi: f64,
    /// Converte texto em curvas (SVG). O PDF sempre usa curvas nesta versão.
    #[arg(long)]
    text_to_curves: bool,
    /// Formato de saída (obrigatório no modo lote): svg, pdf, png, gdr.
    #[arg(long, value_name = "FORMATO")]
    format: Option<String>,
    /// Fundo do PNG: `transparent` (padrão), `white` ou `#rrggbb`.
    #[arg(long, value_name = "COR", default_value = "transparent")]
    background: String,
    /// Desliga o anti-serrilhado do PNG.
    #[arg(long)]
    no_antialias: bool,
    /// Grava o relatório de importação em JSON.
    #[arg(long, value_name = "ARQUIVO")]
    report: Option<PathBuf>,
    /// PDF para gráfica (PDF/X-1a:2001). Condição de impressão opcional:
    /// fogra39 (couché, padrão), fogra47 (não couché) ou swop.
    #[arg(long, value_name = "CONDIÇÃO", num_args = 0..=1, default_missing_value = "fogra39")]
    pdfx: Option<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Convert(args) => convert(&args),
        Command::Info { input } => info(&input),
        Command::Fidelity { corpus, dpi, run } => fidelity(&corpus, dpi, run),
    };
    match result {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("erro: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn fidelity(corpus: &Path, dpi: u32, run: Option<String>) -> Result<ExitCode, String> {
    let run = run.unwrap_or_else(utc_stamp);
    let (report, out) = graphdraw_core::fidelity::run(corpus, dpi, &run)?;
    let s = &report.summary;
    for c in &report.cases {
        let levels: Vec<&str> = c.pages.iter().map(|p| p.level.label()).collect();
        let status = match &c.error {
            Some(e) => format!("ERRO: {e}"),
            None => levels.join(", "),
        };
        println!("{}  {status}", c.case);
    }
    println!(
        "{} casos, {} páginas: Fiel {:.1}%, Fiel ou aceitável {:.1}%, travamentos {}{}",
        s.cases,
        s.pages,
        s.fiel_pct,
        s.fiel_ou_aceitavel_pct,
        s.crashes,
        s.trend_fiel_pp
            .map(|t| format!(", tendência {t:+.1} pp"))
            .unwrap_or_default()
    );
    println!("relatório: {}", out.join("relatorio.html").display());
    Ok(if s.crashes > 0 || s.falha > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

/// `AAAA-MM-DD-HHMM` em UTC, sem dependências de data.
fn utc_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Algoritmo de Howard Hinnant (dias desde 1970 → data civil).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}-{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60
    )
}

fn parse_background(s: &str) -> Result<Option<[u8; 4]>, String> {
    match s {
        "transparent" | "transparente" => Ok(None),
        "white" | "branco" => Ok(Some([255, 255, 255, 255])),
        hex => {
            let c = graphdraw_core::model::Color::from_hex(hex)
                .ok_or_else(|| format!("cor de fundo inválida: '{hex}'"))?;
            let [r, g, b] = c.to_srgb();
            Ok(Some([
                (r * 255.0).round() as u8,
                (g * 255.0).round() as u8,
                (b * 255.0).round() as u8,
                255,
            ]))
        }
    }
}

fn format_for(path: &Path, explicit: Option<&str>) -> Result<ExportFormat, String> {
    let name = match explicit {
        Some(f) => f.to_string(),
        None => path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string(),
    };
    ExportFormat::from_extension(&name).ok_or_else(|| {
        format!("formato de saída desconhecido '{name}'; use svg, pdf, png ou gdr (por extensão ou --format)")
    })
}

fn convert(args: &ConvertArgs) -> Result<ExitCode, String> {
    if args.dpi <= 0.0 || args.dpi > 9600.0 {
        return Err("--dpi deve estar entre 1 e 9600".into());
    }
    let background = parse_background(&args.background)?;
    if args.input.is_dir() {
        return convert_batch(args, background);
    }
    let format = format_for(&args.output, args.format.as_deref())?;
    let report = convert_one(&args.input, &args.output, format, args, background)?;
    if let Some(path) = &args.report {
        write_report(path, &[(args.input.clone(), Some(report))])?;
    }
    Ok(ExitCode::SUCCESS)
}

fn convert_one(
    input: &Path,
    output: &Path,
    format: ExportFormat,
    args: &ConvertArgs,
    background: Option<[u8; 4]>,
) -> Result<ImportReport, String> {
    let (doc, report) = open_path(input).map_err(|e| format!("{}: {e}", input.display()))?;
    print_report(input, &report);
    let pages = match &args.page {
        Some(spec) => export::parse_pages(spec, doc.data.pages.len()).map_err(|e| e.to_string())?,
        None => Vec::new(),
    };
    let pdf = match &args.pdfx {
        None => export::PdfStandard::Screen,
        Some(c) => {
            export::PdfStandard::X1a(export::PrintCondition::from_name(c).ok_or_else(|| {
                format!("condição de impressão desconhecida: '{c}' (use fogra39, fogra47 ou swop)")
            })?)
        }
    };
    let opts = ExportOptions {
        pdf,
        pages,
        dpi: args.dpi,
        text_to_curves: args.text_to_curves,
        antialias: !args.no_antialias,
        background,
    };
    let (written, warnings) = export::export_with_report(&doc, output, format, &opts)
        .map_err(|e| format!("{}: {e}", output.display()))?;
    for w in written {
        println!("{} → {}", input.display(), w.display());
    }
    for w in warnings {
        eprintln!("  aviso: {w}");
    }
    Ok(report)
}

fn convert_batch(args: &ConvertArgs, background: Option<[u8; 4]>) -> Result<ExitCode, String> {
    let format = match args.format.as_deref() {
        Some(f) => format_for(Path::new(""), Some(f))?,
        None => return Err("no modo lote, informe --format (svg, pdf, png ou gdr)".into()),
    };
    std::fs::create_dir_all(&args.output).map_err(|e| format!("{}: {e}", args.output.display()))?;
    let mut inputs: Vec<PathBuf> = std::fs::read_dir(&args.input)
        .map_err(|e| format!("{}: {e}", args.input.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                    matches!(e.to_ascii_lowercase().as_str(), "cdr" | "cmx" | "gdr")
                })
        })
        .collect();
    inputs.sort();
    if inputs.is_empty() {
        return Err(format!(
            "nenhum arquivo .cdr/.cmx/.gdr em {}",
            args.input.display()
        ));
    }
    let mut results = Vec::new();
    let mut failures = 0;
    for input in &inputs {
        let stem = input
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let out = args.output.join(format!("{stem}.{}", format.extension()));
        match convert_one(input, &out, format, args, background) {
            Ok(r) => results.push((input.clone(), Some(r))),
            Err(e) => {
                eprintln!("erro: {e}");
                failures += 1;
                results.push((input.clone(), None));
            }
        }
    }
    if let Some(path) = &args.report {
        write_report(path, &results)?;
    }
    println!(
        "{} convertido(s), {failures} com erro",
        inputs.len() - failures
    );
    Ok(if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn print_report(input: &Path, report: &ImportReport) {
    for item in &report.items {
        if item.severity == Severity::Info {
            continue;
        }
        let page = item
            .page
            .map(|p| format!(" [pág. {p}]"))
            .unwrap_or_default();
        let count = if item.count > 1 {
            format!(" (×{})", item.count)
        } else {
            String::new()
        };
        eprintln!("aviso: {}{page}: {}{count}", input.display(), item.message);
    }
}

fn write_report(path: &Path, results: &[(PathBuf, Option<ImportReport>)]) -> Result<(), String> {
    let list: Vec<serde_json::Value> = results
        .iter()
        .map(|(input, r)| {
            serde_json::json!({
                "input": input.display().to_string(),
                "ok": r.is_some(),
                "report": r,
            })
        })
        .collect();
    let text = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn info(input: &Path) -> Result<ExitCode, String> {
    let (doc, report) = open_path(input).map_err(|e| format!("{}: {e}", input.display()))?;
    println!("arquivo: {}", input.display());
    println!("formato: {}", report.source_format);
    if let Some(v) = &report.detected_version {
        println!("versão: {v}");
    }
    for (i, p) in doc.data.pages.iter().enumerate() {
        let mut count = 0;
        for l in &p.layers {
            for o in &l.objects {
                o.walk(&mut |_| count += 1);
            }
        }
        println!(
            "página {}: {:.2} × {:.2} mm, {} camada(s), {count} objeto(s)",
            i + 1,
            p.width / 1000.0,
            p.height / 1000.0,
            p.layers.len()
        );
    }
    if !report.items.is_empty() {
        println!("relatório:");
        print!("{}", report.to_text());
    }
    Ok(ExitCode::SUCCESS)
}
