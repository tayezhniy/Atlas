//! Фильтр вывода команд (раздел 9.1 ТЗ).
//!
//! Оставляет ошибки, предупреждения и контекст вокруг них, схлопывает повторы,
//! убирает ANSI-коды и шум из каталогов зависимостей.

/// Настройки фильтра.
#[derive(Debug, Clone)]
pub struct FilterOptions {
    pub context: usize,
    pub keep_summary: bool,
}

impl Default for FilterOptions {
    fn default() -> Self {
        Self {
            context: 3,
            keep_summary: true,
        }
    }
}

/// Результат фильтрации.
#[derive(Debug, Clone)]
pub struct FilterResult {
    pub text: String,
    pub original_lines: usize,
    pub kept_lines: usize,
}

const NOISE_PATHS: [&str; 6] = [
    "node_modules/",
    "/vendor/",
    "target/debug/",
    "target/release/",
    "site-packages/",
    "/usr/lib/",
];

/// Фильтрует сырой вывод команды.
pub fn filter_output(raw: &str, options: &FilterOptions) -> FilterResult {
    let stripped = strip_ansi(raw);
    let lines: Vec<&str> = stripped.lines().collect();
    let mut keep = vec![false; lines.len()];

    for (i, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();
        let is_important = lower.contains("error")
            || lower.contains("warning")
            || lower.contains("fail")
            || lower.contains("panic")
            || lower.contains("fatal")
            || lower.contains("exception")
            || lower.contains("traceback");
        if !is_important {
            continue;
        }
        let noisy = NOISE_PATHS.iter().any(|p| lower.contains(p));
        if noisy {
            continue;
        }
        let start = i.saturating_sub(options.context);
        let end = (i + options.context + 1).min(lines.len());
        for flag in keep.iter_mut().take(end).skip(start) {
            *flag = true;
        }
    }

    // Если ничего важного не нашли, оставляем последние строки (обычно итог)
    if !keep.iter().any(|k| *k) && options.keep_summary {
        let start = lines.len().saturating_sub(10);
        for flag in keep.iter_mut().skip(start) {
            *flag = true;
        }
    }

    let mut out: Vec<String> = Vec::new();
    let mut previous: Option<&str> = None;
    let mut repeat = 0usize;
    let mut kept = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if !keep[i] {
            continue;
        }
        kept += 1;
        let trimmed = line.trim_end();
        if previous == Some(trimmed) {
            repeat += 1;
            continue;
        }
        if repeat > 0 {
            out.push(format!("  … ×{}", repeat + 1));
            repeat = 0;
        }
        if trimmed.trim().is_empty() && out.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
            continue;
        }
        out.push(trimmed.to_string());
        previous = Some(trimmed);
    }
    if repeat > 0 {
        out.push(format!("  … ×{}", repeat + 1));
    }

    if options.keep_summary {
        out.push(format!(
            "[отфильтровано: показано {kept} из {} строк]",
            lines.len()
        ));
    }

    FilterResult {
        text: out.join("\n"),
        original_lines: lines.len(),
        kept_lines: kept,
    }
}

/// Удаляет ANSI escape-последовательности.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                // ESC [ ... буква — конец последовательности
                for n in chars.by_ref() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
                continue;
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_errors_with_context_and_drops_noise() {
        let raw = "\u{1b}[31merror\u{1b}[0m: boom\nline2\nline3\nline4\nline5\nnode_modules/x.js error ignored\nok\n";
        let r = filter_output(raw, &FilterOptions::default());
        assert!(r.text.contains("error: boom"));
        assert!(r.text.contains("line2"));
        assert!(!r.text.contains("node_modules"));
        assert!(r.text.contains("отфильтровано"));
    }

    #[test]
    fn collapses_repeats() {
        let raw = "warning: same\nwarning: same\nwarning: same\n";
        let r = filter_output(raw, &FilterOptions::default());
        assert!(r.text.contains("×3"), "{}", r.text);
    }
}
