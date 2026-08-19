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
