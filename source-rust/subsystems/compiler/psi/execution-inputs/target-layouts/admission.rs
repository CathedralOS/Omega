//! Bind the receiving target to an exact Psi program and check its selected layouts.
#![forbid(unsafe_code)]
mod target_context;
pub use layout_plans::LayoutTargetFacts;
use semantic_vocabulary::StructuralTypeId;
pub use target_context::{AssemblyReturnPlans, TerminalEntryError, TerminalTargetContext, terminal_target_context};
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
    target: layout_plans::LayoutTargetFacts,
    closures: Vec<TargetLayoutClosure>,
    records: Vec<StructuralTypeDeclaration>,
    lifecycles: Vec<terminal_psi::TerminalAccessLifecyclePlan>,
}
impl AdmittedTargetLayouts {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }
    pub fn target(&self) -> &str {
        &self.target.profile
    }
    /// The exact facts validated at admission, including when the
    /// artifact has no layout applications or lifecycle rows.
    pub fn target_facts(&self) -> &layout_plans::LayoutTargetFacts {
        &self.target
    }
    pub fn closures(&self) -> &[TargetLayoutClosure] {
        &self.closures
    }
    pub fn resolved_records(&self) -> &[StructuralTypeDeclaration] {
        &self.records
    }
    pub fn resolved_lifecycles(&self) -> &[terminal_psi::TerminalAccessLifecyclePlan] {
        &self.lifecycles
    }
    pub fn geometry<'a>(
        &'a self,
        artifact: impl Into<terminal_codec::TerminalExecutionView<'a>>,
    ) -> Result<TargetLayoutGeometry<'a>, TargetLayoutSelectionError> {
        let artifact = artifact.into();
        if artifact.identity() != self.terminal_psi {
            return Err(TargetLayoutSelectionError::ArtifactIdentityMismatch);
        }
        Ok(TargetLayoutGeometry {
            artifact,
            layouts: Some(self),
        })
    }
}

#[derive(Clone, Copy)]
pub struct TargetLayoutGeometry<'a> {
    artifact: terminal_codec::TerminalExecutionView<'a>,
    layouts: Option<&'a AdmittedTargetLayouts>,
}
impl<'a> TargetLayoutGeometry<'a> {
    /// Borrow portable geometry only; unresolved target layouts remain unresolved.
    pub fn unscoped(artifact: terminal_codec::TerminalExecutionView<'a>) -> Self {
        Self {
            artifact,
            layouts: None,
        }
    }
    fn resolved_record(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> {
        let records = &self.layouts?.records;
        let position = records.binary_search_by_key(&id, |row| row.id).ok()?;
        Some(&records[position])
    }
    pub fn declaration(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> {
        self.resolved_record(id)
            .or_else(|| self.artifact.module().structural_type(id))
    }
    pub fn declarations(&self) -> impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone + '_ {
        self.artifact
            .module()
            .types.structural_types
            .iter()
            .map(move |row| self.resolved_record(row.id).unwrap_or(row))
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
            Self::InvalidTargetInput(reason) => {
                write!(formatter, "layout target input: {reason}")
            }
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

pub fn admit_target_layouts<'artifact>(
    artifact: impl Into<terminal_codec::TerminalExecutionView<'artifact>>,
    selections: &TerminalTargetContext,
    target: &layout_plans::LayoutTargetFacts,
) -> Result<AdmittedTargetLayouts, TargetLayoutSelectionError> {
    let artifact = artifact.into();
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
    for application in &artifact.module().layouts.target_layout_applications {
        let stale = || TargetLayoutSelectionError::StaleApplication(application.id);
        let Some(realization) = application
            .realizations
            .binary_search_by(|row| row.target.as_str().cmp(selections.target()))
            .ok()
            .map(|position| &application.realizations[position])
        else {
            continue;
        };
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
    for plan in &artifact.module().access.access_lifecycle_plans {
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
        target: target.clone(),
        closures,
        records,
        lifecycles,
    })
}

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
