//! Revisioned documents and shared editor contracts.
//!
//! See the repository README for usage, prerequisites and platform limitations.
pub mod git;
pub mod preferences;

pub mod terminal;

pub mod contracts;
pub mod documents;
pub mod operations;
pub mod search;
pub mod text;
pub mod workspaces;
use std::sync::Arc;
pub use text::Text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Stale,
    InvalidRange,
    Composition,
    EmptyHistory,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub revision: u64,
    pub text: Arc<Text>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delta {
    pub revision: u64,
    pub start_utf16: usize,
    pub removed_utf16: usize,
    pub text: String,
}
#[derive(Clone)]
struct Edit {
    start: usize,
    before: String,
    after: String,
}
#[derive(Clone)]
struct EditGroup {
    edits: Vec<Edit>,
    before: u64,
    after: u64,
}
#[derive(Clone)]
struct Composition {
    owner: u64,
    group: EditGroup,
}

#[derive(Clone)]
pub struct Document {
    snapshot: Snapshot,
    checkpoint: u64,
    undo: Vec<EditGroup>,
    redo: Vec<EditGroup>,
    composition: Option<Composition>,
    copied: usize,
    typing: Option<(u64, usize)>,
}

pub fn byte_offset(text: &str, utf16: usize) -> Result<usize, Error> {
    let mut offset = 0;
    for (byte, ch) in text.char_indices() {
        if offset == utf16 {
            return Ok(byte);
        }
        offset += ch.len_utf16();
    }
    if offset == utf16 {
        Ok(text.len())
    } else {
        Err(Error::InvalidRange)
    }
}

impl Document {
    pub fn new(text: String) -> Self {
        Self::from_text(Arc::new(text.into()))
    }
    pub fn restore_text(text: Arc<Text>, revision: u64) -> Self {
        let mut d = Self::from_text(text);
        d.snapshot.revision = revision;
        d.checkpoint = revision;
        d
    }
    pub fn from_text(text: Arc<Text>) -> Self {
        Self {
            snapshot: Snapshot { revision: 0, text },
            checkpoint: 0,
            undo: vec![],
            redo: vec![],
            composition: None,
            copied: 0,
            typing: None,
        }
    }
    pub fn begin_typing(&mut self, view: u64) {
        self.typing = Some((view, self.undo.len()));
    }
    pub fn end_typing(&mut self) {
        self.typing = None;
    }
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }
    pub fn has_composition(&self) -> bool {
        self.composition.is_some()
    }
    pub fn text(&self) -> &str {
        &self.snapshot.text
    }
    pub fn revision(&self) -> u64 {
        self.snapshot.revision
    }
    pub fn checkpoint(&self) -> u64 {
        self.checkpoint
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.clone()
    }
    pub fn confirmed_snapshot(&self) -> Result<Snapshot, Error> {
        if self.composition.is_some() {
            Err(Error::Composition)
        } else {
            Ok(self.snapshot())
        }
    }
    pub fn copied_delta_bytes(&self) -> usize {
        self.copied
    }
    fn validate(&self, revision: u64, view: u64) -> Result<(), Error> {
        if revision != self.revision() {
            return Err(Error::Stale);
        }
        if self.composition.as_ref().is_some_and(|c| c.owner != view) {
            return Err(Error::Composition);
        }
        Ok(())
    }
    fn replace(&mut self, start: usize, length: usize, text: &str) -> Result<Delta, Error> {
        let a = self.snapshot.text.utf16_to_char(start)?;
        let b = self
            .snapshot
            .text
            .utf16_to_char(start.checked_add(length).ok_or(Error::InvalidRange)?)?;
        let next = self
            .snapshot
            .revision
            .checked_add(1)
            .ok_or(Error::InvalidRange)?;
        Arc::make_mut(&mut self.snapshot.text).replace(a, b, text);
        self.snapshot.revision = next;
        // New content gets a fresh token; history navigation restores its existing token.
        self.checkpoint = next;
        self.copied += text.len();
        Ok(Delta {
            revision: self.revision(),
            start_utf16: start,
            removed_utf16: length,
            text: text.into(),
        })
    }
    pub fn edit(
        &mut self,
        revision: u64,
        view: u64,
        start: usize,
        length: usize,
        text: &str,
    ) -> Result<Delta, Error> {
        self.validate(revision, view)?;
        let a = self.snapshot.text.utf16_to_char(start)?;
        let b = self
            .snapshot
            .text
            .utf16_to_char(start.checked_add(length).ok_or(Error::InvalidRange)?)?;
        if self.typing.is_some_and(|(owner, _)| owner != view) {
            self.end_typing();
        }
        let edit = Edit {
            start,
            before: self.snapshot.text.slice(a, b),
            after: text.into(),
        };
        let before = self.checkpoint;
        let delta = self.replace(start, length, text)?;
        if let Some(c) = &mut self.composition {
            c.group.edits.push(edit);
            c.group.after = self.checkpoint;
        } else {
            if self
                .typing
                .is_some_and(|(owner, index)| owner == view && self.undo.len() == index + 1)
            {
                let group = self.undo.last_mut().unwrap();
                group.edits.push(edit);
                group.after = self.checkpoint;
            } else {
                self.undo.push(EditGroup {
                    edits: vec![edit],
                    before,
                    after: self.checkpoint,
                });
            }
            self.redo.clear();
        }
        Ok(delta)
    }
    pub fn begin_composition(&mut self, revision: u64, view: u64) -> Result<(), Error> {
        self.validate(revision, view)?;
        if self.composition.is_some() {
            return Err(Error::Composition);
        }
        self.end_typing();
        self.composition = Some(Composition {
            owner: view,
            group: EditGroup {
                edits: vec![],
                before: self.checkpoint,
                after: self.checkpoint,
            },
        });
        Ok(())
    }
    pub fn commit_composition(&mut self, revision: u64, view: u64) -> Result<(), Error> {
        self.validate(revision, view)?;
        let c = self.composition.take().ok_or(Error::Composition)?;
        if !c.group.edits.is_empty() {
            self.undo.push(c.group);
            self.redo.clear();
        }
        Ok(())
    }
    pub fn cancel_composition(&mut self, revision: u64, view: u64) -> Result<Vec<Delta>, Error> {
        self.validate(revision, view)?;
        let c = self.composition.take().ok_or(Error::Composition)?;
        let deltas = self.reverse(&c.group.edits)?;
        self.checkpoint = c.group.before;
        Ok(deltas)
    }
    fn reverse(&mut self, edits: &[Edit]) -> Result<Vec<Delta>, Error> {
        edits
            .iter()
            .rev()
            .map(|e| self.replace(e.start, e.after.encode_utf16().count(), &e.before))
            .collect()
    }
    pub fn undo(&mut self, revision: u64, view: u64) -> Result<Vec<Delta>, Error> {
        self.validate(revision, view)?;
        if self.composition.is_some() {
            return Err(Error::Composition);
        }
        self.end_typing();
        let group = self.undo.pop().ok_or(Error::EmptyHistory)?;
        let deltas = self.reverse(&group.edits)?;
        self.checkpoint = group.before;
        self.redo.push(group);
        Ok(deltas)
    }
    pub fn redo(&mut self, revision: u64, view: u64) -> Result<Vec<Delta>, Error> {
        self.validate(revision, view)?;
        if self.composition.is_some() {
            return Err(Error::Composition);
        }
        self.end_typing();
        let group = self.redo.pop().ok_or(Error::EmptyHistory)?;
        let deltas = group
            .edits
            .iter()
            .map(|e| self.replace(e.start, e.before.encode_utf16().count(), &e.after))
            .collect::<Result<_, _>>()?;
        self.checkpoint = group.after;
        self.undo.push(group);
        Ok(deltas)
    }
}

/// Reproducible byte-integrity fingerprint, not a cryptographic authenticity claim.
pub fn digest(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

impl Document {
    pub fn apply_transaction(
        &mut self,
        transaction: contracts::EditTransaction,
    ) -> Result<contracts::EditResult, Error> {
        use contracts::*;
        let revision = transaction.expected_revision.0;
        let view = transaction.source_view.0;
        self.validate(revision, view)?;
        for r in &transaction.replacements {
            if r.range.revision.0 != revision || r.range.end < r.range.start {
                return Err(Error::InvalidRange);
            }
        }
        let undoing = transaction.intent == EditIntent::Undo;
        let deltas = match transaction.intent {
            EditIntent::Undo
            | EditIntent::Redo
            | EditIntent::BeginComposition
            | EditIntent::CommitComposition
            | EditIntent::CancelComposition => {
                if !transaction.replacements.is_empty() {
                    return Err(Error::InvalidRange);
                }
                match transaction.intent {
                    EditIntent::Undo => self.undo(revision, view)?,
                    EditIntent::Redo => self.redo(revision, view)?,
                    EditIntent::BeginComposition => {
                        self.begin_composition(revision, view)?;
                        vec![]
                    }
                    EditIntent::CommitComposition => {
                        self.commit_composition(revision, view)?;
                        vec![]
                    }
                    _ => self.cancel_composition(revision, view)?,
                }
            }
            EditIntent::Replace | EditIntent::UpdateComposition => {
                if transaction.intent == EditIntent::UpdateComposition && self.composition.is_none()
                {
                    return Err(Error::Composition);
                }
                if transaction.replacements.len() > 1 {
                    // Validate multi-replacement atomicity on a private staging copy; single edits retain the delta-only path.
                    let mut replacements = transaction.replacements;
                    replacements.sort_by_key(|r| std::cmp::Reverse(r.range.start));
                    if replacements.windows(2).any(|pair| {
                        pair[1].range.end > pair[0].range.start
                            || pair[1].range.start == pair[0].range.start
                    }) {
                        return Err(Error::InvalidRange);
                    }
                    let mut staged = self.clone();
                    let group = staged.composition.is_none();
                    if group {
                        staged.begin_composition(revision, view)?;
                    }
                    let mut deltas = vec![];
                    for r in replacements {
                        deltas.push(staged.edit(
                            staged.revision(),
                            view,
                            r.range.start as usize,
                            (r.range.end - r.range.start) as usize,
                            &r.text,
                        )?);
                    }
                    if group {
                        staged.commit_composition(staged.revision(), view)?;
                    }
                    *self = staged;
                    deltas
                } else if let Some(r) = transaction.replacements.first() {
                    vec![self.edit(
                        revision,
                        view,
                        r.range.start as usize,
                        (r.range.end - r.range.start) as usize,
                        &r.text,
                    )?]
                } else {
                    vec![]
                }
            }
        };
        let revision = BufferRevision(self.revision());
        let selection_anchors = deltas
            .last()
            .map(|d| {
                let position = d.start_utf16 as u64 + d.text.encode_utf16().count() as u64;
                BufferRange {
                    revision,
                    start: if undoing {
                        d.start_utf16 as u64
                    } else {
                        position
                    },
                    end: position,
                }
            })
            .into_iter()
            .collect();
        Ok(EditResult {
            revision,
            deltas,
            selection_anchors,
        })
    }
}
