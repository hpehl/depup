//! Progress bar helpers using `indicatif`.
//!
//! Provides a block-style progress bar for concurrent checks.
//! Hidden automatically when `--json` mode is active (caller passes `ProgressBar::hidden()`).

use console::style;
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use tokio::time::Instant;

/// Creates a labeled, json-aware progress bar.
///
/// Labels are left-aligned and padded to 10 characters so multiple bars align vertically.
/// Returns a hidden bar when `json` is true or `total` is zero.
pub fn phase_bar(label: &str, total: u64, json: bool) -> ProgressBar {
    if json || total == 0 {
        ProgressBar::hidden()
    } else {
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::default_bar()
                .template(&format!(
                    "  {label:<10} [{{pos:>3}}/{{len:<3}}] {{bar:30}} {{msg:.dim}}"
                ))
                .expect("Invalid bar template")
                .progress_chars("▰▱ "),
        );
        bar
    }
}

/// Prints the elapsed wall-clock time after all checks complete.
pub fn done(instant: Instant) {
    println!(
        "\nDone in {}",
        style(HumanDuration(instant.elapsed())).cyan()
    );
}
