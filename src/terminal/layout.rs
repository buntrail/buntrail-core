use crate::contracts::{
    CoreError, ErrorCode, OperationID, RuntimeGeneration, TerminalSessionID, TerminalTabID,
};
use std::collections::HashSet;
pub const MAX_LAYOUT_IDENTITIES: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SplitOrientation {
    Right,
    Below,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TerminalNode {
    Pane(TerminalSessionID),
    Split {
        orientation: SplitOrientation,
        ratio: f64,
        first: Box<TerminalNode>,
        second: Box<TerminalNode>,
    },
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TerminalTab {
    pub id: TerminalTabID,
    pub focused: TerminalSessionID,
    pub root: TerminalNode,
}
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TerminalLayoutState {
    pub tabs: Vec<TerminalTab>,
    pub selected: Option<TerminalTabID>,
    pub runtimes: Vec<(TerminalSessionID, OperationID, RuntimeGeneration)>,
    #[serde(default)]
    pub retired: Vec<(TerminalSessionID, OperationID, RuntimeGeneration)>,
}
fn invalid() -> CoreError {
    CoreError::new(ErrorCode::InvalidArgument, "invalid terminal layout")
}
impl TerminalNode {
    fn sessions(&self, result: &mut Vec<TerminalSessionID>) {
        match self {
            Self::Pane(id) => result.push(*id),
            Self::Split { first, second, .. } => {
                first.sessions(result);
                second.sessions(result);
            }
        }
    }
    fn split(
        &mut self,
        target: TerminalSessionID,
        id: TerminalSessionID,
        orientation: SplitOrientation,
    ) -> bool {
        match self {
            Self::Pane(current) if *current == target => {
                *self = Self::Split {
                    orientation,
                    ratio: 0.5,
                    first: Box::new(Self::Pane(target)),
                    second: Box::new(Self::Pane(id)),
                };
                true
            }
            Self::Pane(_) => false,
            Self::Split { first, second, .. } => {
                first.split(target, id, orientation) || second.split(target, id, orientation)
            }
        }
    }
    // Locate the group before editing so perpendicular boundaries stay independent.
    fn group_path(
        &self,
        target: TerminalSessionID,
        orientation: Option<SplitOrientation>,
    ) -> Option<(Vec<bool>, SplitOrientation)> {
        fn locate(
            node: &TerminalNode,
            target: TerminalSessionID,
            path: &mut Vec<(bool, SplitOrientation)>,
        ) -> bool {
            match node {
                TerminalNode::Pane(id) => *id == target,
                TerminalNode::Split {
                    orientation,
                    first,
                    second,
                    ..
                } => {
                    path.push((false, *orientation));
                    if locate(first, target, path) {
                        return true;
                    }
                    path.last_mut().unwrap().0 = true;
                    if locate(second, target, path) {
                        return true;
                    }
                    path.pop();
                    false
                }
            }
        }
        let mut path = Vec::new();
        if !locate(self, target, &mut path) {
            return None;
        }
        let orientation = orientation.or_else(|| path.last().map(|(_, axis)| *axis))?;
        while path.last().is_some_and(|(_, axis)| *axis == orientation) {
            path.pop();
        }
        Some((
            path.into_iter().map(|(second, _)| second).collect(),
            orientation,
        ))
    }
    fn equalize_at(&mut self, path: &[bool], orientation: SplitOrientation) {
        let mut node = self;
        for second_child in path {
            let Self::Split { first, second, .. } = node else {
                unreachable!("surviving split group path")
            };
            node = if *second_child { second } else { first };
        }
        node.equalize(orientation);
    }
    fn equalize(&mut self, axis: SplitOrientation) -> usize {
        if let Self::Split {
            orientation,
            ratio,
            first,
            second,
        } = self
            && *orientation == axis
        {
            let left = first.equalize(axis);
            let right = second.equalize(axis);
            *ratio = left as f64 / (left + right) as f64;
            return left + right;
        }
        1
    }
    fn resize(&mut self, target: TerminalSessionID, value: f64) -> bool {
        match self {
            Self::Pane(_) => false,
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => {
                if first.resize(target, value) || second.resize(target, value) {
                    return true;
                }
                if matches!(first.as_ref(), Self::Pane(id) if *id == target)
                    || matches!(second.as_ref(), Self::Pane(id) if *id == target)
                {
                    *ratio = value;
                    true
                } else {
                    false
                }
            }
        }
    }
    fn remove(self, target: TerminalSessionID) -> Option<Self> {
        match self {
            Self::Pane(id) => (id != target).then_some(Self::Pane(id)),
            Self::Split {
                orientation,
                ratio,
                first,
                second,
            } => match (first.remove(target), second.remove(target)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    orientation,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(node), None) | (None, Some(node)) => Some(node),
                (None, None) => None,
            },
        }
    }
    fn valid(&self, ids: &mut HashSet<TerminalSessionID>, depth: usize) -> bool {
        if depth > 64 {
            return false;
        }
        match self {
            Self::Pane(id) => ids.insert(*id),
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => {
                ratio.is_finite()
                    && *ratio > 0.0
                    && *ratio < 1.0
                    && first.valid(ids, depth + 1)
                    && second.valid(ids, depth + 1)
            }
        }
    }
}
impl TerminalLayoutState {
    pub fn sessions(&self) -> Vec<TerminalSessionID> {
        let mut result = vec![];
        for tab in &self.tabs {
            tab.root.sessions(&mut result);
        }
        result
    }
    pub fn validate(&self) -> Result<(), CoreError> {
        let mut ids = HashSet::new();
        let mut tabs = HashSet::new();
        let mut operations = HashSet::new();
        let mut all_sessions = ids.clone();
        if self.tabs.len() > 64
            || self.tabs.iter().any(|tab| {
                !tabs.insert(tab.id) || !tab.root.valid(&mut ids, 0) || {
                    let mut pane = vec![];
                    tab.root.sessions(&mut pane);
                    !pane.contains(&tab.focused)
                }
            })
            || ids.len() > 64
            || self.runtimes.len() != ids.len()
            || self.runtimes.iter().any(|(session, operation, _)| {
                !ids.remove(session) || !operations.insert(*operation)
            })
            || self.retired.len() + self.runtimes.len() > MAX_LAYOUT_IDENTITIES
            || {
                all_sessions.extend(self.runtimes.iter().map(|(id, _, _)| *id));
                self.retired.iter().any(|(id, operation, _)| {
                    !all_sessions.insert(*id) || !operations.insert(*operation)
                })
            }
            || (self.tabs.is_empty() != self.selected.is_none())
            || self.selected.is_some_and(|id| !tabs.contains(&id))
        {
            Err(invalid())
        } else {
            Ok(())
        }
    }
    pub fn new_tab(
        &mut self,
        tab_id: TerminalTabID,
        id: TerminalSessionID,
        operation: OperationID,
        runtime: RuntimeGeneration,
    ) -> Result<(), CoreError> {
        self.validate()?;
        if self.retired.len() + self.runtimes.len() >= MAX_LAYOUT_IDENTITIES {
            return Err(CoreError::new(
                ErrorCode::Busy,
                "terminal identity capacity reached; retained runtimes require reconciliation",
            ));
        }
        if self
            .runtimes
            .iter()
            .any(|(session, old, _)| *session == id || *old == operation)
            || self
                .retired
                .iter()
                .any(|(session, old, _)| *session == id || *old == operation)
            || self.sessions().contains(&id)
            || self.tabs.iter().any(|t| t.id == tab_id)
            || self.sessions().len() >= 64
        {
            return Err(invalid());
        }
        self.tabs.push(TerminalTab {
            id: tab_id,
            focused: id,
            root: TerminalNode::Pane(id),
        });
        self.selected = Some(tab_id);
        self.runtimes.push((id, operation, runtime));
        Ok(())
    }
    pub fn split(
        &mut self,
        target: TerminalSessionID,
        id: TerminalSessionID,
        orientation: SplitOrientation,
        operation: OperationID,
        runtime: RuntimeGeneration,
    ) -> Result<(), CoreError> {
        self.validate()?;
        if self.retired.len() + self.runtimes.len() >= MAX_LAYOUT_IDENTITIES {
            return Err(CoreError::new(
                ErrorCode::Busy,
                "terminal identity capacity reached; retained runtimes require reconciliation",
            ));
        }
        if self
            .runtimes
            .iter()
            .any(|(session, old, _)| *session == id || *old == operation)
            || self
                .retired
                .iter()
                .any(|(session, old, _)| *session == id || *old == operation)
            || self.sessions().contains(&id)
            || self.sessions().len() >= 64
        {
            return Err(invalid());
        }
        let tab = self
            .tabs
            .iter_mut()
            .find(|t| {
                let mut ids = vec![];
                t.root.sessions(&mut ids);
                ids.contains(&target)
            })
            .ok_or_else(invalid)?;
        let (path, axis) = tab
            .root
            .group_path(target, Some(orientation))
            .expect("validated target");
        tab.root.split(target, id, orientation);
        tab.root.equalize_at(&path, axis);
        tab.focused = id;
        self.selected = Some(tab.id);
        self.runtimes.push((id, operation, runtime));
        Ok(())
    }
    pub fn focus(&mut self, id: TerminalSessionID) -> Result<(), CoreError> {
        self.validate()?;
        let tab = self
            .tabs
            .iter_mut()
            .find(|t| {
                let mut ids = vec![];
                t.root.sessions(&mut ids);
                ids.contains(&id)
            })
            .ok_or_else(invalid)?;
        tab.focused = id;
        self.selected = Some(tab.id);
        Ok(())
    }
    pub fn resize(&mut self, id: TerminalSessionID, ratio: f64) -> Result<(), CoreError> {
        self.validate()?;
        if !ratio.is_finite() || ratio <= 0.0 || ratio >= 1.0 {
            return Err(invalid());
        }
        if self.tabs.iter_mut().any(|t| t.root.resize(id, ratio)) {
            Ok(())
        } else {
            Err(invalid())
        }
    }
    pub fn resize_at(
        &mut self,
        tab: TerminalTabID,
        path: u64,
        depth: u32,
        ratio: f64,
    ) -> Result<(), CoreError> {
        self.validate()?;
        if depth > 63 || path >> depth != 0 || !ratio.is_finite() || ratio <= 0.0 || ratio >= 1.0 {
            return Err(invalid());
        }
        let mut node = &mut self
            .tabs
            .iter_mut()
            .find(|item| item.id == tab)
            .ok_or_else(invalid)?
            .root;
        for bit in 0..depth {
            node = match node {
                TerminalNode::Split { first, second, .. } => {
                    if path & (1 << bit) == 0 {
                        first
                    } else {
                        second
                    }
                }
                _ => return Err(invalid()),
            };
        }
        if let TerminalNode::Split { ratio: value, .. } = node {
            *value = ratio;
            Ok(())
        } else {
            Err(invalid())
        }
    }
    pub fn close(&mut self, id: TerminalSessionID) -> Result<(), CoreError> {
        self.validate()?;
        let index = self
            .tabs
            .iter()
            .position(|t| {
                let mut ids = vec![];
                t.root.sessions(&mut ids);
                ids.contains(&id)
            })
            .ok_or_else(invalid)?;
        let intent = self
            .runtimes
            .iter()
            .find(|(session, _, _)| *session == id)
            .ok_or_else(invalid)?;
        self.retired.push((intent.0, intent.1, intent.2.clone()));
        self.runtimes.retain(|(session, _, _)| *session != id);
        let group = self.tabs[index].root.group_path(id, None);
        let root = self.tabs[index].root.clone().remove(id);
        if let Some(root) = root {
            let tab = &mut self.tabs[index];
            tab.root = root;
            if let Some((path, axis)) = group {
                tab.root.equalize_at(&path, axis);
            }
            if tab.focused == id {
                let mut ids = vec![];
                tab.root.sessions(&mut ids);
                tab.focused = ids[0];
            }
        } else {
            let removed = self.tabs.remove(index);
            if self.selected == Some(removed.id) {
                self.selected = self
                    .tabs
                    .get(index.min(self.tabs.len().saturating_sub(1)))
                    .map(|t| t.id);
            }
        }
        Ok(())
    }
}
