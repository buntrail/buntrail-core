use crate::contracts::*;
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct WorkspaceDocument {
    pub id: DocumentID,
    pub file: FileRef,
    pub version: Option<FileVersion>,
    pub viewport: Option<EditorViewport>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TerminalStart {
    pub session: TerminalSessionID,
    pub directory: TargetPath,
    pub shell: TargetPath,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum GitRecoveryKind {
    Stage,
    Unstage,
    Checkout,
    Fetch,
    Pull,
    Commit,
    Push,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GitRecoveryReference {
    pub operation: OperationID,
    pub worktree: WorktreeIdentity,
    pub kind: GitRecoveryKind,
    pub request_digest: [u8; 32],
    pub owner: WorkspaceLease,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct WorkspaceRecord {
    pub id: WorkspaceID,
    #[serde(default)]
    pub generation: u64,
    pub lease: WorkspaceLeaseID,
    pub locator: WorkspaceLocator,
    pub target: TargetIdentity,
    pub display_name: String,
    pub documents: Vec<WorkspaceDocument>,
    pub selected: Option<DocumentID>,
    pub show_hidden: bool,
    pub show_icons: bool,
    pub sidebar_visible: bool,
    pub terminal_visible: bool,
    #[serde(default)]
    pub terminal_height: Option<f64>,
    #[serde(default)]
    pub terminal_layout: crate::terminal::layout::TerminalLayoutState,
    #[serde(default)]
    pub terminal_starts: Vec<TerminalStart>,
    pub graph_filters: Vec<String>,
    #[serde(default)]
    pub git_recoveries: Vec<GitRecoveryReference>,
    #[serde(default)]
    pub git_recovery_generation: u64,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct WorkspaceCatalog {
    pub schema: u32,
    pub client: ClientID,
    pub records: Vec<WorkspaceRecord>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum WorkspaceState {
    Ready(WorkspaceRecord),
    Missing(WorkspaceRecord),
}

#[derive(Clone, Debug)]
pub struct RecoveryEntry {
    pub document: DocumentID,
    pub file: Option<FileRef>,
    pub provisional: bool,
    pub unknown_save: bool,
    pub error: Option<CoreError>,
}
