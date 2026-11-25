/// Бюджет токенов для вызова инструмента (раздел 8.3 ТЗ).
#[derive(Debug, Clone)]
pub struct Budget {
    pub max_tokens: usize,
    pub used_tokens: usize,
}

impl Budget {
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            used_tokens: 0,
        }
    }

    pub fn remaining(&self) -> usize {
        self.max_tokens.saturating_sub(self.used_tokens)
    }

    pub fn consume(&mut self, tokens: usize) -> bool {
        if self.used_tokens + tokens > self.max_tokens {
            return false;
        }
        self.used_tokens += tokens;
        true
    }
}

/// Грубая оценка числа токенов: один токен на 4 символа (раздел 5.4 ТЗ).
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

/// Собирает строки в текст, пока они помещаются в бюджет. Возвращает текст и число показанных строк.
pub fn truncate_to_budget(lines: &[String], max_tokens: usize) -> (String, usize) {
    let mut out = String::new();
    let mut used = 0;
    let mut shown = 0;
    for line in lines {
        let cost = estimate_tokens(line) + 1;
        if used + cost > max_tokens && shown > 0 {
            break;
        }
        out.push_str(line);
        out.push('\n');
        used += cost;
        shown += 1;
    }
    (out, shown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_by_budget_but_keeps_first_line() {
        let lines: Vec<String> = (0..100).map(|i| format!("строка номер {i}")).collect();
        let (text, shown) = truncate_to_budget(&lines, 30);
        assert!(shown > 0 && shown < 100);
        assert_eq!(text.lines().count(), shown);
        let (_, one) = truncate_to_budget(&lines, 0);
        assert_eq!(one, 1);
    }
}
