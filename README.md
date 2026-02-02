# codeowners-lsp

Language server providing CODEOWNERS information via hover and inlay hints.

## Features

- **Hover**: Shows file ownership when hovering over any code
- **Inlay Hints**: Displays ownership at the top of each file

## Installation

Download the latest release for your platform from [Releases](https://github.com/radiosilence/codeowners-lsp/releases).

### Zed

Use the [codeowners-zed](https://github.com/radiosilence/codeowners-zed) extension which automatically downloads and manages the LSP.

### Manual

```bash
# Add to PATH or configure your editor to use the binary
codeowners-lsp
```

The LSP communicates over stdio.

## Configuration

The LSP automatically finds CODEOWNERS files in standard locations:
- `.github/CODEOWNERS`
- `CODEOWNERS`
- `docs/CODEOWNERS`

### Custom Path

Pass a custom path via initialization options:

```json
{
  "path": "custom/CODEOWNERS"
}
```

## License

MIT
