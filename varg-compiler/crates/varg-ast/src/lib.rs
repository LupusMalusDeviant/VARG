use logos::Logos;

pub mod ast;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\r\n\f]+")] // Skip whitespace
#[logos(skip r"//.*")]       // Skip single-line comments
#[logos(skip r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/")] // Skip multi-line comments
pub enum Token {
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

    // ---- LINQ / Queries ----
    #[token("from")] From,
    #[token("where")] Where,
    #[token("select")] Select,
    #[token("orderby")] Orderby,
    #[token("descending")] Descending,

    // ---- Type Inference / Output ----
    #[token("var")] Var,
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
    #[token("ulong")] TypeUlong,
    #[token("bool")] TypeBool,
    #[token("void")] TypeVoid,

    // ---- Hardware / Annotations ----
    #[token("@target")] TargetAnnotation,
    #[token("#![no_std]")] NoStd,

    // ---- Literals ----
    // Strings (extremely simplified for now, captures "anything" or 'anything')
    #[regex(r#""(?:[^"\\]|\\.)*""#, |lex| lex.slice().to_string())]
    StringLiteral(String),

    // Prompt Literals (Phase 20.A)
    #[regex(r#"prompt\s*"""([^"]|"[^"]|""[^"])*""""#, |lex| lex.slice().to_string())]
    PromptLiteralToken(String),

    // Numbers (Integers)
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
    #[token(":")] Colon,
    #[token(",")] Comma,
    #[token(".")] Dot,

    #[token("=")] Assign,
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

    // ---- Varg-Min (KI-Optimized Shorthands) ----
    #[token("+A")] PlusA,
    #[token("-A")] MinusA,
    #[token("+M")] PlusM,
    #[token("+V")] PlusV,
    #[token("?")] QuestionMark,
    #[token("F", priority = 3)] FButton,
    #[token("s", priority = 3)] TypeStringShort,
    #[token("i", priority = 3)] TypeIntShort,
    #[token("b", priority = 3)] TypeBoolShort,
    #[token("m", priority = 3)] TypeMapShort,

    // ---- Identifiers ----
    #[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),
}
