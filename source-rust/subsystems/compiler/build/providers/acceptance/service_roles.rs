//! Host service roles and their persisted tags.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum AcceptedSemanticBindingRole {
    /// Accepted terminal input and output operations.
    ConsoleIo,
    /// Package provider for core's ProcessExit boundary requirement.
    ProcessExitExitProcessI32,
    /// Package-owned host filesystem service.
    FilesystemHostService,
    /// Package-owned host clock service.
    TimeHostService,
}

impl AcceptedSemanticBindingRole {
    /// Package locks and prepared dependency identities bind this role tag.
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::ConsoleIo => 10,
            Self::ProcessExitExitProcessI32 => 1,
            Self::FilesystemHostService => 2,
            Self::TimeHostService => 3,
        }
    }

    pub const fn from_canonical_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            10 => Self::ConsoleIo,
            1 => Self::ProcessExitExitProcessI32,
            2 => Self::FilesystemHostService,
            3 => Self::TimeHostService,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::AcceptedSemanticBindingRole;

    #[test]
    fn reviewed_binding_roles_keep_their_persisted_tags() {
        use AcceptedSemanticBindingRole::*;
        for tag in 0..=u8::MAX {
            let role = match tag {
                1 => Some(ProcessExitExitProcessI32),
                2 => Some(FilesystemHostService),
                3 => Some(TimeHostService),
                10 => Some(ConsoleIo),
                _ => None,
            };
            assert_eq!(AcceptedSemanticBindingRole::from_canonical_tag(tag), role);
            if let Some(role) = role {
                assert_eq!(role.canonical_tag(), tag);
            }
        }
    }
}
