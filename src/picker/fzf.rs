//! External fzf fallback picker.
//!
//! Pipes the context list directly to fzf's stdin and reads the
//! selection from stdout. No `FZF_DEFAULT_COMMAND` re-invocation --
//! just a simple stdin pipe.

use std::io::Write;
use std::process::{Command, Stdio};

use super::{PickerItem, PickerResult};

/// Run fzf as an external process for context selection.
///
/// Pipes the item list to fzf's stdin and reads the selected line
/// from stdout. The current context is passed as `--query` to
/// pre-position the cursor.
///
/// # Errors
///
/// Returns an error if fzf is not found on `PATH` or if the
/// subprocess fails unexpectedly.
pub fn pick_fzf(items: &[PickerItem]) -> anyhow::Result<PickerResult> {
    let current = items.iter().find(|i| i.is_current).map(|i| i.name.as_str());

    let mut cmd = Command::new("fzf");
    cmd.args(["--height", "40%", "--reverse", "--no-sort", "--ansi"]);

    if let Some(name) = current {
        cmd.args(["--query", name]);
    }

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!("fzf not found on PATH. Install fzf or use the built-in picker")
        } else {
            anyhow::anyhow!("failed to spawn fzf: {e}")
        }
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        for item in items {
            // Silently ignore write errors -- fzf may close stdin early
            // if the user selects before all items are written.
            let _ = writeln!(stdin, "{}", item.name);
        }
    }

    let output = child.wait_with_output()?;

    if output.status.success() {
        let selected = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if selected.is_empty() {
            Ok(PickerResult::Cancelled)
        } else {
            Ok(PickerResult::Selected(selected))
        }
    } else {
        // fzf exit code 1: no match / Esc; exit code 130: SIGINT.
        // Both mean the user cancelled.
        Ok(PickerResult::Cancelled)
    }
}
