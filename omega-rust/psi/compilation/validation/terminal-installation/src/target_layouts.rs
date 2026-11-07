//! Exact layout closure over one immutable canonical artifact.
//!
//! The checked producer supplied every alternative and policy/schema binding.
//! This owner joins those rows to exact admitted family selections. It retains
//! the original artifact identity as the conservative complete dependency key;
//! no changed module receives that identity. Only selected record declarations
//! are copied into the overlay, and semantic access/custody still needs proof.
//! A retained policy can describe another target's private records. Catalog
//! retention does not demand that geometry here: absent members leave their
//! pending record markers intact. Physical storage, byte projection and ABI
//! consumers must resolve those markers before using geometry; ordinary packing
//! is never a replacement for an unavailable member. Pure logical field values
//! need no physical geometry, so this is not a blanket interpreter type gate.
use crate::AdmittedTargetMachineSelections;
use semantic_vocabulary::StructuralTypeId;
use terminal_codec::TerminalArtifactView;
use terminal_psi::{
    LayoutApplicationId, StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalIdentityIndex, TerminalPsiIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutClosure {
    application: LayoutApplicationId,
    record: StructuralTypeId,
    policy: TerminalIdentityIndex,
}
impl TargetLayoutClosure {
    pub const fn application(&self) -> LayoutApplicationId {
        self.application
    }
    pub const fn record(&self) -> StructuralTypeId {
        self.record
    }
    pub const fn policy(&self) -> TerminalIdentityIndex {
        self.policy
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedTargetLayouts {
    terminal_psi: TerminalPsiIdentity,
    target: String,
    closures: Vec<TargetLayoutClosure>,
    records: Vec<StructuralTypeDeclaration>,
    lifecycles: Vec<terminal_psi::TerminalAccessLifecyclePlan>,
}
impl AdmittedTargetLayouts {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn closures(&self) -> &[TargetLayoutClosure] {
        &self.closures
    }
    /// Only replacement rows, in structural identity order. Consumers retain
    /// the canonical module and this receipt separately; these are not a module.
    pub fn resolved_records(&self) -> &[StructuralTypeDeclaration] {
        &self.records
    }
    /// Whole-value operations consume this target's checked placement, not the
    /// first interpretation retained in the portable artifact.
    pub fn resolved_lifecycles(&self) -> &[terminal_psi::TerminalAccessLifecyclePlan] {
        &self.lifecycles
    }
    pub fn geometry<'a>(
        &'a self,
        artifact: TerminalArtifactView<'a>,
    ) -> Result<TargetLayoutGeometry<'a>, TargetLayoutSelectionError> {
        if artifact.identity() != self.terminal_psi {
            return Err(TargetLayoutSelectionError::ArtifactIdentityMismatch);
        }
        Ok(TargetLayoutGeometry {
            artifact,
            layouts: self,
        })
    }
}

#[derive(Clone, Copy)]
pub struct TargetLayoutGeometry<'a> {
    artifact: TerminalArtifactView<'a>,
    layouts: &'a AdmittedTargetLayouts,
}
impl<'a> TargetLayoutGeometry<'a> {
    pub fn declaration(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> {
        self.layouts
            .records
            .binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|position| &self.layouts.records[position])
            .or_else(|| self.artifact.module().structural_type(id))
    }
    pub fn declarations(&self) -> impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone + '_ {
        self.artifact
            .module()
            .structural_types
            .iter()
            .map(move |row| {
                self.declaration(row.id)
                    .expect("canonical declaration is retained")
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetLayoutSelectionError {
    ArtifactIdentityMismatch,
    MissingMember {
        application: LayoutApplicationId,
        target: String,
    },
    StaleApplication(LayoutApplicationId),
    InvalidTargetInput(String),
    InvalidLifecycle {
        boundary: semantic_vocabulary::BoundaryMachineId,
        reason: &'static str,
    },
}
impl std::fmt::Display for TargetLayoutSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLifecycle { boundary, reason } => {
                write!(formatter, "lifecycle {boundary:?}: {reason}")
            }
            Self::InvalidTargetInput(reason) => write!(formatter, "layout target input: {reason}"),
            Self::ArtifactIdentityMismatch => write!(
                formatter,
                "layout selections belong to a different Terminal artifact"
            ),
            Self::MissingMember {
                application,
                target,
            } => write!(
                formatter,
                "layout application {} has no member for target `{target}`",
                application.0
            ),
            Self::StaleApplication(application) => write!(
                formatter,
                "layout application {} has stale geometry or family custody",
                application.0
            ),
        }
    }
}
impl std::error::Error for TargetLayoutSelectionError {}

pub fn admit_target_layouts(
    artifact: TerminalArtifactView<'_>,
    selections: &AdmittedTargetMachineSelections,
    target: &layout_plans::LayoutTargetFacts,
) -> Result<AdmittedTargetLayouts, TargetLayoutSelectionError> {
    if artifact.identity() != selections.terminal_psi() {
        return Err(TargetLayoutSelectionError::ArtifactIdentityMismatch);
    }
    target
        .validate()
        .map_err(|reason| TargetLayoutSelectionError::InvalidTargetInput(reason.into()))?;
    if target.profile != selections.target() {
        return Err(TargetLayoutSelectionError::InvalidTargetInput(
            "receiving catalog and selected target disagree".into(),
        ));
    }
    let mut records = Vec::new();
    let mut closures = Vec::new();
    for application in &artifact.module().target_layout_applications {
        let stale = || TargetLayoutSelectionError::StaleApplication(application.id);
        let Some(realization) = application
            .realizations
            .binary_search_by(|row| row.target.as_str().cmp(selections.target()))
            .ok()
            .map(|position| &application.realizations[position])
        else {
            // Catalog custody is not a physical application. The original
            // pending record marker remains; a physical consumer cannot
            // obtain ordinary packing or a geometry receipt from absence.
            continue;
        };
        // Internal roster consistency is checked by the codec. A receiving
        // target must independently rejoin its real catalog facts: self-
        // consistent forged pointer geometry is not a deployment receipt.
        if realization.pointer_size != target.pointer_size
            || realization.pointer_alignment != target.pointer_alignment
            || realization.case_tag_size != target.case_tag_size
            || realization.case_tag_alignment != target.case_tag_alignment
            || realization.little_endian != target.little_endian
        {
            return Err(stale());
        }
        let mut record = artifact
            .module()
            .structural_type(application.record)
            .ok_or_else(stale)?
            .clone();
        let StructuralTypeShape::Record { fields, storage } = &mut record.shape else {
            return Err(stale());
        };
        if *storage != StructuralRecordStorage::TargetLayout(application.id) {
            return Err(stale());
        }
        *storage = StructuralRecordStorage::Authored(realization.storage);
        for field in fields
            .iter_mut()
            .filter(|field| !field.relevance.is_erased())
        {
            let row = realization
                .fields
                .binary_search_by_key(&field.id, |row| row.field)
                .ok()
                .map(|position| &realization.fields[position])
                .ok_or_else(stale)?;
            field.storage = Some(row.storage.clone());
        }
        closures.push(TargetLayoutClosure {
            application: application.id,
            record: application.record,
            policy: application.policy_identity,
        });
        records.push(record);
    }
    records.sort_unstable_by_key(|record| record.id);
    let mut lifecycles = Vec::new();
    for plan in &artifact.module().access_lifecycle_plans {
        if let Some(input) = &plan.target
            && !lifecycle_target_matches(input, target).map_err(|reason| {
                TargetLayoutSelectionError::InvalidLifecycle {
                    boundary: plan.boundary,
                    reason,
                }
            })?
        {
            continue;
        }
        if lifecycles
            .last()
            .is_some_and(|previous: &terminal_psi::TerminalAccessLifecyclePlan| {
                previous.boundary == plan.boundary
            })
        {
            return Err(TargetLayoutSelectionError::InvalidLifecycle {
                boundary: plan.boundary,
                reason: "multiple placements apply to the selected target",
            });
        }
        lifecycles.push(plan.clone());
    }
    Ok(AdmittedTargetLayouts {
        terminal_psi: artifact.identity(),
        target: selections.target().to_owned(),
        closures,
        records,
        lifecycles,
    })
}

/// Matching a profile name is insufficient: imported geometry must also agree
/// with the receiving catalog, not just with its own serialized assertions.
fn lifecycle_target_matches(
    input: &terminal_psi::TargetLayoutInput,
    target: &layout_plans::LayoutTargetFacts,
) -> Result<bool, &'static str> {
    if input.target != target.profile {
        return Ok(false);
    }
    if input.pointer_size != target.pointer_size
        || input.pointer_alignment != target.pointer_alignment
    {
        return Err("pointer geometry differs from the receiving target");
    }
    if input.case_tag_size != target.case_tag_size
        || input.case_tag_alignment != target.case_tag_alignment
    {
        return Err("case tag geometry differs from the receiving target");
    }
    if input.little_endian != target.little_endian {
        return Err("byte order differs from the receiving target");
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_admission_checks_the_selected_catalog_not_just_its_profile_name() {
        let target = layout_plans::LayoutTargetFacts {
            profile: "linux_x86_64".into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        };
        let valid = terminal_psi::TargetLayoutInput {
            target: target.profile.clone(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        };
        assert_eq!(lifecycle_target_matches(&valid, &target), Ok(true));
        let mut other = valid.clone();
        other.target = "linux_arm64".into();
        assert_eq!(lifecycle_target_matches(&other, &target), Ok(false));
        let mut altered = valid.clone();
        altered.pointer_size = 4;
        assert_eq!(
            lifecycle_target_matches(&altered, &target),
            Err("pointer geometry differs from the receiving target")
        );
        let mut altered = valid.clone();
        altered.pointer_alignment = 4;
        assert!(lifecycle_target_matches(&altered, &target).is_err());
        let mut altered = valid.clone();
        altered.case_tag_size = 8;
        assert_eq!(
            lifecycle_target_matches(&altered, &target),
            Err("case tag geometry differs from the receiving target")
        );
        let mut altered = valid.clone();
        altered.case_tag_alignment = 2;
        assert!(lifecycle_target_matches(&altered, &target).is_err());
        let mut altered = valid;
        altered.little_endian = false;
        assert_eq!(
            lifecycle_target_matches(&altered, &target),
            Err("byte order differs from the receiving target")
        );
    }
}
