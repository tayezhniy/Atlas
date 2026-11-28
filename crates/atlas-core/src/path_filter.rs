//! Единый фильтр путей на базе `ignore` crate.
//!
//! Используется обходом (walker), read в MCP и наблюдателем (watch) —
//! чтобы правила .gitignore / .atlasignore применялись согласованно везде.
//!
//! `ignore::gitignore::Gitignore` — тот же матчер, на котором построен `WalkBuilder`,
//! поэтому правила совпадают с обходом.

use anyhow::Result;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::Path;

/// Фильтр путей: .gitignore + .atlasignore + пользовательские glob-исключения.
#[derive(Debug, Clone)]
pub struct PathFilter {
    gitignore: Option<Gitignore>,
    atlasignore: Option<Gitignore>,
}

impl PathFilter {
    /// Строит фильтр из корня проекта.
    /// Читает .gitignore и .atlasignore из корня, если они есть.
    pub fn new(root: &Path) -> Result<Self> {
        let gitignore = Self::build_ignore(root, ".gitignore")?;
        let atlasignore = Self::build_ignore(root, ".atlasignore")?;
        Ok(Self {
            gitignore,
            atlasignore,
        })
    }

    fn build_ignore(root: &Path, filename: &str) -> Result<Option<Gitignore>> {
        let path = root.join(filename);
        if !path.exists() {
            return Ok(None);
        }
        let mut builder = GitignoreBuilder::new(root);
        builder.add(&path);
        let gi = builder.build()?;
        Ok(Some(gi))
    }

    /// Проверяет, исключён ли путь (относительный корню).
    /// Возвращает true, если путь должен быть пропущен.
    /// Проверяет сам путь и все родительские каталоги (для паттернов вида `private/`).
    pub fn is_ignored(&self, rel_path: &str) -> bool {
        let path = Path::new(rel_path);
        for ancestor in path.ancestors() {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            let is_dir = ancestor != path;
            if let Some(ref gi) = self.gitignore {
                if gi.matched(ancestor, is_dir).is_ignore() {
                    return true;
                }
            }
            if let Some(ref gi) = self.atlasignore {
                if gi.matched(ancestor, is_dir).is_ignore() {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn make_filter(root: &Path, atlasignore_content: &str) -> PathFilter {
        fs::write(root.join(".atlasignore"), atlasignore_content).unwrap();
        PathFilter::new(root).unwrap()
    }

    #[test]
    fn private_dir_excluded() {
        let dir = tempdir().unwrap();
        let f = make_filter(dir.path(), "private/\n");
        assert!(f.is_ignored("private/p.py"));
        assert!(f.is_ignored("src/private/p.py")); // private/ matches at any depth
        assert!(!f.is_ignored("src/p.py"));
    }

    #[test]
    fn leading_slash_anchored() {
        let dir = tempdir().unwrap();
        let f = make_filter(dir.path(), "/private/\n");
        assert!(f.is_ignored("private/p.py"));
        assert!(!f.is_ignored("src/private/p.py"));
    }

    #[test]
    fn filename_pattern() {
        let dir = tempdir().unwrap();
        let f = make_filter(dir.path(), "p.py\n");
        assert!(f.is_ignored("p.py"));
        assert!(f.is_ignored("src/p.py"));
    }

    #[test]
    fn double_star_pattern() {
        let dir = tempdir().unwrap();
        let f = make_filter(dir.path(), "**/private/\n");
        assert!(f.is_ignored("private/p.py"));
        assert!(f.is_ignored("src/private/p.py"));
        assert!(!f.is_ignored("src/p.py"));
    }

    #[test]
    fn plus_in_name() {
        let dir = tempdir().unwrap();
        let f = make_filter(dir.path(), "a+b/\n");
        assert!(f.is_ignored("a+b/p.py"));
        assert!(!f.is_ignored("ab/p.py"));
    }

    #[test]
    fn gitignore_respected() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        let f = PathFilter::new(dir.path()).unwrap();
        assert!(f.is_ignored("target/debug/app"));
        assert!(!f.is_ignored("src/main.rs"));
    }

    #[test]
    fn no_ignore_files() {
        let dir = tempdir().unwrap();
        let f = PathFilter::new(dir.path()).unwrap();
        assert!(!f.is_ignored("anything.rs"));
    }
}