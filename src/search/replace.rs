use super::find::*;
use crate::{Text, contracts::*};
pub fn transaction(
    text: &Text,
    revision: BufferRevision,
    view: ViewID,
    options: &FindOptions,
    replacement: &str,
) -> Result<EditTransaction, CoreError> {
    let plan = plan(text, revision, options, replacement)?;
    if plan.limited {
        return Err(CoreError::new(
            ErrorCode::Unavailable,
            "More than 20,000 matches; narrow the selection before Replace All",
        ));
    }
    Ok(EditTransaction {
        expected_revision: revision,
        source_view: view,
        replacements: plan.replacements,
        intent: EditIntent::Replace,
    })
}
