use super::async_types::ServiceFuture;
use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightSpan {
    pub range: BufferRange,
    pub scope: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoldRange {
    pub id: String,
    pub range: BufferRange,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InjectionRegion {
    pub range: BufferRange,
    pub language: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineNode {
    pub id: String,
    pub kind: String,
    pub parent_id: Option<String>,
    pub selection: BufferRange,
    pub preview: Option<String>,
    pub label: String,
    pub range: BufferRange,
    pub children: Vec<OutlineNode>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanguageAvailability {
    Available,
    Unavailable(CoreError),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageSnapshot {
    pub document: DocumentID,
    pub revision: BufferRevision,
    pub grammar_id: String,
    pub query_version: String,
    pub highlights: Vec<HighlightSpan>,
    pub folds: Vec<FoldRange>,
    pub injections: Vec<InjectionRegion>,
    pub outline: Vec<OutlineNode>,
    pub availability: LanguageAvailability,
    pub has_errors: bool,
    pub truncated: bool,
}
pub trait LanguageService: Send + Sync {
    fn analyze(&self, snapshot: DocumentSnapshot) -> ServiceFuture<'_, LanguageSnapshot>;
}
