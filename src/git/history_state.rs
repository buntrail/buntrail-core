use crate::contracts::*;
/// Browsing state is independent of the branch checked out on disk.
#[derive(Default)]
pub struct HistoryState {
    pub ref_filter: Vec<String>,
    pub selected_commit: Option<String>,
}
impl HistoryState {
    pub fn select(&mut self, oid: String) -> Result<(), CoreError> {
        if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CoreError::new(
                ErrorCode::InvalidArgument,
                "Select a commit object ID",
            ));
        }
        self.selected_commit = Some(oid);
        Ok(())
    }
}
