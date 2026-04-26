use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

mod formatter;

// ── Package registry ───────────────────────────────────────────────────────────

/// Fallback registry embedded in the binary. Used when the network is unavailable.
const EMBEDDED_REGISTRY: &str = r#"{
  "packages": [
    {
      "name": "varg-http-utils",
      "version": "0.1.0",
      "description": "HTTP utility functions for Varg agents",
      "url": "https://raw.githubusercontent.com/LupusMalusDeviant/VARG/main/packages/http_utils.varg",
      "checksum": ""
    },
    {
      "name": "varg-json-tools",
      "version": "0.1.0",
      "description": "JSON parsing and transformation helpers for Varg",
      "url": "https://raw.githubusercontent.com/LupusMalusDeviant/VARG/main/packages/json_tools.varg",
      "checksum": ""
    },
    {
      "name": "varg-agent-kit",
      "version": "0.1.0",
      "description": "Reusable agent patterns: retry, circuit-breaker, fan-out",
      "url": "https://raw.githubusercontent.com/LupusMalusDeviant/VARG/main/packages/agent_kit.varg",
      "checksum": ""
    }
  ]
}"#;

const REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/LupusMalusDeviant/VARG/main/registry/packages.json";

/// Returns the local package installation directory: ~/.varg/packages/
fn packages_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".varg")
        .join("packages")
}

/// Fetch the registry JSON.  Falls back to the embedded copy on any network error.
fn fetch_registry() -> serde_json::Value {
    match ureq::get(REGISTRY_URL).call() {
        Ok(response) => {
            match response.into_string() {
                Ok(body) => {
                    match serde_json::from_str(&body) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("Warning: registry JSON parse error ({}); using embedded fallback.", e);
                            serde_json::from_str(EMBEDDED_REGISTRY).expect("embedded registry is valid JSON")
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: could not read registry response ({}); using embedded fallback.", e);
                    serde_json::from_str(EMBEDDED_REGISTRY).expect("embedded registry is valid JSON")
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: could not reach registry ({}); using embedded fallback.", e);
            serde_json::from_str(EMBEDDED_REGISTRY).expect("embedded registry is valid JSON")
        }
    }
}

/// `vargc search <query>` — filter packages by name/description
fn cmd_search(query: &str) {
    println!("Searching Varg package registry for \"{}\"...\n", query);
    let registry = fetch_registry();
    let packages = registry["packages"].as_array().cloned().unwrap_or_default();
    let q = query.to_lowercase();
    let matches: Vec<_> = packages.iter().filter(|p| {
        let name = p["name"].as_str().unwrap_or("").to_lowercase();
        let desc = p["description"].as_str().unwrap_or("").to_lowercase();
        name.contains(&q) || desc.contains(&q)
    }).collect();

    if matches.is_empty() {
        println!("No packages found matching \"{}\".", query);
        println!("\nAvailable packages:");
        for p in &packages {
            println!("  {:<25} {} — {}",
                p["name"].as_str().unwrap_or(""),
                p["version"].as_str().unwrap_or(""),
                p["description"].as_str().unwrap_or(""));
        }
    } else {
        println!("{:<25} {:<10} {}", "NAME", "VERSION", "DESCRIPTION");
        println!("{}", "-".repeat(70));
        for p in matches {
            println!("  {:<23} {:<10} {}",
                p["name"].as_str().unwrap_or(""),
                p["version"].as_str().unwrap_or(""),
                p["description"].as_str().unwrap_or(""));
        }
    }
}

/// `vargc install <package>` — download a package to ~/.varg/packages/
fn cmd_install(package_name: &str) {
    println!("Fetching registry...");
    let registry = fetch_registry();
    let packages = registry["packages"].as_array().cloned().unwrap_or_default();

    let pkg = packages.iter().find(|p| {
        p["name"].as_str().unwrap_or("") == package_name
    });

    let pkg = match pkg {
        Some(p) => p.clone(),
        None => {
            eprintln!("Error: package \"{}\" not found in registry.", package_name);
            eprintln!("\nAvailable packages:");
            for p in &packages {
                eprintln!("  {} — {}", p["name"].as_str().unwrap_or(""), p["description"].as_str().unwrap_or(""));
            }
            exit(1);
        }
    };

    let name    = pkg["name"].as_str().unwrap_or(package_name);
    let version = pkg["version"].as_str().unwrap_or("0.1.0");
    let url     = pkg["url"].as_str().unwrap_or("");

    if url.is_empty() {
        eprintln!("Error: package \"{}\" has no download URL.", name);
        exit(1);
    }

    // Resolve destination: ~/.varg/packages/<name>/<version>/<name>.varg
    // (underscores replace hyphens in the filename to be Varg-import-friendly)
    let dest_dir = packages_dir().join(name).join(version);
    let file_name = format!("{}.varg", name.replace('-', "_"));
    let dest_file = dest_dir.join(&file_name);

    fs::create_dir_all(&dest_dir).unwrap_or_else(|e| {
        eprintln!("Error creating package directory {:?}: {}", dest_dir, e);
        exit(1);
    });

    println!("Downloading {} {}...", name, version);
    match ureq::get(url).call() {
        Ok(response) => {
            match response.into_string() {
                Ok(content) => {
                    fs::write(&dest_file, &content).unwrap_or_else(|e| {
                        eprintln!("Error writing package file: {}", e);
                        exit(1);
                    });
                    println!("Installed {} {} -> {}", name, version, dest_file.display());
                }
                Err(e) => {
                    eprintln!("Error downloading package source: {}", e);
                    exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Error downloading {}: {}", url, e);
            exit(1);
        }
    }
}

/// `vargc list` — list all locally installed packages
fn cmd_list() {
    let base = packages_dir();
    if !base.exists() {
        println!("No packages installed. Use `vargc install <package>` to install one.");
        return;
    }

    println!("{:<25} {}", "PACKAGE", "VERSION");
    println!("{}", "-".repeat(40));

    let mut found = false;
    if let Ok(pkg_entries) = fs::read_dir(&base) {
        for pkg_entry in pkg_entries.flatten() {
            let pkg_name = pkg_entry.file_name().to_string_lossy().to_string();
            if let Ok(ver_entries) = fs::read_dir(pkg_entry.path()) {
                for ver_entry in ver_entries.flatten() {
                    let version = ver_entry.file_name().to_string_lossy().to_string();
                    println!("  {:<23} {}", pkg_name, version);
                    found = true;
                }
            }
        }
    }

    if !found {
        println!("No packages installed.");
    }
}

// Maps varg_runtime module prefixes → Cargo feature names.
// vargc scans the generated Rust source for these patterns and enables only
// the features that are actually needed, keeping binaries small.
const FEATURE_MAP: &[(&str, &str)] = &[
    ("varg_runtime::net::",        "net"),
    ("varg_runtime::sse_client::", "net"),
    ("varg_runtime::server::",     "server"),
    ("varg_runtime::db_sqlite::",  "db"),
    ("varg_runtime::checkpoint::", "db"),
    ("varg_runtime::llm::",        "llm"),
    ("varg_runtime::multimodal::", "llm"),
    ("varg_runtime::websocket::",  "ws"),
    ("varg_runtime::pdf::",        "pdf"),
    ("varg_runtime::readline::",   "readline"),
    ("varg_runtime::crypto::",     "crypto"),
    ("varg_runtime::encoding::",   "encoding"),
];

fn detect_runtime_features(rust_src: &str) -> String {
    let mut features: Vec<&str> = vec![];
    for (pattern, feature) in FEATURE_MAP {
        if rust_src.contains(pattern) && !features.contains(feature) {
            features.push(feature);
        }
    }
    if features.is_empty() {
        String::new()
    } else {
        format!(", features = {:?}", features)
    }
}

use varg_parser::{Parser, ParseError};
use varg_typechecker::TypeChecker;
use varg_codegen::RustGenerator;

use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        exit(1);
    }

    let command = &args[1];

    // ── Commands that don't need a file argument ───────────────────────────────

    // Wave 13: REPL doesn't need a file argument
    if command == "repl" {
        run_repl();
        return;
    }

    // Package manager: list installed packages
    if command == "list" {
        cmd_list();
        return;
    }

    // Package manager: install or search need a second argument
    if command == "install" || command == "search" {
        if args.len() < 3 {
            eprintln!("Usage: vargc {} <name>", command);
            exit(1);
        }
        let arg = &args[2];
        match command.as_str() {
            "install" => cmd_install(arg),
            "search"  => cmd_search(arg),
            _ => unreachable!(),
        }
        return;
    }

    // ── Commands that do need a .varg file ────────────────────────────────────

    // Parse --target <triple> before the file name (for build / run).
    // Scan all args after the command for --target and pick up the next token.
    let wasm_target: Option<String> = {
        let mut t = None;
        let mut iter = args[2..].iter();
        while let Some(a) = iter.next() {
            if a == "--target" {
                if let Some(triple) = iter.next() {
                    t = Some(triple.clone());
                }
            }
        }
        t
    };

    // Find the .varg input file: the first argument that ends with ".varg".
    let input_file_str: String = args[2..].iter()
        .find(|a| a.ends_with(".varg"))
        .cloned()
        .unwrap_or_else(|| {
            // No .varg file supplied — commands that need one print usage and exit.
            if ["build", "run", "emit-rs", "watch", "fmt", "doc", "test"].contains(&command.as_str()) {
                eprintln!("Error: no .varg file specified.");
                print_usage();
                exit(1);
            }
            String::new()
        });

    // Wave 14: --debug flag for debug builds (faster compilation, debug symbols)
    let debug_mode = args.iter().any(|a| a == "--debug");

    match command.as_str() {
        "build" => {
            compile_varg_file(&input_file_str, false, debug_mode, wasm_target.as_deref());
        },
        "run" => {
            compile_varg_file(&input_file_str, true, debug_mode, wasm_target.as_deref());
        },
        "emit-rs" => {
            // The old behavior (just spit out the .rs file)
            let (rust_source, _) = parse_and_generate(&input_file_str);
            let output_path = input_file_str.replace(".varg", ".rs");
            // Plan 44: Prepend #![allow(...)] and run rustfmt
            let allow_header = "#![allow(unused_variables, unused_mut, dead_code, unused_imports, unreachable_code, unused_assignments)]\n\n";
            let formatted = format!("{}{}", allow_header, rust_source);
            fs::write(&output_path, &formatted).unwrap();
            let _ = Command::new("rustfmt").args(["--edition", "2021"]).arg(&output_path).status();
            println!("-> Wrote {}", output_path);
        }
        // Wave 13: Watch mode — recompile on .varg file changes
        "watch" => {
            watch_varg_file(&input_file_str);
        }
        // Wave 13: Format .varg source code
        "fmt" => {
            format_varg_file(&input_file_str);
        }
        // Wave 13: Doc generation — output markdown docs from doc comments
        "doc" => {
            generate_docs(&input_file_str);
        }
        // Wave 15: Test runner — find @[Test] methods and run them
        "test" => {
            let coverage = args.iter().any(|a| a == "--coverage");
            test_varg_file(&input_file_str, debug_mode, coverage);
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage();
            exit(1);
        }
    }
}

fn print_usage() {
    println!("Varg Compiler (vargc) v0.12.0");
    println!("Usage:");
    println!("  vargc build [--target <triple>] <file.varg>   Build to a native (or WASM) executable");
    println!("  vargc run   [--target <triple>] <file.varg>   Build and immediately execute");
    println!("  vargc emit-rs <file.varg>                     Emit generated Rust source only");
    println!("  vargc watch <file.varg>                       Watch for changes and recompile");
    println!("  vargc fmt <file.varg>                         Format Varg source code");
    println!("  vargc doc <file.varg>                         Generate markdown docs");
    println!("  vargc test [--coverage] <file.varg>           Run @[Test] methods");
    println!("  vargc repl                                    Interactive REPL");
    println!();
    println!("Package manager:");
    println!("  vargc install <package>   Install a package from the registry");
    println!("  vargc search  <query>     Search the package registry");
    println!("  vargc list                List all locally installed packages");
    println!();
    println!("WASM example:");
    println!("  vargc build --target wasm32-wasip1 hello.varg");
    println!("  wasmtime hello.wasm");
}

/// Wave 13: Interactive REPL — parse, typecheck, and show generated Rust for each line
fn run_repl() {
    use std::io::{self, Write, BufRead};

    println!("Varg REPL v0.12.0  (type :quit to exit, :help for commands)");
    println!();

    let stdin = io::stdin();
    let mut history: Vec<String> = Vec::new();
    let mut accumulated = String::new();

    loop {
        let prompt = if accumulated.is_empty() { "varg> " } else { "  ... " };
        print!("{}", prompt);
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap() == 0 {
            break; // EOF
        }
        let trimmed = line.trim();

        // REPL commands
        if trimmed == ":quit" || trimmed == ":q" {
            println!("Goodbye!");
            break;
        }
        if trimmed == ":help" || trimmed == ":h" {
            println!("Commands:");
            println!("  :quit / :q    — Exit the REPL");
            println!("  :clear / :c   — Clear accumulated input");
            println!("  :history      — Show input history");
            println!("  :rs           — Show generated Rust for last input");
            println!("  :ast          — Show parsed AST for last input");
            println!();
            println!("Input: Enter Varg statements/declarations. Multi-line input");
            println!("       continues until braces are balanced.");
            continue;
        }
        if trimmed == ":clear" || trimmed == ":c" {
            accumulated.clear();
            println!("Cleared.");
            continue;
        }
        if trimmed == ":history" {
            for (i, h) in history.iter().enumerate() {
                println!("[{}] {}", i + 1, h.replace('\n', "\n    "));
            }
            continue;
        }
        if trimmed == ":rs" {
            if let Some(last) = history.last() {
                let wrapped = wrap_repl_input(last);
                match try_compile_repl(&wrapped) {
                    Ok(rust_code) => println!("{}", rust_code),
                    Err(e) => eprintln!("Error: {}", e),
                }
            } else {
                println!("No previous input.");
            }
            continue;
        }
        if trimmed == ":ast" {
            if let Some(last) = history.last() {
                let wrapped = wrap_repl_input(last);
                let mut parser = varg_parser::Parser::new(&wrapped);
                match parser.parse_program() {
                    Ok(program) => println!("{:#?}", program.items),
                    Err(e) => eprintln!("Parse error: {:?}", e),
                }
            } else {
                println!("No previous input.");
            }
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        accumulated.push_str(&line);

        // Check if braces are balanced
        let open = accumulated.matches('{').count();
        let close = accumulated.matches('}').count();
        if open > close {
            continue; // Need more input
        }

        let input = accumulated.trim().to_string();
        accumulated.clear();
        history.push(input.clone());

        // Try to compile as a standalone item or as a statement inside an agent
        let wrapped = wrap_repl_input(&input);
        match try_compile_repl(&wrapped) {
            Ok(rust_code) => {
                println!("=> {}", rust_code.lines()
                    .filter(|l| !l.trim().starts_with("//") && !l.trim().is_empty()
                        && !l.contains("AUTOGENERATED") && !l.contains("use varg_"))
                    .collect::<Vec<_>>()
                    .join("\n   "));
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
}

/// Wrap REPL input into a valid Varg program for compilation
fn wrap_repl_input(input: &str) -> String {
    // If input looks like a top-level item (agent, struct, fn, contract, enum, impl),
    // compile it directly. Otherwise, wrap it in an agent's Run method.
    let first_word = input.split_whitespace().next().unwrap_or("");
    match first_word {
        "agent" | "struct" | "contract" | "enum" | "fn" | "pub" | "public" |
        "import" | "crate" | "type" | "impl" | "prompt" | "system" => {
            input.to_string()
        }
        _ => {
            // Wrap as statements inside an agent
            format!("agent __Repl {{ public void Run() {{ {} }} }}", input)
        }
    }
}

/// Try to parse, typecheck, and generate Rust for REPL input
fn try_compile_repl(source: &str) -> Result<String, String> {
    let mut parser = varg_parser::Parser::new(source);
    let program = parser.parse_program().map_err(|e| format!("Parse error: {:?}", e))?;

    let mut checker = TypeChecker::new();
    if let Err(errors) = checker.check_program(&program) {
        let msgs: Vec<String> = errors.iter().map(|e| e.error.message()).collect();
        return Err(msgs.join("\n"));
    }

    let mut gen = RustGenerator::new();
    let code = gen.generate(&program);
    Ok(code)
}

/// Wave 13: Format a .varg source file
fn format_varg_file(input_file: &str) {
    let source = fs::read_to_string(input_file).unwrap_or_else(|err| {
        eprintln!("Error reading {}: {}", input_file, err);
        exit(1);
    });

    let mut parser = Parser::new(&source);
    let program = parser.parse_program().unwrap_or_else(|err| {
        eprintln!("Parse error: {:?}", err);
        exit(1);
    });

    let mut fmt = formatter::VargFormatter::new();
    let formatted = fmt.format_program(&program);

    // Check if --check flag was passed
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--check") {
        if formatted.trim() != source.trim() {
            eprintln!("{} is not formatted", input_file);
            exit(1);
        }
        println!("{} is formatted", input_file);
    } else {
        fs::write(input_file, &formatted).unwrap();
        println!("Formatted {}", input_file);
    }
}

/// Wave 13: Watch mode — poll for .varg file changes and recompile
fn watch_varg_file(input_file: &str) {
    use std::time::Duration;

    let path = Path::new(input_file);
    let dir = path.parent().unwrap_or(Path::new("."));
    println!("[watch] Watching {} for changes...", dir.display());
    println!("[watch] Press Ctrl+C to stop.");

    // Initial compile
    compile_varg_file(input_file, false, false, None);

    let mut last_modified = get_latest_varg_mtime(dir);

    loop {
        std::thread::sleep(Duration::from_millis(500));
        let current = get_latest_varg_mtime(dir);
        if current > last_modified {
            println!("\n[watch] Change detected, recompiling...");
            compile_varg_file(input_file, false, false, None);
            last_modified = current;
        }
    }
}

/// Get the latest modification time of any .varg file in a directory
fn get_latest_varg_mtime(dir: &Path) -> std::time::SystemTime {
    use std::time::SystemTime;

    let mut latest = SystemTime::UNIX_EPOCH;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "varg") {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if modified > latest {
                            latest = modified;
                        }
                    }
                }
            }
        }
    }
    latest
}

/// Wave 13: Generate markdown documentation from doc comments
fn generate_docs(input_file: &str) {
    let (_, program) = parse_and_generate(input_file);

    let filename = Path::new(input_file).file_name().unwrap_or_default().to_string_lossy();
    println!("# Module: {}\n", filename);

    for item in &program.items {
        let (kind, name) = match item {
            varg_ast::ast::Item::Agent(a) => ("Agent", a.name.as_str()),
            varg_ast::ast::Item::Contract(c) => ("Contract", c.name.as_str()),
            varg_ast::ast::Item::Struct(s) => ("Struct", s.name.as_str()),
            varg_ast::ast::Item::Enum(e) => ("Enum", e.name.as_str()),
            varg_ast::ast::Item::Function(f) => ("Function", f.name.as_str()),
            _ => continue,
        };

        println!("## {} `{}`\n", kind, name);
        if let Some(doc) = program.docs.get(name) {
            println!("> {}\n", doc.replace('\n', "\n> "));
        }

        // Print methods for agents and contracts
        let methods = match item {
            varg_ast::ast::Item::Agent(a) => Some(&a.methods),
            varg_ast::ast::Item::Contract(c) => Some(&c.methods),
            _ => None,
        };
        if let Some(methods) = methods {
            if !methods.is_empty() {
                println!("### Methods\n");
                for m in methods {
                    let vis = if m.is_public { "public " } else { "" };
                    let async_kw = if m.is_async { "async " } else { "" };
                    let args: Vec<String> = m.args.iter().map(|a| format!("{} {}", format_type(&a.ty), a.name)).collect();
                    let ret = m.return_ty.as_ref().map(|t| format!(" -> {}", format_type(t))).unwrap_or_default();
                    println!("- `{}{}{}{}{}`", vis, async_kw, m.name, if args.is_empty() { "()".to_string() } else { format!("({})", args.join(", ")) }, ret);
                }
                println!();
            }
        }

        // Print fields for structs
        if let varg_ast::ast::Item::Struct(s) = item {
            if !s.fields.is_empty() {
                println!("### Fields\n");
                for f in &s.fields {
                    println!("- `{}: {}`", f.name, format_type(&f.ty));
                }
                println!();
            }
        }

        // Print variants for enums
        if let varg_ast::ast::Item::Enum(e) = item {
            println!("### Variants\n");
            for v in &e.variants {
                if v.fields.is_empty() {
                    println!("- `{}`", v.name);
                } else {
                    let fields: Vec<String> = v.fields.iter().map(|(n, t)| format!("{}: {}", n, format_type(t))).collect();
                    println!("- `{}({})`", v.name, fields.join(", "));
                }
            }
            println!();
        }
    }
}

/// Format a TypeNode for documentation output
fn format_type(ty: &varg_ast::ast::TypeNode) -> String {
    use varg_ast::ast::TypeNode;
    match ty {
        TypeNode::Int => "int".to_string(),
        TypeNode::Float => "float".to_string(),
        TypeNode::String => "string".to_string(),
        TypeNode::Bool => "bool".to_string(),
        TypeNode::Void => "void".to_string(),
        TypeNode::Ulong => "ulong".to_string(),
        TypeNode::Array(inner) => format!("{}[]", format_type(inner)),
        TypeNode::Map(k, v) => format!("map<{}, {}>", format_type(k), format_type(v)),
        TypeNode::Nullable(inner) => format!("{}?", format_type(inner)),
        TypeNode::Custom(name) => name.clone(),
        TypeNode::Result(ok, err) => format!("Result<{}, {}>", format_type(ok), format_type(err)),
        TypeNode::Tuple(types) => format!("({})", types.iter().map(|t| format_type(t)).collect::<Vec<_>>().join(", ")),
        _ => format!("{:?}", ty),
    }
}

/// Maps Varg types to JSON Schema representations for MCP discovery
fn varg_type_to_json_schema(ty: &varg_ast::ast::TypeNode) -> serde_json::Value {
    use varg_ast::ast::TypeNode;
    match ty {
        TypeNode::String => serde_json::json!({"type": "string"}),
        TypeNode::Int => serde_json::json!({"type": "integer"}),
        TypeNode::Bool => serde_json::json!({"type": "boolean"}),
        TypeNode::Ulong => serde_json::json!({"type": "integer"}),
        TypeNode::Void => serde_json::json!({"type": "null"}),
        TypeNode::Array(inner) => serde_json::json!({
            "type": "array",
            "items": varg_type_to_json_schema(inner)
        }),
        TypeNode::List(inner) => serde_json::json!({
            "type": "array",
            "items": varg_type_to_json_schema(inner)
        }),
        TypeNode::Map(_, v) => serde_json::json!({
            "type": "object",
            "additionalProperties": varg_type_to_json_schema(v)
        }),
        TypeNode::Nullable(inner) => {
            let mut schema = varg_type_to_json_schema(inner);
            if let Some(obj) = schema.as_object_mut() {
                obj.insert("nullable".to_string(), serde_json::json!(true));
            }
            schema
        },
        TypeNode::Result(ok, _) => varg_type_to_json_schema(ok),
        _ => serde_json::json!({"type": "string"}), // Fallback for Custom, Prompt, etc.
    }
}

/// Builds a JSON Schema for a struct definition (for MCP outputSchema)
fn struct_to_json_schema(struct_def: &varg_ast::ast::StructDef) -> serde_json::Value {
    let mut props = serde_json::Map::new();
    for field in &struct_def.fields {
        props.insert(field.name.clone(), varg_type_to_json_schema(&field.ty));
    }
    serde_json::json!({
        "type": "object",
        "properties": props
    })
}

/// Finds a struct definition by name in the AST
fn find_struct_def<'a>(ast: &'a varg_ast::ast::Program, name: &str) -> Option<&'a varg_ast::ast::StructDef> {
    for item in &ast.items {
        if let varg_ast::ast::Item::Struct(s) = item {
            if s.name == name {
                return Some(s);
            }
        }
    }
    None
}

fn report_parse_error(filename: &str, source: &str, err: &ParseError) {
    let mut files = SimpleFiles::new();
    let file_id = files.add(filename, source);
    let writer = StandardStream::stderr(ColorChoice::Auto);
    let config = term::Config::default();

    let diagnostic = match err {
        ParseError::UnexpectedToken { expected, found, span } => {
            let found_str = match found {
                Some(t) => format!("{:?}", t),
                None => "nothing".to_string(),
            };
            Diagnostic::error()
                .with_message(format!("unexpected token: expected {}", expected))
                .with_labels(vec![
                    Label::primary(file_id, span.clone())
                        .with_message(format!("found `{}` here", found_str)),
                ])
                .with_notes(vec![format!("expected: {}", expected)])
        }
        ParseError::UnexpectedEof => {
            Diagnostic::error()
                .with_message("unexpected end of file")
                .with_labels(vec![
                    Label::primary(file_id, source.len()..source.len())
                        .with_message("file ends here"),
                ])
        }
    };

    term::emit(&mut writer.lock(), &config, &files, &diagnostic).unwrap_or_else(|_| {
        eprintln!("Syntax Error in {}: {:?}", filename, err);
    });
}

fn report_semantic_error(filename: &str, source: &str, err: &varg_typechecker::SpannedTypeError) {
    let mut files = SimpleFiles::new();
    let file_id = files.add(filename, source);
    let writer = StandardStream::stderr(ColorChoice::Auto);
    let config = term::Config::default();

    let span = err.span.clone().unwrap_or(0..0);
    let label_msg = if err.span.is_some() { "here" } else { "in this file" };

    let diagnostic = Diagnostic::error()
        .with_message(err.message())
        .with_labels(vec![
            Label::primary(file_id, span)
                .with_message(label_msg),
        ]);

    term::emit(&mut writer.lock(), &config, &files, &diagnostic).unwrap_or_else(|_| {
        eprintln!("Semantic Error: {:?}", err.error);
    });
}

fn parse_and_generate(input_path: &str) -> (String, varg_ast::ast::Program) {
    let mut loaded = std::collections::HashSet::new();
    let mut merged_ast = varg_ast::ast::Program { no_std: false, items: Vec::new(), docs: std::collections::HashMap::new() };

    parse_recursive(input_path, &mut merged_ast, &mut loaded);

    // Read the source again for error reporting
    let source_for_errors = fs::read_to_string(input_path).unwrap_or_default();

    let mut checker = TypeChecker::new();
    checker.set_source(&source_for_errors);
    if let Err(errors) = checker.check_program(&merged_ast) {
        for err in &errors {
            report_semantic_error(input_path, &source_for_errors, err);
        }
        exit(1);
    }

    let mut generator = RustGenerator::new();
    // Plan 46: Generate with source map comments for error mapping
    let source = generator.generate_with_source_map(&merged_ast, &source_for_errors);
    (source, merged_ast)
}

fn parse_recursive(path: &str, program: &mut varg_ast::ast::Program, loaded: &mut std::collections::HashSet<String>) {
    let abs_path = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    let path_str = abs_path.to_string_lossy().into_owned();
    if loaded.contains(&path_str) { return; }
    loaded.insert(path_str);

    let source = fs::read_to_string(path).unwrap_or_else(|err| {
        eprintln!("Error reading {}: {}", path, err);
        exit(1);
    });

    let mut parser = Parser::new(&source);
    let parsed = parser.parse_program().unwrap_or_else(|err| {
        report_parse_error(path, &source, &err);
        exit(1);
    });

    // Wave 13: Merge doc comments from this module
    for (name, doc) in parsed.docs {
        program.docs.insert(name, doc);
    }

    for item in parsed.items {
        match &item {
            varg_ast::ast::Item::Import(ref module_name) => {
                let parent_dir = Path::new(path).parent().unwrap_or(Path::new(""));
                let mod_path = parent_dir.join(format!("{}.varg", module_name));
                if !mod_path.exists() {
                    eprintln!("Error: Imported module '{}' not found at {:?}", module_name, mod_path);
                    exit(1);
                }
                parse_recursive(mod_path.to_str().unwrap(), program, loaded);
            }
            varg_ast::ast::Item::ImportDecl(ref decl) => {
                let parent_dir = Path::new(path).parent().unwrap_or(Path::new(""));
                // Try module_name.varg, then module_name/mod.varg, then subdir/module_name.varg
                let mod_path = parent_dir.join(format!("{}.varg", decl.module_name));
                let mod_dir_path = parent_dir.join(&decl.module_name).join("mod.varg");
                let actual_path = if mod_path.exists() {
                    mod_path
                } else if mod_dir_path.exists() {
                    mod_dir_path
                } else {
                    // Check nested path: a.b.c → a/b/c.varg
                    let nested = decl.module_name.replace('.', "/");
                    let nested_path = parent_dir.join(format!("{}.varg", nested));
                    if nested_path.exists() {
                        nested_path
                    } else {
                        eprintln!("Error: Imported module '{}' not found. Searched:", decl.module_name);
                        eprintln!("  - {:?}", mod_path);
                        eprintln!("  - {:?}", mod_dir_path);
                        if nested != decl.module_name {
                            eprintln!("  - {:?}", nested_path);
                        }
                        exit(1);
                    }
                };
                parse_recursive(actual_path.to_str().unwrap(), program, loaded);
            }
            _ => {
                program.items.push(item);
            }
        }
    }

}

fn compile_varg_file(input_path: &str, run_immediately: bool, debug_mode: bool, wasm_target: Option<&str>) {
    let varg_name = Path::new(input_path).file_stem().unwrap().to_str().unwrap();

    let is_wasm = wasm_target.map(|t| t.starts_with("wasm")).unwrap_or(false);
    
    println!("-> Transpiling {}...", input_path);
    let (mut final_rust_source, ast) = parse_and_generate(input_path);

    // Plan 27: Detect if program uses async methods
    let has_async = ast.items.iter().any(|item| {
        if let varg_ast::ast::Item::Agent(a) = item {
            a.methods.iter().any(|m| m.is_async)
        } else { false }
    });

    // Wave 19: Check if program already has a standalone fn main()
    let has_standalone_main = ast.items.iter().any(|item| {
        if let varg_ast::ast::Item::Function(f) = item {
            f.name == "main"
        } else { false }
    });

    // We statically inject the bootstrap code (skip if standalone main exists).
    if !has_standalone_main {
    if has_async {
        final_rust_source.push_str("\n#[tokio::main]\nasync fn main() {\n");
    } else {
        final_rust_source.push_str("\nfn main() {\n");
    }
    final_rust_source.push_str("    let _varg_args: Vec<String> = std::env::args().collect();\n");
    
    // Find the first agent and a suitable default method
    let mut main_agent_name = None;
    let mut main_method_name = None;
    for item in &ast.items {
        if let varg_ast::ast::Item::Agent(a) = item {
            main_agent_name = Some(a.name.clone());
            // @[CliCommand] on the agent → run_cli() dispatches all public methods
            if a.annotations.iter().any(|ann| ann.name == "CliCommand") {
                main_method_name = Some("run_cli".to_string());
            } else {
                // Look for `Run` or `Main`, otherwise just the first method *if it has 0 args*
                if let Some(run_m) = a.methods.iter().find(|m| m.name == "Run" || m.name == "Main") {
                    if run_m.args.is_empty() { main_method_name = Some(run_m.name.clone()); }
                } else if let Some(first_m) = a.methods.first() {
                    if first_m.args.is_empty() {
                        main_method_name = Some(first_m.name.clone());
                    }
                }
            }
            break;
        }
    }
    
    if let Some(agent) = main_agent_name {
        // Plan 19: Use new() if agent has fields, otherwise empty struct
        let main_agent_has_fields = ast.items.iter().any(|item| {
            if let varg_ast::ast::Item::Agent(a) = item {
                a.name == agent && !a.fields.is_empty()
            } else { false }
        });
        if main_agent_has_fields {
            final_rust_source.push_str(&format!("    let mut instance = {}::new();\n", agent));
        } else {
            final_rust_source.push_str(&format!("    let mut instance = {} {{}};\n", agent));
        }

        // -- CLI AND MCP DISCOVERY DISPATCHER --
        let mut cli_dispatch = String::from("    if _varg_args.len() > 1 {\n");
        cli_dispatch.push_str("        let cmd = &_varg_args[1];\n");
        
        // 1) First pass: Collect MCP tools for discovery
        let mut tools_json_block = String::from("        if cmd == \"--mcp-discover\" {\n            let mut tools = Vec::new();\n");
        let mut command_dispatch_block = String::new();
        let mut has_cli_or_mcp = false;
        
        for item in &ast.items {
            if let varg_ast::ast::Item::Agent(a) = item {
                for method_decl in &a.methods {
                    let mut is_mcp = false;
                    let mut cmd_name = "".to_string();
                    let mut cmd_desc = "".to_string();

                    for ann in &method_decl.annotations {
                        if ann.name == "McpTool" { 
                            is_mcp = true; 
                            cmd_name = method_decl.name.clone();
                            cmd_desc = ann.values.join(" ");
                        }
                        if ann.name == "CliCommand" { 
                            is_mcp = true; 
                            if !ann.values.is_empty() { cmd_name = ann.values[0].clone(); }
                            if ann.values.len() > 1 { cmd_desc = ann.values[1].clone(); }
                        }
                    }
                    
                    if is_mcp {
                        has_cli_or_mcp = true;

                        // Build inputSchema from method args
                        let mut input_props = Vec::new();
                        let mut input_required = Vec::new();
                        for arg in &method_decl.args {
                            let schema = varg_type_to_json_schema(&arg.ty);
                            input_props.push(format!("\"{}\":{}", arg.name, schema));
                            input_required.push(format!("\"{}\"", arg.name));
                        }

                        // Build outputSchema from return type
                        let output_schema = if let Some(ref ret_ty) = method_decl.return_ty {
                            if *ret_ty != varg_ast::ast::TypeNode::Void {
                                // Check if return type is a struct (Custom type)
                                if let varg_ast::ast::TypeNode::Custom(ref struct_name) = ret_ty {
                                    if let Some(struct_def) = find_struct_def(&ast, struct_name) {
                                        let schema = struct_to_json_schema(struct_def);
                                        Some(format!(",\"outputSchema\":{}", schema))
                                    } else {
                                        let schema = varg_type_to_json_schema(ret_ty);
                                        Some(format!(",\"outputSchema\":{}", schema))
                                    }
                                } else {
                                    let schema = varg_type_to_json_schema(ret_ty);
                                    Some(format!(",\"outputSchema\":{}", schema))
                                }
                            } else { None }
                        } else { None };

                        let output_part = output_schema.unwrap_or_default();

                        // Push to MCP tools JSON array with full schema
                        tools_json_block.push_str(&format!(
                            "            tools.push(serde_json::json!({{\"name\":\"{}\",\"description\":\"{}\",\"inputSchema\":{{\"type\":\"object\",\"properties\":{{{}}},\"required\":[{}]}}{} }}));\n",
                            cmd_name, cmd_desc,
                            input_props.join(","),
                            input_required.join(","),
                            output_part
                        ));
                        
                        // Generate Route
                        command_dispatch_block.push_str(&format!("        }} else if cmd == \"{}\" {{\n", cmd_name));
                        
                        let mut arg_vars = Vec::new();
                        for (i, arg) in method_decl.args.iter().enumerate() {
                            let arg_idx = i + 2;
                            command_dispatch_block.push_str(&format!("            if _varg_args.len() <= {} {{ eprintln!(\"Missing argument '{}'\"); std::process::exit(1); }}\n", arg_idx, arg.name));
                            match arg.ty {
                                varg_ast::ast::TypeNode::Int => {
                                    command_dispatch_block.push_str(&format!("            let arg_{} = _varg_args[{}].parse::<i64>().unwrap_or(0);\n", i, arg_idx));
                                },
                                varg_ast::ast::TypeNode::Bool => {
                                    command_dispatch_block.push_str(&format!("            let arg_{} = _varg_args[{}].parse::<bool>().unwrap_or(false);\n", i, arg_idx));
                                },
                                varg_ast::ast::TypeNode::Ulong => {
                                    command_dispatch_block.push_str(&format!("            let arg_{} = _varg_args[{}].parse::<u64>().unwrap_or(0);\n", i, arg_idx));
                                },
                                _ => {
                                    command_dispatch_block.push_str(&format!("            let arg_{} = _varg_args[{}].clone();\n", i, arg_idx));
                                }
                            }
                            arg_vars.push(format!("arg_{}", i));
                        }
                        
                        command_dispatch_block.push_str(&format!("            let res = instance.{}({});\n", method_decl.name, arg_vars.join(", ")));
                        if method_decl.return_ty.is_some() && method_decl.return_ty != Some(varg_ast::ast::TypeNode::Void) {
                            // Check if return type is a struct → serialize as JSON
                            let is_struct_return = if let Some(varg_ast::ast::TypeNode::Custom(ref name)) = method_decl.return_ty {
                                find_struct_def(&ast, name).is_some()
                            } else { false };
                            if is_struct_return {
                                command_dispatch_block.push_str("            println!(\"{}\", serde_json::to_string(&res).unwrap());\n");
                            } else {
                                command_dispatch_block.push_str("            println!(\"{}\", res);\n");
                            }
                        }
                        command_dispatch_block.push_str("            std::process::exit(0);\n");
                    }
                }
            }
        }
        
        tools_json_block.push_str("            let json_out = serde_json::json!({ \"tools\": tools });\n            println!(\"{}\", json_out);\n            std::process::exit(0);\n");
        command_dispatch_block.push_str("        } else {\n            eprintln!(\"Unknown command '{}'\", cmd);\n            std::process::exit(1);\n        }\n    }\n");

        if has_cli_or_mcp {
            final_rust_source.push_str(&cli_dispatch);
            final_rust_source.push_str(&tools_json_block);
            final_rust_source.push_str(&command_dispatch_block);
        }

        final_rust_source.push_str("    println!(\"[VargOS] Bootstrapping Runtime...\");\n");
        if let Some(m) = main_method_name {
            final_rust_source.push_str(&format!("    instance.{}();\n", m));
        } else {
            final_rust_source.push_str("    println!(\"[VargOS] No parameterless 'Run' or 'Main' method found. Exiting.\");\n");
        }
    }
    
    // Scan for API Endpoints (Phase 14)
    let mut tcp_routing = String::from("    let listener = std::net::TcpListener::bind(\"0.0.0.0:8080\").unwrap();\n");
    tcp_routing.push_str("    println!(\"[VargOS] Agent listening natively on http://0.0.0.0:8080\");\n");
    tcp_routing.push_str("    for stream in listener.incoming() {\n");
    tcp_routing.push_str("        if let Ok(mut stream) = stream {\n");
    tcp_routing.push_str("            std::thread::spawn(move || {\n");
    tcp_routing.push_str("                use std::io::{Read, Write};\n");
    tcp_routing.push_str("                let mut buffer = [0; 4096];\n");
    tcp_routing.push_str("                let _ = stream.read(&mut buffer);\n");
    tcp_routing.push_str("                let request = String::from_utf8_lossy(&buffer);\n");
    tcp_routing.push_str("                let parts: Vec<&str> = request.split(\"\\r\\n\\r\\n\").collect();\n");
    tcp_routing.push_str("                let body = if parts.len() > 1 { parts[1].trim_matches(char::from(0)).to_string() } else { \"\".to_string() };\n");
    tcp_routing.push_str("                let first_line = request.lines().next().unwrap_or(\"\");\n");
    
    let mut tcp_handlers = String::new();
    let mut has_api_endpoints = false;

    for item in &ast.items {
        if let varg_ast::ast::Item::Agent(a) = item {
            for method in &a.methods {
                for ann in &method.annotations {
                    if ann.name == "ApiEndpoint" {
                        if !ann.values.is_empty() {
                            has_api_endpoints = true;
                            let val = ann.values.join(" ");
                            let parts: Vec<&str> = val.split_whitespace().collect();
                            if parts.len() >= 2 {
                                let verb = parts[0].to_uppercase();
                                let path = parts[1];
                                let handler_name = format!("{}_{}", a.name, method.name);
                                
                                tcp_routing.push_str(&format!("                if first_line.starts_with(\"{} {}\") {{\n", verb, path));
                                tcp_routing.push_str(&format!("                    let res = {}(body);\n", handler_name));
                                tcp_routing.push_str("                    let http_res = format!(\"HTTP/1.1 200 OK\\r\\nContent-Type: application/json\\r\\n\\r\\n{}\", res);\n");
                                tcp_routing.push_str("                    let _ = stream.write(http_res.as_bytes());\n");
                                tcp_routing.push_str("                    let _ = stream.flush();\n");
                                tcp_routing.push_str("                    return;\n");
                                tcp_routing.push_str("                }\n");
                                
                                tcp_handlers.push_str(&format!("
fn {handler_name}(body: String) -> String {{
    let mut instance = {} {{}};
    instance.{}(body)
}}
", a.name, method.name));
                            }
                        }
                    }
                }
            }
        }
    }

    if has_api_endpoints {
        tcp_routing.push_str("                let nF = \"HTTP/1.1 404 Not Found\\r\\n\\r\\n\"; let _ = stream.write(nF.as_bytes());\n");
        tcp_routing.push_str("            });\n");
        tcp_routing.push_str("        }\n");
        tcp_routing.push_str("    }\n");
        final_rust_source.push_str(&tcp_routing);
    }

    final_rust_source.push_str("}\n");

    if has_api_endpoints {
        final_rust_source.push_str(&tcp_handlers);
    }
    } // end if !has_standalone_main


    let cache_dir = PathBuf::from(".vargc_cache");
    if !cache_dir.exists() {
        fs::create_dir(&cache_dir).unwrap();
    }

    // Determine absolute paths to varg crates so the generated cargo project can find them
    let current_dir = env::current_dir().unwrap();
    let varg_os_types_path = current_dir.join("crates").join("varg-os-types");
    let varg_runtime_path = current_dir.join("crates").join("varg-runtime");

    // Plan 27: Add tokio dependency if program uses async
    let tokio_dep = if has_async {
        "tokio = { version = \"1\", features = [\"full\"] }\n"
    } else { "" };

    // Plan 41: Collect external crate imports and generate dependency lines
    let mut extra_deps = String::new();
    for item in &ast.items {
        if let varg_ast::ast::Item::CrateImport { crate_name, version, features } = item {
            if features.is_empty() {
                extra_deps.push_str(&format!("{} = \"{}\"\n", crate_name, version));
            } else {
                extra_deps.push_str(&format!("{} = {{ version = \"{}\", features = {:?} }}\n", crate_name, version, features));
            }
        }
    }

    // Detect which varg-runtime features are actually used and emit a minimal dep.
    // For WASM targets we force the "wasm-safe" feature and drop all heavy features.
    let runtime_features = if is_wasm {
        ", features = [\"wasm-safe\"], default-features = false".to_string()
    } else {
        detect_runtime_features(&final_rust_source)
    };

    // For WASM we also skip tokio (it doesn't compile to wasm32).
    let tokio_dep_str = if is_wasm { "" } else { tokio_dep };

    // For WASM targets inject the wasm32 crate type so cargo outputs a .wasm file.
    let lib_section = if is_wasm {
        "\n[lib]\ncrate-type = [\"cdylib\"]\n"
    } else {
        ""
    };

    let cargo_toml = format!(r#"
[package]
name = "{}"
version = "0.1.0"
edition = "2021"
{}
[dependencies]
varg-os-types = {{ path = "{}" }}
varg-runtime  = {{ path = "{}"{} }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
{}{}"#, varg_name,
        lib_section,
        varg_os_types_path.display().to_string().replace("\\", "/"),
        varg_runtime_path.display().to_string().replace("\\", "/"),
        runtime_features,
        tokio_dep_str,
        extra_deps);

    let cargo_toml_path = cache_dir.join("Cargo.toml");
    fs::write(&cargo_toml_path, cargo_toml).unwrap();

    let src_dir = cache_dir.join("src");
    if !src_dir.exists() {
        fs::create_dir(&src_dir).unwrap();
    }

    let main_rs_path = src_dir.join("main.rs");
    // Plan 44: Prepend #![allow(...)] to suppress common Rust warnings
    let allow_header = "#![allow(unused_variables, unused_mut, dead_code, unused_imports, unreachable_code, unused_assignments)]\n\n";
    let formatted_source = format!("{}{}", allow_header, final_rust_source);
    fs::write(&main_rs_path, &formatted_source).unwrap();

    // Plan 44: Run rustfmt on generated code for clean output
    let _ = Command::new("rustfmt")
        .args(["--edition", "2021"])
        .arg(main_rs_path.to_str().unwrap())
        .status();

    // Resolve effective target triple: CLI flag wins, then env var fallback.
    let effective_triple: Option<String> = wasm_target.map(|t| t.to_string())
        .or_else(|| std::env::var("VARGC_TARGET_TRIPLE").ok());

    if is_wasm {
        println!("-> Compiling to WebAssembly (target: {})...", effective_triple.as_deref().unwrap_or("wasm32-wasip1"));
    } else {
        println!("-> Compiling native binary using rustc...");
    }

    // WASM: always use `cargo build` (can't `cargo run` a .wasm directly).
    let cargo_cmd = if is_wasm || !run_immediately { "build" } else { "run" };
    let mut cmd = Command::new("cargo");
    cmd.arg(cargo_cmd);
    // Wave 14: Only use --release when not in debug mode
    if !debug_mode {
        cmd.arg("--release");
    }
    if let Some(ref triple) = effective_triple {
        cmd.arg("--target").arg(triple);
    }
    cmd.current_dir(&cache_dir);

    if !is_wasm && run_immediately {
        cmd.arg("-q"); // Quiet cargo output when running tools
    }

    let status = cmd.status().unwrap_or_else(|err| {
        eprintln!("Failed to execute cargo: {}", err);
        exit(1);
    });

    if !status.success() {
        eprintln!("Compilation failed.");
        exit(1);
    }

    if is_wasm {
        // Copy the .wasm file to the current directory.
        let profile_dir = if debug_mode { "debug" } else { "release" };
        let target_base = std::env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| cache_dir.join("target"));
        let triple = effective_triple.as_deref().unwrap_or("wasm32-wasip1");
        let wasm_src = target_base.join(triple).join(profile_dir).join(format!("{}.wasm", varg_name));
        let wasm_dest = PathBuf::from(format!("{}.wasm", varg_name));

        if fs::copy(&wasm_src, &wasm_dest).is_ok() {
            println!("-> Built {}", wasm_dest.display());
            println!("   Run with: wasmtime {}", wasm_dest.display());
        } else {
            eprintln!("-> Built, but could not locate .wasm at {:?}", wasm_src);
            eprintln!("   Check .vargc_cache/target/{}/{}/ manually.", triple, profile_dir);
        }

        // If the user also asked to run it immediately, invoke wasmtime.
        if run_immediately {
            let run_status = Command::new("wasmtime")
                .arg(wasm_dest.to_str().unwrap_or(""))
                .status();
            match run_status {
                Ok(s) if !s.success() => exit(s.code().unwrap_or(1)),
                Err(e) => {
                    eprintln!("Warning: could not invoke wasmtime: {}", e);
                    eprintln!("Run manually: wasmtime {}", wasm_dest.display());
                }
                _ => {}
            }
        }
    } else if !run_immediately {
        // Determine exe suffix from target triple (or host OS if not cross-compiling)
        let is_windows_target = effective_triple.as_deref()
            .map(|t| t.contains("windows"))
            .unwrap_or(cfg!(target_os = "windows"));
        let exe_name = if is_windows_target {
            format!("{}.exe", varg_name)
        } else {
            varg_name.to_string()
        };

        // Wave 14: Use correct target subdirectory based on build profile
        let profile_dir = if debug_mode { "debug" } else { "release" };
        // Respect CARGO_TARGET_DIR if set (e.g. by the web playground for shared dep caching)
        let target_base = std::env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| cache_dir.join("target"));
        // Cross-compilation places binary under target/<triple>/profile/
        let compiled_exe_path = match &effective_triple {
            Some(triple) => target_base.join(triple).join(profile_dir).join(&exe_name),
            None         => target_base.join(profile_dir).join(&exe_name),
        };
        let dest_path = PathBuf::from(&exe_name);

        if fs::copy(&compiled_exe_path, &dest_path).is_ok() {
            println!("-> Successfully built: {}", dest_path.display());
        } else {
            eprintln!("-> Built, but failed to copy {} to current directory.", exe_name);
        }
    }
}

/// Wave 15: Test runner — finds @[Test] methods in agents and generates a test harness
fn test_varg_file(input_path: &str, debug_mode: bool, coverage: bool) {
    let varg_name = Path::new(input_path).file_stem().unwrap().to_str().unwrap();

    println!("-> Transpiling {} for testing...", input_path);
    let (mut final_rust_source, ast) = parse_and_generate(input_path);

    // Collect all @[Test] methods and lifecycle hooks from agents
    let mut test_methods: Vec<(String, String, bool)> = Vec::new(); // (agent_name, method_name, has_fields)
    let mut before_each: std::collections::HashMap<String, String> = std::collections::HashMap::new(); // agent_name → method_name
    let mut after_each: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for item in &ast.items {
        if let varg_ast::ast::Item::Agent(a) = item {
            let has_fields = !a.fields.is_empty();
            for method in &a.methods {
                if method.annotations.iter().any(|ann| ann.name == "Test") {
                    test_methods.push((a.name.clone(), method.name.clone(), has_fields));
                }
                if method.annotations.iter().any(|ann| ann.name == "BeforeEach") {
                    before_each.insert(a.name.clone(), method.name.clone());
                }
                if method.annotations.iter().any(|ann| ann.name == "AfterEach") {
                    after_each.insert(a.name.clone(), method.name.clone());
                }
            }
        }
    }

    if test_methods.is_empty() {
        println!("No @[Test] methods found in {}.", input_path);
        return;
    }

    println!("-> Found {} test(s).", test_methods.len());

    // Generate test runner main()
    final_rust_source.push_str("\nfn main() {\n");
    final_rust_source.push_str("    let mut passed: i64 = 0;\n");
    final_rust_source.push_str("    let mut failed: i64 = 0;\n\n");

    let mut current_agent = String::new();
    for (agent_name, method_name, has_fields) in &test_methods {
        if *agent_name != current_agent {
            if !current_agent.is_empty() {
                final_rust_source.push_str("    }\n\n");
            }
            final_rust_source.push_str(&format!("    // Agent: {}\n    {{\n", agent_name));
            if *has_fields {
                final_rust_source.push_str(&format!("        let mut instance = {}::new();\n", agent_name));
            } else {
                final_rust_source.push_str(&format!("        let mut instance = {} {{}};\n", agent_name));
            }
            current_agent = agent_name.clone();
        }

        // F41-7: BeforeEach/AfterEach lifecycle hooks
        let before_call = if let Some(setup) = before_each.get(agent_name) {
            format!("            instance.{}();\n", setup)
        } else {
            String::new()
        };
        let after_call = if let Some(teardown) = after_each.get(agent_name) {
            format!("            instance.{}();\n", teardown)
        } else {
            String::new()
        };
        final_rust_source.push_str(&format!(
            r#"
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {{
{}            instance.{}();
{}        }}));
        if result.is_ok() {{
            println!("  test {}::{} ... \x1b[32mok\x1b[0m");
            passed += 1;
        }} else {{
            println!("  test {}::{} ... \x1b[31mFAILED\x1b[0m");
            failed += 1;
        }}
"#,
            before_call, method_name, after_call, agent_name, method_name, agent_name, method_name
        ));
    }
    if !current_agent.is_empty() {
        final_rust_source.push_str("    }\n\n");
    }

    final_rust_source.push_str(r#"    println!("\ntest result: {} passed, {} failed", passed, failed);
    if failed > 0 { std::process::exit(1); }
}
"#);

    // Use the same cache/build infrastructure as compile_varg_file
    let cache_dir = PathBuf::from(".vargc_cache");
    if !cache_dir.exists() {
        fs::create_dir(&cache_dir).unwrap();
    }

    let current_dir = env::current_dir().unwrap();
    let varg_os_types_path = current_dir.join("crates").join("varg-os-types");
    let varg_runtime_path = current_dir.join("crates").join("varg-runtime");

    let runtime_features = detect_runtime_features(&final_rust_source);
    let cargo_toml = format!(r#"
[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
varg-os-types = {{ path = "{}" }}
varg-runtime  = {{ path = "{}"{} }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
"#, varg_name,
    varg_os_types_path.display().to_string().replace("\\", "/"),
    varg_runtime_path.display().to_string().replace("\\", "/"),
    runtime_features);

    let cargo_toml_path = cache_dir.join("Cargo.toml");
    fs::write(&cargo_toml_path, cargo_toml).unwrap();

    let src_dir = cache_dir.join("src");
    if !src_dir.exists() {
        fs::create_dir(&src_dir).unwrap();
    }

    let main_rs_path = src_dir.join("main.rs");
    let allow_header = "#![allow(unused_variables, unused_mut, dead_code, unused_imports, unreachable_code, unused_assignments)]\n\n";
    let formatted_source = format!("{}{}", allow_header, final_rust_source);
    fs::write(&main_rs_path, &formatted_source).unwrap();

    let _ = Command::new("rustfmt")
        .args(["--edition", "2021"])
        .arg(main_rs_path.to_str().unwrap())
        .status();

    if coverage {
        println!("-> Running tests with coverage...\n");

        // Check if cargo-llvm-cov is available
        let check = Command::new("cargo").arg("llvm-cov").arg("--version")
            .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        if check.is_err() || !check.unwrap().success() {
            eprintln!("Error: cargo-llvm-cov not found.");
            eprintln!("Install with: cargo install cargo-llvm-cov");
            eprintln!("Also requires: rustup component add llvm-tools-preview");
            exit(1);
        }

        let mut cmd = Command::new("cargo");
        cmd.arg("llvm-cov").arg("run").arg("--text");
        if !debug_mode {
            cmd.arg("--release");
        }
        cmd.current_dir(&cache_dir);

        let status = cmd.status().unwrap_or_else(|err| {
            eprintln!("Failed to execute cargo llvm-cov: {}", err);
            exit(1);
        });

        if !status.success() {
            exit(1);
        }
    } else {
        println!("-> Running tests...\n");
        let mut cmd = Command::new("cargo");
        cmd.arg("run");
        cmd.arg("-q");
        if !debug_mode {
            cmd.arg("--release");
        }
        cmd.current_dir(&cache_dir);

        let status = cmd.status().unwrap_or_else(|err| {
            eprintln!("Failed to execute cargo: {}", err);
            exit(1);
        });

        if !status.success() {
            exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use varg_ast::ast::*;

    #[test]
    fn test_mcp_schema_string_args() {
        let ty = TypeNode::String;
        let schema = varg_type_to_json_schema(&ty);
        assert_eq!(schema, serde_json::json!({"type": "string"}));
    }

    #[test]
    fn test_mcp_schema_int_type() {
        let ty = TypeNode::Int;
        let schema = varg_type_to_json_schema(&ty);
        assert_eq!(schema, serde_json::json!({"type": "integer"}));
    }

    #[test]
    fn test_mcp_schema_bool_type() {
        let ty = TypeNode::Bool;
        let schema = varg_type_to_json_schema(&ty);
        assert_eq!(schema, serde_json::json!({"type": "boolean"}));
    }

    #[test]
    fn test_mcp_schema_array_type() {
        let ty = TypeNode::Array(Box::new(TypeNode::String));
        let schema = varg_type_to_json_schema(&ty);
        assert_eq!(schema, serde_json::json!({"type": "array", "items": {"type": "string"}}));
    }

    #[test]
    fn test_mcp_schema_map_type() {
        let ty = TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::Int));
        let schema = varg_type_to_json_schema(&ty);
        assert_eq!(schema, serde_json::json!({"type": "object", "additionalProperties": {"type": "integer"}}));
    }

    #[test]
    fn test_mcp_schema_nullable_type() {
        let ty = TypeNode::Nullable(Box::new(TypeNode::String));
        let schema = varg_type_to_json_schema(&ty);
        assert_eq!(schema, serde_json::json!({"type": "string", "nullable": true}));
    }

    #[test]
    fn test_mcp_schema_struct_output() {
        let struct_def = StructDef {
            name: "SearchResult".to_string(),
            is_public: true,
            type_params: vec![],
            fields: vec![
                FieldDecl { name: "title".to_string(), ty: TypeNode::String, default_value: None },
                FieldDecl { name: "url".to_string(), ty: TypeNode::String, default_value: None },
                FieldDecl { name: "relevance".to_string(), ty: TypeNode::Int, default_value: None },
            ],
        };
        let schema = struct_to_json_schema(&struct_def);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["title"]["type"], "string");
        assert_eq!(schema["properties"]["url"]["type"], "string");
        assert_eq!(schema["properties"]["relevance"]["type"], "integer");
    }

    #[test]
    fn test_find_struct_in_ast() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Struct(StructDef {
                    name: "MyStruct".to_string(),
                    is_public: false,
                    type_params: vec![],
                    fields: vec![
                        FieldDecl { name: "value".to_string(), ty: TypeNode::Int, default_value: None },
                    ],
                }),
            ],
        };
        assert!(find_struct_def(&program, "MyStruct").is_some());
        assert!(find_struct_def(&program, "NonExistent").is_none());
    }

    // ── Package manager tests ──────────────────────────────────────────────────

    #[test]
    fn test_packages_dir_returns_valid_path() {
        let dir = packages_dir();
        // Must not be empty and must end with .varg/packages
        assert!(!dir.as_os_str().is_empty());
        assert!(dir.ends_with(std::path::Path::new(".varg/packages")),
            "packages_dir() should end with .varg/packages, got: {:?}", dir);
    }

    #[test]
    fn test_registry_parse_embedded() {
        let registry: serde_json::Value = serde_json::from_str(EMBEDDED_REGISTRY)
            .expect("EMBEDDED_REGISTRY must be valid JSON");
        let packages = registry["packages"].as_array()
            .expect("registry must have a 'packages' array");
        assert!(!packages.is_empty(), "embedded registry must contain at least one package");
        // Every entry must have name, version, description, url fields.
        for pkg in packages {
            assert!(pkg["name"].as_str().is_some(), "package missing 'name'");
            assert!(pkg["version"].as_str().is_some(), "package missing 'version'");
            assert!(pkg["description"].as_str().is_some(), "package missing 'description'");
            assert!(pkg["url"].as_str().is_some(), "package missing 'url'");
        }
    }
}
