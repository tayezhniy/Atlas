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