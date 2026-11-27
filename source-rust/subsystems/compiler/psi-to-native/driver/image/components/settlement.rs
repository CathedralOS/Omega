//! Join the actual shared image and its finite stack demand to the receiving provider occurrence.
use diagnostics::Diagnostic;
use provider_admission::{
    NativeBoundaryRealization, NativeProviderSettlement, ProviderExecutionEvidence,
};
use provider_stack_contribution::{
    SameStackContributionAdmissionReceiptId, SameStackProviderPlanCommitment,
};
use std::sync::Arc;
use verified_optimization_input::AdmittedNativeComponentCall;
use wire::digest_encoder::DomainDigestEncoder;

#[derive(Debug)]
pub(crate) struct ComponentCall<'input> {
    pub product: Arc<native_artifact::NativeComponentProduct>,
    call: AdmittedNativeComponentCall<'input>,
    plan: &'input build_providers::plans::ProviderPlan,
    same_stack: provider_stack_contribution::AdmittedSameStackContribution,
    commitment: [u8; 32],
}

impl<'input> ComponentCall<'input> {
    pub(super) fn join(
        product: Arc<native_artifact::NativeComponentProduct>,
        call: AdmittedNativeComponentCall<'input>,
        plan: &'input build_providers::plans::ProviderPlan,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, Vec<Diagnostic>> {
        let invalid = |message: &str| vec![Diagnostic::error(message)];
        let application = call.application();
        let selection = call.selection();
        if product.selection() != selection
            || product.application() != application
            || *call.plan_digest() != plan.identity_digest()
        {
            return Err(invalid(
                "component image belongs to a different imported callable",
            ));
        }
        let demand = product.image().stack_demand().map_err(|why| vec![why])?;
        let bytes = demand
            .ceiling_bytes()
            .ok_or_else(|| invalid("component calls require a finite supplier stack demand"))?;
        let mut digest = DomainDigestEncoder::new(b"omega.component-native-call.v1");
        digest.raw(product.locator().identity_digest());
        digest.raw(&product.authority().review().identity());
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
        digest.u64(selection.receiving_boundary().get());
        digest.raw(&application.commitment);
        digest.u64(bytes);
        digest.u32(demand.stack_alignment());
        let commitment = digest.finish();
        let report = compact_report(&commitment);
        let receipt = SameStackContributionAdmissionReceiptId::from_normalized_identity(report)
            .map_err(|error| invalid(&format!("component stack receipt: {error}")))?;
        let requirement = identities
            .index_of(&application.requirement_identity)
            .ok_or_else(|| {
                invalid(
                    "component requirement is absent from the receiver selection identity table",
                )
            })?;
        let plan_commitment =
            SameStackProviderPlanCommitment::from_digest(*plan.identity_digest().as_bytes());
        let same_stack = provider_stack_contribution::admit_same_stack_contribution(
            provider_stack_contribution::SameStackContributionAdmissionCandidate {
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
        product.validate_same_stack(&same_stack).map_err(invalid)?;
        Ok(Self {
            product,
            call,
            plan,
            same_stack,
            commitment,
        })
    }

    pub(crate) fn settlement(&self) -> NativeProviderSettlement<'_> {
        NativeProviderSettlement {
            provider_execution: self,
            provider_plan: self.plan,
            realization: NativeBoundaryRealization::Component {
                call: self.call.facts(),
                authority: self.product.authority(),
                locator: self.product.locator(),
                same_stack: &self.same_stack,
            },
        }
    }
}

fn compact_report(digest: &[u8; 32]) -> u64 {
    u64::from_le_bytes(digest[..8].try_into().expect("fixed digest width")).max(1)
}

impl ProviderExecutionEvidence for ComponentCall<'_> {
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
