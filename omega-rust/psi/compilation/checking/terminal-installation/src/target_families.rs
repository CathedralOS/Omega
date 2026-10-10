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

/// An execution demand without a member at its exact deployment tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetMachineSelectionError {
    MissingMember {
        head: TargetMachineCallable,
        target: String,
    },
}

impl std::fmt::Display for TargetMachineSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
        .filter_map(|family| {
            let position = family
                .members
                .binary_search_by(|member| member.target.as_str().cmp(target))
                .ok()?;
            let member = &family.members[position];
            Some(TargetMachineSelection {
                head: family.canonical,
                callable: member.callable,
                supply: member.supply,
            })
        })
        .collect();
    AdmittedTargetMachineSelections {
        terminal_psi: artifact.identity(),
        target: target.to_owned(),
        selections,
    }
}
