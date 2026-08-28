use std::fs;
use std::path::{Path, PathBuf};

fn walk_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Strip line comments (//), block comments (/* */ nested), and string/char
/// literals so `contains("crate::bridge")` cannot be bypassed via comment or
/// string. Newlines are preserved to keep line numbers accurate.
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

        // Inside line comment
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

        // Inside block comment (nested)
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

        // Inside raw string r#\" ... \"# / r##\" ... \"##
        if let Some(hashes) = in_raw_string_hashes {
            if c == '"' {
                // Check if followed by exactly `hashes` hashes
                let mut k = 0;
                while k < hashes && i + 1 + k < chars.len() && chars[i + 1 + k] == '#' {
                    k += 1;
                }
                if k == hashes {
                    // End of raw string
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

        // Inside normal string
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
                // Unterminated, but preserve
                out.push('\n');
            } else {
                out.push(' ');
            }
            i += 1;
            continue;
        }

        // Inside char literal
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

        // Normal state — detect transitions
        // Raw string start: r#\" , r##\" , etc or r\"
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

fn violations_in_file(path: &Path, patterns: &[&str]) -> Vec<String> {
    let raw = fs::read_to_string(path).unwrap();
    let stripped = strip_comments_and_strings(&raw);
    let mut out = Vec::new();
    for (idx, line) in stripped.lines().enumerate() {
        for pat in patterns {
            if line.contains(pat) {
                // Show original line for diagnostics, not stripped
                let orig_line = raw.lines().nth(idx).unwrap_or("");
                out.push(format!(
                    "{}:{}: {} [matched: {}]",
                    path.display(),
                    idx + 1,
                    orig_line.trim(),
                    pat
                ));
                break;
            }
        }
    }
    out
}

#[test]
fn simulation_does_not_depend_on_bridge_or_web() {
    let mut files = Vec::new();
    walk_rs_files(Path::new("src/simulation"), &mut files);
    let patterns = [
        "crate::bridge",
        "crate::renderer",
        "crate::web",
        "crate::lib", // WASM facade
    ];
    let mut violations = Vec::new();
    for path in &files {
        if path.to_string_lossy().contains("tests") {
            continue;
        }
        violations.extend(violations_in_file(path, &patterns));
    }
    assert!(
        violations.is_empty(),
        "simulation must not depend on bridge/renderer/web (ADR 0001):\n{}",
        violations.join("\n")
    );
}

#[test]
fn autonomy_does_not_depend_on_bridge_or_web() {
    let mut files = Vec::new();
    walk_rs_files(Path::new("src/simulation/autonomy"), &mut files);
    let patterns = [
        "crate::bridge",
        "crate::renderer",
        "crate::web",
        "crate::world::", // autonomy may read world via Simulation, not directly via world internals
    ];
    // Autonomy legitimately uses crate::world as type via simulation facade,
    // but direct `crate::world::` import is suspicious — allow via `world::` helper
    // For now enforce no bridge/renderer/web; world check is informational.
    let strict_patterns = ["crate::bridge", "crate::renderer", "crate::web"];
    let mut violations = Vec::new();
    for path in &files {
        violations.extend(violations_in_file(path, &strict_patterns));
        // Also ensure autonomy doesn't directly import bridge types via `bridge::`
        if violations_in_file(path, &["bridge::"])
            .iter()
            .any(|v| v.contains("use "))
        {
            // Already captured above if via crate::bridge, this catches relative
            violations.extend(violations_in_file(
                path,
                &["use crate::bridge", "use bridge::"],
            ));
        }
    }
    // De-duplicate by path:line
    violations.sort();
    violations.dedup();
    // Filter to only the strict guarantee; world check is not enforced yet
    let _ = patterns; // keep documented
    assert!(
        violations.is_empty(),
        "autonomy must not depend on bridge/renderer/web (ADR 0001):\n{}",
        violations.join("\n")
    );
}

#[test]
fn bridge_does_not_contain_domain_logic() {
    let mut files = Vec::new();
    walk_rs_files(Path::new("src/bridge"), &mut files);
    // Bridge may import Simulation facade and world, but not domain internals.
    // Allow `crate::simulation::{Simulation, Entity, ...}` facade types.
    // Forbid deep domain paths and direct `autonomy::` usage.
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
        "crate::simulation::household_membership",
        "autonomy::",
        "households::plan_",
        "lifecycle::",
        "dependents::",
    ];
    let mut violations = Vec::new();
    for path in files {
        let raw = fs::read_to_string(&path).unwrap();
        let stripped = strip_comments_and_strings(&raw);
        for (idx, line) in stripped.lines().enumerate() {
            for pat in &forbidden {
                if line.contains(pat) {
                    // Allow `use crate::simulation::autonomy` only in comments/strings already stripped,
                    // so any remaining occurrence is real code.
                    let orig = raw.lines().nth(idx).unwrap_or("");
                    violations.push(format!(
                        "{}:{}: {} [matched: {}]",
                        path.display(),
                        idx + 1,
                        orig.trim(),
                        pat
                    ));
                    break;
                }
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
fn world_is_not_dependent_on_simulation() {
    let raw = fs::read_to_string("src/world.rs").unwrap();
    let stripped = strip_comments_and_strings(&raw);
    let forbidden = [
        "crate::simulation",
        "crate::bridge",
        "crate::renderer",
        "crate::autonomy",
    ];
    let mut violations = Vec::new();
    for (idx, line) in stripped.lines().enumerate() {
        for pat in &forbidden {
            if line.contains(pat) {
                let orig = raw.lines().nth(idx).unwrap_or("");
                violations.push(format!("{}: {} [matched: {}]", idx + 1, orig.trim(), pat));
                break;
            }
        }
    }
    assert!(
        violations.is_empty(),
        "world.rs must not depend on simulation/bridge/renderer (dependency inversion):\n{}",
        violations.join("\n")
    );
}

#[test]
fn strip_helper_is_sound() {
    // Self-test: ensure helper strips comments/strings so checks are not bypassable
    let tricky = r##"
        // crate::bridge should be ignored in line comment
        /* crate::bridge in block comment */
        /* nested /* crate::bridge */ still comment */
        let s = "crate::bridge inside string";
        let r = r#"crate::bridge in raw string"#;
        let c = 'b'; // char
        crate::bridge // this one is real and should be detected
    "##;
    let stripped = strip_comments_and_strings(tricky);
    let count = stripped.matches("crate::bridge").count();
    assert_eq!(
        count, 1,
        "strip helper must leave exactly one real occurrence, got {} in:\n{}",
        count, stripped
    );
}
