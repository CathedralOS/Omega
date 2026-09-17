use super::rejected;
use crate::capture::PackageReviewInput;
use crate::capture::api::operators::project_operator_coordinate;
use crate::capture::semantics::conformances::policy_callable_identity;
use crate::capture::semantics::declarations::{
    nominal_identity, policy_provider_requirement_identity,
};
use crate::record::*;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    package: PackageKeyIdentity,
    providers: &PackagePolicySelectedProviders,
    original_indices: &[usize],
) -> Result<Vec<PackagePolicyBoundaryApplicationRealization>, Vec<Diagnostic>> {
    let checked =
        super::super::application_realizations::project_boundary_application_realizations(
            compilation,
            package,
        )?;
    let mut rows = Vec::new();
    for checked in checked.rows {
        let matches = compilation
            .custody
            .selected_provider_provenance()
            .iter()
            .enumerate()
            .filter(|(_, provenance)| {
                provenance.plan.identity_digest().as_bytes() == &checked.selected_plan_digest
            })
            .collect::<Vec<_>>();
        let [(original_index, retained)] = matches.as_slice() else {
            return Err(rejected(
                "a closed application without one retained selected plan",
            ));
        };
        let selected_plan_index = original_indices
            .iter()
            .position(|index| index == original_index)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| {
                rejected("a closed application without a canonical selected plan index")
            })?;
        // The row's requirement is a boundary operator or a top-level boundary
        // requirement; either species keys the policy row on its own overload
        // coordinate (the operator overload, or the requirement's normalized
        // machine overload, both result-dispatching named requirements).
        let requirements = retained
            .provider
            .row_requirements
            .iter()
            .enumerate()
            .filter_map(|(index, symbol)| {
                provider_planning::IntrinsicRequirement::by_symbol(&compilation.typed, *symbol)
                    .filter(|requirement| {
                        requirement.requirement_identity == checked.requirement_identity
                            && requirement
                                .as_operator()
                                .is_none_or(|operator| operator.is_boundary)
                    })
                    .map(|requirement| (index, requirement))
            })
            .collect::<Vec<_>>();
        let [(row_index, requirement)] = requirements.as_slice() else {
            return Err(rejected(
                "a closed application without one exact selected operator overload",
            ));
        };
        let operator_coordinate = match requirement.as_operator() {
            Some(operator) => project_operator_coordinate(compilation, operator)?,
            None => project_requirement_coordinate(compilation, requirement.symbol)?,
        };
        if operator_coordinate.identity != checked.operator_declaration {
            return Err(rejected(
                "a closed application with a different operator owner",
            ));
        }
        let operator_symbol = requirement.symbol;
        let selected_symbol = *retained
            .provider
            .row_realizations
            .get(*row_index)
            .ok_or_else(|| rejected("a selected application row without a realization"))?;
        let declaration = nominal_identity(compilation, selected_symbol)?;
        let realization = match checked.realization {
            PackageReviewBoundaryApplicationRealization::NongenericCheckedBody {
                realization_machine,
                ..
            } => {
                if declaration != realization_machine {
                    return Err(rejected(
                        "a nongeneric application detached from its selected machine",
                    ));
                }
                PackagePolicyBoundaryRealization::NongenericCheckedBody {
                    declaration,
                    realization: policy_callable_identity(compilation, selected_symbol)?,
                }
            }
            PackageReviewBoundaryApplicationRealization::SpecializedCheckedBody {
                realization_template,
                ..
            } => {
                if declaration != realization_template {
                    return Err(rejected(
                        "a specialized application detached from its authored selected template",
                    ));
                }
                PackagePolicyBoundaryRealization::SpecializedCheckedBody {
                    declaration,
                    template: policy_callable_identity(compilation, selected_symbol)?,
                }
            }
            PackageReviewBoundaryApplicationRealization::ExactCompilerIntrinsic { execution } => {
                PackagePolicyBoundaryRealization::ExactCompilerIntrinsic { execution }
            }
        };
        rows.push(PackagePolicyBoundaryApplicationRealization {
            operator_coordinate,
            requirement_identity: policy_provider_requirement_identity(
                compilation,
                retained.provider.schema,
                operator_symbol,
            )?
            .path,
            application: checked.application,
            selected_plan_index,
            realization,
        });
    }
    rows.sort_by(|left, right| left.application_key().cmp(&right.application_key()));
    rows.dedup();
    for pair in rows.windows(2) {
        if pair[0].application_key() == pair[1].application_key() {
            return Err(rejected(
                "one application with contradictory normalized realizations",
            ));
        }
    }
    let result = PackagePolicyBoundaryApplications {
        demands: Vec::new(),
        realizations: rows,
    };
    result
        .validate_canonical_structure(package, providers.target(), providers)
        .map_err(rejected)?;
    Ok(result.realizations)
}

/// The policy coordinate of a top-level boundary requirement: its nominal
/// identity and the parameter and result dispatch of its normalized machine
/// overload, the requirement-species twin of the named operator coordinate.
fn project_requirement_coordinate(
    compilation: &PackageReviewInput<'_>,
    requirement: symbols::SymbolHandle,
) -> Result<PackageReviewOperatorCoordinate, Vec<Diagnostic>> {
    let machine = compilation
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == requirement)
        .ok_or_else(|| rejected("a closed application without its requirement machine"))?;
    let overload = compilation
        .typed
        .normalized_machine_overload_identity(machine)
        .ok_or_else(|| rejected("a closed application on a requirement without an entry state"))?;
    Ok(PackageReviewOperatorCoordinate {
        identity: nominal_identity(compilation, requirement)?,
        parameter_dispatch: overload.parameters().to_owned(),
        result_dispatch: overload.result_dispatch().identity(),
    })
}
