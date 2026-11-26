//! Структурированные ответы для интерфейса: метод `atlas/ui`.
//!
//! Это не инструменты MCP: нейросеть их не видит, в `tools/list` они не входят
//! и в журнал вызовов не пишутся. Защита секретов действует та же.

use crate::budget::estimate_tokens;
use crate::tools::{normalize_rel, read_project_lines, ToolRegistry};
use anyhow::{anyhow, bail, Result};
use atlas_core::secrets::{mask_lines, mask_text};
use atlas_store::schema::{Edge, EdgeKind, Node, NodeKind};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

/// Сколько файлов отдавать интерфейсу: дерево и граф крупнее всё равно нечитаемы.
const MAX_FILES: usize = 5000;
const MAX_LINES: usize = 20_000;

fn node_json(n: &Node) -> Value {
    json!({
        "id": n.id.to_string(),
        "kind": n.kind.to_string(),
        "name": n.name,
        "path": n.path,
        "lang": n.lang,
        "line_start": n.line_start,
        "line_end": n.line_end,
        "signature": n.signature.as_deref().map(mask_text),
        "doc": n.doc_summary.as_deref().map(|d| mask_lines(d.lines(), true).join("\n")),
        "rank": n.rank,
        "tokens_est": n.tokens_est,
    })
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("нужен параметр {key}"))
}

impl ToolRegistry {
    pub fn ui(&self, project_root: &str, method: &str, args: &Value) -> Result<Value> {
        self.store.load_closed_paths()?;
        match method {
            "overview" => self.ui_overview(project_root),
            "search" => self.ui_search(args),
            "outline" => self.ui_outline(args),
            "read" => self.ui_read(project_root, args),
            "neighbors" => self.ui_neighbors(args),
            "status" => self.ui_status(),
            "map" => self.ui_map(project_root),
            other => bail!("неизвестный метод интерфейса: {other}"),
        }
    }

    /// Узел по идентификатору, `путь::имя` или пути файла.
    fn ui_node(&self, target: &str) -> Result<Node> {
        if let Ok(id) = Uuid::parse_str(target) {
            return self
                .store
                .get_node(&id)?
                .ok_or_else(|| anyhow!("узел не найден"));
        }
        if let Some((path, name)) = target.split_once("::") {
            return self
                .store
                .nodes_in_file(path)?
                .into_iter()
                .find(|n| n.kind != NodeKind::File && n.name == name)
                .ok_or_else(|| anyhow!("символ «{name}» не найден в {path}"));
        }
        self.store
            .find_file_node_by_path(&normalize_rel(target))?
            .ok_or_else(|| anyhow!("файл «{target}» не найден в индексе"))
    }

    /// Карта проекта в том виде, в каком её получает нейросеть; в журнал не пишется.
    fn ui_map(&self, project_root: &str) -> Result<Value> {
        let text = self.overview_text(project_root)?;
        let full_tokens: i64 = self
            .store
            .get_all_nodes()?
            .iter()
            .filter(|n| n.kind == NodeKind::File)
            .map(|n| n.tokens_est as i64)
            .sum();
        Ok(json!({
            "tokens": estimate_tokens(&text),
            "full_tokens": full_tokens,
            "text": text,
        }))
    }

    fn ui_overview(&self, project_root: &str) -> Result<Value> {
        let all = self.store.get_all_nodes()?;
        let mut files: Vec<&Node> = all.iter().filter(|n| n.kind == NodeKind::File).collect();
        files.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.path.cmp(&b.path)));
        let symbols = all
            .iter()
            .filter(|n| !matches!(n.kind, NodeKind::File | NodeKind::Dir))
            .count();
        let mut langs: BTreeMap<String, usize> = BTreeMap::new();
        for f in &files {
            *langs
                .entry(f.lang.clone().unwrap_or_else(|| "other".into()))
                .or_default() += 1;
        }
        let total_files = files.len();
        files.truncate(MAX_FILES);

        // Связи между файлами: вызовы и импорты, поднятые до уровня файла
        let file_ids: HashMap<&str, String> = files
            .iter()
            .map(|f| (f.path.as_str(), f.id.to_string()))
            .collect();
        let node_paths: HashMap<Uuid, &str> = all.iter().map(|n| (n.id, n.path.as_str())).collect();
        let mut seen = HashSet::new();
        let mut edges = Vec::new();
        for e in self.store.get_all_edges()? {
            if e.kind == EdgeKind::Contains {
                continue;
            }
            let ends = node_paths
                .get(&e.src_id)
                .and_then(|p| file_ids.get(p))
                .zip(node_paths.get(&e.dst_id).and_then(|p| file_ids.get(p)));
            if let Some((src, dst)) = ends {
                if src != dst && seen.insert((src.clone(), dst.clone(), e.kind.to_string())) {
                    edges.push(json!({
                        "source": src,
                        "target": dst,
                        "kind": e.kind.to_string(),
                        "confidence": e.confidence.to_string(),
                    }));
                }
            }
        }

        Ok(json!({
            "root": project_root,
            "files": total_files,
            "symbols": symbols,
            "langs": langs,
            "top": files.iter().map(|f| node_json(f)).collect::<Vec<_>>(),
            "edges": edges,
        }))
    }

    fn ui_search(&self, args: &Value) -> Result<Value> {
        let query = str_arg(args, "query")?;
        let limit = args
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .clamp(1, 100) as usize;
        let items: Vec<Value> = atlas_core::search(&self.store, query, limit)?
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                let mut v = node_json(&r.node);
                v["score"] = json!(limit - i.min(limit));
                v
            })
            .collect();
        Ok(json!({"query": query, "total": items.len(), "items": items}))
    }

    fn ui_outline(&self, args: &Value) -> Result<Value> {
        let path = normalize_rel(str_arg(args, "path")?);
        let symbols: Vec<Value> = self
            .store
            .nodes_in_file(&path)?
            .iter()
            .filter(|n| n.kind != NodeKind::File)
            .map(node_json)
            .collect();
        Ok(json!({"path": path, "symbols": symbols}))
    }

    /// `target` — файл или символ; `mode` — signature, doc, folded или full.
    fn ui_read(&self, project_root: &str, args: &Value) -> Result<Value> {
        let node = self.ui_node(str_arg(args, "target")?)?;
        let mode = args.get("mode").and_then(Value::as_str).unwrap_or("full");
        // Чтение с диска проверяет закрытые пути для любого режима
        let file = read_project_lines(&self.store, project_root, &node.path)?;
        let total = file.len();
        let (start, end) = match (node.kind == NodeKind::File, node.line_start, node.line_end) {
            (false, Some(s), Some(e)) => (s.max(1) as usize, (e.max(s) as usize).min(total)),
            _ => (1, total.min(MAX_LINES)),
        };

        let lines: Vec<(usize, String)> = match mode {
            "signature" | "doc" => self
                .store
                .nodes_in_file(&node.path)?
                .iter()
                .filter(|n| n.kind != NodeKind::File)
                .filter(|n| node.kind == NodeKind::File || n.id == node.id)
                .map(|n| {
                    let mut text = n.signature.clone().unwrap_or_else(|| n.name.clone());
                    if mode == "doc" {
                        if let Some(doc) = &n.doc_summary {
                            text = format!("{text}  — {}", mask_lines(doc.lines(), true).join(" "));
                        }
                    }
                    let text = mask_text(&text.replace('\n', " "));
                    (n.line_start.unwrap_or(0).max(0) as usize, text)
                })
                .collect(),
            "full" | "folded" => {
                let slice: Vec<(usize, &str)> =
                    (start..=end).map(|n| (n, file[n - 1].as_str())).collect();
                let slice = if mode == "folded" {
                    crate::tools::fold_by_indent(slice)
                } else {
                    slice
                };
                slice.into_iter().map(|(n, l)| (n, l.to_string())).collect()
            }
            other => bail!("неизвестный mode «{other}»"),
        };

        Ok(json!({
            "path": node.path,
            "lang": node.lang,
            "total": total,
            "start": start,
            "end": end,
            "lines": lines.iter().map(|(n, t)| json!({"line": n, "text": t})).collect::<Vec<_>>(),
        }))
    }

    fn ui_neighbors(&self, args: &Value) -> Result<Value> {
        let node = self.ui_node(str_arg(args, "target")?)?;
        let side = |edges: Vec<Edge>, outgoing: bool| -> Result<Vec<Value>> {
            let mut out = Vec::new();
            for e in edges
                .iter()
                .filter(|e| e.kind != EdgeKind::Contains)
                .take(100)
            {
                let other = if outgoing { e.dst_id } else { e.src_id };
                if let Some(other) = self.store.get_node(&other)? {
                    out.push(json!({
                        "kind": e.kind.to_string(),
                        "direction": if outgoing { "out" } else { "in" },
                        "target_id": other.id.to_string(),
                        "target_name": other.name,
                        "target_path": other.path,
                        "confidence": e.confidence.to_string(),
                    }));
                }
            }
            Ok(out)
        };
        Ok(json!({
            "node": node_json(&node),
            "outgoing": side(self.store.outgoing_edges(&node.id)?, true)?,
            "incoming": side(self.store.incoming_edges(&node.id)?, false)?,
        }))
    }

    /// Состояние индекса и журнал вызовов нейросети.
    fn ui_status(&self) -> Result<Value> {
        let indexed_at = self.store.project_info()?.and_then(|p| p.1);
        let by_tool: Vec<Value> = self
            .store
            .call_log_summary()?
            .into_iter()
            .map(|(tool, count, tokens_out)| json!({"tool": tool, "count": count, "tokens_out": tokens_out}))
            .collect();
        let recent: Vec<Value> = self
            .store
            .recent_calls(20)?
            .into_iter()
            .map(|(time, tool, tokens_out, _)| json!({"time": time, "tool": tool, "tokens_out": tokens_out}))
            .collect();
        Ok(json!({
            "indexed_at": indexed_at,
            "nodes": self.store.count_by_kind()?.into_iter().collect::<BTreeMap<_, _>>(),
            "edges": self.store.count_edges_by_kind()?.into_iter().collect::<BTreeMap<_, _>>(),
            "calls": {"by_tool": by_tool, "recent": recent},
        }))
    }
}
