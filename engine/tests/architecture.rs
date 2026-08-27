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

#[test]
fn simulation_does_not_depend_on_bridge_or_web() {
    let mut files = Vec::new();
    walk_rs_files(Path::new("src/simulation"), &mut files);
    let mut violations = Vec::new();
    for path in files {
        // Tests are allowed to use bridge for serialization verification
        if path.to_string_lossy().contains("tests") {
            continue;
        }
        let content = fs::read_to_string(&path).unwrap();
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains("crate::bridge") {
                violations.push(format!("{}:{}: {}", path.display(), idx + 1, line));
            }
            if trimmed.contains("crate::renderer") {
                violations.push(format!("{}:{}: {}", path.display(), idx + 1, line));
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
fn bridge_does_not_contain_domain_logic() {
    let mut files = Vec::new();
    walk_rs_files(Path::new("src/bridge"), &mut files);
    let mut violations = Vec::new();
    for path in files {
        let content = fs::read_to_string(&path).unwrap();
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            // bridge may import Simulation facade and world, but not autonomy internals
            if trimmed.contains("autonomy::") && trimmed.contains("use ") {
                violations.push(format!("{}:{}: {}", path.display(), idx + 1, line));
            }
            if trimmed.contains("crate::simulation::autonomy")
                || trimmed.contains("crate::simulation::households")
                || trimmed.contains("crate::simulation::lifecycle")
            {
                violations.push(format!("{}:{}: {}", path.display(), idx + 1, line));
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
    let content = fs::read_to_string("src/world.rs").unwrap();
    assert!(
        !content.contains("crate::simulation"),
        "world.rs must not depend on simulation (dependency inversion)"
    );
}
