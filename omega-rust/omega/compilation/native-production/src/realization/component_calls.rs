//! Exact physical component call inputs borrowed from separately emitted images.
//!
//! A producer's checked body and entry bridge determine the actual same-stack
//! contribution. Compact execution reports describe this retained join; they
//! cannot select an image or grant an installed address/era. The complete
//! image remains borrowed until native realization consumes the settlement.

use diagnostics::Diagnostic;
use installation_evidence::ProviderExecutionEvidence;
use optimization_core::digest_encoder::DomainDigestEncoder;
use terminal_psi_to_abstract_operations::{
    AdmittedNativeComponentCall,
    provider_admission::{NativeBoundaryRealization, NativeProviderSettlement},
};

use super::component_exports::ComponentExportImage;

pub struct ComponentNativeCall<'input> {
    image: &'input ComponentExportImage<'input>,
    call: &'input AdmittedNativeComponentCall<'input>,
    plan: &'input effects::provider_plan::ProviderPlan,
    locator: target::NormalizedComponentLocator,
    same_stack: task_plans::AdmittedSameStackContribution,
    commitment: [u8; 32],
}

impl std::fmt::Debug for ComponentNativeCall<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ComponentNativeCall")
            .field("selection", self.call.selection())
            .field("locator", &self.locator)
            .field("same_stack", &self.same_stack)
            .field("commitment", &self.commitment)
            .finish()
    }
}

impl<'input> ComponentNativeCall<'input> {
    pub fn join(
        image: &'input ComponentExportImage<'input>,
        call: &'input AdmittedNativeComponentCall<'input>,
        plan: &'input effects::provider_plan::ProviderPlan,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, Vec<Diagnostic>> {
        let invalid = |message: &str| vec![Diagnostic::error(message)];
        let application = image.calling_application();
        let selection = call.selection();
        let profile = target::TargetProfile::from_canonical_target_name(&plan.target)
            .map_err(|error| vec![error])?;
        if image.component().artifact() != call.component().artifact()
            || image.component().subject() != selection.provider()
            || image.candidate().candidate != selection.candidate()
            || image.candidate().boundary != selection.provider_boundary()
            || image.authority().subject() != selection.provider()
            || image.authority().entry() != selection.candidate()
            || image.authority().review().terminal_artifact_identity()
                != *call.component().artifact().manifest().identity().as_bytes()
            || image.authority().review().target() != application.target
            || application != call.application()
            || call.plan_digest() != &plan.identity_digest()
            || application.target != profile.native_target()
            || application.recomputed_commitment().ok() != Some(application.commitment)
            || image.image().slot_commitment()
                != &super::component_exports::component_export_slot(
                    selection.provider(),
                    &application.commitment,
                )
        {
            return Err(invalid(
                "component native call does not retain its exact checked export and complete calling application",
            ));
        }
        let demand = image.image().stack_demand().map_err(|error| vec![error])?;
        if demand.psi() != selection.provider()
            || demand.entry() != selection.candidate()
            || demand.target() != application.target
        {
            return Err(invalid(
                "component native stack evidence belongs to another artifact, entry or target",
            ));
        }
        let bytes = demand.ceiling_bytes();
        let locator = target::NormalizedComponentLocator::for_image(
            profile,
            image.image().image().contents(),
            *image.image().slot_commitment(),
        )
        .map_err(invalid)?;
        let mut digest = DomainDigestEncoder::new(b"omega.component-native-call.v1");
        digest.raw(locator.identity_digest());
        digest.raw(&image.authority().review().identity());
        digest.raw(plan.identity_digest().as_bytes());
        digest.raw(
            &selection
                .receiver()
                .semantic()
                .vocabulary_marker
                .get()
                .to_le_bytes(),
        );
        digest.raw(
            selection
                .receiver()
                .semantic()
                .program_fingerprint
                .as_bytes(),
        );
        digest.u64(u64::from(selection.receiving_boundary().get()));
        digest.raw(&application.commitment);
        digest.u64(bytes);
        digest.u32(demand.stack_alignment());
        let commitment = digest.finish();
        let report = compact_report(&commitment);
        let receipt =
            task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(report)
                .map_err(|error| invalid(&format!("component stack receipt: {error}")))?;
        let requirement = identities
            .index_of(&application.requirement_identity)
            .ok_or_else(|| {
                invalid(
                    "component requirement is absent from the receiver selection identity table",
                )
            })?;
        let plan_commitment = task_plans::SameStackProviderPlanCommitment::from_digest(
            *plan.identity_digest().as_bytes(),
        );
        let same_stack = task_plans::admit_same_stack_contribution(
            task_plans::SameStackContributionAdmissionCandidate {
                provider_plan_report_identity: plan.report_fingerprint(),
                provider_plan_commitment: plan_commitment,
                requirement_identity: requirement,
                receipt,
                bytes,
                alignment: u64::from(demand.stack_alignment()),
            },
            plan.report_fingerprint(),
            plan_commitment,
            requirement,
        )
        .map_err(|error| invalid(&format!("component stack admission: {error}")))?;
        Ok(Self {
            image,
            call,
            plan,
            locator,
            same_stack,
            commitment,
        })
    }

    pub const fn call(&self) -> &AdmittedNativeComponentCall<'input> {
        self.call
    }

    pub fn settlement(&self) -> NativeProviderSettlement<'_> {
        NativeProviderSettlement {
            provider_execution: self,
            provider_plan: self.plan,
            realization: NativeBoundaryRealization::Component {
                call: self.call,
                authority: self.image.authority(),
                locator: &self.locator,
                same_stack: &self.same_stack,
            },
        }
    }
}

// Report projections are deliberately non-authoritative. Native realization
// also consumes the retained full plan, cross-artifact call, image and strong
// commitment; collisions cannot substitute those inputs.
fn compact_report(digest: &[u8; 32]) -> u64 {
    u64::from_le_bytes(digest[..8].try_into().expect("fixed digest width")).max(1)
}
impl ProviderExecutionEvidence for ComponentNativeCall<'_> {
    fn requirement_identity(&self) -> &str {
        &self.call.application().requirement_identity
    }
    fn provider_plan_report_identity(&self) -> u64 {
        self.plan.report_fingerprint()
    }
    fn provider_execution_report_identity(&self) -> u64 {
        compact_report(&self.commitment)
    }
    fn provider_execution_report_fingerprint(&self) -> u64 {
        compact_report(&self.commitment)
    }
    fn normalized_root_report_identity(&self) -> u64 {
        self.plan.report_fingerprint()
    }
    fn boundary_contract_report_fingerprint(&self) -> u64 {
        self.call.application().report_fingerprint
    }
}
