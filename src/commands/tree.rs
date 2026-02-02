use std::collections::HashSet;
use std::process::ExitCode;
use std::{env, fs};

use crate::file_cache::FileCache;
use crate::ownership::{check_file_ownership, find_codeowners, get_repo_root};

/// Generate a consistent ANSI color code from a string (for terminal output)
fn owner_color(owner: &str) -> u8 {
    // Use a simple hash to generate a color code
    let mut hash: u32 = 0;
    for byte in owner.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    // Map to bright colors (avoiding dark colors that are hard to read)
    // Use colors 1-14 (skip 0=black and 15=white)
    ((hash % 14) + 1) as u8
}

pub fn tree() -> ExitCode {
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

    let repo_root = get_repo_root(&codeowners_path, &cwd);
    let file_cache = FileCache::new(&repo_root);

    // Collect all files with their owners
    let mut files_with_owners: Vec<(String, Option<String>)> = Vec::new();

    for file in file_cache.all_files() {
        let owners = check_file_ownership(&content, file).map(|r| r.owners.join(" "));
        files_with_owners.push((file.clone(), owners));
    }

    // Sort by path
    files_with_owners.sort_by(|a, b| a.0.cmp(&b.0));

    // Print legend first
    let mut seen_owners: HashSet<String> = HashSet::new();
    for (_, owners) in &files_with_owners {
        if let Some(o) = owners {
            seen_owners.insert(o.clone());
        }
    }
    let mut owners_list: Vec<_> = seen_owners.into_iter().collect();
    owners_list.sort();

    println!("Legend:");
    for owner in &owners_list {
        let color = owner_color(owner);
        println!("  \x1b[38;5;{}m██\x1b[0m {}", color, owner);
    }
    println!("  \x1b[90m██\x1b[0m (no owner)\n");

    // Print files
    for (file, owners) in &files_with_owners {
        match owners {
            Some(owner) => {
                let color = owner_color(owner);
                println!("\x1b[38;5;{}m{}\x1b[0m", color, file);
            }
            None => {
                println!("\x1b[90m{}\x1b[0m", file);
            }
        }
    }

    ExitCode::SUCCESS
}
