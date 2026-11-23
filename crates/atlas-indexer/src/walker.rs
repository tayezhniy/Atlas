use anyhow::Result;
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;

/// Опции обхода проекта (раздел 4.4 ТЗ: исключения).
#[derive(Debug, Clone)]
pub struct WalkOptions {
    pub root: PathBuf,
    pub exclude_hidden: bool,
    pub max_file_size: u64, // байты, по умолчанию 1 МБ (раздел 4.4)
    pub respect_gitignore: bool,
    pub respect_atlasignore: bool,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            exclude_hidden: true,
            max_file_size: 1_048_576, // 1 МБ
            respect_gitignore: true,
            respect_atlasignore: true,
        }
    }
}

/// Результат обхода одного файла.
#[derive(Debug, Clone)]
pub struct WalkedFile {
    pub path: PathBuf,
    pub size: u64,
    pub language: Option<String>,
}

/// Обход файлов проекта с учётом исключений (раздел 4.4 ТЗ).
pub struct ProjectWalker {
    options: WalkOptions,
}

impl ProjectWalker {
    pub fn new(options: WalkOptions) -> Self {
        Self { options }
    }

    /// Обходит проект и возвращает список файлов для индексации.
    pub fn walk(&self) -> Result<Vec<WalkedFile>> {
        let mut files = Vec::new();

        let walker = WalkBuilder::new(&self.options.root)
            .hidden(self.options.exclude_hidden)
            .ignore(self.options.respect_gitignore)
            .parents(true)
            .build();

        for entry in walker {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;

            if !metadata.is_file() {
                continue;
            }

            if metadata.len() > self.options.max_file_size {
                continue;
            }

            let language = detect_language(path);
            files.push(WalkedFile {
                path: path.to_path_buf(),
                size: metadata.len(),
                language,
            });
        }

        Ok(files)
    }
}

/// Определяет язык по расширению файла.
fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?;
    match ext {
        "rs" => Some("rust".to_string()),
        "ts" | "tsx" => Some("typescript".to_string()),
        "js" | "jsx" => Some("javascript".to_string()),
        "py" => Some("python".to_string()),
        "go" => Some("go".to_string()),
        "md" => Some("markdown".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(
            detect_language(std::path::Path::new("src/main.rs")),
            Some("rust".to_string())
        );
        assert_eq!(
            detect_language(std::path::Path::new("src/main.ts")),
            Some("typescript".to_string())
        );
        assert_eq!(
            detect_language(std::path::Path::new("README.md")),
            Some("markdown".to_string())
        );
        assert_eq!(detect_language(std::path::Path::new("data.bin")), None);
    }
}