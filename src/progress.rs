use console::style;
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use tokio::time::Instant;

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

pub fn done(instant: Instant) {
    println!(
        "\nDone in {}",
        style(HumanDuration(instant.elapsed())).cyan()
    );
}

pub fn truncate_middle_pad(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count > width {
        let ellipsis = "\u{2026}";
        let half = (width - 1) / 2;
        let remainder = width - 1 - half;
        let prefix: String = s.chars().take(half).collect();
        let suffix: String = s.chars().skip(char_count - remainder).collect();
        format!("{prefix}{ellipsis}{suffix}")
    } else {
        format!("{s:<width$}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_string_pads() {
        let result = truncate_middle_pad("hello", 10);
        assert_eq!(result, "hello     ");
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn truncate_exact_width_no_change() {
        let result = truncate_middle_pad("abcde", 5);
        assert_eq!(result, "abcde");
    }

    #[test]
    fn truncate_long_string_inserts_ellipsis() {
        let result = truncate_middle_pad("abcdefghij", 7);
        assert!(result.contains('\u{2026}'));
        assert_eq!(result.chars().count(), 7);
        assert!(result.starts_with("abc"));
        assert!(result.ends_with("hij"));
    }

    #[test]
    fn truncate_multibyte_does_not_panic() {
        let result = truncate_middle_pad("ä\u{f6}ü\u{e9}\u{e8}\u{ea}\u{eb}\u{e0}", 5);
        assert_eq!(result.chars().count(), 5);
    }
}
