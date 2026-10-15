//! Owner hooks remain ordinary machines with compiler-selected invocations.
//! An authored call cannot select that reserved role. Once invoked, however,
//! the hook may transition between its own states: those exact selections
//! select control-flow destinations, not another invocation of the hook.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId;
use language_semantics::{
    MachineSupplyMode, ReferenceAccess, TerminationGuarantee, TerminationInterface,
};
use std::collections::BTreeSet;
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::statement::TransitionTargetNode;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub fn type_graph_requires_nominal_drop(
    program: &dyn crate::CheckingInput,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visited: &mut BTreeSet<String>,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => return false,
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                let Some(replacement) = crate::substitutions::actual_for(substitutions, *symbol)
                else {
                    break;
                };
                if replacement == type_reference {
                    return false;
                }
                type_reference = replacement;
            }
            _ => break,
        }
    }

    let identity = program.type_identity_request_text(TypeIdentityRequest {
        substitutions,
        ..TypeIdentityRequest::ordinary(type_reference)
    });
    if !visited.insert(identity) {
        return false;
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => false,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_graph_requires_nominal_drop(program, *base_type, substitutions, visited)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_graph_requires_nominal_drop(program, *element_type, substitutions, visited)
        }
        TypeReferenceNode::Named { symbol, name } => program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol || data.name.as_str() == name.as_str())
            .is_some_and(|data| {
                data_graph_requires_nominal_drop(program, data, substitutions, visited)
            }),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let Some(data) =
                crate::declarations::data_queries::data_definition_by_symbol(program, *base_symbol)
            else {
                return false;
            };
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let parameters = program.data_type_parameters(data);
            if arguments.len() != parameters.len() {
                return false;
            }
            let mut nested_substitutions = substitutions.to_vec();
            nested_substitutions.extend(
                parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
            data_graph_requires_nominal_drop(program, data, &nested_substitutions, visited)
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

/// Check cleanup of an exact nominal owner, including its owned fields.
/// The explicit parameter's exact data declaration owns the hook; a qualified
/// machine name supplies no implicit receiver or `Self` type.
pub fn data_requires_nominal_drop(
    program: &dyn crate::CheckingInput,
    data: &typed_trees::data::DataDefinition,
) -> bool {
    data_graph_requires_nominal_drop(program, data, &[], &mut BTreeSet::new())
}

fn data_graph_requires_nominal_drop(
    program: &dyn crate::CheckingInput,
    data: &typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visited: &mut BTreeSet<String>,
) -> bool {
    if program.machines().iter().any(|machine| {
        machine.name.as_str().ends_with("::drop") && machine.attached_data_symbol == data.symbol
    }) {
        return true;
    }
    // An erased member stays semantically present but owns no runtime
    // representation, so it can never produce runtime cleanup even when its
    // type would otherwise require a nominal drop.
    program
        .data_members(data)
        .iter()
        .any(|member| match member {
            DataMember::Field(field) => {
                !field.relevance.is_erased()
                    && type_graph_requires_nominal_drop(
                        program,
                        field.type_reference,
                        substitutions,
                        visited,
                    )
            }
            DataMember::Variant(variant) => {
                program.data_payload_fields(variant).iter().any(|field| {
                    !field.relevance.is_erased()
                        && type_graph_requires_nominal_drop(
                            program,
                            field.type_reference,
                            substitutions,
                            visited,
                        )
                })
            }
        })
}

pub(crate) fn validate_cleanup_machine_declarations(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines().iter().filter(|machine| {
        machine.attached_data.is_some() && machine.name.as_str().ends_with("::drop")
    }) {
        validate_cleanup_machine(program, machine, diagnostics);
    }
}

/// Reject authored access to the one compiler-selected cleanup hook attached
/// to a nominal owner. Automatic edge cleanup is deliberately absent from the
/// authored-selection ledger, so matching an exact retained declaration here
/// cannot reject compiler-planned cleanup. Spelling is never sufficient: an
/// unattached ordinary machine named `drop` remains callable.
pub(crate) fn collect_reserved_cleanup_selection_diagnostics(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Reserved cleanup machines are few; find them once rather than testing
    // every machine's name against every authored selection.
    let cleanups = program
        .machines()
        .iter()
        .filter(|machine| is_reserved_cleanup(program, machine))
        .collect::<Vec<_>>();
    if cleanups.is_empty() {
        return;
    }
    let internal_transitions = internal_cleanup_transitions(program, &cleanups);
    for selection in program.authored_declaration_selections() {
        let selection_target = selection.target();
        let Some(target) = selection_target.as_resolved() else {
            continue;
        };
        let Some(cleanup) =
            reserved_cleanup_selected_by(program, &cleanups, target.selected_symbol())
        else {
            continue;
        };
        if internal_transitions.contains(&selection.occurrence_id()) {
            continue;
        }
        diagnostics.push(
            Diagnostic::error(format!(
                "reserved cleanup machine `{}` is compiler-selected and cannot be selected by source; consume the value with `omega::core::drop(value)` or call an ordinary owner-published protocol operation",
                cleanup.name,
            ))
            .with_source_span(selection.source_span()),
        );
    }
}

/// A transition selects a state within the already executing machine, not a
/// callable hook. Join the exact retained occurrence and resolved state owner;
/// a same-spelled call, static argument or external state path gains no access.
fn internal_cleanup_transitions(
    program: &dyn crate::CheckingInput,
    cleanups: &[&Machine],
) -> BTreeSet<AuthoredDeclarationSelectionOccurrenceId> {
    let mut transitions = BTreeSet::new();
    for machine in cleanups {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                for target in transition.targets() {
                    let TransitionTargetNode::Named {
                        path,
                        authored_call_selection: Some(occurrence),
                        ..
                    } = program.statement_table.transition_target(target)
                    else {
                        continue;
                    };
                    if !path.symbol.is_valid() {
                        continue;
                    }
                    let selected = program.symbols.get(path.symbol);
                    if selected.kind == symbols::SymbolKind::State
                        && selected.parent == machine.symbol
                    {
                        transitions.insert(*occurrence);
                    }
                }
            }
        }
    }
    transitions
}

pub fn validate_reserved_cleanup_selections(
    program: &dyn crate::CheckingInput,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    collect_reserved_cleanup_selection_diagnostics(program, &mut diagnostics);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// A `drop` machine attached to known data: the compiler selects it and
/// source may not.
fn is_reserved_cleanup(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
    let owner = machine.attached_data_symbol;
    machine.attached_data.is_some()
        && owner.is_valid()
        && machine.name.as_str().rsplit("::").next() == Some("drop")
        && crate::declarations::data_queries::data_definition_by_symbol(program, owner).is_some()
}

/// Recover the owner-designated cleanup role from an exact checked declaration.
/// Specialization keeps the template's reserved role and the instance's owner;
/// an attached borrowed method or a similarly named free function gains none.
/// Declaration validation separately enforces the full owner and hook contract.
pub fn nominal_cleanup_owner(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
        if let Ok(owners) = memos.nominal_cleanup_owners.lock()
            && let Some(owner) = owners.get(&symbol).copied()
        {
            return owner;
        }
        let owner = nominal_cleanup_owner_walk(program, symbol);
        if let Ok(mut owners) = memos.nominal_cleanup_owners.lock() {
            owners.insert(symbol, owner);
        }
        return owner;
    }
    nominal_cleanup_owner_walk(program, symbol)
}

fn nominal_cleanup_owner_walk(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    let machine = program.machine(symbol)?;
    let mut declaration = machine;
    let mut visited = Vec::new();
    while !is_reserved_cleanup(program, declaration) {
        if visited.contains(&declaration.symbol) {
            return None;
        }
        visited.push(declaration.symbol);
        let template = program.machine_specialization(declaration.symbol)?.template;
        declaration = program.machine(template)?;
    }
    machine
        .attached_data_symbol
        .is_valid()
        .then_some(machine.attached_data_symbol)
}

/// Bind an owner-designated hook to the actual receiver application. Source
/// fields and regions keep their namespace; the executable declaration may
/// belong to a closed carrier derived from that exact owner. Carrier identity
/// alone never licenses a different nominal's hook. Source admission checks
/// every selected body's contract, including ordinary empty Unit bodies.
pub fn nominal_cleanup_machine_for_type<'program>(
    program: &'program dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<&'program Machine> {
    let reference = crate::peel_declared_constraints(program, reference);
    let owner = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
        _ => return None,
    };
    let identity = program.type_identity(TypeIdentityRequest {
        substitutions,
        ..TypeIdentityRequest::ordinary(reference)
    });
    let mut selected = None;
    let Some(candidates) = nominal_cleanup_candidates(program).get(&owner).cloned() else {
        return None;
    };
    for candidate in candidates {
        let Some(machine) = program.machine(candidate) else {
            continue;
        };
        let Some(state) = program.machine_states(machine).first() else {
            continue;
        };
        let [receiver] = program.state_parameters(state) else {
            continue;
        };
        let Some(shell) = crate::reference_shell(program, receiver.type_reference) else {
            continue;
        };
        if program.normalized_type_identity(shell.referee) != identity {
            continue;
        }
        if selected.is_some() {
            return None;
        }
        selected = Some(machine);
    }
    selected
}

/// Effective owner -> cleanup machine symbols, computed once per program.
/// A machine qualifies for `owner` when its own cleanup owner is `owner`, or
/// when that owner's origin specializes the generic `owner` base.
fn nominal_cleanup_candidates(
    program: &dyn crate::CheckingInput,
) -> std::sync::Arc<symbols::SymbolKeyMap<symbols::SymbolHandle, Vec<symbols::SymbolHandle>>> {
    crate::frozen_program::frozen_memo(
        program,
        |memos| &memos.nominal_cleanup_machine_index,
        || {
            let mut index: symbols::SymbolKeyMap<
                symbols::SymbolHandle,
                Vec<symbols::SymbolHandle>,
            > = Default::default();
            for machine in program.machines() {
                let Some(candidate_owner) = nominal_cleanup_owner(program, machine.symbol) else {
                    continue;
                };
                index
                    .entry(candidate_owner)
                    .or_default()
                    .push(machine.symbol);
                if let Some(origin) = program
                    .unique_data_definition_by_symbol(candidate_owner)
                    .and_then(|data| data.generic_instance)
                    && let TypeReferenceNode::Generic { base_symbol, .. } =
                        program.type_reference_table.type_reference(origin)
                    && *base_symbol != candidate_owner
                {
                    index.entry(*base_symbol).or_default().push(machine.symbol);
                }
            }
            index
        },
    )
}

/// The first reserved cleanup machine, in program order, that
/// `selected_symbol` names.
fn reserved_cleanup_selected_by<'program>(
    program: &dyn crate::CheckingInput,
    cleanups: &[&'program Machine],
    selected_symbol: SymbolHandle,
) -> Option<&'program Machine> {
    // At most one machine can ever match: the selected symbol is either the
    // machine itself or one of its states — and a state's retained parent
    // names its owning machine directly, so real selections collapse to a
    // single candidate. Handles without a symbol-table row (test fixtures
    // forge them) keep the machine/state handle scan.
    let machine_symbol = match program.symbols.get(selected_symbol).kind {
        symbols::SymbolKind::Machine => Some(selected_symbol),
        symbols::SymbolKind::State => Some(program.symbols.get(selected_symbol).parent),
        _ => None,
    };
    cleanups.iter().copied().find(|machine| {
        if let Some(machine_symbol) = machine_symbol {
            machine.symbol == machine_symbol
        } else {
            program.machine_or_state_symbol(machine, selected_symbol)
        }
    })
}

fn validate_cleanup_machine(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(_attached_name) = machine.attached_data.as_ref() else {
        return;
    };
    // The explicit exclusive parameter names the exact data owner. A qualified
    // machine path contributes no receiver or alternative owner identity.
    let attached_symbol = machine.attached_data_symbol;
    let Some(entry) = program.machine_states(machine).first() else {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` has no entry state; reserved cleanup takes one exclusive loan of its own data and returns Unit",
            machine.name
        )));
        return;
    };
    let parameters = program.state_parameters(entry);
    // The shape is one exclusive loan of the machine's own data and nothing
    // else. Owner applications keep the declaration's entire telescope in its
    // original positions; a specialization or permuted region list cannot
    // designate the universal cleanup role of the template.
    let shape_is_exact = matches!(parameters, [owner]
    if owner.is_mutable
        && !owner.is_const
            && mutable_reference_targets(program, owner.type_reference, attached_symbol));
    if !shape_is_exact {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` must take exactly one exclusive loan of its own data \
             and no other parameters, as `drop(value: &mut {})`",
            machine.name,
            machine
                .attached_data
                .as_ref()
                .map(|name| name.as_str())
                .unwrap_or("Data")
        )));
    }
    if !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
    {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` may not declare method-local lifetime or type parameters",
            machine.name
        )));
    }
    if entry.return_type.is_valid()
        && !matches!(
            program
                .type_reference_table
                .type_reference(entry.return_type),
            TypeReferenceNode::Unit
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` must return Unit; fallible cleanup is an explicit consuming machine",
            machine.name
        )));
    }
    if machine.suspends || machine.blocks {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` must be non-suspending and nonblocking",
            machine.name
        )));
    }
    if program
        .machine_contracts(machine)
        .iter()
        .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
    {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` may not declare a crash outcome",
            machine.name
        )));
    }
    if machine.supply_mode != MachineSupplyMode::CheckedBody
        && !matches!(
            machine.termination_plan.interface,
            TerminationInterface::Published(TerminationGuarantee::Terminates { .. })
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "bodyless cleanup machine `{}` must publish `terminates;`",
            machine.name
        )));
    }
}

fn mutable_reference_targets(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    attached_symbol: symbols::SymbolHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            mutable_reference_targets(program, *base_type, attached_symbol)
        }
        TypeReferenceNode::Reference {
            referee,
            access: ReferenceAccess::Mutable,
            ..
        } => owner_type_matches(program, *referee, attached_symbol),
        _ => false,
    }
}

/// Match the nominal owner and its identity application, not just its spelling
/// or generic head. Declaration binders are implicit inputs of the owner hook;
/// method-local binders remain forbidden. Regions keep their erased binder
/// coordinates while type, value and callable operands use resolved symbols.
fn owner_type_matches(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    attached_symbol: symbols::SymbolHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            owner_type_matches(program, *base_type, attached_symbol)
        }
        TypeReferenceNode::Named { symbol, .. } => {
            attached_symbol.is_valid() && *symbol == attached_symbol
        }
        TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } => {
            let Some(owner) = program.data_definition_by_symbol(attached_symbol) else {
                return false;
            };
            let parameters = program.data_type_parameters(owner);
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            *base_symbol == attached_symbol
                && lifetime_arguments == &owner.lifetime_parameters
                && arguments.len() == parameters.len()
                && arguments
                    .iter()
                    .zip(parameters)
                    .all(|(argument, parameter)| {
                        match program.type_reference_table.type_reference(*argument) {
                            TypeReferenceNode::Named { symbol, .. } => *symbol == parameter.symbol,
                            TypeReferenceNode::ConstExpression(expression)
                                if parameter.kind.is_const_or_value() =>
                            {
                                program
                                    .expression_table
                                    .name_path_node(*expression)
                                    .is_some_and(|name| name.symbol == parameter.symbol)
                            }
                            _ => false,
                        }
                    })
        }
        _ => false,
    }
}
