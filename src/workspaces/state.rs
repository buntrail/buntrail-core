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
    Repository,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_action: Option<RepositoryAction>,
    pub request_digest: [u8; 32],
    pub owner: WorkspaceLease,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum FileRecoveryKind {
    Create,
    CreateDirectory,
    Import,
    Move,
    Remove,
    Trash,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FileRecoveryReference {
    pub operation: OperationID,
    pub file: FileRef,
    pub destination: Option<TargetPath>,
    pub kind: FileRecoveryKind,
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
    #[serde(default)]
    pub file_recoveries: Vec<FileRecoveryReference>,
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

#[cfg(test)]
mod explorer_operation_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn explorer_operations_survive_workspace_round_trip() {
        let target = TargetIdentity {
            security_binding: vec![1],
            effective_user: vec![2],
            filesystem_namespace: vec![3],
            container: None,
        };
        let record = WorkspaceRecord {
            id: WorkspaceID([1; 16]),
            generation: 0,
            lease: WorkspaceLeaseID([2; 16]),
            locator: WorkspaceLocator {
                environment: EnvironmentLocator {
                    route: RouteKind::Local,
                    ssh_profile: None,
                    engine: None,
                    engine_context: None,
                    container_selector: None,
                    effective_user: None,
                },
                root: TargetPath::posix(b"/project".to_vec()).unwrap(),
            },
            target: target.clone(),
            display_name: "project".into(),
            documents: vec![],
            selected: None,
            show_hidden: true,
            show_icons: true,
            sidebar_visible: true,
            terminal_visible: false,
            terminal_height: None,
            terminal_layout: Default::default(),
            terminal_starts: vec![],
            graph_filters: vec![],
            git_recoveries: vec![],
            git_recovery_generation: 0,
            file_recoveries: vec![],
        };
        let mut disk = serde_json::to_value(record).unwrap();
        let retained = json!([{
            "operation": OperationID([3;16]),
            "file": FileRef { target, path: TargetPath::posix(b"/project/a".to_vec()).unwrap(), resolved_identity: None },
            "destination": null, "kind": "Trash", "request_digest": vec![4u8;32],
            "owner": WorkspaceLease { client: ClientID([5;16]), workspace: WorkspaceID([1;16]),
                lease: WorkspaceLeaseID([2;16]), runtime: RuntimeGeneration { supervisor_boot: [6;16], container_start: None } }
        }]);
        disk["file_recoveries"] = retained.clone();
        let parsed: WorkspaceRecord = serde_json::from_value(disk).unwrap();
        assert_eq!(
            serde_json::to_value(parsed).unwrap()["file_recoveries"],
            retained
        );
    }
}
