//! Compile-time specialization of generic machines.
//!
//! Type parameters and static machine parameters are one specialization
//! tuple. The first concrete tuple reuses the authored declaration; every
//! additional tuple receives a deep-copied body with fresh lexical symbols.
//! Calls are rewritten to their selected concrete state, calls through
//! `F(...)` become direct calls to the selected entry, and each tuple records
//! a deterministic cache identity. Incomplete tuples remain generic and are
//! fenced by validation; no runtime callable value or dictionary is
//! introduced.

use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::{SymbolHandle, SymbolKind};
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::TypeParameterKind;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use omega_typed_trees::signature::StateSignature;
use omega_typed_trees::statement::{StatementHandle, StatementNode};
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone)]
struct Candidate {
    machine_index: usize,
    template_symbol: SymbolHandle,
    template_name: String,
    state_symbols: Vec<SymbolHandle>,
    type_parameters: Vec<(SymbolHandle, String)>,
    parameter_bounds: Vec<Vec<omega_validation::DeclaredPropertyRequirement>>,
    type_bindings: Vec<Option<TypeReferenceHandle>>,
    machine_parameters: Vec<(SymbolHandle, String, StateSignature)>,
    machine_bindings: Vec<Option<StaticMachineArgument>>,
    conflicted: bool,
}

struct CalleeState {
    symbol: SymbolHandle,
    name: String,
    candidate_index: usize,
    return_type: TypeReferenceHandle,
    parameter_types: Vec<TypeReferenceHandle>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallSite {
    Statement(StatementHandle),
    Expression(ExpressionHandle),
}

#[derive(Clone)]
struct CallSelection {
    site: CallSite,
    callee_symbol: SymbolHandle,
    candidate_index: usize,
    type_bindings: Vec<Option<TypeReferenceHandle>>,
    machine_bindings: Vec<Option<StaticMachineArgument>>,
    conflicted: bool,
}

impl CallSelection {
    fn is_complete(&self) -> bool {
        !self.conflicted
            && self.type_bindings.iter().all(Option::is_some)
            && self.machine_bindings.iter().all(Option::is_some)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct SpecializationKey {
    type_arguments: Vec<String>,
    machine_arguments: Vec<SymbolHandle>,
}

pub(crate) fn monomorphize_generic_machine_value_calls(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut candidates = Vec::new();
    let mut callee_states = Vec::new();
    let mut all_type_parameter_symbols = Vec::new();

    for (machine_index, machine) in program.machines().iter().enumerate() {
        let parameters = program.machine_type_parameters(machine);
        if parameters.is_empty() {
            continue;
        }

        let mut type_parameters = Vec::new();
        let mut parameter_bounds = Vec::new();
        let mut machine_parameters = Vec::new();
        for parameter in parameters {
            match &parameter.kind {
                TypeParameterKind::Type => {
                    type_parameters.push((parameter.symbol, parameter.name.as_str().to_owned()));
                    parameter_bounds.push(omega_validation::declared_property_requirements(
                        &parameter.bounds,
                    ));
                }
                TypeParameterKind::Machine { contract } => machine_parameters.push((
                    parameter.symbol,
                    parameter.name.as_str().to_owned(),
                    contract.clone(),
                )),
                TypeParameterKind::Const { .. } => {
                    // Const-machine specialization is a separate rung. Keep
                    // this template incomplete so validation/backend fences it.
                }
            }
        }
        all_type_parameter_symbols.extend(type_parameters.iter().cloned());

        let candidate_index = candidates.len();
        let states = program.machine_states(machine);
        for state in states {
            callee_states.push(CalleeState {
                symbol: state.symbol,
                name: state.name.as_str().to_owned(),
                candidate_index,
                return_type: state.return_type,
                parameter_types: program
                    .state_parameters(state)
                    .iter()
                    .map(|parameter| parameter.type_reference)
                    .collect(),
            });
        }
        candidates.push(Candidate {
            machine_index,
            template_symbol: machine.symbol,
            template_name: machine.name.as_str().to_owned(),
            state_symbols: states.iter().map(|state| state.symbol).collect(),
            type_bindings: vec![None; type_parameters.len()],
            machine_bindings: vec![None; machine_parameters.len()],
            type_parameters,
            parameter_bounds,
            machine_parameters,
            conflicted: false,
        });
    }
    if candidates.is_empty() {
        return Ok(());
    }

    let mut type_proposals = Vec::new();
    let mut machine_proposals = Vec::new();

    // Static selections may occur in any expression position. Their symbols
    // alone are sufficient to bind machine parameters and to infer type
    // parameters from requirement/implementation shape.
    for (_, expression) in program.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = expression else {
            continue;
        };
        collect_machine_proposals(
            program,
            &candidates,
            &callee_states,
            call.target_symbol,
            call.target.as_str(),
            &call.machine_arguments,
            &mut machine_proposals,
            &mut type_proposals,
        );
    }

    // Statement calls additionally provide parameter-position type inference.
    // Annotated locals provide both parameter- and return-position inference.
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Call(call) => collect_call_proposals(
                        program,
                        machine,
                        state,
                        &candidates,
                        &callee_states,
                        call.target_symbol,
                        call.target.as_str(),
                        &call.machine_arguments,
                        program.statement_table.expression_handles(call.arguments),
                        None,
                        &mut machine_proposals,
                        &mut type_proposals,
                    ),
                    StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                        if let ExpressionNode::Call(call) =
                            program.expression_table.expression(local.initial_value)
                        {
                            collect_call_proposals(
                                program,
                                machine,
                                state,
                                &candidates,
                                &callee_states,
                                call.target_symbol,
                                call.target.as_str(),
                                &call.machine_arguments,
                                program.expression_table.expression_handles(call.arguments),
                                local
                                    .type_reference
                                    .is_valid()
                                    .then_some(local.type_reference),
                                &mut machine_proposals,
                                &mut type_proposals,
                            );
                        }
                    }
                    StatementNode::Expression(expression) => {
                        if let ExpressionNode::Call(call) =
                            program.expression_table.expression(*expression)
                        {
                            collect_call_proposals(
                                program,
                                machine,
                                state,
                                &candidates,
                                &callee_states,
                                call.target_symbol,
                                call.target.as_str(),
                                &call.machine_arguments,
                                program.expression_table.expression_handles(call.arguments),
                                None,
                                &mut machine_proposals,
                                &mut type_proposals,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Keep every concrete call-site tuple distinct. The aggregate proposal
    // path below remains the cheap single-tuple case; this ledger is what
    // lets the multi-tuple path clone once per unique specialization and
    // rewrite only the calls that selected it.
    let selections = collect_call_selections(program, &candidates, &callee_states);

    for (candidate_index, parameter_index, binding) in type_proposals {
        if type_reference_is_still_generic(program, binding, &all_type_parameter_symbols) {
            continue;
        }
        let candidate = &mut candidates[candidate_index];
        match candidate.type_bindings[parameter_index] {
            None => candidate.type_bindings[parameter_index] = Some(binding),
            Some(existing) if !same_type_display(program, existing, binding) => {
                candidate.conflicted = true;
            }
            Some(_) => {}
        }
    }

    for (candidate_index, parameter_index, binding) in machine_proposals {
        let candidate = &mut candidates[candidate_index];
        match &candidate.machine_bindings[parameter_index] {
            None => candidate.machine_bindings[parameter_index] = Some(binding),
            Some(existing) if existing.symbol != binding.symbol => candidate.conflicted = true,
            Some(_) => {}
        }
    }

    let multi_tuple_candidates: Vec<usize> = (0..candidates.len())
        .filter(|candidate_index| {
            unique_complete_selections(program, &selections, *candidate_index).len() > 1
        })
        .collect();

    let approved = approved_type_bounds(program, &candidates);
    let mut diagnostics = Vec::new();
    for (candidate_index, approved) in approved.into_iter().enumerate() {
        if multi_tuple_candidates.contains(&candidate_index) {
            continue;
        }
        let candidate = candidates[candidate_index].clone();
        let has_static_selection = candidate.machine_bindings.iter().any(Option::is_some);
        let has_incomplete_call = selections.iter().any(|selection| {
            selection.candidate_index == candidate_index && !selection.is_complete()
        });
        if has_incomplete_call {
            if has_static_selection {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` has a static machine selection, but its complete type/machine specialization tuple cannot be derived",
                    candidate.template_name
                )));
            }
            continue;
        }
        if candidate.conflicted {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` has conflicting specialization evidence that cannot be assigned to complete concrete call-site tuples; make each call's type/result evidence explicit",
                candidate.template_name
            )));
            continue;
        }
        if has_static_selection
            && (candidate.type_bindings.iter().any(Option::is_none)
                || candidate.machine_bindings.iter().any(Option::is_none))
        {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` has a static machine selection, but its complete type/machine specialization tuple cannot be derived",
                candidate.template_name
            )));
            continue;
        }
        if !approved
            || candidate.type_bindings.iter().any(Option::is_none)
            || candidate.machine_bindings.iter().any(Option::is_none)
        {
            continue;
        }
        apply_specialization(program, &candidate);
    }

    if diagnostics.is_empty() {
        for candidate_index in multi_tuple_candidates {
            if let Err(mut errors) = apply_multiple_specializations(
                program,
                &candidates[candidate_index],
                &selections,
                candidate_index,
            ) {
                diagnostics.append(&mut errors);
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_call_proposals(
    program: &TypedTrees,
    caller_machine: &omega_typed_trees::machine::Machine,
    caller_state: &omega_typed_trees::state::State,
    candidates: &[Candidate],
    callee_states: &[CalleeState],
    target_symbol: SymbolHandle,
    target_name: &str,
    machine_arguments: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
    expected_return: Option<TypeReferenceHandle>,
    machine_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    let Some(callee) = resolve_callee(callee_states, target_symbol, target_name) else {
        return;
    };
    collect_machine_proposals_for_callee(
        program,
        candidates,
        callee,
        machine_arguments,
        machine_proposals,
        type_proposals,
    );

    let candidate = &candidates[callee.candidate_index];
    let skip = callee.parameter_types.len().saturating_sub(arguments.len());
    for (argument, required) in arguments
        .iter()
        .zip(callee.parameter_types.iter().skip(skip))
    {
        let Some(actual) = omega_validation::declared_place_type_raw(
            program,
            caller_machine,
            Some(caller_state),
            *argument,
        ) else {
            continue;
        };
        infer_type_bindings(
            program,
            *required,
            actual,
            &candidate.type_parameters,
            callee.candidate_index,
            type_proposals,
        );
    }
    if let Some(actual) = expected_return {
        infer_type_bindings(
            program,
            callee.return_type,
            actual,
            &candidate.type_parameters,
            callee.candidate_index,
            type_proposals,
        );
    }
}

fn collect_machine_proposals(
    program: &TypedTrees,
    candidates: &[Candidate],
    callee_states: &[CalleeState],
    target_symbol: SymbolHandle,
    target_name: &str,
    machine_arguments: &[StaticMachineArgument],
    machine_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    let Some(callee) = resolve_callee(callee_states, target_symbol, target_name) else {
        return;
    };
    collect_machine_proposals_for_callee(
        program,
        candidates,
        callee,
        machine_arguments,
        machine_proposals,
        type_proposals,
    );
}

fn collect_machine_proposals_for_callee(
    program: &TypedTrees,
    candidates: &[Candidate],
    callee: &CalleeState,
    machine_arguments: &[StaticMachineArgument],
    machine_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    let candidate = &candidates[callee.candidate_index];
    if machine_arguments.len() != candidate.machine_parameters.len() {
        return;
    }
    for (index, selected) in machine_arguments.iter().enumerate() {
        if !selected.symbol.is_valid() {
            continue;
        }
        machine_proposals.push((callee.candidate_index, index, selected.clone()));
        let requirement = &candidate.machine_parameters[index].2;
        let Some(actual_state) = state_by_symbol(program, selected.symbol) else {
            continue;
        };
        for (required, actual) in program
            .state_signature_parameters(requirement)
            .iter()
            .zip(program.state_parameters(actual_state))
        {
            infer_type_bindings(
                program,
                required.type_reference,
                actual.type_reference,
                &candidate.type_parameters,
                callee.candidate_index,
                type_proposals,
            );
        }
        infer_type_bindings(
            program,
            requirement.return_type,
            actual_state.return_type,
            &candidate.type_parameters,
            callee.candidate_index,
            type_proposals,
        );
    }
}

fn resolve_callee<'a>(
    callee_states: &'a [CalleeState],
    target_symbol: SymbolHandle,
    target_name: &str,
) -> Option<&'a CalleeState> {
    callee_states
        .iter()
        .find(|callee| target_symbol.is_valid() && callee.symbol == target_symbol)
        .or_else(|| {
            let mut matching = callee_states
                .iter()
                .filter(|callee| callee.name == target_name);
            match (matching.next(), matching.next()) {
                (Some(only), None) => Some(only),
                _ => None,
            }
        })
}

fn state_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::state::State> {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|state| state.symbol == symbol)
}

fn collect_call_selections(
    program: &TypedTrees,
    candidates: &[Candidate],
    callee_states: &[CalleeState],
) -> Vec<CallSelection> {
    let mut selections = Vec::new();
    let mut covered_expressions = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for offset in 0..state.statement_nodes.count() {
                let handle = Handle::from_parts(
                    state.statement_nodes.start().arena_index() + offset,
                    state.statement_nodes.start().generation(),
                );
                match program.statement_table.statement(handle) {
                    StatementNode::Call(call) => {
                        if let Some(selection) = selection_for_call(
                            program,
                            machine,
                            state,
                            candidates,
                            callee_states,
                            CallSite::Statement(handle),
                            call.target_symbol,
                            call.target.as_str(),
                            &call.machine_arguments,
                            program.statement_table.expression_handles(call.arguments),
                            None,
                        ) {
                            upsert_selection(&mut selections, selection);
                        }
                    }
                    StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                        if let ExpressionNode::Call(call) =
                            program.expression_table.expression(local.initial_value)
                        {
                            covered_expressions.push(local.initial_value);
                            if let Some(selection) = selection_for_call(
                                program,
                                machine,
                                state,
                                candidates,
                                callee_states,
                                CallSite::Expression(local.initial_value),
                                call.target_symbol,
                                call.target.as_str(),
                                &call.machine_arguments,
                                program.expression_table.expression_handles(call.arguments),
                                local
                                    .type_reference
                                    .is_valid()
                                    .then_some(local.type_reference),
                            ) {
                                upsert_selection(&mut selections, selection);
                            }
                        }
                    }
                    StatementNode::Expression(expression) => {
                        if let ExpressionNode::Call(call) =
                            program.expression_table.expression(*expression)
                        {
                            covered_expressions.push(*expression);
                            if let Some(selection) = selection_for_call(
                                program,
                                machine,
                                state,
                                candidates,
                                callee_states,
                                CallSite::Expression(*expression),
                                call.target_symbol,
                                call.target.as_str(),
                                &call.machine_arguments,
                                program.expression_table.expression_handles(call.arguments),
                                None,
                            ) {
                                upsert_selection(&mut selections, selection);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Nested value calls may not have a local result annotation, but explicit
    // static-machine arguments still determine a complete tuple through the
    // authored machine requirement. Preserve the old all-expression scan for
    // precisely that case.
    for (handle, expression) in program.expression_table.iter_expressions() {
        if covered_expressions.contains(&handle) {
            continue;
        }
        let ExpressionNode::Call(call) = expression else {
            continue;
        };
        let Some(callee) = resolve_callee(callee_states, call.target_symbol, call.target.as_str())
        else {
            continue;
        };
        let candidate = &candidates[callee.candidate_index];
        let mut machine_proposals = Vec::new();
        let mut type_proposals = Vec::new();
        collect_machine_proposals_for_callee(
            program,
            candidates,
            callee,
            &call.machine_arguments,
            &mut machine_proposals,
            &mut type_proposals,
        );
        let selection = selection_from_proposals(
            program,
            CallSite::Expression(handle),
            callee,
            candidate,
            machine_proposals,
            type_proposals,
        );
        upsert_selection(&mut selections, selection);
    }

    selections
}

#[allow(clippy::too_many_arguments)]
fn selection_for_call(
    program: &TypedTrees,
    caller_machine: &omega_typed_trees::machine::Machine,
    caller_state: &omega_typed_trees::state::State,
    candidates: &[Candidate],
    callee_states: &[CalleeState],
    site: CallSite,
    target_symbol: SymbolHandle,
    target_name: &str,
    machine_arguments: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
    expected_return: Option<TypeReferenceHandle>,
) -> Option<CallSelection> {
    let callee = resolve_callee(callee_states, target_symbol, target_name)?;
    let candidate = &candidates[callee.candidate_index];
    let mut machine_proposals = Vec::new();
    let mut type_proposals = Vec::new();
    collect_call_proposals(
        program,
        caller_machine,
        caller_state,
        candidates,
        callee_states,
        target_symbol,
        target_name,
        machine_arguments,
        arguments,
        expected_return,
        &mut machine_proposals,
        &mut type_proposals,
    );
    Some(selection_from_proposals(
        program,
        site,
        callee,
        candidate,
        machine_proposals,
        type_proposals,
    ))
}

fn selection_from_proposals(
    program: &TypedTrees,
    site: CallSite,
    callee: &CalleeState,
    candidate: &Candidate,
    machine_proposals: Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: Vec<(usize, usize, TypeReferenceHandle)>,
) -> CallSelection {
    let mut selection = CallSelection {
        site,
        callee_symbol: callee.symbol,
        candidate_index: callee.candidate_index,
        type_bindings: vec![None; candidate.type_parameters.len()],
        machine_bindings: vec![None; candidate.machine_parameters.len()],
        conflicted: false,
    };
    for (_, parameter, binding) in type_proposals {
        if type_reference_is_any_generic_parameter(program, binding) {
            continue;
        }
        match selection.type_bindings[parameter] {
            None => selection.type_bindings[parameter] = Some(binding),
            Some(existing) if !same_type_display(program, existing, binding) => {
                selection.conflicted = true
            }
            Some(_) => {}
        }
    }
    for (_, parameter, binding) in machine_proposals {
        match &selection.machine_bindings[parameter] {
            None => selection.machine_bindings[parameter] = Some(binding),
            Some(existing) if existing.symbol != binding.symbol => selection.conflicted = true,
            Some(_) => {}
        }
    }
    selection
}

fn type_reference_is_any_generic_parameter(
    program: &TypedTrees,
    binding: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(binding)
    else {
        return false;
    };
    program.machines().iter().any(|machine| {
        program
            .machine_type_parameters(machine)
            .iter()
            .any(|parameter| {
                matches!(parameter.kind, TypeParameterKind::Type)
                    && (parameter.symbol == *symbol
                        || (!parameter.symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter.name.as_str() == name.as_str()))
            })
    })
}

fn upsert_selection(selections: &mut Vec<CallSelection>, selection: CallSelection) {
    if let Some(existing) = selections
        .iter_mut()
        .find(|existing| existing.site == selection.site)
    {
        let existing_evidence = existing
            .type_bindings
            .iter()
            .filter(|item| item.is_some())
            .count()
            + existing
                .machine_bindings
                .iter()
                .filter(|item| item.is_some())
                .count();
        let new_evidence = selection
            .type_bindings
            .iter()
            .filter(|item| item.is_some())
            .count()
            + selection
                .machine_bindings
                .iter()
                .filter(|item| item.is_some())
                .count();
        if new_evidence >= existing_evidence {
            *existing = selection;
        }
    } else {
        selections.push(selection);
    }
}

fn unique_complete_selections(
    program: &TypedTrees,
    selections: &[CallSelection],
    candidate_index: usize,
) -> Vec<(SpecializationKey, Vec<usize>)> {
    let mut groups: Vec<(SpecializationKey, Vec<usize>)> = Vec::new();
    for (selection_index, selection) in selections.iter().enumerate() {
        if selection.candidate_index != candidate_index || !selection.is_complete() {
            continue;
        }
        let key = SpecializationKey {
            type_arguments: selection
                .type_bindings
                .iter()
                .map(|binding| program.display_type_reference(binding.expect("complete selection")))
                .collect(),
            machine_arguments: selection
                .machine_bindings
                .iter()
                .map(|binding| binding.as_ref().expect("complete selection").symbol)
                .collect(),
        };
        if let Some((_, members)) = groups.iter_mut().find(|(existing, _)| *existing == key) {
            members.push(selection_index);
        } else {
            groups.push((key, vec![selection_index]));
        }
    }
    groups
}

fn infer_type_bindings(
    program: &TypedTrees,
    required: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    parameters: &[(SymbolHandle, String)],
    candidate_index: usize,
    proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    if !required.is_valid() || !actual.is_valid() {
        return;
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
        && let Some(index) = parameters
            .iter()
            .position(|(parameter_symbol, parameter_name)| {
                parameter_symbol == symbol
                    || (!parameter_symbol.is_valid()
                        && !symbol.is_valid()
                        && parameter_name == name.as_str())
            })
    {
        proposals.push((candidate_index, index, actual));
        return;
    }

    match (
        program.type_reference_table.type_reference(required),
        program.type_reference_table.type_reference(actual),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: required, ..
            },
            TypeReferenceNode::Reference {
                referee: actual, ..
            },
        ) => infer_type_bindings(
            program,
            *required,
            *actual,
            parameters,
            candidate_index,
            proposals,
        ),
        // Borrow syntax is carried by the CALL edge, not as a wrapper on the
        // place expression itself. Thus `f<T>(&T)` called as `f(&place)` sees
        // the declared type of `place` here. Peel the requirement-side borrow
        // and infer from that place type; ordinary call validation separately
        // checks that the authored borrow mode is legal.
        (
            TypeReferenceNode::Reference {
                referee: required, ..
            },
            _,
        ) => infer_type_bindings(
            program,
            *required,
            actual,
            parameters,
            candidate_index,
            proposals,
        ),
        (TypeReferenceNode::Constrained { base_type, .. }, _) => infer_type_bindings(
            program,
            *base_type,
            omega_validation::unwrapped_type_reference(program, actual).unwrap_or(actual),
            parameters,
            candidate_index,
            proposals,
        ),
        (
            TypeReferenceNode::Slice {
                element_type: required,
            },
            TypeReferenceNode::Slice {
                element_type: actual,
            },
        )
        | (
            TypeReferenceNode::FixedArray {
                element_type: required,
                ..
            },
            TypeReferenceNode::FixedArray {
                element_type: actual,
                ..
            },
        ) => infer_type_bindings(
            program,
            *required,
            *actual,
            parameters,
            candidate_index,
            proposals,
        ),
        (
            TypeReferenceNode::Generic {
                base_name: required_base,
                arguments: required_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_name: actual_base,
                arguments: actual_arguments,
                ..
            },
        ) if required_base == actual_base => {
            for (required, actual) in program
                .type_reference_table
                .type_reference_handles(*required_arguments)
                .iter()
                .zip(
                    program
                        .type_reference_table
                        .type_reference_handles(*actual_arguments),
                )
            {
                infer_type_bindings(
                    program,
                    *required,
                    *actual,
                    parameters,
                    candidate_index,
                    proposals,
                );
            }
        }
        _ => {}
    }
}

fn type_reference_is_still_generic(
    program: &TypedTrees,
    binding: TypeReferenceHandle,
    all_parameters: &[(SymbolHandle, String)],
) -> bool {
    matches!(
        program.type_reference_table.type_reference(binding),
        TypeReferenceNode::Named { symbol, name }
            if all_parameters.iter().any(|(parameter_symbol, parameter_name)| {
                parameter_symbol == symbol
                    || (!parameter_symbol.is_valid()
                        && !symbol.is_valid()
                        && parameter_name == name.as_str())
            })
    )
}

fn same_type_display(
    program: &TypedTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    program
        .type_reference_table
        .display_name_with_constraints(left, &program.expression_table)
        == program
            .type_reference_table
            .display_name_with_constraints(right, &program.expression_table)
}

fn approved_type_bounds(program: &TypedTrees, candidates: &[Candidate]) -> Vec<bool> {
    let mut symbol_diagnostics = Vec::new();
    let symbols = omega_validation::TopLevelSymbols::build(program, &mut symbol_diagnostics);
    candidates
        .iter()
        .map(|candidate| {
            candidate
                .parameter_bounds
                .iter()
                .zip(candidate.type_bindings.iter())
                .all(|(bounds, binding)| {
                    let Some(binding) = binding else {
                        return true;
                    };
                    let Some(unwrapped) =
                        omega_validation::unwrapped_type_reference(program, *binding)
                    else {
                        return false;
                    };
                    bounds.iter().all(|property| {
                        omega_validation::type_satisfies_declared_property(
                            program,
                            &symbols,
                            &[],
                            unwrapped,
                            *property,
                        )
                    })
                })
        })
        .collect()
}

fn apply_multiple_specializations(
    program: &mut TypedTrees,
    template: &Candidate,
    selections: &[CallSelection],
    candidate_index: usize,
) -> Result<(), Vec<Diagnostic>> {
    let groups = unique_complete_selections(program, selections, candidate_index);
    if groups.len() < 2 {
        return Ok(());
    }

    let concrete_candidates: Vec<Candidate> = groups
        .iter()
        .map(|(_, members)| candidate_for_selection(template, &selections[members[0]]))
        .collect();
    if approved_type_bounds(program, &concrete_candidates)
        .iter()
        .any(|approved| !approved)
    {
        return Err(vec![Diagnostic::error(format!(
            "generic machine `{}` has a concrete specialization tuple that does not satisfy its authored type bounds",
            template.template_name
        ))]);
    }

    if selections
        .iter()
        .any(|selection| selection.candidate_index == candidate_index && !selection.is_complete())
    {
        return Err(vec![Diagnostic::error(format!(
            "generic machine `{}` has a static machine selection, but its complete type/machine specialization tuple cannot be derived",
            template.template_name
        ))]);
    }

    // Clones must be sourced from the untouched generic graph. The first
    // tuple reuses the authored declaration in place; subsequent tuples are
    // copied from this snapshot, receive fresh lexical symbols, and are then
    // rewritten independently.
    let source = program.clone();
    apply_specialization(program, &concrete_candidates[0]);

    for (group_index, ((_, members), candidate)) in groups
        .iter()
        .zip(concrete_candidates.iter())
        .enumerate()
        .skip(1)
    {
        let state_symbols = clone_specialized_machine(&source, program, candidate, group_index);
        for selection_index in members {
            let selection = &selections[*selection_index];
            let Some((_, concrete_state)) = state_symbols
                .iter()
                .find(|(template_state, _)| *template_state == selection.callee_symbol)
            else {
                continue;
            };
            rewrite_selected_call(program, selection.site, *concrete_state);
        }
    }

    Ok(())
}

fn candidate_for_selection(template: &Candidate, selection: &CallSelection) -> Candidate {
    let mut candidate = template.clone();
    candidate.type_bindings = selection.type_bindings.clone();
    candidate.machine_bindings = selection.machine_bindings.clone();
    candidate.conflicted = selection.conflicted;
    candidate
}

fn rewrite_selected_call(program: &mut TypedTrees, site: CallSite, target: SymbolHandle) {
    let target_name = state_by_symbol(program, target)
        .map(|state| state.name.clone())
        .expect("cloned specialization state");
    match site {
        CallSite::Statement(handle) => {
            let StatementNode::Call(call) = program.statement_table.statement_mut(handle) else {
                return;
            };
            call.target_symbol = target;
            call.target = target_name;
            call.machine_arguments = Box::default();
        }
        CallSite::Expression(handle) => {
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
                return;
            };
            call.target_symbol = target;
            call.target = target_name;
            call.machine_arguments = Box::default();
        }
    }
}

fn clone_specialized_machine(
    source: &TypedTrees,
    program: &mut TypedTrees,
    candidate: &Candidate,
    ordinal: usize,
) -> Vec<(SymbolHandle, SymbolHandle)> {
    let source_machine = &source.machines()[candidate.machine_index];
    let source_states = source.machine_states(source_machine).to_vec();
    let source_contained = source.machine_contained_objects(source_machine).to_vec();
    let source_owned = source.machine_owned_data(source_machine).to_vec();
    let type_start = program.type_reference_table.type_reference_count();
    let expression_start = program.expression_table.iter_expressions().count();

    let type_arguments: Vec<String> = candidate
        .type_bindings
        .iter()
        .map(|binding| source.display_type_reference(binding.expect("complete specialization")))
        .collect();
    let machine_paths: Vec<String> = candidate
        .machine_bindings
        .iter()
        .map(|binding| {
            binding
                .as_ref()
                .expect("complete specialization")
                .path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        })
        .collect();
    let fingerprint =
        specialization_fingerprint(&candidate.template_name, &type_arguments, &machine_paths);
    let generated_name = format!(
        "{}$specialized${fingerprint:016x}${ordinal}",
        candidate.template_name
    );
    let machine_symbol = program
        .symbols
        .insert_generated_root(SymbolKind::Machine, &generated_name);

    let machine_children = program.symbols.insert_generated_children(
        machine_symbol,
        source_contained
            .iter()
            .map(|item| (SymbolKind::Object, item.name.as_str()))
            .chain(
                source_owned
                    .iter()
                    .map(|item| (SymbolKind::Field, item.name.as_str())),
            )
            .chain(
                source_states
                    .iter()
                    .map(|state| (SymbolKind::State, state.name.as_str())),
            ),
    );
    let machine_children: Vec<SymbolHandle> =
        omega_core::symbols::SymbolTableBuilder::child_handles(machine_children).collect();
    let mut next_child = machine_children.into_iter();
    let mut symbol_map = vec![(source_machine.symbol, machine_symbol)];
    for item in &source_contained {
        symbol_map.push((
            item.symbol,
            next_child.next().expect("contained clone symbol"),
        ));
    }
    for item in &source_owned {
        symbol_map.push((
            item.symbol,
            next_child.next().expect("owned-data clone symbol"),
        ));
    }
    let state_symbols: Vec<(SymbolHandle, SymbolHandle)> = source_states
        .iter()
        .map(|state| (state.symbol, next_child.next().expect("state clone symbol")))
        .collect();
    symbol_map.extend(state_symbols.iter().copied());

    for (source_state, (_, state_symbol)) in source_states.iter().zip(state_symbols.iter()) {
        let parameters = source.state_parameters(source_state);
        let locals: Vec<_> = source
            .statement_table
            .statements(source_state.statement_nodes)
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::LocalData(local) => Some(local),
                _ => None,
            })
            .collect();
        let children = program.symbols.insert_generated_children(
            *state_symbol,
            parameters
                .iter()
                .map(|parameter| (SymbolKind::Parameter, parameter.name.as_str()))
                .chain(
                    locals
                        .iter()
                        .map(|local| (SymbolKind::Local, local.name.as_str())),
                ),
        );
        let mut children = omega_core::symbols::SymbolTableBuilder::child_handles(children);
        for parameter in parameters {
            symbol_map.push((
                parameter.symbol,
                children.next().expect("state-parameter clone symbol"),
            ));
        }
        for local in locals {
            symbol_map.push((local.symbol, children.next().expect("local clone symbol")));
        }
    }

    let mut cloned = source_machine.clone();
    cloned.symbol = machine_symbol;
    cloned.name = omega_typed_trees::name::Identifier::generated(generated_name);
    cloned.type_parameters = HandleSpan::empty();
    cloned.contains = HandleSpan::empty();
    cloned.owned_data = HandleSpan::empty();
    cloned.satisfies = HandleSpan::empty();
    cloned.decreases = copy_expression_span(source, program, source_machine.decreases, &symbol_map);
    cloned.decrease_order = program.signature_effects.insert_many(
        source
            .signature_effects
            .span_or_empty(source_machine.decrease_order)
            .iter()
            .cloned(),
    );
    cloned.decrease_view_arguments = copy_expression_span(
        source,
        program,
        source_machine.decrease_view_arguments,
        &symbol_map,
    );
    cloned.decrease_range =
        copy_expression(source, program, source_machine.decrease_range, &symbol_map);
    cloned.effects = program.signature_effects.insert_many(
        source
            .signature_effects
            .span_or_empty(source_machine.effects)
            .iter()
            .cloned(),
    );
    cloned.contracts = HandleSpan::empty();
    cloned.states = HandleSpan::empty();

    for (source_item, (_, fresh_symbol)) in source_contained.iter().zip(symbol_map.iter().skip(1)) {
        let mut item = source_item.clone();
        item.symbol = *fresh_symbol;
        program.push_machine_contained_object(&mut cloned, item);
    }
    let owned_symbol_offset = 1 + source_contained.len();
    for (index, source_item) in source_owned.iter().enumerate() {
        let mut item = source_item.clone();
        item.symbol = symbol_map[owned_symbol_offset + index].1;
        item.type_reference =
            copy_type_reference(source, program, source_item.type_reference, &symbol_map);
        item.initial_value =
            copy_expression(source, program, source_item.initial_value, &symbol_map);
        program.push_machine_owned_data(&mut cloned, item);
    }
    for conformance in source.machine_trait_conformances(source_machine) {
        program.push_machine_trait_conformance(&mut cloned, conformance.clone());
    }
    for contract in source.machine_contracts(source_machine) {
        let contract = copy_signature_contract(source, program, *contract, &symbol_map);
        program.push_machine_contract(&mut cloned, contract);
    }

    for (source_state, (_, fresh_symbol)) in source_states.iter().zip(state_symbols.iter()) {
        let mut state = source_state.clone();
        state.symbol = *fresh_symbol;
        state.parameters = HandleSpan::empty();
        state.return_type =
            copy_type_reference(source, program, source_state.return_type, &symbol_map);
        state.statement_nodes = {
            let tables = &mut program.tables;
            tables.statement_table.copy_statement_nodes_deep_from(
                &source.statement_table,
                &source.expression_table,
                &mut tables.expression_table,
                &source.type_reference_table,
                &mut tables.type_reference_table,
                source_state.statement_nodes,
            )
        };
        {
            let tables = &mut program.tables;
            tables.statement_table.remap_symbols_in(
                state.statement_nodes,
                &mut tables.expression_table,
                &mut tables.type_reference_table,
                &symbol_map,
            );
        }
        for source_parameter in source.state_parameters(source_state) {
            let mut parameter = source_parameter.clone();
            parameter.symbol = remapped_symbol(parameter.symbol, &symbol_map);
            parameter.type_reference = copy_type_reference(
                source,
                program,
                source_parameter.type_reference,
                &symbol_map,
            );
            program.push_state_parameter(&mut state, parameter);
        }
        program.push_machine_state(&mut cloned, state);
    }

    substitute_cloned_type_parameters(source, program, candidate, type_start);
    rewrite_cloned_calls(
        source,
        program,
        candidate,
        &state_symbols,
        expression_start,
        cloned.states,
    );
    program.push_machine(cloned);
    program
        .machine_specializations
        .push(omega_typed_trees::typed_trees::MachineSpecialization {
            template: candidate.template_symbol,
            type_arguments,
            machine_arguments: candidate
                .machine_bindings
                .iter()
                .map(|binding| binding.as_ref().expect("complete specialization").symbol)
                .collect(),
            fingerprint,
        });

    state_symbols
}

fn copy_expression(
    source: &TypedTrees,
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> ExpressionHandle {
    if !expression.is_valid() {
        return ExpressionHandle::invalid();
    }
    let copied = program
        .expression_table
        .copy_from(&source.expression_table, expression);
    program.expression_table.remap_symbols_in(copied, symbols);
    copied
}

fn copy_expression_span(
    source: &TypedTrees,
    program: &mut TypedTrees,
    expressions: HandleSpan<ExpressionHandle>,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> HandleSpan<ExpressionHandle> {
    let copied: Vec<_> = source
        .expression_table
        .expression_handles(expressions)
        .iter()
        .map(|expression| copy_expression(source, program, *expression, symbols))
        .collect();
    program.expression_table.insert_expression_handles(copied)
}

fn copy_type_reference(
    source: &TypedTrees,
    program: &mut TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> TypeReferenceHandle {
    if !type_reference.is_valid() {
        return TypeReferenceHandle::invalid();
    }
    let copied = {
        let tables = &mut program.tables;
        tables.type_reference_table.copy_from(
            &source.type_reference_table,
            &source.expression_table,
            &mut tables.expression_table,
            type_reference,
        )
    };
    {
        let tables = &mut program.tables;
        tables
            .type_reference_table
            .remap_symbols_in(copied, &mut tables.expression_table, symbols);
    }
    copied
}

fn copy_signature_contract(
    source: &TypedTrees,
    program: &mut TypedTrees,
    contract: omega_typed_trees::signature::SignatureContract,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> omega_typed_trees::signature::SignatureContract {
    let mut copied = contract;
    copied.facts = HandleSpan::empty();
    for fact in source.proof_facts.span_or_empty(contract.facts) {
        let fact = match fact {
            omega_typed_trees::domain::ProofFact::Expression(expression) => {
                omega_typed_trees::domain::ProofFact::Expression(copy_expression(
                    source,
                    program,
                    *expression,
                    symbols,
                ))
            }
            omega_typed_trees::domain::ProofFact::Membership(membership) => {
                omega_typed_trees::domain::ProofFact::Membership(
                    omega_typed_trees::domain::ProofMembershipFact {
                        value: copy_expression(source, program, membership.value, symbols),
                        domain: program.domain_path_members.insert_many(
                            source
                                .domain_path_members(membership.domain)
                                .iter()
                                .cloned(),
                        ),
                        domain_symbol: remapped_symbol(membership.domain_symbol, symbols),
                    },
                )
            }
        };
        program.proof_facts.append_to_span(&mut copied.facts, fact);
    }
    copied
}

fn substitute_cloned_type_parameters(
    source: &TypedTrees,
    program: &mut TypedTrees,
    candidate: &Candidate,
    type_start: usize,
) {
    for ((parameter_symbol, _), binding) in candidate
        .type_parameters
        .iter()
        .zip(candidate.type_bindings.iter())
    {
        let occurrences: Vec<_> = program
            .type_reference_table
            .named_references()
            .filter(|(handle, symbol, _)| {
                handle.arena_index() as usize >= type_start && symbol == parameter_symbol
            })
            .map(|(handle, _, _)| handle)
            .collect();
        let replacement = copy_type_reference(
            source,
            program,
            binding.expect("complete specialization"),
            &[],
        );
        let replacement = program
            .type_reference_table
            .type_reference(replacement)
            .clone();
        for occurrence in occurrences {
            program
                .type_reference_table
                .substitute_node(occurrence, replacement.clone());
        }
    }
}

fn rewrite_cloned_calls(
    source: &TypedTrees,
    program: &mut TypedTrees,
    candidate: &Candidate,
    state_symbols: &[(SymbolHandle, SymbolHandle)],
    expression_start: usize,
    states: HandleSpan<omega_typed_trees::state::State>,
) {
    let rewrites: Vec<_> = candidate
        .machine_parameters
        .iter()
        .zip(candidate.machine_bindings.iter())
        .map(|((parameter, _, _), binding)| {
            let binding = binding.as_ref().expect("complete specialization");
            let name = state_by_symbol(source, binding.symbol)
                .map(|state| state.name.clone())
                .or_else(|| binding.path.last().cloned())
                .expect("static machine entry name");
            (*parameter, binding.symbol, name)
        })
        .collect();
    for state in program.machine_states.span_or_empty(states).to_vec() {
        for statement in program
            .statement_table
            .statements_mut(state.statement_nodes)
        {
            let StatementNode::Call(call) = statement else {
                continue;
            };
            if let Some((_, target, name)) = rewrites
                .iter()
                .find(|(parameter, _, _)| *parameter == call.target_symbol)
            {
                call.target_symbol = *target;
                call.target = name.clone();
            }
            if state_symbols
                .iter()
                .any(|(_, concrete)| *concrete == call.target_symbol)
            {
                call.machine_arguments = Box::default();
            }
        }
    }
    let handles: Vec<_> = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() as usize >= expression_start)
        .map(|(handle, _)| handle)
        .collect();
    for handle in handles {
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            continue;
        };
        if let Some((_, target, name)) = rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == call.target_symbol)
        {
            call.target_symbol = *target;
            call.target = name.clone();
        }
        if state_symbols
            .iter()
            .any(|(_, concrete)| *concrete == call.target_symbol)
        {
            call.machine_arguments = Box::default();
        }
    }
}

fn remapped_symbol(symbol: SymbolHandle, symbols: &[(SymbolHandle, SymbolHandle)]) -> SymbolHandle {
    symbols
        .iter()
        .find_map(|(before, after)| (*before == symbol).then_some(*after))
        .unwrap_or(symbol)
}

fn apply_specialization(program: &mut TypedTrees, candidate: &Candidate) {
    let type_arguments: Vec<String> = candidate
        .type_bindings
        .iter()
        .map(|binding| {
            program.display_type_reference(binding.expect("complete type specialization"))
        })
        .collect();
    let machine_arguments: Vec<SymbolHandle> = candidate
        .machine_bindings
        .iter()
        .map(|binding| {
            binding
                .as_ref()
                .expect("complete machine specialization")
                .symbol
        })
        .collect();
    let machine_paths: Vec<String> = candidate
        .machine_bindings
        .iter()
        .map(|binding| {
            binding
                .as_ref()
                .expect("complete machine specialization")
                .path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        })
        .collect();
    program
        .machine_specializations
        .push(omega_typed_trees::typed_trees::MachineSpecialization {
            template: candidate.template_symbol,
            type_arguments: type_arguments.clone(),
            machine_arguments,
            fingerprint: specialization_fingerprint(
                &candidate.template_name,
                &type_arguments,
                &machine_paths,
            ),
        });

    for ((parameter_symbol, parameter_name), binding) in candidate
        .type_parameters
        .iter()
        .zip(candidate.type_bindings.iter())
    {
        let replacement = program
            .type_reference_table
            .type_reference(binding.expect("complete type specialization"))
            .clone();
        let occurrences: Vec<TypeReferenceHandle> = program
            .type_reference_table
            .named_references()
            .filter(|(_, symbol, name)| {
                symbol == parameter_symbol
                    || (!symbol.is_valid()
                        && !parameter_symbol.is_valid()
                        && *name == parameter_name.as_str())
            })
            .map(|(handle, _, _)| handle)
            .collect();
        for occurrence in occurrences {
            program
                .type_reference_table
                .substitute_node(occurrence, replacement.clone());
        }
    }

    let rewrites: Vec<(
        SymbolHandle,
        SymbolHandle,
        omega_typed_trees::name::Identifier,
    )> = candidate
        .machine_parameters
        .iter()
        .zip(candidate.machine_bindings.iter())
        .map(|((parameter_symbol, _, _), binding)| {
            let binding = binding.as_ref().expect("complete machine specialization");
            let target = state_by_symbol(program, binding.symbol)
                .map(|state| state.name.clone())
                .or_else(|| binding.path.last().cloned())
                .expect("admitted static machine argument has an entry name");
            (*parameter_symbol, binding.symbol, target)
        })
        .collect();

    let state_spans: Vec<HandleSpan<StatementNode>> = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .map(|state| state.statement_nodes)
        .collect();
    for span in state_spans {
        for statement in program.statement_table.statements_mut(span) {
            let StatementNode::Call(call) = statement else {
                continue;
            };
            if let Some((_, symbol, name)) = rewrites
                .iter()
                .find(|(parameter, _, _)| *parameter == call.target_symbol)
            {
                call.target_symbol = *symbol;
                call.target = name.clone();
            }
            if candidate.state_symbols.contains(&call.target_symbol) {
                call.machine_arguments = Box::default();
            }
        }
    }

    let expression_handles: Vec<ExpressionHandle> = program
        .expression_table
        .iter_expressions()
        .map(|(handle, _)| handle)
        .collect();
    for handle in expression_handles {
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            continue;
        };
        if let Some((_, symbol, name)) = rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == call.target_symbol)
        {
            call.target_symbol = *symbol;
            call.target = name.clone();
        }
        if candidate.state_symbols.contains(&call.target_symbol) {
            call.machine_arguments = Box::default();
        }
    }

    program.machines_mut()[candidate.machine_index].type_parameters = HandleSpan::empty();
}

fn specialization_fingerprint(
    template: &str,
    type_arguments: &[String],
    machine_arguments: &[String],
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for part in std::iter::once(template)
        .chain(type_arguments.iter().map(String::as_str))
        .chain(machine_arguments.iter().map(String::as_str))
    {
        for byte in part.as_bytes().iter().copied().chain(std::iter::once(0xff)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    hash
}
