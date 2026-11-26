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
        store.load_closed_paths()?;
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
        match read_readme(store, root) {
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

        // README и сигнатуры могут содержать секреты
        Ok(crate::secrets::mask_text(&limit_tokens(
            out,
            self.options.max_tokens,
        )))
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
            let count = symbols.iter().filter(|s| s.path == f.path).count();
            writeln!(out, "- {} — ранг {:.2}, символов {}", f.path, f.rank, count)?;
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
            *deps
                .entry((src.path.clone(), dst.path.clone()))
                .or_default() += 1;
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
        let no_lang: Vec<&atlas_store::Node> = files.iter().filter(|f| f.lang.is_none()).collect();
        writeln!(out, "- Файлов без поддерживаемого языка: {}", no_lang.len())?;
        let weak = edges
            .iter()
            .filter(|e| format!("{}", e.confidence) != "certain")
            .count();
        writeln!(out, "- Связей с низкой уверенностью: {weak}")?;

        let called: HashSet<uuid::Uuid> = edges
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    EdgeKind::Calls | EdgeKind::Tests | EdgeKind::Extends
                )
            })
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
        dead.sort_by(|a, b| {
            b.tokens_est
                .cmp(&a.tokens_est)
                .then_with(|| a.path.cmp(&b.path))
        });
        if !dead.is_empty() {
            writeln!(
                out,
                "- Возможный мёртвый код (без входящих вызовов, топ-10):"
            )?;
            for s in dead.iter().take(10) {
                writeln!(
                    out,
                    "  - {} ({}:{})",
                    s.name,
                    s.path,
                    s.line_start.unwrap_or(0)
                )?;
            }
        }
        writeln!(out)?;
        Ok(())
    }
}

fn read_readme(store: &Store, root: &std::path::Path) -> Option<String> {
    for name in ["README.md", "README", "readme.md"] {
        // README не должен быть ссылкой на закрытый файл или за пределы проекта
        let Ok((base, file)) = root
            .canonicalize()
            .and_then(|b| Ok((b, root.join(name).canonicalize()?)))
        else {
            continue;
        };
        let Ok(inside) = file.strip_prefix(&base) else {
            continue;
        };
        if crate::secrets::is_closed_path(&inside.to_string_lossy())
            || store.is_user_closed(&inside.to_string_lossy())
        {
            continue;
        }
        if let Ok(raw) = std::fs::read_to_string(&file) {
            let text = crate::secrets::mask_lines(raw.lines(), true).join("\n");
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
        if dfs_cycle(
            graph,
            start,
            &mut stack,
            &mut path,
            &mut visited,
            &mut cycles,
        ) {
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

/// Структурированный отчёт для `report --format json` и интерфейса.
pub fn report_json(store: &Store, project_root: &str) -> anyhow::Result<serde_json::Value> {
    store.load_closed_paths()?;
    use serde_json::json;

    let nodes = store.get_all_nodes()?;
    let edges = store.get_all_edges()?;
    let files = nodes.iter().filter(|n| n.kind == NodeKind::File).count();
    let mut languages: BTreeMap<String, usize> = BTreeMap::new();
    for f in nodes.iter().filter(|n| n.kind == NodeKind::File) {
        *languages
            .entry(f.lang.clone().unwrap_or_else(|| "other".into()))
            .or_default() += 1;
    }
    let mut symbols: Vec<_> = nodes
        .iter()
        .filter(|n| !matches!(n.kind, NodeKind::File | NodeKind::Dir))
        .collect();
    symbols.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.path.cmp(&b.path)));
    let calls: Vec<_> = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .take(500)
        .collect();
    let used: HashSet<uuid::Uuid> = calls.iter().flat_map(|e| [e.src_id, e.dst_id]).collect();
    let by_tool = store.call_log_summary()?;

    Ok(json!({
        "project": {
            "root": project_root,
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "files": files,
            "symbols": symbols.len(),
            "languages": languages,
        },
        "index_stats": {
            "nodes_by_kind": store.count_by_kind()?.into_iter().collect::<BTreeMap<_, _>>(),
            "edges_by_kind": store.count_edges_by_kind()?.into_iter().collect::<BTreeMap<_, _>>(),
            "total_nodes": nodes.len(),
            "total_edges": edges.len(),
        },
        "top_symbols": symbols.iter().take(50).map(|s| json!({
            "name": s.name, "kind": s.kind.to_string(), "path": s.path, "rank": s.rank,
            "tokens_est": s.tokens_est, "line_start": s.line_start, "line_end": s.line_end,
        })).collect::<Vec<_>>(),
        "call_graph": {
            "nodes": nodes.iter().filter(|n| used.contains(&n.id)).map(|n| json!({
                "id": n.id.to_string(), "name": n.name, "kind": n.kind.to_string(), "path": n.path, "rank": n.rank,
            })).collect::<Vec<_>>(),
            "edges": calls.iter().map(|e| json!({
                "source": e.src_id.to_string(), "target": e.dst_id.to_string(),
                "kind": e.kind.to_string(), "confidence": e.confidence.to_string(),
            })).collect::<Vec<_>>(),
        },
        "token_usage": {
            "calls": by_tool.iter().map(|(_, n, _)| n).sum::<i64>(),
            "tokens_out": by_tool.iter().map(|(_, _, t)| t).sum::<i64>(),
        },
    }))
}

/// Генерация отчёта в HTML.
pub fn report_html(store: &Store, project_root: &str) -> anyhow::Result<String> {
    store.load_closed_paths()?;
    let nodes = store.get_all_nodes()?;
    let edges = store.get_all_edges()?;
    let file_nodes: Vec<_> = nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    let mut languages: BTreeMap<String, usize> = BTreeMap::new();
    for f in &file_nodes {
        *languages
            .entry(f.lang.clone().unwrap_or_else(|| "other".into()))
            .or_default() += 1;
    }
    let mut symbols: Vec<_> = nodes
        .iter()
        .filter(|n| !matches!(n.kind, NodeKind::File | NodeKind::Dir))
        .collect();
    symbols.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.path.cmp(&b.path)));
    let calls: Vec<_> = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .take(500)
        .collect();
    let _used: HashSet<uuid::Uuid> = calls.iter().flat_map(|e| [e.src_id, e.dst_id]).collect();
    let _ = store.call_log_summary()?;

    let project_name = std::path::Path::new(project_root)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| project_root.to_string());

    let indexed_at = store
        .project_info()?
        .and_then(|(_, t)| t)
        .unwrap_or_else(|| "—".into());

    let mut out = String::new();
    out.push_str(&format!(
        r#"<!DOCTYPE html>
<html lang="ru">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Отчёт по проекту: {}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; line-height: 1.6; max-width: 1000px; margin: 0 auto; padding: 20px; color: #333; }}
        h1 {{ color: #1a1a2e; border-bottom: 2px solid #4a90d9; padding-bottom: 10px; }}
        h2 {{ color: #16213e; margin-top: 30px; border-bottom: 1px solid #e0e0e0; padding-bottom: 5px; }}
        h3 {{ color: #0f3460; }}
        code {{ background: #f4f4f4; padding: 2px 6px; border-radius: 3px; font-family: "SF Mono", Monaco, monospace; }}
        pre {{ background: #1e1e1e; color: #d4d4d4; padding: 15px; border-radius: 5px; overflow-x: auto; }}
        table {{ width: 100%; border-collapse: collapse; margin: 15px 0; }}
        th, td {{ padding: 10px; text-align: left; border-bottom: 1px solid #e0e0e0; }}
        th {{ background: #f5f5f5; font-weight: 600; }}
        tr:hover {{ background: #fafafa; }}
        .badge {{ display: inline-block; padding: 2px 8px; border-radius: 12px; font-size: 0.85em; font-weight: 500; }}
        .badge-certain {{ background: #e8f5e9; color: #2e7d32; }}
        .badge-likely {{ background: #fff3e0; color: #ef6c00; }}
        .badge-guess {{ background: #fce4ec; color: #c2185b; }}
        .stats {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 15px; margin: 20px 0; }}
        .stat-card {{ background: #f8f9fa; padding: 15px; border-radius: 8px; border-left: 4px solid #4a90d9; }}
        .stat-value {{ font-size: 1.5em; font-weight: 700; color: #1a1a2e; }}
        .stat-label {{ color: #666; font-size: 0.9em; }}
        .footer {{ margin-top: 40px; padding-top: 20px; border-top: 1px solid #e0e0e0; color: #888; font-size: 0.9em; }}
        ul {{ padding-left: 20px; }}
        li {{ margin: 5px 0; }}
        .cycle {{ background: #fff3e0; padding: 10px; border-radius: 5px; font-family: monospace; }}
    </style>
</head>
<body>
    <h1>Отчёт по проекту: {}</h1>
    <p>Сформирован: {} · Atlas {} · Индекс: fresh ({})</p>
"#,
        html_escape(&project_name),
        html_escape(&project_name),
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
        env!("CARGO_PKG_VERSION"),
        html_escape(&indexed_at)
    ));

    // README
    let readme = read_readme(store, std::path::Path::new(project_root))
        .map(|s| html_escape(&s))
        .unwrap_or_else(|| "README не найден; описание построено по индексу.".to_string());
    out.push_str(&format!(
        r#"
    <h2>Что это за проект</h2>
    <p>{}</p>
"#,
        readme.replace('\n', "<br>")
    ));

    // Stats
    let main_lang = languages
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|(l, _)| l.clone())
        .unwrap_or_else(|| "—".into());
    out.push_str(&format!(r#"
    <h2>Статистика</h2>
    <div class="stats">
        <div class="stat-card"><div class="stat-value">{}</div><div class="stat-label">Файлов</div></div>
        <div class="stat-card"><div class="stat-value">{}</div><div class="stat-label">Символов</div></div>
        <div class="stat-card"><div class="stat-value">{}</div><div class="stat-label">Узлов</div></div>
        <div class="stat-card"><div class="stat-value">{}</div><div class="stat-label">Связей</div></div>
    </div>
    <p>Основной язык: <strong>{}</strong></p>
"#, file_nodes.len(), symbols.len(), nodes.len(), edges.len(), html_escape(&main_lang)));

    // Languages
    if !languages.is_empty() {
        out.push_str(
            "<h2>Языки</h2><table><thead><tr><th>Язык</th><th>Файлов</th></tr></thead><tbody>",
        );
        for (lang, count) in languages {
            out.push_str(&format!(
                "<tr><td>{}</td><td>{}</td></tr>",
                html_escape(&lang),
                count
            ));
        }
        out.push_str("</tbody></table>");
    }

    // Structure
    out.push_str("<h2>Структура проекта</h2><ul>");
    let mut groups: BTreeMap<String, usize> = BTreeMap::new();
    for f in &file_nodes {
        let parts: Vec<&str> = f.path.split('/').collect();
        let depth = parts.len().saturating_sub(1).min(3);
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
    for (path, count) in groups {
        let depth = path.matches('/').count();
        out.push_str(&format!(
            "<li>{}<strong>{}</strong> — {} файлов</li>",
            "· ".repeat(depth + 1),
            html_escape(&path),
            count
        ));
    }
    out.push_str("</ul>");

    // Key modules
    out.push_str("<h2>Ключевые модули и точки входа</h2><ul>");
    for f in file_nodes.iter().take(10) {
        let count = symbols.iter().filter(|s| s.path == f.path).count();
        out.push_str(&format!(
            "<li><strong>{}</strong> — ранг {:.2}, символов {}</li>",
            html_escape(&f.path),
            f.rank,
            count
        ));
    }
    if !symbols.is_empty() {
        out.push_str("</ul><h3>Главные символы</h3><ul>");
        let mut top_symbols: Vec<_> = symbols.iter().take(8).collect();
        top_symbols.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.path.cmp(&b.path)));
        for s in top_symbols.iter().take(8) {
            out.push_str(&format!(
                "<li>{} (<code>{}</code>, {}:{})</li>",
                html_escape(&s.name),
                s.kind,
                html_escape(&s.path),
                s.line_start.unwrap_or(0)
            ));
        }
    }
    out.push_str("</ul>");

    // Dependencies
    out.push_str("<h2>Зависимости</h2>");
    let by_id: HashMap<uuid::Uuid, &atlas_store::Node> =
        file_nodes.iter().map(|f| (f.id, *f)).collect();
    let mut deps: BTreeMap<(String, String), usize> = BTreeMap::new();
    for e in edges.iter().filter(|e| e.kind == EdgeKind::Imports) {
        let (Some(src), Some(dst)) = (by_id.get(&e.src_id), by_id.get(&e.dst_id)) else {
            continue;
        };
        *deps
            .entry((src.path.clone(), dst.path.clone()))
            .or_default() += 1;
    }
    if deps.is_empty() {
        out.push_str("<p>Связей импорта между файлами не найдено.</p>");
    } else {
        out.push_str(
            "<table><thead><tr><th>Источник</th><th>Цель</th><th>Кол-во</th></tr></thead><tbody>",
        );
        for ((src, dst), n) in deps.iter().take(50) {
            out.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(src),
                html_escape(dst),
                n
            ));
        }
        out.push_str("</tbody></table>");
    }

    // Cycles
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (src, dst) in deps.keys() {
        graph.entry(src.clone()).or_default().push(dst.clone());
    }
    let cycles = find_cycles(&graph);
    if !cycles.is_empty() {
        out.push_str("<h3>Циклические зависимости</h3>");
        for cycle in cycles.iter().take(5) {
            out.push_str(&format!(
                "<div class=\"cycle\">{}</div>",
                cycle
                    .iter()
                    .map(|s| html_escape(s))
                    .collect::<Vec<_>>()
                    .join(" → ")
            ));
        }
    }

    // Activity
    out.push_str("<h2>Что было сделано</h2>");
    match store.files_by_mtime(10)? {
        recent if !recent.is_empty() => {
            out.push_str("<p>Git-история не подключена; ниже — самые свежие файлы по времени изменения.</p><ul>");
            for (path, mtime) in recent {
                out.push_str(&format!(
                    "<li><strong>{}</strong> — {}</li>",
                    html_escape(&path),
                    mtime
                ));
            }
            out.push_str("</ul>");
        }
        _ => out.push_str("<p>Данных об изменениях нет.</p>"),
    }

    // AI Usage
    out.push_str("<h2>Работа нейросети</h2>");
    let summary = store.call_log_summary()?;
    if summary.is_empty() {
        out.push_str("<p>Вызовов MCP ещё не было.</p>");
    } else {
        let calls: i64 = summary.iter().map(|(_, n, _)| *n).sum();
        let tokens: i64 = summary.iter().map(|(_, _, t)| *t).sum();
        out.push_str(&format!(
            "<p>Вызовов: <strong>{}</strong>, токенов ответов ~<strong>{}</strong></p>",
            calls, tokens
        ));
        out.push_str("<table><thead><tr><th>Инструмент</th><th>Вызовов</th><th>Токенов</th></tr></thead><tbody>");
        for (tool, n, t) in summary {
            out.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(&tool),
                n,
                t
            ));
        }
        out.push_str("</tbody></table>");
    }

    // Problems
    out.push_str("<h2>Проблемы и ограничения</h2><ul>");
    let no_lang: Vec<_> = file_nodes.iter().filter(|f| f.lang.is_none()).collect();
    out.push_str(&format!(
        "<li>Файлов без поддерживаемого языка: {}</li>",
        no_lang.len()
    ));
    let weak = edges
        .iter()
        .filter(|e| format!("{}", e.confidence) != "certain")
        .count();
    out.push_str(&format!("<li>Связей с низкой уверенностью: {}</li>", weak));
    let called: HashSet<uuid::Uuid> = edges
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                EdgeKind::Calls | EdgeKind::Tests | EdgeKind::Extends
            )
        })
        .map(|e| e.dst_id)
        .collect();
    let mut dead: Vec<_> = symbols
        .iter()
        .filter(|s| {
            matches!(s.kind, NodeKind::Function | NodeKind::Method)
                && !s.path.to_lowercase().contains("test")
                && !called.contains(&s.id)
        })
        .collect();
    dead.sort_by(|a, b| {
        b.tokens_est
            .cmp(&a.tokens_est)
            .then_with(|| a.path.cmp(&b.path))
    });
    if !dead.is_empty() {
        out.push_str("<li>Возможный мёртвый код (без входящих вызовов, топ-10):<ul>");
        for s in dead.iter().take(10) {
            out.push_str(&format!(
                "<li>{} (<code>{}</code>:{})</li>",
                html_escape(&s.name),
                html_escape(&s.path),
                s.line_start.unwrap_or(0)
            ));
        }
        out.push_str("</ul></li>");
    }
    out.push_str("</ul>");

    // Footer
    out.push_str(&format!(
        r#"
    <div class="footer">
        <p>Сгенерировано Atlas v{} · {}</p>
    </div>
</body>
</html>"#,
        env!("CARGO_PKG_VERSION"),
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    ));

    Ok(out)
}

/// HTML-экранирование для безопасной вставки в HTML.
/// Сущности собраны из частей: один раз редактор уже превратил их обратно в символы.
fn html_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for ch in text.chars() {
        match ch {
            '&' => out.push_str(concat!("&", "amp;")),
            '<' => out.push_str(concat!("&", "lt;")),
            '>' => out.push_str(concat!("&", "gt;")),
            '"' => out.push_str(concat!("&", "#34;")),
            '\'' => out.push_str(concat!("&", "#39;")),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_store::schema::{Confidence, Edge, EdgeKind, Node, NodeKind};
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

    #[test]
    fn html_report_escapes_project_data() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            "<script>alert(1)</script> a & b",
        )
        .unwrap();
        let store = Store::open(dir.path().join("test.db")).unwrap();
        let html = report_html(&store, &dir.path().to_string_lossy()).unwrap();
        assert!(!html.contains("<script"), "{html}");
        assert!(html.contains(concat!("&", "lt;script", "&", "gt;")));
        assert!(html.contains(concat!("a ", "&", "amp; b")));
    }
}
