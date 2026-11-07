use super::*;
use crate::contracts::*;
use std::collections::HashSet;
#[derive(Default)]
pub struct WorkspaceCoordinator {
    pub records: Vec<WorkspaceRecord>,
    open: HashSet<WorkspaceID>,
}
impl WorkspaceCoordinator {
    pub fn register(&mut self, record: WorkspaceRecord) -> WorkspaceID {
        let id = record.id;
        if let Some(existing) = self.records.iter_mut().find(|r| r.id == id) {
            *existing = record;
        } else {
            self.records.push(record);
        }
        self.open.insert(id);
        id
    }
    pub fn is_open(&self, id: WorkspaceID) -> bool {
        self.open.contains(&id)
    }
    pub fn detach(&mut self, id: WorkspaceID) {
        self.open.remove(&id);
    }
    pub fn record(&self, id: WorkspaceID) -> Result<&WorkspaceRecord, CoreError> {
        self.records
            .iter()
            .find(|r| r.id == id)
            .ok_or_else(|| CoreError::new(ErrorCode::InvalidArgument, "unknown workspace"))
    }
    pub fn record_mut(&mut self, id: WorkspaceID) -> Result<&mut WorkspaceRecord, CoreError> {
        self.records
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| CoreError::new(ErrorCode::InvalidArgument, "unknown workspace"))
    }
    /// A stored offset means something only for the exact disk content and scalar boundaries.
    pub fn validated_viewport(
        saved: &WorkspaceDocument,
        current: &DocumentSnapshot,
        version: &FileVersion,
    ) -> Option<EditorViewport> {
        let viewport = saved.viewport.as_ref()?;
        if saved.version.as_ref()?.content_digest != version.content_digest {
            return None;
        }
        let mut v = viewport.clone();
        v.selection.revision = current.revision;
        v.selection.to_bytes(current).ok()?;
        if !v.horizontal_offset.is_finite()
            || !v.vertical_offset.is_finite()
            || v.horizontal_offset < 0.0
            || v.vertical_offset < 0.0
        {
            return None;
        }
        v.revision_anchor = current.revision;
        Some(v)
    }
}
