//! Инкрементальное обновление индекса (раздел 4.3 ТЗ).
//!
//! Обновление одного файла делегируется тому же `ProjectIndexer`, что и полная
//! индексация: раньше здесь лежала копия логики, и поведение расходилось.

use crate::indexer::ProjectIndexer;
use crate::support::atlas_hash;
use anyhow::Result;
use atlas_store::db::Store;

/// Обновление индекса по изменённым файлам.
pub struct IncrementalUpdater<'a> {
    store: &'a mut Store,
    root_path: String,
}

impl<'a> IncrementalUpdater<'a> {
    pub fn new(store: &'a mut Store, root_path: &str) -> Self {
        Self {
            store,
            root_path: root_path.to_string(),
        }
    }

    /// Обновляет индекс для одного файла по уже прочитанному содержимому.
    /// Возвращает true, если файл был переиндексирован.
    pub fn update_file(&mut self, rel_path: &str, content: &str) -> Result<bool> {
        let new_hash = atlas_hash(content);
        if let Some(existing) = self.store.find_file_node_by_path(rel_path)? {
            if existing.hash.as_ref() == Some(&new_hash) {
                return Ok(false);
            }
        }
        let stats =
            ProjectIndexer::new(self.store, &self.root_path).index_content(rel_path, content)?;
        Ok(stats.files_indexed > 0)
    }

    /// Читает файл с диска (путь относительно корня проекта) и обновляет индекс,
    /// если содержимое изменилось. Удалённый файл убирается из индекса.
    pub fn update_file_from_disk(&mut self, rel_path: &str) -> Result<bool> {
        let full = std::path::Path::new(&self.root_path).join(rel_path);
        if !full.exists() {
            self.store.delete_nodes_by_path(rel_path)?;
            self.store.clear_empty_dirs()?;
            atlas_core::ranking::recompute_ranks(self.store)?;
            return Ok(true);
        }
        let content = std::fs::read_to_string(&full)?;
        self.update_file(rel_path, &content)
    }

    /// Обновляет набор файлов за один проход (пакет событий наблюдателя).
    pub fn update_files_from_disk(&mut self, rel_paths: &[String]) -> Result<usize> {
        let mut changed = 0;
        for rel in rel_paths {
            if self.update_file_from_disk(rel)? {
                changed += 1;
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_hash_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("i.db")).unwrap();
        let mut upd = IncrementalUpdater::new(&mut store, dir.path().to_str().unwrap());

        let content = "def foo():\n    return 1\n";
        assert!(
            upd.update_file("a.py", content).unwrap(),
            "первый раз индексируется"
        );
        assert!(
            !upd.update_file("a.py", content).unwrap(),
            "повторный — пропуск по хешу"
        );
    }

    #[test]
    fn changed_file_is_reindexed_and_count_stays_stable() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("i.db")).unwrap();
        let root = dir.path().to_str().unwrap().to_string();

        {
            let mut upd = IncrementalUpdater::new(&mut store, &root);
            upd.update_file("a.py", "def foo():\n    return 1\n")
                .unwrap();
        }
        let foo_before = store.symbols_by_name("foo").unwrap().len();
        assert_eq!(foo_before, 1);

        {
            let mut upd = IncrementalUpdater::new(&mut store, &root);
            assert!(upd
                .update_file("a.py", "def foo():\n    return 2\n")
                .unwrap());
        }
        assert_eq!(
            store.symbols_by_name("foo").unwrap().len(),
            1,
            "переиндексация не должна плодить дубли символов"
        );
    }

    #[test]
    fn deleted_file_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("i.db")).unwrap();
        let root = dir.path().to_str().unwrap().to_string();

        std::fs::write(dir.path().join("a.py"), "def foo():\n    return 1\n").unwrap();
        {
            let mut upd = IncrementalUpdater::new(&mut store, &root);
            assert!(upd.update_file_from_disk("a.py").unwrap());
        }
        assert!(store.find_file_node_by_path("a.py").unwrap().is_some());

        std::fs::remove_file(dir.path().join("a.py")).unwrap();
        {
            let mut upd = IncrementalUpdater::new(&mut store, &root);
            assert!(upd.update_file_from_disk("a.py").unwrap());
        }
        assert!(
            store.find_file_node_by_path("a.py").unwrap().is_none(),
            "удалённый файл уходит из индекса"
        );
    }
}
