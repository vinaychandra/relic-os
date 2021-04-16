pub mod registers;
pub mod runtime;

/// Enum that represents user return info.
#[derive(Debug)]
pub enum UserReturn {
    SystemCall,
    Interrupt,
}
