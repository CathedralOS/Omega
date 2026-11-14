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

#[derive(Debug, Clone)]
pub(super) struct FragmentPublicationBinding {
    source: Arc<StagedOptimizedRelocationFreeObjectContainer>,
    identity: [u8; 32],
}

impl FragmentPublicationBinding {
    pub(super) fn validate_object(&self, object: &ObjectArtifact) -> Result<(), &'static str> {
        if object
            .retained_fragment_source()
            .map(|source| source.custody())
            != Some(self.source.custody())
        {
            return Err(
                "fragment publication object differs from its independently admitted bytes or evidence",
            );
        }
        Ok(())
    }

    pub(super) const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub(super) fn replay_foreign_call_custody(
        &self,
        object: &ObjectArtifact,
    ) -> Result<(), &'static str> {
        let reconstructed = image_emission::derive_normalized_foreign_call_custody(&self.source)
            .map_err(|_| "fragment publication foreign call custody failed source replay")?;
        if reconstructed != object.foreign_calls() {
            return Err("native physical evidence foreign calls differ from fragment projection");
        }
        Ok(())
    }

    pub(super) fn relocation_free_object(&self) -> &object_file::RelocationFreeObjectPlan {
        self.source.object()
    }

    pub(super) fn text_section(&self) -> &machine_code::RelocationFreeTextSectionPlacement {
        self.source.source().text_section()
    }

    pub(super) fn selected_plan(&self) -> &selected_instructions::SelectedInstructionPlan {
        &self
            .source
            .source()
            .source()
            .source()
            .source()
            .program()
            .selected
    }
}

impl PartialEq for FragmentPublicationBinding {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity && self.source.custody() == other.source.custody()
    }
}

impl Eq for FragmentPublicationBinding {}

pub(crate) fn derive_scope(
    coverage: &boundary_applications::TerminalBoundaryApplicationCoverage,
    coverage_identity: [u8; 32],
    object: &ObjectArtifact,
) -> Result<ValidatedOptimizedNativePhysicalEvidenceScope, &'static str> {
    let source = object
        .retained_fragment_source()
        .ok_or("physical publication requires retained fragment source")?;
    let fragments = source.source().source().source();
    let optimized = fragments.source().optimized_target().optimized();
    let final_plan = optimized.plan();
    let terminal = optimized.projection_psi();
    let validation = optimized.projection();
    let final_unit = optimized.final_unit();
    validate_source_terminal(final_plan.psi, terminal, object.psi())?;
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
        source: Arc::clone(source),
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

fn validate_source_terminal(
    plan_terminal: terminal_psi::TerminalPsiIdentity,
    terminal: terminal_psi::TerminalPsiIdentity,
    object_terminal: terminal_psi::TerminalPsiIdentity,
) -> Result<(), &'static str> {
    if plan_terminal != terminal || object_terminal != terminal {
        return Err("fragment publication is detached from the validated abstract projection");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_source_terminal;

    #[test]
    fn construction_rejects_detached_source_and_object_terminal() {
        let psi = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([17; 32]),
        };
        assert!(validate_source_terminal(psi, psi, psi).is_ok());
        let mut changed_terminal = psi;
        changed_terminal.program_fingerprint =
            terminal_psi::SemanticFingerprint::from_bytes([18; 32]);
        assert!(validate_source_terminal(changed_terminal, psi, psi).is_err());
        assert!(validate_source_terminal(psi, changed_terminal, psi).is_err());
        assert!(validate_source_terminal(psi, psi, changed_terminal).is_err());
    }
}
