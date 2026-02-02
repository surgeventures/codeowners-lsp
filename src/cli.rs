#[allow(dead_code)]
mod diagnostics;
mod file_cache;
#[allow(dead_code)]
mod github;
mod parser;
mod pattern;
#[allow(dead_code)]
mod validation;

use std::path::PathBuf;
use std::process::ExitCode;
use std::{env, fs};

use file_cache::FileCache;

fn print_usage() {
    eprintln!(
        "codeowners-cli - Lint and inspect CODEOWNERS files

USAGE:
    codeowners-cli <command> [options]

COMMANDS:
    lint [path]       Check CODEOWNERS for issues (default: auto-detect)
    fmt [path]        Format CODEOWNERS file (normalizes spacing)
    fix [path]        Auto-fix safe issues (duplicate owners, etc)
    check <file>      Show which rule owns a specific file
    coverage          Show files without owners

OPTIONS:
    --json            Output as JSON (for lint command)
    --write, -w       Write changes (for fmt/fix commands, default: dry-run)
    --help, -h        Show this help

EXAMPLES:
    codeowners-cli lint
    codeowners-cli lint .github/CODEOWNERS
    codeowners-cli lint --json
    codeowners-cli fmt
    codeowners-cli fmt --write
    codeowners-cli fix --write
    codeowners-cli check src/main.rs
    codeowners-cli coverage
"
    );
}

fn find_codeowners(start: &PathBuf) -> Option<PathBuf> {
    codeowners::locate(start)
}

fn lint(path: Option<&str>, json_output: bool) -> ExitCode {
    let cwd = env::current_dir().expect("Failed to get current directory");

    let codeowners_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        match find_codeowners(&cwd) {
            Some(p) => p,
            None => {
                eprintln!("No CODEOWNERS file found");
                return ExitCode::from(1);
            }
        }
    };

    if !codeowners_path.exists() {
        eprintln!("File not found: {}", codeowners_path.display());
        return ExitCode::from(1);
    }

    let content = match fs::read_to_string(&codeowners_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read {}: {}", codeowners_path.display(), e);
            return ExitCode::from(1);
        }
    };

    // Get repo root (parent of .github or location of CODEOWNERS)
    let repo_root = codeowners_path
        .parent()
        .and_then(|p| {
            if p.ends_with(".github") {
                p.parent()
            } else {
                Some(p)
            }
        })
        .unwrap_or(&cwd);

    let file_cache = FileCache::new(&repo_root.to_path_buf());
    let (diagnostics, _) = diagnostics::compute_diagnostics_sync(&content, Some(&file_cache));

    if json_output {
        let json = serde_json::json!({
            "file": codeowners_path.display().to_string(),
            "diagnostics": diagnostics.iter().map(|d| {
                serde_json::json!({
                    "line": d.range.start.line + 1,
                    "column": d.range.start.character + 1,
                    "severity": match d.severity {
                        Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR) => "error",
                        Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING) => "warning",
                        Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT) => "hint",
                        Some(tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION) => "info",
                        _ => "unknown",
                    },
                    "code": d.code,
                    "message": d.message,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    } else {
        if diagnostics.is_empty() {
            println!("✓ {} - no issues found", codeowners_path.display());
            return ExitCode::SUCCESS;
        }

        println!("{}:", codeowners_path.display());
        for d in &diagnostics {
            let severity = match d.severity {
                Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR) => "error",
                Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING) => "warning",
                Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT) => "hint",
                Some(tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION) => "info",
                _ => "unknown",
            };
            let code = d
                .code
                .as_ref()
                .map(|c| match c {
                    tower_lsp::lsp_types::NumberOrString::String(s) => s.clone(),
                    tower_lsp::lsp_types::NumberOrString::Number(n) => n.to_string(),
                })
                .unwrap_or_default();
            println!(
                "  line {}: [{}] {} - {}",
                d.range.start.line + 1,
                severity,
                code,
                d.message
            );
        }
    }

    let has_errors = diagnostics.iter().any(|d| {
        matches!(
            d.severity,
            Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR)
        )
    });

    if has_errors {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn check_file(file_path: &str) -> ExitCode {
    let cwd = env::current_dir().expect("Failed to get current directory");

    let codeowners_path = match find_codeowners(&cwd) {
        Some(p) => p,
        None => {
            eprintln!("No CODEOWNERS file found");
            return ExitCode::from(1);
        }
    };

    let content = match fs::read_to_string(&codeowners_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read {}: {}", codeowners_path.display(), e);
            return ExitCode::from(1);
        }
    };

    let lines = parser::parse_codeowners_file_with_positions(&content);

    // Normalize file path
    let file_path = file_path.trim_start_matches("./");

    // Find matching rule (last match wins)
    let mut matching_rule = None;
    for parsed_line in &lines {
        if let parser::CodeownersLine::Rule { pattern, owners } = &parsed_line.content {
            if pattern::pattern_matches(pattern, file_path) {
                matching_rule = Some((parsed_line.line_number, pattern.clone(), owners.clone()));
            }
        }
    }

    match matching_rule {
        Some((line, pattern, owners)) => {
            println!("File: {}", file_path);
            println!("Rule: {} (line {})", pattern, line + 1);
            println!("Owners: {}", owners.join(" "));
            ExitCode::SUCCESS
        }
        None => {
            println!("File: {}", file_path);
            println!("No matching rule found - file has no owners");
            ExitCode::from(1)
        }
    }
}

fn coverage() -> ExitCode {
    let cwd = env::current_dir().expect("Failed to get current directory");

    let codeowners_path = match find_codeowners(&cwd) {
        Some(p) => p,
        None => {
            eprintln!("No CODEOWNERS file found");
            return ExitCode::from(1);
        }
    };

    let content = match fs::read_to_string(&codeowners_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read {}: {}", codeowners_path.display(), e);
            return ExitCode::from(1);
        }
    };

    // Get repo root
    let repo_root = codeowners_path
        .parent()
        .and_then(|p| {
            if p.ends_with(".github") {
                p.parent()
            } else {
                Some(p)
            }
        })
        .unwrap_or(&cwd);

    let file_cache = FileCache::new(&repo_root.to_path_buf());
    let lines = parser::parse_codeowners_file_with_positions(&content);
    let unowned = file_cache.get_unowned_files(&lines);

    let total_files = file_cache.count_matches("*");
    let owned_count = total_files - unowned.len();
    let coverage_pct = if total_files > 0 {
        (owned_count as f64 / total_files as f64) * 100.0
    } else {
        100.0
    };

    println!(
        "Coverage: {:.1}% ({}/{} files have owners)",
        coverage_pct, owned_count, total_files
    );

    if unowned.is_empty() {
        println!("\n✓ All files have owners!");
    } else {
        println!("\nFiles without owners ({}):", unowned.len());
        for file in unowned.iter().take(50) {
            println!("  {}", file);
        }
        if unowned.len() > 50 {
            println!("  ... and {} more", unowned.len() - 50);
        }
    }

    ExitCode::SUCCESS
}

fn fmt_codeowners(path: Option<&str>, write: bool) -> ExitCode {
    let cwd = env::current_dir().expect("Failed to get current directory");

    let codeowners_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        match find_codeowners(&cwd) {
            Some(p) => p,
            None => {
                eprintln!("No CODEOWNERS file found");
                return ExitCode::from(1);
            }
        }
    };

    if !codeowners_path.exists() {
        eprintln!("File not found: {}", codeowners_path.display());
        return ExitCode::from(1);
    }

    let content = match fs::read_to_string(&codeowners_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read {}: {}", codeowners_path.display(), e);
            return ExitCode::from(1);
        }
    };

    let formatted = format_codeowners(&content);

    if formatted == content {
        println!("✓ {} is already formatted", codeowners_path.display());
        return ExitCode::SUCCESS;
    }

    if write {
        match fs::write(&codeowners_path, &formatted) {
            Ok(_) => {
                println!("✓ Formatted {}", codeowners_path.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Failed to write {}: {}", codeowners_path.display(), e);
                ExitCode::from(1)
            }
        }
    } else {
        println!("Would format {}:\n", codeowners_path.display());
        println!("--- original");
        println!("+++ formatted\n");

        // Simple diff: show lines that differ
        let old_lines: Vec<&str> = content.lines().collect();
        let new_lines: Vec<&str> = formatted.lines().collect();

        let max_lines = old_lines.len().max(new_lines.len());
        for i in 0..max_lines {
            let old = old_lines.get(i).copied().unwrap_or("");
            let new = new_lines.get(i).copied().unwrap_or("");

            if old != new {
                if !old.is_empty() {
                    println!("-{}", old);
                }
                if !new.is_empty() {
                    println!("+{}", new);
                }
            }
        }

        println!("\nRun with --write or -w to apply changes");
        ExitCode::from(1)
    }
}

/// Format a CODEOWNERS file content
fn format_codeowners(content: &str) -> String {
    let mut result = Vec::new();
    let mut prev_was_empty = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Preserve blank lines but collapse multiple
        if trimmed.is_empty() {
            if !prev_was_empty && !result.is_empty() {
                result.push(String::new());
            }
            prev_was_empty = true;
            continue;
        }
        prev_was_empty = false;

        // Comments: normalize to single space after #
        if let Some(comment_text) = trimmed.strip_prefix('#') {
            let comment_text = comment_text.trim();
            if comment_text.is_empty() {
                result.push("#".to_string());
            } else {
                result.push(format!("# {}", comment_text));
            }
            continue;
        }

        // Rules: normalize spacing between pattern and owners
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let pattern = parts[0];
        let owners = &parts[1..];

        if owners.is_empty() {
            // No owners - just the pattern
            result.push(pattern.to_string());
        } else {
            // Pattern + owners with single space
            result.push(format!("{} {}", pattern, owners.join(" ")));
        }
    }

    // Ensure trailing newline
    let mut output = result.join("\n");
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

/// Auto-fix safe issues in CODEOWNERS file
fn fix_codeowners(path: Option<&str>, write: bool) -> ExitCode {
    let cwd = env::current_dir().expect("Failed to get current directory");

    let codeowners_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        match find_codeowners(&cwd) {
            Some(p) => p,
            None => {
                eprintln!("No CODEOWNERS file found");
                return ExitCode::from(1);
            }
        }
    };

    if !codeowners_path.exists() {
        eprintln!("File not found: {}", codeowners_path.display());
        return ExitCode::from(1);
    }

    let content = match fs::read_to_string(&codeowners_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read {}: {}", codeowners_path.display(), e);
            return ExitCode::from(1);
        }
    };

    let (fixed, fixes_applied) = apply_safe_fixes(&content);

    if fixes_applied.is_empty() {
        println!("✓ {} - no fixable issues", codeowners_path.display());
        return ExitCode::SUCCESS;
    }

    if write {
        match fs::write(&codeowners_path, &fixed) {
            Ok(_) => {
                println!(
                    "✓ Fixed {} ({} changes):",
                    codeowners_path.display(),
                    fixes_applied.len()
                );
                for fix in &fixes_applied {
                    println!("  - {}", fix);
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Failed to write {}: {}", codeowners_path.display(), e);
                ExitCode::from(1)
            }
        }
    } else {
        println!(
            "Would fix {} ({} changes):",
            codeowners_path.display(),
            fixes_applied.len()
        );
        for fix in &fixes_applied {
            println!("  - {}", fix);
        }
        println!("\nRun with --write or -w to apply fixes");
        ExitCode::from(1)
    }
}

/// Apply safe fixes to CODEOWNERS content.
/// Returns (fixed_content, list_of_fixes_applied)
fn apply_safe_fixes(content: &str) -> (String, Vec<String>) {
    use std::collections::HashSet;

    let lines = parser::parse_codeowners_file_with_positions(content);
    let original_lines: Vec<&str> = content.lines().collect();

    let mut fixes = Vec::new();
    let mut lines_to_delete: HashSet<usize> = HashSet::new();
    let mut line_replacements: std::collections::HashMap<usize, String> =
        std::collections::HashMap::new();

    // Track patterns for shadowed rule detection
    let mut exact_patterns: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for parsed_line in &lines {
        if let parser::CodeownersLine::Rule { pattern, owners } = &parsed_line.content {
            let line_num = parsed_line.line_number as usize;
            let normalized_pattern = pattern.trim_start_matches('/');

            // Fix 1: Remove duplicate owners
            let mut seen_owners: HashSet<&str> = HashSet::new();
            let deduped: Vec<&str> = owners
                .iter()
                .map(|s| s.as_str())
                .filter(|o| seen_owners.insert(*o))
                .collect();

            if deduped.len() < owners.len() {
                let new_line = if deduped.is_empty() {
                    pattern.clone()
                } else {
                    format!("{} {}", pattern, deduped.join(" "))
                };
                line_replacements.insert(line_num, new_line);
                fixes.push(format!("line {}: removed duplicate owners", line_num + 1));
            }

            // Fix 2: Remove shadowed rules (exact duplicates)
            if let Some(&prev_line) = exact_patterns.get(normalized_pattern) {
                lines_to_delete.insert(prev_line);
                fixes.push(format!(
                    "line {}: removed shadowed rule (duplicated on line {})",
                    prev_line + 1,
                    line_num + 1
                ));
            }
            exact_patterns.insert(normalized_pattern.to_string(), line_num);
        }
    }

    // Build the fixed content
    let mut result = Vec::new();
    for (i, line) in original_lines.iter().enumerate() {
        if lines_to_delete.contains(&i) {
            continue; // Skip deleted lines
        }
        if let Some(replacement) = line_replacements.get(&i) {
            result.push(replacement.clone());
        } else {
            result.push(line.to_string());
        }
    }

    let mut output = result.join("\n");
    if !content.is_empty() && content.ends_with('\n') && !output.ends_with('\n') {
        output.push('\n');
    }

    (output, fixes)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return ExitCode::from(1);
    }

    let command = &args[1];

    match command.as_str() {
        "lint" => {
            let mut path = None;
            let mut json_output = false;

            for arg in &args[2..] {
                match arg.as_str() {
                    "--json" => json_output = true,
                    "--help" | "-h" => {
                        print_usage();
                        return ExitCode::SUCCESS;
                    }
                    _ if !arg.starts_with('-') => path = Some(arg.as_str()),
                    _ => {
                        eprintln!("Unknown option: {}", arg);
                        return ExitCode::from(1);
                    }
                }
            }

            lint(path, json_output)
        }
        "fmt" | "format" => {
            let mut path = None;
            let mut write = false;

            for arg in &args[2..] {
                match arg.as_str() {
                    "--write" | "-w" => write = true,
                    "--help" | "-h" => {
                        print_usage();
                        return ExitCode::SUCCESS;
                    }
                    _ if !arg.starts_with('-') => path = Some(arg.as_str()),
                    _ => {
                        eprintln!("Unknown option: {}", arg);
                        return ExitCode::from(1);
                    }
                }
            }

            fmt_codeowners(path, write)
        }
        "fix" => {
            let mut path = None;
            let mut write = false;

            for arg in &args[2..] {
                match arg.as_str() {
                    "--write" | "-w" => write = true,
                    "--help" | "-h" => {
                        print_usage();
                        return ExitCode::SUCCESS;
                    }
                    _ if !arg.starts_with('-') => path = Some(arg.as_str()),
                    _ => {
                        eprintln!("Unknown option: {}", arg);
                        return ExitCode::from(1);
                    }
                }
            }

            fix_codeowners(path, write)
        }
        "check" => {
            if args.len() < 3 {
                eprintln!("Usage: codeowners-cli check <file>");
                return ExitCode::from(1);
            }
            check_file(&args[2])
        }
        "coverage" => coverage(),
        "--help" | "-h" | "help" => {
            print_usage();
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage();
            ExitCode::from(1)
        }
    }
}
