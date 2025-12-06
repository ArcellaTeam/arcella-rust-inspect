# 📊 Output Format of `arcella-inspect`

The `arcella-inspect` utility (part of the **Arcella** ecosystem) performs static analysis of Rust projects and produces structured metadata about the source code as a hierarchical **YAML 1.2** document. This format is optimized for downstream consumption by AI agents, dependency analyzers, documentation generators, and architectural auditing tools.

## 🧭 Core Principles

1. **Validity**: The output is always a strictly valid YAML 1.2 document.
2. **Hierarchy**: Functions and their calls are represented declaratively, preserving both local and fully qualified names.
3. **Identification**: Every element (function, struct) includes a unique name and associated metadata (file path, line number).
4. **Subprojects**: Supports Cargo workspaces—each workspace member is treated as an independent subproject.
5. **Metadata**: For each element, the following are preserved:
   - File path (relative to the subproject root),
   - Line number,
   - Type (`function`, `struct`),
   - Attributes, signature, and documentation.

---

## 🗂 YAML Document Structure

```yaml
version: "1.0"                     # arcella-inspect format version
project_name: "my-platform"        # Top-level project name (optional)
subprojects:
  - name: "auth-service"           # Crate name (from Cargo.toml `package.name`)
    root: "crates/auth/"           # Path from analysis root to subproject root
    structures:
      - name: "AuthState"
        file: "src/state.rs"
        line: 8
    functions:
      - name: "validate_token"
        file: "src/validator.rs"
        line: 34
        returns: "Result<Claims, AuthError>"
        parameters:
          - "token: &str"
          - "secret: &[u8]"
        docstring: "Validates a JWT token using the provided secret."
        attributes:
          - "pub"
          - "async"
        calls:
          - name: "jsonwebtoken::decode"
            external: true
          - name: "metrics::record_auth_attempt"
            file: "src/metrics.rs"
            line: 12
    # (optional) flat call list — for simplified graph analysis
    edges:
      - from: "validate_token"
        to: "metrics::record_auth_attempt"
```

---

## 🔍 Format Details

### 1. `version`

- **Type**: string  
- **Value**: `"1.0"`  
- **Purpose**: Ensures backward compatibility for future format revisions.

### 2. `project_name` (optional)

- Derived from `workspace.package.name` or the root `Cargo.toml`.
- May be omitted—the `subprojects` section provides full identification.

### 3. `subprojects[]`

Each subproject corresponds to a crate in a workspace or a standalone project.

| Field        | Required | Description |
|--------------|----------|-------------|
| `name`       | ✅       | Crate name (`package.name` from `Cargo.toml`) |
| `root`       | ✅       | Relative path from analysis root to subproject root |
| `structures` | ❌       | List of structs |
| `functions`  | ✅       | List of all functions and methods |
| `edges`      | ❌       | Flat list of function calls (for call-graph tools) |

---

### 4. `structures[]` Items

| Field  | Required | Description |
|--------|----------|-------------|
| `name` | ✅       | Struct name (`struct MyStruct`) |
| `file` | ✅       | Path relative to subproject `root` |
| `line` | ✅       | Line number of definition |

> Traits, enums, and modules may be added in future versions.

---

### 5. `functions[]` Items

| Field        | Required | Description |
|--------------|----------|-------------|
| `name`       | ✅       | Fully qualified name: `module::MyStruct::method` or `free_function` |
| `file`       | ✅       | Path relative to subproject `root` |
| `line`       | ✅       | Line number of definition |
| `returns`    | ✅       | String representation of return type (`"Result<(), Error>"`) |
| `parameters` | ❌       | Parameter list: `["config: &Config", "ctx: &Context"]` |
| `docstring`  | ❌       | Content of `///` comments, normalized (newlines preserved, prefixes stripped) |
| `attributes` | ❌       | List including: `"pub"`, `"async"`, `"unsafe"`, `"const"`, `"#[test]"`, `"#[instrument]"`, etc. |
| `calls`      | ✅       | List of called functions (see below) |

---

### 6. `calls[]` Items (within a function)

| Field      | Required | Description |
|------------|----------|-------------|
| `name`     | ✅       | Fully qualified name of called function (`tokio::time::sleep`, `self::helper`) |
| `file`     | ❌       | Present only for **internal** calls (defined within the same project) |
| `line`     | ❌       | Present only if `file` is present |
| `external` | ❌       | `true` if the call originates from an external crate (`std`, `tokio`, `serde`, etc.) |

> When `external: true`, the `file` and `line` fields **must not be present** (or should be ignored by parsers).

---

### 7. Flat `edges[]` List (optional)

For simplified call-graph construction:

```yaml
edges:
  - from: "auth::validate_token"
    to: "metrics::record_auth_attempt"
  - from: "auth::validate_token"
    to: "jsonwebtoken::decode"
```

- Both `from` and `to` use fully qualified function names.
- Intended for tools that prefer flat call relationships over nested structures.

---

## 🔧 Minimal Valid Output Example

```yaml
version: "1.0"
subprojects:
  - name: "hello"
    root: "."
    functions:
      - name: "main"
        file: "src/main.rs"
        line: 1
        returns: "()"
        attributes:
          - "pub"
        calls:
          - name: "println!"
            external: true
```

---

## ⚙️ Implementation Notes (Internal)

- Analysis is based on the **AST** via `syn`, not MIR or HIR—ensuring faithful representation of source code.
- Parameter and return types are serialized **as written**, without compiler normalization.
- `docstring` is reconstructed from all `#[doc = "..."]` and `///` lines, joined with `\n`.
- Attributes include both **language keywords** (`pub`, `async`) and **attribute macros** (`#[cfg(...)]`, `#[instrument]`).
- External calls are inferred by absence from the current subproject’s function index.

---

## 🧩 Extensibility

The format is designed for future enhancements:

- Custom tags: `tags: ["hot-path", "security-critical"]`
- AI-generated annotations: `ai_summary: "Auth validator"`
- Support for traits, impl blocks, constants
- Integration with profiling data: `avg_latency_ms: 12.4`

All new fields will be **optional** and will not break backward compatibility.