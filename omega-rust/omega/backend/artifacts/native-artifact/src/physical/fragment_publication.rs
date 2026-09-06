//! Exact publication custody over the shared, independently validated object route.

use std::sync::Arc;

use image_emission::ObjectArtifact;
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use sha2::{Digest, Sha256};

use super::model::ValidatedOptimizedNativePhysicalEvidenceScope;

/// Immutable admitted object evidence. Equality deliberately covers the complete
/// object, including metadata not used by the current physical-child projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FragmentPublicationBinding {
    object: Arc<ObjectArtifact>,
    identity: [u8; 32],
}

impl FragmentPublicationBinding {
    pub(super) fn validate_object(&self, object: &ObjectArtifact) -> Result<(), &'static str> {
        if self.object.as_ref() != object {
            return Err(
                "fragment publication object differs from its independently admitted bytes or evidence",
            );
        }
        Ok(())
    }

    pub(super) const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_scope(
    final_plan: &abstract_operations::AbstractOperationPlan,
    terminal: terminal_psi::TerminalPsiIdentity,
    validation: optimization_core::OptimizedAbstractPlanProjectionIdentity,
    final_unit: optimization_core::OptimizationUnitIdentity,
    coverage: &boundary_applications::TerminalBoundaryApplicationCoverage,
    coverage_identity: [u8; 32],
    source: &StagedOptimizedRelocationFreeObjectContainer,
    object: &ObjectArtifact,
) -> Result<ValidatedOptimizedNativePhysicalEvidenceScope, &'static str> {
    image_emission::validate_function_fragment_object_artifact(source, object)
        .map_err(|_| "fragment publication object failed independent source replay")?;
    let fragments = source.source().source();
    let optimized = fragments.source().optimized_target().optimized();
    validate_final_plan(final_plan, optimized.plan(), terminal, object.psi())?;
    if optimized.validation().psi() != terminal
        || optimized.validation().identity() != validation
        || optimized.validation().final_unit() != final_unit
    {
        return Err("fragment publication is detached from the validated abstract projection");
    }
    let scope = super::projection::derive_validated_optimization_scope(
        final_plan,
        terminal,
        validation,
        final_unit,
        coverage,
        coverage_identity,
    )?;
    let mut digest = Sha256::new();
    digest.update(b"omega.native-artifact.fragment-publication-binding.sha256.v1\0");
    digest.update(source.custody().manifest().bytes());
    digest.update(source.custody().source_text_section_manifest().bytes());
    digest.update(source.custody().object().bytes());
    digest.update(source.custody().object_container().bytes());
    // Independent projection is a deterministic function of this exact source
    // custody. Its role domain binds that interpretation; retained full-object
    // equality below checks all derived fields without a parallel serializer.
    let publication = FragmentPublicationBinding {
        object: Arc::new(object.clone()),
        identity: digest.finalize().into(),
    };
    let mut digest = Sha256::new();
    digest.update(b"omega.native-artifact.validated-fragment-physical-scope.sha256.v1\0");
    digest.update(scope.identity());
    digest.update(publication.identity());
    Ok(
        super::model::validated_fragment_native_physical_evidence_scope(
            scope,
            publication,
            digest.finalize().into(),
        ),
    )
}

fn validate_final_plan(
    proposed: &abstract_operations::AbstractOperationPlan,
    retained: &abstract_operations::AbstractOperationPlan,
    terminal: terminal_psi::TerminalPsiIdentity,
    object_terminal: terminal_psi::TerminalPsiIdentity,
) -> Result<(), &'static str> {
    if proposed != retained || proposed.psi != terminal || object_terminal != terminal {
        return Err("fragment publication is detached from the validated abstract projection");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
