use std::{
    io::{self, Write},
    str,
};

use crate::{config::OutFormat, lint::LintResult};

use ariadne::{
    CharSet, Color, Config as CliConfig, Fmt, Label, LabelAttach, Report as CliReport,
    ReportKind as CliReportKind, Source,
};
use lib::Severity;
use rnix::{TextRange, TextSize};
use vfs::ReadOnlyVfs;

pub trait WriteDiagnostic {
    fn write(
        &mut self,
        report: &LintResult,
        vfs: &ReadOnlyVfs,
        format: OutFormat,
    ) -> io::Result<()>;
}

impl<T> WriteDiagnostic for T
where
    T: Write,
{
    fn write(
        &mut self,
        lint_result: &LintResult,
        vfs: &ReadOnlyVfs,
        format: OutFormat,
    ) -> io::Result<()> {
        match format {
            OutFormat::Json => json::write_json(self, lint_result, vfs),
            OutFormat::StdErr => write_stderr(self, lint_result, vfs),
            OutFormat::Errfmt => write_errfmt(self, lint_result, vfs),
            OutFormat::Agent => write_agent(self, lint_result, vfs),
        }
    }
}

fn write_stderr<T: Write>(
    writer: &mut T,
    lint_result: &LintResult,
    vfs: &ReadOnlyVfs,
) -> io::Result<()> {
    let file_id = lint_result.file_id;
    let src = str::from_utf8(vfs.get(file_id)).unwrap();
    let path = vfs.file_path(file_id);
    let range = |at: TextRange| at.start().into()..at.end().into();
    let src_id = path.to_str().unwrap_or("<unknown>");
    for report in &lint_result.reports {
        let report_range = report
            .total_diagnostic_range()
            .map(range)
            .unwrap_or(0usize..0usize);
        let report_kind = match report.severity {
            Severity::Warn => CliReportKind::Warning,
            Severity::Error => CliReportKind::Error,
            Severity::Hint => CliReportKind::Advice,
        };
        report
            .diagnostics
            .iter()
            .fold(
                CliReport::build(report_kind, (src_id, report_range))
                    .with_config(
                        CliConfig::default()
                            .with_cross_gap(true)
                            .with_multiline_arrows(false)
                            .with_label_attach(LabelAttach::Middle)
                            .with_char_set(CharSet::Unicode),
                    )
                    .with_message(report.note)
                    .with_code(report.name),
                |cli_report, diagnostic| {
                    cli_report.with_label(
                        Label::new((src_id, range(diagnostic.at)))
                            .with_message(colorize(&diagnostic.message))
                            .with_color(Color::Magenta),
                    )
                },
            )
            .finish()
            .write((src_id, Source::from(src)), &mut *writer)?;
    }
    Ok(())
}

fn write_errfmt<T: Write>(
    writer: &mut T,
    lint_result: &LintResult,
    vfs: &ReadOnlyVfs,
) -> io::Result<()> {
    let file_id = lint_result.file_id;
    let src = str::from_utf8(vfs.get(file_id)).unwrap();
    let path = vfs.file_path(file_id);
    for report in &lint_result.reports {
        for diagnostic in &report.diagnostics {
            let line = line(diagnostic.at.start(), src);
            let col = column(diagnostic.at.start(), src);
            writeln!(
                writer,
                "{filename}>{linenumber}:{columnnumber}:{errortype}:{errornumber}:[{errorname}] {errormessage}",
                filename = path.to_str().unwrap_or("<unknown>"),
                linenumber = line,
                columnnumber = col,
                errortype = match report.severity {
                    Severity::Warn => "W",
                    Severity::Error => "E",
                    Severity::Hint => "I", /* "info" message */
                },
                errornumber = report.code,
                errorname = report.name,
                errormessage = diagnostic.message
            )?;
        }
    }
    Ok(())
}

/// Markdown task list for coding agents: what `statix fix` handles, and for
/// everything else where it is, the surrounding source and how to fix it.
fn write_agent<T: Write>(
    writer: &mut T,
    lint_result: &LintResult,
    vfs: &ReadOnlyVfs,
) -> io::Result<()> {
    let file_id = lint_result.file_id;
    let src = str::from_utf8(vfs.get(file_id)).unwrap();
    let path = vfs.file_path(file_id).to_str().unwrap_or("<unknown>");
    let lines: Vec<&str> = src.lines().collect();

    let mut manual = Vec::new();
    let mut fixable = Vec::new();
    for report in &lint_result.reports {
        for d in &report.diagnostics {
            let entry = (
                line(d.at.start(), src),
                column(d.at.start(), src),
                report,
                d,
            );
            if d.is_fixable() {
                fixable.push(entry);
            } else {
                manual.push(entry);
            }
        }
    }
    if manual.is_empty() && fixable.is_empty() {
        return Ok(());
    }
    manual.sort_by_key(|e| (e.0, e.1));
    fixable.sort_by_key(|e| (e.0, e.1));

    writeln!(writer, "# statix: `{path}`\n")?;
    if !fixable.is_empty() {
        writeln!(
            writer,
            "Run `statix fix {path}` first, it fixes these automatically (and may fix more after re-running lints):\n"
        )?;
        for (l, c, report, d) in &fixable {
            writeln!(writer, "- {path}:{l}:{c} [{}] {}", report.name, d.message)?;
        }
        writeln!(writer)?;
    }
    if manual.is_empty() {
        return Ok(());
    }
    writeln!(writer, "Fix by editing the file:\n")?;
    if manual
        .iter()
        .any(|(_, _, report, _)| ["shellcheck", "ruff"].contains(&report.name))
    {
        writeln!(writer, "{}\n", lib::NIX_ESCAPING)?;
    }
    writeln!(
        writer,
        "Change only what each finding points at and keep what the code does the same. \
Afterwards re-run `statix fix {path}` and `statix check -o agent {path}` until nothing is left.\n"
    )?;
    for (i, (l, c, report, d)) in manual.iter().enumerate() {
        let severity = match report.severity {
            Severity::Error => "error",
            Severity::Warn => "warning",
            Severity::Hint => "hint",
        };
        writeln!(
            writer,
            "## {}. {path}:{l}:{c} [{}] {}\n",
            i + 1,
            report.name,
            d.message
        )?;
        writeln!(writer, "Severity: {severity}\n")?;
        match (&d.help, lint_docs(report.code)) {
            (Some(help), _) => writeln!(writer, "How to fix: {help}\n")?,
            (None, Some(docs)) => writeln!(writer, "{docs}\n")?,
            (None, None) => writeln!(writer, "How to fix: {}.\n", report.note)?,
        }
        match &d.external {
            Some(e) => {
                let text = std::fs::read_to_string(&e.path).unwrap_or_default();
                let ext_lines: Vec<&str> = text.lines().collect();
                writeln!(
                    writer,
                    "In `{}:{}:{}` (referenced from {path}:{l}):\n",
                    e.path.display(),
                    e.line,
                    e.column
                )?;
                write_context(writer, "", &ext_lines, e.line)?;
            }
            None => write_context(writer, "nix", &lines, *l)?,
        }
    }
    Ok(())
}

fn write_context<T: Write>(
    writer: &mut T,
    lang: &str,
    lines: &[&str],
    line: usize,
) -> io::Result<()> {
    const CONTEXT: usize = 2;
    writeln!(writer, "```{lang}")?;
    let first = line.saturating_sub(CONTEXT).max(1);
    let last = (line + CONTEXT).min(lines.len());
    for n in first..=last {
        let marker = if n == line { ">" } else { " " };
        writeln!(writer, "{marker}{n:>5} | {}", lines[n - 1])?;
    }
    writeln!(writer, "```\n")
}

/// The "Why is this bad?" and "Example" parts of a lint's documentation.
fn lint_docs(code: u32) -> Option<String> {
    let docs = crate::utils::lint_map()
        .values()
        .flatten()
        .find(|l| l.code() == code)?
        .explanation();
    let why = docs.find("## Why is this bad?")?;
    Some(docs[why..].replace("## ", "### ").trim().to_string())
}

mod json {
    use crate::lint::LintResult;

    use std::io::{self, Write};

    use lib::Severity;
    use rnix::TextRange;
    use serde::Serialize;
    use vfs::ReadOnlyVfs;

    #[derive(Serialize)]
    struct Out<'μ> {
        #[serde(rename = "file")]
        path: &'μ std::path::Path,
        report: Vec<JsonReport<'μ>>,
    }

    #[derive(Serialize)]
    struct JsonReport<'μ> {
        name: &'static str,
        note: &'static str,
        code: u32,
        severity: &'μ Severity,
        diagnostics: Vec<JsonDiagnostic<'μ>>,
    }

    #[derive(Serialize)]
    struct JsonDiagnostic<'μ> {
        at: JsonSpan,
        message: &'μ String,
        suggestion: Option<JsonSuggestion>,
        fixable: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        help: Option<&'μ String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        external: Option<JsonExternal<'μ>>,
    }

    /// The finding is in another file (a referenced script).
    #[derive(Serialize)]
    struct JsonExternal<'μ> {
        file: &'μ std::path::Path,
        line: usize,
        column: usize,
    }

    #[derive(Serialize)]
    struct JsonSuggestion {
        at: JsonSpan,
        fix: String,
    }

    #[derive(Serialize)]
    struct JsonSpan {
        from: Position,
        to: Position,
    }

    #[derive(Serialize)]
    struct Position {
        line: usize,
        column: usize,
    }

    impl JsonSpan {
        fn from_textrange(at: TextRange, src: &str) -> Self {
            let start = at.start();
            let end = at.end();
            let from = Position {
                line: super::line(start, src),
                column: super::column(start, src),
            };
            let to = Position {
                line: super::line(end, src),
                column: super::column(end, src),
            };
            Self { from, to }
        }
    }

    pub fn write_json<T: Write>(
        writer: &mut T,
        lint_result: &LintResult,
        vfs: &ReadOnlyVfs,
    ) -> io::Result<()> {
        let file_id = lint_result.file_id;
        let path = vfs.file_path(file_id);
        let src = vfs.get_str(file_id);
        let report = lint_result
            .reports
            .iter()
            .map(|r| {
                let name = r.name;
                let note = r.note;
                let code = r.code;
                let severity = &r.severity;
                let diagnostics = r
                    .diagnostics
                    .iter()
                    .map(|d| JsonDiagnostic {
                        at: JsonSpan::from_textrange(d.at, src),
                        message: &d.message,
                        suggestion: d.suggestion.as_ref().map(|s| JsonSuggestion {
                            at: JsonSpan::from_textrange(s.at, src),
                            fix: s.fix.to_string(),
                        }),
                        fixable: d.is_fixable(),
                        help: d.help.as_ref(),
                        external: d.external.as_ref().map(|e| JsonExternal {
                            file: &e.path,
                            line: e.line,
                            column: e.column,
                        }),
                    })
                    .collect::<Vec<_>>();
                JsonReport {
                    name,
                    note,
                    code,
                    severity,
                    diagnostics,
                }
            })
            .collect();
        writeln!(
            writer,
            "{}",
            serde_json::to_string_pretty(&Out { path, report }).unwrap()
        )?;
        Ok(())
    }
}

fn line(at: TextSize, src: &str) -> usize {
    let at = at.into();
    src[..at].chars().filter(|&c| c == '\n').count() + 1
}

fn column(at: TextSize, src: &str) -> usize {
    let at = at.into();
    src[..at].rfind('\n').map_or_else(|| at + 1, |c| at - c)
}

// everything within backticks is colorized, backticks are removed
fn colorize(message: &str) -> String {
    message
        .split('`')
        .enumerate()
        .map(|(idx, part)| {
            if idx % 2 == 1 {
                part.fg(Color::Cyan).to_string()
            } else {
                part.to_string()
            }
        })
        .collect::<String>()
}
