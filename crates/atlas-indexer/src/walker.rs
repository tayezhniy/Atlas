use anyhow::Result;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// Опции обхода проекта (раздел 4.4 ТЗ: исключения).
#[derive(Debug, Clone)]
pub struct WalkOptions {
    pub root: PathBuf,
    pub exclude_hidden: bool,
    pub max_file_size: u64,
    pub respect_gitignore: bool,
    pub respect_atlasignore: bool,
    /// Пользовательские glob-исключения из настроек.
    pub custom_exclusions: Vec<String>,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            exclude_hidden: true,
            max_file_size: 1_048_576,
            respect_gitignore: true,
            respect_atlasignore: true,
            custom_exclusions: Vec::new(),
        }
    }
}

/// Результат обхода одного файла.
#[derive(Debug, Clone)]
pub struct WalkedFile {
    pub path: PathBuf,
    pub size: u64,
    pub language: Option<String>,
    /// Файл больше порога: индексируется как узел без разбора содержимого.
    pub too_large: bool,
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
        let mut builder = WalkBuilder::new(&self.options.root);
        builder
            .hidden(self.options.exclude_hidden)
            .ignore(self.options.respect_gitignore)
            .git_ignore(self.options.respect_gitignore)
            .git_global(self.options.respect_gitignore)
            .parents(true)
            .follow_links(false);
        if self.options.respect_atlasignore {
            builder.add_custom_ignore_filename(".atlasignore");
        }
        // Пользовательские исключения
        for pattern in &self.options.custom_exclusions {
            builder.add_ignore(pattern);
        }

        for entry in builder.build() {
            let entry = entry?;
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !metadata.is_file() {
                continue;
            }
            let rel = path
                .strip_prefix(&self.options.root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            if is_always_excluded(&rel) {
                continue;
            }
            // Проверяем пользовательские исключения
            if self.is_custom_excluded(&rel) {
                continue;
            }
            files.push(WalkedFile {
                path: path.to_path_buf(),
                size: metadata.len(),
                language: detect_language(path),
                too_large: metadata.len() > self.options.max_file_size,
            });
        }

        Ok(files)
    }
    
    /// Проверяет, соответствует ли путь пользовательским исключениям (glob).
    fn is_custom_excluded(&self, rel: &str) -> bool {
        use glob::Pattern;
        for pattern in &self.options.custom_exclusions {
            if let Ok(p) = Pattern::new(pattern) {
                if p.matches(rel) {
                    return true;
                }
            }
        }
        false
    }
}

/// Пути и имена, которые не индексируются никогда (раздел 4.4 и 10.1 ТЗ).
fn is_always_excluded(rel: &str) -> bool {
    let lower = rel.to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);

    // Секреты и ключи
    if name == ".env"
        || name.starts_with(".env.")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name.ends_with(".p12")
        || name.ends_with(".pfx")
        || name.ends_with(".keystore")
        || name.ends_with(".jks")
    {
        return true;
    }

    // Lock-файлы и сгенерированные бандлы
    if matches!(
        name,
        "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "cargo.lock"
            | "poetry.lock"
            | "go.sum"
            | "composer.lock"
            | "gemfile.lock"
    ) || name.ends_with(".min.js")
        || name.ends_with(".min.css")
        || name.ends_with(".map")
    {
        return true;
    }

    // Бинарные и архивные форматы
    const BINARY_EXT: [&str; 34] = [
        "png", "jpg", "jpeg", "gif", "webp", "ico", "bmp", "tiff", "svg", "pdf", "zip", "gz",
        "tar", "bz2", "7z", "rar", "exe", "dll", "so", "dylib", "a", "o", "class", "jar", "wasm",
        "woff", "woff2", "ttf", "otf", "mp3", "mp4", "mov", "avi", "db",
    ];
    if let Some(ext) = name.rsplit_once('.').map(|(_, e)| e) {
        if BINARY_EXT.contains(&ext) {
            return true;
        }
    }

    // Каталоги зависимостей и сборки (на случай, если .gitignore отсутствует)
    const EXCLUDED_DIRS: [&str; 10] = [
        "node_modules", "vendor", "target", "dist", "build", ".git", ".atlas", ".venv", "venv",
        "coverage",
    ];
    lower
        .split('/')
        .any(|part| EXCLUDED_DIRS.contains(&part))
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

    #[test]
    fn secrets_and_vendor_are_excluded() {
        assert!(is_always_excluded(".env"));
        assert!(is_always_excluded("config/.env.local"));
        assert!(is_always_excluded("deploy/server.pem"));
        assert!(is_always_excluded("node_modules/react/index.js"));
        assert!(is_always_excluded("Cargo.lock"));
        assert!(!is_always_excluded("src/main.rs"));
        assert!(!is_always_excluded("docs/README.md"));
    }
}
