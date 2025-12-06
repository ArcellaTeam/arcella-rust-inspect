// arcella-rust-inspect/arcella-inspect/src/lib.rs
//
// Copyright (c) 2025 Alexey Rybakov, Arcella Team
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE>
// or the MIT license <LICENSE-MIT>, at your option.
// This file may not be copied, modified, or distributed
// except according to those terms.

//! `arcella-inspect`: Structured static analysis of Rust source code.
//!
//! This crate provides tools to extract rich metadata from Rust projects:
//! - Functions and methods (with full paths, e.g. `my_mod::MyStruct::method`)
//! - Structs
//! - Call graphs (internal vs external calls)
//! - Function signatures (parameters, return types)
//! - Documentation comments (`///`)
//! - Attributes (`pub`, `async`, `#[cfg(...)]`, etc.)
//!
//! The primary output format is **YAML 1.2**, designed for consumption by:
//! - AI code reasoning agents
//! - Architecture visualization tools
//! - Documentation generators
//! - Dependency and compliance auditors
//!
//! ## Usage
//!
//! ### As a library
//!
//! ```rust
//! use arcella_inspect::{analyze_project, analysis_to_yaml};
//! use std::path::Path;
//!
//! let root = Path::new("./my-rust-project");
//! let analysis = analyze_project(root)?;
//! let yaml = analysis_to_yaml(&analysis, root)?;
//! println!("{}", yaml);
//! ```
//!
//! ### As a CLI
//!
//! Install via `cargo install arcella-inspect` and run:
//!
//! ```bash
//! arc-inspect ./my-project > metadata.yaml
//! ```
//!
//! ## Output Format
//!
//! See the full specification in [`FORMAT.md`](https://github.com/arcella-team/arcella-inspect/blob/main/FORMAT.md).
//!
//! ## Design Notes
//!
//! - Analysis is based on the **AST** (via `syn`), not MIR/HIRpreserving source-level fidelity.
//! - Only **meaningful function calls** are recorded (noise like `.clone()`, `.unwrap()` is filtered).
//! - Supports **Cargo workspaces** (each crate = subproject).
//! - External calls (e.g. from `std`, `tokio`) are marked with `external: true`.

use std::{
    collections::HashMap,
    fs,
    path::{Path},
};
use syn::{
    Attribute,
    ItemFn, ItemStruct, ImplItem,
    Meta,
    Visibility,
    visit::Visit,
    FnArg, Pat, Type, ReturnType, Expr, ExprCall, ExprMethodCall, ExprPath,
};
use walkdir::{WalkDir, DirEntry};
use quote::quote;
use serde_yaml_ng as serde_yaml;

// =============== PUBLIC API ===============

pub use data::{
    AnalysisResult, StructDecl, FunctionDecl,
    YamlOutput, YamlSubproject, YamlFunction, YamlStruct, YamlCall,
};

/// Analyzes a Rust project directory and returns structured metadata about its code.
///
/// This function recursively scans all `.rs` files (excluding common ignored paths like `target/`,
/// `.git/`, etc.), parses them using `syn`, and extracts:
/// - Struct definitions
/// - Free functions and methods
/// - Function calls (filtered for semantic relevance)
/// - Documentation, attributes, and type signatures
///
/// # Arguments
///
/// * `root` - Path to the root of the Rust project (must contain `Cargo.toml` or be a valid crate/workspace root).
///
/// # Returns
///
/// A `Result` containing an [`AnalysisResult`] on success, or an error if:
/// - `root` is not a directory
/// - I/O errors occur during file reading
///
/// # Example
///
/// ```rust
/// use arcella_inspect::analyze_project;
/// use std::path::Path;
///
/// let analysis = analyze_project(Path::new("./examples/hello-world"))?;
/// println!("Found {} functions", analysis.functions.len());
/// ```
pub fn analyze_project(root: &Path) -> Result<AnalysisResult, Box<dyn std::error::Error>> {
    if !root.is_dir() {
        return Err(format!("'{}' is not a directory", root.display()).into());
    }
    let (structs, functions) = collect_all_items(root)?;
    Ok(AnalysisResult { structs, functions })
}

/// Serializes an [`AnalysisResult`] to a YAML 1.2 string.
///
/// The output conforms to the `arcella-inspect` metadata schema:
/// - Includes crate name (from `Cargo.toml`)
/// - Maps each function to its calls, distinguishing internal vs external
/// - Preserves file paths relative to the project root
///
/// # Arguments
///
/// * `analysis` - The result of a prior call to [`analyze_project`].
/// * `root` - The same project root passed to `analyze_project`.
///
/// # Returns
///
/// A `Result` containing a YAML string on success, or an error if:
/// - `Cargo.toml` cannot be read or parsed
/// - Serialization fails
///
/// # Example
///
/// ```rust
/// use arcella_inspect::{analyze_project, analysis_to_yaml};
/// use std::path::Path;
///
/// let root = Path::new("./my-crate");
/// let analysis = analyze_project(root)?;
/// let yaml = analysis_to_yaml(&analysis, root)?;
/// assert!(yaml.contains("version: \"1.0\""));
/// ```
pub fn analysis_to_yaml(
    analysis: &AnalysisResult,
    root: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let cargo_toml = root.join("Cargo.toml");
    let crate_name = read_crate_name(&cargo_toml);

    let index = index_functions(&analysis.functions);

    let yaml_functions: Vec<YamlFunction> = analysis
        .functions
        .iter()
        .map(|f| {
            let calls: Vec<YamlCall> = f
                .calls
                .iter()
                .map(|call_name| {
                    if index.contains_key(call_name) {
                        let target = &index[call_name];
                        YamlCall {
                            name: call_name.clone(),
                            file: Some(target.file.clone()),
                            line: Some(target.line),
                            external: None,
                        }
                    } else {
                        YamlCall {
                            name: call_name.clone(),
                            file: None,
                            line: None,
                            external: Some(true),
                        }
                    }
                })
                .collect();

            YamlFunction {
                name: f.full_name.clone(),
                file: f.file.clone(),
                line: f.line,
                returns: f.returns.clone(),
                parameters: if f.parameters.is_empty() {
                    None
                } else {
                    Some(f.parameters.clone())
                },
                docstring: f.docstring.clone(),
                attributes: f.attributes.clone(),
                calls,
            }
        })
        .collect();

    let yaml_structs: Vec<YamlStruct> = analysis
        .structs
        .iter()
        .map(|s| YamlStruct {
            name: s.name.clone(),
            file: s.file.clone(),
            line: s.line,
        })
        .collect();

    let subproject = YamlSubproject {
        name: crate_name,
        root: ".".to_string(),
        structures: yaml_structs,
        functions: yaml_functions,
    };

    let output = YamlOutput {
        version: "1.0".to_string(),
        project_name: None,
        subprojects: vec![subproject],
    };

    let yaml = serde_yaml::to_string(&output)?;
    Ok(yaml)
}

// =============== INTERNAL MODULES ===============

/// Internal data structures for analysis results and YAML serialization.
mod data {
    use serde::Serialize;

    /// Represents a struct definition found in the source code.
    #[derive(Debug, Clone)]
    pub struct StructDecl {
        /// Name of the struct (e.g. `"AuthState"`).
        pub name: String,
        /// File path relative to the subproject root (e.g. `"src/state.rs"`).
        pub file: String,
        /// Line number where the struct is defined.
        pub line: usize,
    }

    /// Represents a function or method found in the source code.
    #[derive(Debug, Clone)]
    pub struct FunctionDecl {
        /// Fully qualified name (e.g. `"my_mod::Auth::validate"` or `"free_function"`).
        pub full_name: String,
        /// File path relative to the subproject root.
        pub file: String,
        /// Line number of the function definition.
        pub line: usize,
        /// String representation of the return type (e.g. `"Result<(), Error>"`).
        pub returns: String,
        /// List of parameters in `"name: Type"` format.
        pub parameters: Vec<String>,
        /// Documentation comment text (if any), with newlines preserved.
        pub docstring: Option<String>,
        /// List of attributes and qualifiers:
        /// - Language keywords: `"pub"`, `"async"`, `"unsafe"`, `"const"`
        /// - Attribute macros: `"#[test]"`, `"#[instrument]"`
        pub attributes: Vec<String>,
        /// List of **meaningful** called function names (filtered for noise).
        pub calls: Vec<String>,
    }

    /// The top-level result of a project analysis.
    #[derive(Debug)]
    pub struct AnalysisResult {
        /// All struct definitions found.
        pub structs: Vec<StructDecl>,
        /// All function and method definitions found.
        pub functions: Vec<FunctionDecl>,
    }

    // === YAML OUTPUT STRUCTS ===

    /// YAML representation of a struct.
    #[derive(Serialize, Debug)]
    pub struct YamlStruct {
        pub name: String,
        pub file: String,
        pub line: usize,
    }

    /// YAML representation of a function call.
    #[derive(Serialize, Debug)]
    pub struct YamlCall {
        /// Fully qualified name of the called function.
        pub name: String,
        /// Present only for internal calls: file path relative to subproject root.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub file: Option<String>,
        /// Present only for internal calls: line number.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub line: Option<usize>,
        /// `true` if the call is to an external crate (e.g. `std`, `tokio`).
        #[serde(skip_serializing_if = "Option::is_none")]
        pub external: Option<bool>,
    }

    /// YAML representation of a function or method.
    #[derive(Serialize, Debug)]
    pub struct YamlFunction {
        pub name: String,
        pub file: String,
        pub line: usize,
        pub returns: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub parameters: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub docstring: Option<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        pub attributes: Vec<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        pub calls: Vec<YamlCall>,
    }

    /// YAML representation of a crate (subproject).
    #[derive(Serialize, Debug)]
    pub struct YamlSubproject {
        /// Crate name from `Cargo.toml`.
        pub name: String,
        /// Path from analysis root to this subproject root.
        pub root: String,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        pub structures: Vec<YamlStruct>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        pub functions: Vec<YamlFunction>,
    }

    /// Top-level YAML output document.
    #[derive(Serialize, Debug)]
    pub struct YamlOutput {
        /// Schema version (`"1.0"`).
        pub version: String,
        /// Optional top-level project name (e.g. from workspace).
        #[serde(skip_serializing_if = "Option::is_none")]
        pub project_name: Option<String>,
        /// List of analyzed crates (subprojects).
        pub subprojects: Vec<YamlSubproject>,
    }
}

// =============== HELPER FUNCTIONS ===============

/// Determines if a function call name is considered "noise" and should be filtered out.
///
/// Examples of noise: `.clone()`, `.unwrap()`, iterator adapters, logging macros.
/// This improves signal-to-noise ratio in call graphs.
fn is_noise_call(name: &str) -> bool {
    const NOISE: &[&str] = &[
        "clone", "to_string", "into", "from", "as_ref", "deref", "borrow", "as_mut",
        "map", "map_err", "and_then", "or_else", "unwrap", "expect", "ok", "err",
        "is_ok", "is_err", "is_some", "is_none", "unwrap_or", "unwrap_or_else",
        "iter", "into_iter", "next", "collect", "filter", "find", "for_each",
        "enumerate", "zip", "take", "skip", "inspect",
        "push", "pop", "insert", "remove", "get", "contains", "len", "is_empty",
        "clear", "extend", "keys", "values",
        "new", "default", "Some", "Ok", "Err",
        "info", "warn", "error", "debug", "trace",
        "serialize", "deserialize",
        "join", "starts_with", "ends_with", "contains", "file_name", "extension",
    ];
    NOISE.contains(&name)
}

/// Checks whether a call name should be included in the output.
///
/// A call is meaningful if:
/// - It is not in the noise list
/// - It is not an empty or single/two-letter all-lowercase identifier (e.g. `x`, `ok`)
/// - It starts with a letter or `_`, or contains `::` (indicating a path)
fn is_meaningful_call(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if is_noise_call(name) {
        return false;
    }
    if name.len() <= 2 && name.chars().all(|c| c.is_ascii_lowercase()) {
        return false;
    }
    name.starts_with(|c: char| c.is_alphabetic() || c == '_') || name.contains("::")
}

/// Converts a `syn::Path` (e.g. `std::fmt::Display`) to a string (`"std::fmt::Display"`).
fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// Checks if an attribute is a documentation attribute (`#[doc = "..."]` or `///`).
fn is_doc_attr(attr: &Attribute) -> bool {
    attr.path().segments.len() == 1 && attr.path().is_ident("doc")
}

/// Extracts documentation from a list of attributes.
///
/// Combines all `#[doc = "..."]` and `///` lines into a single string with `\n` separators.
fn extract_docstring(attrs: &[Attribute]) -> Option<String> {
    let mut lines = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        match &attr.meta {
            Meta::NameValue(namevalue) => {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit),
                    ..
                }) = &namevalue.value
                {
                    lines.push(lit.value());
                }
            }
            Meta::List(meta_list) => {
                if let Ok(lit) = meta_list.parse_args::<syn::LitStr>() {
                    lines.push(lit.value());
                }
            }
            Meta::Path(_) => {}
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// Extracts relevant attributes from a free function (`ItemFn`).
///
/// Includes:
/// - Non-doc attributes (e.g. `#[test]`, `#[instrument]`)
/// - Visibility (`pub`)
/// - Qualifiers (`async`, `unsafe`, `const`)
fn extract_attributes_from_fn(item_fn: &ItemFn) -> Vec<String> {
    let mut attrs = Vec::new();
    for attr in &item_fn.attrs {
        if is_doc_attr(attr) {
            continue;
        }
        if let Some(seg) = attr.path().segments.last() {
            attrs.push(format!("#[{}]", seg.ident));
        }
    }
    if matches!(item_fn.vis, Visibility::Public(_)) {
        attrs.push("pub".to_string());
    }
    if item_fn.sig.asyncness.is_some() {
        attrs.push("async".to_string());
    }
    if item_fn.sig.unsafety.is_some() {
        attrs.push("unsafe".to_string());
    }
    if item_fn.sig.constness.is_some() {
        attrs.push("const".to_string());
    }
    attrs
}

/// Extracts relevant attributes from an `impl` method.
///
/// Similar to `extract_attributes_from_fn`, but operates on a `Signature` and attribute slice.
fn extract_attributes_from_impl_method(
    sig: &syn::Signature,
    attrs: &[Attribute],
) -> Vec<String> {
    let mut result = Vec::new();
    for attr in attrs {
        if is_doc_attr(attr) {
            continue;
        }
        if let Some(seg) = attr.path().segments.last() {
            result.push(format!("#[{}]", seg.ident));
        }
    }
    if sig.asyncness.is_some() {
        result.push("async".to_string());
    }
    if sig.unsafety.is_some() {
        result.push("unsafe".to_string());
    }
    if sig.constness.is_some() {
        result.push("const".to_string());
    }
    result
}

/// Formats a function's return type as a string.
///
/// Truncates very long types (>60 chars) to `"> &"` for readability in YAML.
fn format_return_type(ret: &ReturnType) -> String {
    match ret {
        ReturnType::Default => "()".to_string(),
        ReturnType::Type(_, ty) => {
            let s = quote! { #ty }.to_string();
            if s.len() > 60 {
                "> &".to_string()
            } else {
                s
            }
        }
    }
}

/// Formats function parameters as `"name: Type"` strings.
///
/// Handles:
/// - Named parameters (`x: i32`)
/// - Self receivers (`self`, `&self`, `&mut self`, `mut self`)
fn format_parameters(
    inputs: &syn::punctuated::Punctuated<FnArg,
	syn::Token![,]>
) -> Vec<String> {
    inputs.iter().map(|arg| {
        match arg {
            FnArg::Typed(pat_type) => {
                let pat_str = match &*pat_type.pat {
                    Pat::Ident(p) => p.ident.to_string(),
                    _ => "_".to_string(),
                };
                let ty_str = quote! { #pat_type.ty }.to_string();
                format!("{}: {}", pat_str, ty_str)
            }
            FnArg::Receiver(r) => {
                let mut s = String::from("self");
                if let Some((_, lifetime)) = &r.reference {
                    s.insert_str(0, "&");
                    if let Some(lt) = lifetime {
                        s.insert_str(1, &format!("{} ", lt));
                    } else {
                        s.insert(1, ' ');
                    }
                    if r.mutability.is_some() {
                        let pos = s.find(' ').unwrap_or(0) + 1;
                        s.insert_str(pos, "mut ");
                    }
                } else if r.mutability.is_some() {
                    s = "mut self".to_string();
                }
                s
            }
        }
    }).collect()
}

// =============== AST VISITORS ===============

/// Visitor that walks the AST and collects struct/function declarations.
struct FullVisitor {
    current_file: String,
    functions: Vec<FunctionDecl>,
    structs: Vec<data::StructDecl>,
}

/// Visitor that walks a function body and collects called function names.
#[derive(Default)]
struct CallVisitor {
    calls: Vec<String>,
}

impl<'ast> Visit<'ast> for FullVisitor {
    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        self.structs.push(data::StructDecl {
            name: i.ident.to_string(),
            file: self.current_file.clone(),
            line: i.ident.span().start().line,
        });
        syn::visit::visit_item_struct(self, i);
    }

    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        let name = i.sig.ident.to_string();
        let line = i.sig.ident.span().start().line;
        let returns = format_return_type(&i.sig.output);
        let parameters = format_parameters(&i.sig.inputs);
        let docstring = extract_docstring(&i.attrs);
        let attributes = extract_attributes_from_fn(i);

        let mut call_visitor = CallVisitor::default();
        call_visitor.visit_block(&i.block);

        let calls = call_visitor
            .calls
            .into_iter()
            .filter(|c| is_meaningful_call(c))
            .collect();

        self.functions.push(FunctionDecl {
            full_name: name,
            file: self.current_file.clone(),
            line,
            returns,
            parameters,
            docstring,
            attributes,
            calls,
        });
        syn::visit::visit_item_fn(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        let impl_type_name = match &*i.self_ty {
            Type::Path(type_path) => {
                type_path
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::")
            }
            _ => "UnknownType".to_string(),
        };

        for item in &i.items {
            if let ImplItem::Fn(method) = item {
                let method_name = method.sig.ident.to_string();
                let full_name = format!("{}::{}", impl_type_name, method_name);
                let line = method.sig.ident.span().start().line;
                let returns = format_return_type(&method.sig.output);
                let parameters = format_parameters(&method.sig.inputs);
                let docstring = extract_docstring(&method.attrs);
                let attributes = extract_attributes_from_impl_method(&method.sig, &method.attrs);

                let mut call_visitor = CallVisitor::default();
                call_visitor.visit_block(&method.block);

                let calls = call_visitor
                    .calls
                    .into_iter()
                    .filter(|c| is_meaningful_call(c))
                    .collect();

                self.functions.push(FunctionDecl {
                    full_name,
                    file: self.current_file.clone(),
                    line,
                    returns,
                    parameters,
                    docstring,
                    attributes,
                    calls,
                });
            }
        }
        syn::visit::visit_item_impl(self, i);
    }
}

impl<'ast> Visit<'ast> for CallVisitor {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Expr::Path(ExprPath { path, .. }) = &*node.func {
            let path_str = path_to_string(path);
            if !path_str.is_empty() {
                self.calls.push(path_str);
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let method_name = node.method.to_string();
        self.calls.push(method_name);
        syn::visit::visit_expr_method_call(self, node);
    }
}

// =============== FILE SYSTEM & PARSING ===============

/// Parses a single Rust source file and extracts metadata.
fn parse_file(
    path: &Path,
    root: &Path,
) -> Result<(Vec<data::StructDecl>, Vec<FunctionDecl>), Box<dyn std::error::Error>> {
    let code = fs::read_to_string(path)?;

    if code.trim().is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let syntax = match syn::parse_file(&code) {
        Ok(syntax) => syntax,
        Err(e) => {
            eprintln!("Skipping invalid Rust file: {}: {}", path.display(), e);
            return Ok((Vec::new(), Vec::new()));
        }
    };

    let relative_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let mut visitor = FullVisitor {
        current_file: relative_path,
        functions: Vec::new(),
        structs: Vec::new(),
    };
    visitor.visit_file(&syntax);
    Ok((visitor.structs, visitor.functions))
}

/// Checks if a directory entry is hidden (starts with `.`).
fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Determines whether a directory entry should be skipped during traversal.
///
/// Skips:
/// - Hidden files/dirs (`.git`, `.env`)
/// - Common build/output directories (`target`, `dist`, `build`, `node_modules`)
fn should_skip_entry(entry: &DirEntry) -> bool {
    if is_hidden(entry) {
        return true;
    }
    let name = entry.file_name();
    name == "target"
        || name == ".git"
        || name == "node_modules"
        || name == "dist"
        || name == "build"
}

/// Recursively collects all items from a project root.
fn collect_all_items(
    root_dir: &Path,
) -> Result<(Vec<data::StructDecl>, Vec<FunctionDecl>), Box<dyn std::error::Error>> {
    let mut all_structs = Vec::new();
    let mut all_functions = Vec::new();

    for entry in WalkDir::new(root_dir)
        .into_iter()
        .filter_entry(|e| !should_skip_entry(e))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
        if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
            if name.contains('~') || name.ends_with(".bk") || name.ends_with(".tmp") {
                continue;
            }
        }

        let (structs, functions) = parse_file(entry.path(), root_dir)?;
        all_structs.extend(structs);
        all_functions.extend(functions);
    }

    Ok((all_structs, all_functions))
}

/// Builds an index of functions by their full name for fast lookup.
fn index_functions(functions: &[FunctionDecl]) -> HashMap<String, &FunctionDecl> {
    let mut map = HashMap::new();
    for f in functions {
        map.insert(f.full_name.clone(), f);
    }
    map
}

/// Reads the crate name from `Cargo.toml`.
///
/// Returns `"unknown"` if the file is missing or malformed.
fn read_crate_name(cargo_toml_path: &Path) -> String {
    let contents = match fs::read_to_string(cargo_toml_path) {
        Ok(c) => c,
        Err(_) => return "unknown".to_string(),
    };
    let contents = contents.trim_start_matches('\u{feff}'); // Remove UTF-8 BOM
    let table: toml::Table = match contents.parse() {
        Ok(t) => t,
        Err(_) => return "unknown".to_string(),
    };
    table
        .get("package")
        .and_then(|p| p.as_table())
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
