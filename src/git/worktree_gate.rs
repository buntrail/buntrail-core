use crate::contracts::*;
use std::sync::{Arc, Mutex};
/// Owner-thread admission only: identity resolution and all I/O happen after acquisition.
#[derive(Default)]
pub struct WorktreeMutationGate {
    active: Arc<Mutex<Vec<WorktreeIdentity>>>,
}
pub struct WorktreeMutationLease {
    worktree: WorktreeIdentity,
    active: Arc<Mutex<Vec<WorktreeIdentity>>>,
}
impl WorktreeMutationGate {
    pub fn acquire(&self, worktree: WorktreeIdentity) -> Result<WorktreeMutationLease, CoreError> {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.contains(&worktree) {
            return Err(CoreError::new(
                ErrorCode::Busy,
                "A worktree change is already pending",
            ));
        }
        active.push(worktree.clone());
        Ok(WorktreeMutationLease {
            worktree,
            active: self.active.clone(),
        })
    }
}
impl Drop for WorktreeMutationLease {
    fn drop(&mut self) {
        self.active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|worktree| worktree != &self.worktree);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_worktree_leases_keep_linked_siblings_independent() {
        let first = WorktreeIdentity {
            target: TargetIdentity {
                security_binding: vec![1],
                effective_user: vec![],
                filesystem_namespace: vec![2],
                container: None,
            },
            canonical_path: TargetPath::PosixBytes(b"/first".to_vec()),
            common_git_directory_identity: b"shared-repository".to_vec(),
        };
        let sibling = WorktreeIdentity {
            canonical_path: TargetPath::PosixBytes(b"/linked".to_vec()),
            ..first.clone()
        };
        let gate = WorktreeMutationGate::default();
        let first_lease = gate.acquire(first.clone()).unwrap();
        let sibling_lease = gate.acquire(sibling.clone()).unwrap();
        assert!(matches!(
            gate.acquire(first.clone()),
            Err(CoreError {
                code: ErrorCode::Busy,
                ..
            })
        ));
        drop(first_lease);
        let replacement = gate.acquire(first).unwrap();
        assert!(matches!(
            gate.acquire(sibling.clone()),
            Err(CoreError {
                code: ErrorCode::Busy,
                ..
            })
        ));
        drop(sibling_lease);
        assert!(gate.acquire(sibling).is_ok());
        drop(replacement);
    }
}
