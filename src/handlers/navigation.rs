//! Navigation handlers: references, rename

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::parser::{find_owner_at_position, parse_codeowners_file_with_positions, CodeownersLine};

/// Find all references to an owner in CODEOWNERS file
pub fn find_references(content: &str, position: Position, uri: &Url) -> Option<Vec<Location>> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];
    let char_idx = position.character as usize;

    // Find owner at cursor position
    let owner = find_owner_at_position(line, char_idx)?;

    // Find all lines containing this owner
    let parsed = parse_codeowners_file_with_positions(content);
    let mut locations = Vec::new();

    for parsed_line in &parsed {
        if let CodeownersLine::Rule {
            owners: line_owners,
            ..
        } = &parsed_line.content
        {
            for (idx, o) in line_owners.iter().enumerate() {
                if o == &owner {
                    let line_text = lines.get(parsed_line.line_number as usize).unwrap_or(&"");
                    if let Some(pos) = find_nth_owner_position(line_text, &owner, idx) {
                        locations.push(Location {
                            uri: uri.clone(),
                            range: Range {
                                start: Position {
                                    line: parsed_line.line_number,
                                    character: pos as u32,
                                },
                                end: Position {
                                    line: parsed_line.line_number,
                                    character: (pos + owner.len()) as u32,
                                },
                            },
                        });
                    }
                }
            }
        }
    }

    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}

/// Prepare rename: validate and return the range of the owner to rename
pub fn prepare_rename(content: &str, position: Position) -> Option<Range> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];
    let char_idx = position.character as usize;

    // Only allow renaming @owners
    let owner = find_owner_at_position(line, char_idx)?;

    // Find the range of this owner
    line.find(&owner).map(|start_pos| Range {
        start: Position {
            line: position.line,
            character: start_pos as u32,
        },
        end: Position {
            line: position.line,
            character: (start_pos + owner.len()) as u32,
        },
    })
}

/// Rename an owner across all rules
pub fn rename_owner(
    content: &str,
    position: Position,
    new_name: &str,
    uri: &Url,
) -> Option<WorkspaceEdit> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];
    let char_idx = position.character as usize;

    let old_owner = find_owner_at_position(line, char_idx)?;

    let parsed = parse_codeowners_file_with_positions(content);
    let mut edits = Vec::new();

    for parsed_line in &parsed {
        if let CodeownersLine::Rule {
            owners: line_owners,
            ..
        } = &parsed_line.content
        {
            for (idx, o) in line_owners.iter().enumerate() {
                if o == &old_owner {
                    let line_text = lines.get(parsed_line.line_number as usize).unwrap_or(&"");
                    if let Some(pos) = find_nth_owner_position(line_text, &old_owner, idx) {
                        edits.push(TextEdit {
                            range: Range {
                                start: Position {
                                    line: parsed_line.line_number,
                                    character: pos as u32,
                                },
                                end: Position {
                                    line: parsed_line.line_number,
                                    character: (pos + old_owner.len()) as u32,
                                },
                            },
                            new_text: new_name.to_string(),
                        });
                    }
                }
            }
        }
    }

    if edits.is_empty() {
        None
    } else {
        let mut changes = HashMap::new();
        changes.insert(uri.clone(), edits);
        Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        })
    }
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
