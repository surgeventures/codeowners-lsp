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
    check <file>      Show which rule owns a specific file
    coverage          Show files without owners

OPTIONS:
    --json            Output as JSON (for lint command)
    --help, -h        Show this help

EXAMPLES:
    codeowners-cli lint
    codeowners-cli lint .github/CODEOWNERS
    codeowners-cli lint --json
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
