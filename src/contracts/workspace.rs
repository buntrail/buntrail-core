use super::{CoreError, ErrorCode};
macro_rules! uuid_ids { ($($name:ident),*) => { $(#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct $name(pub [u8;16]);)* }; }
uuid_ids!(
    WorkspaceID,
    DocumentID,
    OperationID,
    TerminalSessionID,
    TerminalTabID,
    ClientID,
    WorkspaceLeaseID,
    TransferID
);
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RequestId(pub u64);
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewID(pub u64);
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RouteKind {
    Local,
    Ssh,
    LocalContainer,
    SshContainer,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ContainerEngine {
    Docker,
    Podman,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentLocator {
    pub route: RouteKind,
    pub ssh_profile: Option<String>,
    pub engine: Option<ContainerEngine>,
    pub engine_context: Option<String>,
    pub container_selector: Option<String>,
    pub effective_user: Option<String>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceLocator {
    pub environment: EnvironmentLocator,
    pub root: TargetPath,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContainerIdentity {
    pub engine_endpoint: Vec<u8>,
    pub immutable_id: Vec<u8>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TargetIdentity {
    pub security_binding: Vec<u8>,
    pub effective_user: Vec<u8>,
    pub filesystem_namespace: Vec<u8>,
    pub container: Option<ContainerIdentity>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RuntimeGeneration {
    pub supervisor_boot: [u8; 16],
    pub container_start: Option<Vec<u8>>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceLease {
    pub client: ClientID,
    pub workspace: WorkspaceID,
    pub lease: WorkspaceLeaseID,
    pub runtime: RuntimeGeneration,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TargetPath {
    PosixBytes(Vec<u8>),
    WindowsWide(Vec<u16>),
}
impl TargetPath {
    /// Absolute filesystem paths only; preserve native units, including unpaired surrogates.
    pub fn windows(units: Vec<u16>) -> Result<Self, CoreError> {
        fn ascii_eq(units: &[u16], ascii: &str) -> bool {
            units.len() == ascii.len()
                && units.iter().zip(ascii.bytes()).all(|(u, b)| {
                    *u == u16::from(b.to_ascii_uppercase())
                        || *u == u16::from(b.to_ascii_lowercase())
                })
        }
        fn trimmed(mut units: &[u16]) -> &[u16] {
            while units.last().is_some_and(|u| *u == 32 || *u == 46) {
                units = &units[..units.len() - 1];
            }
            units
        }
        fn dos_device(component: &[u16]) -> bool {
            let base = trimmed(
                component
                    .split(|u| *u == 46 || *u == 58)
                    .next()
                    .unwrap_or_default(),
            );
            ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"]
                .iter()
                .any(|name| ascii_eq(base, name))
                || (base.len() == 4
                    && (ascii_eq(&base[..3], "COM") || ascii_eq(&base[..3], "LPT"))
                    && ((49..=57).contains(&base[3]) || [0xb9, 0xb2, 0xb3].contains(&base[3])))
        }
        let separator = |c: &u16| *c == 92 || *c == 47;
        let drive = |p: &[u16]| {
            p.len() >= 3
                && ((65..=90).contains(&p[0]) || (97..=122).contains(&p[0]))
                && p[1] == 58
                && separator(&p[2])
        };
        let unc = |p: &[u16]| {
            let mut parts = p.split(separator);
            let server = parts.next().unwrap_or_default();
            let share = parts.next().unwrap_or_default();
            !server.is_empty()
                && !share.is_empty()
                && ![&[46][..], &[46, 46][..], &[63][..]].contains(&server)
                && ![&[46][..], &[46, 46][..]].contains(&share)
                && !ascii_eq(trimmed(share), "pipe")
                && !ascii_eq(trimmed(share), "IPC$")
        };
        let absolute = if let Some(p) = units.strip_prefix(&[92, 92, 63, 92]) {
            !p.contains(&47)
                && (drive(p)
                    || (p.len() >= 4
                        && p[..3]
                            .iter()
                            .zip([85, 78, 67])
                            .all(|(c, upper)| *c == upper || *c == upper + 32)
                        && p[3] == 92
                        && unc(&p[4..])))
        } else {
            drive(&units)
                || (units.len() >= 2
                    && separator(&units[0])
                    && separator(&units[1])
                    && unc(&units[2..]))
        };
        // Verbatim disk paths bypass DOS alias normalization and may name literal files.
        let device = !units.starts_with(&[92, 92, 63, 92])
            && if drive(&units) {
                units[3..].split(separator).any(dos_device)
            } else {
                units
                    .get(2..)
                    .unwrap_or_default()
                    .split(separator)
                    .skip(2)
                    .any(dos_device)
            };
        if units.contains(&0) || !absolute || device {
            return Err(CoreError::new(
                ErrorCode::InvalidArgument,
                "expected absolute NUL-free Windows filesystem path",
            ));
        }
        Ok(Self::WindowsWide(units))
    }

    pub fn posix(bytes: Vec<u8>) -> Result<Self, CoreError> {
        if bytes.first() != Some(&b'/') || bytes.contains(&0) {
            return Err(CoreError::new(
                ErrorCode::InvalidArgument,
                "expected absolute NUL-free POSIX path",
            ));
        }
        Ok(Self::PosixBytes(bytes))
    }
    pub fn display(&self) -> String {
        match self {
            Self::PosixBytes(v) => String::from_utf8_lossy(v).into_owned(),
            Self::WindowsWide(v) => String::from_utf16_lossy(v),
        }
    }
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    Files,
    Watch,
    Search,
    Git,
    Terminal,
    Operations,
    Leases,
    Language,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Capabilities(pub Vec<Capability>);
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Resolving,
    Authenticating,
    PreparingHelper,
    Negotiating,
    Ready,
    Reconnecting,
    Disconnected,
    Failed(CoreError),
    TargetChanged {
        expected: TargetIdentity,
        observed: TargetIdentity,
    },
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionCloseReason {
    WindowDetachment,
    ResolvedFinalClose,
    ApplicationShutdown,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseReleaseIntent {
    ViewDetachment,
    ResolvedWorkspaceClosure,
}

/// A read-only observation never registers or resumes the owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeasePresence {
    Active,
    Detached { remaining_millis: u64 },
    Absent,
    RuntimeLost,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseObservation {
    pub owner: WorkspaceLease,
    pub target: TargetIdentity,
    pub observed_runtime: RuntimeGeneration,
    pub presence: LeasePresence,
    pub workers_retired: bool,
}
