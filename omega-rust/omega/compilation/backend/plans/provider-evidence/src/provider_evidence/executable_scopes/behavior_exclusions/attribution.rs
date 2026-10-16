//! Human descriptions of exclusion evidence.
//! Diagnostic text never supplies service ownership or grants authority.

use super::{BehaviorExclusion, EvidenceGapKind, ProhibitedSite};

impl BehaviorExclusion {
    /// Describe this exclusion, resolving service identities in the exact module.
    pub fn describe(self, module: &terminal_psi::TerminalModule) -> String {
        match self {
            Self::CrashCause(cause) => {
                format!("crash cause {cause:?}")
            }
            Self::Service(service) => match module.service(service) {
                Some(declaration) => format!(
                    "service `{}`",
                    module
                        .identities
                        .text(declaration.identity)
                        .unwrap_or_default()
                ),
                None => format!("service {service}"),
            },
            // A physical-class selection is a demand on the mechanism closure:
            // this walk never yields a prohibited site for it, but the arm keeps
            // the description total if one is ever named.
            Self::PhysicalAuthorityClass(class) => {
                format!("physical authority class {class:?}")
            }
        }
    }
}

impl ProhibitedSite {
    /// Describe a retained possible behavior site, not an observed execution.
    pub fn describe(&self, module: &terminal_psi::TerminalModule) -> String {
        match self {
            ProhibitedSite::CrashTerminator { block } => {
                format!("crash terminator in block {block}")
            }
            ProhibitedSite::BoundaryCall {
                block,
                operation,
                boundary,
            } => {
                let identity =
                    terminal_psi::find_boundary_machine(&module.boundary_machines, *boundary)
                        .map(|declaration| {
                            module
                                .identities
                                .text(declaration.identity)
                                .unwrap_or("<undeclared boundary>")
                        })
                        .unwrap_or("<undeclared boundary>");
                format!("call to boundary `{identity}` (block {block}, operation {operation})")
            }
            ProhibitedSite::CheckedInstruction { block, operation } => {
                format!("checked instruction (block {block}, operation {operation})")
            }
        }
    }
}

impl EvidenceGapKind {
    /// Describe why the retained closure cannot establish absence.
    pub fn describe(&self) -> String {
        match self {
            EvidenceGapKind::UnknownCallee(callee) => {
                format!("call target machine {callee} is not retained in the module")
            }
            EvidenceGapKind::UnknownBoundary(boundary) => {
                format!("boundary {boundary} has no retained declaration")
            }
            EvidenceGapKind::UnknownBoundaryService(boundary) => {
                format!("invoked trait requirement {boundary} has no retained service owner")
            }
            EvidenceGapKind::UnknownEntry => {
                "the selected entry is not retained in the module".to_owned()
            }
            EvidenceGapKind::TargetFamilySelection {
                head,
                target,
                reason,
            } => {
                format!(
                    "target-family head {head} has incomplete member evidence for deployment `{target}`: {reason}"
                )
            }
        }
    }
}
