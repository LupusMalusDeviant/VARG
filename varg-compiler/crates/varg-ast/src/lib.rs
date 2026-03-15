use logos::Logos;

pub mod ast;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\r\n\f]+")] // Skip whitespace
#[logos(skip r"//([^/\n].*)?")]  // Skip single-line comments (but NOT /// doc comments)
#[logos(skip r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/")] // Skip multi-line comments
pub enum Token {
    // ---- Doc Comments (Wave 13) ----
    #[regex(r"///[^\n]*", |lex| lex.slice()[3..].trim().to_string(), priority = 10)]
    DocComment(String),

    // ---- Keywords ----
    #[token("contract")] Contract,
    #[token("agent")] Agent,
    #[token("system")] System,
    #[token("struct")] Struct,
    #[token("public")] Public,
    #[token("private")] Private,
    #[token("unsafe")] Unsafe,
    #[token("import")] Import,
    #[token("return")] Return,
    #[token("if")] If,
    #[token("else")] Else,
    #[token("query")] Query,
    
    // ---- Error Handling (Phase 12) ----
    #[token("try")] Try,
    #[token("catch")] Catch,
    #[token("throw")] Throw,
    
    // ---- Streaming (Phase 20) ----
    #[token("stream")] Stream,

    // ---- Type System Extensions (Plan 07) ----
    #[token("enum")] Enum,
    #[token("type")] Type,
    #[token("null")] Null,
    #[token("match")] Match,

    // ---- OCAP Capability Tokens (Plan 03) ----
    #[token("NetworkAccess")] NetworkAccess,
    #[token("FileAccess")] FileAccess,
    #[token("DbAccess")] DbAccess,
    #[token("LlmAccess")] LlmAccess,
    #[token("SystemAccess")] SystemAccess,

    // ---- Plan 06: Closures & Pattern Matching ----
    #[token("=>")] FatArrow,
    #[token("_", priority = 3)] Underscore,

    // ---- Control Flow / Loops ----
    #[token("while")] While,
    #[token("for")] For,
    #[token("foreach")] Foreach,
    #[token("in")] In,
    #[token("break")] Break,
    #[token("continue")] Continue,
    #[token("async")] Async,
    #[token("await")] Await,
    #[token("const")] Const,
    #[token("fn", priority = 3)] Fn,
    #[token("impl", priority = 3)] Impl,  // Wave 13: impl blocks for structs
    #[token("as", priority = 3)] As,

    // ---- LINQ / Queries ----
    #[token("from")] From,
    #[token("where")] Where,
    #[token("select")] Select,
    #[token("orderby")] Orderby,
    #[token("descending")] Descending,

    // ---- Type Inference / Output ----
    #[token("var")] Var,
    #[token("let")] Let,           // Plan 45: let as alias for var
    #[token("mut", priority = 3)] Mut,  // Plan 45: accepted but ignored (vars are mutable by default)
    #[token("pub", priority = 3)] Pub,  // Plan 45: pub as alias for public
    #[token("print")] Print,

    // ---- Annotations ----
    #[token("@")] At,  // Used for @[CliCommand] etc.

    // ---- Native AI / OS Types ----
    #[token("Prompt")] Prompt,
    #[token("Context")] Context,
    #[token("Tensor")] Tensor,
    #[token("Embedding")] Embedding,
    #[token("Result")] Result,

    // ---- Primitive Types ----
    #[token("string")] TypeString,
    #[token("int")] TypeInt,
    #[token("float")] TypeFloat,  // Plan 42: Float type
    #[token("ulong")] TypeUlong,
    #[token("bool")] TypeBool,
    #[token("void")] TypeVoid,

    // ---- Hardware / Annotations ----
    #[token("@target")] TargetAnnotation,
    #[token("#![no_std]")] NoStd,

    // ---- Literals ----
    // Wave 16: Multiline strings with triple quotes """..."""
    #[regex(r#""""([^"]|"[^"]|""[^"])*""""#, |lex| lex.slice().to_string(), priority = 5)]
    MultilineStringLiteral(String),

    // Interpolated strings: $"Hello {name}!" (Plan 35)
    #[regex(r#"\$"(?:[^"\\]|\\.)*""#, |lex| lex.slice().to_string())]
    InterpolatedStringLiteral(String),

    // Strings (extremely simplified for now, captures "anything" or 'anything')
    #[regex(r#""(?:[^"\\]|\\.)*""#, |lex| lex.slice().to_string())]
    StringLiteral(String),

    // Prompt Literals (Phase 20.A)
    #[regex(r#"prompt\s*"""([^"]|"[^"]|""[^"])*""""#, |lex| lex.slice().to_string())]
    PromptLiteralToken(String),

    // Numbers (Plan 42: Float literals must come before Int to get priority via longest-match)
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse().ok())]
    FloatLiteral(f64),

    #[regex(r"[0-9]+", |lex| lex.slice().parse().ok())]
    IntLiteral(i64),

    // Booleans
    #[token("true", |_| true)]
    #[token("false", |_| false)]
    BoolLiteral(bool),

    // ---- Symbols & Operators ----
    #[token("{")] LBrace,
    #[token("}")] RBrace,
    #[token("(")] LParen,
    #[token(")")] RParen,
    #[token("[")] LBracket,
    #[token("]")] RBracket,
    #[token(";")] Semicolon,
    #[token("::")] ColonColon,  // Wave 12: Enum path separator
    #[token(":")] Colon,
    #[token(",")] Comma,
    #[token(".")] Dot,

    #[token("=")] Assign,
    #[token("+=")] PlusAssign,
    #[token("-=")] MinusAssign,
    #[token("*=")] MulAssign,
    #[token("/=")] DivAssign,
    #[token("%=")] ModAssign,
    #[token("==")] Equals,
    #[token("!=")] NotEquals,
    #[token("<")] LessThan,
    #[token(">")] GreaterThan,
    #[token("<=")] LessOrEqual,
    #[token(">=")] GreaterOrEqual,

    #[token("+")] Plus,
    #[token("-")] Minus,
    #[token("*")] Multiply,
    #[token("/")] Divide,
    #[token("%")] Percent,
    #[token("~")] Tilde, // Phase 20.B: Vector Math Operator

    // ---- Logical Operators ----
    #[token("&&")] And,
    #[token("||")] Or,
    #[token("!")] Bang,

    #[token("->")] Arrow,
    #[token("|>")] Pipe,

    // ---- Plan 37: Range Expressions ----
    #[token("..=")] DotDotEquals,
    #[token("..")] DotDot,

    // ---- Wave 6: LLM-Native Features ----
    #[token("retry")] Retry,
    #[token("fallback")] Fallback,
    #[token("spawn")] Spawn,

    // ---- Wave 7: Actor-Model Concurrency ----
    #[token("timeout")] Timeout,

    // ---- Plan 24: Error Propagation ----
    #[token("or", priority = 3)] OrKeyword,

    // ---- Varg-Min (KI-Optimized Shorthands) ----
    // Note: Single-letter shorthands (s, i, b, m, F) removed in Plan 32
    // because they blocked normal variable names. Multi-char shorthands kept.
    #[token("+A")] PlusA,
    #[token("-A")] MinusA,
    #[token("+M")] PlusM,
    #[token("+V")] PlusV,
    #[token("?")] QuestionMark,

    // ---- Identifiers ----
    #[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),
}
