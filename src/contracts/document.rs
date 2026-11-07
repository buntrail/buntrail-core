use super::*;
use std::sync::Arc;
#[derive(
    serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct BufferRevision(pub u64);
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BufferRange {
    pub revision: BufferRevision,
    pub start: u64,
    pub end: u64,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TextByteRange {
    pub revision: BufferRevision,
    pub start: u64,
    pub end: u64,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8 { bom: bool },
    Other(String),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum LineEndings {
    Lf,
    CrLf,
    Cr,
    Mixed,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DocumentSnapshot {
    pub document: DocumentID,
    pub revision: BufferRevision,
    pub text: Arc<crate::Text>,
    pub encoding: Encoding,
    pub line_endings: LineEndings,
    pub language: Option<String>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Replacement {
    pub range: BufferRange,
    pub text: String,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum EditIntent {
    Replace,
    Undo,
    Redo,
    BeginComposition,
    UpdateComposition,
    CommitComposition,
    CancelComposition,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EditTransaction {
    pub expected_revision: BufferRevision,
    pub source_view: ViewID,
    pub replacements: Vec<Replacement>,
    pub intent: EditIntent,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditResult {
    pub revision: BufferRevision,
    pub deltas: Vec<crate::Delta>,
    pub selection_anchors: Vec<BufferRange>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct EditorViewport {
    pub selection: BufferRange,
    pub horizontal_offset: f64,
    pub vertical_offset: f64,
    pub folded_ranges: Vec<String>,
    pub revision_anchor: BufferRevision,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum DocumentSaveState {
    Clean(FileVersion),
    Dirty,
    Saving {
        operation: OperationID,
        revision: BufferRevision,
    },
    Failed(OperationFailure),
    Unknown(ReconciliationEvidence),
}

impl BufferRange {
    pub fn to_bytes(&self, snapshot: &DocumentSnapshot) -> Result<TextByteRange, CoreError> {
        if self.revision != snapshot.revision {
            return Err(CoreError::new(
                ErrorCode::StaleRevision,
                "range belongs to another snapshot",
            ));
        }
        if self.end < self.start {
            return Err(CoreError::new(ErrorCode::InvalidArgument, "reversed range"));
        }
        let offset = |value: u64| {
            usize::try_from(value)
                .ok()
                .and_then(|n| snapshot.text.utf16_to_byte(n).ok())
                .ok_or_else(|| {
                    CoreError::new(ErrorCode::InvalidArgument, "invalid UTF-16 boundary")
                })
        };
        Ok(TextByteRange {
            revision: self.revision,
            start: offset(self.start)? as u64,
            end: offset(self.end)? as u64,
        })
    }
}
impl TextByteRange {
    pub fn to_utf16(&self, snapshot: &DocumentSnapshot) -> Result<BufferRange, CoreError> {
        if self.revision != snapshot.revision {
            return Err(CoreError::new(
                ErrorCode::StaleRevision,
                "range belongs to another snapshot",
            ));
        }
        let start = usize::try_from(self.start)
            .map_err(|_| CoreError::new(ErrorCode::InvalidArgument, "byte offset overflow"))?;
        let end = usize::try_from(self.end)
            .map_err(|_| CoreError::new(ErrorCode::InvalidArgument, "byte offset overflow"))?;
        if end < start
            || snapshot.text.byte_to_utf16(start).is_err()
            || snapshot.text.byte_to_utf16(end).is_err()
        {
            return Err(CoreError::new(
                ErrorCode::InvalidArgument,
                "invalid UTF-8 boundary",
            ));
        }
        Ok(BufferRange {
            revision: self.revision,
            start: snapshot.text.byte_to_utf16(start).unwrap() as u64,
            end: snapshot.text.byte_to_utf16(end).unwrap() as u64,
        })
    }
}
