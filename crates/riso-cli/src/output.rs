//! Rendering command results in the format the caller asked for.
//!
//! Human output is each command's own text, unchanged from what it always
//! printed; `-o json` and `-o yaml` serialize a structured value instead, so
//! scripts never have to parse prose.

use clap::ValueEnum;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Yaml,
}

/// Serialize `value` in the requested format.
///
/// Returns false for human output, telling the caller to print its own text:
/// keeping the human rendering at the call site is what lets it stay exactly
/// the text it always was.
pub fn emit<T: Serialize>(format: OutputFormat, value: &T) -> Result<bool, String> {
    match format {
        OutputFormat::Human => Ok(false),
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(value).map_err(|e| e.to_string())?
            );
            Ok(true)
        }
        OutputFormat::Yaml => {
            print!(
                "{}",
                serde_norway::to_string(value).map_err(|e| e.to_string())?
            );
            Ok(true)
        }
    }
}

/// The format config.toml asks for, when `-o` does not override it. A
/// value the enum does not know is reported and ignored, never fatal.
pub fn default_output() -> OutputFormat {
    let config = riso_core::config::Config::load_or_default();
    match OutputFormat::from_str(&config.output, true) {
        Ok(format) => format,
        Err(_) => {
            eprintln!(
                "riso: config.toml asks for output {:?}, which is not human, json or yaml; using human",
                config.output
            );
            OutputFormat::Human
        }
    }
}

/// Warnings go to stderr so they never contaminate piped output.
pub fn report_warnings(warnings: &[riso_core::palette::Warning]) {
    use riso_core::palette::Warning;
    for warning in warnings {
        let text = match warning {
            Warning::UnsupportedKey { line } => {
                format!("line {line}: key has unsupported characters, skipped")
            }
            Warning::UnsupportedValue { line, key } => {
                format!("line {line}: value of '{key}' has unsupported characters, skipped")
            }
            Warning::EmptyValue { key } => {
                format!("'{key}' resolved to nothing, its placeholder will render empty")
            }
            Warning::UnderivableColor { key, source } => {
                format!("cannot derive '{key}': '{source}' is not a hex color")
            }
        };
        eprintln!("riso: {text}");
    }
}
