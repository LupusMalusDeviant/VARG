use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

use varg_parser::{Parser, ParseError};
use varg_typechecker::TypeChecker;
use varg_codegen::RustGenerator;

use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        print_usage();
        exit(1);
    }

    let command = &args[1];
    let input_file = &args[2];

    if !input_file.ends_with(".varg") {
        eprintln!("Error: Input file must have a .varg extension.");
        exit(1);
    }

    match command.as_str() {
        "build" => {
            compile_varg_file(input_file, false);
        },
        "run" => {
            compile_varg_file(input_file, true);
        },
        "emit-rs" => {
            // The old behavior (just spit out the .rs file)
            let (rust_source, _) = parse_and_generate(input_file);
            let output_path = input_file.replace(".varg", ".rs");
            fs::write(&output_path, rust_source).unwrap();
            println!("-> Wrote {}", output_path);
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

fn report_semantic_error(filename: &str, source: &str, err: &varg_typechecker::TypeError) {
    let mut files = SimpleFiles::new();
    let file_id = files.add(filename, source);
    let writer = StandardStream::stderr(ColorChoice::Auto);
    let config = term::Config::default();

    let diagnostic = Diagnostic::error()
        .with_message(err.message())
        .with_labels(vec![
            Label::primary(file_id, 0..0)
                .with_message("in this file"),
        ]);

    term::emit(&mut writer.lock(), &config, &files, &diagnostic).unwrap_or_else(|_| {
        eprintln!("Semantic Error: {:?}", err);
    });
}

fn parse_and_generate(input_path: &str) -> (String, varg_ast::ast::Program) {
    let mut loaded = std::collections::HashSet::new();
    let mut merged_ast = varg_ast::ast::Program { no_std: false, items: Vec::new() };

    parse_recursive(input_path, &mut merged_ast, &mut loaded);

    // Read the source again for error reporting
    let source_for_errors = fs::read_to_string(input_path).unwrap_or_default();

    let mut checker = TypeChecker::new();
    if let Err(err) = checker.check_program(&merged_ast) {
        report_semantic_error(input_path, &source_for_errors, &err);
        exit(1);
    }

    let generator = RustGenerator::new();
    let source = generator.generate(&merged_ast);
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
    let ast = parser.parse_program().unwrap_or_else(|err| {
        report_parse_error(path, &source, &err);
        exit(1);
    });

    for item in ast.items {
        if let varg_ast::ast::Item::Import(ref module_name) = item {
            let parent_dir = Path::new(path).parent().unwrap_or(Path::new(""));
            let mod_path = parent_dir.join(format!("{}.varg", module_name));
            if !mod_path.exists() {
                eprintln!("Error: Imported module '{}' not found at {:?}", module_name, mod_path);
                exit(1);
            }
            parse_recursive(mod_path.to_str().unwrap(), program, loaded);
        } else {
            program.items.push(item);
        }
    }
}

fn compile_varg_file(input_path: &str, run_immediately: bool) {
    let varg_name = Path::new(input_path).file_stem().unwrap().to_str().unwrap();
    
    println!("-> Transpiling {}...", input_path);
    let (mut final_rust_source, ast) = parse_and_generate(input_path);

    // We statically inject the bootstrap code.
    final_rust_source.push_str("\nfn main() {\n");
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
        final_rust_source.push_str(&format!("    let mut instance = {} {{}};\n", agent));

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
                        
                        // Push to MCP tools JSON array
                        tools_json_block.push_str(&format!(
                            "            tools.push(serde_json::json!({{ \"name\": \"{}\", \"description\": \"{}\" }}));\n",
                            cmd_name, cmd_desc
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
                            command_dispatch_block.push_str("            println!(\"{}\", res);\n");
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
    fs::write(&main_rs_path, final_rust_source).unwrap();

    println!("-> Compiling native binary using rustc...");
    
    let cargo_cmd = if run_immediately { "run" } else { "build" };
    let mut cmd = Command::new("cargo");
    cmd.arg(cargo_cmd)
       .arg("--release")
       .current_dir(&cache_dir);

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

        let compiled_exe_path = cache_dir.join("target").join("release").join(&exe_name);
        let dest_path = PathBuf::from(&exe_name);
        
        if fs::copy(&compiled_exe_path, &dest_path).is_ok() {
            println!("-> Successfully built: {}", dest_path.display());
        } else {
            eprintln!("-> Built, but failed to copy {} to current directory.", exe_name);
        }
    }
}
