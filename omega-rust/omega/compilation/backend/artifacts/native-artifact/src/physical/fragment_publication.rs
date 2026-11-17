/*
We bind fragment-published object custody to a validated optimized physical
evidence scope, retaining the relocation-free object plan, placed text section,
and current call provenance/transport records so derivation can rejoin unresolved
import fields and instruction spans directly.
*/

use std::sync::Arc;

use image_emission::ObjectArtifact;

use super::ValidatedOptimizedNativePhysicalEvidenceScope;

#[derive(Debug, Clone)]
pub(super) struct FragmentPublicationBinding {
    publication: object_file::ObjectPublication,
    identity: [u8; 32],
    call_origins: Arc<machine_code::call_origins::PhysicalCallOrigins>,
    foreign_call_plans: Arc<machine_code::foreign_call_plans::PhysicalForeignCallPlans>,
}

impl FragmentPublicationBinding {
    pub(super) fn validate_object(&self, object: &ObjectArtifact) -> Result<(), &'static str> {
        if object.publication().custody() != self.publication.custody()
            || object.call_origins() != &self.call_origins
            || object.foreign_call_plans() != &self.foreign_call_plans
        {
            return Err(
                "fragment publication object differs from its independently admitted bytes or evidence",
            );
        }
        Ok(())
    }

    pub(super) fn foreign_call_plans(
        &self,
    ) -> &machine_code::foreign_call_plans::PhysicalForeignCallPlans {
        &self.foreign_call_plans
    }

    pub(super) fn call_origins(&self) -> &machine_code::call_origins::PhysicalCallOrigins {
        &self.call_origins
    }

    pub(super) const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub(super) fn replay_foreign_call_custody(
        &self,
        object: &ObjectArtifact,
    ) -> Result<(), &'static str> {
        let reconstructed = self
            .foreign_call_plans
            .calls
            .iter()
            .map(|plan| image_emission::place_foreign_call(plan, self.text_section()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "fragment publication foreign call custody failed physical span checks")?;
        if reconstructed != object.foreign_calls() {
            return Err("native physical evidence foreign calls differ from fragment projection");
        }
        Ok(())
    }

    pub(super) fn relocation_free_object(&self) -> &object_file::RelocationFreeObjectPlan {
        self.publication.object()
    }

    pub(super) fn text_section(&self) -> &machine_code::RelocationFreeTextSectionPlacement {
        self.publication.text()
    }
}

impl PartialEq for FragmentPublicationBinding {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.publication.custody() == other.publication.custody()
            && self.call_origins == other.call_origins
            && self.foreign_call_plans == other.foreign_call_plans
    }
}

impl Eq for FragmentPublicationBinding {}

pub(crate) fn derive_scope(
    coverage: &boundary_applications::TerminalBoundaryApplicationCoverage,
    coverage_identity: [u8; 32],
    object: &ObjectArtifact,
) -> Result<ValidatedOptimizedNativePhysicalEvidenceScope, &'static str> {
    let publication = object.publication();
    if object.call_origins().psi != publication.text().psi
        || object.foreign_call_plans().psi != publication.text().psi
        || object.call_origins().target != publication.text().target
        || object.foreign_call_plans().target != publication.text().target
        || object.call_origins().physical_identity != object.foreign_call_plans().physical_identity
    {
        return Err("physical call plans are detached from the published program and target");
    }
    let facts = object.emission_facts();
    let final_plan = &facts.program;
    let terminal = facts.projection.terminal;
    let validation = facts.projection.identity;
    let final_unit = facts.projection.final_unit;
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
    digest.raw(&publication.custody().manifest().bytes());
    digest.raw(&publication.custody().source_text_section_manifest().bytes());
    digest.raw(&publication.custody().object().bytes());
    digest.raw(&publication.custody().object_container().bytes());
    let publication = FragmentPublicationBinding {
        publication: publication.clone(),
        identity: digest.finish(),
        call_origins: Arc::clone(object.call_origins()),
        foreign_call_plans: Arc::clone(object.foreign_call_plans()),
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
