//! Verified components for the build's `Independent` provider selections.
//!
//! A dependency compiled as its own component publishes a canonical
//! description; the package inputs attach those bytes with the component
//! subject that compilation observed. This is the consumer side of the
//! verified-description contract
//! (wiki/spec/build/component_publication.md#verified-component-descriptions):
//! every attached description is independently re-verified here under the
//! build's own admission profile, and only the resulting `VerifiedComponent`
//! values reach provider planning's component-closure join. Nothing is read
//! out of the description to decide admission, a rejected description is a
//! settlement diagnostic rather than a dropped input, and an `Independent`
//! selection left without a verified component still rejects at the join
//! instead of falling back to a fused edge.

use std::collections::BTreeSet;

use component_description::{
    AdmissionProfile, COMPONENT_DESCRIPTION_SCHEMA_V2, ComponentVerificationRequest,
    VerifiedComponent, verify_component,
};
use diagnostics::Diagnostic;
use package_compilation::{IndependentComponentDescription, PackageCompilationInputs};

/// The build's admission profile for one attached description.
///
/// The expected subject is the identity the dependency's compilation
/// observed, carried beside the bytes rather than read from them. The build
/// admits the current description schema only, and it accepts no assumption
/// digests: the build program has no vocabulary yet for accepting a declared
/// physical mechanism, so a description that binds one rejects as an
/// unaccepted assumption rather than being admitted on the producer's word.
/// The module-proof admission profile is likewise empty: only a module whose
/// obligations are kernel-dischargeable verifies, until the build grows the
/// vocabulary to accept site-bound admission evidence.
fn verification_request(
    description: &IndependentComponentDescription,
) -> ComponentVerificationRequest {
    ComponentVerificationRequest {
        expected_subject: description.expected_subject(),
        accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V2]),
        accepted_assumptions: BTreeSet::new(),
        admission_profile: AdmissionProfile::default(),
    }
}

/// Verify every component description the package inputs attached, in
/// package-identity order. A compilation without package inputs supplies no
/// components, so its `Independent` selections reject at the join.
pub(super) fn verify_independent_components(
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<Vec<VerifiedComponent>, Vec<Diagnostic>> {
    let Some(package_inputs) = package_inputs else {
        return Ok(Vec::new());
    };
    let mut components = Vec::new();
    let mut diagnostics = Vec::new();
    for description in package_inputs.independent_component_descriptions() {
        let request = verification_request(description);
        match verify_component(description.description(), &request) {
            Ok(component) => components.push(component),
            Err(rejection) => {
                let package = package_inputs
                    .package_name(description.package())
                    .unwrap_or("<unnamed>");
                diagnostics.push(Diagnostic::error(format!(
                    "component description attached for dependency package `{package}` failed independent verification: {rejection}; the build cannot deploy that package as an independent component",
                )));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(components)
    } else {
        Err(diagnostics)
    }
}
