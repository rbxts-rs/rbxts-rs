//! Diagnostics as TypeScript 7.1 (`tsc`, tsgo) prints them: its diagnostic reporter (`CreateDiagnosticReporter`) and
//! error summary (`CreateReportErrorSummary`), through tsrs's writer (`tsrs_compiler::diagnosticwriter`, a port of
//! tsgo's).
//!
//! Pretty output is a code frame per diagnostic, related information as `  file:line:col - message` with its own
//! frame, and a summary after them all ("Found 2 errors in 2 files." and a table of the files); plain output is
//! `file(line,col): error TS2322: message` alone, without related information or a summary. Lines end with `\n` on
//! every OS, as tsgo's do. rbxts-rs: roblox-ts's own diagnostics and plugin diagnostics have no TypeScript code (tsrs
//! gives them -1); the code slot shows what [`Format::code`] gives for them instead (`TS roblox-ts`).

use tsrs_ast::Diagnostic;
use tsrs_compiler::diagnosticwriter::{
    FormattingOptions, format_diagnostic_with_color_and_context, write_error_summary_text, write_format_diagnostic,
};
use tsrs_core::P;
use tsrs_core::tspath::{self, ComparePathsOptions};

/// tsgo's `FormattingOptions.NewLine`, the same on every OS.
pub const NEW_LINE: &str = "\n";

/// How diagnostics are printed.
pub struct Format<'a> {
    options: FormattingOptions,
    /// Pretty (colours, code frames, a summary) or plain.
    pub pretty: bool,
    /// The code slot's text after `TS` for a diagnostic without a TypeScript code (code -1).
    code: &'a dyn Fn(P<Diagnostic>) -> String,
}

impl<'a> Format<'a> {
    /// Locations relative to `cwd`, file names compared as the OS does (tsgo: `sys.FS().UseCaseSensitiveFileNames()`).
    pub fn new(cwd: &str, pretty: bool, code: &'a dyn Fn(P<Diagnostic>) -> String) -> Self {
        let compare_paths_options = ComparePathsOptions {
            use_case_sensitive_file_names: rbxts_project::os_case_sensitive_file_names(),
            current_directory: tspath::normalize_path(cwd),
        };
        Format { options: FormattingOptions { compare_paths_options, new_line: NEW_LINE.to_owned() }, pretty, code }
    }
}

/// One diagnostic as tsgo's pretty writer formats it (`FormatDiagnosticWithColorAndContext`): without the line end
/// its reporter adds after it.
pub fn format_pretty(diagnostic: P<Diagnostic>, format: &Format<'_>) -> String {
    let mut out = Vec::new();
    format_diagnostic_with_color_and_context(&mut out, diagnostic, &format.options);
    code_slot(out, diagnostic, format, "\u{1b}[90m TS")
}

/// One diagnostic as tsgo's diagnostic reporter writes it: pretty, then a line end; or plain
/// (`WriteFormatDiagnostic`), a line.
pub fn format_reported(diagnostic: P<Diagnostic>, format: &Format<'_>) -> String {
    if format.pretty {
        return format_pretty(diagnostic, format) + NEW_LINE;
    }
    let mut out = Vec::new();
    write_format_diagnostic(&mut out, diagnostic, &format.options);
    code_slot(out, diagnostic, format, &format!("{} TS", diagnostic.category().name()))
}

/// The error summary tsgo prints after the diagnostics (`WriteErrorSummaryText`), pretty output only: empty without
/// errors or for plain output.
pub fn error_summary(diagnostics: &[P<Diagnostic>], format: &Format<'_>) -> String {
    if !format.pretty {
        return String::new();
    }
    let mut out = Vec::new();
    write_error_summary_text(&mut out, diagnostics, &format.options);
    String::from_utf8(out).expect("the summary is UTF-8")
}

/// `text` with a code-less diagnostic's code slot (the first `<before>-1: `, `before` ending with `TS`) showing
/// [`Format::code`]'s text instead of `-1`.
fn code_slot(text: Vec<u8>, diagnostic: P<Diagnostic>, format: &Format<'_>, before: &str) -> String {
    let text = String::from_utf8(text).expect("a diagnostic's text is UTF-8");
    if diagnostic.code() != -1 {
        return text;
    }
    let slot = format!("{before}-1: ");
    match text.find(&slot) {
        Some(at) => format!("{}{before}{}: {}", &text[..at], (format.code)(diagnostic), &text[at + slot.len()..]),
        None => text,
    }
}
