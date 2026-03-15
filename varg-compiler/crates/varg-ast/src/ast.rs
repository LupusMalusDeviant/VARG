#[derive(Debug, PartialEq, Clone)]
pub struct Program {
    pub no_std: bool,
    pub items: Vec<Item>,
    /// Wave 13: Doc comments keyed by item name (agent/struct/fn/contract/enum)
    pub docs: std::collections::HashMap<String, String>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Item {
    Agent(AgentDef),
    Contract(ContractDef),
    Struct(StructDef),
    Enum(EnumDef),
    TypeAlias { name: String, target: TypeNode },
    Import(String),
    // Plan 26: Extended import with selective/qualified access
    ImportDecl(ImportDeclNode),
    // Plan 23: First-class prompt templates
    PromptTemplate(PromptTemplateDef),
    // Plan 25: Standalone top-level functions
    Function(FunctionDef),
    // Plan 41: External crate imports
    CrateImport { crate_name: String, version: String, features: Vec<String> },
    // Wave 13: impl blocks for structs
    Impl {
        type_name: String,
        type_params: Vec<String>,
        methods: Vec<MethodDecl>,
    },
}

// ---- Prompt Templates (Plan 23) ----
#[derive(Debug, PartialEq, Clone)]
pub struct PromptTemplateDef {
    pub name: String,
    pub params: Vec<FieldDecl>,
    pub body: String,  // Raw template text with {var} placeholders
}

// ---- Module System (Plan 26) ----
#[derive(Debug, PartialEq, Clone)]
pub struct ImportDeclNode {
    pub module_name: String,
    pub items: ImportItems,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ImportItems {
    All,                        // import math;  or  import math.*;
    Selected(Vec<String>),      // import math.{sqrt, abs};
    Single(String),             // import math.sqrt;
}

// ---- Standalone Functions (Plan 25) ----
#[derive(Debug, PartialEq, Clone)]
pub struct FunctionDef {
    pub name: String,
    pub is_public: bool,
    pub params: Vec<FieldDecl>,
    pub return_ty: Option<TypeNode>,
    pub body: Block,
}

// ---- Annotations ----
#[derive(Debug, PartialEq, Clone)]
pub struct Annotation {
    pub name: String,
    pub values: Vec<String>, // Support multiple arguments: @[CliCommand("name", "desc")] 
}

// ---- Agent, Contract, Struct ----
#[derive(Debug, PartialEq, Clone)]
pub struct AgentDef {
    pub name: String,
    pub is_system: bool,
    pub is_public: bool,
    pub target_annotation: Option<String>, // legacy @target
    pub annotations: Vec<Annotation>,
    pub implements: Vec<String>,  // Plan 29: Contract names this agent implements
    pub fields: Vec<FieldDecl>,  // Agent-State (Plan 19)
    pub methods: Vec<MethodDecl>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ContractDef {
    pub name: String,
    pub is_public: bool,
    pub target_annotation: Option<String>,
    pub methods: Vec<MethodDecl>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct StructDef {
    pub name: String,
    pub is_public: bool,
    pub type_params: Vec<String>,
    pub fields: Vec<FieldDecl>,
}

// ---- Enum Types (Plan 07) ----
#[derive(Debug, PartialEq, Clone)]
pub struct EnumDef {
    pub name: String,
    pub is_public: bool,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<(String, TypeNode)>, // Optional associated data: Suspended(string reason)
}

// ---- Generic Constraints (Plan 07) ----
#[derive(Debug, PartialEq, Clone)]
pub struct GenericConstraint {
    pub type_param: String,       // T
    pub bounds: Vec<String>,      // [Comparable, Clone] — multiple trait bounds via +
}

// ---- Methods and Fields ----
#[derive(Debug, PartialEq, Clone)]
pub struct FieldDecl {
    pub name: String,
    pub ty: TypeNode,
    pub default_value: Option<Expression>,  // Plan 40: Default parameter values
}

#[derive(Debug, PartialEq, Clone)]
pub struct MethodDecl {
    pub name: String,
    pub is_public: bool,
    pub is_async: bool,
    pub annotations: Vec<Annotation>,
    pub type_params: Vec<String>,
    pub constraints: Vec<GenericConstraint>,
    pub args: Vec<FieldDecl>,
    pub return_ty: Option<TypeNode>,
    pub body: Option<Block>, // Contracts only have declarations (None)
}

// ---- Execution Blocks, Statements & Expressions ----
#[derive(Debug, PartialEq, Clone)]
pub struct Block {
    pub statements: Vec<Statement>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    Let { name: String, ty: Option<TypeNode>, value: Expression }, // ty is Option because of 'var'
    Assign { name: String, value: Expression },
    IndexAssign { target: Expression, index: Expression, value: Expression },
    PropertyAssign { target: Expression, property: String, value: Expression },
    Expr(Expression),
    UnsafeBlock(Block),           // OCAP: Unsafe Code Marker
    Return(Option<Expression>),
    
    // Phase 9: Control Flow
    If { condition: Expression, then_block: Block, else_block: Option<Block> },
    While { condition: Expression, body: Block },
    For { init: Box<Statement>, condition: Expression, update: Box<Statement>, body: Block },
    Foreach { item_name: String, value_name: Option<String>, collection: Expression, body: Block },
    Break,
    Continue,
    Const { name: String, ty: Option<TypeNode>, value: Expression },
    Print(Expression),
    
    // Phase 12: Error Handling
    TryCatch { try_block: Block, catch_var: String, catch_block: Block },
    Throw(Expression),
    
    // Phase 20: Streaming
    Stream(Expression),

    // Plan 06: Pattern Matching
    Match { subject: Expression, arms: Vec<MatchArm> },

    // Plan 06: Destructuring
    LetDestructure { pattern: DestructurePattern, value: Expression },

    // Plan 20: Actor-Model select statement
    Select { arms: Vec<SelectArm> },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expression {
    Int(i64),
    Float(f64),  // Plan 42: Float literals
    String(String),
    Bool(bool),
    Null,
    Identifier(String),
    BinaryOp {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },
    MethodCall {
        caller: Box<Expression>,
        method_name: String,
        args: Vec<Expression>,
    },
    PropertyAccess {
        caller: Box<Expression>,
        property_name: String,
    },
    IndexAccess {
        caller: Box<Expression>,
        index: Box<Expression>,
    },
    Linq(LinqQuery), // Phase 9: LINQ
    ArrayLiteral(Vec<Expression>), // Phase 10: [1, 2, 3]
    MapLiteral(Vec<(Expression, Expression)>), // Phase 10: {"key": value}
    PromptLiteral(String), // Phase 20: prompt """ ... """
    Query(SurrealQueryNode), // Phase 15: Native Database Query Expression

    // Wave 5: await expression
    Await(Box<Expression>),

    // Unary operators: -x, !x
    UnaryOp {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },

    // Plan 06: Closures/Lambdas
    Lambda {
        params: Vec<FieldDecl>,
        return_ty: Option<Box<TypeNode>>,
        body: Box<LambdaBody>,
    },

    // Wave 6: Retry/Fallback
    Retry {
        max_attempts: Box<Expression>,
        body: Box<Block>,
        fallback: Option<Box<Block>>,
    },

    // Wave 6: Spawn agent
    Spawn {
        agent_name: String,
        args: Vec<Expression>,
    },

    // Plan 24: Error Propagation
    TryPropagate(Box<Expression>),  // expr? — auto-propagate Result error
    OrDefault {
        expr: Box<Expression>,
        default: Box<Expression>,
    },  // expr or default — unwrap with fallback

    // Plan 35: String interpolation $"Hello {name}!"
    // parts = ["Hello ", name_expr, "!"]
    InterpolatedString(Vec<InterpolationPart>),

    // Plan 38: Tuple literals (1, "hello")
    TupleLiteral(Vec<Expression>),

    // Plan 37: Range expressions 0..10, 0..=10
    Range {
        start: Box<Expression>,
        end: Box<Expression>,
        inclusive: bool,
    },

    // Wave 11: Type casting — expr as Type
    Cast {
        expr: Box<Expression>,
        target_type: TypeNode,
    },

    // Wave 11: If-expression — if cond { a } else { b }
    IfExpr {
        condition: Box<Expression>,
        then_block: Block,
        else_block: Block,
    },

    // Wave 12: Struct literal — Point { x: 5, y: 10 }
    StructLiteral {
        type_name: String,
        fields: Vec<(String, Expression)>,
    },

    // Wave 12: Enum variant construction — Shape::Circle(5) or Ok(value)
    EnumConstruct {
        enum_name: String,       // "Shape" or "" for bare variants
        variant_name: String,    // "Circle"
        args: Vec<Expression>,
    },
}

// Plan 35: Parts of an interpolated string
#[derive(Debug, PartialEq, Clone)]
pub enum InterpolationPart {
    Literal(String),
    Expression(Expression),
}

// ---- Lambda Body (Plan 06) ----
#[derive(Debug, PartialEq, Clone)]
pub enum LambdaBody {
    Expression(Expression),  // (a, b) => a + b
    Block(Block),            // (a) => { ... }
}

#[derive(Debug, PartialEq, Clone)]
pub struct LinqQuery {
    pub from_var: String,
    pub in_collection: Box<Expression>,
    pub where_clause: Option<Box<Expression>>,
    pub orderby_clause: Option<Box<Expression>>,
    pub descending: bool,
    pub select_clause: Box<Expression>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum BinaryOperator {
    Add, Sub, Mul, Div, Mod, Eq, NotEq, Lt, Gt, LtEq, GtEq, And, Or, CosineSim
}

#[derive(Debug, PartialEq, Clone)]
pub enum UnaryOperator {
    Negate, // -x
    Not,    // !x
}

// ---- OCAP Capability Tokens (Plan 03) ----
#[derive(Debug, PartialEq, Clone)]
pub enum CapabilityType {
    NetworkAccess,
    FileAccess,
    DbAccess,
    LlmAccess,
    SystemAccess,
}

// ---- Pattern Matching (Plan 06) ----
#[derive(Debug, PartialEq, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expression>,  // Wave 11: if guard — Ok(x) if x > 0 => { ... }
    pub body: Block,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Pattern {
    Literal(Expression),           // 200, "hello", true
    Variant(String, Vec<String>),  // Ok(value), Err(e)
    Wildcard,                      // _
}

// ---- Destructuring (Plan 06) ----
#[derive(Debug, PartialEq, Clone)]
pub enum DestructurePattern {
    Tuple(Vec<String>),                    // (a, b, c)
    Struct(Vec<(String, Option<String>)>), // { name, age: a }
}

// ---- Actor-Model Concurrency (Plan 20) ----
#[derive(Debug, PartialEq, Clone)]
pub struct SelectArm {
    pub var_name: String,          // msg binding variable
    pub source: SelectSource,      // from agent | timeout(ms)
    pub body: Block,
}

#[derive(Debug, PartialEq, Clone)]
pub enum SelectSource {
    Agent(Expression),             // from agent_handle expression
    Timeout(Expression),           // timeout(ms) expression
}

// ---- AI-OS Native Types ----
#[derive(Debug, PartialEq, Clone)]
pub enum TypeNode {
    Int,
    Float,  // Plan 42: f64
    String,
    Bool,
    Void,
    Ulong,
    Prompt,
    Context,
    Tensor,
    Embedding, // Embeddings represent float vectors mapped natively
    Result(Box<TypeNode>, Box<TypeNode>), // e.g., Result<string, Error>
    Error, // New
    
    // Complex / Varg-Min Types
    Array(Box<TypeNode>), // e.g., string[]
    Map(Box<TypeNode>, Box<TypeNode>), // e.g., map<string, int>
    Set(Box<TypeNode>),              // Wave 16: set<string> → HashSet<String>
    
    // Plan 07: Nullable Types
    Nullable(Box<TypeNode>),         // string? → Option<String>

    // Phase 23: Generics!
    TypeVar(String),                 // An unbound generic type like T or K
    Generic(String, Vec<TypeNode>),  // Standard User-Generics Box<int>
    List(Box<TypeNode>),             // Standard Library List<T>
    Custom(String), // References to Agent or Struct names

    // Plan 38: Tuple types (int, string)
    Tuple(Vec<TypeNode>),

    // Plan 03: OCAP Capability Tokens
    Capability(CapabilityType),

    // Plan 06: Function/Closure types
    Func(Vec<TypeNode>, Box<TypeNode>), // (params) => return

    // Plan 16: Agent messaging handle
    AgentHandle(String), // AgentHandle("Worker") — handle to spawned agent

    // Wave 15: Opaque JSON value (serde_json::Value)
    JsonValue,
}

// ---- SurrealDB AST Node ----
#[derive(Debug, PartialEq, Clone)]
pub struct SurrealQueryNode {
    pub raw_query: String, // To be expanded with proper SurrealDB AST later
}

// ---- TDD AST Verification ----
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dummy_agent_ast() {
        // Constructing an AST manually without parsing:
        // system agent VramManager {
        //     public void Allocate(ulong size) {
        //         unsafe { return; }
        //     }
        // }
        let ast = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "VramManager".to_string(),
                    is_system: true,
                    is_public: false,
                    target_annotation: None,
                    annotations: vec![],
                    implements: vec![],
                    fields: vec![],
                    methods: vec![
                        MethodDecl {
                            name: "Allocate".to_string(),
                            is_public: true, is_async: false,
                            annotations: vec![],
                            type_params: vec![],
                            constraints: vec![],
                            args: vec![
                                FieldDecl { name: "size".to_string(), ty: TypeNode::Ulong, default_value: None }
                            ],
                            return_ty: Some(TypeNode::Void),
                            body: Some(Block { statements: vec![
                                    Statement::UnsafeBlock(Block {
                                        statements: vec![Statement::Return(None)] })
                                ]
                            }),
                        }
                    ],
                })
            ],
        };

        // If the compiler builds this without lifetime/borrowing errors, our Box usage and nesting is sound!
        assert_eq!(ast.items.len(), 1);
        if let Item::Agent(ref agent) = ast.items[0] {
            assert!(agent.is_system);
            assert_eq!(agent.name, "VramManager");
        } else {
            panic!("Expected Agent");
        }
    }
}
