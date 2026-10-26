//! Matching a machine state against a trait signature: effect ceilings,
//! parameter matches, trait type bindings, lifetimes and fixed array
//! lengths. `satisfier_parameter_correspondence` is the one parameter-count
//! judgment that trait, top-level and operator conformance paths share.
//!
//! Requirement parameters are ordinary explicit parameters: the trait's types
//! bind from its explicit application, never from a subject position or the
//! realization's namespace.
//! A specialization retains universal satisfaction edges in their authored
//! family, not its closed executable telescope. Operand admission and signature
//! matching use the same exact source owner; body/layout checking stays concrete.

mod borrow_refinement;
mod region_application;

use crate::value_custody::type_references::{type_reference_label, type_references_match};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::domain::ProofFact;
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
use typed_trees::state::State;
use typed_trees::trait_definition::{AppliedTraitScope, TraitDefinition};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

/// Count the trailing parameters a specialized instance gained by realizing
/// runtime-bound `Value` binders as ordinary parameters. The specialization
/// cloner appends one parameter per runtime-bound slot after the template
/// state's authored parameters, so the tail is `instance arity - template
/// arity` on the corresponding state; every tail parameter carries a declared
/// `Value` carrier in binder order. Zero also reports a non-instance machine
/// or a tail that cannot be realized subjects: callers then apply the
/// ordinary strict arity check.
fn specialized_instance_realized_tail(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
) -> usize {
    let Some(application) = program.machine_specialization(machine.symbol) else {
        return 0;
    };
    let Some(template) = program.machine(application.template) else {
        return 0;
    };
    // Cloned states keep template order, so the checked state's positional
    // counterpart in the template supplies the authored parameter count.
    let Some(state_ordinal) =
        typed_trees::state::state_position(program.machine_states(machine), state.symbol)
    else {
        return 0;
    };
    let Some(template_state) = program.machine_states(template).get(state_ordinal) else {
        return 0;
    };
    let realized = program
        .state_parameters(state)
        .len()
        .saturating_sub(program.state_parameters(template_state).len());
    if realized == 0 {
        return 0;
    }
    let carriers = program
        .data_type_parameters
        .span_or_empty(application.template_parameters)
        .iter()
        .filter_map(|parameter| match &parameter.kind {
            TypeParameterKind::Value { type_reference } => Some(*type_reference),
            _ => None,
        })
        .collect::<Vec<_>>();
    let parameters = program.state_parameters(state);
    let trailing = &parameters[parameters.len() - realized..];
    // The tail must be realized `Value` subjects: their declared carriers
    // appear as an order-preserving subsequence of the template's `Value`
    // binder carriers.
    let mut cursor = 0usize;
    let mut matched = 0usize;
    for parameter in trailing {
        while cursor < carriers.len() {
            let carrier = carriers[cursor];
            cursor += 1;
            if type_references_match(program, parameter.type_reference, carrier) {
                matched += 1;
                break;
            }
        }
    }
    if matched == realized { realized } else { 0 }
}

/// Judge one satisfier's value-parameter list against its requirement's.
///
/// A satisfier declares exactly its requirement's parameters: none the
/// requirement lacks and none fewer (`wiki/spec/language/machines.md`,
/// "Static declarations and explicit state"). A provider is only its
/// satisfier machines, so there is no provider value for a call to supply:
/// state that outlives one call is a value the service returns and the caller
/// passes back through the requirement's own declared parameters. Trait rows
/// (exact edges, named maps, `via` leaves) and top-level and operator
/// requirements all judge their parameter correspondence here, so no
/// conformance path can admit an extra leading provider-state parameter that
/// a caller would have to supply implicitly. Callers pass the authored
/// parameter count; only parameters the compiler itself appended to a
/// specialized instance are excluded beforehand.
pub(crate) fn satisfier_parameter_correspondence(
    satisfier: &str,
    requirement: &str,
    declared: usize,
    required: usize,
) -> Result<(), Diagnostic> {
    if declared > required {
        return Err(Diagnostic::error(format!(
            "{satisfier} declares {declared} parameter(s), but {requirement} declares {required}: a satisfier declares no parameter its requirement lacks; a provider holds no state, so state that outlives a call passes through the requirement's declared parameters"
        )));
    }
    if declared < required {
        return Err(Diagnostic::error(format!(
            "{satisfier} declares {declared} parameter(s), but {requirement} declares {required}: a satisfier declares every parameter its requirement declares"
        )));
    }
    Ok(())
}

/// The parameters a satisfier state authored. A specialized instance appends
/// one trailing ordinary parameter per runtime-bound `Value` binder, after the
/// template's authored parameters in binder order (see monomorphization's
/// realized-subject cloning). The realized subjects carry a declared binder
/// carrier at runtime rather than declaring a new callable argument, so they
/// are sliced off before arity and positional correspondence.
pub(crate) fn authored_satisfier_parameters<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
) -> &'program [StateParameter] {
    let parameters = program.state_parameters(state);
    let realized_tail = specialized_instance_realized_tail(program, machine, state);
    &parameters[..parameters.len() - realized_tail]
}

/// Recover the owner of a retained universal satisfaction edge. Specialization
/// closes executable types but deliberately keeps the complete authored edge
/// roster unchanged; its operands still name that source family's binders.
/// A missing or disagreeing origin is an error, not a name-based fallback.
pub(crate) fn authored_conformance_machine<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
) -> Result<&'program Machine, Diagnostic> {
    if !program.machine_type_parameters(machine).is_empty()
        || program.machine_trait_conformances(machine).is_empty()
    {
        return Ok(machine);
    }
    let mut applications = program
        .machine_specializations
        .iter()
        .filter(|application| application.instance == machine.symbol);
    let Some(application) = applications.next() else {
        return Ok(machine);
    };
    let template = program.machines().iter().find(|template| {
        template.symbol == application.template
            && template.symbol != machine.symbol
            && template.type_parameters == application.template_parameters
            && !program.machine_type_parameters(template).is_empty()
            && program.machine_trait_conformances(template)
                == program.machine_trait_conformances(machine)
    });
    template
        .filter(|_| applications.next().is_none())
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "machine `{}` generic satisfaction lost its exact original family and application",
                machine.name,
            ))
        })
}

pub(crate) fn validate_applied_trait_signature(
    program: &dyn crate::CheckingInput,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    machine: &Machine,
    state: &State,
    application: &AppliedTraitScope<'_>,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let trait_definition = application.declaration;
    // The inherited declaration owns its nodes, while the complete application
    // owns their meaning. Bare composite arguments cannot carry that meaning
    // across an edge, so retain every outer telescope through signature checks.
    let explicit_type_arguments = program
        .trait_type_parameters(trait_definition)
        .iter()
        .map(|parameter| application.parameter_argument(program.typed_program(), parameter.symbol))
        .collect::<Vec<_>>();
    // Satisfaction quantifies over the source family, not one demanded
    // specialization. Private instances retain their original edge unchanged;
    // its arguments still name the original binders. Compare all signature
    // axes in that same environment rather than mixing concrete parameters
    // with an open application. Ordinary checking still judges each concrete
    // body, and its operational ceiling is checked below.
    let callable_machine = match authored_conformance_machine(program, machine) {
        Ok(owner) => owner,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return;
        }
    };
    let callable_state = if callable_machine.symbol == machine.symbol {
        state
    } else {
        let Some(source_state) =
            typed_trees::state::state_position(program.machine_states(machine), state.symbol)
                .and_then(|ordinal| program.machine_states(callable_machine).get(ordinal))
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` generic satisfaction lost its exact original family and application",
                machine.name,
            )));
            return;
        };
        source_state
    };
    let actual_parameters =
        authored_satisfier_parameters(program, callable_machine, callable_state);
    let required_parameters = program.state_signature_parameters(requirement);
    if let Err(diagnostic) = satisfier_parameter_correspondence(
        &format!("machine `{}` state `{}`", machine.name, state.name),
        &format!(
            "trait `{}` machine `{}`",
            trait_definition.name, requirement.name
        ),
        actual_parameters.len(),
        required_parameters.len(),
    ) {
        diagnostics.push(diagnostic);
        return;
    }

    let trait_type_parameters = program.trait_type_parameters(trait_definition);
    let requirement_type_parameters = program.state_signature_type_parameters(requirement);
    // Nested callable refinement borrows a fully applied, alpha-fresh telescope.
    // The ordinary value/result comparisons below still read the declaration
    // under AppliedTraitScope, so their positional bindings must continue to
    // name its original locals. Mixing those two symbol rosters loses scope.
    let applied_type_parameters = if requirement.type_parameters.is_empty() {
        program.state_signature_type_parameters(requirement)
    } else {
        let Some(prepared) = program
            .typed_program()
            .applied_callable_signature(application, requirement.symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "trait requirement `{}::{}` has no prepared callable telescope for its exact application",
                trait_definition.name, requirement.name
            )));
            return;
        };
        program.state_signature_type_parameters(prepared)
    };
    // Fixed application arguments and an inline implementation's environment
    // are not fresh invocation binders. All remaining binders still undergo
    // ordinary universal kind, property and contract refinement below.
    let actual_type_parameters = crate::machine_calls::static_arguments::invocation_parameters(
        program,
        machine,
        callable_machine,
        program.machine_type_parameters(callable_machine),
        trait_definition.symbol,
        requirement.symbol,
    );
    let indexed_law_telescope_groups = indexed_law_callable_telescope_groups(
        program,
        trait_definition,
        requirement,
        &explicit_type_arguments,
    );
    let expected_callable_parameter_count = requirement_type_parameters.len()
        + indexed_law_telescope_groups
            .iter()
            .map(|group| group.len())
            .sum::<usize>();
    if expected_callable_parameter_count != actual_type_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not satisfy trait `{}` machine `{}`: expected {} callable generic parameter(s), got {}",
            machine.name,
            trait_definition.name,
            requirement.name,
            expected_callable_parameter_count,
            actual_type_parameters.len(),
        )));
        return;
    }
    let effective_requirement_type_parameters = requirement_type_parameters
        .iter()
        .chain(
            indexed_law_telescope_groups
                .iter()
                .flat_map(|group| group.iter()),
        )
        .collect::<Vec<_>>();
    if let Some((index, (required, actual))) = effective_requirement_type_parameters
        .iter()
        .zip(actual_type_parameters.iter())
        .enumerate()
        .find(|(_, (required, actual))| {
            !super::type_parameter_kinds_callable(&required.kind, &actual.kind)
        })
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not satisfy trait `{}` machine `{}`: callable generic parameter {} has incompatible kinds (`{}` versus `{}`)",
            machine.name,
            trait_definition.name,
            requirement.name,
            index,
            required.name,
            actual.name,
        )));
        return;
    }
    let mut required_generic_types = trait_type_parameters.to_vec();
    // Value and const parameters may occur as named index arguments in a
    // dependent result too. Their positional bindings above must participate
    // in matching just as type binders do; declaration-local symbols differ.
    required_generic_types.extend(
        requirement_type_parameters
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind,
                    TypeParameterKind::Type
                        | TypeParameterKind::Const { .. }
                        | TypeParameterKind::Value { .. }
                )
            })
            .cloned(),
    );
    let applied_generic_types = applied_type_parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
        .collect::<Vec<_>>();
    crate::machine_calls::machine_parameters::validate_trait_callable_parameter_refinement(
        program,
        &format!(
            "provider machine `{}` for trait requirement `{}::{}`",
            machine.name, trait_definition.name, requirement.name
        ),
        applied_type_parameters,
        &actual_type_parameters[..requirement_type_parameters.len()],
        &applied_generic_types,
        diagnostics,
    );
    let mut actual_offset = requirement_type_parameters.len();
    for (representative, carrier_telescope) in indexed_law_telescope_groups.iter().enumerate() {
        let actual_end = actual_offset + carrier_telescope.len();
        let actual_group = &actual_type_parameters[actual_offset..actual_end];
        let carrier_generic_parameters = carrier_telescope.iter().collect::<Vec<_>>();
        crate::machine_calls::machine_parameters::validate_trait_callable_parameter_refinement(
            program,
            &format!(
                "provider machine `{}` indexed representative {} for proposition-law requirement `{}::{}`",
                machine.name,
                representative + 1,
                trait_definition.name,
                requirement.name
            ),
            carrier_telescope,
            actual_group,
            &carrier_generic_parameters,
            diagnostics,
        );
        actual_offset = actual_end;
    }
    let mut type_bindings = application
        .substitutions
        .iter()
        .enumerate()
        .map(|(ordinal, (parameter, actual))| TraitTypeBinding {
            parameter_symbol: *parameter,
            parameter_name: program.symbols.name(*parameter).to_owned(),
            target: TraitTypeBindingTarget::Type(*actual),
            scope_start: application
                .substitution_scope_starts
                .iter()
                .rev()
                .find(|start| **start <= ordinal)
                .copied()
                .unwrap_or(0),
        })
        .collect::<Vec<_>>();
    type_bindings.extend(
        super::type_parameter_binding_pairs(requirement_type_parameters, &actual_type_parameters)
            .map(|(required, actual)| TraitTypeBinding {
                parameter_symbol: required.symbol,
                parameter_name: required.name.as_str().to_owned(),
                target: TraitTypeBindingTarget::Parameter(actual.symbol),
                scope_start: 0,
            }),
    );
    for (index, (required, actual)) in requirement_type_parameters
        .iter()
        .zip(actual_type_parameters.iter())
        .enumerate()
    {
        match (&required.kind, &actual.kind) {
            (TypeParameterKind::Type, TypeParameterKind::Type) => {
                if typed_trees::data::type_parameter_demands_stronger_properties(required, actual) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` does not satisfy trait `{}` machine `{}`: callable generic parameter {} demands stronger type properties",
                        machine.name, trait_definition.name, requirement.name, index
                    )));
                }
            }
            _ => {
                let Some((required_type, actual_type)) =
                    crate::value_custody::type_shapes::scalar_parameter_type_pair(
                        &required.kind,
                        &actual.kind,
                    )
                else {
                    continue;
                };
                if !type_references_match_for_trait_bindings(
                    program,
                    actual_type,
                    required_type,
                    &required_generic_types,
                    &mut type_bindings,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` does not satisfy trait `{}` machine `{}`: callable const generic parameter {} has a different type",
                        machine.name, trait_definition.name, requirement.name, index
                    )));
                }
            }
        }
    }

    // Dependent parameter/result recipes use the same positional value
    // correspondence as the callable's ordinary operands. Parameter spelling
    // never identifies a binder across two declarations.
    type_bindings.extend(
        required_parameters
            .iter()
            .zip(actual_parameters.iter())
            .map(|(required, actual)| TraitTypeBinding {
                parameter_symbol: required.symbol,
                parameter_name: required.name.to_string(),
                target: TraitTypeBindingTarget::Parameter(actual.symbol),
                scope_start: 0,
            }),
    );

    for (index, (actual, required)) in actual_parameters
        .iter()
        .zip(required_parameters.iter())
        .enumerate()
    {
        validate_trait_parameter_match(
            program,
            machine,
            state,
            trait_definition.name.as_str(),
            requirement,
            &required_generic_types,
            &mut type_bindings,
            index,
            actual,
            required,
            diagnostics,
        );
    }

    if !type_references_match_for_trait_bindings(
        program,
        callable_state.return_type,
        requirement.return_type,
        &required_generic_types,
        &mut type_bindings,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: expected return `{}`, got `{}`",
            machine.name,
            state.name,
            trait_definition.name,
            requirement.name,
            type_reference_label(program, requirement.return_type),
            type_reference_label(program, callable_state.return_type)
        )));
    }
    if let Err(message) = borrow_refinement::validate(
        program,
        application,
        requirement,
        callable_machine,
        callable_state,
        actual_parameters,
        &actual_type_parameters,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: conformance borrow contract: {message}",
            machine.name, state.name, trait_definition.name, requirement.name,
        )));
    }

    crate::proof_contracts::contract_entailment::check_contract_refinement(
        program,
        callable_machine,
        callable_state,
        actual_parameters,
        &actual_type_parameters,
        application,
        requirement,
        diagnostics,
    );
    validate_trait_effect_ceiling(
        program,
        service_reaches,
        machine,
        state,
        trait_definition.name.as_str(),
        requirement,
        diagnostics,
    );
}

fn validate_trait_effect_ceiling(
    program: &dyn crate::CheckingInput,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let allowed_services =
        crate::machine_calls::effect_inference::signature_service_reach(program, requirement);
    let operational =
        crate::machine_calls::effect_inference::signature_operational(program, requirement);
    super::require_operational_ceiling(
        program,
        service_reaches,
        machine,
        &allowed_services,
        operational.may_suspend,
        operational.may_block,
        |breach| match breach {
            super::OperationalCeilingBreach::ReachedService(service_name) => format!(
                "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: service `{service_name}` is not allowed by the trait requirement",
                machine.name, state.name, trait_name, requirement.name,
            ),
            super::OperationalCeilingBreach::Suspends => format!(
                "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: `suspends;` exceeds the trait requirement's operational ceiling",
                machine.name, state.name, trait_name, requirement.name,
            ),
            super::OperationalCeilingBreach::Blocks => format!(
                "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: `blocks;` exceeds the trait requirement's operational ceiling",
                machine.name, state.name, trait_name, requirement.name,
            ),
        },
        diagnostics,
    );
}

fn validate_trait_parameter_match(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    trait_type_parameters: &[TypeParameter],
    type_bindings: &mut Vec<TraitTypeBinding>,
    index: usize,
    actual: &StateParameter,
    required: &StateParameter,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Every requirement parameter is an ordinary explicit parameter; no
    // position names a subject. Access has to agree before types compare.
    if actual.is_mutable != required.is_mutable {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter {}: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            index,
            parameter_shape_label(program, required),
            parameter_shape_label(program, actual)
        )));
        return;
    }

    if !type_references_match_for_trait_bindings(
        program,
        actual.type_reference,
        required.type_reference,
        trait_type_parameters,
        type_bindings,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter `{}`: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            required.name,
            bound_type_reference_label(program, required.type_reference, type_bindings),
            type_reference_label(program, actual.type_reference)
        )));
    }
}

/// A requirement parameter's type as the satisfier must spell it: each trait
/// parameter the edge has bound reads as its argument (`&mut Counter`, not
/// the declaration's `&mut T`). Diagnostic text only.
fn bound_type_reference_label(
    program: &dyn crate::CheckingInput,
    handle: TypeReferenceHandle,
    bindings: &[TraitTypeBinding],
) -> String {
    let mut label = type_reference_label(program, handle);
    for binding in bindings {
        let TraitTypeBindingTarget::Type(argument) = binding.target else {
            continue;
        };
        if binding.parameter_name.is_empty() {
            continue;
        }
        let replacement = type_reference_label(program, argument);
        let mut rebuilt = String::with_capacity(label.len());
        let mut rest = label.as_str();
        while let Some(index) = rest.find(binding.parameter_name.as_str()) {
            let end = index + binding.parameter_name.len();
            let word = |character: Option<char>| {
                character.is_some_and(|character| character.is_alphanumeric() || character == '_')
            };
            let bounded =
                !word(rest[..index].chars().next_back()) && !word(rest[end..].chars().next());
            rebuilt.push_str(&rest[..index]);
            rebuilt.push_str(if bounded {
                &replacement
            } else {
                &binding.parameter_name
            });
            rest = &rest[end..];
        }
        rebuilt.push_str(rest);
        label = rebuilt;
    }
    label
}

#[derive(Debug, Clone)]
pub(crate) struct TraitTypeBinding {
    pub(crate) parameter_symbol: SymbolHandle,
    pub(crate) parameter_name: String,
    pub(crate) target: TraitTypeBindingTarget,
    /// The argument sees only application bindings before this simultaneous
    /// telescope. Inferred/callable arguments belong directly to the satisfier.
    pub(crate) scope_start: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum TraitTypeBindingTarget {
    Type(TypeReferenceHandle),
    Parameter(SymbolHandle),
}

/// `parameter ↔ argument` pairs collected as `TraitTypeBinding`s — the
/// projection every conformance site builds before matching.
pub(crate) fn trait_type_bindings<'a>(
    parameters: impl Iterator<Item = &'a TypeParameter>,
    arguments: impl Iterator<Item = &'a TypeReferenceHandle>,
) -> Vec<TraitTypeBinding> {
    parameters
        .zip(arguments)
        .map(|(parameter, argument)| TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.to_string(),
            target: TraitTypeBindingTarget::Type(*argument),
            scope_start: 0,
        })
        .collect()
}

pub(crate) fn type_references_match_for_trait_bindings(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    trait_type_parameters: &[TypeParameter],
    bindings: &mut Vec<TraitTypeBinding>,
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }

    // Ordinary formal values can index dependent recipes just as local
    // generic binders do. Their positional correspondence was established by
    // the signature owner above; read that exact symbol binding before the
    // separate generic-type inference path. No spelling or carrier equality
    // may identify two independently declared value subjects.
    if let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(required)
        && symbol.is_valid()
        && let Some(binding) = bindings
            .iter()
            .find(|binding| binding.parameter_symbol == *symbol)
    {
        return binding_matches_actual(program, binding, actual, bindings);
    }
    if let Some(parameter) = required_trait_type_parameter(program, required, trait_type_parameters)
    {
        if let Some(binding) = bindings.iter().find(|binding| {
            binding.parameter_symbol == parameter.symbol
                && binding.parameter_name == parameter.name.as_str()
        }) {
            return binding_matches_actual(program, binding, actual, bindings);
        }

        bindings.push(TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.to_string(),
            target: TraitTypeBindingTarget::Type(actual),
            scope_start: 0,
        });
        return true;
    }

    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_referee,
                access: actual_access,
                // Lifetimes do not participate in trait-conformance matching.
                lifetime: _,
            },
            TypeReferenceNode::Reference {
                referee: required_referee,
                access: required_access,
                lifetime: _,
            },
        ) => {
            actual_access == required_access
                && type_references_match_for_trait_bindings(
                    program,
                    *actual_referee,
                    *required_referee,
                    trait_type_parameters,
                    bindings,
                )
        }
        typed_trees::constrained_type_reference_pair!(actual_base, required_base) => {
            type_references_match_for_trait_bindings(
                program,
                *actual_base,
                *required_base,
                trait_type_parameters,
                bindings,
            )
        }
        (
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: required_element,
                length: required_length,
            },
        ) => {
            fixed_array_lengths_match(program, actual_length, required_length, bindings)
                && type_references_match_for_trait_bindings(
                    program,
                    *actual_element,
                    *required_element,
                    trait_type_parameters,
                    bindings,
                )
        }
        (
            TypeReferenceNode::Slice {
                element_type: actual_element,
            },
            TypeReferenceNode::Slice {
                element_type: required_element,
            },
        ) => type_references_match_for_trait_bindings(
            program,
            *actual_element,
            *required_element,
            trait_type_parameters,
            bindings,
        ),
        (
            TypeReferenceNode::Generic {
                base_symbol: actual_base_symbol,
                arguments: actual_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: required_base_symbol,
                arguments: required_arguments,
                ..
            },
        )
        | (
            TypeReferenceNode::DynamicTrait {
                symbol: actual_base_symbol,
                arguments: actual_arguments,
                ..
            },
            TypeReferenceNode::DynamicTrait {
                symbol: required_base_symbol,
                arguments: required_arguments,
                ..
            },
        ) => {
            // A selector's arguments inhabit the same callable telescope as
            // data arguments. Compare them under the conformance substitution,
            // not by the unrelated declaration identities of their binders.
            actual_base_symbol.is_valid()
                && actual_base_symbol == required_base_symbol
                && actual_arguments.count() == required_arguments.count()
                && program
                    .type_reference_table
                    .type_reference_handles(*actual_arguments)
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*required_arguments)
                            .iter(),
                    )
                    .all(|(actual_argument, required_argument)| {
                        type_references_match_for_trait_bindings(
                            program,
                            *actual_argument,
                            *required_argument,
                            trait_type_parameters,
                            bindings,
                        )
                    })
        }
        (
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                ..
            },
            TypeReferenceNode::Generic { .. },
        ) if actual_symbol.is_valid() => {
            crate::declarations::data_queries::data_definition_by_symbol(program, *actual_symbol)
                .and_then(|definition| definition.generic_instance)
                .is_some_and(|origin| {
                    type_references_match_for_trait_bindings(
                        program,
                        origin,
                        required,
                        trait_type_parameters,
                        bindings,
                    )
                })
        }
        (TypeReferenceNode::ConstExpression(_), _) | (_, TypeReferenceNode::ConstExpression(_)) => {
            let pairs = bindings
                .iter()
                .filter_map(|binding| match binding.target {
                    TraitTypeBindingTarget::Parameter(actual) => {
                        Some((binding.parameter_symbol, actual))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let required_binders = pairs
                .iter()
                .enumerate()
                .map(|(ordinal, (required, _))| (*required, format!("callable-value:{ordinal}")))
                .collect::<Vec<_>>();
            let actual_binders = pairs
                .iter()
                .enumerate()
                .map(|(ordinal, (_, actual))| (*actual, format!("callable-value:{ordinal}")))
                .collect::<Vec<_>>();
            let identity = |reference, binders: &[(symbols::SymbolHandle, String)]| {
                typed_trees::type_identity::try_normalized_index_identity(
                    program,
                    typed_trees::type_identity::IndexIdentityArgument::TypeReference(reference),
                    binders,
                    &[],
                )
            };
            identity(actual, &actual_binders)
                .zip(identity(required, &required_binders))
                .is_some_and(|(actual, required)| actual == required)
        }
        _ => type_references_match(program, actual, required),
    }
}

/// Recheck the part of a requirement type erased by ordinary runtime type
/// equality. `target_lifetimes` is the telescope a `required` lifetime
/// mention resolves against — the target trait's parameters for a trait
/// conformance, or the requirement machine's own telescope for a top-level
/// `boundary requirement` — and `raw_application` maps each target ordinal to
/// the realizing machine's lifetime ordinal. Lifetimes outside
/// `target_lifetimes` are callable-local and keep their existing owner.
pub(crate) fn type_reference_lifetimes_match_requirement_application(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    target_lifetimes: &[typed_trees::name::Identifier],
    machine: &Machine,
    raw_application: &[u32],
    bindings: &[TraitTypeBinding],
) -> bool {
    let required_scope = region_application::RegionApplicationScope::requirement(bindings);
    let actual_scope = region_application::RegionApplicationScope::provider(bindings);
    crate::type_regions::type_reference_regions_match(
        program,
        actual,
        required,
        actual_scope,
        required_scope,
        &mut (),
        &mut |_, _, required_scope, _, actual, required| {
            if required_scope.is_provider() {
                required.is_none_or(|required| actual == Some(required))
            } else {
                requirement_lifetime_matches(
                    actual,
                    required,
                    target_lifetimes,
                    machine,
                    raw_application,
                )
            }
        },
    )
}

fn requirement_lifetime_matches(
    actual: Option<&typed_trees::name::Identifier>,
    required: Option<&typed_trees::name::Identifier>,
    target_lifetimes: &[typed_trees::name::Identifier],
    machine: &Machine,
    raw_application: &[u32],
) -> bool {
    let Some(required) = required else {
        return true;
    };
    let Some(trait_ordinal) = target_lifetimes
        .iter()
        .position(|parameter| parameter.as_str() == required.as_str())
    else {
        return true;
    };
    let Some(machine_ordinal) = raw_application
        .get(trait_ordinal)
        .and_then(|ordinal| usize::try_from(*ordinal).ok())
    else {
        return false;
    };
    let Some(expected) = machine.lifetime_parameters.get(machine_ordinal) else {
        return false;
    };
    actual.is_some_and(|actual| actual.as_str() == expected.as_str())
}

fn fixed_array_lengths_match(
    program: &dyn crate::CheckingInput,
    actual: &FixedArrayLength,
    required: &FixedArrayLength,
    bindings: &[TraitTypeBinding],
) -> bool {
    let FixedArrayLength::ConstParameter {
        symbol: required_symbol,
        name: required_name,
    } = required
    else {
        return actual == required;
    };
    let Some(binding) = bindings.iter().find(|binding| {
        binding.parameter_symbol == *required_symbol
            && binding.parameter_name == required_name.as_str()
    }) else {
        return actual == required;
    };
    let FixedArrayLength::ConstParameter {
        symbol: actual_symbol,
        ..
    } = actual
    else {
        return false;
    };
    match binding.target {
        TraitTypeBindingTarget::Parameter(expected_symbol) => {
            expected_symbol.is_valid() && *actual_symbol == expected_symbol
        }
        TraitTypeBindingTarget::Type(expected) => matches!(
            program.type_reference_table.type_reference(expected),
            TypeReferenceNode::Named { symbol, .. }
                if symbol.is_valid() && *actual_symbol == *symbol
        ),
    }
}

fn binding_matches_actual(
    program: &dyn crate::CheckingInput,
    binding: &TraitTypeBinding,
    actual: TypeReferenceHandle,
    bindings: &[TraitTypeBinding],
) -> bool {
    if let TraitTypeBindingTarget::Type(expected) = binding.target {
        let outer = &bindings[..binding.scope_start];
        let substitutions = outer
            .iter()
            .filter_map(|binding| match binding.target {
                TraitTypeBindingTarget::Type(reference) => {
                    Some((binding.parameter_symbol, reference))
                }
                TraitTypeBindingTarget::Parameter(_) => None,
            })
            .collect::<Vec<_>>();
        let mut scope_starts = outer
            .iter()
            .map(|binding| binding.scope_start)
            .collect::<Vec<_>>();
        scope_starts.dedup();
        if substitutions.len() == outer.len()
            && program.normalized_type_identity(actual)
                == program.scoped_type_identity(
                    typed_trees::type_identity::TypeIdentityRequest {
                        substitutions: &substitutions,
                        ..typed_trees::type_identity::TypeIdentityRequest::ordinary(expected)
                    },
                    &scope_starts,
                )
        {
            return true;
        }
    }
    match &binding.target {
        TraitTypeBindingTarget::Type(expected) => match (
            program.type_reference_table.type_reference(actual),
            program.type_reference_table.type_reference(*expected),
        ) {
            (
                TypeReferenceNode::Named {
                    symbol: actual_symbol,
                    ..
                },
                TypeReferenceNode::Named {
                    symbol: expected_symbol,
                    ..
                },
            ) if actual_symbol.is_valid() && expected_symbol.is_valid() => {
                actual_symbol == expected_symbol
                    || type_reference_is_instance_of_family(
                        program,
                        actual,
                        outer_binding_argument(program, *expected, bindings, binding.scope_start),
                    )
            }
            _ => {
                type_references_match(program, actual, *expected)
                    || type_reference_is_instance_of_family(
                        program,
                        actual,
                        outer_binding_argument(program, *expected, bindings, binding.scope_start),
                    )
            }
        },
        TraitTypeBindingTarget::Parameter(symbol) => matches!(
            program.type_reference_table.type_reference(actual),
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                ..
            } if (*symbol).is_valid() && *actual_symbol == *symbol
        ),
    }
}

/// A nested application's argument may name a parameter of an enclosing
/// application (`Reflexive<C, R>` inherited by `Equivalence<C, R>`). Follow
/// those simultaneous telescopes outward to the argument the enclosing
/// application supplies, each lookup seeing only the bindings before its own
/// telescope.
fn outer_binding_argument(
    program: &dyn crate::CheckingInput,
    mut argument: TypeReferenceHandle,
    bindings: &[TraitTypeBinding],
    mut scope_start: usize,
) -> TypeReferenceHandle {
    while let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(argument)
        && let Some(outer) = bindings[..scope_start]
            .iter()
            .rev()
            .find(|binding| binding.parameter_symbol == *symbol)
        && let TraitTypeBindingTarget::Type(supplied) = outer.target
    {
        argument = supplied;
        scope_start = outer.scope_start;
    }
    argument
}

fn type_reference_is_instance_of_family(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    family: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Named {
        symbol: family_symbol,
        ..
    } = program.type_reference_table.type_reference(family)
    else {
        return false;
    };
    let Some(family_definition) =
        crate::declarations::data_queries::data_definition_by_symbol(program, *family_symbol)
    else {
        return false;
    };
    if program.data_type_parameters(family_definition).is_empty() {
        return false;
    }
    matches!(
        program.type_reference_table.type_reference(actual),
        TypeReferenceNode::Generic { base_symbol, .. } if base_symbol == family_symbol
    )
}

fn indexed_law_callable_telescope_groups<'program>(
    program: &'program dyn crate::CheckingInput,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    explicit_type_arguments: &[TypeReferenceHandle],
) -> Vec<&'program [TypeParameter]> {
    let has_proposition_law = program
        .state_signature_contracts(requirement)
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .any(|fact| matches!(fact, ProofFact::Proposition(_)));
    if !has_proposition_law {
        return Vec::new();
    }
    let trait_parameters = program.trait_type_parameters(trait_definition);
    program
        .state_signature_parameters(requirement)
        .iter()
        .filter_map(|representative| {
            let carrier_parameter = required_trait_type_parameter(
                program,
                representative.type_reference,
                trait_parameters,
            )?;
            let carrier = carrier_data_definition_for_argument(
                program,
                trait_parameters,
                explicit_type_arguments,
                carrier_parameter,
            )?;
            let telescope = program.data_type_parameters(carrier);
            (!telescope.is_empty()).then_some(telescope)
        })
        .collect()
}

pub(crate) fn required_trait_type_parameter<'program>(
    program: &'program dyn crate::CheckingInput,
    required: TypeReferenceHandle,
    trait_type_parameters: &'program [TypeParameter],
) -> Option<&'program TypeParameter> {
    let symbol = &program
        .type_reference_table
        .type_reference(required)
        .as_named_symbol()?;

    symbol.is_valid().then_some(())?;
    trait_type_parameters
        .iter()
        .find(|parameter| parameter.symbol.is_valid() && parameter.symbol == *symbol)
}

/// The named data definition a trait type parameter's applied argument
/// resolves to: positionally zip the parameter to its argument, then require
/// a `Named` head resolving to exactly one definition.
pub(crate) fn carrier_data_definition_for_argument<'program>(
    program: &'program dyn crate::CheckingInput,
    trait_parameters: &[TypeParameter],
    applied_arguments: &[TypeReferenceHandle],
    carrier_parameter: &TypeParameter,
) -> Option<&'program typed_trees::data::DataDefinition> {
    let carrier_index =
        typed_trees::data::type_parameter_position(trait_parameters, carrier_parameter.symbol)?;
    let carrier_argument = *applied_arguments.get(carrier_index)?;
    let TypeReferenceNode::Named {
        symbol: carrier_symbol,
        ..
    } = program
        .type_reference_table
        .type_reference(carrier_argument)
    else {
        return None;
    };
    crate::declarations::data_queries::data_definition_by_symbol(program, *carrier_symbol)
}

pub(crate) fn parameter_shape_label(
    program: &dyn crate::CheckingInput,
    parameter: &StateParameter,
) -> String {
    format!(
        "{}: {}",
        parameter.name,
        type_reference_label(program, parameter.type_reference)
    )
}
