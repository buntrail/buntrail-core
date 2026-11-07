use crate::{Document, Text, contracts::*};
use std::{collections::HashMap, sync::Arc};
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LoadedDocument {
    pub document: DocumentID,
    pub file: FileRef,
    pub version: FileVersion,
    pub text: Arc<Text>,
    pub encoding: Encoding,
    pub line_endings: LineEndings,
    pub read_only: Option<String>,
    #[serde(default)]
    pub journal_epoch: u64,
    #[serde(skip)]
    pub journal_lease: Option<Arc<()>>,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecoverySnapshot {
    pub snapshot: DocumentSnapshot,
    pub file: FileRef,
    pub version: FileVersion,
    pub pending_operation: Option<OperationID>,
    pub pending_revision: Option<BufferRevision>,
    pub provisional_composition: bool,
    pub generation: u64,
    #[serde(default)]
    pub epoch: u64,
    #[serde(skip)]
    pub lease: Option<Arc<()>>,
    pub clean: bool,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PreparedSave {
    pub operation: OperationID,
    pub recovery: RecoverySnapshot,
}
#[derive(Clone, Copy, Debug)]
pub enum ExternalResolution {
    KeepEdits,
    Reload,
}
pub struct DocumentSession {
    pub buffer: Document,
    loaded: LoadedDocument,
    saved_checkpoint: Option<u64>,
    restored_provisional: bool,
    journal_generation: u64,
    state: DocumentSaveState,
    pub external: Option<LoadedDocument>,
    pub missing: bool,
    pending: Option<(OperationID, BufferRevision, Option<u64>)>,
    selections: HashMap<ViewID, BufferRange>,
    undo_selections: Vec<BufferRange>,
    redo_selections: Vec<BufferRange>,
}
fn error(code: ErrorCode, message: &str) -> CoreError {
    CoreError::new(code, message)
}
impl DocumentSession {
    pub fn from_loaded(loaded: LoadedDocument) -> Self {
        Self {
            buffer: Document::from_text(loaded.text.clone()),
            saved_checkpoint: Some(0),
            restored_provisional: false,
            journal_generation: 0,
            state: DocumentSaveState::Clean(loaded.version.clone()),
            external: None,
            missing: false,
            loaded,
            pending: None,
            selections: HashMap::new(),
            undo_selections: vec![],
            redo_selections: vec![],
        }
    }
    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            document: self.loaded.document,
            revision: BufferRevision(self.buffer.revision()),
            text: self.buffer.snapshot().text,
            encoding: self.loaded.encoding.clone(),
            line_endings: self.loaded.line_endings.clone(),
            language: None,
        }
    }
    pub fn version(&self) -> &FileVersion {
        &self.loaded.version
    }
    pub fn revalidate(&mut self, loaded: LoadedDocument) -> Result<bool, CoreError> {
        if loaded.file.target != self.loaded.file.target
            || loaded.file.path != self.loaded.file.path
        {
            return Err(error(
                ErrorCode::TargetChanged,
                "revalidation identity mismatch",
            ));
        }
        self.missing = false;
        if loaded.version == self.loaded.version {
            self.external = None;
            return Ok(false);
        }
        if self.is_dirty() || self.pending.is_some() || self.buffer.has_composition() {
            self.external = Some(loaded);
            return Ok(false);
        }
        let revision = self
            .buffer
            .revision()
            .checked_add(1)
            .ok_or_else(|| error(ErrorCode::Busy, "revision exhausted"))?;
        self.buffer = Document::restore_text(loaded.text.clone(), revision);
        self.loaded.text = loaded.text;
        self.loaded.version = loaded.version;
        self.loaded.file = loaded.file;
        self.loaded.encoding = loaded.encoding;
        self.loaded.line_endings = loaded.line_endings;
        self.saved_checkpoint = Some(self.buffer.checkpoint());
        self.state = DocumentSaveState::Clean(self.loaded.version.clone());
        self.undo_selections.clear();
        self.redo_selections.clear();
        self.external = None;
        self.journal_generation = self.journal_generation.saturating_add(1);
        Ok(true)
    }
    pub fn rebind(&mut self, file: FileRef, version: FileVersion) -> Result<(), CoreError> {
        if self.pending.is_some() {
            return Err(error(ErrorCode::Busy, "resolve active save before moving"));
        }
        if file.target != self.loaded.file.target || version.target != file.target {
            return Err(error(ErrorCode::TargetChanged, "moved target mismatch"));
        }
        self.loaded.file = file;
        // A rename can race an external write; retaining the old base keeps the next save in conflict.
        if version.content_digest == self.loaded.version.content_digest {
            self.loaded.version = version;
            if !self.is_dirty() {
                self.state = DocumentSaveState::Clean(self.loaded.version.clone());
            }
        }
        self.missing = false;
        self.journal_generation = self.journal_generation.saturating_add(1);
        Ok(())
    }
    /// The fresh read must still match the version the user compared and the captured revision.
    pub fn resolve_external(
        &mut self,
        loaded: LoadedDocument,
        revision: BufferRevision,
        compared: &FileVersion,
        choice: ExternalResolution,
    ) -> Result<(), CoreError> {
        if self.snapshot().revision != revision
            || loaded.version != *compared
            || self
                .external
                .as_ref()
                .is_none_or(|external| external.version != *compared)
        {
            return Err(error(
                ErrorCode::StaleRevision,
                "file or edits changed; compare again before resolving",
            ));
        }
        if loaded.document != self.loaded.document
            || loaded.file.target != self.loaded.file.target
            || loaded.file.path != self.loaded.file.path
            || loaded.version.target != loaded.file.target
            || self
                .external
                .as_ref()
                .is_none_or(|external| external.file != loaded.file)
        {
            return Err(error(
                ErrorCode::TargetChanged,
                "resolution identity mismatch",
            ));
        }
        self.discard()?;
        match choice {
            ExternalResolution::KeepEdits => {
                self.loaded.file = loaded.file;
                self.loaded.version = loaded.version;
                self.loaded.text = loaded.text;
                self.saved_checkpoint = None;
                self.state = DocumentSaveState::Dirty;
            }
            ExternalResolution::Reload => {
                let revision = revision
                    .0
                    .checked_add(1)
                    .ok_or_else(|| error(ErrorCode::Busy, "revision exhausted"))?;
                self.buffer = Document::restore_text(loaded.text.clone(), revision);
                self.loaded = LoadedDocument {
                    journal_epoch: self.loaded.journal_epoch,
                    journal_lease: self.loaded.journal_lease.clone(),
                    ..loaded
                };
                self.saved_checkpoint = Some(self.buffer.checkpoint());
                self.state = DocumentSaveState::Clean(self.loaded.version.clone());
                self.undo_selections.clear();
                self.redo_selections.clear();
            }
        }
        self.external = None;
        self.missing = false;
        self.journal_generation = self.journal_generation.saturating_add(1);
        Ok(())
    }
    /// Prepare a recovery tombstone without changing the live buffer or saved checkpoint.
    /// The owner must enqueue it only when the authority is actually discarded.
    pub fn discard(&self) -> Result<RecoverySnapshot, CoreError> {
        if self.pending.is_some() || self.buffer.has_composition() || self.restored_provisional {
            return Err(error(
                ErrorCode::Busy,
                "resolve active save/composition before discarding",
            ));
        }
        let mut marker = self.recovery_snapshot();
        marker.clean = true;
        marker.generation = marker
            .generation
            .checked_add(1)
            .ok_or_else(|| error(ErrorCode::Busy, "recovery generation exhausted"))?;
        Ok(marker)
    }
    pub fn begin_typing(&mut self, view: ViewID) {
        self.buffer.begin_typing(view.0);
    }
    pub fn end_typing(&mut self) {
        self.buffer.end_typing();
    }
    pub fn file(&self) -> &FileRef {
        &self.loaded.file
    }
    pub fn is_dirty(&self) -> bool {
        self.saved_checkpoint != Some(self.buffer.checkpoint())
    }
    pub fn save_state(&self) -> &DocumentSaveState {
        &self.state
    }
    pub fn set_selection(&mut self, view: ViewID, selection: BufferRange) -> Result<(), CoreError> {
        selection.to_bytes(&self.snapshot())?;
        self.selections.insert(view, selection);
        Ok(())
    }
    pub fn apply_transaction(
        &mut self,
        transaction: EditTransaction,
    ) -> Result<EditResult, CoreError> {
        let current = self.snapshot();
        if transaction.expected_revision != current.revision {
            return Err(error(ErrorCode::StaleRevision, "stale document revision"));
        }
        let mut length = current.text.len();
        for replacement in &transaction.replacements {
            let bytes = replacement.range.to_bytes(&current)?;
            length = length
                .checked_sub((bytes.end - bytes.start) as usize)
                .and_then(|n| n.checked_add(replacement.text.len()))
                .ok_or_else(|| error(ErrorCode::InvalidArgument, "replacement length overflow"))?;
        }
        if length > 16 * 1024 * 1024 {
            return Err(error(
                ErrorCode::Unavailable,
                "document exceeds editable size limit",
            ));
        }
        if self.loaded.read_only.is_some() {
            return Err(error(
                ErrorCode::Unavailable,
                "document requires explicit encoding selection",
            ));
        }
        if self.restored_provisional {
            if transaction.intent != EditIntent::CommitComposition {
                return Err(error(
                    ErrorCode::Composition,
                    "resolve restored provisional text explicitly before editing",
                ));
            }
            self.buffer
                .begin_composition(self.buffer.revision(), transaction.source_view.0)
                .map_err(|_| error(ErrorCode::Composition, "composition active"))?;
        }
        let view = transaction.source_view;
        let intent = transaction.intent.clone();
        let before = self
            .selections
            .get(&view)
            .cloned()
            .or_else(|| transaction.replacements.first().map(|r| r.range.clone()));
        let composition = self.buffer.has_composition();
        let undo_depth = self.buffer.undo_depth();
        let mut result = self.buffer.apply_transaction(transaction).map_err(|e| {
            error(
                match e {
                    crate::Error::Stale => ErrorCode::StaleRevision,
                    crate::Error::Composition => ErrorCode::Composition,
                    _ => ErrorCode::InvalidArgument,
                },
                "edit rejected",
            )
        })?;
        match intent {
            EditIntent::Undo => {
                if let Some(s) = self.undo_selections.pop() {
                    self.redo_selections.push(s.clone());
                    result.selection_anchors = vec![BufferRange {
                        revision: result.revision,
                        ..s
                    }];
                }
            }
            EditIntent::Redo => {
                if let Some(s) = self.redo_selections.pop() {
                    self.undo_selections.push(s);
                }
            }
            EditIntent::Replace
                if !composition
                    && self.buffer.undo_depth() > undo_depth
                    && !result.deltas.is_empty() =>
            {
                if let Some(s) = before {
                    self.undo_selections.push(s);
                    self.redo_selections.clear();
                }
            }
            EditIntent::BeginComposition => {
                self.undo_selections.push(before.unwrap_or(BufferRange {
                    revision: result.revision,
                    start: 0,
                    end: 0,
                }));
            }
            EditIntent::CommitComposition if self.buffer.undo_depth() == undo_depth => {
                self.undo_selections.pop();
            }
            EditIntent::CancelComposition => {
                self.undo_selections.pop();
            }
            _ => {}
        }
        self.journal_generation = self.journal_generation.saturating_add(1);
        if intent == EditIntent::CommitComposition {
            self.restored_provisional = false;
        }
        if let Some(s) = result.selection_anchors.first() {
            self.selections.insert(view, s.clone());
        }
        if self.pending.is_none() {
            if self.is_dirty() {
                self.state = DocumentSaveState::Dirty;
            } else if matches!(self.state, DocumentSaveState::Dirty) {
                self.state = DocumentSaveState::Clean(self.loaded.version.clone());
            }
        }
        Ok(result)
    }
    pub fn recovery_snapshot(&self) -> RecoverySnapshot {
        RecoverySnapshot {
            snapshot: self.snapshot(),
            file: self.loaded.file.clone(),
            version: self.loaded.version.clone(),
            pending_operation: self.pending.map(|p| p.0),
            pending_revision: self.pending.map(|p| p.1),
            provisional_composition: self.buffer.has_composition() || self.restored_provisional,
            generation: self.journal_generation,
            epoch: self.loaded.journal_epoch,
            lease: self.loaded.journal_lease.clone(),
            clean: !self.is_dirty()
                && self.pending.is_none()
                && !self.buffer.has_composition()
                && !self.restored_provisional,
        }
    }
    pub fn prepare_save(&mut self, operation: OperationID) -> Result<PreparedSave, CoreError> {
        if self.buffer.has_composition() || self.restored_provisional {
            return Err(error(
                ErrorCode::Composition,
                "resolve composition before saving",
            ));
        }
        if self.loaded.read_only.is_some() {
            return Err(error(ErrorCode::Unavailable, "read-only document"));
        }
        if self.pending.is_some() || matches!(self.state, DocumentSaveState::Unknown(_)) {
            return Err(error(
                ErrorCode::Busy,
                "inspect pending save before another save",
            ));
        }
        // Keep the submitted checkpoint reachable if typing continues during the save.
        self.buffer.end_typing();
        let revision = BufferRevision(self.buffer.revision());
        self.journal_generation = self.journal_generation.saturating_add(1);
        self.pending = Some((operation, revision, Some(self.buffer.checkpoint())));
        self.state = DocumentSaveState::Saving {
            operation,
            revision,
        };
        Ok(PreparedSave {
            operation,
            recovery: self.recovery_snapshot(),
        })
    }
    pub fn complete_save(&mut self, receipt: OperationReceipt) -> Result<(), CoreError> {
        let Some((id, revision, checkpoint)) = self.pending else {
            return Err(error(ErrorCode::InvalidTransition, "no pending save"));
        };
        if receipt.id != id {
            return Err(error(ErrorCode::InvalidArgument, "save operation mismatch"));
        }
        match receipt.state {
            OperationState::Confirmed(MutationResult::Saved(saved)) => {
                if saved.saved_revision != revision
                    || saved.version.target != self.loaded.file.target
                {
                    return Err(error(
                        ErrorCode::InvalidArgument,
                        "save result identity mismatch",
                    ));
                }
                self.saved_checkpoint = checkpoint;
                self.loaded.version = saved.version.clone();
                self.loaded.file.resolved_identity = Some(saved.version.file_identity.clone());
                self.state = if self.is_dirty() {
                    DocumentSaveState::Dirty
                } else {
                    DocumentSaveState::Clean(saved.version)
                };
                self.pending = None;
            }
            OperationState::Failed(f) => {
                self.state = DocumentSaveState::Failed(f);
                self.pending = None;
            }
            OperationState::Unknown(e) => self.state = DocumentSaveState::Unknown(e),
            OperationState::Accepted | OperationState::Running => {}
            _ => return Err(error(ErrorCode::InvalidArgument, "not a save receipt")),
        }
        self.journal_generation = self.journal_generation.saturating_add(1);
        Ok(())
    }
}
#[derive(Default)]
pub struct DocumentCoordinator {
    sessions: HashMap<DocumentID, DocumentSession>,
}
impl DocumentCoordinator {
    pub fn restore(&mut self, recovery: RecoverySnapshot) -> Result<DocumentID, CoreError> {
        if self.sessions.contains_key(&recovery.snapshot.document)
            || self.id_for_file(&recovery.file).is_some()
        {
            return Err(error(
                ErrorCode::Busy,
                "resolve open document before restoring a draft",
            ));
        }
        let revision = recovery.snapshot.revision;
        let target = recovery.file.target.clone();
        let id = self.open(LoadedDocument {
            document: recovery.snapshot.document,
            file: recovery.file,
            version: recovery.version,
            text: recovery.snapshot.text.clone(),
            encoding: recovery.snapshot.encoding,
            line_endings: recovery.snapshot.line_endings,
            read_only: None,
            journal_epoch: recovery.epoch,
            journal_lease: recovery.lease,
        })?;
        let session = self.get_mut(id)?;
        session.buffer = Document::restore_text(recovery.snapshot.text, revision.0);
        session.saved_checkpoint = None;
        session.journal_generation = recovery.generation;
        session.pending = recovery
            .pending_operation
            .zip(recovery.pending_revision)
            .map(|(id, pending_revision)| {
                // Only this exact journaled revision identifies the submitted checkpoint after restore.
                let checkpoint = (pending_revision == revision
                    && !recovery.provisional_composition)
                    .then_some(session.buffer.checkpoint());
                (id, pending_revision, checkpoint)
            });
        session.state = if session.pending.is_some() {
            DocumentSaveState::Unknown(ReconciliationEvidence {
                target,
                observed: ObservedState::Unobserved,
                explanation: "restored pending save; inspect before another save".into(),
            })
        } else {
            DocumentSaveState::Dirty
        };
        if recovery.provisional_composition {
            session.restored_provisional = true;
        }
        Ok(id)
    }
    pub fn open(&mut self, loaded: LoadedDocument) -> Result<DocumentID, CoreError> {
        if loaded.version.target != loaded.file.target
            || loaded
                .file
                .resolved_identity
                .as_ref()
                .is_some_and(|i| *i != loaded.version.file_identity)
        {
            return Err(error(ErrorCode::TargetChanged, "loaded identity mismatch"));
        }
        if let Some((id, _)) = self.sessions.iter().find(|(_, s)| {
            s.loaded.file.target == loaded.file.target
                && s.loaded.version.file_identity == loaded.version.file_identity
        }) {
            return Ok(*id);
        }
        if self.sessions.contains_key(&loaded.document) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "document ID already registered",
            ));
        }
        if self.sessions.len() >= 64 {
            return Err(error(ErrorCode::Busy, "open document limit reached"));
        }
        let id = loaded.document;
        self.sessions
            .insert(id, DocumentSession::from_loaded(loaded));
        Ok(id)
    }
    pub fn ids(&self) -> Vec<DocumentID> {
        self.sessions.keys().copied().collect()
    }
    pub fn pending_document(&self, operation: OperationID) -> Option<DocumentID> {
        self.sessions
            .iter()
            .find(|(_, s)| s.pending.is_some_and(|p| p.0 == operation))
            .map(|(id, _)| *id)
    }
    pub fn close(&mut self, id: DocumentID) -> Result<(), CoreError> {
        let s = self.get(id)?;
        if s.is_dirty()
            || s.pending.is_some()
            || s.buffer.has_composition()
            || s.restored_provisional
        {
            return Err(error(
                ErrorCode::Busy,
                "resolve unsaved work before closing document",
            ));
        }
        self.sessions.remove(&id);
        Ok(())
    }
    /// Remove the authority after a completed, explicitly authorized ownership transition.
    pub fn close_discarding(&mut self, id: DocumentID) -> Result<(), CoreError> {
        self.get(id)?.discard()?;
        self.sessions.remove(&id);
        Ok(())
    }
    /// The owner has durably retained this exact unknown-save draft and confirmed final release.
    pub fn close_retaining_recovery(&mut self, id: DocumentID) -> Result<(), CoreError> {
        let session = self.get(id)?;
        if !matches!(session.state, DocumentSaveState::Unknown(_))
            || session.pending.is_none()
            || session.buffer.has_composition()
            || session.restored_provisional
        {
            return Err(error(
                ErrorCode::Busy,
                "only a resolved close choice may retain an unknown-save draft",
            ));
        }
        self.sessions.remove(&id);
        Ok(())
    }
    pub fn check_close(&self) -> Result<(), CoreError> {
        for s in self.sessions.values() {
            if s.buffer.has_composition() || s.restored_provisional {
                return Err(error(
                    ErrorCode::Composition,
                    "resolve composition before closing",
                ));
            }
            if s.pending.is_some() {
                return Err(error(
                    ErrorCode::Busy,
                    "inspect pending save before closing",
                ));
            }
        }
        Ok(())
    }
    pub fn id_for_file(&self, file: &FileRef) -> Option<DocumentID> {
        self.sessions
            .iter()
            .find(|(_, s)| {
                s.loaded.file.target == file.target
                    && (s.loaded.file.path == file.path
                        || file.resolved_identity.as_ref() == Some(&s.loaded.version.file_identity))
            })
            .map(|(id, _)| *id)
    }
    pub fn get(&self, id: DocumentID) -> Result<&DocumentSession, CoreError> {
        self.sessions
            .get(&id)
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "unknown document"))
    }
    pub fn get_mut(&mut self, id: DocumentID) -> Result<&mut DocumentSession, CoreError> {
        self.sessions
            .get_mut(&id)
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "unknown document"))
    }
}
