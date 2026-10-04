use super::super::semantics::declarations::nominal_identity;
use super::selection::validate_selected_provider_declaration_owner;
use crate::capture::PackageReviewInput;
use crate::record::{
    CheckedPackageProviderFamilyCoordinateReview, CheckedPackageProviderFamilyReview,
    CheckedPackageProviderReview, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderSelectionAuthority,
};
use diagnostics::Diagnostic;
use provider_planning::{ProviderSelection, ProviderSelectionSubject};

#[derive(Clone)]
struct FamilySelectionSeed {
    authority: PackageReviewProviderSelectionAuthority,
    declaration: ProviderSelection,
}

fn declarations_for_authority(
    provenance: &provider_planning::ProviderSelectionProvenance,
) -> Option<(
    PackageReviewProviderSelectionAuthority,
    &[ProviderSelection],
)> {
    match provenance {
        provider_planning::ProviderSelectionProvenance::BuildOverride(declarations) => Some((
            PackageReviewProviderSelectionAuthority::BuildOverride,
            declarations,
        )),
        provider_planning::ProviderSelectionProvenance::TargetDefault(declarations) => Some((
            PackageReviewProviderSelectionAuthority::TargetDefault,
            declarations,
        )),
        provider_planning::ProviderSelectionProvenance::UniqueCoveringCandidate => None,
    }
}

fn same_family_selection(left: &ProviderSelection, right: &ProviderSelection) -> bool {
    left.subject.same_declaration_as(&right.subject)
        && left.provider_type.symbol == right.provider_type.symbol
}

fn provenance_selects_family(
    provenance: &provider_planning::ProviderSelectionProvenance,
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
    coordinate: &provider_planning::ProviderOperatorFamilyCoordinate,
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

/// Lazily reconstruct the compilation's closed specialized operator
/// applications once a generic family coordinate needs them.
fn specialized_operator_applications<'a>(
    cache: &'a mut Option<Vec<selected_dispatch::CheckedSpecializedOperatorApplicationRealization>>,
    compilation: &PackageReviewInput<'_>,
) -> Result<
    &'a [selected_dispatch::CheckedSpecializedOperatorApplicationRealization],
    Vec<Diagnostic>,
> {
    match cache {
        Some(applications) => Ok(applications.as_slice()),
        cache @ None => {
            let derived =
                selected_dispatch::derive_checked_specialized_operator_application_realizations(
                    compilation,
                    &compilation.custody.execution().selected_provider_plan_facts,
                )?;
            Ok(cache.insert(derived).as_slice())
        }
    }
}

/// A generic coordinate is admissible only while every retained closed
/// application of that coordinate rejoins the plan its family selected. The
/// derivation has already reconstructed each nonempty D29 demand and
/// rechecked its specialization replay and realization contract; the
/// commitment join here binds that evidence to this coordinate's plan.
fn validate_generic_coordinate_applications(
    coordinate: &provider_planning::ProviderOperatorFamilyCoordinate,
    family_selected_plan_digest: &[u8; 32],
    applications: &[selected_dispatch::CheckedSpecializedOperatorApplicationRealization],
) -> Result<(), Vec<Diagnostic>> {
    for application in applications {
        if application.requirement_operator != coordinate.symbol {
            continue;
        }
        if application.provider_plan_commitment.as_bytes() != family_selected_plan_digest {
            return Err(vec![Diagnostic::error(format!(
                "generic boundary-operator coordinate `{}` retains a closed application whose selected plan commitment disagrees with the family-selected ProviderPlan",
                coordinate.requirement_identity,
            ))]);
        }
    }
    Ok(())
}

pub(crate) fn project_selected_provider_families(
    compilation: &PackageReviewInput<'_>,
    target: target::TargetProfile,
    selected_providers: &[CheckedPackageProviderReview],
) -> Result<Vec<CheckedPackageProviderFamilyReview>, Vec<Diagnostic>> {
    let selected_execution = compilation.custody.execution();
    let selected_plans = selected_execution.selected_provider_plan_facts.plans();
    let provenance = selected_execution.selected_provider_provenance.as_slice();
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

    let mut specialized_applications = None;
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
            let operator = typed_trees::operator::declaration_by_symbol(
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
            if expected_static_parameter_count != 0 {
                let applications =
                    specialized_operator_applications(&mut specialized_applications, compilation)?;
                validate_generic_coordinate_applications(
                    coordinate,
                    selected_provider.selected_plan_digest().as_bytes(),
                    applications,
                )?;
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
