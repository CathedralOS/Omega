use super::rejected;
use crate::capture::semantics::declarations::{
    nominal_identity, policy_provider_requirement_identity,
};
use crate::record::{
    PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate, PackagePolicyProviderPlan,
    PackageReviewProviderFamilyCoverage, PackageReviewProviderSelectionAuthority,
};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use provider_planning::plans::ProviderSelectionProvenance;
use provider_planning::{ProviderSelection, ProviderSelectionSubject};
use target::TargetProfile;

pub(super) fn project(
    compilation: &CheckedCompilation,
    target: TargetProfile,
    plans: &[(usize, PackagePolicyProviderPlan)],
) -> Result<Vec<PackagePolicyProviderFamily>, Vec<Diagnostic>> {
    let provenance = compilation.selected_provider_provenance();
    let mut seeds: Vec<(PackageReviewProviderSelectionAuthority, &ProviderSelection)> = Vec::new();
    for retained in provenance {
        let Some((authority, declarations)) = authored(&retained.selected_by) else {
            continue;
        };
        for declaration in declarations {
            if !matches!(
                declaration.subject,
                ProviderSelectionSubject::BoundaryOperatorFamily(_)
            ) {
                continue;
            }
            if let Some((_, existing)) = seeds.iter().find(|(candidate_authority, candidate)| {
                *candidate_authority == authority
                    && candidate.subject.same_declaration_as(&declaration.subject)
            }) {
                if existing.provider_type.symbol != declaration.provider_type.symbol {
                    return Err(rejected(
                        "operator family retains conflicting provider declarations",
                    ));
                }
            } else {
                seeds.push((authority, declaration));
            }
        }
    }
    let mut families = Vec::with_capacity(seeds.len());
    for (authority, selection) in seeds {
        let ProviderSelectionSubject::BoundaryOperatorFamily(family) = &selection.subject else {
            unreachable!("family seed category checked above")
        };
        let provider_type_declaration =
            nominal_identity(compilation, selection.provider_type.symbol)?;
        let first = family
            .coordinates()
            .first()
            .ok_or_else(|| rejected("selected operator family has no declaration coordinates"))?;
        let family_identity = nominal_identity(compilation, first.symbol)?;
        if family_identity.path() != family.canonical_path {
            return Err(rejected(
                "operator family path disagrees with its exact declaration",
            ));
        }
        let selected_count = provenance
            .iter()
            .filter(|retained| selects(&retained.selected_by, authority, selection))
            .count();
        if selected_count != family.coordinates().len() {
            return Err(rejected(
                "selected operator family has incomplete or padded plan coverage",
            ));
        }
        let mut coordinates = Vec::with_capacity(family.coordinates().len());
        for coordinate in family.coordinates() {
            let operator =
                typed_trees::operator::declaration_by_symbol(&compilation.typed, coordinate.symbol)
                    .ok_or_else(|| {
                        rejected("family coordinate has no exact typed operator declaration")
                    })?;
            let static_parameter_count = operator.lifetime_parameters.len()
                + compilation.operator_type_parameters(operator).len();
            if static_parameter_count != coordinate.static_parameter_count
                || typed_trees::operator::boundary_operator_requirement_identity(
                    &compilation.typed,
                    operator,
                ) != coordinate.requirement_identity
            {
                return Err(rejected(
                    "family coordinate differs from its declaration telescope or overload",
                ));
            }
            let operator_declaration = nominal_identity(compilation, coordinate.symbol)?;
            if operator_declaration != family_identity {
                return Err(rejected(
                    "operator coordinate is outside the selected nominal family",
                ));
            }
            let matches = plans
                .iter()
                .enumerate()
                .filter(|(_, (original_index, plan))| {
                    let retained = &provenance[*original_index];
                    retained.provider.schema.symbol() == coordinate.symbol
                        && retained.plan.schema.trait_name == coordinate.requirement_identity
                        && retained.provider.provider_type == Some(selection.provider_type.symbol)
                        && plan.provider_type_declaration() == Some(&provider_type_declaration)
                        && selects(&retained.selected_by, authority, selection)
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [index] = matches.as_slice() else {
                return Err(rejected(
                    "operator coordinate has no unique normalized selected plan",
                ));
            };
            coordinates.push(PackagePolicyProviderFamilyCoordinate {
                requirement_identity: policy_provider_requirement_identity(
                    compilation,
                    provider_planning::plans::ProviderSchemaDeclaration::BoundaryOperator(
                        coordinate.symbol,
                    ),
                    coordinate.symbol,
                )?
                .path,
                operator_declaration,
                plan_index: u32::try_from(*index)
                    .map_err(|_| rejected("selected family plan index exceeds u32"))?,
            });
        }
        coordinates.sort();
        families.push(PackagePolicyProviderFamily {
            family_identity,
            provider_type_declaration,
            target,
            authority,
            coverage: PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily,
            coordinates,
        });
    }
    families.sort_by(|left, right| {
        left.family_identity
            .cmp(&right.family_identity)
            .then(
                left.provider_type_declaration
                    .cmp(&right.provider_type_declaration),
            )
            .then(left.target.target_name().cmp(right.target.target_name()))
            .then(left.authority.cmp(&right.authority))
            .then(left.coverage.cmp(&right.coverage))
            .then(left.coordinates.cmp(&right.coordinates))
    });
    Ok(families)
}

fn authored(
    provenance: &ProviderSelectionProvenance,
) -> Option<(
    PackageReviewProviderSelectionAuthority,
    &[ProviderSelection],
)> {
    match provenance {
        ProviderSelectionProvenance::BuildOverride(declarations) => Some((
            PackageReviewProviderSelectionAuthority::BuildOverride,
            declarations,
        )),
        ProviderSelectionProvenance::TargetDefault(declarations) => Some((
            PackageReviewProviderSelectionAuthority::TargetDefault,
            declarations,
        )),
        ProviderSelectionProvenance::UniqueCoveringCandidate => None,
    }
}

fn selects(
    provenance: &ProviderSelectionProvenance,
    authority: PackageReviewProviderSelectionAuthority,
    selection: &ProviderSelection,
) -> bool {
    authored(provenance).is_some_and(|(candidate_authority, declarations)| {
        candidate_authority == authority
            && declarations.iter().any(|candidate| {
                candidate.subject.same_declaration_as(&selection.subject)
                    && candidate.provider_type.symbol == selection.provider_type.symbol
            })
    })
}
