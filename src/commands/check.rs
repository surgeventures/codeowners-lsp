use std::process::ExitCode;
use std::{env, fs};

use crate::ownership::{check_file_ownership, find_codeowners};

pub fn check(file_path: &str) -> ExitCode {
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

    match check_file_ownership(&content, file_path) {
        Some(result) => {
            println!("File: {}", file_path);
            println!("Rule: {} (line {})", result.pattern, result.line_number + 1);
            println!("Owners: {}", result.owners.join(" "));
            ExitCode::SUCCESS
        }
        None => {
            println!("File: {}", file_path);
            println!("No matching rule found - file has no owners");
            ExitCode::from(1)
        }
    }
}
