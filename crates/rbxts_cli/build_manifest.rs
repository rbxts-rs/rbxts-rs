//! The build script's reading of a crate manifest (`build.rs`, [`workspace_dependencies`]): a minimal TOML reading of
//! the dependency tables, without a TOML crate (none is in the build scripts' dependency tree). Shared with the table
//! test in `tests/build_manifest.rs`.

/// The `rbxts_*` package names of a crate manifest's `[dependencies]` and `[build-dependencies]` (target-specific ones
/// included), in the order they first appear. Read as TOML is written: comments after a header or a value, spaces in
/// a header (`[ dependencies ]`), dotted tables (`[dependencies.rbxts_x]`), dotted keys (`rbxts_x.workspace = true`),
/// quoted keys, and renamed packages (`foo = { package = "rbxts_x" }`, or `package = "rbxts_x"` in a dotted table),
/// which count by their package name. Multi-line values and multi-line strings are not read (a dependency entry is
/// one line).
pub fn workspace_dependencies(manifest: &str) -> Vec<String> {
    // per dependency key of the current table set: (key, package name when renamed)
    let mut dependencies: Vec<(String, Option<String>)> = Vec::new();
    let mut note = |key: &str, package: Option<String>| match dependencies.iter_mut().find(|(k, _)| k == key) {
        Some((_, known)) => {
            if package.is_some() {
                *known = package;
            }
        }
        None => dependencies.push((key.to_owned(), package)),
    };
    // where the lines are: in a dependency table, in one dependency's own table (`[dependencies.<key>]`), or elsewhere
    enum Table {
        Other,
        Dependencies,
        Dependency(String),
    }
    let mut table = Table::Other;
    let mut names = Vec::new();
    for line in manifest.lines() {
        let line = line.trim();
        if let Some(header) = line.strip_prefix('[') {
            table = Table::Other;
            // an array of tables (`[[bin]]`) is never a dependency table
            if header.starts_with('[') {
                continue;
            }
            let Some((keys, rest)) = dotted_key(header) else { continue };
            if !rest.trim_start().starts_with(']') {
                continue;
            }
            let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
            let at = match keys.as_slice() {
                ["target", _, rest @ ..] => rest,
                rest => rest,
            };
            match at {
                [kind] if is_dependency_table(kind) => table = Table::Dependencies,
                [kind, key] if is_dependency_table(kind) => {
                    note(key, None);
                    table = Table::Dependency((*key).to_owned());
                }
                _ => {}
            }
            continue;
        }
        let Some((keys, rest)) = dotted_key(line) else { continue };
        let Some(value) = rest.trim_start().strip_prefix('=') else { continue };
        let value = value.trim_start();
        match (&table, keys.as_slice()) {
            (Table::Dependencies, [key]) => note(key, inline_package(value)),
            (Table::Dependencies, [key, field]) => note(key, (field == "package").then(|| string(value)).flatten()),
            (Table::Dependencies, [key, ..]) => note(key, None),
            (Table::Dependency(key), [field]) if field == "package" => note(key, string(value)),
            _ => {}
        }
    }
    for (key, package) in dependencies {
        let name = package.unwrap_or(key);
        if name.starts_with("rbxts_") && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

fn is_dependency_table(name: &str) -> bool {
    matches!(name, "dependencies" | "build-dependencies" | "build_dependencies")
}

/// A TOML key at the start of `text` (whitespace around the dots allowed): its parts, unquoted, and the text after it.
/// `None` when `text` does not start with a key.
fn dotted_key(text: &str) -> Option<(Vec<String>, &str)> {
    let mut keys = Vec::new();
    let mut rest = text.trim_start();
    loop {
        let (key, after) = simple_key(rest)?;
        keys.push(key);
        let after_space = after.trim_start();
        match after_space.strip_prefix('.') {
            Some(next) => rest = next.trim_start(),
            None => return Some((keys, after)),
        }
    }
}

/// One key part: bare (`A-Za-z0-9_-`), a basic string or a literal string.
fn simple_key(text: &str) -> Option<(String, &str)> {
    if text.starts_with(['"', '\'']) {
        return quoted(text);
    }
    let end = text.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')).unwrap_or(text.len());
    (end > 0).then(|| (text[..end].to_owned(), &text[end..]))
}

/// A basic (`"…"`, with escapes) or literal (`'…'`) string at the start of `text`: its value and the text after it.
fn quoted(text: &str) -> Option<(String, &str)> {
    let quote = text.chars().next()?;
    let mut value = String::new();
    let mut chars = text.char_indices().skip(1);
    while let Some((i, c)) = chars.next() {
        match c {
            c if c == quote => return Some((value, &text[i + 1..])),
            '\\' if quote == '"' => {
                let (_, escaped) = chars.next()?;
                value.push(match escaped {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    other => other,
                });
            }
            c => value.push(c),
        }
    }
    None
}

/// The string a value starts with, if it is one.
fn string(value: &str) -> Option<String> {
    quoted(value.trim_start()).map(|(s, _)| s)
}

/// `package = "…"` of an inline table value (`{ package = "rbxts_x", workspace = true }`).
fn inline_package(value: &str) -> Option<String> {
    let mut rest = value.strip_prefix('{')?;
    let mut package = None;
    loop {
        rest = rest.trim_start();
        if rest.starts_with('}') || rest.is_empty() {
            return package;
        }
        let (keys, after) = dotted_key(rest)?;
        let after = after.trim_start().strip_prefix('=')?.trim_start();
        let after = if after.starts_with(['"', '\'']) {
            let (text, after) = quoted(after)?;
            if keys.len() == 1 && keys[0] == "package" {
                package = Some(text);
            }
            after
        } else {
            skip_value(after)
        };
        rest = after.trim_start();
        rest = rest.strip_prefix(',').unwrap_or(rest);
    }
}

/// The text after a value that is not a string: an array or inline table (balanced, strings skipped), or a bare value
/// (a number, a boolean, a date) up to the next `,` or `}`.
fn skip_value(text: &str) -> &str {
    let mut depth = 0usize;
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        match c {
            '"' | '\'' => match quoted(rest) {
                Some((_, after)) => {
                    rest = after;
                    continue;
                }
                None => return "",
            },
            '[' | '{' => depth += 1,
            ']' | '}' if depth > 0 => depth -= 1,
            ',' | '}' if depth == 0 => return rest,
            _ => {}
        }
        rest = &rest[c.len_utf8()..];
    }
    rest
}
