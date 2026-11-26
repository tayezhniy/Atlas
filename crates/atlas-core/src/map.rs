//! Компактная карта проекта (L0/L1, раздел 6.1 ТЗ).

use atlas_store::{EdgeKind, Node, NodeKind, Store};
use std::collections::{BTreeMap, HashMap};

/// Генератор карты проекта с ограничением по токенам.
pub struct ProjectMap {
    max_tokens: usize,
}

impl ProjectMap {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }

    /// Карта проекта: языки, группы каталогов, ключевые файлы и хабы.
    pub fn render(&self, store: &Store) -> anyhow::Result<String> {
        let files = store.nodes_of_kinds(&[NodeKind::File])?;
        let symbols = store.nodes_of_kinds(&[
            NodeKind::Class,
            NodeKind::Interface,
            NodeKind::Function,
            NodeKind::Method,
            NodeKind::Constant,
            NodeKind::Type,
        ])?;
        let mut langs: BTreeMap<String, usize> = BTreeMap::new();
        for f in &files {
            *langs
                .entry(f.lang.clone().unwrap_or_else(|| "other".into()))
                .or_default() += 1;
        }
        let lang_text = langs
            .iter()
            .map(|(l, n)| format!("{l} {n}"))
            .collect::<Vec<_>>()
            .join(", ");

        let mut lines = vec![
            "## Карта проекта".to_string(),
            format!(
                "{} файлов, {} символов; языки: {}",
                files.len(),
                symbols.len(),
                lang_text
            ),
        ];

        let mut groups: BTreeMap<String, Vec<&Node>> = BTreeMap::new();
        for f in &files {
            let top = f.path.split('/').next().unwrap_or(".");
            groups.entry(top.to_string()).or_default().push(f);
        }
        lines.push(String::new());
        lines.push("### Группы".into());
        for (name, members) in &groups {
            let tokens: i32 = members.iter().map(|f| f.tokens_est).sum();
            let top: Vec<&str> = members.iter().take(3).map(|f| f.path.as_str()).collect();
            lines.push(format!(
                "- {name}/ — {} файлов, ~{tokens} ток.; ключевые: {}",
                members.len(),
                top.join(", ")
            ));
        }

        // Хабы — файлы с наибольшим числом входящих imports/calls
        let mut hubs: HashMap<uuid::Uuid, usize> = HashMap::new();
        for edge in store.get_all_edges()? {
            if matches!(edge.kind, EdgeKind::Imports | EdgeKind::Calls) {
                *hubs.entry(edge.dst_id).or_default() += 1;
            }
        }
        let mut hub_files: Vec<(&Node, usize)> = files
            .iter()
            .filter_map(|f| hubs.get(&f.id).map(|n| (f, *n)))
            .collect();
        hub_files.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.path.cmp(&b.0.path)));
        if !hub_files.is_empty() {
            lines.push(String::new());
            lines.push("### Хабы".into());
            for (f, n) in hub_files.iter().take(5) {
                lines.push(format!("- {} — входящих связей {n}", f.path));
            }
        }

        lines.push(String::new());
        lines.push("Дальше: overview{path}, search{query}, outline{path}, read{symbol}.".into());

        Ok(truncate(lines, self.max_tokens))
    }
}

/// Обрезает текст по бюджету токенов, сохраняя хотя бы первые строки.
fn truncate(lines: Vec<String>, max_tokens: usize) -> String {
    if max_tokens == 0 {
        return lines.join("\n");
    }
    let mut out = String::new();
    let mut used = 0;
    let mut shown = 0;
    for line in &lines {
        let cost = (line.chars().count() / 4).max(1) + 1;
        if used + cost > max_tokens && shown > 0 {
            break;
        }
        out.push_str(line);
        out.push('\n');
        used += cost;
        shown += 1;
    }
    if shown < lines.len() {
        out.push_str(&format!("[показано {shown} из {} строк]\n", lines.len()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_keeps_at_least_one_line() {
        let lines: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
        let text = truncate(lines, 1);
        assert!(text.starts_with("line 0"));
        assert!(text.contains("[показано"));
    }
}
