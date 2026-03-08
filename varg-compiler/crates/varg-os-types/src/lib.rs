// The native mapping for varg OS internal types

#[derive(Debug, Clone)]
pub struct Prompt {
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Error {
    pub message: String,
    pub code: i32,
}

#[derive(Debug, Clone)]
pub struct Tensor {
    pub data: Vec<f32>,
    pub shape: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct Embedding {
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Context {
    pub id: String,
    pub messages: Vec<Message>,
}

impl Context {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            messages: Vec::new(),
        }
    }

    pub fn push(&mut self, role: &str, content: &str) {
        self.messages.push(Message {
            role: role.to_string(),
            content: content.to_string(),
        });
        
        // MVP Dynamic Shrinking logic: Only keep the most recent 10 messages
        // in a real system this would count tokens and dynamically slide
        if self.messages.len() > 10 {
            self.messages.remove(0);
        }
    }
}

// OS Level Kernel Execution Macro
#[macro_export]
macro_rules! varg_os_query {
    ($query:expr) => {
        // In the real varg OS, this would pass the query string over an IPC or FFI boundary
        // to the isolated SurrealDB container running in ring 0.
        println!("\n[varg-OS EXECUTING KERNEL QUERY]: {}\n", $query);
    };
}
