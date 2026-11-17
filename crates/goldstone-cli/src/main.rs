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
struct Cli {
    /// Without a command, opens the editor.
    #[command(subcommand)]
    command: Option<Command>,
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
    },
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
        } => {
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
                let (src, sc) = match load(input) {
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
        } => {
            let doc = goldstone_core::expr::from_latex(&expression).map_err(|e| vec![e])?;
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
                    let sc = scene::build(&doc).map_err(|e| vec![e.to_string()])?;
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
