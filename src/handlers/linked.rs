//! Linked editing ranges - edit owner in multiple places at once

use tower_lsp::lsp_types::*;

use crate::parser::{find_owner_at_position, parse_codeowners_file_with_positions, CodeownersLine};

/// Find all ranges where the same owner appears for linked editing
pub fn linked_editing_ranges(content: &str, position: Position) -> Option<LinkedEditingRanges> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = position.line as usize;

    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];
    let char_idx = position.character as usize;

    // Find owner at cursor position
    let owner = find_owner_at_position(line, char_idx)?;

    // Find all occurrences of this owner
    let parsed = parse_codeowners_file_with_positions(content);
    let mut ranges = Vec::new();

    for parsed_line in &parsed {
        if let CodeownersLine::Rule {
            owners: line_owners,
            ..
        } = &parsed_line.content
        {
            let line_text = lines.get(parsed_line.line_number as usize).unwrap_or(&"");

            for (idx, o) in line_owners.iter().enumerate() {
                if o == &owner {
                    if let Some(pos) = find_nth_owner_position(line_text, &owner, idx) {
                        ranges.push(Range {
                            start: Position {
                                line: parsed_line.line_number,
                                character: pos as u32,
                            },
                            end: Position {
                                line: parsed_line.line_number,
                                character: (pos + owner.len()) as u32,
                            },
                        });
                    }
                }
            }
        }
    }

    if ranges.len() <= 1 {
        // No point in linked editing if only one occurrence
        return None;
    }

    Some(LinkedEditingRanges {
        ranges,
        word_pattern: None,
    })
}

/// Find the nth occurrence of an owner string in a line
fn find_nth_owner_position(line: &str, owner: &str, n: usize) -> Option<usize> {
    let mut count = 0;
    let mut start = 0;
    while let Some(pos) = line[start..].find(owner) {
        let abs_pos = start + pos;
        // Verify it's a whole word (not part of pattern)
        let before_ok = abs_pos == 0
            || line
                .chars()
                .nth(abs_pos - 1)
                .map(|c| c.is_whitespace())
                .unwrap_or(true);
        let after_ok = abs_pos + owner.len() >= line.len()
            || line
                .chars()
                .nth(abs_pos + owner.len())
                .map(|c| c.is_whitespace())
                .unwrap_or(true);

        if before_ok && after_ok {
            if count == n {
                return Some(abs_pos);
            }
            count += 1;
        }
        start = abs_pos + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linked_editing_single_occurrence() {
        let content = "*.rs @owner";
        let result = linked_editing_ranges(
            content,
            Position {
                line: 0,
                character: 6,
            },
        );
        // Single occurrence, no linked editing
        assert!(result.is_none());
    }

    #[test]
    fn test_linked_editing_multiple_occurrences() {
        let content = "*.rs @owner\n*.js @owner";
        let result = linked_editing_ranges(
            content,
            Position {
                line: 0,
                character: 6,
            },
        );
        assert!(result.is_some());
        let ranges = result.unwrap().ranges;
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn test_linked_editing_not_on_owner() {
        let content = "*.rs @owner";
        let result = linked_editing_ranges(
            content,
            Position {
                line: 0,
                character: 1,
            },
        );
        assert!(result.is_none());
    }
}
