use super::*;
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum MutationKind {
    Save,
    File,
    Git,
    TerminalOpen,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ExpectedState {
    File {
        file: FileRef,
        version: Option<FileVersion>,
    },
    Git {
        worktree: WorktreeIdentity,
        state: GitExpectedState,
    },
    Runtime(RuntimeGeneration),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OperationEnvelope {
    pub id: OperationID,
    pub target: TargetIdentity,
    pub request_digest: [u8; 32],
    pub kind: MutationKind,
    pub expected: ExpectedState,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum MutationResult {
    Saved(SaveResult),
    FileChanged {
        file: FileRef,
        version: Option<FileVersion>,
    },
    GitChanged {
        worktree: WorktreeIdentity,
        state: GitExpectedState,
    },
    TerminalOpened {
        session: TerminalSessionID,
        runtime: RuntimeGeneration,
    },
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ObservedState {
    Unobserved,
    File {
        file: FileRef,
        version: Option<FileVersion>,
        digest: Option<[u8; 32]>,
    },
    Git {
        worktree: WorktreeIdentity,
        state: GitExpectedState,
    },
    GitNetwork {
        worktree: WorktreeIdentity,
        state: GitExpectedState,
        intent: GitNetworkIntent,
        observation: Option<GitNetworkObservation>,
    },
    Runtime(RuntimeGeneration),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ReconciliationEvidence {
    pub target: TargetIdentity,
    pub observed: ObservedState,
    pub explanation: String,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum FailureEffect {
    ProvenNoEffect,
    PossiblePartialEffects,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OperationFailure {
    pub error: CoreError,
    pub target: TargetIdentity,
    pub observed: ObservedState,
    pub effect: FailureEffect,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum OperationState {
    Accepted,
    Running,
    Confirmed(MutationResult),
    Failed(OperationFailure),
    Unknown(ReconciliationEvidence),
}
impl OperationState {
    pub fn allows_automatic_replay(&self) -> bool {
        false
    }
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OperationReceipt {
    pub id: OperationID,
    pub state: OperationState,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum OperationEvent {
    Started,
    Confirmed(MutationResult),
    Failed(OperationFailure),
    LostAcknowledgement(ReconciliationEvidence),
    Reconciled(MutationResult),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancellationOutcome {
    ConfirmedCancel,
    TooLate,
    Unknown,
}
