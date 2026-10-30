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
use crate::TerminalTargetContext;
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
}
impl std::fmt::Display for TargetLayoutSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
    selections: &TerminalTargetContext,
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
    Ok(AdmittedTargetLayouts {
        terminal_psi: artifact.identity(),
        target: selections.target().to_owned(),
        closures,
        records,
    })
}
