//! `goldstone` command-line tool.

// On Windows the program is a windowed (GUI) program, so opening the editor
// shows no console window; run from a terminal, it attaches to that
// terminal for the command-line output (see `attach_console`).
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use goldstone_core::expr::Rule;
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
    /// .gold or Goldstone .svg files: open them in the editor, one tab each.
    files: Vec<PathBuf>,
    /// With a .tex file: read the equations with the IMSRG rule (default: CC).
    #[arg(long)]
    srg: bool,
    /// With a .tex file: format of the drawn equations (pdf, svg, png, tikz, gold).
    #[arg(short, long, default_value = "pdf")]
    format: String,
    /// With a .tex file: output directory (default: FILE-diagrams next to it).
    #[arg(long)]
    out_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Open the editor, optionally with .gold files (or SVGs made by Goldstone), one tab each.
    Edit { files: Vec<PathBuf> },
    /// Print the keyboard and mouse reference (Markdown, as in docs/MANUAL.md).
    Keys,
    /// Print the licenses of the third-party software and fonts in this program.
    Licenses,
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
        /// Read the equations with the IMSRG rule (default: CC).
        #[arg(long)]
        srg: bool,
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
    /// Draw the diagram of a tensor expression written in LaTeX with the
    /// CC rule, e.g. "E2 = \sum_{abij} V_{ijab} T_{abij}".
    ///
    /// An operator is X^{label}_{pqrs} = <pq|X|rs>; an index is a or a_1;
    /// a–g are particles, h–n holes, other letters general. Without
    /// --output the .gold text is printed; with --output ending in .gold it
    /// is saved, any other extension (.pdf .svg .png .tikz .tex) renders it.
    #[command(name = "cc-latex", alias = "latex")]
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
    /// Draw the diagram of an IMSRG expression written in LaTeX, e.g.
    /// "\frac{dE}{ds} = \sum_{ab} (n_a - n_b) \eta_{ab} f_{ba}".
    ///
    /// Every index is a general orbital unless an occupation factor says
    /// otherwise (n_a hole, \bar n_a or (1 - n_a) particle); indices may be
    /// digits (f_{12}); permutations (1+P_{12}) are left out. Output as for
    /// cc-latex.
    #[command(name = "srg-latex")]
    SrgLatex {
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
    /// List the built-in templates, render them all on one sheet
    /// (`-o sheet.pdf`), or write one (`--name "5-Point Star" -o star.gold`).
    Templates {
        /// The template to write (as listed).
        #[arg(long)]
        name: Option<String>,
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Background color (e.g. white); default transparent.
        #[arg(long)]
        background: Option<String>,
    },
    /// Plot functions of x with axes, e.g. "sin(x); cos(x)" or
    /// "\\frac{1}{1+x^2}" (plain or LaTeX notation; several separated by ;).
    ///
    /// The plot is ordinary objects in one group: open the .gold file in the
    /// editor to restyle it. Output as for `latex`.
    Plot {
        functions: String,
        /// x range FROM:TO; numbers may use pi, e.g. -2pi:2pi.
        #[arg(long, default_value = "-5:5", allow_hyphen_values = true)]
        x: String,
        /// y range FROM:TO (default: from the values).
        #[arg(long, allow_hyphen_values = true)]
        y: Option<String>,
        /// Size of the plot area in pt, WIDTHxHEIGHT.
        #[arg(long, default_value = "200x130")]
        size: String,
        /// Write each function next to its curve (default: with several functions).
        #[arg(long)]
        labels: Option<bool>,
        /// Label of the x axis (LaTeX).
        #[arg(long, default_value = "$x$")]
        x_label: String,
        /// Label of the y axis (LaTeX).
        #[arg(long, default_value = "$y$")]
        y_label: String,
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

/// Prints to stdout; a closed pipe (`goldstone licenses | head`) is not an error.
fn out(text: &str) {
    use std::io::Write as _;
    let _ = std::io::stdout().write_all(text.as_bytes());
}

fn parse_background(s: Option<&str>) -> Result<Option<goldstone_core::color::Color>, Vec<String>> {
    s.map(|c| {
        goldstone_core::color::Color::parse(c).map_err(|e| vec![format!("--background: {e}")])
    })
    .transpose()
}

/// Prints the `.gold` text, saves it (`.gold`), or renders the document.
fn write_doc(
    doc: &goldstone_core::model::Document,
    text: &str,
    output: Option<PathBuf>,
    dpi: f32,
    bg: Option<goldstone_core::color::Color>,
) -> Result<(), Vec<String>> {
    match output {
        None => print!("{text}"),
        Some(out) if out.extension().is_some_and(|e| e == "gold") => {
            std::fs::write(&out, text).map_err(|e| vec![format!("{}: {e}", out.display())])?;
            eprintln!("→ {}", out.display());
        }
        Some(out) => {
            let format = out
                .extension()
                .and_then(|e| Format::from_extension(&e.to_string_lossy()))
                .ok_or_else(|| vec![format!("{}: unknown output format", out.display())])?;
            let mut sc = scene::build(doc).map_err(|e| vec![e.to_string()])?;
            if bg.is_some() {
                sc.background = bg;
            }
            let opts = Options {
                source: Some(text),
                source_name: None,
                dpi,
            };
            let bytes =
                export(&sc, format, &mut Labels::new(), &opts).map_err(|e| vec![e.to_string()])?;
            std::fs::write(&out, bytes).map_err(|e| vec![format!("{}: {e}", out.display())])?;
            eprintln!("→ {}", out.display());
        }
    }
    Ok(())
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
        let is_tex = |f: &PathBuf| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("tex"));
        if cli.files.iter().any(is_tex) {
            for f in cli.files.iter().filter(|f| !is_tex(f)) {
                errors.push(format!(
                    "{}: only .tex files can be given together",
                    f.display()
                ));
            }
            if !errors.is_empty() {
                return Err(errors);
            }
            for f in &cli.files {
                let rule = if cli.srg { Rule::Srg } else { Rule::Cc };
                tex_batch(f, rule, cli.out_dir.clone(), &cli.format, 300.0)?;
            }
            return Ok(());
        }
        return goldstone_app::run(cli.files)
            .map_err(|e| vec![format!("cannot open the editor: {e}")]);
    };
    match command {
        Command::Edit { files } => {
            return goldstone_app::run(files)
                .map_err(|e| vec![format!("cannot open the editor: {e}")]);
        }
        Command::Keys => out(&goldstone_app::keys::manual_markdown()),
        Command::Licenses => out(goldstone_app::THIRD_PARTY_LICENSES),
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
        } => latex_command(&expression, Rule::Cc, output, dpi, background)?,
        Command::SrgLatex {
            expression,
            output,
            dpi,
            background,
        } => latex_command(&expression, Rule::Srg, output, dpi, background)?,
        Command::Templates {
            name,
            output,
            background,
        } => {
            let bg = parse_background(background.as_deref())?;
            let all = goldstone_core::templates::builtin();
            match (name, output) {
                (None, None) => {
                    let mut text = String::new();
                    for cat in goldstone_core::templates::CATEGORIES {
                        text.push_str(&format!("{cat}\n"));
                        for t in all.iter().filter(|t| t.category == *cat) {
                            text.push_str(&format!("  {}\n", t.name));
                        }
                    }
                    out(&text);
                }
                (None, Some(o)) => {
                    let doc = goldstone_core::templates::sheet(&all);
                    let text = io::to_toml(&doc);
                    write_doc(&doc, &text, Some(o), 150.0, bg)?;
                }
                (Some(n), o) => {
                    let t = all
                        .iter()
                        .find(|t| t.name.eq_ignore_ascii_case(&n))
                        .ok_or_else(|| {
                            vec![format!(
                                "no template {n:?}; `goldstone templates` lists them"
                            )]
                        })?;
                    let text = io::to_toml(&t.doc);
                    write_doc(&t.doc, &text, o, 300.0, bg)?;
                }
            }
        }
        Command::Plot {
            functions,
            x,
            y,
            size,
            labels,
            x_label,
            y_label,
            output,
            dpi,
            background,
        } => {
            let bg = parse_background(background.as_deref())?;
            let range = |s: &str, what: &str| -> Result<[String; 2], Vec<String>> {
                let (a, b) = s
                    .split_once(':')
                    .ok_or_else(|| vec![format!("--{what}: write it as FROM:TO, e.g. -2pi:2pi")])?;
                Ok([a.trim().to_string(), b.trim().to_string()])
            };
            let (w, h) = size
                .split_once(['x', 'X'])
                .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)))
                .ok_or_else(|| {
                    vec!["--size: write it as WIDTHxHEIGHT in pt, e.g. 200x130".to_string()]
                })?;
            let source = goldstone_core::model::PlotSource {
                functions: functions.clone(),
                x: range(&x, "x")?,
                y: y.as_deref().map(|y| range(y, "y")).transpose()?,
                size: [w, h],
                labels,
                x_label: Some(x_label),
                y_label: Some(y_label),
            };
            let p = goldstone_core::plot::from_source(&source).map_err(|e| vec![e])?;
            for w in &p.warnings {
                eprintln!("warning: {w}");
            }
            let text = format!(
                "# Generated by: goldstone plot {functions:?} --x {x:?}\n{}",
                io::to_toml(&p.doc)
            );
            write_doc(&p.doc, &text, output, dpi, bg)?;
        }
        Command::Tex {
            input,
            srg,
            out_dir,
            format,
            dpi,
        } => {
            let rule = if srg { Rule::Srg } else { Rule::Cc };
            return tex_batch(&input, rule, out_dir, &format, dpi);
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

/// Draws every equation of a LaTeX document into a directory.
/// `cc-latex` / `srg-latex`: the diagram of one expression.
fn latex_command(
    expression: &str,
    rule: Rule,
    output: Option<PathBuf>,
    dpi: f32,
    background: Option<String>,
) -> Result<(), Vec<String>> {
    let bg = parse_background(background.as_deref())?;
    let drawn =
        goldstone_core::expr::from_latex_editable_with(expression, rule).map_err(|e| vec![e])?;
    for w in &drawn.warnings {
        eprintln!("warning: {w}");
    }
    let cmd = if rule == Rule::Srg {
        "srg-latex"
    } else {
        "cc-latex"
    };
    let text = format!(
        "# Generated by: goldstone {cmd} {expression:?}\n{}",
        io::to_toml(&drawn.doc)
    );
    write_doc(&drawn.doc, &text, output, dpi, bg)
}

fn tex_batch(
    input: &Path,
    rule: Rule,
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
        let d = match expr::from_latex_with(&eq.source, rule) {
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

/// Windows: when started from a terminal, write to it (a windowed program
/// has no console of its own). Double-clicked, there is no terminal and
/// nothing happens.
#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // SAFETY: plain Win32 call without pointers; failure (no parent
    // console) is harmless.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn main() -> ExitCode {
    #[cfg(windows)]
    attach_console();
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
