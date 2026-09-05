//! Public producer declarations are available choices, not active selections.

use super::rejected;
use crate::capture::api::policy::conformances as project_public_conformances;
use crate::capture::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::record::{PackagePolicyRepresentationAvailability, PackageReviewConformanceSubject};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

pub(super) fn project(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<PackagePolicyRepresentationAvailability>, Vec<Diagnostic>> {
    // Reuse the independent public-surface eligibility owner. In particular,
    // do not turn a private conformance/carrier into public availability merely
    // because the selected-application projector permits private choices.
    let public = project_public_conformances(compilation, package)?;
    let mut rows = Vec::new();
    for declaration in compilation.conformances().iter().filter(|declaration| {
        declaration.is_public
            && representation_planning::is_compiler_owned_opaque_representation_trait(
                &compilation.typed,
                declaration.trait_symbol,
            )
    }) {
        let identity = nominal_identity(compilation, declaration.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let matches = public
            .iter()
            .filter(|candidate| candidate.identity() == &identity)
            .collect::<Vec<_>>();
        let [projected] = matches.as_slice() else {
            return Err(rejected(
                "producer has no unique ordinary public conformance",
            ));
        };
        let arguments = compilation
            .type_reference_table
            .type_reference_handles(declaration.arguments);
        let [opaque_argument] = arguments else {
            return Err(rejected("producer has no exact opaque trait argument"));
        };
        let opaque_symbol = compilation
            .type_reference_table
            .type_symbol(*opaque_argument);
        let opaque_definitions = compilation
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == opaque_symbol)
            .collect::<Vec<_>>();
        let [opaque] = opaque_definitions.as_slice() else {
            return Err(rejected(
                "producer opaque argument is not one exact declaration",
            ));
        };
        if !opaque.is_public
            || opaque.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque
        {
            return Err(rejected(
                "producer does not expose a public boundary-opaque declaration",
            ));
        }
        let carriers = compilation
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == declaration.carrier_symbol)
            .collect::<Vec<_>>();
        let [carrier] = carriers.as_slice() else {
            return Err(rejected(
                "producer carrier is not one exact concrete data declaration",
            ));
        };
        if !carrier.is_public
            || carrier.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        {
            return Err(rejected(
                "producer does not expose an ordinary public checked carrier",
            ));
        }
        let carrier = nominal_identity(compilation, carrier.symbol)?;
        if projected.subject() != &PackageReviewConformanceSubject::Nominal(carrier.clone()) {
            return Err(rejected(
                "producer carrier differs from its ordinary public conformance",
            ));
        }
        rows.push(PackagePolicyRepresentationAvailability {
            opaque: nominal_identity(compilation, opaque.symbol)?,
            conformance: (*projected).clone(),
            carrier,
        });
    }
    rows.sort_by(|left, right| {
        left.conformance
            .identity()
            .cmp(right.conformance.identity())
    });
    Ok(rows)
}
