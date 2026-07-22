//! MP2b: admission of compile-time machine-symbol arguments.
//!
//! Static selections are checked at the generic call edge. The selected
//! machine must be concrete, match the authored callable shape, stay within
//! the required effect ceiling, and conservatively refine conjunctive
//! requires/ensures facts. This pass never invents a callback contract.

use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{TypeParameter, TypeParameterKind};
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{ExpressionNode, StaticMachineArgument};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use omega_typed_trees::state::State;
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_static_machine_arguments(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            validate_call_selection(
                program,
                call.target_symbol,
                call.target.as_str(),
                &call.machine_arguments,
                diagnostics,
            );
        }
    }

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Call(call) = statement {
                    validate_call_selection(
                        program,
                        call.target_symbol,
                        call.target.as_str(),
                        &call.machine_arguments,
                        diagnostics,
                    );
                }
            }
        }
    }
}

/// Run MP2b admission as a standalone pre-specialization gate. MP4 consumes
/// the static argument syntax, so its refinement proof must happen first.
pub fn validate_static_machine_selections(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    validate_static_machine_arguments(program, &mut diagnostics);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_call_selection(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    target_name: &str,
    arguments: &[StaticMachineArgument],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((callee, _)) = machine_and_state(program, target_symbol) else {
        if !arguments.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "call `{target_name}` supplies static machine arguments, but its generic callee did not resolve"
            )));
        }
        return;
    };

    let requirements: Vec<(
        &TypeParameter,
        &omega_typed_trees::signature::StateSignature,
    )> = program
        .machine_type_parameters(callee)
        .iter()
        .filter_map(|parameter| match &parameter.kind {
            TypeParameterKind::Machine { contract } => Some((parameter, contract)),
            _ => None,
        })
        .collect();

    if arguments.len() != requirements.len() {
        diagnostics.push(Diagnostic::error(format!(
            "generic call `{target_name}` requires {} static machine argument(s), got {}",
            requirements.len(),
            arguments.len()
        )));
        return;
    }

    let generic_types: Vec<&TypeParameter> = program
        .machine_type_parameters(callee)
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
        .collect();
    let mut bindings = Vec::new();

    for ((parameter, requirement), selected) in requirements.into_iter().zip(arguments) {
        let rendered = selected
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        // A recursive generic body may forward its own authored machine
        // parameter (`map<F>(tail)`). This is not a concrete selection yet,
        // but it is already governed by exactly this requirement; the
        // eventual external selection is validated at the concrete call edge.
        if selected.symbol == parameter.symbol {
            continue;
        }
        if !selected.symbol.is_valid() {
            diagnostics.push(Diagnostic::error(format!(
                "static machine argument `{rendered}` for `{}` does not resolve to a concrete machine",
                parameter.name
            )));
            continue;
        }
        let Some((actual_machine, actual_state)) = machine_and_state(program, selected.symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "static machine argument `{rendered}` does not name a callable machine entry"
            )));
            continue;
        };
        if !program.machine_type_parameters(actual_machine).is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "static machine argument `{rendered}` is still generic; select a concrete machine symbol"
            )));
            continue;
        }

        validate_callable_shape(
            program,
            target_name,
            parameter,
            requirement,
            actual_machine,
            actual_state,
            &generic_types,
            &mut bindings,
            diagnostics,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_callable_shape(
    program: &TypedTrees,
    generic_call: &str,
    parameter: &TypeParameter,
    requirement: &omega_typed_trees::signature::StateSignature,
    actual_machine: &Machine,
    actual_state: &State,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_parameters = program.state_signature_parameters(requirement);
    let actual_parameters = program.state_parameters(actual_state);
    let label = format!(
        "machine argument `{}` for `{generic_call}`",
        actual_machine.name
    );
    if required_parameters.len() != actual_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: expected {} parameter(s), got {}",
            parameter.name,
            required_parameters.len(),
            actual_parameters.len()
        )));
        return;
    }

    for (index, (required, actual)) in required_parameters
        .iter()
        .zip(actual_parameters)
        .enumerate()
    {
        if required.is_self != actual.is_self
            || required.is_mutable != actual.is_mutable
            || required.is_const != actual.is_const
        {
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: parameter {} has a different calling mode",
                parameter.name, index
            )));
            continue;
        }
        if !required_type_matches(
            program,
            actual.type_reference,
            required.type_reference,
            generic_types,
            bindings,
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: parameter {} expects `{}`, got `{}`",
                parameter.name,
                index,
                program.display_type_reference(required.type_reference),
                program.display_type_reference(actual.type_reference)
            )));
        }
    }

    if !required_type_matches(
        program,
        actual_state.return_type,
        requirement.return_type,
        generic_types,
        bindings,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: expected return `{}`, got `{}`",
            parameter.name,
            program.display_type_reference(requirement.return_type),
            program.display_type_reference(actual_state.return_type)
        )));
    }

    let allowed_effects = program.state_signature_effects(requirement);
    for effect in program.machine_effects(actual_machine) {
        if !allowed_effects.iter().any(|allowed| allowed == effect) {
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: effect `{effect}` exceeds its authored ceiling",
                parameter.name
            )));
        }
    }

    if requirement.terminates_guarantee && !actual_machine.terminates {
        diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: the requirement guarantees termination",
            parameter.name
        )));
    }

    validate_contract_facts(
        program,
        &label,
        parameter,
        requirement,
        actual_machine,
        required_parameters,
        actual_parameters,
        diagnostics,
    );
}

/// N7 data-family admission uses the same refinement judgment as a generic
/// call, but its selected symbol is carried in a generic type argument rather
/// than an expression call node. Keeping this entry point here prevents proof
/// data from growing a weaker, shape-only callback check.
pub(crate) fn validate_data_machine_selection(
    program: &TypedTrees,
    family_name: &str,
    parameter: &TypeParameter,
    requirement: &omega_typed_trees::signature::StateSignature,
    selected_symbol: SymbolHandle,
    selected_name: &str,
    generic_types: &[&TypeParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((actual_machine, actual_state)) = machine_and_state(program, selected_symbol) else {
        diagnostics.push(Diagnostic::error(format!(
            "machine argument `{selected_name}` for proof data `{family_name}` does not name a callable machine entry"
        )));
        return;
    };
    if !program.machine_type_parameters(actual_machine).is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "machine argument `{selected_name}` for proof data `{family_name}` is still generic; select a concrete machine symbol"
        )));
        return;
    }

    validate_callable_shape(
        program,
        family_name,
        parameter,
        requirement,
        actual_machine,
        actual_state,
        generic_types,
        &mut Vec::new(),
        diagnostics,
    );
}

#[derive(Clone, Copy)]
struct TypeBinding {
    symbol: SymbolHandle,
    actual: TypeReferenceHandle,
}

fn required_type_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
        && let Some(parameter) = generic_types.iter().find(|parameter| {
            (parameter.symbol.is_valid() && parameter.symbol == *symbol)
                || parameter.name.as_str() == name.as_str()
        })
    {
        if let Some(binding) = bindings
            .iter()
            .find(|binding| binding.symbol == parameter.symbol)
        {
            return crate::type_references::type_references_match(program, actual, binding.actual);
        }
        bindings.push(TypeBinding {
            symbol: parameter.symbol,
            actual,
        });
        return true;
    }

    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_inner,
                is_mutable: actual_mutable,
                ..
            },
            TypeReferenceNode::Reference {
                referee: required_inner,
                is_mutable: required_mutable,
                ..
            },
        ) => {
            actual_mutable == required_mutable
                && required_type_matches(
                    program,
                    *actual_inner,
                    *required_inner,
                    generic_types,
                    bindings,
                )
        }
        (
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                ..
            },
            TypeReferenceNode::Constrained {
                base_type: required_base,
                ..
            },
        ) => required_type_matches(
            program,
            *actual_base,
            *required_base,
            generic_types,
            bindings,
        ),
        _ => crate::type_references::type_references_match(program, actual, required),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_contract_facts(
    program: &TypedTrees,
    label: &str,
    parameter: &TypeParameter,
    requirement: &omega_typed_trees::signature::StateSignature,
    actual_machine: &Machine,
    required_parameters: &[StateParameter],
    actual_parameters: &[StateParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_contracts = program.state_signature_contracts(requirement);
    let actual_contracts = program.machine_contracts(actual_machine);
    for kind in [
        SignatureContractKind::Requires,
        SignatureContractKind::Ensures,
        SignatureContractKind::Boundary,
    ] {
        let required = normalized_facts(program, required_contracts, kind, required_parameters);
        let actual = normalized_facts(program, actual_contracts, kind, actual_parameters);
        let valid = match kind {
            // Required preconditions may be stronger: callers of the generic
            // already prove them, so the selected implementation may demand a
            // subset. Selected postconditions must cover the requirement.
            SignatureContractKind::Requires => actual.iter().all(|fact| required.contains(fact)),
            SignatureContractKind::Ensures | SignatureContractKind::Boundary => {
                required.iter().all(|fact| actual.contains(fact))
            }
        };
        if !valid {
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: its {} facts are not a conservative refinement",
                parameter.name,
                contract_kind_name(kind)
            )));
        }
    }
}

fn normalized_facts(
    program: &TypedTrees,
    contracts: &[SignatureContract],
    kind: SignatureContractKind,
    parameters: &[StateParameter],
) -> Vec<String> {
    let mut facts = Vec::new();
    for contract in contracts.iter().filter(|contract| contract.kind == kind) {
        for fact in program.tables.proof_facts.span_or_empty(contract.facts) {
            let raw = match fact {
                ProofFact::Expression(expression) => {
                    program.expression_table.display_name(*expression)
                }
                ProofFact::Membership(membership) => format!(
                    "{} in {}",
                    program.expression_table.display_name(membership.value),
                    program
                        .domain_path_members(membership.domain)
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::")
                ),
            };
            facts.push(alpha_normalize(&raw, parameters));
        }
    }
    facts.sort();
    facts.dedup();
    facts
}

fn alpha_normalize(text: &str, parameters: &[StateParameter]) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    while cursor < characters.len() {
        if !characters[cursor].is_ascii_alphanumeric() && characters[cursor] != '_' {
            output.push(characters[cursor]);
            cursor += 1;
            continue;
        }

        let start = cursor;
        while cursor < characters.len()
            && (characters[cursor].is_ascii_alphanumeric() || characters[cursor] == '_')
        {
            cursor += 1;
        }
        let word: String = characters[start..cursor].iter().collect();
        let previous = characters[..start]
            .iter()
            .rev()
            .copied()
            .find(|character| !character.is_whitespace());
        let next = characters[cursor..]
            .iter()
            .copied()
            .find(|character| !character.is_whitespace());
        // Replace value references, never field/case labels, member names,
        // call targets, or type constructors that merely share the spelling.
        let is_declaration_name =
            matches!(previous, Some('.' | ':')) || matches!(next, Some(':' | '(' | '{'));
        if !is_declaration_name
            && let Some(index) = parameters
                .iter()
                .position(|parameter| parameter.name.as_str() == word)
        {
            output.push('$');
            output.push_str(&index.to_string());
        } else {
            output.push_str(&word);
        }
    }
    output
}

fn contract_kind_name(kind: SignatureContractKind) -> &'static str {
    match kind {
        SignatureContractKind::Requires => "requires",
        SignatureContractKind::Ensures => "ensures",
        SignatureContractKind::Boundary => "boundary",
    }
}

fn machine_and_state(
    program: &TypedTrees,
    selected_symbol: SymbolHandle,
) -> Option<(&Machine, &State)> {
    if !selected_symbol.is_valid() {
        return None;
    }
    program.machines().iter().find_map(|machine| {
        let states = program.machine_states(machine);
        states
            .iter()
            .find(|state| state.symbol == selected_symbol)
            .or_else(|| {
                (machine.symbol == selected_symbol)
                    .then(|| states.first())
                    .flatten()
            })
            .map(|state| (machine, state))
    })
}
