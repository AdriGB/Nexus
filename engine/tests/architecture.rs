use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Strip line comments (//), block comments (/* */ nested), and string/char
/// literals so dependency checks cannot be bypassed via comment or string.
/// Newlines are preserved to keep line numbers accurate.
fn strip_comments_and_strings(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut block_depth: usize = 0;
    let mut in_line_comment = false;
    let mut in_string = false;
    let mut in_char = false;
    let mut in_raw_string_hashes: Option<usize> = None;
    let mut escape = false;

    while i < chars.len() {
        let c = chars[i];
        let next = if i + 1 < chars.len() {
            Some(chars[i + 1])
        } else {
            None
        };

        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
                out.push('\n');
            } else {
                out.push(' ');
            }
            i += 1;
            continue;
        }

        if block_depth > 0 {
            if c == '/' && next == Some('*') {
                block_depth += 1;
                out.push(' ');
                out.push(' ');
                i += 2;
                continue;
            }
            if c == '*' && next == Some('/') {
                block_depth -= 1;
                out.push(' ');
                out.push(' ');
                i += 2;
                continue;
            }
            if c == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
            i += 1;
            continue;
        }

        if let Some(hashes) = in_raw_string_hashes {
            if c == '"' {
                let mut k = 0;
                while k < hashes && i + 1 + k < chars.len() && chars[i + 1 + k] == '#' {
                    k += 1;
                }
                if k == hashes {
                    out.push(' ');
                    for _ in 0..hashes {
                        out.push(' ');
                    }
                    in_raw_string_hashes = None;
                    i += 1 + hashes;
                    continue;
                }
            }
            if c == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
            i += 1;
            continue;
        }

        if in_string {
            if escape {
                escape = false;
                out.push(' ');
                i += 1;
                continue;
            }
            if c == '\\' {
                escape = true;
                out.push(' ');
                i += 1;
                continue;
            }
            if c == '"' {
                in_string = false;
                out.push(' ');
                i += 1;
                continue;
            }
            if c == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
            i += 1;
            continue;
        }

        if in_char {
            if escape {
                escape = false;
                out.push(' ');
                i += 1;
                continue;
            }
            if c == '\\' {
                escape = true;
                out.push(' ');
                i += 1;
                continue;
            }
            if c == '\'' {
                in_char = false;
                out.push(' ');
                i += 1;
                continue;
            }
            if c == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
            i += 1;
            continue;
        }

        if c == 'r' {
            let mut hashes = 0;
            let mut j = i + 1;
            while j < chars.len() && chars[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < chars.len() && chars[j] == '"' {
                in_raw_string_hashes = Some(hashes);
                out.push(' ');
                for _ in 0..hashes {
                    out.push(' ');
                }
                out.push(' ');
                i = j + 1;
                continue;
            }
        }
        if c == '/' && next == Some('/') {
            in_line_comment = true;
            out.push(' ');
            out.push(' ');
            i += 2;
            continue;
        }
        if c == '/' && next == Some('*') {
            block_depth = 1;
            out.push(' ');
            out.push(' ');
            i += 2;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(' ');
            i += 1;
            continue;
        }
        if c == '\'' {
            in_char = true;
            out.push(' ');
            i += 1;
            continue;
        }

        out.push(c);
        i += 1;
    }

    out
}

fn read_stripped(path: &Path) -> String {
    let raw = fs::read_to_string(path).unwrap_or_default();
    strip_comments_and_strings(&raw)
}

/// Map every source file to its absolute crate module path, e.g.
/// `src/simulation/autonomy/mod.rs` -> `["simulation", "autonomy"]`.
fn build_module_map() -> Vec<(PathBuf, Vec<String>)> {
    let mut map: Vec<(PathBuf, Vec<String>)> = Vec::new();
    let src_root = Path::new("src");
    let lib = src_root.join("lib.rs");
    if !lib.exists() {
        return map;
    }
    visit(&lib, Vec::new(), &mut map);
    map
}

fn visit(file: &Path, mod_path: Vec<String>, map: &mut Vec<(PathBuf, Vec<String>)>) {
    if map.iter().any(|(p, _)| p == file) {
        return;
    }
    map.push((file.to_path_buf(), mod_path.clone()));

    let content = read_stripped(file);
    for child in extract_mods(&content) {
        let dir = file.parent().unwrap_or_else(|| Path::new(""));
        let as_file = dir.join(format!("{}.rs", child));
        let as_mod = dir.join(&child).join("mod.rs");
        let child_path = if as_file.exists() { as_file } else { as_mod };
        if child_path.exists() {
            let mut np = mod_path.clone();
            np.push(child);
            visit(&child_path, np, map);
        }
    }
}

/// Extract `mod foo;` declarations from stripped file content.
fn extract_mods(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if !t.ends_with(';') {
            continue;
        }
        let idx = match t.find("mod ") {
            Some(i) if i == 0 || t.as_bytes()[i - 1] == b' ' || t.as_bytes()[i - 1] == b'\t' => i,
            _ => continue,
        };
        let after = &t[idx + 4..];
        let name: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            out.push(name);
        }
    }
    out
}

/// Parse a single `use` tree into absolute crate paths (segments).
/// `current` is the module path of the file containing the `use`.
fn parse_use_tree(tree: &str, current: &[String]) -> Vec<Vec<String>> {
    let tree = tree.trim();
    let (base, rest) = resolve_qualifier(tree, current);
    let mut out = Vec::new();
    walk_use_tree(rest, &base, &mut out);
    out
}

fn resolve_qualifier<'a>(tree: &'a str, current: &[String]) -> (Vec<String>, &'a str) {
    if let Some(rest) = tree.strip_prefix("crate") {
        let rest = rest.strip_prefix("::").unwrap_or(rest);
        (Vec::new(), rest)
    } else if let Some(rest) = tree.strip_prefix("self") {
        let rest = rest.strip_prefix("::").unwrap_or(rest);
        (current.to_vec(), rest)
    } else if tree.starts_with("super") {
        let mut count = 0;
        let mut rest = tree;
        while let Some(r) = rest.strip_prefix("super::") {
            count += 1;
            rest = r;
        }
        if rest.starts_with("super") {
            (Vec::new(), "")
        } else {
            let len = current.len().saturating_sub(count);
            (current[..len].to_vec(), rest)
        }
    } else if tree.starts_with("::") {
        (Vec::new(), "")
    } else {
        (current.to_vec(), tree)
    }
}

fn walk_use_tree(s: &str, prefix: &[String], out: &mut Vec<Vec<String>>) {
    let s = s.trim();
    if s.is_empty() {
        return;
    }
    if s == "*" {
        out.push(prefix.to_vec());
        return;
    }
    let segs = split_double_colon(s);
    let mut cur = prefix.to_vec();
    let mut saw_group = false;
    for raw_seg in segs {
        let seg = raw_seg.trim();
        if seg.is_empty() {
            continue;
        }
        if seg.starts_with('{') && seg.ends_with('}') {
            saw_group = true;
            let inner = &seg[1..seg.len() - 1];
            for sub in split_top_commas(inner) {
                walk_use_tree(&sub, &cur, out);
            }
        } else if seg == "*" {
            out.push(cur.clone());
            saw_group = true;
        } else {
            let name = match seg.rfind(" as ") {
                Some(i) => &seg[..i],
                None => seg,
            };
            cur.push(name.trim().to_string());
        }
    }
    if !saw_group {
        out.push(cur);
    }
}

/// Split by `::` but do not break inside `{...}`.
fn split_double_colon(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '{' {
            depth += 1;
            cur.push(c);
        } else if c == '}' {
            depth -= 1;
            cur.push(c);
        } else if c == ':' && depth == 0 && i + 1 < chars.len() && chars[i + 1] == ':' {
            out.push(std::mem::take(&mut cur));
            i += 2;
            continue;
        } else {
            cur.push(c);
        }
        i += 1;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Split by top-level commas (depth 0) inside a `{...}` group body.
fn split_top_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in s.chars() {
        if c == '{' {
            depth += 1;
            cur.push(c);
        } else if c == '}' {
            depth -= 1;
            cur.push(c);
        } else if c == ',' && depth == 0 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// Extract all `use` paths from a stripped file as raw strings (content after
/// the `use` keyword, before the terminating `;`).
fn is_use_at(bytes: &[u8], i: usize) -> bool {
    bytes[i] == b'u'
        && i + 3 < bytes.len()
        && bytes[i + 1] == b's'
        && bytes[i + 2] == b'e'
        && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t' || bytes[i - 1] == b'\n')
}

fn extract_use_paths(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if is_use_at(bytes, i) {
            let mut j = i + 3;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'(' {
                while j < bytes.len() && bytes[j] != b')' {
                    j += 1;
                }
                if j < bytes.len() {
                    j += 1;
                }
                while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                    j += 1;
                }
            }
            if j < bytes.len() && is_use_at(bytes, j) {
                i = j;
                j = i + 3;
            }
            let mut k = j;
            let mut depth = 0;
            while k < bytes.len() {
                let c = bytes[k];
                if c == b'{' {
                    depth += 1;
                } else if c == b'}' {
                    if depth > 0 {
                        depth -= 1;
                    }
                } else if c == b';' && depth == 0 {
                    break;
                }
                k += 1;
            }
            if k < bytes.len() {
                let path = &content[i + 3..k];
                out.push(path.trim().to_string());
            }
            i = k + 1;
        } else {
            i += 1;
        }
    }
    out
}

fn module_is_exempt(mod_path: &[String]) -> bool {
    mod_path.iter().any(|s| s == "tests")
}

fn edges_for(file: &Path, mod_path: &[String]) -> Vec<String> {
    let content = read_stripped(file);
    let mut edges = Vec::new();
    for raw in extract_use_paths(&content) {
        for segs in parse_use_tree(&raw, mod_path) {
            if segs.is_empty() {
                continue;
            }
            edges.push(format!("crate::{}", segs.join("::")));
        }
    }
    edges
}

fn starts_with_any(edge: &str, prefixes: &[&str]) -> Option<String> {
    for p in prefixes {
        if edge.starts_with(*p) {
            return Some((*p).to_string());
        }
    }
    None
}

#[test]
fn simulation_does_not_depend_on_bridge_or_renderer() {
    let map = build_module_map();
    let forbidden = ["crate::bridge", "crate::renderer"];
    let mut violations = Vec::new();
    for (file, mod_path) in &map {
        if !mod_path.starts_with(&["simulation".to_string()]) {
            continue;
        }
        if module_is_exempt(mod_path) {
            continue;
        }
        for edge in edges_for(file, mod_path) {
            if let Some(p) = starts_with_any(&edge, &forbidden) {
                violations.push(format!(
                    "{} ({}) -> {} [forbidden: {}]",
                    mod_path.join("::"),
                    file.display(),
                    edge,
                    p
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "simulation must not depend on bridge/renderer (ADR 0001):\n{}",
        violations.join("\n")
    );
}

#[test]
fn autonomy_does_not_depend_on_bridge_or_renderer() {
    let map = build_module_map();
    let forbidden = ["crate::bridge", "crate::renderer"];
    let mut violations = Vec::new();
    for (file, mod_path) in &map {
        if mod_path.first().map(|s| s.as_str()) != Some("simulation") {
            continue;
        }
        if mod_path.get(1).map(|s| s.as_str()) != Some("autonomy") {
            continue;
        }
        if module_is_exempt(mod_path) {
            continue;
        }
        for edge in edges_for(file, mod_path) {
            if let Some(p) = starts_with_any(&edge, &forbidden) {
                violations.push(format!(
                    "{} ({}) -> {} [forbidden: {}]",
                    mod_path.join("::"),
                    file.display(),
                    edge,
                    p
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "autonomy must not depend on bridge/renderer (ADR 0001):\n{}",
        violations.join("\n")
    );
}

#[test]
fn bridge_is_transport_only() {
    let map = build_module_map();
    let forbidden = [
        "crate::simulation::autonomy",
        "crate::simulation::households",
        "crate::simulation::lifecycle",
        "crate::simulation::dependents",
        "crate::simulation::food_sharing",
        "crate::simulation::events",
        "crate::simulation::grief",
        "crate::simulation::kinship",
        "crate::simulation::pipeline",
        "crate::simulation::renewal",
        "crate::simulation::spatial",
    ];
    let mut violations = Vec::new();
    for (file, mod_path) in &map {
        if mod_path.first().map(|s| s.as_str()) != Some("bridge") {
            continue;
        }
        for edge in edges_for(file, mod_path) {
            if let Some(p) = starts_with_any(&edge, &forbidden) {
                violations.push(format!(
                    "{} ({}) -> {} [forbidden: {}]",
                    mod_path.join("::"),
                    file.display(),
                    edge,
                    p
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "bridge must be transport-only (ADR 0001), must not import domain internals:\n{}",
        violations.join("\n")
    );
}

#[test]
fn world_is_not_dependent_on_simulation_or_bridge() {
    let map = build_module_map();
    let forbidden = [
        "crate::simulation",
        "crate::bridge",
        "crate::renderer",
        "crate::autonomy",
    ];
    let mut violations = Vec::new();
    for (file, mod_path) in &map {
        if mod_path.len() != 1 || mod_path[0] != "world" {
            continue;
        }
        for edge in edges_for(file, mod_path) {
            if let Some(p) = starts_with_any(&edge, &forbidden) {
                violations.push(format!(
                    "{} ({}) -> {} [forbidden: {}]",
                    mod_path.join("::"),
                    file.display(),
                    edge,
                    p
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "world must not depend on simulation/bridge/renderer (dependency inversion):\n{}",
        violations.join("\n")
    );
}

#[test]
fn strip_helper_is_sound() {
    let tricky = r##"
        // crate::bridge should be ignored in line comment
        /* crate::bridge in block comment */
        /* nested /* crate::bridge */ still comment */
        let s = "crate::bridge inside string";
        let r = r#"crate::bridge in raw string"#;
        let c = 'b'; // char
        use crate::bridge::X; // this one is real and should be detected
    "##;
    let stripped = strip_comments_and_strings(tricky);
    let count = stripped.matches("crate::bridge").count();
    assert_eq!(
        count, 1,
        "strip helper must leave exactly one real occurrence, got {} in:\n{}",
        count, stripped
    );
}

#[test]
fn module_map_resolves_expected_paths() {
    let map = build_module_map();
    let mut found = HashMap::new();
    for (_, mod_path) in &map {
        found.insert(mod_path.join("::"), true);
    }
    assert!(
        found.contains_key("simulation"),
        "simulation module must be discovered"
    );
    assert!(
        found.contains_key("simulation::autonomy"),
        "simulation::autonomy module must be discovered"
    );
    assert!(
        found.contains_key("bridge"),
        "bridge module must be discovered"
    );
}

#[test]
fn use_tree_parser_handles_groups_and_aliases() {
    let cur = vec!["simulation".to_string(), "autonomy".to_string()];
    let edges = parse_use_tree(
        "crate::simulation::{autonomy::{X, Y as Z}, lifecycle}",
        &cur,
    );
    let mut strings: Vec<String> = edges
        .iter()
        .map(|e| format!("crate::{}", e.join("::")))
        .collect();
    strings.sort();
    assert_eq!(
        strings,
        vec![
            "crate::simulation::autonomy::X".to_string(),
            "crate::simulation::autonomy::Y".to_string(),
            "crate::simulation::lifecycle".to_string(),
        ]
    );

    let edges2 = parse_use_tree(
        "super::super::foo::bar",
        &["a".to_string(), "b".to_string(), "c".to_string()],
    );
    assert_eq!(
        edges2,
        vec![vec!["a".to_string(), "foo".to_string(), "bar".to_string()]]
    );

    let edges3 = parse_use_tree("self::events::*", &["simulation".to_string()]);
    assert_eq!(
        edges3,
        vec![vec!["simulation".to_string(), "events".to_string()]]
    );
}

#[test]
fn simulation_does_not_use_nondeterministic_apis() {
    let map = build_module_map();
    let forbidden_patterns = ["rand::", "thread_rng", "getrandom", "SystemTime::now"];
    let mut violations = Vec::new();
    for (file, mod_path) in &map {
        if !mod_path.starts_with(&["simulation".to_string()]) {
            continue;
        }
        if module_is_exempt(mod_path) {
            continue;
        }
        let content = read_stripped(file);
        for &pattern in &forbidden_patterns {
            if content.contains(pattern) {
                violations.push(format!(
                    "{} ({}) contains forbidden pattern '{}'",
                    mod_path.join("::"),
                    file.display(),
                    pattern
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "simulation must not depend on non-deterministic PRNGs or wall clocks:\n{}",
        violations.join("\n")
    );
}
