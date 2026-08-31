use super::super::semantics::declarations::nominal_identity;
use super::selection::validate_selected_provider_declaration_owner;
use crate::record::{
    CheckedPackageProviderFamilyCoordinateReview, CheckedPackageProviderFamilyReview,
    CheckedPackageProviderReview, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderSelectionAuthority,
};
use omega_compiler::CheckedCompilation;
use omega_provider_planning::{ProviderSelection, ProviderSelectionSubject};
use psi_diagnostics::Diagnostic;

#[derive(Clone)]
struct FamilySelectionSeed {
    authority: PackageReviewProviderSelectionAuthority,
    declaration: ProviderSelection,
}

fn declarations_for_authority(
    provenance: &omega_provider_planning::plans::ProviderSelectionProvenance,
) -> Option<(
    PackageReviewProviderSelectionAuthority,
    &[ProviderSelection],
)> {
    match provenance {
        omega_provider_planning::plans::ProviderSelectionProvenance::BuildOverride(
            declarations,
        ) => Some((
            PackageReviewProviderSelectionAuthority::BuildOverride,
            declarations,
        )),
        omega_provider_planning::plans::ProviderSelectionProvenance::TargetDefault(
            declarations,
        ) => Some((
            PackageReviewProviderSelectionAuthority::TargetDefault,
            declarations,
        )),
        omega_provider_planning::plans::ProviderSelectionProvenance::UniqueCoveringCandidate => {
            None
        }
    }
}

fn same_family_selection(left: &ProviderSelection, right: &ProviderSelection) -> bool {
    left.subject.same_declaration_as(&right.subject)
        && left.provider_type.symbol == right.provider_type.symbol
}

fn provenance_selects_family(
    provenance: &omega_provider_planning::plans::ProviderSelectionProvenance,
    seed: &FamilySelectionSeed,
) -> bool {
    declarations_for_authority(provenance).is_some_and(|(authority, declarations)| {
        authority == seed.authority
            && declarations
                .iter()
                .any(|declaration| same_family_selection(declaration, &seed.declaration))
    })
}

fn validate_retained_static_parameter_count(
    coordinate: &omega_provider_planning::ProviderOperatorFamilyCoordinate,
    expected_static_parameter_count: usize,
) -> Result<(), Vec<Diagnostic>> {
    if coordinate.static_parameter_count != expected_static_parameter_count {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator coordinate `{}` retains static-telescope arity {}, but its exact typed declaration has arity {}",
            coordinate.requirement_identity,
            coordinate.static_parameter_count,
            expected_static_parameter_count,
        ))]);
    }
    Ok(())
}

pub(crate) fn project_selected_provider_families(
    compilation: &CheckedCompilation,
    target: omega_target::TargetProfile,
    selected_providers: &[CheckedPackageProviderReview],
) -> Result<Vec<CheckedPackageProviderFamilyReview>, Vec<Diagnostic>> {
    let selected_plans = compilation.selected_provider_plans().plans();
    let provenance = compilation.selected_provider_provenance();
    if selected_plans.len() != selected_providers.len() || selected_plans.len() != provenance.len()
    {
        return Err(vec![Diagnostic::error(
            "selected-provider family projection is not aligned with the canonical selected plan set",
        )]);
    }

    let mut seeds: Vec<FamilySelectionSeed> = Vec::new();
    for retained in provenance {
        let Some((authority, declarations)) = declarations_for_authority(&retained.selected_by)
        else {
            continue;
        };
        for declaration in declarations {
            if !matches!(
                declaration.subject,
                ProviderSelectionSubject::BoundaryOperatorFamily(_)
            ) {
                continue;
            }
            if let Some(existing) = seeds.iter().find(|seed| {
                seed.authority == authority
                    && seed
                        .declaration
                        .subject
                        .same_declaration_as(&declaration.subject)
            }) {
                if existing.declaration.provider_type.symbol != declaration.provider_type.symbol {
                    return Err(vec![Diagnostic::error(format!(
                        "selected boundary-operator family `{}` retains conflicting provider identities",
                        declaration.subject.canonical_path(),
                    ))]);
                }
                continue;
            }
            seeds.push(FamilySelectionSeed {
                authority,
                declaration: declaration.clone(),
            });
        }
    }

    let mut families = Vec::with_capacity(seeds.len());
    for seed in seeds {
        let ProviderSelectionSubject::BoundaryOperatorFamily(family) = &seed.declaration.subject
        else {
            unreachable!("family seeds retain only operator-family subjects")
        };
        let provider_type_declaration =
            nominal_identity(compilation, seed.declaration.provider_type.symbol)?;
        validate_selected_provider_declaration_owner(
            &provider_type_declaration,
            seed.declaration.provider_type.package,
            family.canonical_path.as_str(),
            "family provider type",
        )?;

        let first = family.coordinates().first().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "selected boundary-operator family `{}` has no exact coordinates",
                family.canonical_path,
            ))]
        })?;
        let family_identity = nominal_identity(compilation, first.symbol)?;
        validate_selected_provider_declaration_owner(
            &family_identity,
            family.package,
            family.canonical_path.as_str(),
            "operator family",
        )?;
        if family_identity.path() != family.canonical_path {
            return Err(vec![Diagnostic::error(format!(
                "selected boundary-operator family `{}` disagrees with exact declaration identity `{}`",
                family.canonical_path,
                family_identity.path(),
            ))]);
        }

        let selected_by_family = provenance
            .iter()
            .filter(|retained| provenance_selects_family(&retained.selected_by, &seed))
            .count();
        if selected_by_family != family.coordinates().len() {
            return Err(vec![Diagnostic::error(format!(
                "selected boundary-operator family `{}` retains {} exact coordinates but {} selected plans",
                family.canonical_path,
                family.coordinates().len(),
                selected_by_family,
            ))]);
        }

        let mut coordinates = Vec::with_capacity(family.coordinates().len());
        for coordinate in family.coordinates() {
            let operator = psi_typed_trees::operator::declaration_by_symbol(
                &compilation.typed,
                coordinate.symbol,
            )
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "selected boundary-operator coordinate `{}` has no exact typed declaration",
                    coordinate.requirement_identity,
                ))]
            })?;
            let expected_static_parameter_count = operator.lifetime_parameters.len()
                + compilation.operator_type_parameters(operator).len();
            validate_retained_static_parameter_count(coordinate, expected_static_parameter_count)?;
            if expected_static_parameter_count != 0 {
                return Err(vec![Diagnostic::error(format!(
                    "generic boundary-operator coordinate `{}` remains outside package review until final specialization reconstructs D29 demand and rechecks the selected realization",
                    coordinate.requirement_identity,
                ))]);
            }
            let operator_declaration = nominal_identity(compilation, coordinate.symbol)?;
            if operator_declaration != family_identity {
                return Err(vec![Diagnostic::error(format!(
                    "selected boundary-operator coordinate `{}` is outside exact family `{}`",
                    coordinate.requirement_identity, family.canonical_path,
                ))]);
            }
            let matches = provenance
                .iter()
                .enumerate()
                .filter(|(_, retained)| {
                    retained.plan.schema.trait_name == coordinate.requirement_identity
                        && retained.provider.schema.symbol() == coordinate.symbol
                        && retained.provider.provider_type
                            == Some(seed.declaration.provider_type.symbol)
                        && provenance_selects_family(&retained.selected_by, &seed)
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [index] = matches.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected boundary-operator coordinate `{}` maps to {} exact selected provider plans; expected one",
                    coordinate.requirement_identity,
                    matches.len(),
                ))]);
            };
            let selected_provider = &selected_providers[*index];
            if selected_provider.provider_type_declaration() != Some(&provider_type_declaration) {
                return Err(vec![Diagnostic::error(format!(
                    "selected boundary-operator coordinate `{}` disagrees with family provider `{}`",
                    coordinate.requirement_identity,
                    provider_type_declaration.path(),
                ))]);
            }
            coordinates.push(CheckedPackageProviderFamilyCoordinateReview {
                requirement_identity: coordinate.requirement_identity.clone(),
                operator_declaration,
                plan_report_fingerprint: selected_provider.plan_report_fingerprint(),
            });
        }
        if coordinates
            .windows(2)
            .any(|pair| pair[0].requirement_identity >= pair[1].requirement_identity)
        {
            return Err(vec![Diagnostic::error(format!(
                "selected boundary-operator family `{}` is not canonically ordered by exact coordinate identity",
                family.canonical_path,
            ))]);
        }
        families.push(CheckedPackageProviderFamilyReview {
            family_identity,
            provider_type_declaration,
            target,
            authority: seed.authority,
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
    if families.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(vec![Diagnostic::error(
            "package review contains a duplicate exact selected provider-family row",
        )]);
    }
    Ok(families)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coordinate(arity: usize) -> omega_provider_planning::ProviderOperatorFamilyCoordinate {
        omega_provider_planning::ProviderOperatorFamilyCoordinate {
            symbol: psi_symbols::SymbolHandle::invalid(),
            requirement_identity: "operator::Transfer::move($0,$1)->unit".to_owned(),
            static_parameter_count: arity,
        }
    }

    #[test]
    fn family_review_rejects_retained_telescope_arity_drift() {
        let coordinate = coordinate(1);
        let diagnostics = validate_retained_static_parameter_count(&coordinate, 2)
            .expect_err("review must rederive telescope arity from the typed declaration");
        assert!(
            diagnostics[0]
                .message
                .contains("retains static-telescope arity 1")
        );
        assert!(
            diagnostics[0]
                .message
                .contains("typed declaration has arity 2")
        );
    }
}
