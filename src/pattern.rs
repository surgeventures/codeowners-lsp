/// Simple glob pattern matching for CODEOWNERS patterns
pub fn pattern_matches(pattern: &str, path: &str) -> bool {
    pattern_matches_impl(pattern, path)
}

fn pattern_matches_impl(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim_start_matches('/');

    // Handle ** (matches everything)
    if pattern == "*" || pattern == "**" {
        return true;
    }

    // Handle complex patterns with * or ** - use full glob matching
    // This handles: deployment/*/deploy/**, *crowdin*, src/**/test.rs, etc.
    if pattern.contains('*') {
        return glob_match(pattern, path);
    }

    // Handle directory patterns like /dir/ or dir/
    if pattern.ends_with('/') {
        let dir = pattern.trim_end_matches('/');
        return path.starts_with(dir)
            && (path.len() == dir.len() || path[dir.len()..].starts_with('/'));
    }

    // Exact match or prefix match for directories
    path == pattern || path.starts_with(&format!("{}/", pattern))
}

/// Simple glob matching with * wildcards
fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let path_parts: Vec<&str> = path.split('/').collect();

    // Special case: single segment pattern like *.rs should match files in any directory
    // This is CODEOWNERS semantics - *.rs means "any .rs file anywhere"
    if pattern_parts.len() == 1 && pattern_parts[0].contains('*') {
        // Match against the filename (last segment)
        if let Some(filename) = path_parts.last() {
            return segment_matches(pattern_parts[0], filename);
        }
        return false;
    }

    glob_match_parts(&pattern_parts, &path_parts)
}

fn glob_match_parts(pattern_parts: &[&str], path_parts: &[&str]) -> bool {
    let mut pi = 0; // pattern index
    let mut fi = 0; // file path index

    while pi < pattern_parts.len() {
        let pattern_part = pattern_parts[pi];

        // Handle ** (matches zero or more path segments)
        if pattern_part == "**" {
            // If ** is the last pattern part, it matches everything remaining
            if pi == pattern_parts.len() - 1 {
                return true;
            }

            // Try matching ** against zero or more segments
            for skip in 0..=(path_parts.len().saturating_sub(fi)) {
                if glob_match_parts(&pattern_parts[pi + 1..], &path_parts[fi + skip..]) {
                    return true;
                }
            }
            return false;
        }

        // No more path parts but still have pattern parts
        if fi >= path_parts.len() {
            return false;
        }

        let path_part = path_parts[fi];

        // Handle * within a segment (e.g., *.rs, *crowdin*, create_service*)
        if pattern_part.contains('*') {
            if !segment_matches(pattern_part, path_part) {
                return false;
            }
        } else if pattern_part != path_part {
            // Exact segment match required
            return false;
        }

        pi += 1;
        fi += 1;
    }

    // All pattern parts consumed - check if all path parts consumed too
    fi >= path_parts.len()
}

/// Match a single path segment against a pattern with * wildcards
fn segment_matches(pattern: &str, segment: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    let parts: Vec<&str> = pattern.split('*').collect();

    if parts.len() == 1 {
        // No wildcard
        return pattern == segment;
    }

    if parts.len() == 2 {
        // Simple case: prefix*suffix
        let prefix = parts[0];
        let suffix = parts[1];

        if prefix.is_empty() && suffix.is_empty() {
            return true; // Just "*"
        }
        if prefix.is_empty() {
            return segment.ends_with(suffix);
        }
        if suffix.is_empty() {
            return segment.starts_with(prefix);
        }
        return segment.starts_with(prefix)
            && segment.ends_with(suffix)
            && segment.len() >= prefix.len() + suffix.len();
    }

    // Multiple wildcards (e.g., *foo*bar*) - use sequential matching
    if parts.is_empty() {
        return true; // Pattern is just "*"
    }

    let first = parts[0];
    if !first.is_empty() && !segment.starts_with(first) {
        return false;
    }

    let mut remaining = if first.is_empty() {
        segment
    } else {
        &segment[first.len()..]
    };

    for (i, part) in parts.iter().enumerate().skip(1) {
        if part.is_empty() {
            continue;
        }
        if i == parts.len() - 1 {
            // Last part must match at the end
            if !remaining.ends_with(part) {
                return false;
            }
        } else {
            // Middle part must exist somewhere
            match remaining.find(part) {
                Some(pos) => remaining = &remaining[pos + part.len()..],
                None => return false,
            }
        }
    }

    true
}

/// Check if pattern `a` is subsumed by pattern `b` (i.e., everything `a` matches, `b` also matches).
/// If true, and `b` comes after `a` in CODEOWNERS, then `a` is a dead rule.
pub fn pattern_subsumes(a: &str, b: &str) -> bool {
    let a = a.trim_start_matches('/');
    let b = b.trim_start_matches('/');

    // Identical patterns
    if a == b {
        return true;
    }

    // Universal patterns subsume everything
    if b == "*" || b == "**" {
        return true;
    }

    // Extension patterns: *.rs is subsumed by *
    if let Some(a_ext) = a.strip_prefix('*') {
        if b == "*" || b == "**" {
            return true;
        }
        // *.rs.bak is subsumed by *.bak
        if let Some(b_ext) = b.strip_prefix('*') {
            return a_ext.ends_with(b_ext);
        }
        return false;
    }

    // Directory patterns: /src/lib/ is subsumed by /src/
    let a_dir = a
        .trim_end_matches('/')
        .trim_end_matches("/**")
        .trim_end_matches("/*");
    let b_dir = b
        .trim_end_matches('/')
        .trim_end_matches("/**")
        .trim_end_matches("/*");

    let a_is_dir = a.ends_with('/') || a.ends_with("/**") || a.ends_with("/*");
    let b_is_dir = b.ends_with('/') || b.ends_with("/**") || b.ends_with("/*");

    // /src/lib/ subsumed by /src/ (more specific dir under more general)
    if a_is_dir && b_is_dir {
        return a_dir.starts_with(b_dir)
            && (a_dir == b_dir || a_dir.starts_with(&format!("{}/", b_dir)));
    }

    // Exact file in directory: src/main.rs subsumed by src/ or src/**
    if b_is_dir && !a_is_dir {
        return a.starts_with(b_dir) && (a == b_dir || a.starts_with(&format!("{}/", b_dir)));
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildcard_all() {
        assert!(pattern_matches("*", "any/file.rs"));
        assert!(pattern_matches("**", "any/nested/file.rs"));
    }

    #[test]
    fn test_extension_pattern() {
        assert!(pattern_matches("*.rs", "src/main.rs"));
        assert!(pattern_matches("*.rs", "lib.rs"));
        assert!(!pattern_matches("*.rs", "src/main.go"));
        assert!(!pattern_matches("*.rs", "readme.md"));
    }

    #[test]
    fn test_directory_pattern_trailing_slash() {
        assert!(pattern_matches("/src/", "src/main.rs"));
        assert!(pattern_matches("/src/", "src/lib/mod.rs"));
        assert!(pattern_matches("src/", "src/main.rs"));
        assert!(!pattern_matches("/src/", "other/file.rs"));
    }

    #[test]
    fn test_directory_pattern_glob() {
        assert!(pattern_matches("/src/**", "src/main.rs"));
        assert!(pattern_matches("/src/**", "src/nested/deep/file.rs"));
        assert!(pattern_matches("/src/*", "src/main.rs"));
        assert!(!pattern_matches("/src/**", "other/file.rs"));
    }

    #[test]
    fn test_exact_match() {
        assert!(pattern_matches("Makefile", "Makefile"));
        assert!(pattern_matches("/Makefile", "Makefile"));
        assert!(!pattern_matches("Makefile", "other/Makefile"));
    }

    #[test]
    fn test_directory_prefix() {
        assert!(pattern_matches("src", "src/main.rs"));
        assert!(pattern_matches("src", "src/nested/file.rs"));
        assert!(!pattern_matches("src", "other/src/file.rs"));
    }

    #[test]
    fn test_leading_slash_stripped() {
        assert!(pattern_matches("/src/main.rs", "src/main.rs"));
        assert!(pattern_matches("src/main.rs", "src/main.rs"));
    }

    #[test]
    fn test_nested_directory() {
        assert!(pattern_matches("src/lib/", "src/lib/mod.rs"));
        assert!(pattern_matches("/src/lib/", "src/lib/nested.rs"));
        assert!(!pattern_matches("src/lib/", "src/other/file.rs"));
    }

    #[test]
    fn test_double_star_prefix() {
        // **/foo.txt matches foo.txt in any directory
        assert!(pattern_matches(
            "**/mirrord_config.json",
            "src/mirrord_config.json"
        ));
        assert!(pattern_matches(
            "**/mirrord_config.json",
            "mirrord_config.json"
        ));
        assert!(pattern_matches(
            "**/mirrord_config.json",
            "a/b/c/mirrord_config.json"
        ));
        assert!(!pattern_matches("**/mirrord_config.json", "src/other.json"));

        // Leading slash variant
        assert!(pattern_matches(
            "/**/mirrord_config.json",
            "src/mirrord_config.json"
        ));
        assert!(pattern_matches("/**/foo.txt", "foo.txt"));
        assert!(pattern_matches("/**/foo.txt", "dir/foo.txt"));
    }

    #[test]
    fn test_double_star_middle() {
        // src/**/test.rs matches src/test.rs, src/foo/test.rs, etc.
        assert!(pattern_matches("src/**/test.rs", "src/test.rs"));
        assert!(pattern_matches("src/**/test.rs", "src/foo/test.rs"));
        assert!(pattern_matches("src/**/test.rs", "src/foo/bar/test.rs"));
        assert!(!pattern_matches("src/**/test.rs", "other/test.rs"));
        assert!(!pattern_matches("src/**/test.rs", "src/foo/other.rs"));
    }

    #[test]
    fn test_single_star_in_path() {
        // deployment/*/deploy matches deployment/foo/deploy
        assert!(pattern_matches(
            "deployment/*/deploy/apps/staging/Chart.yaml",
            "deployment/analytics/deploy/apps/staging/Chart.yaml"
        ));
        assert!(pattern_matches(
            "deployment/*/deploy/**",
            "deployment/foo/deploy/bar/baz.yaml"
        ));
        assert!(!pattern_matches(
            "deployment/*/deploy/**",
            "other/foo/deploy/bar.yaml"
        ));
    }

    #[test]
    fn test_star_in_filename() {
        // *crowdin* matches files with crowdin in the name
        assert!(pattern_matches(
            ".github/workflows/*crowdin*",
            ".github/workflows/crowdin-download.yaml"
        ));
        assert!(pattern_matches(
            ".github/workflows/*crowdin*",
            ".github/workflows/upload-crowdin-files.yaml"
        ));
        assert!(!pattern_matches(
            ".github/workflows/*crowdin*",
            ".github/workflows/deploy.yaml"
        ));

        // create_service*.ex
        assert!(pattern_matches(
            "src/apps/platform_rpc/lib/platform_rpc/grpc/action/create_service*.ex",
            "src/apps/platform_rpc/lib/platform_rpc/grpc/action/create_service_foo.ex"
        ));
        assert!(pattern_matches(
            "lib/create_service*.ex",
            "lib/create_service_provider.ex"
        ));
    }

    #[test]
    fn test_star_prefix_suffix() {
        // appointment_review* matches appointment_review.ex, appointment_review_test.ex
        assert!(pattern_matches(
            "src/apps/platform/lib/schemas/appointment_review*",
            "src/apps/platform/lib/schemas/appointment_review.ex"
        ));
        assert!(pattern_matches(
            "src/apps/platform/lib/schemas/appointment_review*",
            "src/apps/platform/lib/schemas/appointment_review_test.ex"
        ));
        assert!(!pattern_matches(
            "src/apps/platform/lib/schemas/appointment_review*",
            "src/apps/platform/lib/schemas/other.ex"
        ));
    }

    // Subsumption tests
    #[test]
    fn test_subsumes_identical() {
        assert!(pattern_subsumes("*.rs", "*.rs"));
        assert!(pattern_subsumes("/src/", "/src/"));
        assert!(pattern_subsumes("Makefile", "Makefile"));
    }

    #[test]
    fn test_subsumes_wildcard() {
        // * subsumes everything
        assert!(pattern_subsumes("*.rs", "*"));
        assert!(pattern_subsumes("*.go", "*"));
        assert!(pattern_subsumes("/src/", "*"));
        assert!(pattern_subsumes("Makefile", "*"));
        assert!(pattern_subsumes("src/main.rs", "*"));

        // ** also subsumes everything
        assert!(pattern_subsumes("*.rs", "**"));
        assert!(pattern_subsumes("/src/lib/", "**"));
    }

    #[test]
    fn test_subsumes_extension() {
        // *.rs.bak subsumed by *.bak
        assert!(pattern_subsumes("*.rs.bak", "*.bak"));
        // but not the other way
        assert!(!pattern_subsumes("*.bak", "*.rs.bak"));
        // *.rs not subsumed by *.go
        assert!(!pattern_subsumes("*.rs", "*.go"));
    }

    #[test]
    fn test_subsumes_directory() {
        // /src/lib/ subsumed by /src/
        assert!(pattern_subsumes("/src/lib/", "/src/"));
        assert!(pattern_subsumes("src/lib/", "src/"));
        // /src/** also subsumes /src/lib/
        assert!(pattern_subsumes("/src/lib/", "/src/**"));
        // but /src/ not subsumed by /src/lib/
        assert!(!pattern_subsumes("/src/", "/src/lib/"));
    }

    #[test]
    fn test_subsumes_file_in_dir() {
        // src/main.rs subsumed by src/
        assert!(pattern_subsumes("src/main.rs", "src/"));
        assert!(pattern_subsumes("src/main.rs", "src/**"));
        // but not by a different dir
        assert!(!pattern_subsumes("src/main.rs", "lib/"));
    }

    #[test]
    fn test_not_subsumed() {
        // Different extensions
        assert!(!pattern_subsumes("*.rs", "*.go"));
        // Different directories
        assert!(!pattern_subsumes("/src/", "/lib/"));
        // Wildcard doesn't subsume specific
        assert!(!pattern_subsumes("*", "*.rs"));
    }
}
