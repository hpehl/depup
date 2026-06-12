use console::style;
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;
use tokio::time::Instant;

const NAME_WIDTH: usize = 40;

pub fn step(emoji: &str, message: &str) {
    println!("{} {}", emoji, message);
}

pub fn done(instant: Instant) {
    println!(
        "\nDone in {}",
        style(HumanDuration(instant.elapsed())).cyan()
    );
}

#[derive(Clone)]
pub struct Progress {
    name: String,
    bar: ProgressBar,
}

impl Progress {
    pub fn join(multi_progress: &MultiProgress, name: &str) -> Progress {
        let progress = Progress {
            name: name.to_string(),
            bar: Self::spinner(),
        };
        progress.bar.enable_steady_tick(Duration::from_millis(100));
        multi_progress.add(progress.bar.clone());
        progress
            .bar
            .set_message(style(name).cyan().to_string());
        progress
    }

    pub fn hidden(name: &str) -> Progress {
        Progress {
            name: name.to_string(),
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

    pub fn finish_success(&self) {
        self.bar.set_style(Self::finished_style());
        let padded = format!("{:<width$}", self.name, width = NAME_WIDTH);
        self.bar.finish_with_message(format!(
            "{} {}",
            style("\u{2713}").green().bold(),
            style(padded).cyan()
        ));
    }

    pub fn finish_error(&self, err: &str) {
        self.bar.set_style(Self::finished_style());
        let padded = format!("{:<width$}", self.name, width = NAME_WIDTH);
        self.bar.abandon_with_message(format!(
            "{} {} {}",
            style("\u{2717}").red().bold(),
            style(padded).cyan(),
            style(err).red()
        ));
    }
}
