# `arcella-inspect`

> **Static code introspection for Rust — structured, machine-readable metadata for AI agents, architecture tools, and automated analysis.**

`arcella-inspect` is a Rust library and CLI tool that parses Rust source code and extracts rich structural metadata — functions, structs, calls, attributes, documentation — into a clean, hierarchical **YAML 1.2** format. Designed for integration with AI-driven code understanding, dependency mapping, documentation generation, and architectural audits.

[![crates.io](https://img.shields.io/crates/v/arcella-inspect.svg)](https://crates.io/crates/arcella-inspect)  
[![docs.rs](https://img.shields.io/docsrs/arcella-inspect)](https://docs.rs/arcella-inspect)  
[![License: Apache-2.0/MIT](https://img.shields.io/badge/license-Apache%202.0%20%7C%20MIT-blue)](https://github.com/ArcellaTeam/arcella-rust-inspect)

Part of the [**Arcella**](https://github.com/ArcellaTeam) ecosystem.

---

## 🚀 Features

- ✅ **Accurate AST-based analysis** using `syn`
- ✅ **Full function signatures**: parameters, return types, attributes (`pub`, `async`, `unsafe`, etc.)
- ✅ **Call graph extraction** with internal/external call distinction
- ✅ **Documentation strings** from `///` and `#[doc = "..."]`
- ✅ **Cargo workspace support** — each crate analyzed as a subproject
- ✅ **YAML 1.2 output** — human-readable and AI-friendly
- ✅ **Extensible format** designed for future enrichment (AI tags, performance hints, etc.)

---

## 📦 Installation

### As a CLI tool

```bash
cargo install arcella-inspect
```

### As a library (in your `Cargo.toml`)

```toml
[dependencies]
arcella-inspect = "0.1"
```

---

## ▶️ Usage

### CLI: Analyze a project and output YAML

```bash
# Analyze current directory
arc-inspect

# Analyze specific project
arc-inspect ./my-rust-project

# Pipe to file
arc-inspect > architecture.yaml
```

### Library: Use in your own tool

```rust
use arcella_inspect::{analyze_project, analysis_to_yaml};

let root = std::path::Path::new("./my-project");
let analysis = analyze_project(root)?;
let yaml = analysis_to_yaml(&analysis, root)?;

println!("{}", yaml);
```

---

## 📊 Output Format (Simplified)

```yaml
version: "1.0"
subprojects:
  - name: "auth-service"
    root: "crates/auth"
    functions:
      - name: "validate_token"
        file: "src/validator.rs"
        line: 34
        returns: "Result<Claims, AuthError>"
        parameters:
          - "token: &str"
          - "secret: &[u8]"
        attributes: ["pub", "async"]
        docstring: "Validates a JWT token using the provided secret."
        calls:
          - name: "jsonwebtoken::decode"
            external: true
          - name: "metrics::record_auth_attempt"
            file: "src/metrics.rs"
            line: 12
```

👉 See the full [**output format specification**](../FORMAT.md) for details.

---

## 🛠 Use Cases

- **AI code agents**: feed structured Rust metadata to LLMs for reasoning
- **Architecture visualization**: generate call graphs and module maps
- **Documentation automation**: extract public APIs with docs and signatures
- **Security & compliance**: audit for unsafe code, external dependencies, or missing docs
- **Refactoring assistance**: identify dead code, tight coupling, or complex call chains

---

## 🧪 Example

Given `src/lib.rs`:

```rust
/// Adds two numbers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

Running `arc-inspect` produces:

```yaml
version: "1.0"
subprojects:
  - name: "my-crate"
    root: "."
    functions:
      - name: "add"
        file: "src/lib.rs"
        line: 2
        returns: "i32"
        parameters:
          - "a: i32"
          - "b: i32"
        docstring: "Adds two numbers."
        attributes: ["pub"]
        calls: []
```

---

## 📚 Documentation

- [Output Format (English)](../FORMAT.md)
- [Output Format (Russian)](../FORMAT-ru.md)

---

## 📎 License

Dual-licensed under **MIT** or **Apache 2.0** — your choice.

---

## 🤝 Contributing

Contributions are welcome! Please open an issue or PR on [GitHub](https://github.com/ArcellaTeam/arcella-rust-inspect).

Planned improvements:
- Support for enums, traits, and constants
- Performance optimizations for large codebases
- JSON and NDJSON output formats
- Integration with `rust-analyzer` LSP for real-time analysis

---