//! Shared portable-provision frame: a failed assertion inside one fixture
//! provision becomes a red fixture result carrying its message, never an
//! aborted corpus binary before its other authored controls run.
//!
//! The frame also owns the two projections every provisioned fixture used to
//! re-state for itself: the artifact's own admitted provider catalog with the
//! runtime supplies it grants, and the storage-geometry invocation inputs its
//! single owned extent parameter declares. One owner keeps a provision from
//! inventing a different roster or geometry than the artifact publishes.

// Shared across test targets through `#[path]`; each target uses the subset
// its fixtures need.
#![allow(dead_code)]

use semantic_vocabulary::IntegerValue;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_interpreter::{
    ProviderInstallationSelection, TerminalProgram, TerminalRuntimeSupplies,
    TerminalRuntimeSupplyRequest, TerminalScalarValue, TerminalStructuralScalarFieldValue,
    TerminalStructuralValue, admit_provider_installation,
};
use terminal_psi::StructuralTypeShape;

/// Run one provision body, converting a caught panic into the provision's
/// error text; `fallback` names the provision when the payload has no message.
pub(super) fn result(body: impl FnOnce(), fallback: &str) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).map_err(|failure| {
        failure
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                failure
                    .downcast_ref::<&str>()
                    .map(|message| (*message).to_owned())
            })
            .unwrap_or_else(|| fallback.to_owned())
    })
}

/// Admit the artifact's published provider catalog and answer its supplies.
pub(super) fn admitted_supplies(
    artifact: &CanonicalTerminalArtifact,
    program: &TerminalProgram<'_>,
) -> TerminalRuntimeSupplies {
    let module = program.module();
    let selections = module
        .provider_candidates
        .iter()
        .map(|candidate| ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: module
                .identities
                .text(candidate.provider_identity)
                .unwrap()
                .to_owned(),
            provider_declaration: module
                .identities
                .text(candidate.provider_declaration.identity())
                .unwrap()
                .to_owned(),
            candidate: candidate.candidate,
        })
        .collect::<Vec<_>>();
    let installed = admit_provider_installation(artifact.view(), &selections, &[]).unwrap();
    program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            x86_flags: None,
            memory_ordering: None,
            entry: None,
            installation: Some(&installed),
            host_imports: &[],
            target_context: None,
            target_layouts: None,
        })
        .unwrap()
}

/// The invocation shapes one provisioned fixture's single owned extent
/// parameter reads: the whole-value argument and the plan's `base` and length
/// storage coordinates as its two scalar fields, valued at `length_value`.
pub(super) fn storage_inputs(
    program: &TerminalProgram<'_>,
    plan: &terminal_psi::TerminalPlacementLifecyclePlan,
    length_value: u64,
) -> (
    Vec<TerminalStructuralValue>,
    Vec<TerminalStructuralScalarFieldValue>,
) {
    let entry = program.entry().unwrap();
    let [parameter] = entry.structural_parameters else {
        panic!("one owned extent input");
    };
    let StructuralTypeShape::Record { fields, .. } = &program
        .structural_type(parameter.structural_type)
        .unwrap()
        .shape
    else {
        panic!("extent record");
    };
    let values = vec![TerminalStructuralValue {
        opaque_identity: 100,
        structural_type: parameter.structural_type,
        qualifications: parameter.qualifications.clone(),
        projected_qualifications: parameter.projected_qualifications.clone(),
        path: Vec::new(),
    }];
    let scalars = fields
        .iter()
        .map(|field| {
            let coordinate = if field.id == plan.storage.base {
                4096_u64
            } else {
                assert_eq!(field.id, plan.storage.length);
                length_value
            };
            TerminalStructuralScalarFieldValue {
                argument_index: 0,
                path: Vec::new(),
                field: field.id,
                value: TerminalScalarValue::Integer {
                    scalar_type: field
                        .field_type
                        .scalar_type()
                        .unwrap()
                        .as_integer()
                        .unwrap(),
                    value: IntegerValue::Unsigned(coordinate.into()),
                },
            }
        })
        .collect();
    (values, scalars)
}
