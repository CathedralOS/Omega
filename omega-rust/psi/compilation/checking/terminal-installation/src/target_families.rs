//! Exact deployment-tag selection over the canonical family catalog.
//!
//! These immutable rows record catalog membership, not checked conformance,
//! provider grants, or boundary issuance. Execution consumers must separately
//! admit the portable head contract and preserve authentic member result and
//! claim transport from the portable producer's checked member refinement.
//! Selection alone cannot open a checking guard or turn an opaque member into
//! an executable body. The complete canonical module remains with the artifact
//! owner.

use terminal_codec::TerminalArtifactView;
use terminal_psi::{TargetMachineCallable, TargetMachineSupply, TerminalPsiIdentity};

/// One exact catalog member selected for a family head.
/// Private fields preserve the catalog association; observing it establishes
/// neither refinement nor permission to execute the callable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetMachineSelection {
    head: TargetMachineCallable,
    callable: TargetMachineCallable,
    supply: TargetMachineSupply,
}

impl TargetMachineSelection {
    pub const fn head(&self) -> TargetMachineCallable {
        self.head
    }

    pub const fn callable(&self) -> TargetMachineCallable {
        self.callable
    }

    pub const fn supply(&self) -> TargetMachineSupply {
        self.supply
    }
}

/// Exact tagged selections bound to one canonical Terminal artifact.
/// Missing members impose no demand until an execution consumer requests them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedTargetMachineSelections {
    terminal_psi: TerminalPsiIdentity,
    target: String,
    selections: Vec<TargetMachineSelection>,
}

impl AdmittedTargetMachineSelections {
    /// Resolve a portable root in the same admitted artifact. This proves
    /// catalog membership, not entry backing, effects or provider authority.
    pub fn entry(
        &self,
        identity: TerminalPsiIdentity,
        module: &terminal_psi::TerminalModule,
        root: TargetMachineCallable,
    ) -> Result<semantic_vocabulary::MachineId, TargetMachineSelectionError> {
        if self.terminal_psi != identity {
            return Err(TargetMachineSelectionError::TerminalIdentityMismatch);
        }
        match root {
            TargetMachineCallable::Machine(machine) => {
                if module.machine(machine).is_none() {
                    return Err(TargetMachineSelectionError::NonExecutableEntry {
                        head: root,
                        target: self.target.clone(),
                    });
                }
                for family in &module.target_machine_families {
                    if family.members.iter().any(|member| member.callable == root)
                        && self.member(family.canonical)?.callable() != root
                    {
                        return Err(TargetMachineSelectionError::NonExecutableEntry {
                            head: family.canonical,
                            target: self.target.clone(),
                        });
                    }
                }
                Ok(machine)
            }
            head @ TargetMachineCallable::Boundary(_) => {
                let member = self.member(head)?;
                executable_entry(head, member, &self.target)
            }
        }
    }

    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    /// Borrow exact selected rows in canonical head order.
    pub fn selections(&self) -> &[TargetMachineSelection] {
        &self.selections
    }

    /// Demand this family's exact member for the selected deployment tag.
    /// Unknown tags and absent heads report the same unsatisfied member demand;
    /// neither selects a host-derived or representative implementation.
    pub fn member(
        &self,
        head: TargetMachineCallable,
    ) -> Result<&TargetMachineSelection, TargetMachineSelectionError> {
        self.selections
            .binary_search_by_key(&head, |selection| selection.head)
            .map(|position| &self.selections[position])
            .map_err(|_| TargetMachineSelectionError::MissingMember {
                head,
                target: self.target.clone(),
            })
    }
}

fn executable_entry(
    head: TargetMachineCallable,
    member: &TargetMachineSelection,
    target: &str,
) -> Result<semantic_vocabulary::MachineId, TargetMachineSelectionError> {
    match (member.callable(), member.supply()) {
        (
            TargetMachineCallable::Machine(machine),
            TargetMachineSupply::CheckedBody | TargetMachineSupply::Boundary,
        ) => Ok(machine),
        _ => Err(TargetMachineSelectionError::NonExecutableEntry {
            head,
            target: target.to_owned(),
        }),
    }
}

fn selected_member(
    family: &terminal_psi::TargetMachineFamily,
    target: &str,
) -> Option<TargetMachineSelection> {
    let member = borrowed_selected_member(family, target)?;
    Some(TargetMachineSelection {
        head: family.canonical,
        callable: member.callable,
        supply: member.supply,
    })
}

fn borrowed_selected_member<'family>(
    family: &'family terminal_psi::TargetMachineFamily,
    target: &str,
) -> Option<&'family terminal_psi::TargetMachineFamilyMember> {
    let position = family
        .members
        .binary_search_by(|member| member.target.as_str().cmp(target))
        .ok()?;
    family.members.get(position)
}

/// Borrow an independent family's exact deployment member without creating
/// identity-bound admission evidence. Ordinary callables and legacy canonical
/// members preserve their own exact identity and return `None`. A demanded
/// independent head with no member rejects; no host or representative fallback
/// participates. The consumer still checks the member's supply and declaration.
pub fn resolve_target_machine_member<'module>(
    module: &'module terminal_psi::TerminalModule,
    callable: TargetMachineCallable,
    target: &str,
) -> Result<Option<&'module terminal_psi::TargetMachineFamilyMember>, TargetMachineSelectionError> {
    let Some(family) = module
        .target_machine_families
        .iter()
        .find(|family| family.canonical == callable)
    else {
        return Ok(None);
    };
    if family
        .members
        .iter()
        .any(|member| member.callable == callable)
    {
        return Ok(None);
    }
    borrowed_selected_member(family, target)
        .map(Some)
        .ok_or_else(|| TargetMachineSelectionError::MissingMember {
            head: callable,
            target: target.to_owned(),
        })
}

/// An execution demand without a member at its exact deployment tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetMachineSelectionError {
    TerminalIdentityMismatch,
    NonExecutableEntry {
        head: TargetMachineCallable,
        target: String,
    },
    MissingMember {
        head: TargetMachineCallable,
        target: String,
    },
}

impl std::fmt::Display for TargetMachineSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TerminalIdentityMismatch => {
                formatter.write_str("entry selection names a different Terminal artifact")
            }
            Self::NonExecutableEntry { head, target } => write!(
                formatter,
                "target family {head:?} has no executable entry for deployment tag `{target}`",
            ),
            Self::MissingMember { head, target } => write!(
                formatter,
                "target family {head:?} has no member for deployment tag `{target}`"
            ),
        }
    }
}

impl std::error::Error for TargetMachineSelectionError {}

/// Select exact catalog members without changing canonical storage.
/// An unrecognized or unavailable tag is valid until a family is demanded.
/// Artifact shape admission supplies sorted, unique head and member catalogs;
/// this operation supplies no semantic proof, provider selection, or grant.
pub fn admit_target_machine_selections(
    artifact: TerminalArtifactView<'_>,
    target: &str,
) -> AdmittedTargetMachineSelections {
    let selections = artifact
        .module()
        .target_machine_families
        .iter()
        .filter_map(|family| selected_member(family, target))
        .collect();
    AdmittedTargetMachineSelections {
        terminal_psi: artifact.identity(),
        target: target.to_owned(),
        selections,
    }
}
