//! Satisfied boundary and checked operators and specialized applications.

use crate::TypedTrees;
use crate::type_identity::TypeIdentityRequest;
use crate::typed_trees::declarations::operator::applications::{
    ClosedOperatorRealizationApplication, closed_operator_realization_application,
};
use crate::typed_trees::declarations::operator::{OperatorDefinition, declaration_by_symbol};
use symbols::SymbolHandle;

/// Resolve one machine's explicit `satisfies Namespace::requirement` edge to
/// an exact overloaded boundary operator. Trait conformances keep their own
/// resolver; this path is for target/provider machines realizing an operator
/// requirement such as the f32 or f64 overload of `Float::add`.
pub fn resolve_satisfied_boundary_operator<'program>(
    program: &'program TypedTrees,
    machine: &'program crate::machine::Machine,
    namespace: &str,
    requirement: &str,
) -> Option<&'program OperatorDefinition> {
    resolve_satisfied_operator(program, machine, namespace, requirement, true)
}

/// Resolve an ordinary checked machine's exact operator requirement. This is
/// the PDI3 counterpart to the boundary-provider route: signature and path are
/// identical, while `boundary_only` is deliberately false.
pub fn resolve_satisfied_checked_operator<'program>(
    program: &'program TypedTrees,
    machine: &'program crate::machine::Machine,
    namespace: &str,
    requirement: &str,
) -> Option<&'program OperatorDefinition> {
    resolve_satisfied_operator(program, machine, namespace, requirement, false)
}

/// Conformance-aware counterpart to [`resolve_satisfied_boundary_operator`].
///
/// Path plus signature are not package-qualified: two packages may each
/// declare an identically spelled and signed boundary operator, and their
/// requirement identities are then equal too. The clause itself carries the
/// exact joins -- `requirement_symbol` is the settled declaration selection
/// once checking has run, and the satisfied owner declaration's package still
/// disqualifies same-spelled foreign overloads before settlement.
pub fn resolve_satisfied_boundary_operator_for_conformance<'program>(
    program: &'program TypedTrees,
    machine: &'program crate::machine::Machine,
    conformance: &'program crate::machine::TraitConformance,
) -> Option<&'program OperatorDefinition> {
    resolve_satisfied_operator_for_conformance(program, machine, conformance, true)
}

/// Conformance-aware counterpart to [`resolve_satisfied_checked_operator`].
/// See [`resolve_satisfied_boundary_operator_for_conformance`] for the exact
/// join contract.
pub fn resolve_satisfied_checked_operator_for_conformance<'program>(
    program: &'program TypedTrees,
    machine: &'program crate::machine::Machine,
    conformance: &'program crate::machine::TraitConformance,
) -> Option<&'program OperatorDefinition> {
    resolve_satisfied_operator_for_conformance(program, machine, conformance, false)
}

fn resolve_satisfied_operator_for_conformance<'program>(
    program: &'program TypedTrees,
    machine: &'program crate::machine::Machine,
    conformance: &'program crate::machine::TraitConformance,
    boundary_only: bool,
) -> Option<&'program OperatorDefinition> {
    let requirement = conformance.requirement.as_ref()?;
    let candidates = satisfied_operator_candidates(
        program,
        machine,
        conformance.name.as_str(),
        requirement.as_str(),
        boundary_only,
    )
    .collect::<Vec<_>>();
    // The settled requirement symbol is the exact checked selection. Replaying
    // it through the signature predicate keeps drift fail-closed while never
    // substituting a same-spelled declaration owned by another package.
    if conformance.requirement_symbol.is_valid() {
        return candidates
            .into_iter()
            .find(|operator| operator.symbol == conformance.requirement_symbol);
    }
    match candidates.as_slice() {
        [only] => Some(*only),
        _ => {
            let owner_package = conformance
                .symbol
                .is_valid()
                .then(|| program.symbols.symbol_package_identity(conformance.symbol))?;
            let mut owned = candidates.into_iter().filter(|operator| {
                program.symbols.symbol_package_identity(operator.symbol) == owner_package
            });
            let first = owned.next()?;
            owned.next().is_none().then_some(first)
        }
    }
}

/// Resolve a concrete generic checked-body specialization through the exact
/// closed operator application retained by authoritative monomorphization.
///
/// This is deliberately separate from [`resolve_satisfied_checked_operator`]:
/// the latter proves the authored generic declaration relation, while this
/// path proves one closed specialization and returns its retained application
/// for category/bounds/commitment replay.
pub fn resolve_specialized_checked_operator_application<'program>(
    program: &'program TypedTrees,
    machine: &'program crate::machine::Machine,
    namespace: &str,
    requirement: &str,
) -> Option<(
    &'program OperatorDefinition,
    &'program ClosedOperatorRealizationApplication,
)> {
    let mut specializations = program
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.instance == machine.symbol);
    let specialization = specializations.next()?;
    if specializations.next().is_some() {
        return None;
    }
    let mut rows = specialization
        .operator_realizations
        .iter()
        .filter_map(|row| {
            let operator = declaration_by_symbol(program, row.requirement_symbol)?;
            operator_path_matches(operator, program, namespace, requirement)
                .then_some((operator, row))
        });
    let (operator, retained) = rows.next()?;
    if rows.next().is_some()
        || closed_operator_realization_application(program, machine, operator).as_ref()
            != Some(retained)
    {
        return None;
    }
    Some((operator, retained))
}

fn resolve_satisfied_operator<'program>(
    program: &'program TypedTrees,
    machine: &'program crate::machine::Machine,
    namespace: &str,
    requirement: &str,
    boundary_only: bool,
) -> Option<&'program OperatorDefinition> {
    let mut candidates =
        satisfied_operator_candidates(program, machine, namespace, requirement, boundary_only);
    let selected = candidates.next()?;
    candidates.next().is_none().then_some(selected)
}

/// Every operator declaration whose path and alpha-renamed signature the
/// machine's entry state could satisfy. The result is deliberately not
/// package-disambiguated; callers holding a conformance clause must use
/// [`resolve_satisfied_operator_for_conformance`] so the settled requirement
/// symbol (or the satisfied owner package) joins the answer.
fn satisfied_operator_candidates<'program>(
    program: &'program TypedTrees,
    machine: &'program crate::machine::Machine,
    namespace: &str,
    requirement: &str,
    boundary_only: bool,
) -> impl Iterator<Item = &'program OperatorDefinition> {
    let state = program.machine_states(machine).first();
    let actual_parameters = state.map(|state| program.state_parameters(state));
    program.operators().iter().filter(move |operator| {
        let (Some(state), Some(actual_parameters)) = (state, actual_parameters) else {
            return false;
        };
        if (boundary_only && !operator.is_boundary)
            || !operator_path_matches(operator, program, namespace, requirement)
        {
            return false;
        }
        let Some((machine_binders, operator_binders)) =
            operator_realization_static_binders(program, machine, operator)
        else {
            return false;
        };
        let required_parameters = program.operator_parameters(operator);
        actual_parameters.len() == required_parameters.len()
            && actual_parameters
                .iter()
                .zip(required_parameters.iter())
                .all(|(actual, required)| {
                    actual.is_self == required.is_self
                        && actual.is_const == required.is_const
                        && actual.is_mutable == required.is_mutable
                        && program.type_identity(TypeIdentityRequest {
                            binders: &machine_binders,
                            ..TypeIdentityRequest::ordinary(actual.type_reference)
                        }) == program.type_identity(TypeIdentityRequest {
                            binders: &operator_binders,
                            ..TypeIdentityRequest::ordinary(required.type_reference)
                        })
                })
            && state.return_type.is_valid() == operator.return_type.is_valid()
            && (!state.return_type.is_valid()
                || program.type_identity(TypeIdentityRequest {
                    binders: &machine_binders,
                    ..TypeIdentityRequest::ordinary(state.return_type)
                }) == program.type_identity(TypeIdentityRequest {
                    binders: &operator_binders,
                    ..TypeIdentityRequest::ordinary(operator.return_type)
                }))
    })
}

/// Build one alpha-normalized relation between a realizing machine's static
/// telescope and the operator requirement telescope it claims to satisfy.
///
/// Type and const parameters are matched by category and declaration order;
/// names and private symbols are not identity. Const carriers remain exact;
/// provider type-property demands may weaken the requirement but never
/// strengthen it. Static-machine and proposition parameters have no
/// operator-application replay rule yet and therefore fail closed here instead
/// of being flattened to a count.
fn operator_realization_static_binders(
    program: &TypedTrees,
    machine: &crate::machine::Machine,
    operator: &OperatorDefinition,
) -> Option<(Vec<(SymbolHandle, String)>, Vec<(SymbolHandle, String)>)> {
    let machine_parameters = program.machine_type_parameters(machine);
    let operator_parameters = program.operator_type_parameters(operator);
    if machine_parameters.len() != operator_parameters.len() {
        return None;
    }
    let machine_binders = machine_parameters
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, format!("${ordinal}")))
        .collect::<Vec<_>>();
    let operator_binders = operator_parameters
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, format!("${ordinal}")))
        .collect::<Vec<_>>();
    for (machine_parameter, operator_parameter) in
        machine_parameters.iter().zip(operator_parameters)
    {
        match (&machine_parameter.kind, &operator_parameter.kind) {
            (crate::data::TypeParameterKind::Type, crate::data::TypeParameterKind::Type)
                if !crate::data::type_parameter_demands_stronger_properties(
                    operator_parameter,
                    machine_parameter,
                ) => {}
            (
                crate::data::TypeParameterKind::Const {
                    type_reference: machine_carrier,
                },
                crate::data::TypeParameterKind::Const {
                    type_reference: operator_carrier,
                },
            ) if machine_parameter.bounds == operator_parameter.bounds
                && program.type_identity(TypeIdentityRequest {
                    binders: &machine_binders,
                    ..TypeIdentityRequest::ordinary(*machine_carrier)
                }) == program.type_identity(TypeIdentityRequest {
                    binders: &operator_binders,
                    ..TypeIdentityRequest::ordinary(*operator_carrier)
                }) => {}
            _ => return None,
        }
    }
    Some((machine_binders, operator_binders))
}

fn operator_path_matches(
    operator: &OperatorDefinition,
    program: &TypedTrees,
    namespace: &str,
    requirement: &str,
) -> bool {
    let path = program.operator_path_members(operator.name);
    matches!(path, [owner, member]
        if owner.as_str() == namespace && member.as_str() == requirement)
}

/// Stable slot identity for one exact overloaded boundary-operator
/// requirement. Provider selection must distinguish f32 and f64 overloads even
/// though both are authored as `Float::add`.
pub fn boundary_operator_requirement_identity(
    program: &TypedTrees,
    operator: &OperatorDefinition,
) -> String {
    let path = program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let parameters = program
        .operator_parameters(operator)
        .iter()
        .map(|parameter| {
            program
                .normalized_type_identity(parameter.type_reference)
                .into_string()
        })
        .collect::<Vec<_>>()
        .join(",");
    let result = operator.return_type.is_valid().then(|| {
        program
            .normalized_type_identity(operator.return_type)
            .into_string()
    });
    format!(
        "operator::{path}({parameters})->{}",
        result.as_deref().unwrap_or("unit")
    )
}
