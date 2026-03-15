use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

mod formatter;

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

    // Wave 13: REPL doesn't need a file argument
    if command == "repl" {
        run_repl();
        return;
    }

    if args.len() < 3 {
        print_usage();
        exit(1);
    }

    let input_file = &args[2];

    if !input_file.ends_with(".varg") {
        eprintln!("Error: Input file must have a .varg extension.");
        exit(1);
    }

    // Wave 14: --debug flag for debug builds (faster compilation, debug symbols)
    let debug_mode = args.iter().any(|a| a == "--debug");

    match command.as_str() {
        "build" => {
            compile_varg_file(input_file, false, debug_mode);
        },
        "run" => {
            compile_varg_file(input_file, true, debug_mode);
        },
        "emit-rs" => {
            // The old behavior (just spit out the .rs file)
            let (rust_source, _) = parse_and_generate(input_file);
            let output_path = input_file.replace(".varg", ".rs");
            // Plan 44: Prepend #![allow(...)] and run rustfmt
            let allow_header = "#![allow(unused_variables, unused_mut, dead_code, unused_imports, unreachable_code, unused_assignments)]\n\n";
            let formatted = format!("{}{}", allow_header, rust_source);
            fs::write(&output_path, &formatted).unwrap();
            let _ = Command::new("rustfmt").arg(&output_path).status();
            println!("-> Wrote {}", output_path);
        }
        // Wave 13: Watch mode — recompile on .varg file changes
        "watch" => {
            watch_varg_file(input_file);
        }
        // Wave 13: Format .varg source code
        "fmt" => {
            format_varg_file(input_file);
        }
        // Wave 13: Doc generation — output markdown docs from doc comments
        "doc" => {
            generate_docs(input_file);
        }
        // Wave 15: Test runner — find @[Test] methods and run them
        "test" => {
            let coverage = args.iter().any(|a| a == "--coverage");
            test_varg_file(input_file, debug_mode, coverage);
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage();
            exit(1);
        }
    }
}

fn print_usage() {
    println!("Varg Compiler (vargc) v0.1.0");
    println!("Usage:");
    println!("  vargc build <file.varg>   - Compiles down to a native executable in the current directory");
    println!("  vargc run <file.varg>     - Compiles and immediately executes the script");
    println!("  vargc emit-rs <file.varg> - Translates to Rust source code (.rs) but does not compile");
    println!("  vargc watch <file.varg>   - Watch for changes and recompile automatically");
    println!("  vargc fmt <file.varg>     - Format Varg source code");
    println!("  vargc doc <file.varg>     - Generate markdown documentation from doc comments");
    println!("  vargc test <file.varg>    - Run @[Test] methods in the file");
    println!("  vargc test <file.varg> --coverage - Run tests with code coverage report");
    println!("  vargc repl                - Interactive REPL (Read-Eval-Print Loop)");
}

/// Wave 13: Interactive REPL — parse, typecheck, and show generated Rust for each line
fn run_repl() {
    use std::io::{self, Write, BufRead};

    println!("Varg REPL v0.1.0  (type :quit to exit, :help for commands)");
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
    compile_varg_file(input_file, false, false);

    let mut last_modified = get_latest_varg_mtime(dir);

    loop {
        std::thread::sleep(Duration::from_millis(500));
        let current = get_latest_varg_mtime(dir);
        if current > last_modified {
            println!("\n[watch] Change detected, recompiling...");
            compile_varg_file(input_file, false, false);
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

fn compile_varg_file(input_path: &str, run_immediately: bool, debug_mode: bool) {
    let varg_name = Path::new(input_path).file_stem().unwrap().to_str().unwrap();
    
    println!("-> Transpiling {}...", input_path);
    let (mut final_rust_source, ast) = parse_and_generate(input_path);

    // Plan 27: Detect if program uses async methods
    let has_async = ast.items.iter().any(|item| {
        if let varg_ast::ast::Item::Agent(a) = item {
            a.methods.iter().any(|m| m.is_async)
        } else { false }
    });

    // We statically inject the bootstrap code.
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
            // Look for `Run` or `Main`, otherwise just the first method *if it has 0 args*
            if let Some(run_m) = a.methods.iter().find(|m| m.name == "Run" || m.name == "Main") {
                if run_m.args.is_empty() { main_method_name = Some(run_m.name.clone()); }
            } else if let Some(first_m) = a.methods.first() {
                if first_m.args.is_empty() {
                    main_method_name = Some(first_m.name.clone());
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

    let cargo_toml = format!(r#"
[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
varg-os-types = {{ path = "{}" }}
varg-runtime = {{ path = "{}" }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
{}{}"#, varg_name,
    varg_os_types_path.display().to_string().replace("\\", "/"),
    varg_runtime_path.display().to_string().replace("\\", "/"),
    tokio_dep,
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
        .arg(main_rs_path.to_str().unwrap())
        .status();

    println!("-> Compiling native binary using rustc...");
    
    let cargo_cmd = if run_immediately { "run" } else { "build" };
    let mut cmd = Command::new("cargo");
    cmd.arg(cargo_cmd);
    // Wave 14: Only use --release when not in debug mode
    if !debug_mode {
        cmd.arg("--release");
    }
    cmd.current_dir(&cache_dir);

    if run_immediately {
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

    if !run_immediately {
        // Copy the executable out of the target folder into the current directory
        #[cfg(target_os = "windows")]
        let exe_name = format!("{}.exe", varg_name);
        #[cfg(not(target_os = "windows"))]
        let exe_name = varg_name.to_string();

        // Wave 14: Use correct target subdirectory based on build profile
        let profile_dir = if debug_mode { "debug" } else { "release" };
        let compiled_exe_path = cache_dir.join("target").join(profile_dir).join(&exe_name);
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

    let cargo_toml = format!(r#"
[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
varg-os-types = {{ path = "{}" }}
varg-runtime = {{ path = "{}" }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
"#, varg_name,
    varg_os_types_path.display().to_string().replace("\\", "/"),
    varg_runtime_path.display().to_string().replace("\\", "/"));

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
}
