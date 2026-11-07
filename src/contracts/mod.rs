pub mod async_types;
pub mod document;
pub mod file;
pub mod git;
pub mod language;
pub mod operation;
pub mod terminal;
pub mod workspace;
pub use document::*;
pub use file::*;
pub use git::*;
pub use language::*;
pub use operation::*;
pub use terminal::*;
pub use workspace::*;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidArgument,
    StaleRevision,
    Composition,
    Unavailable,
    WrongThread,
    Closed,
    Busy,
    Cancelled,
    Unknown,
    TargetChanged,
    InvalidTransition,
    Io,
    Poisoned,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ErrorContext {
    None,
    Document(DocumentID),
    Target(Box<TargetIdentity>),
    Operation(OperationID),
    Request(RequestId),
    Capability(Capability),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CoreError {
    pub code: ErrorCode,
    pub message: String,
    pub context: ErrorContext,
}
impl CoreError {
    pub fn new(code: ErrorCode, message: &str) -> Self {
        Self {
            code,
            message: message.into(),
            context: ErrorContext::None,
        }
    }
}
impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for CoreError {}
