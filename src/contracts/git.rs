use super::*;
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WorktreeIdentity {
    pub target: TargetIdentity,
    pub canonical_path: TargetPath,
    pub common_git_directory_identity: Vec<u8>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GitHead {
    Branch { name: Vec<u8>, oid: String },
    Detached(String),
    Unborn(Vec<u8>),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GitExpectedState {
    pub head: GitHead,
    pub index_fingerprint: [u8; 32],
    pub worktree_fingerprint: [u8; 32],
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GitEntry {
    pub path: TargetPath,
    pub previous_path: Option<TargetPath>,
    pub status: String,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GitSnapshot {
    pub worktree: WorktreeIdentity,
    pub state: GitExpectedState,
    pub staged: Vec<GitEntry>,
    pub unstaged: Vec<GitEntry>,
    pub untracked: Vec<GitEntry>,
    pub conflicts: Vec<GitEntry>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum DiffSource {
    Index,
    Worktree,
    Commit(String),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DiffRequest {
    pub worktree: WorktreeIdentity,
    pub path: TargetPath,
    pub source: DiffSource,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum DiffContent {
    Text,
    Binary,
    Unavailable(String),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum DiffChange {
    Modified,
    Added,
    Deleted,
    Renamed,
    Conflicted,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DiffSnapshot {
    pub source: DiffSource,
    pub content: DiffContent,
    pub change: DiffChange,
    pub previous_path: Option<TargetPath>,
    pub expected: GitExpectedState,
    pub bytes: Vec<u8>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HistoryQuery {
    pub worktree: WorktreeIdentity,
    pub ref_filter: Vec<Vec<u8>>,
    pub cursor: Option<Vec<u8>>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GraphCommit {
    pub lane: u32,
    pub parent_lanes: Vec<u32>,
    pub author: String,
    pub authored_at: String,
    pub message: String,
    pub oid: String,
    pub parents: Vec<String>,
    pub decorations: Vec<Vec<u8>>,
    pub subject: String,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GraphPage {
    pub commits: Vec<GraphCommit>,
    pub continuation_edges: Vec<(String, String)>,
    pub next_cursor: Option<Vec<u8>>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GitMutation {
    Stage(Vec<TargetPath>),
    Unstage(Vec<TargetPath>),
    Commit { message: String },
    Checkout { reference: Vec<u8> },
    Fetch { remote: Vec<u8> },
    Push { remote: Vec<u8>, reference: Vec<u8> },
    PullFastForward { remote: Vec<u8>, reference: Vec<u8> },
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GitMutationRequest {
    pub operation: OperationID,
    pub worktree: WorktreeIdentity,
    pub expected: GitExpectedState,
    pub command: GitMutation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitSaveApproval {
    pub operation: OperationID,
    pub request_digest: [u8; 32],
    pub actor: WorkspaceLease,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GitNetworkAction {
    Fetch,
    Push,
    PullFastForward,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GitNetworkIntent {
    pub action: GitNetworkAction,
    pub remote: Vec<u8>,
    pub reference: Vec<u8>,
    /// Digest of the single endpoint resolved before execution; never persist credential URLs.
    pub endpoint_fingerprint: Option<[u8; 32]>,
    /// Only an absent target remote.push mapping qualifies this observer's source-named ref.
    #[serde(default)]
    pub push_mapping_unconfigured: bool,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GitNetworkObservation {
    /// Present only when the selected full ref exists at the observed endpoint.
    pub remote_oid: Option<String>,
    pub remote_refs_fingerprint: [u8; 32],
    pub local_refs_fingerprint: [u8; 32],
    pub fetch_head_fingerprint: [u8; 32],
    /// Validated newline-separated OIDs only; FETCH_HEAD descriptions may contain credentials.
    pub fetch_head_oids: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorktreeChangePhase {
    Resolving,
    NeedsSave,
    Saving,
    Running,
    Refreshing,
}
#[derive(Clone, Debug)]
pub struct WorktreeChangeState {
    pub phase: WorktreeChangePhase,
    pub documents: Vec<DocumentID>,
    pub message: String,
}

/// Fixed read-only queries accepted by native consumers; no caller-supplied Git flags.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitRead {
    Upstream,
    StatusShort,
    StatusPorcelain,
    Head,
    HeadSymbolic,
    Inside,
    Refs,
    Stashes,
    Remotes,
    Diff,
    StagedDiff,
    Log,
    Graph,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GitReadOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub success: bool,
}

impl GitRead {
    pub fn from_args(args: &[&str]) -> Option<GitRead> {
        Some(match args {
            ["status", "--short", "--branch"] => GitRead::StatusShort,
            ["status", "--porcelain=v1", "-z", "--untracked-files=all"] => GitRead::StatusPorcelain,
            ["rev-parse", "--abbrev-ref", "HEAD"] => GitRead::Head,
            ["symbolic-ref", "--quiet", "--short", "HEAD"] => GitRead::HeadSymbolic,
            ["rev-parse", "--is-inside-work-tree"] => GitRead::Inside,
            [
                "for-each-ref",
                "--format=%(refname)%00%(HEAD)%00%(symref)",
                "refs/heads",
                "refs/remotes",
                "refs/tags",
            ] => GitRead::Refs,
            ["stash", "list", "--format=%gd: %s"] => GitRead::Stashes,
            ["remote", "-v"] => GitRead::Remotes,
            ["diff", "--no-ext-diff", "--"] => GitRead::Diff,
            ["diff", "--cached", "--no-ext-diff", "--"] => GitRead::StagedDiff,
            ["log", "-30", "--oneline", "--decorate"] => GitRead::Log,
            ["log", "--graph", "--oneline", "--decorate", "--all", "-100"] => GitRead::Graph,
            _ => return None,
        })
    }
}
