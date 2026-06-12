use console::style;
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;
use tokio::time::Instant;

use crate::pom::ArtifactKind;
use crate::registry::CheckResult;

const KIND_WIDTH: usize = 12;
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
    kind: String,
    name: String,
    artifact: String,
    current_version: String,
    bar: ProgressBar,
}

impl Progress {
    pub fn join(
        multi_progress: &MultiProgress,
        kind: &ArtifactKind,
        name: &str,
        artifact: &str,
        current_version: &str,
    ) -> Self {
        let bar = multi_progress.add(Self::spinner());
        let progress = Self {
            kind: kind.to_string(),
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

    pub fn hidden(
        kind: &ArtifactKind,
        name: &str,
        artifact: &str,
        current_version: &str,
    ) -> Self {
        Self {
            kind: kind.to_string(),
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

    pub fn finish_with_result(&self, result: &CheckResult) {
        self.bar.set_style(Self::finished_style());
        let columns = self.format_columns();

        if let Some(err) = &result.error {
            self.bar.abandon_with_message(format!(
                "{} {columns}  {}",
                style("\u{2717}").red().bold(),
                style(err).red()
            ));
        } else if result.outdated {
            let latest = result.latest_version.as_deref().unwrap_or("?");
            self.bar.finish_with_message(format!(
                "{} {columns}  {}",
                style("\u{2192}").yellow().bold(),
                style(format!("\u{2192} {latest}")).yellow()
            ));
        } else {
            self.bar.finish_with_message(format!(
                "{} {columns}  {}",
                style("\u{2713}").green().bold(),
                style("up-to-date").green()
            ));
        }
    }

    pub fn clear(&self) {
        self.bar.finish_and_clear();
    }

    fn format_columns(&self) -> String {
        let kind = truncate_pad(&self.kind, KIND_WIDTH);
        let name = truncate_pad(&self.name, NAME_WIDTH);
        let artifact = truncate_pad(&self.artifact, ARTIFACT_WIDTH);
        let version = truncate_pad(&self.current_version, VERSION_WIDTH);
        format!(
            "{}  {}  {}  {}",
            style(kind).dim(),
            style(name).cyan(),
            style(artifact).dim(),
            style(version).white(),
        )
    }
}

fn truncate_pad(s: &str, width: usize) -> String {
    if s.len() > width {
        format!("{}\u{2026}", &s[..width - 1])
    } else {
        format!("{:<width$}", s, width = width)
    }
}
