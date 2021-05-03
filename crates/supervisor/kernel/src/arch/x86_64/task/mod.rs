pub mod registers;

/// Enum that represents user return info.
#[derive(Debug)]
pub enum UserReturn {
    SystemCall,
    Interrupt,
}
