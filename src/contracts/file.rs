use super::*;
use std::sync::Arc;
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FileRef {
    pub target: TargetIdentity,
    pub path: TargetPath,
    pub resolved_identity: Option<Vec<u8>>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FileVersion {
    pub target: TargetIdentity,
    pub opaque_token: Vec<u8>,
    pub content_digest: [u8; 32],
    pub file_identity: Vec<u8>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ByteSource {
    Memory(Arc<[u8]>),
    PrivateSpool {
        local_path: TargetPath,
        length: u64,
        digest: [u8; 32],
        /// Pure lifetime token; filesystem cleanup runs in the service worker.
        #[serde(skip)]
        lease: Option<Arc<()>>,
    },
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FileSnapshot {
    pub file: FileRef,
    pub version: FileVersion,
    pub size: u64,
    pub encoding: Encoding,
    pub source: ByteSource,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum FileKind {
    File,
    Directory,
    Symlink,
    Other,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub file: FileRef,
    pub display_name: String,
    pub kind: FileKind,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum FileEvent {
    Changed(FileRef),
    Removed(FileRef),
    RescanRequired(FileRef),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SaveRequest {
    pub operation: OperationID,
    pub file: FileRef,
    pub expected_version: Option<FileVersion>,
    pub submitted_revision: BufferRevision,
    pub source: ByteSource,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SaveResult {
    pub saved_revision: BufferRevision,
    pub version: FileVersion,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum FileMutation {
    Create {
        source: ByteSource,
    },
    Rename {
        destination: TargetPath,
    },
    Move {
        destination: TargetPath,
    },
    Delete,
    /// Remove a file or empty directory; never descend into a directory.
    Remove,
    CreateDirectory,
    Trash,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ExpectedPathState {
    Absent,
    Present(FileVersion),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FileMutationRequest {
    pub operation: OperationID,
    pub file: FileRef,
    pub expected: ExpectedPathState,
    pub command: FileMutation,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum SearchMode {
    Literal,
    Regex,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SearchRequest {
    pub request: RequestId,
    pub root: FileRef,
    pub expression: String,
    pub mode: SearchMode,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub hidden: bool,
    pub ignored: bool,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SearchMatch {
    pub file: FileRef,
    pub line: u64,
    pub preview_column_utf8: u64,
    pub preview: String,
    pub searched_version: FileVersion,
    pub searched_digest: [u8; 32],
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum SearchEvent {
    Match(Box<SearchMatch>),
    Progress { files: u64, matches: u64 },
    Complete,
    Cancelled,
    Limited { maximum: u64 },
    Failed(CoreError),
}

/// Root resolution follows the root directory; child intents retain the requested alias.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileResolvePurpose {
    Root,
    OpenChild(TargetPath),
    MutationNode(TargetPath),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileResolution {
    pub root: FileRef,
    pub file: FileRef,
    pub resolved_path: TargetPath,
    /// Kind of the observed node (a leaf symlink stays a symlink).
    pub kind: Option<FileKind>,
    /// Only present when the existing strong version policy actually observed it.
    pub version: Option<FileVersion>,
}
