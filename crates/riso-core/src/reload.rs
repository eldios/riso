//! Telling the desktop that the theme changed.
//!
//! Running a command is the one thing the core cannot do in a test, so it goes
//! through a trait. Tests inject a recorder and assert on what would have run;
//! nothing here needs a graphical session.

use std::process::Command;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReloadError {
    #[error("could not run '{0}': {1}")]
    Spawn(String, #[source] std::io::Error),
    #[error("'{0}' failed: {1}")]
    Failed(String, String),
}

/// Runs a command, or pretends to.
pub trait Executor {
    /// Run `program` with `args`. A non-zero exit is not an error: consumers
    /// that are not running are the normal case, not a failure to report.
    fn run(&self, program: &str, args: &[String]) -> Result<(), ReloadError>;

    /// Run `program` and return what it printed.
    ///
    /// Unlike `run`, a non-zero exit is an error here: a caller asking for
    /// output cannot do anything useful with a failed command's empty string.
    fn capture(&self, program: &str, args: &[String]) -> Result<String, ReloadError>;
}

pub struct ProcessExecutor;

impl Executor for ProcessExecutor {
    fn run(&self, program: &str, args: &[String]) -> Result<(), ReloadError> {
        Command::new(program)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|_| ())
            .map_err(|e| ReloadError::Spawn(program.to_owned(), e))
    }

    fn capture(&self, program: &str, args: &[String]) -> Result<String, ReloadError> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| ReloadError::Spawn(program.to_owned(), e))?;

        if !output.status.success() {
            return Err(ReloadError::Failed(
                program.to_owned(),
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

/// An executor that records instead of running, for tests and dry runs.
#[derive(Debug, Default)]
pub struct RecordingExecutor {
    calls: std::cell::RefCell<Vec<String>>,
    /// What `capture` should return, in call order.
    output: std::cell::RefCell<Vec<String>>,
}

impl RecordingExecutor {
    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }

    /// Queue what the next `capture` calls will return.
    pub fn will_output(&self, lines: &[&str]) {
        *self.output.borrow_mut() = lines.iter().rev().map(|l| (*l).to_owned()).collect();
    }
}

impl Executor for RecordingExecutor {
    fn run(&self, program: &str, args: &[String]) -> Result<(), ReloadError> {
        self.calls
            .borrow_mut()
            .push(format!("{program} {}", args.join(" ")));
        Ok(())
    }

    fn capture(&self, program: &str, args: &[String]) -> Result<String, ReloadError> {
        self.run(program, args)?;
        Ok(self.output.borrow_mut().pop().unwrap_or_default())
    }
}

/// Hand the palette to a running Omarchy shell over its IPC socket.
///
/// The payloads are base64 so they survive an argument list, which is how the
/// shell's `applyTheme` expects them. A shell that is not running simply makes
/// the command fail, and that is not an error worth stopping an apply for.
pub fn notify_omarchy_shell(
    exec: &dyn Executor,
    colors: Option<&str>,
    shell: Option<&str>,
) -> Result<(), ReloadError> {
    exec.run(
        "omarchy-shell",
        &[
            "shell".to_owned(),
            "applyTheme".to_owned(),
            colors.unwrap_or_default().to_owned(),
            shell.unwrap_or_default().to_owned(),
        ],
    )
}

/// Minimal base64, so a payload can be passed as one argument.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b = |i: usize| *chunk.get(i).unwrap_or(&0) as u32;
        let triple = (b(0) << 16) | (b(1) << 8) | b(2);
        for i in 0..4 {
            if i <= chunk.len() {
                let index = (triple >> (18 - i * 6)) & 0x3f;
                out.push(ALPHABET[index as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_the_ipc_call_without_running_it() {
        let recorder = RecordingExecutor::default();
        notify_omarchy_shell(&recorder, Some("Y29sb3Jz"), Some("c2hlbGw=")).expect("notify");
        assert_eq!(
            recorder.calls(),
            ["omarchy-shell shell applyTheme Y29sb3Jz c2hlbGw="]
        );
    }

    #[test]
    fn passes_empty_payloads_as_empty_arguments() {
        let recorder = RecordingExecutor::default();
        notify_omarchy_shell(&recorder, None, None).expect("notify");
        assert_eq!(recorder.calls(), ["omarchy-shell shell applyTheme  "]);
    }

    // Vectors from RFC 4648 section 10.
    #[test]
    fn encodes_base64_including_padding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn encodes_bytes_that_are_not_text() {
        assert_eq!(base64(&[0xff, 0xfe, 0xfd]), "//79");
        assert_eq!(base64(&[0x00, 0x00, 0x00]), "AAAA");
    }
}
