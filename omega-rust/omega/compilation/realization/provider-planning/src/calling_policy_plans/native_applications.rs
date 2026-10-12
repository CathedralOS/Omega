//! Publish completed calling applications independently of policy selection.
//! Explicit and native compatibility producers retain their own authority;
//! this owner transports validated plans and source-free value geometry.

use super::MaterializedBoundarySignature;
use calling_conventions::ValidatedBoundaryEntryPlan;

/// Publish completed geometry and validated plan from either policy producer.
pub(crate) fn publish_native_application(
    signature: &MaterializedBoundarySignature,
    public_requirement_identity: String,
    validated_plan: ValidatedBoundaryEntryPlan,
    report_fingerprint: u64,
    commitment: [u8; 32],
    callback_context_closed: bool,
) -> calling_conventions::BoundaryCallingApplication {
    calling_conventions::BoundaryCallingApplication {
        requirement_identity: signature.owner_requirement_identity().to_owned(),
        public_requirement_identity,
        target: signature.native_target(),
        report_fingerprint,
        commitment,
        plan: validated_plan,
        signature: calling_conventions::BoundarySignatureGeometry {
            shapes: signature.shapes().to_vec(),
            fields: signature.fields().to_vec(),
            parameters: signature.parameters().to_vec(),
            result: signature.result(),
        },
        opaque_applications: signature
            .opaque_representation_uses()
            .iter()
            .map(
                |representation_use| calling_conventions::BoundaryOpaqueApplication {
                    shape_root: representation_use.shape_root(),
                    application_report_fingerprint: representation_use
                        .application_report_fingerprint(),
                    selected_application_commitment: representation_use
                        .selected_application_commitment(),
                },
            )
            .collect(),
        callback_context_closed,
    }
}
