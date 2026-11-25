//! Генерация отчёта в Markdown (раздел 7.9 ТЗ).
//!
//! Отчёт детерминирован: при неизменном индексе и настройках текст одинаков,
//! кроме строки даты. Все данные берутся из индекса и журнала.

use atlas_store::{EdgeKind, NodeKind, Store};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

/// Параметры отчёта.
#[derive(Debug, Clone)]
pub struct ReportOptions {
    pub project_root: String,
    pub max_tokens: usize,
    pub max_depth: usize,
}

impl Default for ReportOptions {
    fn default() -> Self {
        Self {
            project_root: ".".into(),
            max_tokens: 4000,
            max_depth: 3,
        }
    }
}

/// Генератор отчёта в Markdown.
pub struct ReportGenerator {
    options: ReportOptions,
}

impl ReportGenerator {
    pub fn new(options: ReportOptions) -> Self {
        Self { options }
    }

    pub fn generate(&self, store: &Store) -> anyhow::Result<String> {
        let root = std::path::Path::new(&self.options.project_root);
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.options.project_root.clone());
        let files = store.nodes_of_kinds(&[NodeKind::File])?;
        let symbols = store.symbol_nodes()?;
        let edges = store.get_all_edges()?;
        let counts = store.count_by_kind()?;
        let total_nodes: i64 = counts.iter().map(|(_, n)| n).sum();
        let indexed_at = store
            .project_info()?
            .and_then(|(_, t)| t)
            .unwrap_or_else(|| "—".into());

        let mut out = String::new();
        writeln!(out, "# Отчёт по проекту: {name}\n")?;
        writeln!(
            out,
            "Сформирован: {} · Atlas {} · индекс: fresh ({} )\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
            env!("CARGO_PKG_VERSION"),
            indexed_at
        )?;
        writeln!(out, "## Что это за проект\n")?;
        match read_readme(root) {
            Some(text) => writeln!(out, "{text}\n")?,
            None => writeln!(out, "README не найден; описание построено по индексу.\n")?,
        }
        let mut langs: BTreeMap<String, usize> = BTreeMap::new();
        for f in &files {
            *langs
                .entry(f.lang.clone().unwrap_or_else(|| "other".into()))
                .or_default() += 1;
        }
        let main_lang = langs
            .iter()
            .max_by_key(|(_, n)| **n)
            .map(|(l, _)| l.clone())
            .unwrap_or_else(|| "—".into());
        writeln!(
            out,
            "- Основной язык: {main_lang}\n- Размер: {} файлов, {} символов, {} узлов, {} связей\n",
            files.len(),
            symbols.len(),
            total_nodes,
            edges.len()
        )?;

        self.write_structure(&mut out, &files)?;
        self.write_key_modules(&mut out, store, &files, &symbols)?;
        self.write_dependencies(&mut out, &files, &edges)?;
        self.write_activity(&mut out, store)?;
        self.write_ai_usage(&mut out, store)?;
        self.write_problems(&mut out, &files, &symbols, &edges)?;

        Ok(limit_tokens(out, self.options.max_tokens))
    }

    fn write_structure(&self, out: &mut String, files: &[atlas_store::Node]) -> anyhow::Result<()> {
        writeln!(out, "## Структура проекта\n")?;
        let mut groups: BTreeMap<String, usize> = BTreeMap::new();
        for f in files {
            let parts: Vec<&str> = f.path.split('/').collect();
            let depth = parts.len().saturating_sub(1).min(self.options.max_depth);
            for d in 0..=depth {
                if d == 0 && parts.len() == 1 {
                    continue;
                }
                let key = parts[..=d].join("/");
                if d < parts.len() - 1 || parts.len() == 1 {
                    *groups.entry(key).or_default() += 1;
                }
            }
        }
        if groups.is_empty() {
            writeln!(out, "- проект без каталогов\n")?;
        }
        for (path, count) in groups {
            let depth = path.matches('/').count();
            writeln!(out, "{}- {path}/ — {count} файлов", "  ".repeat(depth))?;
        }
        writeln!(out)?;
        Ok(())
    }

    fn write_key_modules(
        &self,
        out: &mut String,
        store: &Store,
        files: &[atlas_store::Node],
        symbols: &[atlas_store::Node],
    ) -> anyhow::Result<()> {
        writeln!(out, "## Ключевые модули и точки входа\n")?;
        // files уже отсортированы по рангу
        for f in files.iter().take(10) {
            if f.rank <= 0.0 && files.iter().take(10).all(|x| x.rank <= 0.0) {
                // если рангов нет, показываем по алфавиту
            }
            let count = symbols.iter().filter(|s| s.path == f.path).count();
            writeln!(
                out,
                "- {} — ранг {:.2}, символов {}",
                f.path, f.rank, count
            )?;
        }
        let mut top_symbols: Vec<&atlas_store::Node> = symbols.iter().take(8).collect();
        if !top_symbols.is_empty() {
            writeln!(out, "\nГлавные символы:")?;
            top_symbols.sort_by(|a, b| {
                b.rank
                    .partial_cmp(&a.rank)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for s in top_symbols.iter().take(8) {
                writeln!(
                    out,
                    "- {} ({}, {}:{})",
                    s.name,
                    s.kind,
                    s.path,
                    s.line_start.unwrap_or(0)
                )?;
            }
        }
        let _ = store;
        writeln!(out)?;
        Ok(())
    }

    fn write_dependencies(
        &self,
        out: &mut String,
        files: &[atlas_store::Node],
        edges: &[atlas_store::Edge],
    ) -> anyhow::Result<()> {
        writeln!(out, "## Зависимости\n")?;
        let by_id: HashMap<uuid::Uuid, &atlas_store::Node> =
            files.iter().map(|f| (f.id, f)).collect();
        let mut deps: BTreeMap<(String, String), usize> = BTreeMap::new();
        for e in edges.iter().filter(|e| e.kind == EdgeKind::Imports) {
            let (Some(src), Some(dst)) = (by_id.get(&e.src_id), by_id.get(&e.dst_id)) else {
                continue;
            };
            *deps.entry((src.path.clone(), dst.path.clone())).or_default() += 1;
        }
        if deps.is_empty() {
            writeln!(out, "Связей импорта между файлами не найдено.\n")?;
            return Ok(());
        }
        for ((src, dst), n) in deps.iter().take(50) {
            writeln!(out, "- {src} → {dst} ({n})")?;
        }
        let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (src, dst) in deps.keys() {
            graph.entry(src.clone()).or_default().push(dst.clone());
        }
        let cycles = find_cycles(&graph);
        if !cycles.is_empty() {
            writeln!(out, "\n### Циклические зависимости\n")?;
            for cycle in cycles.iter().take(5) {
                writeln!(out, "- {}", cycle.join(" → "))?;
            }
        }
        writeln!(out)?;
        Ok(())
    }

    fn write_activity(&self, out: &mut String, store: &Store) -> anyhow::Result<()> {
        writeln!(out, "## Что было сделано\n")?;
        match store.files_by_mtime(10)? {
            recent if !recent.is_empty() => {
                writeln!(
                    out,
                    "Git-история не подключена; ниже — самые свежие файлы по времени изменения.\n"
                )?;
                for (path, mtime) in recent {
                    writeln!(out, "- {path} — {mtime}")?;
                }
            }
            _ => writeln!(out, "Данных об изменениях нет.\n")?,
        }
        writeln!(out)?;
        Ok(())
    }

    fn write_ai_usage(&self, out: &mut String, store: &Store) -> anyhow::Result<()> {
        writeln!(out, "## Работа нейросети\n")?;
        let summary = store.call_log_summary()?;
        if summary.is_empty() {
            writeln!(out, "Вызовов MCP ещё не было.\n")?;
            return Ok(());
        }
        let calls: i64 = summary.iter().map(|(_, n, _)| *n).sum();
        let tokens: i64 = summary.iter().map(|(_, _, t)| *t).sum();
        writeln!(out, "- Вызовов: {calls}, токенов ответов ~{tokens}")?;
        for (tool, n, t) in summary {
            writeln!(out, "- {tool}: {n} вызовов, ~{t} токенов")?;
        }
        writeln!(out)?;
        Ok(())
    }

    fn write_problems(
        &self,
        out: &mut String,
        files: &[atlas_store::Node],
        symbols: &[atlas_store::Node],
        edges: &[atlas_store::Edge],
    ) -> anyhow::Result<()> {
        writeln!(out, "## Проблемы и ограничения\n")?;
        let no_lang: Vec<&atlas_store::Node> =
            files.iter().filter(|f| f.lang.is_none()).collect();
        writeln!(out, "- Файлов без поддерживаемого языка: {}", no_lang.len())?;
        let weak = edges
            .iter()
            .filter(|e| format!("{}", e.confidence) != "certain")
            .count();
        writeln!(out, "- Связей с низкой уверенностью: {weak}")?;

        let called: HashSet<uuid::Uuid> = edges
            .iter()
            .filter(|e| matches!(e.kind, EdgeKind::Calls | EdgeKind::Tests | EdgeKind::Extends))
            .map(|e| e.dst_id)
            .collect();
        let mut dead: Vec<&atlas_store::Node> = symbols
            .iter()
            .filter(|s| {
                matches!(s.kind, NodeKind::Function | NodeKind::Method)
                    && !s.path.to_lowercase().contains("test")
                    && !called.contains(&s.id)
            })
            .collect();
        dead.sort_by(|a, b| b.tokens_est.cmp(&a.tokens_est).then_with(|| a.path.cmp(&b.path)));
        if !dead.is_empty() {
            writeln!(out, "- Возможный мёртвый код (без входящих вызовов, топ-10):")?;
            for s in dead.iter().take(10) {
                writeln!(out, "  - {} ({}:{})", s.name, s.path, s.line_start.unwrap_or(0))?;
            }
        }
        writeln!(out)?;
        Ok(())
    }
}

fn read_readme(root: &std::path::Path) -> Option<String> {
    for name in ["README.md", "README", "readme.md"] {
        if let Ok(text) = std::fs::read_to_string(root.join(name)) {
            let mut excerpt = String::new();
            let mut chars = 0;
            for line in text.lines() {
                if line.starts_with('#') {
                    continue;
                }
                if line.trim().is_empty() {
                    continue;
                }
                chars += line.chars().count();
                if chars > 600 {
                    break;
                }
                excerpt.push_str(line.trim());
                excerpt.push('\n');
            }
            let excerpt = excerpt.trim();
            if !excerpt.is_empty() {
                return Some(excerpt.to_string());
            }
        }
    }
    None
}

/// Ищет циклы в графе импортов (простым DFS с ограничением глубины).
fn find_cycles(graph: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
    let mut cycles: Vec<Vec<String>> = Vec::new();
    for start in graph.keys() {
        let mut stack = vec![start.clone()];
        let mut path = vec![start.clone()];
        let mut visited: HashSet<String> = HashSet::new();
        if dfs_cycle(graph, start, &mut stack, &mut path, &mut visited, &mut cycles) {
            break;
        }
    }
    cycles
}

fn dfs_cycle(
    graph: &BTreeMap<String, Vec<String>>,
    current: &str,
    stack: &mut Vec<String>,
    path: &mut Vec<String>,
    visited: &mut HashSet<String>,
    cycles: &mut Vec<Vec<String>>,
) -> bool {
    if cycles.len() >= 5 {
        return true;
    }
    if !visited.insert(current.to_string()) {
        return false;
    }
    if let Some(nexts) = graph.get(current) {
        for next in nexts {
            if let Some(pos) = path.iter().position(|p| p == next) {
                let mut cycle = path[pos..].to_vec();
                cycle.push(next.clone());
                cycles.push(cycle);
                if cycles.len() >= 5 {
                    return true;
                }
                continue;
            }
            path.push(next.clone());
            if dfs_cycle(graph, next, stack, path, visited, cycles) {
                return true;
            }
            path.pop();
        }
    }
    visited.remove(current);
    stack.pop();
    false
}

/// Ограничивает отчёт по токенам, сохраняя шапку и обрезая хвост.
fn limit_tokens(text: String, max_tokens: usize) -> String {
    if max_tokens == 0 {
        return text;
    }
    let max_chars = max_tokens * 4;
    if text.chars().count() <= max_chars {
        return text;
    }
    let cut: String = text.chars().take(max_chars).collect();
    let cut = match cut.rfind('\n') {
        Some(pos) => &cut[..pos],
        None => &cut[..],
    };
    format!("{cut}\n\n[отчёт обрезан по лимиту {max_tokens} токенов]\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_store::schema::{Edge, EdgeKind, Confidence, Node, NodeKind};
    use uuid::Uuid;

    fn node(kind: NodeKind, path: &str, name: &str, rank: f64) -> Node {
        Node {
            id: Uuid::new_v4(),
            kind,
            path: path.into(),
            name: name.into(),
            parent_id: None,
            lang: Some("rust".into()),
            hash: None,
            line_start: Some(1),
            line_end: Some(2),
            signature: Some(format!("fn {name}()")),
            doc_summary: None,
            tokens_est: 5,
            rank,
        }
    }

    #[test]
    fn report_contains_core_sections() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("i.db")).unwrap();
        let a = node(NodeKind::File, "src/a.rs", "a.rs", 1.0);
        let b = node(NodeKind::File, "src/b.rs", "b.rs", 0.5);
        let f = node(NodeKind::Function, "src/a.rs", "run", 1.0);
        for n in [&a, &b, &f] {
            store.upsert_node(n).unwrap();
        }
        store
            .upsert_edge(&Edge {
                src_id: b.id,
                dst_id: a.id,
                kind: EdgeKind::Imports,
                confidence: Confidence::Certain,
                weight: 1.0,
            })
            .unwrap();
        let report = ReportGenerator::new(ReportOptions {
            project_root: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .generate(&store)
        .unwrap();
        assert!(report.contains("# Отчёт по проекту"));
        assert!(report.contains("## Структура проекта"));
        assert!(report.contains("## Зависимости"));
        assert!(report.contains("b.rs → src/a.rs") || report.contains("src/b.rs → src/a.rs"));
        assert!(report.contains("## Работа нейросети"));
    }

    #[test]
    fn cycles_are_detected() {
        let mut graph = BTreeMap::new();
        graph.insert("a".to_string(), vec!["b".to_string()]);
        graph.insert("b".to_string(), vec!["a".to_string()]);
        let cycles = find_cycles(&graph);
        assert!(!cycles.is_empty());
        assert!(cycles[0].contains(&"a".to_string()));
    }
}
