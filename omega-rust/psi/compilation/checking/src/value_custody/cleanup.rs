//! Owner hooks remain ordinary machines with compiler-selected invocations.
//! An authored call cannot select that reserved role. Once invoked, however,
//! the hook may transition between its own states: those exact selections
//! select control-flow destinations, not another invocation of the hook.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionTarget,
};
use language_semantics::{
    MachineSupplyMode, ReferenceAccess, TerminationGuarantee, TerminationInterface,
};
use std::collections::BTreeSet;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// True when automatic disposal would have to run a reachable nominal
/// `::drop`. Borrowed views do not own referent cleanup; callers assessing an
/// owned receiver must pass its referent type rather than its loan type.
pub fn type_graph_requires_nominal_drop(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    type_graph_requires_nominal_drop_with_substitutions(
        program,
        type_reference,
        &[],
        &mut BTreeSet::new(),
    )
}

fn type_graph_requires_nominal_drop_with_substitutions(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visited: &mut BTreeSet<String>,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => return false,
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    break;
                };
                if *replacement == type_reference {
                    return false;
                }
                type_reference = *replacement;
            }
            _ => break,
        }
    }

    let identity = program
        .type_identity(TypeIdentityRequest {
            substitutions,
            ..TypeIdentityRequest::ordinary(type_reference)
        })
        .into_string();
    if !visited.insert(identity) {
        return false;
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => false,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_graph_requires_nominal_drop_with_substitutions(
                program,
                *base_type,
                substitutions,
                visited,
            )
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_graph_requires_nominal_drop_with_substitutions(
                program,
                *element_type,
                substitutions,
                visited,
            )
        }
        TypeReferenceNode::Named { symbol, name } => program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol || data.name.as_str() == name.as_str())
            .is_some_and(|data| {
                data_graph_requires_nominal_drop_with_substitutions(
                    program,
                    data,
                    substitutions,
                    visited,
                )
            }),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let Some(data) = crate::program_scope::data_definitions::data_definition_by_symbol(
                program,
                *base_symbol,
            ) else {
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
            data_graph_requires_nominal_drop_with_substitutions(
                program,
                data,
                &nested_substitutions,
                visited,
            )
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

/// Check cleanup of an exact nominal owner, including its owned fields.
/// An implicit `Self` type names its machine, so receiver provisioning uses
/// the machine's resolved data declaration rather than inspecting the loan.
pub fn data_requires_nominal_drop(
    program: &TypedTrees,
    data: &typed_trees::data::DataDefinition,
) -> bool {
    data_graph_requires_nominal_drop_with_substitutions(program, data, &[], &mut BTreeSet::new())
}

fn data_graph_requires_nominal_drop_with_substitutions(
    program: &TypedTrees,
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
                    && type_graph_requires_nominal_drop_with_substitutions(
                        program,
                        field.type_reference,
                        substitutions,
                        visited,
                    )
            }
            DataMember::Variant(variant) => {
                program.data_payload_fields(variant).iter().any(|field| {
                    !field.relevance.is_erased()
                        && type_graph_requires_nominal_drop_with_substitutions(
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
    program: &TypedTrees,
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
    program: &TypedTrees,
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
        let AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
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
    program: &TypedTrees,
    cleanups: &[&Machine],
) -> BTreeSet<AuthoredDeclarationSelectionOccurrenceId> {
    let mut transitions = BTreeSet::new();
    for machine in cleanups {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::Transition(transition) = statement else {
                    continue;
                };
                for target in [transition.target, transition.continuation] {
                    if !target.is_valid() {
                        continue;
                    }
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

pub fn validate_reserved_cleanup_selections(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
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
fn is_reserved_cleanup(program: &TypedTrees, machine: &Machine) -> bool {
    let owner = machine.attached_data_symbol;
    machine.attached_data.is_some()
        && owner.is_valid()
        && machine.name.as_str().rsplit("::").next() == Some("drop")
        && crate::program_scope::data_definitions::data_definition_by_symbol(program, owner)
            .is_some()
}

/// Recover the owner-designated cleanup role from an exact checked declaration.
/// Specialization keeps the template's reserved role and the instance's owner;
/// an attached borrowed method or a similarly named free function gains none.
/// Declaration validation separately enforces the full owner and hook contract.
pub fn nominal_cleanup_owner(program: &TypedTrees, symbol: SymbolHandle) -> Option<SymbolHandle> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)?;
    let mut declaration = machine;
    let mut visited = Vec::new();
    while !is_reserved_cleanup(program, declaration) {
        if visited.contains(&declaration.symbol) {
            return None;
        }
        visited.push(declaration.symbol);
        let template = program
            .machine_specializations
            .iter()
            .find(|specialization| specialization.instance == declaration.symbol)?
            .template;
        declaration = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == template)?;
    }
    machine
        .attached_data_symbol
        .is_valid()
        .then_some(machine.attached_data_symbol)
}

/// The first reserved cleanup machine, in program order, that
/// `selected_symbol` names.
fn reserved_cleanup_selected_by<'program>(
    program: &TypedTrees,
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
            machine.symbol == selected_symbol
                || program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == selected_symbol)
        }
    })
}

fn validate_cleanup_machine(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(_attached_name) = machine.attached_data.as_ref() else {
        return;
    };
    // A receiver's synthesized type points at the machine, while a named
    // parameter's `&mut Guard` points at the DATA the machine is attached to.
    // Both denote the same storage, so the shape test accepts either target.
    let attached_symbol = machine.attached_data_symbol;
    let Some(entry) = program.machine_states(machine).first() else {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` has no entry state; reserved cleanup has shape `drop(&mut self) -> ()`",
            machine.name
        )));
        return;
    };
    let parameters = program.state_parameters(entry);
    // The shape is one exclusive loan of the machine's own data and nothing
    // else. A receiver and a named parameter spell that same shape, and under
    // the static-machine contract a qualified path "contributes no argument or
    // storage", so which spelling carries it is not cleanup's business. Every
    // other condition is unchanged: exactly one parameter, mutable, not const,
    // and a mutable reference to this machine's data.
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
    program: &TypedTrees,
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
        } => named_type_matches(program, *referee, attached_symbol),
        _ => false,
    }
}

/// Whether a reference's referee names this cleanup machine's own data,
/// written as `&mut Guard`. The symbol identifies the storage, so the
/// spelling is not compared.
fn named_type_matches(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    attached_symbol: symbols::SymbolHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            named_type_matches(program, *base_type, attached_symbol)
        }
        TypeReferenceNode::Named { symbol, .. } => {
            attached_symbol.is_valid() && *symbol == attached_symbol
        }
        _ => false,
    }
}
