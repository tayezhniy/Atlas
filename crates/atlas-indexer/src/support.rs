//! Общие помощники индексатора: хеши, пути, имена и разрешение импортов.
//!
//! Модуль существует, чтобы `indexer.rs` и `incremental.rs` не расходились в
//! поведении: раньше каждый нёс свою копию этих функций.

use atlas_langs::SymbolKind;
use atlas_store::schema::NodeKind;
use std::collections::HashMap;
use std::path::{Component, Path};
use uuid::Uuid;

/// Быстрый детерминированный хеш содержимого файла.
pub fn atlas_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Оценка размера в токенах: примерно один токен на 4 символа.
pub fn estimate_tokens(text: &str) -> i32 {
    (text.chars().count() / 4).max(1) as i32
}

/// Имена методов, которые есть почти у каждого типа: связь по такому имени — догадка.
pub fn is_generic_name(name: &str) -> bool {
    const GENERIC: [&str; 24] = [
        "new", "default", "from", "into", "clone", "fmt", "drop", "eq", "hash", "len", "get", "set",
        "push", "pop", "insert", "remove", "next", "iter", "map", "to_string", "as_str", "unwrap",
        "ok", "is_empty",
    ];
    GENERIC.contains(&name)
}

pub fn is_test_file(path: &str) -> bool {
    let p = path.to_lowercase();
    p.contains("test") || p.contains("spec")
}

pub fn is_generated_file(path: &str) -> bool {
    let p = path.to_lowercase();
    p.contains("generated") || p.contains("build/") || p.contains("target/")
}

/// Убирает `.` и `..` из пути, не обращаясь к файловой системе.
pub fn normalize_path(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::RootDir => {}
            other => parts.push(other.as_os_str().to_string_lossy().to_string()),
        }
    }
    parts.join("/")
}

/// Последний сегмент имени: `self.helper` -> `helper`, `a::b::run` -> `run`.
pub fn simple_name(callee: &str) -> String {
    let t = callee.trim();
    let t = t.split('<').next().unwrap_or(t);
    t.rsplit(['.', ':'])
        .next()
        .unwrap_or(t)
        .trim()
        .to_string()
}

pub fn map_symbol_kind(kind: &SymbolKind) -> NodeKind {
    match kind {
        SymbolKind::Function => NodeKind::Function,
        SymbolKind::Method => NodeKind::Method,
        SymbolKind::Class => NodeKind::Class,
        SymbolKind::Interface => NodeKind::Interface,
        SymbolKind::Constant => NodeKind::Constant,
        SymbolKind::Type => NodeKind::Type,
        SymbolKind::Field => NodeKind::Constant, // Fields are similar to constants
    }
}

fn path_parts(path: &Path) -> Vec<String> {
    normalize_path(path)
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Прогрессивные варианты модуля: `a::b::C` -> `a/b/C`, `a/b`, `a`.
fn module_candidates(segments: &[&str], prefix: &[String]) -> Vec<String> {
    let mut segs: Vec<&str> = segments
        .iter()
        .copied()
        .filter(|s| !s.is_empty() && *s != "*")
        .collect();
    // Отбрасываем обобщения и псевдонимы
    if let Some(pos) = segs.iter().position(|s| *s == "as") {
        segs.truncate(pos);
    }
    let mut out = Vec::new();
    for end in (1..=segs.len()).rev() {
        let mut parts: Vec<String> = prefix.to_vec();
        parts.extend(segs[..end].iter().map(|s| s.to_string()));
        out.push(parts.join("/"));
    }
    out
}

const SUFFIXES: [&str; 14] = [
    "",
    ".rs",
    "/mod.rs",
    "/lib.rs",
    "/main.rs",
    ".py",
    "/__init__.py",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    "/index.ts",
    "/index.tsx",
    "/index.js",
];

/// Разрешает цель импорта в ID узла файла.
///
/// Поддерживает Rust (`crate::`, `self::`, `super::`, `::`), Python (точки,
/// относительные `.pkg`), TypeScript/JavaScript (`./x`, `../x`). Внешние пакеты
/// (`react`, `std`) просто не находятся в индексе и пропускаются.
pub fn resolve_import_target(
    files: &HashMap<String, Uuid>,
    target: &str,
    from_path: &str,
) -> Option<Uuid> {
    let target = target.trim();
    if target.is_empty() || target == "*" {
        return None;
    }
    let from_dir = Path::new(from_path).parent().unwrap_or_else(|| Path::new(""));
    let mut bases: Vec<String> = Vec::new();

    if target.contains("::") {
        let mut segs: Vec<&str> = target.split("::").filter(|s| !s.is_empty()).collect();
        let mut prefix = path_parts(from_dir);
        match segs.first().copied() {
            Some("crate") => {
                segs.remove(0);
                // Корень крейта может быть как корнем проекта, так и src/
                bases.extend(module_candidates(&segs, &[]));
                bases.extend(module_candidates(&segs, &["src".to_string()]));
            }
            Some("self") => {
                segs.remove(0);
                bases.extend(module_candidates(&segs, &prefix));
            }
            Some("super") => {
                while segs.first() == Some(&"super") {
                    segs.remove(0);
                    prefix.pop();
                }
                bases.extend(module_candidates(&segs, &prefix));
            }
            _ => {
                bases.extend(module_candidates(&segs, &[]));
                if !prefix.is_empty() {
                    bases.extend(module_candidates(&segs, &prefix));
                }
                bases.extend(module_candidates(&segs, &["src".to_string()]));
            }
        }
    } else if target.starts_with('.') {
        // Python: одна точка — текущий пакет, две — на уровень выше, и т.д.
        let dots = target.chars().take_while(|c| *c == '.').count();
        let rest = target[dots..].trim_start_matches('/');
        let mut parts = path_parts(from_dir);
        for _ in 1..dots {
            parts.pop();
        }
        if !rest.is_empty() {
            parts.push(rest.replace('.', "/"));
        }
        bases.push(parts.join("/"));
    } else if target.starts_with("./") || target.starts_with("../") || target == "." || target == ".." {
        bases.push(normalize_path(&from_dir.join(target)));
    } else if target.contains('/') {
        bases.push(normalize_path(Path::new(target)));
    } else {
        // Python dotted module или TS bare-пакет. Внешние пакеты не найдутся — это нормально.
        bases.push(target.replace('.', "/"));
        if !from_dir.as_os_str().is_empty() {
            bases.push(normalize_path(&from_dir.join(target)));
        }
        bases.push(normalize_path(&Path::new("src").join(target)));
    }

    for base in bases {
        for suffix in SUFFIXES {
            let candidate = format!("{base}{suffix}");
            if Path::new(&candidate).extension().is_none() {
                continue;
            }
            if let Some(id) = files.get(&candidate) {
                return Some(*id);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(paths: &[&str]) -> HashMap<String, Uuid> {
        paths
            .iter()
            .map(|p| (p.to_string(), Uuid::new_v4()))
            .collect()
    }

    #[test]
    fn rust_crate_import_resolves_to_module_file() {
        let f = files(&["src/store.rs", "src/store/model.rs", "src/main.rs"]);
        assert_eq!(
            resolve_import_target(&f, "crate::store::Store", "src/main.rs"),
            f.get("src/store.rs").copied()
        );
        assert_eq!(
            resolve_import_target(&f, "crate::store::model::Row", "src/main.rs"),
            f.get("src/store/model.rs").copied()
        );
    }

    #[test]
    fn rust_super_and_self_resolve() {
        let f = files(&["src/a/mod.rs", "src/a/util.rs", "src/lib.rs"]);
        assert_eq!(
            resolve_import_target(&f, "self::util::helper", "src/a/mod.rs"),
            f.get("src/a/util.rs").copied()
        );
        assert_eq!(
            resolve_import_target(&f, "super::a::util::helper", "src/a/mod.rs"),
            f.get("src/a/util.rs").copied()
        );
    }

    #[test]
    fn python_and_ts_imports_resolve() {
        let f = files(&["pkg/a.py", "pkg/__init__.py", "web/x.ts", "web/index.ts"]);
        assert_eq!(
            resolve_import_target(&f, "pkg.a", "main.py"),
            f.get("pkg/a.py").copied()
        );
        assert_eq!(
            resolve_import_target(&f, ".a", "pkg/main.py"),
            f.get("pkg/a.py").copied()
        );
        assert_eq!(
            resolve_import_target(&f, "./x", "web/index.ts"),
            f.get("web/x.ts").copied()
        );
    }

    #[test]
    fn external_packages_are_ignored() {
        let f = files(&["src/main.rs"]);
        assert_eq!(resolve_import_target(&f, "std::collections", "src/main.rs"), None);
        assert_eq!(resolve_import_target(&f, "react", "web/a.tsx"), None);
    }
}
