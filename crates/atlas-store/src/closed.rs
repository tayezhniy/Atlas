//! Закрытые пути из настроек проекта (`closedPaths`): синтаксис как в `.gitignore`.

use crate::schema::Node;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::Path;

pub(crate) fn build(patterns: &[String]) -> Gitignore {
    let mut builder = GitignoreBuilder::new("");
    // регистр не важен, как и во встроенном списке
    let _ = builder.case_insensitive(true);
    for pattern in patterns {
        // неверный шаблон не должен отключать остальные
        let _ = builder.add_line(None, pattern);
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

pub(crate) fn matches(rules: &Gitignore, path: &str) -> bool {
    let path = Path::new(path);
    !path.has_root() && rules.matched_path_or_any_parents(path, false).is_ignore()
}

/// Убирает из узла всё, что взято из содержимого файла.
pub(crate) trait Redact {
    fn redact(&mut self, rules: &Gitignore);
}

impl Redact for Node {
    fn redact(&mut self, rules: &Gitignore) {
        if matches(rules, &self.path) {
            self.signature = None;
            self.doc_summary = None;
        }
    }
}

impl<T: Redact> Redact for Option<T> {
    fn redact(&mut self, rules: &Gitignore) {
        if let Some(item) = self {
            item.redact(rules);
        }
    }
}

impl<T: Redact> Redact for Vec<T> {
    fn redact(&mut self, rules: &Gitignore) {
        self.iter_mut().for_each(|item| item.redact(rules));
    }
}
