use console::style;
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;
use tokio::time::Instant;

use crate::registry::CheckerKind;

pub enum ProgressOutcome<'a> {
    UpToDate,
    Outdated { latest: &'a str },
    Skipped,
    Error { message: &'a str },
}

const NAME_WIDTH: usize = 40;
const ARTIFACT_WIDTH: usize = 50;
const VERSION_WIDTH: usize = 14;

pub fn step(emoji: &str, message: &str) {
    println!("{emoji} {message}");
}

pub fn done(instant: Instant) {
    println!(
        "\nDone in {}",
        style(HumanDuration(instant.elapsed())).cyan()
    );
}

#[derive(Clone)]
pub struct Progress {
    kind: CheckerKind,
    name: String,
    artifact: String,
    current_version: String,
    bar: ProgressBar,
}

impl Progress {
    pub fn join(
        multi_progress: &MultiProgress,
        kind: CheckerKind,
        name: &str,
        artifact: &str,
        current_version: &str,
    ) -> Self {
        let bar = multi_progress.add(Self::spinner());
        let progress = Self {
            kind,
            name: name.to_string(),
            artifact: artifact.to_string(),
            current_version: current_version.to_string(),
            bar,
        };
        let columns = progress.format_columns();
        let status = style("checking...").dim().to_string();
        progress.bar.set_message(format!("{columns}  {status}"));
        progress.bar.enable_steady_tick(Duration::from_millis(100));
        progress
    }

    pub fn hidden(kind: CheckerKind, name: &str, artifact: &str, current_version: &str) -> Self {
        Self {
            kind,
            name: name.to_string(),
            artifact: artifact.to_string(),
            current_version: current_version.to_string(),
            bar: ProgressBar::hidden(),
        }
    }

    fn spinner() -> ProgressBar {
        ProgressBar::new_spinner().with_style(
            ProgressStyle::default_spinner()
                .tick_strings(&[
                    "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}",
                    "\u{2826}", "\u{2827}", "\u{2807}", "\u{280f}", " ",
                ])
                .template("  {spinner:.dim.bold} {wide_msg}")
                .expect("Invalid spinner template"),
        )
    }

    fn finished_style() -> ProgressStyle {
        ProgressStyle::default_spinner()
            .template("  {wide_msg}")
            .expect("Invalid template")
    }

    pub fn finish(&self, outcome: ProgressOutcome<'_>) {
        self.bar.set_style(Self::finished_style());
        let columns = self.format_columns();

        match outcome {
            ProgressOutcome::Error { message } => {
                self.bar.abandon_with_message(format!(
                    "{} {columns}  {}",
                    style("\u{2717}").red().bold(),
                    style(message).red()
                ));
            }
            ProgressOutcome::Skipped => {
                self.bar.finish_with_message(format!(
                    "{} {columns}  {}",
                    style("-").dim().bold(),
                    style("skipped").dim()
                ));
            }
            ProgressOutcome::Outdated { latest } => {
                self.bar.finish_with_message(format!(
                    "{} {columns}  {}",
                    style("\u{2192}").yellow().bold(),
                    style(format!("\u{2192} {latest}")).yellow()
                ));
            }
            ProgressOutcome::UpToDate => {
                self.bar.finish_with_message(format!(
                    "{} {columns}  {}",
                    style("\u{2713}").green().bold(),
                    style("up-to-date").green()
                ));
            }
        }
    }

    fn format_columns(&self) -> String {
        let name = truncate_middle_pad(&self.name, NAME_WIDTH);
        let artifact = truncate_middle_pad(&self.artifact, ARTIFACT_WIDTH);
        let version = truncate_middle_pad(&self.current_version, VERSION_WIDTH);
        let styled_name = self.kind.color().apply_to(name);
        format!("{}  {}  {}", styled_name, artifact, style(version).white(),)
    }
}

fn truncate_middle_pad(s: &str, width: usize) -> String {
    if s.len() > width {
        let ellipsis = "\u{2026}";
        let half = (width - 1) / 2;
        let remainder = width - 1 - half;
        format!("{}{ellipsis}{}", &s[..half], &s[s.len() - remainder..])
    } else {
        format!("{:<width$}", s, width = width)
    }
}
