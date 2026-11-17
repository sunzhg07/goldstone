//! `goldstone` command-line tool.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use goldstone_core::{io, scene};
use goldstone_export::{Format, Labels, Options, export, svg};

#[derive(Parser)]
#[command(
    name = "goldstone",
    version,
    about = "Draw many-body (coupled-cluster, IMSRG, MBPT) diagrams"
)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    /// Without a command, opens the editor.
    #[command(subcommand)]
    command: Option<Command>,
    /// A .tex file: draw every equation in it (as `goldstone tex FILE`).
    /// A .gold or Goldstone .svg file: open it in the editor.
    file: Option<PathBuf>,
    /// With a .tex file: format of the drawn equations (pdf, svg, png, tikz, gold).
    #[arg(short, long, default_value = "pdf")]
    format: String,
    /// With a .tex file: output directory (default: FILE-diagrams next to it).
    #[arg(long)]
    out_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Open the editor, optionally with a .gold file (or an SVG made by Goldstone).
    Edit { file: Option<PathBuf> },
    /// Print the keyboard and mouse reference (Markdown, as in docs/MANUAL.md).
    Keys,
    /// Render diagrams to SVG, PDF, PNG, or TikZ (.tex standalone / .tikz picture).
    ///
    /// Inputs are .gold files, or SVG files written by Goldstone (their
    /// embedded source is used).
    Render {
        /// Input files.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Output file (single input only). The format follows the extension.
        #[arg(short, long, conflicts_with = "out_dir")]
        output: Option<PathBuf>,
        /// Output format: svg, pdf, png, tex, tikz. Default: from --output, else pdf.
        #[arg(short, long, value_parser = parse_format)]
        format: Option<Format>,
        /// Directory for outputs (default: next to each input).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Resolution for PNG output.
        #[arg(long, default_value_t = 300.0)]
        dpi: f32,
        /// Background color (e.g. white); default transparent, or the file's own.
        #[arg(long)]
        background: Option<String>,
    },
    /// Validate diagrams, including typesetting of all labels.
    Check {
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
    },
    /// Print a diagram in canonical form (fixed order, defaults omitted).
    /// Note: comments are not preserved.
    Fmt { input: PathBuf },
    /// Print the .gold source embedded in an SVG written by Goldstone.
    Extract { input: PathBuf },
    /// Draw every equation of a LaTeX document.
    ///
    /// Equations come from equation, align, eqnarray, multline, gather, dmath,
    /// \[ \] and $$ $$; an align with several equations gives one diagram
    /// each. Writes one file per equation (eq0003-L37.pdf: equation 3, line
    /// 37), overview pages (overview-01.pdf, 20 equations each), all.gold
    /// (everything, editable) and report.txt.
    Tex {
        input: PathBuf,
        /// Output directory (default: INPUT-diagrams next to the input).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Format of the per-equation files: pdf, svg, png, tikz or gold.
        #[arg(short, long, default_value = "pdf")]
        format: String,
        /// Resolution for PNG output.
        #[arg(long, default_value_t = 300.0)]
        dpi: f32,
    },
    /// List every object with its id and absolute coordinates (pt, y up),
    /// as the editor shows them with View ▸ Object Ids (key I).
    Ls { input: PathBuf },
    /// Draw the diagram of a tensor expression written in LaTeX, e.g.
    /// "E2 = \sum_{abij} V_{ijab} T_{abij}" or "Z_{abic} = V_{abef} S_{efic}".
    ///
    /// X_{pqrs} = <pq|X|rs>; a–h are particles, i–n holes. Without
    /// --output the .gold text is printed; with --output ending in .gold it
    /// is saved, any other extension (.pdf .svg .png .tikz .tex) renders it.
    Latex {
        expression: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Resolution for PNG output.
        #[arg(long, default_value_t = 300.0)]
        dpi: f32,
        /// Background color (e.g. white); default transparent.
        #[arg(long)]
        background: Option<String>,
    },
}

fn parse_background(s: Option<&str>) -> Result<Option<goldstone_core::color::Color>, Vec<String>> {
    s.map(|c| {
        goldstone_core::color::Color::parse(c).map_err(|e| vec![format!("--background: {e}")])
    })
    .transpose()
}

fn parse_format(s: &str) -> Result<Format, String> {
    Format::from_extension(s)
        .ok_or_else(|| format!("unknown format {s:?}; use svg, pdf, png, tex or tikz"))
}

/// Reads a `.gold` file, or the source embedded in a Goldstone SVG.
fn read_source(path: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
    {
        svg::extract_source(&text).ok_or_else(|| {
            format!(
                "{}: no Goldstone source embedded in this SVG",
                path.display()
            )
        })
    } else {
        Ok(text)
    }
}

fn load(path: &Path) -> Result<(String, scene::Scene), String> {
    let src = read_source(path)?;
    let prefix = |e: &dyn std::fmt::Display| {
        // One "file: error: …" line per diagnostic; continuation lines
        // (TOML source excerpts) are kept as they are.
        e.to_string()
            .lines()
            .map(|l| {
                if l.starts_with("error:") {
                    format!("{}: {l}", path.display())
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let doc = io::parse(&src).map_err(|e| prefix(&e))?;
    let sc = scene::build(&doc).map_err(|e| prefix(&e))?;
    Ok((src, sc))
}

fn run(cli: Cli) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let Some(command) = cli.command else {
        if let Some(file) = cli.file {
            if file
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("tex"))
            {
                return tex_batch(&file, cli.out_dir, &cli.format, 300.0);
            }
            return goldstone_app::run(Some(file))
                .map_err(|e| vec![format!("cannot open the editor: {e}")]);
        }
        return goldstone_app::run(None).map_err(|e| vec![format!("cannot open the editor: {e}")]);
    };
    match command {
        Command::Edit { file } => {
            return goldstone_app::run(file)
                .map_err(|e| vec![format!("cannot open the editor: {e}")]);
        }
        Command::Keys => print!("{}", goldstone_app::keys::manual_markdown()),
        Command::Render {
            inputs,
            output,
            format,
            out_dir,
            dpi,
            background,
        } => {
            let bg = parse_background(background.as_deref())?;
            if output.is_some() && inputs.len() > 1 {
                return Err(vec![
                    "--output works with a single input; use --out-dir for several".into(),
                ]);
            }
            let format = format
                .or_else(|| {
                    output
                        .as_ref()
                        .and_then(|o| o.extension())
                        .and_then(|e| Format::from_extension(&e.to_string_lossy()))
                })
                .unwrap_or(Format::Pdf);
            let mut labels = Labels::new();
            for input in &inputs {
                let (src, mut sc) = match load(input) {
                    Ok(x) => x,
                    Err(e) => {
                        errors.push(e);
                        continue;
                    }
                };
                let out = output.clone().unwrap_or_else(|| {
                    let name = input.with_extension(format.extension());
                    match &out_dir {
                        Some(dir) => dir.join(name.file_name().expect("input has a file name")),
                        None => name,
                    }
                });
                if out == *input {
                    errors.push(format!(
                        "{}: refusing to overwrite the input file",
                        input.display()
                    ));
                    continue;
                }
                let name = input.file_name().map(|n| n.to_string_lossy().into_owned());
                let opts = Options {
                    source: Some(&src),
                    source_name: name.as_deref(),
                    dpi,
                };
                if bg.is_some() {
                    sc.background = bg;
                }
                match export(&sc, format, &mut labels, &opts) {
                    Ok(bytes) => {
                        if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty())
                            && let Err(e) = std::fs::create_dir_all(dir)
                        {
                            errors.push(format!("{}: {e}", dir.display()));
                            continue;
                        }
                        match std::fs::write(&out, bytes) {
                            Ok(()) => eprintln!("{} → {}", input.display(), out.display()),
                            Err(e) => errors.push(format!("{}: {e}", out.display())),
                        }
                    }
                    Err(e) => errors.push(format!("{}: {e}", input.display())),
                }
            }
        }
        Command::Check { inputs } => {
            let mut labels = Labels::new();
            for input in &inputs {
                match load(input) {
                    Ok((_, sc)) => match export(&sc, Format::Svg, &mut labels, &Options::default())
                    {
                        Ok(_) => eprintln!("{}: ok", input.display()),
                        Err(e) => errors.push(format!("{}: {e}", input.display())),
                    },
                    Err(e) => errors.push(e),
                }
            }
        }
        Command::Latex {
            expression,
            output,
            dpi,
            background,
        } => {
            let bg = parse_background(background.as_deref())?;
            let drawn = goldstone_core::expr::from_latex(&expression).map_err(|e| vec![e])?;
            for w in &drawn.warnings {
                eprintln!("warning: {w}");
            }
            let doc = drawn.doc;
            let text = format!(
                "# Generated by: goldstone latex {expression:?}\n{}",
                io::to_toml(&doc)
            );
            match output {
                None => print!("{text}"),
                Some(out) if out.extension().is_some_and(|e| e == "gold") => {
                    std::fs::write(&out, &text)
                        .map_err(|e| vec![format!("{}: {e}", out.display())])?;
                    eprintln!("→ {}", out.display());
                }
                Some(out) => {
                    let format = out
                        .extension()
                        .and_then(|e| Format::from_extension(&e.to_string_lossy()))
                        .ok_or_else(|| vec![format!("{}: unknown output format", out.display())])?;
                    let mut sc = scene::build(&doc).map_err(|e| vec![e.to_string()])?;
                    if bg.is_some() {
                        sc.background = bg;
                    }
                    let opts = Options {
                        source: Some(&text),
                        source_name: None,
                        dpi,
                    };
                    let bytes = export(&sc, format, &mut Labels::new(), &opts)
                        .map_err(|e| vec![e.to_string()])?;
                    std::fs::write(&out, bytes)
                        .map_err(|e| vec![format!("{}: {e}", out.display())])?;
                    eprintln!("→ {}", out.display());
                }
            }
        }
        Command::Tex {
            input,
            out_dir,
            format,
            dpi,
        } => return tex_batch(&input, out_dir, &format, dpi),
        Command::Ls { input } => {
            let src = read_source(&input).map_err(|e| vec![e])?;
            let doc = io::parse(&src).map_err(|e| vec![format!("{}: {e}", input.display())])?;
            print!("{}", goldstone_core::inspect::list(&doc));
        }
        Command::Fmt { input } => {
            let src = read_source(&input).map_err(|e| vec![e])?;
            let doc = io::parse(&src).map_err(|e| vec![format!("{}: {e}", input.display())])?;
            print!("{}", io::to_toml(&doc));
        }
        Command::Extract { input } => {
            let text = std::fs::read_to_string(&input)
                .map_err(|e| vec![format!("{}: {e}", input.display())])?;
            match svg::extract_source(&text) {
                Some(src) => print!("{src}"),
                None => errors.push(format!(
                    "{}: no Goldstone source embedded in this SVG",
                    input.display()
                )),
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Draws every equation of a LaTeX document into a directory.
fn tex_batch(
    input: &Path,
    out_dir: Option<PathBuf>,
    format: &str,
    dpi: f32,
) -> Result<(), Vec<String>> {
    use goldstone_core::{expr, tex};
    let source =
        std::fs::read_to_string(input).map_err(|e| vec![format!("{}: {e}", input.display())])?;
    let format = format.trim_start_matches('.').to_ascii_lowercase();
    let fmt = match format.as_str() {
        "gold" => None,
        f => Some(Format::from_extension(f).ok_or_else(|| {
            vec![format!(
                "unknown format {f:?}; use pdf, svg, png, tikz or gold"
            )]
        })?),
    };
    let stem = input
        .file_stem()
        .map_or("equations".into(), |s| s.to_string_lossy().into_owned());
    let dir = out_dir.unwrap_or_else(|| input.with_file_name(format!("{stem}-diagrams")));
    std::fs::create_dir_all(&dir).map_err(|e| vec![format!("{}: {e}", dir.display())])?;
    let eqs = tex::equations(&source);
    if eqs.is_empty() {
        return Err(vec![format!("{}: no equations found", input.display())]);
    }
    let mut labels = Labels::new();
    let mut report = format!("Equations of {} ({} found)\n\n", input.display(), eqs.len());
    let mut drawn: Vec<(String, goldstone_core::model::Document)> = Vec::new();
    let (mut ok, mut partly, mut skipped) = (0, 0, 0);
    for (n, eq) in eqs.iter().enumerate() {
        let num = n + 1;
        let name = format!("eq{num:04}-L{}", eq.line);
        let one_line: String = eq.source.split_whitespace().collect::<Vec<_>>().join(" ");
        let short: String = one_line.chars().take(90).collect();
        let d = match expr::from_latex(&eq.source) {
            Ok(d) => d,
            Err(e) => {
                skipped += 1;
                let e = e.lines().next().unwrap_or_default().to_string();
                report.push_str(&format!(
                    "({num}) line {}: not drawn: {e}\n    {short}\n",
                    eq.line
                ));
                continue;
            }
        };
        let text = format!(
            "# Equation ({num}), line {} of {}:\n# {one_line}\n{}",
            eq.line,
            input.display(),
            io::to_toml(&d.doc)
        );
        let file = dir.join(format!("{name}.{}", fmt.map_or("gold", |f| f.extension())));
        let written = match fmt {
            None => std::fs::write(&file, &text).map_err(|e| e.to_string()),
            Some(f) => scene::build(&d.doc)
                .map_err(|e| e.to_string())
                .and_then(|sc| {
                    let opts = Options {
                        source: Some(&text),
                        source_name: None,
                        dpi,
                    };
                    export(&sc, f, &mut labels, &opts).map_err(|e| e.to_string())
                })
                .and_then(|bytes| std::fs::write(&file, bytes).map_err(|e| e.to_string())),
        };
        if let Err(e) = written {
            skipped += 1;
            report.push_str(&format!(
                "({num}) line {}: not drawn: {e}\n    {short}\n",
                eq.line
            ));
            continue;
        }
        let fname = file
            .file_name()
            .map_or(String::new(), |f| f.to_string_lossy().into_owned());
        if d.warnings.is_empty() {
            ok += 1;
            report.push_str(&format!("({num}) line {}: {fname}\n", eq.line));
        } else {
            partly += 1;
            report.push_str(&format!(
                "({num}) line {}: {fname}, with terms left out:\n",
                eq.line
            ));
            for w in &d.warnings {
                report.push_str(&format!("    {w}\n"));
            }
        }
        drawn.push((format!("({num}) line {}", eq.line), d.doc));
    }
    // Overview pages and one editable file with everything.
    let mut pages = 0;
    for (k, chunk) in drawn.chunks(20).enumerate() {
        let sheet = tex::stack(chunk.to_vec());
        if let Ok(sc) = scene::build(&sheet)
            && let Ok(bytes) = export(&sc, Format::Pdf, &mut labels, &Options::default())
            && std::fs::write(dir.join(format!("overview-{:02}.pdf", k + 1)), bytes).is_ok()
        {
            pages += 1;
        }
    }
    let all = tex::stack(drawn);
    let _ = std::fs::write(dir.join("all.gold"), io::to_toml(&all));
    let summary = format!(
        "\n{} equations: {ok} drawn, {partly} drawn with terms left out, {skipped} not drawn.\n",
        eqs.len()
    );
    report.push_str(&summary);
    let _ = std::fs::write(dir.join("report.txt"), &report);
    // On the terminal: what was not (fully) drawn, and the totals.
    for line in report
        .lines()
        .filter(|l| l.starts_with('(') && (l.contains("not drawn") || l.contains("left out")))
    {
        eprintln!("{line}");
    }
    eprint!("{summary}");
    eprintln!(
        "Wrote {} into {} ({pages} overview page(s), all.gold, report.txt)",
        ok + partly,
        dir.display()
    );
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(errors) => {
            for e in errors {
                eprintln!("{e}");
            }
            ExitCode::FAILURE
        }
    }
}
