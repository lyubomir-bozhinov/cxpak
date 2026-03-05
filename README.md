# cxpak

> Spends CPU cycles so you don't spend tokens. The LLM gets a briefing packet instead of a flashlight in a dark room.

A Rust CLI that indexes codebases using tree-sitter and produces token-budgeted context bundles for LLMs.

## Installation

```bash
cargo install cxpak
```

## Usage

```bash
# Structured repo summary within a token budget
cxpak overview --tokens 50k

# Trace from a function/error, pack relevant code paths
cxpak trace --tokens 50k "function_name"
```

## License

MIT
