//! Журнал вызовов нейросети (раздел 7.8, 10.3 ТЗ).
//!
//! Запись идёт через `Store::log_call` прямо из `ToolRegistry::execute`: у журнала нет
//! отдельного состояния, поэтому отдельная структура не нужна.
use atlas_store::Store;

/// Последние вызовы журнала в виде строк `время  инструмент  токены`.
pub fn recent_calls(store: &Store, limit: usize) -> anyhow::Result<Vec<String>> {
    let mut stmt = store
        .conn()
        .prepare("SELECT time, tool, tokens_out FROM call_log ORDER BY id DESC LIMIT ?1")?;
    let rows = stmt.query_map([limit as i64], |r| {
        Ok(format!(
            "{}  {}  {}",
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?
        ))
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
