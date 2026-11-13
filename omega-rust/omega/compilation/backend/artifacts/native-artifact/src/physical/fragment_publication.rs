/*
We bind fragment-published object custody to a validated optimized physical
evidence scope, retaining the relocation-free object plan, placed text section,
and selected instruction plan so physical derivation can rejoin unresolved
import fields and instruction spans directly.
*/

use std::sync::Arc;

use image_emission::ObjectArtifact;
use object_file::StagedOptimizedRelocationFreeObjectContainer;

use super::ValidatedOptimizedNativePhysicalEvidenceScope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FragmentPublicationBinding {
    object: Arc<ObjectArtifact>,
    foreign_call_custody: Vec<image_emission::ObjectForeignCall>,
    relocation_free_object: Arc<object_file::RelocationFreeObjectPlan>,
    text_section: Arc<machine_code::RelocationFreeTextSectionPlacement>,
    selected: Arc<selected_instructions::SelectedInstructionPlan>,
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

    #[expect(
        dead_code,
        reason = "unwired, not superseded: native-artifact physical/mod.rs and TASKS_OPTIMIZER.md PIPELINE-OWNER-CONSOLIDATION"
    )]
    pub(super) const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub(super) fn foreign_call_custody(&self) -> &[image_emission::ObjectForeignCall] {
        &self.foreign_call_custody
    }

    pub(super) fn relocation_free_object(&self) -> &object_file::RelocationFreeObjectPlan {
        &self.relocation_free_object
    }

    pub(super) fn text_section(&self) -> &machine_code::RelocationFreeTextSectionPlacement {
        &self.text_section
    }

    pub(super) fn selected_plan(&self) -> &selected_instructions::SelectedInstructionPlan {
        &self.selected
    }
}

#[allow(clippy::too_many_arguments)]
#[expect(
    dead_code,
    reason = "unwired, not superseded: native-artifact physical/mod.rs and TASKS_OPTIMIZER.md PIPELINE-OWNER-CONSOLIDATION"
)]
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
    let foreign_call_custody = image_emission::derive_normalized_foreign_call_custody(source)
        .map_err(|_| "fragment publication foreign call custody failed source replay")?;
    let relocation_free_object = source.shared_object();
    if relocation_free_object.identity != source.custody().object() {
        return Err("fragment publication retained a detached relocation-free object");
    }
    let text_section = source.source().shared_text_section();
    if text_section.recomputed_identity() != source.source().custody().text_section() {
        return Err("fragment publication retained a detached placed text section");
    }
    let selected = Arc::clone(
        &source
            .source()
            .source()
            .source()
            .source()
            .program()
            .selected,
    );
    let fragments = source.source().source().source();
    let optimized = fragments.source().optimized_target().optimized();
    validate_final_plan(final_plan, optimized.plan(), terminal, object.psi())?;
    if optimized.projection_psi() != terminal
        || optimized.projection() != validation
        || optimized.final_unit() != final_unit
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
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-artifact.fragment-publication-binding.sha256.v1\0",
    );
    digest.raw(&source.custody().manifest().bytes());
    digest.raw(&source.custody().source_text_section_manifest().bytes());
    digest.raw(&source.custody().object().bytes());
    digest.raw(&source.custody().object_container().bytes());
    let publication = FragmentPublicationBinding {
        object: Arc::new(object.clone()),
        foreign_call_custody,
        relocation_free_object,
        text_section,
        selected,
        identity: digest.finish(),
    };
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-artifact.validated-fragment-physical-scope.sha256.v1\0",
    );
    digest.raw(scope.identity());
    digest.raw(publication.identity());
    Ok(super::validated_fragment_native_physical_evidence_scope(
        scope,
        publication,
        digest.finish(),
    ))
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "unwired, not superseded: native-artifact physical/mod.rs and TASKS_OPTIMIZER.md PIPELINE-OWNER-CONSOLIDATION"
    )
)]
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
mod tests {
    use super::validate_final_plan;

    #[test]
    fn construction_rejects_detached_final_plan_and_object_terminal() {
        let psi = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([17; 32]),
        };
        let retained = abstract_operations::AbstractOperationPlan {
            psi,
            entry: semantic_vocabulary::MachineId::new(1).unwrap(),
            identities: Default::default(),
            structural_types: Vec::new().into(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            dynamic_dispatch: Default::default(),
            callables: Default::default(),
            functions: Vec::new(),
        };
        assert!(validate_final_plan(&retained, &retained, psi, psi).is_ok());
        let mut changed = retained.clone();
        changed.entry = semantic_vocabulary::MachineId::new(2).unwrap();
        assert!(validate_final_plan(&changed, &retained, psi, psi).is_err());
        let mut changed_terminal = psi;
        changed_terminal.program_fingerprint =
            terminal_psi::SemanticFingerprint::from_bytes([18; 32]);
        assert!(validate_final_plan(&retained, &retained, psi, changed_terminal).is_err());
    }
}
