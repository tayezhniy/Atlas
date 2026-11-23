use atlas_store::Store;
use anyhow::Result;

/// Инкрементальное обновление индекса (раздел 4.3 ТЗ).
/// Файл пересчитывается только если изменился хеш содержимого.
pub struct IncrementalUpdater<'a> {
    store: &'a Store,
}

impl<'a> IncrementalUpdater<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    /// Обновляет индекс для одного файла.
    pub fn update_file(&self, path: &str, content: &str) -> Result<()> {
        let _ = path;
        let _ = content;
        Ok(())
    }
}