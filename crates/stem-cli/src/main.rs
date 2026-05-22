//! `stem` — command-line entry point for the Stem markup language.
//!
//! Subcommands:
//! - `stem parse`        read source from stdin, pretty-print the AST
//! - `stem check`        read stdin, run parser + validator
//! - `stem render`       read stdin, render to HTML/docx/PDF on stdout
//! - `stem registry`     dump the function registry
//!
//! ## Why stdin/stdout only
//!
//! The CLI deliberately does not open files by path. All I/O flows
//! through stdin/stdout, so the canonical invocation is:
//!
//! ```sh
//! stem render --format html < input.stem > output.html
//! stem check < input.stem
//! ```
//!
//! This makes the tool composable with the shell (`|`, `<`, `>`, `tee`,
//! `xargs`) without re-implementing file handling, avoids any
//! path-traversal surface in the binary itself, and keeps the binary's
//! sandbox profile trivial — it never touches the filesystem.

use std::io::{Read, Write};
use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};

use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_exports::HtmlExporter;
use stem_parser::parse;
use stem_types::{default_registry, validate};

#[derive(Parser)]
#[command(name = "stem", version, about = "Stem markup language CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Read a Stem source from stdin and print the AST as Debug.
    Parse,
    /// Read stdin, run parser + validator. Exit non-zero on errors.
    Check,
    /// Read stdin and render to stdout.
    Render {
        /// Output format.
        #[arg(short, long, value_enum, default_value_t = Format::Html)]
        format: Format,
    },
    /// Print the function registry as a human-readable list.
    Registry,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Html,
    Docx,
    Pdf,
}

fn main() -> ExitCode {
    match Cli::parse().cmd {
        Cmd::Parse => run_parse(),
        Cmd::Check => run_check(),
        Cmd::Render { format } => run_render(format),
        Cmd::Registry => run_registry(),
    }
}

fn read_stdin() -> anyhow::Result<String> {
    let mut s = String::new();
    std::io::stdin()
        .read_to_string(&mut s)
        .context("read stdin")?;
    Ok(s)
}

fn run_parse() -> ExitCode {
    let src = match read_stdin() {
        Ok(s) => s,
        Err(e) => return error_exit(e),
    };
    let r = parse(&src);
    println!("{:#?}", r.document);
    for d in &r.diagnostics {
        eprintln!("{}", format_diagnostic(d));
    }
    if r.diagnostics
        .iter()
        .any(|d| d.severity == stem_core::diagnostic::Severity::Error)
    {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn run_check() -> ExitCode {
    let src = match read_stdin() {
        Ok(s) => s,
        Err(e) => return error_exit(e),
    };
    let r = parse(&src);
    let mut diags = r.diagnostics.clone();
    diags.extend(validate(&r.document, &default_registry()));
    let mut errors = 0usize;
    for d in &diags {
        eprintln!("{}", format_diagnostic(d));
        if d.severity == stem_core::diagnostic::Severity::Error {
            errors += 1;
        }
    }
    if errors > 0 {
        eprintln!("{} error(s)", errors);
        ExitCode::from(1)
    } else {
        eprintln!(
            "ok ({} diagnostic{})",
            diags.len(),
            if diags.len() == 1 { "" } else { "s" }
        );
        ExitCode::SUCCESS
    }
}

fn run_render(format: Format) -> ExitCode {
    let src = match read_stdin() {
        Ok(s) => s,
        Err(e) => return error_exit(e),
    };
    let r = parse(&src);
    for d in &r.diagnostics {
        eprintln!("{}", format_diagnostic(d));
    }
    if r.diagnostics
        .iter()
        .any(|d| d.severity == stem_core::diagnostic::Severity::Error)
    {
        eprintln!("refusing to render: source has parse errors");
        return ExitCode::from(1);
    }
    let theme = Theme::default();
    let bytes: Vec<u8> = match format {
        Format::Html => match HtmlExporter::new().export(&r.document, &theme) {
            Ok(s) => s.into_bytes(),
            Err(e) => return error_exit(anyhow::Error::from(e)),
        },
        Format::Docx => match stem_exports::DocxExporter::new().export(&r.document, &theme) {
            Ok(b) => b,
            Err(e) => return error_exit(anyhow::Error::from(e)),
        },
        Format::Pdf => match stem_exports::PdfExporter::new().export(&r.document, &theme) {
            Ok(b) => b,
            Err(e) => return error_exit(anyhow::Error::from(e)),
        },
    };
    if let Err(e) = std::io::stdout().write_all(&bytes) {
        return error_exit(anyhow::Error::from(e));
    }
    ExitCode::SUCCESS
}

fn run_registry() -> ExitCode {
    let r = default_registry();
    for ty in [
        stem_types::DocumentType::Document,
        stem_types::DocumentType::Presentation,
        stem_types::DocumentType::Sheet,
    ] {
        println!("# {}", ty.as_str());
        for name in r.names_for(ty) {
            if let Some(s) = r.get(name, ty) {
                println!("  {:16}  {}", s.name, s.doc);
            }
        }
        println!();
    }
    ExitCode::SUCCESS
}

fn format_diagnostic(d: &stem_core::Diagnostic) -> String {
    let sev = match d.severity {
        stem_core::diagnostic::Severity::Error => "error",
        stem_core::diagnostic::Severity::Warning => "warning",
        stem_core::diagnostic::Severity::Hint => "hint",
    };
    format!(
        "{}: [{}] {} @ L{}:{}",
        sev, d.code, d.message, d.span.start.line, d.span.start.col
    )
}

fn error_exit(e: anyhow::Error) -> ExitCode {
    eprintln!("error: {:#}", e);
    ExitCode::from(1)
}
