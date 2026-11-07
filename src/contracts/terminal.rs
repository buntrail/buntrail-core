use super::*;
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSize {
    pub columns: std::num::NonZeroU16,
    pub rows: std::num::NonZeroU16,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct TerminalStyle {
    pub font_family: String,
    pub font_size: f64,
    pub ansi_palette: [u32; 16],
    pub foreground: u32,
    pub background: u32,
    pub cursor: u32,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TerminalOpenRequest {
    pub operation: OperationID,
    pub session: TerminalSessionID,
    pub owner: WorkspaceLease,
    pub target: TargetIdentity,
    pub runtime: RuntimeGeneration,
    pub directory: TargetPath,
    pub shell: TargetPath,
    pub size: TerminalSize,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TerminalAttachRequest {
    pub session: TerminalSessionID,
    pub owner: WorkspaceLease,
    pub runtime: RuntimeGeneration,
    pub last_output_sequence: u64,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalEndIntent {
    ExplicitClose,
    FinalWorkspaceClose,
    RetentionExpiry,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum TerminalEvent {
    Output { sequence: u64, bytes: Vec<u8> },
    Gap { first: u64, last: u64 },
    ProcessExit { code: Option<i32> },
    Ended { intent: TerminalEndIntent },
    Disconnected { expires_at_unix_seconds: u64 },
    Reattached,
    Expired,
    RuntimeLost,
}
pub const RETENTION_SECONDS: u64 = 1800;
