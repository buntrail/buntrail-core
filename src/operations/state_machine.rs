use crate::contracts::*;
pub struct OperationStateMachine {
    state: OperationState,
}
impl Default for OperationStateMachine {
    fn default() -> Self {
        Self::new()
    }
}
impl OperationStateMachine {
    pub fn new() -> Self {
        Self {
            state: OperationState::Accepted,
        }
    }
    pub fn state(&self) -> &OperationState {
        &self.state
    }
    pub fn transition(&mut self, event: OperationEvent) -> Result<OperationState, CoreError> {
        let next = match (&self.state, event) {
            (OperationState::Accepted, OperationEvent::Started) => OperationState::Running,
            (
                OperationState::Accepted | OperationState::Running,
                OperationEvent::Confirmed(result),
            ) => OperationState::Confirmed(result),
            (
                OperationState::Accepted | OperationState::Running,
                OperationEvent::Failed(failure),
            ) => match failure.effect {
                FailureEffect::ProvenNoEffect => OperationState::Failed(failure),
                FailureEffect::PossiblePartialEffects => {
                    OperationState::Unknown(ReconciliationEvidence {
                        target: failure.target,
                        observed: failure.observed,
                        explanation: failure.error.message,
                    })
                }
            },
            (
                OperationState::Accepted | OperationState::Running | OperationState::Unknown(_),
                OperationEvent::LostAcknowledgement(evidence),
            ) => OperationState::Unknown(evidence),
            (OperationState::Unknown(_), OperationEvent::Reconciled(result)) => {
                OperationState::Confirmed(result)
            }
            _ => {
                return Err(CoreError::new(
                    ErrorCode::InvalidTransition,
                    "operation requires evidence; replay is forbidden",
                ));
            }
        };
        self.state = next;
        Ok(self.state.clone())
    }
}
