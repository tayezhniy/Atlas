//! Защита секретов в ответах (раздел 10.1 ТЗ).
//!
//! Маскируется только само значение: отступы и остальной код строки не меняются.
//! Это эвристика, а не гарантия: секрет под нейтральным именем она не распознает.

const MASK: &str = "***";

/// Префиксы известных форматов ключей: маскируются всегда.
const TOKEN_PREFIXES: [&str; 13] = [
    "sk-", "sk_live_", "AKIA", "ASIA", "AIza", "ghp_", "gho_", "ghs_", "github_pat_", "glpat-",
    "xoxb-", "xoxp-", "eyJ",
];

/// Пути, содержимое которых не отдаётся совсем.
pub fn is_closed_path(path: &str) -> bool {
    let p = path.replace('\\', "/").to_lowercase();
    let name = p.rsplit('/').next().unwrap_or(&p);
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
    // `secrets.rs` и `CredentialsForm.tsx` — обычный код; значения в нём маскируются
    let source = matches!(
        ext,
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "go" | "java" | "cs" | "c" | "h" | "cpp" | "kt" | "swift" | "md"
    );
    let secret_data = !source
        && (name.contains("credentials")
            || name.starts_with("secret.")
            || name.starts_with("secrets.")
            || name.starts_with("service-account")
            || p.split('/').any(|part| part == "secrets" || part == "credentials"));
    secret_data
        || name == ".env"
        || name.starts_with(".env.")
        || name.starts_with("env.local")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || matches!(name, ".envrc" | ".npmrc" | ".netrc" | ".pypirc")
        || ["env", "pem", "key", "p12", "pfx", "keystore", "jks", "ppk", "tfstate"].contains(&ext)
        || p.split('/').any(|part| matches!(part, ".atlas" | ".git" | ".aws" | ".kube" | ".ssh"))
        || p.ends_with(".docker/config.json")
}

/// Файл настроек или текста: в нём значения секретов часто пишут без кавычек.
pub fn is_config_path(path: &str) -> bool {
    let p = path.to_lowercase();
    let name = p.rsplit('/').next().unwrap_or(&p);
    match name.rsplit_once('.') {
        None => true,
        Some((_, ext)) => matches!(
            ext,
            "yaml" | "yml" | "toml" | "ini" | "cfg" | "conf" | "properties" | "sh" | "txt" | "md" | "env"
        ),
    }
}

/// Похоже ли имя на имя секрета. `max_tokens` и `token_count` секретами не считаются.
fn is_secret_name(ident: &str) -> bool {
    let lower = ident.to_lowercase().replace('-', "_");
    [
        "password", "passwd", "db_pass", "secret", "api_key", "apikey", "private_key", "privatekey",
        "authorization", "bearer", "access_key",
    ]
    .iter()
    .any(|k| lower.contains(k))
        || lower.ends_with("token")
        || lower == "pwd"
        || lower.ends_with("_pwd")
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

fn has_secret_name(text: &str) -> bool {
    text.split(|c: char| !is_ident_char(c)).any(is_secret_name)
}

/// Строковые литералы строки: байтовые границы содержимого (без кавычек).
fn string_literals(line: &str) -> Vec<(usize, usize)> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let q = bytes[i];
        // `&'a` и `<'a` — времена жизни Rust, а не начало строки
        let lifetime = q == b'\'' && i > 0 && matches!(bytes[i - 1], b'&' | b'<');
        if !matches!(q, b'"' | b'\'' | b'`') || lifetime {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && bytes[j] != q {
            j += if bytes[j] == b'\\' { 2 } else { 1 };
        }
        if j < bytes.len() {
            out.push((i + 1, j));
            i = j + 1;
        } else {
            i += 1; // незакрытая кавычка (апостроф в комментарии)
        }
    }
    out
}

/// Присваивается ли литерал, открывающийся после `before`, чему-то с именем секрета.
fn assigned_to_secret(before: &str) -> bool {
    // префиксы строк вроде r", b", f", r#"
    let head = before
        .trim_end_matches(['r', 'b', 'f', 'u', '#'])
        .trim_end();
    // значение по умолчанию: env.get("DB_PASSWORD") or "значение"
    if let Some(rest) = [" or", "||", "??"].iter().find_map(|op| head.strip_suffix(op)) {
        return has_secret_name(rest);
    }
    if let Some(rest) = head.strip_suffix(',') {
        // set("password", "значение"), но не список ["password", "username"]
        let rest = rest.trim_end().trim_end_matches(['"', '\'']);
        let start = rest.rfind(|c: char| !is_ident_char(c)).map_or(0, |p| p + 1);
        let called = rest[..start].trim_end_matches(['"', '\'']).ends_with('(');
        return called && is_secret_name(&rest[start..]);
    }
    let Some(lhs) = head.strip_suffix("=>").or_else(|| head.strip_suffix(['=', ':'])) else {
        return false;
    };
    let lhs = lhs.trim_end_matches(['=', '!', ':', '+']);
    // левая часть — от ближайшего разделителя выражений
    let start = lhs.rfind([';', '{', '(', ',']).map_or(0, |p| p + 1);
    has_secret_name(&lhs[start..])
}

/// `ключ=значение` без кавычек: в настройках и комментариях.
/// `whole` — значение занимает всю строку и может содержать пробелы.
fn unquoted_value(line: &str, whole: bool) -> Option<(usize, usize)> {
    for (pos, sep) in line.char_indices().filter(|(_, c)| matches!(c, '=' | ':')) {
        let key = line[..pos]
            .trim_end()
            .rsplit(|c: char| !(is_ident_char(c) || c == '.'))
            .next()
            .unwrap_or("");
        if !key.split('.').any(is_secret_name) {
            continue;
        }
        let after = &line[pos + sep.len_utf8()..];
        let mut value = after.trim_start();
        if let Some(rest) = value.strip_prefix("Bearer ") {
            value = rest.trim_start();
        }
        let start = line.len() - value.len();
        let len = if whole {
            value.find(" #").unwrap_or(value.len())
        } else {
            value.find(char::is_whitespace).unwrap_or(value.len())
        };
        let token = value[..len].trim_end_matches(['"', '\'', ';', ',', ' ']);
        let reference = token.starts_with("${") || token.starts_with("{{");
        if token.len() >= 4 && !reference && !token.starts_with(['"', '\'', '`', '<', '|', '>']) {
            return Some((start, start + token.len()));
        }
    }
    None
}

/// Пароль в адресе вида `схема://пользователь:пароль@хост`.
fn url_passwords(line: &str) -> Vec<(usize, usize)> {
    line.match_indices("://")
        .filter_map(|(pos, sep)| {
            let from = pos + sep.len();
            let rest = &line[from..];
            let end = rest.find(|c: char| c == '/' || c.is_whitespace() || c == '"').unwrap_or(rest.len());
            let at = rest[..end].rfind('@')?;
            let colon = rest[..at].find(':')?;
            let password = &rest[colon + 1..at];
            (!password.is_empty() && !password.starts_with(['$', '{']))
                .then_some((from + colon + 1, from + at))
        })
        .collect()
}

fn known_tokens(line: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for prefix in TOKEN_PREFIXES {
        for (pos, _) in line.match_indices(prefix) {
            // `task-runner` не должен считаться ключом `sk-`
            if line[..pos].chars().next_back().is_some_and(char::is_alphanumeric) {
                continue;
            }
            let len = line[pos..]
                .find(|c: char| !(is_ident_char(c) || (prefix == "eyJ" && c == '.')))
                .unwrap_or(line.len() - pos);
            let token = &line[pos..pos + len];
            // `.sk-spinner-wrapper` — не ключ: в настоящих ключах есть цифры или
            // заглавные, либо это длинный сплошной хвост без дефисов
            let tail = &token[prefix.len()..];
            let random = prefix != "sk-"
                || tail.chars().any(|c| c.is_ascii_digit() || c.is_ascii_uppercase())
                || (tail.len() >= 16 && !tail.contains('-'));
            let jwt = prefix != "eyJ" || token.matches('.').count() == 2;
            if len >= prefix.len() + 6 && random && jwt {
                out.push((pos, pos + len));
            }
        }
    }
    out
}

/// Маскирует значения, похожие на секреты, в одной строке.
pub fn mask_line(line: &str, config: bool) -> String {
    let mut ranges = known_tokens(line);
    ranges.extend(url_passwords(line));
    for (start, end) in string_literals(line) {
        let value = &line[start..end];
        if value.len() >= 4
            && !value.starts_with("${")
            // имя переменной окружения (SECRET_KEY) — не секрет
            && !(value.contains('_') && value.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
            && value != MASK
            && assigned_to_secret(&line[..start - 1])
        {
            ranges.push((start, end));
        }
    }
    let trimmed = line.trim_start();
    let comment = ["//", "# ", "#!", "##", "/*", "* ", "-- "].iter().any(|m| trimmed.starts_with(m));
    if config {
        ranges.extend(unquoted_value(line, !trimmed.starts_with(['#', '-', 'e', 'E'])));
    } else if comment {
        ranges.extend(unquoted_value(line, false));
    } else if let Some(at) = [" //", " # ", " /*"].iter().filter_map(|m| line.find(m)).min() {
        ranges.extend(unquoted_value(&line[at..], false).map(|(s, e)| (s + at, e + at)));
    }
    if ranges.is_empty() {
        return line.to_string();
    }

    ranges.sort_unstable();
    let mut out = String::with_capacity(line.len());
    let mut done = 0;
    for (start, end) in ranges {
        if start >= done {
            out.push_str(&line[done..start]);
            out.push_str(MASK);
            done = end;
        } else {
            done = done.max(end);
        }
    }
    out.push_str(&line[done..]);
    out
}

/// Маскирует многострочный текст, включая тела PEM-ключей. Число строк сохраняется.
pub fn mask_lines<'a>(lines: impl Iterator<Item = &'a str>, config: bool) -> Vec<String> {
    let mut in_key = false;
    lines
        .map(|line| {
            if in_key {
                let body = line.trim().trim_matches(['"', '\'', '+', ',']).trim_end_matches("\\n");
                // тело ключа — base64 и заголовки; всё прочее означает, что ключ кончился
                let base64 = body.chars().all(|c| c.is_ascii_alphanumeric() || "+/=".contains(c));
                if line.contains("-----END") {
                    in_key = false;
                    return mask_line(line, config);
                }
                if base64 || body.starts_with("Proc-Type:") || body.starts_with("DEK-Info:") {
                    return if body.is_empty() { line.to_string() } else { MASK.to_string() };
                }
                in_key = false;
            }
            // начало ключа — только если после маркера в строке ничего существенного
            if line.contains("-----BEGIN") {
                if let Some((_, tail)) = line.split_once("PRIVATE KEY-----") {
                    in_key = matches!(tail.trim(), "" | "\\n" | "\"" | "'" | "`");
                }
            }
            mask_line(line, config)
        })
        .collect()
}

/// Маскирует готовый текст ответа (сигнатуры, докстринги, отчёт).
pub fn mask_text(text: &str) -> String {
    let mut out = mask_lines(text.split('\n'), false).join("\n");
    // split сохраняет завершающий перевод строки пустым элементом
    if out.is_empty() {
        out = text.to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_secrets_and_keeps_the_rest() {
        for (input, expected) in [
            (r#"const TOKEN: &str = "hunter2hunter2";"#, r#"const TOKEN: &str = "***";"#),
            (r#"    let password = "SuperSecret123"; // fn x("#, r#"    let password = "***"; // fn x("#),
            (r#"const password = "SuperSecret123"; run(() => 1);"#, r#"const password = "***"; run(() => 1);"#),
            (r#"const cfg = { "password": "SuperSecret123" };"#, r#"const cfg = { "password": "***" };"#),
            (r#"const K = "sk-abcdefghijklmnop"; go(x => x);"#, r#"const K = "***"; go(x => x);"#),
            (r#"const F: fn() = || {}; const API_KEY: &str = "abc123456789";"#, r#"const F: fn() = || {}; const API_KEY: &str = "***";"#),
            (r#"connect(password="qwerty123")"#, r#"connect(password="***")"#),
            (r#"cfg.set("api_key", "abc123456789")"#, r#"cfg.set("api_key", "***")"#),
            (r#"if password == "letmein1" {"#, r#"if password == "***" {"#),
            ("/// db password=SuperSecret123", "/// db password=***"),
            (r#"headers = {"Authorization": "Bearer abc.def.ghi"}"#, r#"headers = {"Authorization": "***"}"#),
        ] {
            assert_eq!(mask_line(input, false), expected);
        }
        assert_eq!(mask_line("  password: hunter2hunter2", true), "  password: ***");
        assert_eq!(mask_line("export API_KEY=abc123456", true), "export API_KEY=***");
    }

    #[test]
    fn leaves_ordinary_code_alone() {
        for line in [
            "pub fn connect(password: &str) -> bool { helper() }",
            "fn f<'a>(password: &'a str, token: &'a str) {}",
            "    let tokens = n * 2;",
            "    let max_tokens: usize = 800;",
            r#"let token_count = "12345";"#,
            "let password = read_password()?;",
            "    pub password: String,",
            r#"const NAME: &str = "task-runner-service";"#,
            r#"label = "Введите пароль""#,
        ] {
            assert_eq!(mask_line(line, false), line);
        }
    }

    #[test]
    fn handles_review_cases() {
        for (input, config) in [
            ("x = 1  # password=LEAK1234", false),
            ("SECRET_KEY=$LEAK1234", true),
            ("password: correct horse LEAK1234", true),
            ("DATABASE_URL=postgres://u:LEAK1234@host/db", true),
            ("pwd = \"LEAK1234\"", false),
            ("os.environ.get('DB_PASSWORD') or 'LEAK1234'", false),
            ("const p = process.env.DB_PASSWORD || \"LEAK1234\"", false),
            ("k = \"glpat-LEAK1234abcd\"", false),
        ] {
            let out = mask_line(input, config);
            assert!(!out.contains("LEAK1234"), "{input} -> {out}");
        }
        for line in [
            "SECRET_KEY = os.getenv(\"SECRET_KEY\")",
            "fields = [\"password\", \"username\"]",
            "    *token = next_token(s);",
            ".sk-spinner-wrapper { color: red }",
            ".sk-spinner { color: red }",
            "password: ${DB_PASSWORD}",
        ] {
            assert_eq!(mask_line(line, false), line);
        }
        let code = "if \"-----BEGIN PRIVATE KEY-----\" in s:\n    return True";
        assert_eq!(mask_text(code), code);
        assert!(!is_closed_path("crates/atlas-core/src/secrets.rs"));
        for path in [".envrc", "prod.env", "secrets/db.yaml", "config/credentials.json", ".aws/config"] {
            assert!(is_closed_path(path), "{path}");
        }
    }

    #[test]
    fn masks_private_key_body() {
        let text = "-----BEGIN RSA PRIVATE KEY-----\nMIIEow\n-----END RSA PRIVATE KEY-----\nok";
        assert_eq!(
            mask_text(text),
            "-----BEGIN RSA PRIVATE KEY-----\n***\n-----END RSA PRIVATE KEY-----\nok"
        );
    }
}
