# Extension Update Guide: New LSP Capabilities (v0.8.0)

This document describes LSP features in codeowners-lsp. Most work automatically without extension changes, but some require client-side support to be fully utilized.

## New in v0.8.0

| Feature | LSP Method | Auto-works? | Notes |
|---------|------------|-------------|-------|
| Signature Help | `textDocument/signatureHelp` | Yes | Pattern syntax docs |
| Selection Range | `textDocument/selectionRange` | Yes | Smart expand |
| Linked Editing | `textDocument/linkedEditingRange` | Yes | Multi-cursor owners |
| Pattern Hover | `textDocument/hover` | Yes | Show matching files |
| Related Diagnostics | N/A | Yes | Click to jump |

## v0.7.0 Capabilities

| Feature | LSP Method | Auto-works? | Notes |
|---------|------------|-------------|-------|
| Document Symbols | `textDocument/documentSymbol` | Yes | Outline view |
| Folding Ranges | `textDocument/foldingRange` | Yes | Collapse sections |
| Semantic Tokens | `textDocument/semanticTokens/full` | Needs theme | Syntax highlighting |
| Find References | `textDocument/references` | Yes | Find owner usages |
| Rename | `textDocument/rename` | Yes | Rename owners |
| Workspace Symbols | `workspace/symbol` | Yes | Search patterns/owners |
| Code Lens | `textDocument/codeLens` | Yes | Inline file counts |

## v0.8.0 Features Detail

### Signature Help

**LSP Method:** `textDocument/signatureHelp`

Shows documentation for glob pattern syntax when typing:
- `*` - Match any characters (single segment)
- `**` - Match any characters (recursive)
- `?` - Match single character
- `[...]` - Match character class
- `/` - Directory separator / anchor

**Trigger Characters:** `*`, `?`, `[`

**Extension Work:** None required.

### Selection Range

**LSP Method:** `textDocument/selectionRange`

Smart expand selection hierarchy:
1. Current word (owner or pattern segment)
2. All owners on line (if in owners section)
3. Whole rule line
4. Comment block (if in comment)
5. Section (from header to next section)
6. Entire file

**Extension Work:** None required. Use "Expand Selection" command (Cmd+Shift+→).

### Linked Editing

**LSP Method:** `textDocument/linkedEditingRange`

When cursor is on an `@owner`, returns all occurrences of that owner in the file. Editing one updates all simultaneously.

**Extension Work:** May need to enable linked editing mode. In VSCode: `editor.linkedEditing: true`.

### Pattern Hover (Enhanced)

Hovering over a pattern now shows matching files:
- File count
- Up to 10 sample file paths
- "...and N more" for large matches

### Related Diagnostics

"Shadowed rule" diagnostics now include `relatedInformation` linking to the shadowing rule. Click to navigate.

## 1. Document Symbols

**LSP Method:** `textDocument/documentSymbol`

Returns hierarchical outline of CODEOWNERS file:
- Section headers (comments starting with uppercase) become `Namespace` symbols
- Rules become `File` symbols nested under sections
- Pattern is the symbol name, owners shown in `detail`

**Extension Work:** None required. Editors with outline/breadcrumb support will display automatically.

**Response Structure:**
```typescript
DocumentSymbol {
  name: string,        // Pattern (e.g., "*.rs") or section name
  detail?: string,     // Owners joined by space
  kind: SymbolKind,    // NAMESPACE for sections, FILE for rules
  range: Range,
  selectionRange: Range,
  children?: DocumentSymbol[]  // Rules nested under sections
}
```

## 2. Folding Ranges

**LSP Method:** `textDocument/foldingRange`

Returns two types of foldable regions:
- **Comment blocks** (consecutive `#` lines) - `FoldingRangeKind.Comment`
- **Sections** (comment header through rules until next section) - `FoldingRangeKind.Region`

**Extension Work:** None required. Editors handle automatically.

## 3. Semantic Tokens

**LSP Method:** `textDocument/semanticTokens/full`

Provides syntax highlighting tokens:

| Token Type | Index | Usage |
|------------|-------|-------|
| `comment` | 0 | Lines starting with `#` |
| `string` | 1 | Patterns without globs |
| `variable` | 2 | `@user` owners |
| `class` | 3 | `@org/team` owners |
| `operator` | 4 | Glob chars: `*`, `?`, `[`, `]` |

**Extension Work Required:**

### VSCode
Add semantic token color customizations to theme or extension:
```json
{
  "semanticTokenColors": {
    "comment": "#6A9955",
    "string": "#CE9178",
    "variable": "#9CDCFE",
    "class": "#4EC9B0",
    "operator": "#D4D4D4"
  }
}
```

Or register a `DocumentSemanticTokensProvider` if not using built-in LSP client.

### Zed
Semantic tokens should work automatically with themes. If custom highlighting needed, map in `languages/codeowners/highlights.scm` or extension config.

## 4. Find All References

**LSP Method:** `textDocument/references`

When cursor is on an `@owner` in CODEOWNERS file, returns all locations where that owner appears.

**Extension Work:** None required. Works with standard "Find References" command.

**Use Case:** See all rules a team/person owns before removing them.

## 5. Rename

**LSP Methods:** 
- `textDocument/prepareRename` - Validates rename is possible, returns range
- `textDocument/rename` - Returns workspace edit

Renames an `@owner` across all rules in CODEOWNERS file.

**Extension Work:** None required. Works with standard "Rename Symbol" command.

**Use Case:** Team renamed? User changed handle? One command updates everywhere.

## 6. Workspace Symbols

**LSP Method:** `workspace/symbol`

Search across all patterns and owners:
- Patterns return as `SymbolKind.File` with owners as container
- Owners return as `SymbolKind.Class` (teams) or `SymbolKind.Constant` (users) with pattern as container
- Query filters by substring match (case-insensitive)

**Extension Work:** None required. Works with standard "Go to Symbol in Workspace" (Cmd+T / Ctrl+T).

**Use Case:** Quickly jump to any rule or find all rules for an owner.

## 7. Code Lens

**LSP Method:** `textDocument/codeLens`

Shows inline information above each rule:
```
5 files · @team-frontend @user
/src/components/**/*.tsx  @team-frontend @user
```

The lens displays: `{count} files · {owners}`

**Extension Work:** None required if code lens is enabled. Some editors disable by default.

### VSCode
Enable in settings:
```json
{
  "editor.codeLens": true
}
```

### Zed
Code lens support may vary. Check Zed's LSP documentation for current status.

## Server Capabilities Advertised

```json
{
  "capabilities": {
    "documentSymbolProvider": true,
    "foldingRangeProvider": true,
    "referencesProvider": true,
    "renameProvider": {
      "prepareProvider": true
    },
    "workspaceSymbolProvider": true,
    "codeLensProvider": {
      "resolveProvider": false
    },
    "semanticTokensProvider": {
      "legend": {
        "tokenTypes": ["comment", "string", "variable", "class", "operator"],
        "tokenModifiers": []
      },
      "full": true,
      "range": false
    }
  }
}
```

## Testing the Features

### Quick Test Commands

**Document Symbols:** Open CODEOWNERS, use "Go to Symbol in Editor" (Cmd+Shift+O)

**Folding:** Look for fold indicators on comment blocks and sections

**Semantic Tokens:** Check if patterns, owners, and comments have different colors

**References:** Place cursor on `@owner`, trigger "Find All References"

**Rename:** Place cursor on `@owner`, trigger "Rename Symbol", enter new name

**Workspace Symbols:** Use "Go to Symbol in Workspace" (Cmd+T), type owner or pattern

**Code Lens:** Check for text above each rule showing file count

## Migration Notes

- All features are additive - existing functionality unchanged
- No breaking changes to existing capabilities
- Clients that don't support these features simply ignore the capabilities
