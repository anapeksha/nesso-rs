use std::path::{Path, PathBuf};
use std::process;

// ── result types ─────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Violation {
    file: PathBuf,
    line: usize,
    rule: &'static str,
    text: String,
}

impl Violation {
    fn new(file: &Path, line: usize, rule: &'static str, text: &str) -> Self {
        Self {
            file: file.to_owned(),
            line,
            rule,
            text: text.trim().to_owned(),
        }
    }
}

// ── rules ─────────────────────────────────────────────────────────────────────

struct Rule {
    id: &'static str,
    description: &'static str,
    check: fn(&str) -> bool,
    skip_in_tests: bool,
}

fn is_rule_pattern_literal(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("description:")
        || trimmed.starts_with('"')
        || trimmed.starts_with("r#")
        || trimmed.starts_with("&& line.contains")
        || trimmed.starts_with("&& (t.contains")
        || trimmed.starts_with("&& (t.starts_with")
        || trimmed.starts_with("|| t.contains")
        || trimmed.starts_with("t.contains")
        || trimmed.starts_with("let vague")
        || trimmed.starts_with("let vague_bindings")
}

const RULES: &[Rule] = &[
    Rule {
        id: "no-bare-unwrap",
        description: "bare .unwrap() outside tests — use .expect(\"reason\") or restructure",
        check: |line| {
            let trimmed = line.trim();
            !trimmed.starts_with("//")
                && !is_rule_pattern_literal(line)
                && line.contains(".unwrap()")
        },
        skip_in_tests: true,
    },
    Rule {
        id: "no-restating-comment",
        description: "comment that likely restates the next line (initialize / return / check)",
        check: |line| {
            let t = line.trim().to_lowercase();
            if !t.starts_with("//") {
                return false;
            }
            // Strip the comment marker and leading whitespace.
            let body = t.trim_start_matches('/').trim();
            let restating_prefixes = [
                "initialize the",
                "initialise the",
                "return the",
                "return result",
                "check if",
                "check whether",
                "call the",
                "get the",
                "set the",
                "create the",
                "create a new",
                "this function",
                "this method",
                "helper function",
                "helper method",
            ];
            restating_prefixes.iter().any(|p| body.starts_with(p))
        },
        skip_in_tests: false,
    },
    Rule {
        id: "no-safe-unwrap-comment",
        description: "comment claiming an unwrap is safe — use .expect() or restructure",
        check: |line| {
            let t = line.trim().to_lowercase();
            t.starts_with("//")
                && (t.contains("unwrap is safe")
                    || t.contains("safe to unwrap")
                    || t.contains("cannot fail here")
                    || t.contains("guaranteed to be some")
                    || t.contains("will never be none")
                    || t.contains("unwrap: "))
        },
        skip_in_tests: false,
    },
    Rule {
        id: "no-todo-on-implemented-code",
        description: "todo!() or unimplemented!() in non-test library code",
        check: |line| {
            let t = line.trim();
            !t.starts_with("//")
                && !is_rule_pattern_literal(line)
                && (t.contains("todo!()")
                    || t.contains("todo!(\"")
                    || t.contains("unimplemented!()"))
        },
        skip_in_tests: true,
    },
    Rule {
        id: "no-placeholder-comments",
        description: "TODO/FIXME/XXX placeholder comments in checked-in code",
        check: |line| {
            let t = line.trim().to_ascii_lowercase();
            t.starts_with("// todo")
                || t.starts_with("// fixme")
                || t.starts_with("// xxx")
                || t.starts_with("/// todo")
                || t.starts_with("/// fixme")
                || t.starts_with("/// xxx")
        },
        skip_in_tests: false,
    },
    Rule {
        id: "no-placeholder-expect",
        description: "expect() message uses TODO/FIXME/placeholder text",
        check: |line| {
            if is_rule_pattern_literal(line) {
                return false;
            }
            let t = line.trim().to_ascii_lowercase();
            t.contains(".expect(\"todo")
                || t.contains(".expect(\"fixme")
                || t.contains(".expect(\"should work")
                || t.contains(".expect(\"should not fail")
        },
        skip_in_tests: false,
    },
    Rule {
        id: "no-vague-error-variants",
        description: "vague error variant name (SomethingFailed / InitializationError / ConfigurationError / InvalidInput)",
        check: |line| {
            let t = line.trim();
            // Only flag inside enum-like lines (variant definitions or construction).
            if t.starts_with("//") {
                return false;
            }
            if is_rule_pattern_literal(line) {
                return false;
            }
            let vague = [
                "InitializationError",
                "InitialisationError",
                "ConfigurationError",
                "InvalidInput",
                "SomethingFailed",
                "GenericError",
                "UnknownError",
                "GeneralError",
            ];
            vague.iter().any(|v| t.contains(v))
        },
        skip_in_tests: false,
    },
    Rule {
        id: "no-vague-variable-names",
        description: "variable binding named result / data / value / temp / val without qualification",
        check: |line| {
            let t = line.trim();
            if t.starts_with("//") {
                return false;
            }
            if is_rule_pattern_literal(line) {
                return false;
            }
            let vague_bindings = [
                "let result =",
                "let result:",
                "let mut result =",
                "let mut result:",
                "let data =",
                "let data:",
                "let mut data =",
                "let value =",
                "let value:",
                "let mut value =",
                "let temp =",
                "let temp:",
                "let mut temp =",
                "let val =",
                "let val:",
                "let mut val =",
            ];
            vague_bindings.iter().any(|b| t.contains(b))
        },
        skip_in_tests: true,
    },
    Rule {
        id: "no-handle-process-do-functions",
        description: "function named handle_X / process_X / do_X — name the actual behavior",
        check: |line| {
            let t = line.trim();
            if t.starts_with("//") {
                return false;
            }
            (t.starts_with("fn handle_")
                || t.starts_with("pub fn handle_")
                || t.starts_with("pub(crate) fn handle_")
                || t.starts_with("fn process_")
                || t.starts_with("pub fn process_")
                || t.starts_with("pub(crate) fn process_")
                || t.starts_with("fn do_")
                || t.starts_with("pub fn do_")
                || t.starts_with("pub(crate) fn do_"))
                // process_ is legitimate in some embedded contexts (process_packet etc.)
                // so require it to be truly generic: process_data, process_input, process_event
                && !t.contains("process_packet")
                && !t.contains("process_frame")
                && !t.contains("process_irq")
        },
        skip_in_tests: false,
    },
    Rule {
        id: "no-panic-in-library",
        description: "panic!() in library code — use Result instead",
        check: |line| {
            let t = line.trim();
            !t.starts_with("//")
                && !is_rule_pattern_literal(line)
                && (t.starts_with("panic!(") || t.contains(" panic!("))
        },
        skip_in_tests: true,
    },
    Rule {
        id: "no-is-initialized-fields",
        description: "struct field named is_initialized / has_been_set / was_called — model state with types",
        check: |line| {
            let t = line.trim();
            if t.starts_with("//") {
                return false;
            }
            if is_rule_pattern_literal(line) {
                return false;
            }
            t.contains("is_initialized")
                || t.contains("is_initialised")
                || t.contains("has_been_set")
                || t.contains("was_called")
                || t.contains("was_initialized")
                || t.contains("was_initialised")
        },
        skip_in_tests: false,
    },
    Rule {
        id: "no-allow-dead-code-without-comment",
        description: "#[allow(dead_code)] without an adjacent comment explaining why",
        check: |line| {
            // This is a single-line check — a companion pass handles context.
            // Here we flag all occurrences; the context pass below will clear
            // ones that have a comment on the preceding line.
            line.trim() == "#[allow(dead_code)]"
        },
        skip_in_tests: false,
    },
    Rule {
        id: "no-vague-test-names",
        description: "test named test_it_works / test_basic / test_simple / test_foo",
        check: |line| {
            let t = line.trim();
            if is_rule_pattern_literal(line) {
                return false;
            }
            let vague = [
                "fn test_it_works",
                "fn test_basic",
                "fn test_simple",
                "fn test_foo",
                "fn test_bar",
                "fn test_test",
                "fn it_works",
            ];
            vague.iter().any(|v| t.contains(v))
        },
        skip_in_tests: false,
    },
    Rule {
        id: "no-ok-only-test-assertion",
        description: "test asserts only is_ok() / is_err() with no content check",
        check: |line| {
            let t = line.trim();
            // assert!(something.is_ok()) with nothing else on the line
            t == "assert!(result.is_ok());"
                || t == "assert!(res.is_ok());"
                || t == "assert!(output.is_ok());"
        },
        skip_in_tests: false,
    },
];

// ── file walking ──────────────────────────────────────────────────────────────

fn collect_rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_recursive(root, &mut files);
    files
}

fn collect_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Skip target/, .git/, node_modules/ etc.
            if matches!(name, "target" | ".git" | "node_modules" | ".cargo") {
                continue;
            }
            collect_recursive(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

// ── context-aware checks ──────────────────────────────────────────────────────

// Returns true if the line at `idx` is inside a #[cfg(test)] module.
fn inside_test_module(lines: &[&str], idx: usize) -> bool {
    let mut depth: i32 = 0;
    let mut in_test_scope = false;
    let mut test_scope_depth: i32 = -1;

    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.contains("#[cfg(test)]") {
            in_test_scope = true;
            test_scope_depth = depth;
        }
        for ch in t.chars() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if in_test_scope && depth <= test_scope_depth {
                    in_test_scope = false;
                    test_scope_depth = -1;
                }
            }
        }
        if i == idx {
            return in_test_scope;
        }
    }
    false
}

// Clears allow(dead_code) violations where the preceding line is a comment.
fn has_preceding_comment(lines: &[&str], idx: usize) -> bool {
    if idx == 0 {
        return false;
    }
    let prev = lines[idx - 1].trim();
    prev.starts_with("//") || prev.starts_with("///") || prev.ends_with("*/")
}

// ── main lint pass ────────────────────────────────────────────────────────────

fn lint_file(path: &Path) -> Vec<Violation> {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let lines: Vec<&str> = source.lines().collect();
    let mut violations = Vec::new();

    for (idx, &line) in lines.iter().enumerate() {
        let lineno = idx + 1;

        for rule in RULES {
            if !(rule.check)(line) {
                continue;
            }

            // Context-aware overrides.
            if rule.skip_in_tests && inside_test_module(&lines, idx) {
                continue;
            }
            if rule.id == "no-allow-dead-code-without-comment" && has_preceding_comment(&lines, idx)
            {
                continue;
            }

            violations.push(Violation::new(path, lineno, rule.id, line));
        }
    }

    violations
}

// ── single-file module check ──────────────────────────────────────────────────

// A module file is suspicious if it contains exactly one struct/enum/type
// definition and nothing else of substance.
fn check_single_item_modules(root: &Path, violations: &mut Vec<Violation>) {
    let files = collect_rust_files(root);
    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let struct_count = source.matches("\nstruct ").count()
            + source.matches("\npub struct ").count()
            + source.matches("\npub(crate) struct ").count();
        let enum_count = source.matches("\nenum ").count()
            + source.matches("\npub enum ").count()
            + source.matches("\npub(crate) enum ").count();
        let fn_count = source.matches("\nfn ").count()
            + source.matches("\npub fn ").count()
            + source.matches("\npub(crate) fn ").count();
        let impl_count = source.matches("\nimpl ").count() + source.matches("\nimpl<").count();

        let total_items = struct_count + enum_count + fn_count + impl_count;

        // A file with exactly one struct/enum and nothing else (no fns, no impls)
        // is likely fake modularity.
        if (struct_count + enum_count) == 1 && total_items == 1 && source.len() < 512 {
            violations.push(Violation::new(
                path,
                1,
                "no-single-item-module",
                "file contains exactly one type definition with no methods — consider flattening into parent module",
            ));
        }
    }
}

// ── copy-paste detection ──────────────────────────────────────────────────────

// Flags identical multi-line error-handling blocks repeated 3+ times.
// Looks for repeated `.map_err(` or `match ... Err` blocks of 3+ consecutive lines.
fn check_copy_pasted_error_blocks(path: &Path, violations: &mut Vec<Violation>) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let lines: Vec<&str> = source.lines().collect();

    let mut windows: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();

    for (i, window) in lines.windows(3).enumerate() {
        let key = window
            .iter()
            .map(|l| l.trim())
            .collect::<Vec<_>>()
            .join("\n");
        let meaningful_chars = key.chars().filter(|ch| !ch.is_whitespace()).count();
        let has_control_flow = key.contains("match ") || key.contains("=>");
        let has_error_mapping = key.contains("map_err") || key.contains("Err(");
        if meaningful_chars >= 80 && has_error_mapping && has_control_flow {
            windows.entry(key).or_default().push(i + 1);
        }
    }

    for (block, occurrences) in &windows {
        if occurrences.len() >= 3 {
            violations.push(Violation::new(
                path,
                occurrences[0],
                "no-copy-pasted-error-blocks",
                &format!(
                    "identical error-handling block appears {} times — extract a helper\n  block: {}",
                    occurrences.len(),
                    block.lines().next().unwrap_or(""),
                ),
            ));
        }
    }
}

// ── entry point ───────────────────────────────────────────────────────────────

pub fn run() {
    let root = workspace_root();
    let rust_files = collect_rust_files(&root);

    let mut all_violations: Vec<Violation> = Vec::new();

    // Per-line rule checks.
    for path in &rust_files {
        let mut vs = lint_file(path);
        all_violations.append(&mut vs);
    }

    // File-level structural checks.
    check_single_item_modules(&root, &mut all_violations);

    // Per-file copy-paste checks.
    for path in &rust_files {
        check_copy_pasted_error_blocks(path, &mut all_violations);
    }

    // Sort by file then line for readable output.
    all_violations.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

    if all_violations.is_empty() {
        println!("cargo xtask lint: no violations found");
        return;
    }

    eprintln!(
        "\ncargo xtask lint: {} violation(s) found\n",
        all_violations.len()
    );

    let mut last_file: Option<&PathBuf> = None;
    for v in &all_violations {
        if last_file != Some(&v.file) {
            eprintln!("\n{}", v.file.display());
            last_file = Some(&v.file);
        }
        eprintln!(
            "  {:>4}  [{}]  {}",
            v.line,
            v.rule,
            v.description_for(v.rule)
        );
        eprintln!("        {}", v.text);
    }

    eprintln!();
    process::exit(1);
}

impl Violation {
    fn description_for(&self, rule: &str) -> &'static str {
        RULES
            .iter()
            .find(|r| r.id == rule)
            .map(|r| r.description)
            .unwrap_or("unknown rule")
    }
}

fn workspace_root() -> PathBuf {
    // Walk up from the xtask binary location to find Cargo.toml workspace root.
    let mut dir = std::env::current_dir().expect("cannot determine current directory");
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            let content = std::fs::read_to_string(&candidate).unwrap_or_default();
            if content.contains("[workspace]") {
                return dir;
            }
        }
        if !dir.pop() {
            return std::env::current_dir().expect("current directory was available earlier");
        }
    }
}
