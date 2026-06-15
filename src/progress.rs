//! Progress bar helpers using `indicatif`.
//!
//! Provides a block-style progress bar for concurrent checks.
//! Hidden automatically when `--json` mode is active (caller passes `ProgressBar::hidden()`).

use console::style;
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use tokio::time::Instant;

/// Creates a styled progress bar with `[pos/len]` format and block characters.
pub fn bar(total: u64) -> ProgressBar {
    let bar = ProgressBar::new(total);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("  [{pos}/{len}] {bar:30} {msg}")
            .expect("Invalid bar template")
            .progress_chars("\u{2588}\u{2592}\u{2591}"),
    );
    bar
}

/// Prints the elapsed wall-clock time after all checks complete.
pub fn done(instant: Instant) {
    println!(
        "\nDone in {}",
        style(HumanDuration(instant.elapsed())).cyan()
    );
}

