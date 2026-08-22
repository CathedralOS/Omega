//! Compile-time specialization of generic machines.
//!
//! Type parameters, canonical const parameters, and static machine parameters
//! are one specialization tuple. The first concrete tuple reuses the authored
//! declaration; every additional tuple receives a deep-copied body with fresh
//! lexical symbols. Calls are rewritten to their selected concrete state,
//! calls through `F(...)` become direct calls to the selected entry, and each
//! tuple records a deterministic cache identity. Incomplete tuples remain
//! generic and are fenced by validation; no runtime const/callable value or
//! dictionary is introduced.

use psi_arena::{Handle, HandleSpan};
use psi_diagnostics::Diagnostic;
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::TypeParameterKind;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use psi_typed_trees::signature::StateSignature;
use psi_typed_trees::statement::{StatementHandle, StatementNode};
use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone)]
struct Candidate {
    machine_index: usize,
    template_symbol: SymbolHandle,
    template_name: String,
    state_symbols: Vec<SymbolHandle>,
    type_parameters: Vec<(SymbolHandle, String)>,
    parameter_bounds: Vec<Vec<psi_validation::DeclaredPropertyRequirement>>,
    conformance_bounds: Vec<psi_typed_trees::machine::GenericConformanceBound>,
    type_bindings: Vec<Option<TypeReferenceHandle>>,
    const_parameters: Vec<(SymbolHandle, String, TypeReferenceHandle)>,
    const_bindings: Vec<Option<TypeReferenceHandle>>,
    machine_parameters: Vec<(SymbolHandle, String, StateSignature)>,
    machine_bindings: Vec<Option<StaticMachineArgument>>,
    evidence_parameters: Vec<psi_typed_trees::machine::GenericConformanceBound>,
    evidence_bindings: Vec<Option<StaticMachineArgument>>,
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
    caller_is_generic: bool,
    self_forwarded_machine_parameters: bool,
    self_forwarded_evidence_parameters: bool,
    type_bindings: Vec<Option<TypeReferenceHandle>>,
    const_bindings: Vec<Option<TypeReferenceHandle>>,
    machine_bindings: Vec<Option<StaticMachineArgument>>,
    evidence_bindings: Vec<Option<StaticMachineArgument>>,
    conflicted: bool,
}

impl CallSelection {
    fn is_complete(&self) -> bool {
        !self.conflicted
            && !self.self_forwarded_machine_parameters
            && !self.self_forwarded_evidence_parameters
            && self.type_bindings.iter().all(Option::is_some)
            && self.const_bindings.iter().all(Option::is_some)
            && self.machine_bindings.iter().all(Option::is_some)
            && self.evidence_bindings.iter().all(Option::is_some)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct SpecializationKey {
    type_arguments: Vec<String>,
    const_arguments: Vec<String>,
    machine_arguments: Vec<SymbolHandle>,
    evidence_arguments: Vec<u64>,
}

pub(crate) fn monomorphize_generic_machine_value_calls_with_nominal_uses(
    program: &mut TypedTrees,
    nominal_uses: &mut Vec<psi_validation::ValidatedNominalMachineUse>,
) -> Result<(), Vec<Diagnostic>> {
    materialize_static_const_argument_types(program);
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
        let mut const_parameters = Vec::new();
        let mut machine_parameters = Vec::new();
        let evidence_parameters = machine
            .conformance_bounds
            .iter()
            .filter(|bound| bound.binder.is_some())
            .cloned()
            .collect::<Vec<_>>();
        for parameter in parameters {
            match &parameter.kind {
                TypeParameterKind::Type => {
                    type_parameters.push((parameter.symbol, parameter.name.as_str().to_owned()));
                    parameter_bounds.push(psi_validation::declared_property_requirements(
                        &parameter.bounds,
                    ));
                }
                TypeParameterKind::Machine { contract } => {
                    let signature = program
                        .machine_parameter_contract_view(contract)
                        .expect(
                            "typed machine-parameter contract must retain a valid requirement identity",
                        )
                        .signature();
                    machine_parameters.push((
                        parameter.symbol,
                        parameter.name.as_str().to_owned(),
                        signature.clone(),
                    ));
                }
                TypeParameterKind::Const { type_reference } => const_parameters.push((
                    parameter.symbol,
                    parameter.name.as_str().to_owned(),
                    *type_reference,
                )),
                // Proposition parameters are currently legal only on trait
                // abstraction surfaces, never on executable machines.
                TypeParameterKind::Proposition { .. } => {}
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
            const_bindings: vec![None; const_parameters.len()],
            machine_bindings: vec![None; machine_parameters.len()],
            evidence_bindings: vec![None; evidence_parameters.len()],
            type_parameters,
            parameter_bounds,
            conformance_bounds: machine.conformance_bounds.clone(),
            const_parameters,
            machine_parameters,
            evidence_parameters,
            conflicted: false,
        });
    }
    if candidates.is_empty() {
        return Ok(());
    }

    let mut type_proposals = Vec::new();
    let mut const_proposals = Vec::new();
    let mut machine_proposals = Vec::new();
    let mut evidence_proposals = Vec::new();
    let contract_expressions = contract_expression_handles(program);

    // Static selections may occur in any EXECUTABLE expression position.
    // Contract calls are universal logical propositions, not runtime call
    // sites: using one as specialization evidence consumes the generic schema
    // and can rewrite every law stated over it to one accidental concrete
    // tuple (notably heterogeneous quotient relations).
    for (handle, expression) in program.expression_table.iter_expressions() {
        if contract_expressions.contains(&handle) {
            continue;
        }
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
            &mut evidence_proposals,
            &mut type_proposals,
            &mut const_proposals,
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
                        &mut evidence_proposals,
                        &mut type_proposals,
                        &mut const_proposals,
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
                                &mut evidence_proposals,
                                &mut type_proposals,
                                &mut const_proposals,
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
                                &mut evidence_proposals,
                                &mut type_proposals,
                                &mut const_proposals,
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
    let selections =
        collect_call_selections(program, &candidates, &callee_states, &contract_expressions);

    for (candidate_index, parameter_index, binding) in type_proposals {
        if type_reference_is_still_generic(program, binding, &all_type_parameter_symbols) {
            continue;
        }
        let candidate = &mut candidates[candidate_index];
        match candidate.type_bindings[parameter_index] {
            None => candidate.type_bindings[parameter_index] = Some(binding),
            Some(existing) if !same_type_identity(program, existing, binding) => {
                candidate.conflicted = true;
            }
            Some(_) => {}
        }
    }

    for (candidate_index, parameter_index, binding) in const_proposals {
        if type_reference_is_any_generic_parameter(program, binding) {
            continue;
        }
        let candidate = &mut candidates[candidate_index];
        match candidate.const_bindings[parameter_index] {
            None => candidate.const_bindings[parameter_index] = Some(binding),
            Some(existing) if !same_type_identity(program, existing, binding) => {
                candidate.conflicted = true;
            }
            Some(_) => {}
        }
    }

    for (candidate_index, parameter_index, binding) in machine_proposals {
        // A generic body may forward its own machine parameter recursively.
        // That symbol is a lexical placeholder, not specialization evidence;
        // only a concrete entry selected by an outer call binds the tuple.
        if machine_parameter_by_symbol(program, binding.symbol).is_some() {
            continue;
        }
        let candidate = &mut candidates[candidate_index];
        match &candidate.machine_bindings[parameter_index] {
            None => candidate.machine_bindings[parameter_index] = Some(binding),
            Some(existing) if existing.symbol != binding.symbol => candidate.conflicted = true,
            Some(_) => {}
        }
    }

    for (candidate_index, parameter_index, binding) in evidence_proposals {
        if matches!(
            program.symbols.get(binding.symbol).kind,
            SymbolKind::ConformanceParameter
        ) {
            continue;
        }
        let candidate = &mut candidates[candidate_index];
        match &candidate.evidence_bindings[parameter_index] {
            None => candidate.evidence_bindings[parameter_index] = Some(binding),
            Some(existing)
                if existing.symbol != binding.symbol
                    || existing.display_name() != binding.display_name() =>
            {
                candidate.conflicted = true
            }
            Some(_) => {}
        }
    }

    let multi_tuple_candidates: Vec<usize> = (0..candidates.len())
        .filter(|candidate_index| {
            !has_forwarded_generic_call(&selections, *candidate_index)
                && unique_complete_selections(program, &selections, *candidate_index).len() > 1
        })
        .collect();

    let mut diagnostics = Vec::new();
    let approved = approved_type_bounds(program, &candidates);
    let conformance_approved = candidates
        .iter()
        .map(
            |candidate| match validate_candidate_conformance_bounds(program, candidate) {
                Ok(()) => true,
                Err(mut errors) => {
                    diagnostics.append(&mut errors);
                    false
                }
            },
        )
        .collect::<Vec<_>>();
    let mut applied_any = false;
    for (candidate_index, approved) in approved.into_iter().enumerate() {
        if multi_tuple_candidates.contains(&candidate_index) {
            continue;
        }
        if has_forwarded_generic_call(&selections, candidate_index) {
            // A generic caller is forwarding one of its own parameters into
            // this template. Specialize the caller first; the next fixed-point
            // round will then see the concrete argument in its rewritten body.
            continue;
        }
        let candidate = candidates[candidate_index].clone();
        let has_static_selection = candidate.const_bindings.iter().any(Option::is_some)
            || candidate.machine_bindings.iter().any(Option::is_some)
            || candidate.evidence_bindings.iter().any(Option::is_some);
        let has_incomplete_call = selections.iter().any(|selection| {
            selection.candidate_index == candidate_index
                && !selection.self_forwarded_machine_parameters
                && !selection.is_complete()
        });
        if has_incomplete_call {
            if has_static_selection {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` has a static selection, but its complete type/const/machine/conformance specialization tuple cannot be derived",
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
                || candidate.const_bindings.iter().any(Option::is_none)
                || candidate.machine_bindings.iter().any(Option::is_none)
                || candidate.evidence_bindings.iter().any(Option::is_none))
        {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` has a static selection, but its complete type/const/machine/conformance specialization tuple cannot be derived",
                candidate.template_name
            )));
            continue;
        }
        if !approved
            || !conformance_approved[candidate_index]
            || candidate.type_bindings.iter().any(Option::is_none)
            || candidate.const_bindings.iter().any(Option::is_none)
            || candidate.machine_bindings.iter().any(Option::is_none)
            || candidate.evidence_bindings.iter().any(Option::is_none)
        {
            continue;
        }
        apply_specialization(program, &candidate);
        applied_any = true;
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
            } else {
                applied_any = true;
            }
        }
    }
    if diagnostics.is_empty() && applied_any {
        refresh_closed_domain_instance_identities(program).map_err(|error| vec![error])?;
        nominal_uses
            .extend(psi_validation::validate_static_machine_selections_with_facts(program)?);
        monomorphize_generic_machine_value_calls_with_nominal_uses(program, nominal_uses)
    } else if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn materialize_static_const_argument_types(program: &mut TypedTrees) {
    fn collect(arguments: &[StaticMachineArgument], literals: &mut Vec<String>) {
        for argument in arguments {
            if let Some(literal) = &argument.const_literal {
                let literal = literal.text().to_owned();
                if !literals.contains(&literal) {
                    literals.push(literal);
                }
            }
            if let Some(application) = &argument.application {
                collect(&application.arguments, literals);
            }
        }
    }

    let mut literals = Vec::new();
    for (_, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            collect(&call.machine_arguments, &mut literals);
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Call(call) = statement {
                    collect(&call.machine_arguments, &mut literals);
                }
            }
        }
    }
    for literal in literals {
        let exists = program
            .type_reference_table
            .named_references()
            .any(|(_, symbol, name)| !symbol.is_valid() && name == literal);
        if !exists {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: psi_typed_trees::name::Identifier::generated(literal),
                });
        }
    }
}

fn machine_parameter_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&psi_typed_trees::data::TypeParameter> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_type_parameters(machine)
            .iter()
            .find(|parameter| {
                parameter.symbol == symbol
                    && matches!(parameter.kind, TypeParameterKind::Machine { .. })
            })
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_call_proposals(
    program: &TypedTrees,
    caller_machine: &psi_typed_trees::machine::Machine,
    caller_state: &psi_typed_trees::state::State,
    candidates: &[Candidate],
    callee_states: &[CalleeState],
    target_symbol: SymbolHandle,
    target_name: &str,
    machine_arguments: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
    expected_return: Option<TypeReferenceHandle>,
    machine_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    evidence_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
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
        evidence_proposals,
        type_proposals,
        const_proposals,
    );

    let candidate = &candidates[callee.candidate_index];
    let skip = callee.parameter_types.len().saturating_sub(arguments.len());
    for (argument, required) in arguments
        .iter()
        .zip(callee.parameter_types.iter().skip(skip))
    {
        let Some(actual) = psi_validation::declared_place_type_raw(
            program,
            caller_machine,
            Some(caller_state),
            *argument,
        ) else {
            continue;
        };
        infer_static_bindings(
            program,
            *required,
            actual,
            &candidate.type_parameters,
            &candidate.const_parameters,
            callee.candidate_index,
            type_proposals,
            const_proposals,
        );
    }
    if let Some(actual) = expected_return {
        infer_static_bindings(
            program,
            callee.return_type,
            actual,
            &candidate.type_parameters,
            &candidate.const_parameters,
            callee.candidate_index,
            type_proposals,
            const_proposals,
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
    evidence_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
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
        evidence_proposals,
        type_proposals,
        const_proposals,
    );
}

fn collect_machine_proposals_for_callee(
    program: &TypedTrees,
    candidates: &[Candidate],
    callee: &CalleeState,
    machine_arguments: &[StaticMachineArgument],
    machine_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    evidence_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    let candidate = &candidates[callee.candidate_index];
    let mut type_index = 0usize;
    let mut const_index = 0usize;
    let mut machine_index = 0usize;
    let mut evidence_index = 0usize;
    for selected in machine_arguments {
        if let Some(literal) = &selected.const_literal {
            if const_index < candidate.const_parameters.len()
                && let Some((handle, _, _)) = program
                    .type_reference_table
                    .named_references()
                    .find(|(_, symbol, name)| !symbol.is_valid() && *name == literal.text())
            {
                const_proposals.push((callee.candidate_index, const_index, handle));
                const_index += 1;
            }
            continue;
        }
        if !selected.symbol.is_valid() {
            continue;
        }
        let kind = program.symbols.get(selected.symbol).kind;
        if matches!(
            kind,
            SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
        ) {
            if type_index < candidate.type_parameters.len()
                && let Some((handle, _, _)) = program
                    .type_reference_table
                    .named_references()
                    .find(|(_, symbol, _)| *symbol == selected.symbol)
            {
                type_proposals.push((callee.candidate_index, type_index, handle));
                type_index += 1;
            }
            continue;
        }
        if matches!(
            kind,
            SymbolKind::Conformance | SymbolKind::ConformanceParameter
        ) {
            if evidence_index < candidate.evidence_parameters.len() {
                evidence_proposals.push((callee.candidate_index, evidence_index, selected.clone()));
                evidence_index += 1;
            }
            continue;
        }
        if !matches!(kind, SymbolKind::State | SymbolKind::MachineParameter) {
            continue;
        }
        if machine_index >= candidate.machine_parameters.len() {
            continue;
        }
        machine_proposals.push((callee.candidate_index, machine_index, selected.clone()));
        let requirement = &candidate.machine_parameters[machine_index].2;
        machine_index += 1;
        let Some(actual_state) = state_by_symbol(program, selected.symbol) else {
            continue;
        };
        for (required, actual) in program
            .state_signature_parameters(requirement)
            .iter()
            .zip(program.state_parameters(actual_state))
        {
            // The selected entry's refinement remains part of its machine
            // contract. Generic specialization binds the underlying runtime
            // carrier so a qualified entry still matches the ordinary value
            // supplied at the selecting call site.
            let actual_type =
                psi_validation::unwrapped_type_reference(program, actual.type_reference)
                    .unwrap_or(actual.type_reference);
            infer_static_bindings(
                program,
                required.type_reference,
                actual_type,
                &candidate.type_parameters,
                &candidate.const_parameters,
                callee.candidate_index,
                type_proposals,
                const_proposals,
            );
        }
        infer_static_bindings(
            program,
            requirement.return_type,
            actual_state.return_type,
            &candidate.type_parameters,
            &candidate.const_parameters,
            callee.candidate_index,
            type_proposals,
            const_proposals,
        );
    }
}

fn resolve_callee<'a>(
    callee_states: &'a [CalleeState],
    target_symbol: SymbolHandle,
    target_name: &str,
) -> Option<&'a CalleeState> {
    if target_symbol.is_valid() {
        return callee_states
            .iter()
            .find(|callee| callee.symbol == target_symbol);
    }
    let mut matching = callee_states
        .iter()
        .filter(|callee| callee.name == target_name);
    match (matching.next(), matching.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

fn state_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&psi_typed_trees::state::State> {
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
    contract_expressions: &[ExpressionHandle],
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
                            !program.machine_type_parameters(machine).is_empty(),
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
                                !program.machine_type_parameters(machine).is_empty(),
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
                                !program.machine_type_parameters(machine).is_empty(),
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
        if covered_expressions.contains(&handle) || contract_expressions.contains(&handle) {
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
        let mut evidence_proposals = Vec::new();
        let mut type_proposals = Vec::new();
        let mut const_proposals = Vec::new();
        collect_machine_proposals_for_callee(
            program,
            candidates,
            callee,
            &call.machine_arguments,
            &mut machine_proposals,
            &mut evidence_proposals,
            &mut type_proposals,
            &mut const_proposals,
        );
        let selection = selection_from_proposals(
            program,
            CallSite::Expression(handle),
            callee,
            candidate,
            false,
            machine_proposals,
            evidence_proposals,
            type_proposals,
            const_proposals,
        );
        upsert_selection(&mut selections, selection);
    }

    selections
}

/// Every expression node reachable from an ordinary machine/state contract.
/// These nodes share the global expression arena with executable bodies, but
/// their static-machine selections quantify a proof schema and must never
/// trigger runtime monomorphization.
fn contract_expression_handles(program: &TypedTrees) -> Vec<ExpressionHandle> {
    let mut handles = Vec::new();
    for machine in program.machines() {
        for contract in program.machine_contracts(machine) {
            collect_contract_facts(program, contract.facts, &mut handles);
        }
        for state in program.machine_states(machine) {
            for contract in program.state_contracts(state) {
                collect_contract_facts(program, contract.facts, &mut handles);
            }
        }
    }
    handles
}

fn collect_contract_facts(
    program: &TypedTrees,
    facts: psi_arena::HandleSpan<ProofFact>,
    handles: &mut Vec<ExpressionHandle>,
) {
    for fact in program.proof_facts.span_or_empty(facts) {
        if let ProofFact::Expression(expression) = fact {
            collect_expression_tree(program, *expression, handles);
        }
    }
}

fn collect_expression_tree(
    program: &TypedTrees,
    expression: ExpressionHandle,
    handles: &mut Vec<ExpressionHandle>,
) {
    if !expression.is_valid() || handles.contains(&expression) {
        return;
    }
    handles.push(expression);
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_tree(program, *value, handles);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_expression_tree(program, atomic.value, handles);
            collect_expression_tree(program, atomic.result, handles);
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_tree(program, binary.left, handles);
            collect_expression_tree(program, binary.right, handles);
        }
        ExpressionNode::Cast(cast) => collect_expression_tree(program, cast.value, handles),
        ExpressionNode::Call(call) => {
            collect_expression_tree(program, call.receiver, handles);
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_tree(program, *argument, handles);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_tree(program, indexed.collection, handles);
            collect_expression_tree(program, indexed.index, handles);
        }
        ExpressionNode::Member(member) => {
            collect_expression_tree(program, member.receiver, handles)
        }
        ExpressionNode::Mutable(inner) => collect_expression_tree(program, *inner, handles),
        ExpressionNode::Range(range) => {
            collect_expression_tree(program, range.start, handles);
            collect_expression_tree(program, range.end, handles);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                collect_expression_tree(program, field.value, handles);
            }
        }
        ExpressionNode::Unary(unary) => collect_expression_tree(program, unary.operand, handles),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn selection_for_call(
    program: &TypedTrees,
    caller_machine: &psi_typed_trees::machine::Machine,
    caller_state: &psi_typed_trees::state::State,
    candidates: &[Candidate],
    callee_states: &[CalleeState],
    site: CallSite,
    target_symbol: SymbolHandle,
    target_name: &str,
    machine_arguments: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
    expected_return: Option<TypeReferenceHandle>,
    caller_is_generic: bool,
) -> Option<CallSelection> {
    let callee = resolve_callee(callee_states, target_symbol, target_name)?;
    let candidate = &candidates[callee.candidate_index];
    let mut machine_proposals = Vec::new();
    let mut evidence_proposals = Vec::new();
    let mut type_proposals = Vec::new();
    let mut const_proposals = Vec::new();
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
        &mut evidence_proposals,
        &mut type_proposals,
        &mut const_proposals,
    );
    Some(selection_from_proposals(
        program,
        site,
        callee,
        candidate,
        caller_is_generic,
        machine_proposals,
        evidence_proposals,
        type_proposals,
        const_proposals,
    ))
}

fn selection_from_proposals(
    program: &TypedTrees,
    site: CallSite,
    callee: &CalleeState,
    candidate: &Candidate,
    caller_is_generic: bool,
    machine_proposals: Vec<(usize, usize, StaticMachineArgument)>,
    evidence_proposals: Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: Vec<(usize, usize, TypeReferenceHandle)>,
) -> CallSelection {
    let mut selection = CallSelection {
        site,
        callee_symbol: callee.symbol,
        candidate_index: callee.candidate_index,
        caller_is_generic,
        self_forwarded_machine_parameters: false,
        self_forwarded_evidence_parameters: false,
        type_bindings: vec![None; candidate.type_parameters.len()],
        const_bindings: vec![None; candidate.const_parameters.len()],
        machine_bindings: vec![None; candidate.machine_parameters.len()],
        evidence_bindings: vec![None; candidate.evidence_parameters.len()],
        conflicted: false,
    };
    for (_, parameter, binding) in type_proposals {
        if type_reference_is_any_generic_parameter(program, binding) {
            continue;
        }
        match selection.type_bindings[parameter] {
            None => selection.type_bindings[parameter] = Some(binding),
            Some(existing) if !same_type_identity(program, existing, binding) => {
                selection.conflicted = true
            }
            Some(_) => {}
        }
    }
    for (_, parameter, binding) in const_proposals {
        if type_reference_is_any_generic_parameter(program, binding) {
            continue;
        }
        match selection.const_bindings[parameter] {
            None => selection.const_bindings[parameter] = Some(binding),
            Some(existing) if !same_type_identity(program, existing, binding) => {
                selection.conflicted = true
            }
            Some(_) => {}
        }
    }
    for (_, parameter, binding) in machine_proposals {
        if candidate.machine_parameters[parameter].0 == binding.symbol {
            selection.self_forwarded_machine_parameters = true;
        }
        match &selection.machine_bindings[parameter] {
            None => selection.machine_bindings[parameter] = Some(binding),
            Some(existing) if existing.symbol != binding.symbol => selection.conflicted = true,
            Some(_) => {}
        }
    }
    for (_, parameter, binding) in evidence_proposals {
        if candidate.evidence_parameters[parameter].binder == Some(binding.symbol) {
            selection.self_forwarded_evidence_parameters = true;
        }
        match &selection.evidence_bindings[parameter] {
            None => selection.evidence_bindings[parameter] = Some(binding),
            Some(existing)
                if existing.symbol != binding.symbol
                    || existing.display_name() != binding.display_name() =>
            {
                selection.conflicted = true
            }
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
                matches!(
                    parameter.kind,
                    TypeParameterKind::Type | TypeParameterKind::Const { .. }
                ) && (parameter.symbol == *symbol
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
                .const_bindings
                .iter()
                .filter(|item| item.is_some())
                .count()
            + existing
                .machine_bindings
                .iter()
                .filter(|item| item.is_some())
                .count()
            + existing
                .evidence_bindings
                .iter()
                .filter(|item| item.is_some())
                .count();
        let new_evidence = selection
            .type_bindings
            .iter()
            .filter(|item| item.is_some())
            .count()
            + selection
                .const_bindings
                .iter()
                .filter(|item| item.is_some())
                .count()
            + selection
                .machine_bindings
                .iter()
                .filter(|item| item.is_some())
                .count()
            + selection
                .evidence_bindings
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
                .map(|binding| {
                    program
                        .normalized_type_identity(binding.expect("complete selection"))
                        .into_string()
                })
                .collect(),
            const_arguments: selection
                .const_bindings
                .iter()
                .map(|binding| {
                    program
                        .normalized_type_identity(binding.expect("complete selection"))
                        .into_string()
                })
                .collect(),
            machine_arguments: selection
                .machine_bindings
                .iter()
                .map(|binding| binding.as_ref().expect("complete selection").symbol)
                .collect(),
            evidence_arguments: selection
                .evidence_bindings
                .iter()
                .map(|binding| {
                    crate::conformance_applications::close_conformance_application(
                        program,
                        binding.as_ref().expect("complete selection"),
                    )
                    .expect("validated complete conformance application")
                    .fingerprint
                })
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

fn has_forwarded_generic_call(selections: &[CallSelection], candidate_index: usize) -> bool {
    selections.iter().any(|selection| {
        selection.candidate_index == candidate_index
            && selection.caller_is_generic
            && !selection.self_forwarded_machine_parameters
            && !selection.self_forwarded_evidence_parameters
            && !selection.is_complete()
    })
}

fn infer_static_bindings(
    program: &TypedTrees,
    required: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    type_parameters: &[(SymbolHandle, String)],
    const_parameters: &[(SymbolHandle, String, TypeReferenceHandle)],
    candidate_index: usize,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    if !required.is_valid() || !actual.is_valid() {
        return;
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
        && let Some(index) =
            type_parameters
                .iter()
                .position(|(parameter_symbol, parameter_name)| {
                    parameter_symbol == symbol
                        || (!parameter_symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter_name == name.as_str())
                })
    {
        type_proposals.push((candidate_index, index, actual));
        return;
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
        && let Some(index) =
            const_parameters
                .iter()
                .position(|(parameter_symbol, parameter_name, _)| {
                    parameter_symbol == symbol
                        || (!parameter_symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter_name == name.as_str())
                })
    {
        const_proposals.push((candidate_index, index, actual));
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
        ) => infer_static_bindings(
            program,
            *required,
            *actual,
            type_parameters,
            const_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
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
        ) => infer_static_bindings(
            program,
            *required,
            actual,
            type_parameters,
            const_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
        ),
        (
            TypeReferenceNode::Constrained {
                base_type: required_base,
                constraints: required_constraints,
            },
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                constraints: actual_constraints,
            },
        ) => {
            infer_static_bindings(
                program,
                *required_base,
                *actual_base,
                type_parameters,
                const_parameters,
                candidate_index,
                type_proposals,
                const_proposals,
            );
            infer_domain_argument_bindings(
                program,
                *required_constraints,
                *actual_constraints,
                type_parameters,
                const_parameters,
                candidate_index,
                type_proposals,
                const_proposals,
            );
        }
        (TypeReferenceNode::Constrained { base_type, .. }, _) => infer_static_bindings(
            program,
            *base_type,
            psi_validation::unwrapped_type_reference(program, actual).unwrap_or(actual),
            type_parameters,
            const_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
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
        ) => infer_static_bindings(
            program,
            *required,
            *actual,
            type_parameters,
            const_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
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
                infer_static_bindings(
                    program,
                    *required,
                    *actual,
                    type_parameters,
                    const_parameters,
                    candidate_index,
                    type_proposals,
                    const_proposals,
                );
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn infer_domain_argument_bindings(
    program: &TypedTrees,
    required_constraints: HandleSpan<TypeConstraintNode>,
    actual_constraints: HandleSpan<TypeConstraintNode>,
    type_parameters: &[(SymbolHandle, String)],
    const_parameters: &[(SymbolHandle, String, TypeReferenceHandle)],
    candidate_index: usize,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    for required in program
        .type_reference_table
        .constraints(required_constraints)
    {
        let TypeConstraintNode::Domain(required) = required else {
            continue;
        };
        let Some(actual) = program
            .type_reference_table
            .constraints(actual_constraints)
            .iter()
            .find_map(|constraint| {
                let TypeConstraintNode::Domain(actual) = constraint else {
                    return None;
                };
                same_domain_family(required, actual).then_some(actual)
            })
        else {
            continue;
        };
        for (required, actual) in required.arguments.iter().zip(&actual.arguments) {
            infer_static_bindings(
                program,
                *required,
                *actual,
                type_parameters,
                const_parameters,
                candidate_index,
                type_proposals,
                const_proposals,
            );
        }
    }
}

fn same_domain_family(
    left: &psi_typed_trees::types::DomainConstraint,
    right: &psi_typed_trees::types::DomainConstraint,
) -> bool {
    if left.symbol.is_valid() && right.symbol.is_valid() {
        return left.symbol == right.symbol;
    }
    left.name == right.name
        || left.name.as_str().rsplit("::").next() == right.name.as_str().rsplit("::").next()
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

fn same_type_identity(
    program: &TypedTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    program.normalized_type_identity(left) == program.normalized_type_identity(right)
}

fn approved_type_bounds(program: &TypedTrees, candidates: &[Candidate]) -> Vec<bool> {
    let mut symbol_diagnostics = Vec::new();
    let symbols = psi_validation::TopLevelSymbols::build(program, &mut symbol_diagnostics);
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
                        psi_validation::unwrapped_type_reference(program, *binding)
                    else {
                        return false;
                    };
                    bounds.iter().all(|property| {
                        psi_validation::type_satisfies_declared_property(
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

fn validate_candidate_conformance_bounds(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for bound in &candidate.conformance_bounds {
        let Some(parameter_index) = candidate
            .type_parameters
            .iter()
            .position(|(symbol, _)| *symbol == bound.subject)
        else {
            continue;
        };
        let Some(binding) = candidate.type_bindings[parameter_index] else {
            continue;
        };
        let Some(type_name) = concrete_data_type_name(program, binding) else {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` binds `{}` to `{}`, which is not a nominal data type and cannot satisfy conformance bound `{}`",
                candidate.template_name,
                bound.subject_name,
                program.display_type_reference(binding),
                bound.carrier_name,
            )));
            continue;
        };
        let type_identity = program.display_type_reference(binding);

        if let Some(binder) = bound.binder {
            let Some(evidence_index) = candidate
                .evidence_parameters
                .iter()
                .position(|parameter| parameter.binder == Some(binder))
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` lost explicit conformance binder `{}` from its specialization telescope",
                    candidate.template_name,
                    bound
                        .binder_name
                        .as_ref()
                        .map_or("<missing>", |name| name.as_str()),
                )));
                continue;
            };
            let Some(selected_binding) = candidate.evidence_bindings[evidence_index].as_ref()
            else {
                continue;
            };
            let selected_symbol = selected_binding.symbol;
            let Some(selected) = program
                .conformances()
                .iter()
                .find(|conformance| conformance.symbol == selected_symbol)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` binds `{}` to a symbol that is not a package-scoped conformance",
                    candidate.template_name,
                    bound
                        .binder_name
                        .as_ref()
                        .map_or("<missing>", |name| name.as_str()),
                )));
                continue;
            };
            let application = match crate::conformance_applications::close_conformance_application(
                program,
                selected_binding,
            ) {
                Ok(application) => application,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let expected_trait = program
                .traits()
                .iter()
                .find(|definition| definition.symbol == bound.carrier);
            if application.subject_identity.as_deref() != Some(type_identity.as_str())
                || expected_trait
                    .is_none_or(|definition| application.trait_definition != definition.symbol)
                || !conformance_application_arguments_match_candidate(
                    program,
                    candidate,
                    bound,
                    &application,
                )
            {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` cannot bind `{}` to conformance `{}`: expected a complete `{type_identity} satisfies {}` map with the instantiated trait arguments",
                    candidate.template_name,
                    bound
                        .binder_name
                        .as_ref()
                        .map_or("<missing>", |name| name.as_str()),
                    selected
                        .alias
                        .as_ref()
                        .map_or("<unnamed>", |name| name.as_str()),
                    bound.carrier_name,
                )));
            }
            continue;
        }

        if let Some(conformance_symbol) = bound.conformance {
            let selected_carrier = program
                .conformances()
                .iter()
                .find(|conformance| conformance.symbol == conformance_symbol)
                .and_then(|conformance| conformance.carrier_name())
                .map(|carrier| carrier.as_str());
            if selected_carrier != Some(bound.carrier_name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` names conformance `{}::{}`, but that declaration belongs to `{}`",
                    candidate.template_name,
                    bound.carrier_name,
                    bound
                        .conformance_name
                        .as_ref()
                        .map_or("<missing>", |name| name.as_str()),
                    selected_carrier.unwrap_or("a carrierless evidence package"),
                )));
                continue;
            }
            if type_name != bound.carrier_name.as_str() {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` binds `{}` to `{type_name}`, but named conformance `{}::{}` belongs to `{}`",
                    candidate.template_name,
                    bound.subject_name,
                    bound.carrier_name,
                    bound
                        .conformance_name
                        .as_ref()
                        .map_or("<missing>", |name| name.as_str()),
                    bound.carrier_name,
                )));
            }
            continue;
        }

        let matches = program
            .conformances()
            .iter()
            .filter(|conformance| {
                conformance
                    .carrier_name()
                    .is_some_and(|carrier| carrier.as_str() == type_name)
                    && conformance.trait_name == bound.carrier_name
                    && conformance_arguments_match_candidate(program, candidate, bound, conformance)
            })
            .count();
        match matches {
            1 => {}
            0 => diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` binds `{}` to `{type_name}`, which has no nominal conformance to `{}`",
                candidate.template_name, bound.subject_name, bound.carrier_name,
            ))),
            count => diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` binds `{}` to `{type_name}`, which has {count} conformances to `{}`; select one with `where {} satisfies {type_name}::Name`",
                candidate.template_name,
                bound.subject_name,
                bound.carrier_name,
                bound.subject_name,
            ))),
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn concrete_data_type_name(program: &TypedTrees, handle: TypeReferenceHandle) -> Option<&str> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => concrete_data_type_name(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            concrete_data_type_name(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, name }
            if symbol.is_valid() && program.symbols.get(*symbol).kind == SymbolKind::Data =>
        {
            Some(name.as_str())
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } if base_symbol.is_valid()
            && program.symbols.get(*base_symbol).kind == SymbolKind::Data =>
        {
            Some(base_name.as_str())
        }
        _ => None,
    }
}

fn conformance_application_arguments_match_candidate(
    program: &TypedTrees,
    candidate: &Candidate,
    bound: &psi_typed_trees::machine::GenericConformanceBound,
    application: &psi_typed_trees::typed_trees::ClosedConformanceApplication,
) -> bool {
    let substitutions = candidate
        .type_parameters
        .iter()
        .zip(candidate.type_bindings.iter())
        .filter_map(|((symbol, _), binding)| {
            binding.map(|binding| (*symbol, program.display_type_reference(binding)))
        })
        .chain(
            candidate
                .const_parameters
                .iter()
                .zip(candidate.const_bindings.iter())
                .filter_map(|((symbol, _, _), binding)| {
                    binding.map(|binding| (*symbol, program.display_type_reference(binding)))
                }),
        )
        .collect::<Vec<_>>();
    let machine_lifetimes = program.machines()[candidate.machine_index]
        .lifetime_parameters
        .iter()
        .map(|parameter| {
            (
                parameter.as_str().to_owned(),
                "__ordinary_call_region".to_owned(),
            )
        })
        .collect::<Vec<_>>();
    application.trait_arguments.len() == bound.arguments.len()
        && bound
            .arguments
            .iter()
            .zip(application.trait_arguments.iter())
            .all(|(required, actual)| {
                let required =
                    crate::conformance_applications::substituted_type_identity_with_lifetimes(
                        program,
                        *required,
                        &substitutions,
                        &machine_lifetimes,
                    );
                let actual = application.lifetime_arguments.iter().fold(
                    actual.clone(),
                    |identity, lifetime| {
                        identity.replace(&format!("'{lifetime}"), "'__ordinary_call_region")
                    },
                );
                required == actual
            })
}

fn conformance_arguments_match_candidate(
    program: &TypedTrees,
    candidate: &Candidate,
    bound: &psi_typed_trees::machine::GenericConformanceBound,
    conformance: &psi_typed_trees::trait_definition::Conformance,
) -> bool {
    let actual = program
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    actual.len() == bound.arguments.len()
        && bound
            .arguments
            .iter()
            .zip(actual.iter())
            .all(|(required, actual)| {
                let required = candidate
                    .type_parameters
                    .iter()
                    .zip(candidate.type_bindings.iter())
                    .find_map(|((symbol, _), binding)| {
                        let TypeReferenceNode::Named {
                            symbol: required_symbol,
                            ..
                        } = program.type_reference_table.type_reference(*required)
                        else {
                            return None;
                        };
                        (*symbol == *required_symbol)
                            .then_some(binding.as_ref().copied())
                            .flatten()
                    })
                    .unwrap_or(*required);
                same_type_identity(program, required, *actual)
            })
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
    let mut conformance_diagnostics = Vec::new();
    for candidate in &concrete_candidates {
        if let Err(mut errors) = validate_candidate_conformance_bounds(program, candidate) {
            conformance_diagnostics.append(&mut errors);
        }
    }
    if !conformance_diagnostics.is_empty() {
        return Err(conformance_diagnostics);
    }

    if selections
        .iter()
        .any(|selection| selection.candidate_index == candidate_index && !selection.is_complete())
    {
        return Err(vec![Diagnostic::error(format!(
            "generic machine `{}` has a static selection, but its complete type/const/machine/conformance specialization tuple cannot be derived",
            template.template_name
        ))]);
    }

    // Clones must be sourced from the untouched generic graph. The first
    // tuple reuses the authored declaration in place; subsequent tuples are
    // copied from this snapshot, receive fresh lexical symbols, and are then
    // rewritten independently.
    let source = program.clone();
    let template_contract_fingerprint =
        template_contract_fingerprint(&source, template.machine_index);
    let accepted_template_commitment =
        accepted_template_commitment(&source, template.machine_index);
    apply_specialization(program, &concrete_candidates[0]);
    if let Some(first) = program.machine_specializations.last_mut() {
        first.template_contract_fingerprint = template_contract_fingerprint;
        first.accepted_template_commitment = accepted_template_commitment.clone();
    }

    for (group_index, ((_, members), candidate)) in groups
        .iter()
        .zip(concrete_candidates.iter())
        .enumerate()
        .skip(1)
    {
        let state_symbols = clone_specialized_machine(
            &source,
            program,
            candidate,
            group_index,
            template_contract_fingerprint,
            accepted_template_commitment.clone(),
        );
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
    candidate.const_bindings = selection.const_bindings.clone();
    candidate.machine_bindings = selection.machine_bindings.clone();
    candidate.evidence_bindings = selection.evidence_bindings.clone();
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
    template_contract_fingerprint: u64,
    accepted_template_commitment: Option<String>,
) -> Vec<(SymbolHandle, SymbolHandle)> {
    let source_machine = &source.machines()[candidate.machine_index];
    let source_states = source.machine_states(source_machine).to_vec();
    let source_owned = source.machine_owned_data(source_machine).to_vec();
    let specialized_attached_data = specialized_attached_data(source, candidate, source_machine);
    let inherited_field_names = specialized_attached_data
        .as_ref()
        .into_iter()
        .flat_map(|attached_data| {
            source
                .data_definitions()
                .iter()
                .filter(move |data| data.name == *attached_data)
        })
        .flat_map(|data| source.data_members(data))
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => Some(field.name.as_str().to_owned()),
            psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let type_start = program.type_reference_table.type_reference_count();
    let expression_start = program.expression_table.iter_expressions().count();

    let type_arguments: Vec<String> = candidate
        .type_bindings
        .iter()
        .map(|binding| source.display_type_reference(binding.expect("complete specialization")))
        .collect();
    let type_identities: Vec<String> = candidate
        .type_bindings
        .iter()
        .map(|binding| {
            source
                .normalized_type_identity(binding.expect("complete specialization"))
                .into_string()
        })
        .collect();
    let const_arguments: Vec<String> = candidate
        .const_bindings
        .iter()
        .map(|binding| source.display_type_reference(binding.expect("complete specialization")))
        .collect();
    let const_identities: Vec<String> = candidate
        .const_bindings
        .iter()
        .map(|binding| {
            source
                .normalized_type_identity(binding.expect("complete specialization"))
                .into_string()
        })
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
    let evidence_paths: Vec<String> = candidate
        .evidence_bindings
        .iter()
        .map(|binding| {
            binding
                .as_ref()
                .expect("complete specialization")
                .display_name()
        })
        .collect();
    let fingerprint = specialization_fingerprint(
        &candidate.template_name,
        &type_identities,
        &const_identities,
        &machine_paths,
        &evidence_paths,
    );
    let generated_name = format!(
        "{}$specialized${fingerprint:016x}${ordinal}",
        candidate.template_name
    );
    let machine_symbol = program
        .symbols
        .insert_generated_root(SymbolKind::Machine, &generated_name);

    let machine_children = program.symbols.insert_generated_children(
        machine_symbol,
        inherited_field_names
            .iter()
            .map(|name| (SymbolKind::Field, name.as_str()))
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
        psi_symbols::SymbolTableBuilder::child_handles(machine_children).collect();
    let mut next_child = machine_children.into_iter();
    let mut symbol_map = vec![(source_machine.symbol, machine_symbol)];
    let source_machine_children = source_machine
        .symbol
        .is_valid()
        .then(|| source.symbols.child_handles(source_machine.symbol))
        .flatten()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    for field_name in &inherited_field_names {
        let cloned_field = next_child.next().expect("inherited-field clone symbol");
        let source_fields = source_machine_children
            .iter()
            .copied()
            .filter(|symbol| {
                source.symbols.get(*symbol).kind == SymbolKind::Field
                    && source.symbols.name(*symbol) == field_name
            })
            .collect::<Vec<_>>();
        if let [source_field] = source_fields.as_slice() {
            symbol_map.push((*source_field, cloned_field));
        }
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
        let mut children = psi_symbols::SymbolTableBuilder::child_handles(children);
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
    cloned.name = psi_typed_trees::name::Identifier::generated(generated_name);
    cloned.attached_data = specialized_attached_data;
    cloned.type_parameters = HandleSpan::empty();
    cloned.conformance_bounds.clear();
    cloned.owned_data = HandleSpan::empty();
    cloned.satisfies = HandleSpan::empty();
    if let Some(subjects) =
        psi_typed_trees::ranking::resolve_machine_witness_subjects(source, source_machine)
    {
        for expression in subjects {
            let _ = copy_expression(source, program, expression, &symbol_map);
        }
    }
    if let Some(arguments) =
        psi_typed_trees::ranking::resolve_machine_witness_view_arguments(source, source_machine)
    {
        for expression in arguments {
            let _ = copy_expression(source, program, expression, &symbol_map);
        }
    }
    cloned.contracts = HandleSpan::empty();
    cloned.states = HandleSpan::empty();

    let owned_symbol_offset = 1;
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
        let contract = copy_signature_contract(source, program, contract.clone(), &symbol_map);
        program.push_machine_contract(&mut cloned, contract);
    }

    for (source_state, (_, fresh_symbol)) in source_states.iter().zip(state_symbols.iter()) {
        let mut state = source_state.clone();
        state.symbol = *fresh_symbol;
        state.parameters = HandleSpan::empty();
        state.contracts = HandleSpan::empty();
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
        for contract in source.state_contracts(source_state) {
            let contract = copy_signature_contract(source, program, contract.clone(), &symbol_map);
            program.push_state_contract(&mut state, contract);
        }
        program.push_machine_state(&mut cloned, state);
    }

    copy_cloned_expression_type_payloads(source, program, expression_start, &symbol_map);
    substitute_cloned_type_parameters(source, program, candidate, type_start);
    rewrite_cloned_calls(
        source,
        program,
        candidate,
        &state_symbols,
        expression_start,
        cloned.states,
    );
    resolve_specialized_receiver_calls(program, &cloned);
    let instance_symbol = cloned.symbol;
    program.push_machine(cloned);
    program
        .machine_specializations
        .push(psi_typed_trees::typed_trees::MachineSpecialization {
            template: candidate.template_symbol,
            instance: instance_symbol,
            type_arguments,
            const_arguments,
            type_argument_identities: type_identities,
            const_argument_identities: const_identities,
            machine_arguments: candidate
                .machine_bindings
                .iter()
                .map(|binding| binding.as_ref().expect("complete specialization").symbol)
                .collect(),
            conformance_arguments: candidate
                .evidence_bindings
                .iter()
                .map(|binding| binding.as_ref().expect("complete specialization").symbol)
                .collect(),
            conformance_applications: candidate
                .evidence_bindings
                .iter()
                .map(|binding| {
                    crate::conformance_applications::close_conformance_application(
                        source,
                        binding.as_ref().expect("complete specialization"),
                    )
                    .expect("validated closed conformance application")
                })
                .collect(),
            template_contract_fingerprint,
            accepted_template_commitment,
            machine_argument_contract_fingerprints: Vec::new(),
            conformance_argument_fingerprints: Vec::new(),
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

/// `ExpressionTable::copy_from` owns expression recursion but deliberately
/// cannot clone handles from the separate type-reference table. A specialized
/// machine is a new semantic graph, so copy cast/zero-value type payloads here
/// before binder substitution; otherwise a cloned cast would still point into
/// the first in-place specialization's argument span.
fn copy_cloned_expression_type_payloads(
    source: &TypedTrees,
    program: &mut TypedTrees,
    expression_start: usize,
    symbols: &[(SymbolHandle, SymbolHandle)],
) {
    let cast_payloads = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() as usize >= expression_start)
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            Some((handle, cast.target_type, cast.semantic_domain_arguments))
        })
        .collect::<Vec<_>>();
    for (handle, target_type, arguments) in cast_payloads {
        let target_type = copy_type_reference(source, program, target_type, symbols);
        let copied_arguments = source
            .type_reference_table
            .type_reference_handles(arguments)
            .iter()
            .map(|argument| copy_type_reference(source, program, *argument, symbols))
            .collect::<Vec<_>>();
        let arguments = program
            .type_reference_table
            .insert_type_reference_handles(copied_arguments);
        let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(handle) else {
            unreachable!("collected cast changed kind")
        };
        cast.target_type = target_type;
        cast.semantic_domain_arguments = arguments;
    }

    let zero_values = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() as usize >= expression_start)
        .filter_map(|(handle, expression)| {
            let ExpressionNode::ZeroValue(type_reference) = expression else {
                return None;
            };
            Some((handle, *type_reference))
        })
        .collect::<Vec<_>>();
    for (handle, type_reference) in zero_values {
        let type_reference = copy_type_reference(source, program, type_reference, symbols);
        let ExpressionNode::ZeroValue(current) = program.expression_table.expression_mut(handle)
        else {
            unreachable!("collected zero-value expression changed kind")
        };
        *current = type_reference;
    }
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
    contract: psi_typed_trees::signature::SignatureContract,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> psi_typed_trees::signature::SignatureContract {
    let original_facts = contract.facts;
    let mut copied = contract;
    copied.facts = HandleSpan::empty();
    for fact in source.proof_facts.span_or_empty(original_facts) {
        let fact = match fact {
            psi_typed_trees::domain::ProofFact::Expression(expression) => {
                psi_typed_trees::domain::ProofFact::Expression(copy_expression(
                    source,
                    program,
                    *expression,
                    symbols,
                ))
            }
            psi_typed_trees::domain::ProofFact::Membership(membership) => {
                psi_typed_trees::domain::ProofFact::Membership(
                    psi_typed_trees::domain::ProofMembershipFact {
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
            psi_typed_trees::domain::ProofFact::Proposition(application) => {
                let arguments = source
                    .expression_table
                    .expression_handles(application.arguments)
                    .iter()
                    .map(|argument| copy_expression(source, program, *argument, symbols))
                    .collect::<Vec<_>>();
                let arguments = program
                    .expression_table
                    .insert_expression_handles(arguments);
                psi_typed_trees::domain::ProofFact::Proposition(
                    psi_typed_trees::proposition::PropositionApplication {
                        proposition: remapped_symbol(application.proposition, symbols),
                        name: application.name.clone(),
                        binder_arguments: application
                            .binder_arguments
                            .iter()
                            .map(|argument| {
                                psi_typed_trees::proposition::PropositionBinderArgument {
                                    kind: argument.kind,
                                    path: argument.path.clone(),
                                    const_literal: argument.const_literal.clone(),
                                    evidence_projection: argument.evidence_projection.clone(),
                                    symbol: remapped_symbol(argument.symbol, symbols),
                                }
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        arguments,
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
    for ((parameter_symbol, _, _), binding) in candidate
        .const_parameters
        .iter()
        .zip(candidate.const_bindings.iter())
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
            binding.expect("complete const specialization"),
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
    substitute_const_index_expression_parameters(program, candidate, Some(type_start));
    substitute_machine_parameter_type_references(program, candidate, Some(type_start));
}

fn substitute_const_index_expression_parameters(
    program: &mut TypedTrees,
    candidate: &Candidate,
    type_start: Option<usize>,
) {
    let replacements = candidate
        .const_parameters
        .iter()
        .zip(candidate.const_bindings.iter())
        .filter_map(|((parameter_symbol, parameter_name, _), binding)| {
            let TypeReferenceNode::Named { symbol, name } = program
                .type_reference_table
                .type_reference(binding.expect("complete const specialization"))
            else {
                return None;
            };
            Some((
                *parameter_symbol,
                parameter_name.as_str().to_owned(),
                *symbol,
                name.clone(),
            ))
        })
        .collect::<Vec<_>>();
    if replacements.is_empty() {
        return;
    }

    let expressions = if let Some(type_start) = type_start {
        program
            .type_reference_table
            .const_expression_sites()
            .into_iter()
            .filter(|(handle, _)| handle.arena_index() as usize >= type_start)
            .map(|(_, expression)| expression)
            .collect::<Vec<_>>()
    } else {
        candidate_const_index_expressions(program, candidate)
    };
    for expression in expressions {
        substitute_const_index_expression(&mut program.expression_table, expression, &replacements);
    }
}

fn candidate_const_index_expressions(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<ExpressionHandle> {
    let machine = &program.machines()[candidate.machine_index];
    let mut roots = program
        .machine_owned_data(machine)
        .iter()
        .map(|owned| owned.type_reference)
        .collect::<Vec<_>>();
    for state in program.machine_states(machine) {
        roots.extend(
            program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.type_reference),
        );
        roots.push(state.return_type);
        roots.extend(
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .filter_map(|statement| match statement {
                    StatementNode::LocalData(local) => Some(local.type_reference),
                    _ => None,
                }),
        );
    }

    let mut visited = Vec::new();
    let mut expressions = Vec::new();
    for root in roots {
        collect_const_index_expressions_from_type(
            &program.type_reference_table,
            root,
            &mut visited,
            &mut expressions,
        );
    }

    // Indexed qualification arguments live on cast expressions rather than
    // in the machine signature/local type graph. They are still owned by the
    // specialization and must receive the same const-binder substitution as
    // retained expressions in declared types.
    let mut body_expressions = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_expression_trees(program, statement, &mut body_expressions);
        }
    }
    for handle in body_expressions {
        let ExpressionNode::Cast(cast) = program.expression_table.expression(handle) else {
            continue;
        };
        collect_const_index_expressions_from_type(
            &program.type_reference_table,
            cast.target_type,
            &mut visited,
            &mut expressions,
        );
        for argument in program
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments)
        {
            collect_const_index_expressions_from_type(
                &program.type_reference_table,
                *argument,
                &mut visited,
                &mut expressions,
            );
        }
    }
    expressions
}

fn collect_statement_expression_trees(
    program: &TypedTrees,
    statement: &StatementNode,
    handles: &mut Vec<ExpressionHandle>,
) {
    match statement {
        StatementNode::AssemblyFact(fact) => {
            collect_expression_tree(program, fact.expression, handles)
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_tree(program, assignment.target, handles);
            collect_expression_tree(program, assignment.value, handles);
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_tree(program, *argument, handles);
            }
        }
        StatementNode::Expression(expression) => {
            collect_expression_tree(program, *expression, handles)
        }
        StatementNode::LocalData(local) => {
            collect_expression_tree(program, local.initial_value, handles)
        }
        StatementNode::Transition(transition) => {
            if let psi_typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_tree(program, guard, handles);
            }
            collect_transition_target_expression_trees(program, transition.target, handles);
            collect_transition_target_expression_trees(program, transition.continuation, handles);
        }
    }
}

fn collect_transition_target_expression_trees(
    program: &TypedTrees,
    target: psi_typed_trees::statement::TransitionTargetHandle,
    handles: &mut Vec<ExpressionHandle>,
) {
    if !target.is_valid() {
        return;
    }
    match program.statement_table.transition_target(target) {
        psi_typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_expression_tree(program, *argument, handles);
            }
        }
        psi_typed_trees::statement::TransitionTargetNode::Value(expression) => {
            collect_expression_tree(program, *expression, handles)
        }
        psi_typed_trees::statement::TransitionTargetNode::SelfTarget
        | psi_typed_trees::statement::TransitionTargetNode::Terminal => {}
    }
}

fn collect_const_index_expressions_from_type(
    table: &psi_typed_trees::types::TypeReferenceTable,
    type_reference: TypeReferenceHandle,
    visited: &mut Vec<TypeReferenceHandle>,
    expressions: &mut Vec<ExpressionHandle>,
) {
    if !type_reference.is_valid() || visited.contains(&type_reference) {
        return;
    }
    visited.push(type_reference);
    match table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_const_index_expressions_from_type(table, *referee, visited, expressions)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_const_index_expressions_from_type(table, *base_type, visited, expressions);
            for constraint in table.constraints(*constraints) {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    continue;
                };
                for argument in &domain.arguments {
                    collect_const_index_expressions_from_type(
                        table,
                        *argument,
                        visited,
                        expressions,
                    );
                }
            }
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_const_index_expressions_from_type(table, *element_type, visited, expressions)
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in table.type_reference_handles(*arguments) {
                collect_const_index_expressions_from_type(table, *argument, visited, expressions);
            }
        }
        TypeReferenceNode::ConstExpression(expression) => expressions.push(*expression),
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => {}
    }
}

fn substitute_const_index_expression(
    expressions: &mut psi_typed_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    replacements: &[(
        SymbolHandle,
        String,
        SymbolHandle,
        psi_typed_trees::name::Identifier,
    )],
) {
    match expressions.expression(expression).clone() {
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            let [member] = members else {
                return;
            };
            let Some((_, _, replacement_symbol, replacement_name)) =
                replacements
                    .iter()
                    .find(|(parameter_symbol, parameter_name, _, _)| {
                        (path.symbol.is_valid() && path.symbol == *parameter_symbol)
                            || member.as_str() == parameter_name
                    })
            else {
                return;
            };
            expressions.set_name_path_member_at_offset(path.members, 0, replacement_name.clone());
            if !path.member_symbols.is_empty() {
                expressions.set_name_path_member_symbol_at_offset(
                    path.member_symbols,
                    0,
                    *replacement_symbol,
                );
            }
            let ExpressionNode::Name(path) = expressions.expression_mut(expression) else {
                unreachable!("open-index expression changed while substituting")
            };
            path.head_symbol = *replacement_symbol;
            path.symbol = *replacement_symbol;
        }
        ExpressionNode::Binary(binary) => {
            substitute_const_index_expression(expressions, binary.left, replacements);
            substitute_const_index_expression(expressions, binary.right, replacements);
        }
        ExpressionNode::Unary(unary) => {
            substitute_const_index_expression(expressions, unary.operand, replacements)
        }
        _ => {}
    }
}

fn substitute_machine_parameter_type_references(
    program: &mut TypedTrees,
    candidate: &Candidate,
    type_start: Option<usize>,
) {
    for ((parameter_symbol, _, _), binding) in candidate
        .machine_parameters
        .iter()
        .zip(candidate.machine_bindings.iter())
    {
        let binding = binding.as_ref().expect("complete specialization");
        let name = binding
            .path
            .last()
            .cloned()
            .or_else(|| state_by_symbol(program, binding.symbol).map(|state| state.name.clone()))
            .expect("admitted static machine argument has an entry name");
        let occurrences: Vec<_> = program
            .type_reference_table
            .named_references()
            .filter(|(handle, symbol, _)| {
                type_start.is_none_or(|start| handle.arena_index() as usize >= start)
                    && symbol == parameter_symbol
            })
            .map(|(handle, _, _)| handle)
            .collect();
        for occurrence in occurrences {
            program.type_reference_table.substitute_node(
                occurrence,
                TypeReferenceNode::Named {
                    symbol: binding.symbol,
                    name: name.clone(),
                },
            );
        }
    }
}

fn substitute_forwarded_machine_arguments(
    arguments: &mut [StaticMachineArgument],
    static_rewrites: &[(SymbolHandle, StaticMachineArgument)],
    rewrites: &[(
        SymbolHandle,
        SymbolHandle,
        psi_typed_trees::name::Identifier,
    )],
) {
    for argument in arguments {
        if let Some((_, replacement)) = static_rewrites
            .iter()
            .find(|(parameter, _)| *parameter == argument.symbol)
        {
            *argument = replacement.clone();
        } else if let Some((_, symbol, name)) = rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == argument.symbol)
        {
            argument.symbol = *symbol;
            argument.path = vec![name.clone()].into_boxed_slice();
        }
        if let Some(application) = &mut argument.application {
            substitute_forwarded_machine_arguments(
                &mut application.arguments,
                static_rewrites,
                rewrites,
            );
        }
    }
}

fn forwarded_static_argument_rewrites(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<(SymbolHandle, StaticMachineArgument)> {
    candidate
        .type_parameters
        .iter()
        .zip(candidate.type_bindings.iter())
        .filter_map(|((parameter, _), binding)| {
            static_argument_from_type_reference(program, binding.as_ref().copied()?)
                .map(|argument| (*parameter, argument))
        })
        .chain(
            candidate
                .const_parameters
                .iter()
                .zip(candidate.const_bindings.iter())
                .filter_map(|((parameter, _, _), binding)| {
                    static_const_literal_from_type_reference(program, binding.as_ref().copied()?)
                        .map(|literal| {
                            (
                                *parameter,
                                StaticMachineArgument {
                                    path: Box::default(),
                                    application: None,
                                    const_literal: Some(literal),
                                    evidence_projection: None,
                                    symbol: SymbolHandle::invalid(),
                                },
                            )
                        })
                }),
        )
        .collect()
}

fn static_argument_from_type_reference(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<StaticMachineArgument> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { symbol, name } => Some(StaticMachineArgument {
            path: vec![name.clone()].into_boxed_slice(),
            application: None,
            const_literal: None,
            evidence_projection: None,
            symbol: *symbol,
        }),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => Some(StaticMachineArgument {
            path: vec![base_name.clone()].into_boxed_slice(),
            application: Some(Box::new(
                psi_typed_trees::expression::StaticSymbolApplication {
                    lifetime_arguments: lifetime_arguments.clone().into_boxed_slice(),
                    arguments: program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .filter_map(|argument| {
                            static_argument_from_type_reference(program, *argument)
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                },
            )),
            const_literal: None,
            evidence_projection: None,
            symbol: *base_symbol,
        }),
        _ => None,
    }
}

fn static_const_literal_from_type_reference(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<psi_numerics::literals::IntegerLiteral> {
    let TypeReferenceNode::Named { name, .. } = program.type_reference_table.type_reference(handle)
    else {
        return None;
    };
    let mut spelling = name.as_str();
    let negative = spelling.starts_with('-');
    if negative {
        spelling = &spelling[1..];
    }
    let (radix, digits) = if let Some(digits) = spelling.strip_prefix("0b") {
        (psi_numerics::literals::IntegerRadix::Binary, digits)
    } else if let Some(digits) = spelling.strip_prefix("0o") {
        (psi_numerics::literals::IntegerRadix::Octal, digits)
    } else if let Some(digits) = spelling.strip_prefix("0x") {
        (psi_numerics::literals::IntegerRadix::Hexadecimal, digits)
    } else {
        (psi_numerics::literals::IntegerRadix::Decimal, spelling)
    };
    psi_numerics::literals::IntegerLiteral::from_parts(negative, radix, digits).ok()
}

fn evidence_argument_rewrites(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<(
    SymbolHandle,
    SymbolHandle,
    psi_typed_trees::name::Identifier,
)> {
    candidate
        .evidence_parameters
        .iter()
        .zip(candidate.evidence_bindings.iter())
        .filter_map(|(parameter, binding)| {
            let binder = parameter.binder?;
            let binding = binding.as_ref()?;
            let selected = program
                .conformances()
                .iter()
                .find(|conformance| conformance.symbol == binding.symbol)?;
            Some((
                binder,
                binding.symbol,
                selected.alias.clone().unwrap_or_else(|| {
                    psi_typed_trees::name::Identifier::generated("<unnamed-conformance>")
                }),
            ))
        })
        .collect()
}

struct EvidenceRequirementRewrite {
    placeholder: SymbolHandle,
    target: SymbolHandle,
    name: psi_typed_trees::name::Identifier,
    application_arguments: Box<[StaticMachineArgument]>,
}

fn evidence_requirement_rewrites(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<EvidenceRequirementRewrite> {
    let mut rewrites = Vec::new();
    for (parameter, binding) in candidate
        .evidence_parameters
        .iter()
        .zip(candidate.evidence_bindings.iter())
    {
        let (Some(binder), Some(binding)) = (parameter.binder, binding.as_ref()) else {
            continue;
        };
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|trait_definition| trait_definition.symbol == parameter.carrier)
        else {
            continue;
        };
        let Some(selected) = program
            .conformances()
            .iter()
            .find(|conformance| conformance.symbol == binding.symbol)
        else {
            continue;
        };
        let Some(rows) = program.closed_conformance_rows(selected) else {
            continue;
        };
        let mut requirements = Vec::new();
        collect_evidence_requirement_closure(
            program,
            trait_definition,
            &mut Vec::new(),
            &mut requirements,
        );
        let placeholders = program.symbols.child_handles(binder).into_iter().flatten();
        for (placeholder, requirement) in placeholders.zip(requirements) {
            let Some(row) = rows
                .iter()
                .find(|row| row.requirement == requirement.symbol)
            else {
                continue;
            };
            rewrites.push(EvidenceRequirementRewrite {
                placeholder,
                target: row.realization_state,
                name: psi_typed_trees::name::Identifier::generated(
                    row.realization_name
                        .as_str()
                        .rsplit("::")
                        .next()
                        .unwrap_or(row.realization_name.as_str()),
                ),
                application_arguments: binding
                    .application
                    .as_ref()
                    .map_or_else(Box::default, |application| application.arguments.clone()),
            });
        }
    }
    rewrites
}

fn collect_evidence_requirement_closure<'program>(
    program: &'program TypedTrees,
    trait_definition: &'program psi_typed_trees::trait_definition::TraitDefinition,
    visited: &mut Vec<SymbolHandle>,
    output: &mut Vec<&'program StateSignature>,
) {
    if visited.contains(&trait_definition.symbol) {
        return;
    }
    visited.push(trait_definition.symbol);
    output.extend(program.trait_machine_signatures(trait_definition).iter());
    for parent in program.trait_requirements(trait_definition) {
        let Some(parent_trait) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == parent.symbol)
        else {
            continue;
        };
        collect_evidence_requirement_closure(program, parent_trait, visited, output);
    }
}

fn span_without_first<T>(span: HandleSpan<T>) -> HandleSpan<T> {
    if span.count() <= 1 {
        return HandleSpan::empty();
    }
    let start = span.start();
    HandleSpan::from_parts(
        Handle::from_parts(
            start
                .arena_index()
                .checked_add(1)
                .expect("argument span index overflow"),
            start.generation(),
        ),
        span.count() - 1,
    )
}

fn statement_span_handles(span: HandleSpan<StatementNode>) -> Vec<Handle<StatementNode>> {
    (0..span.count())
        .map(|offset| {
            Handle::from_parts(
                span.start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("statement span index overflow"),
                span.start().generation(),
            )
        })
        .collect()
}

fn statement_receiver_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Vec<psi_typed_trees::name::Identifier>)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => statement_receiver_path(program, atomic.value),
        ExpressionNode::Mutable(inner) => statement_receiver_path(program, *inner),
        ExpressionNode::Name(path) => Some((
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .to_vec(),
        )),
        ExpressionNode::Member(member) => {
            let (symbol, mut path) = statement_receiver_path(program, member.receiver)?;
            path.push(member.member.clone());
            Some((symbol, path))
        }
        _ => None,
    }
}

fn rewrite_cloned_calls(
    source: &TypedTrees,
    program: &mut TypedTrees,
    candidate: &Candidate,
    state_symbols: &[(SymbolHandle, SymbolHandle)],
    expression_start: usize,
    states: HandleSpan<psi_typed_trees::state::State>,
) {
    let machine_rewrites: Vec<_> = candidate
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
    let evidence_target_rewrites = evidence_requirement_rewrites(source, candidate);
    let mut target_rewrites = machine_rewrites.clone();
    target_rewrites.extend(
        evidence_target_rewrites
            .iter()
            .map(|rewrite| (rewrite.placeholder, rewrite.target, rewrite.name.clone())),
    );
    let mut argument_rewrites = machine_rewrites;
    argument_rewrites.extend(evidence_argument_rewrites(source, candidate));
    let static_argument_rewrites = forwarded_static_argument_rewrites(source, candidate);
    for state in program.machine_states.span_or_empty(states).to_vec() {
        for statement_handle in statement_span_handles(state.statement_nodes) {
            let StatementNode::Call(snapshot) =
                program.statement_table.statement(statement_handle).clone()
            else {
                continue;
            };
            let evidence_dispatch = evidence_target_rewrites
                .iter()
                .find(|rewrite| rewrite.placeholder == snapshot.target_symbol);
            let evidence_receiver = evidence_dispatch
                .is_some()
                .then(|| {
                    program
                        .statement_table
                        .expression_handles(snapshot.arguments)
                        .first()
                        .copied()
                })
                .flatten()
                .and_then(|receiver| statement_receiver_path(program, receiver));
            let receiver = evidence_receiver.as_ref().map(|(_, members)| {
                let mut receiver = HandleSpan::empty();
                for member in members {
                    program
                        .statement_table
                        .push_name_path_member(&mut receiver, member.clone());
                }
                receiver
            });
            let StatementNode::Call(call) = program.statement_table.statement_mut(statement_handle)
            else {
                unreachable!();
            };
            if let Some((_, target, name)) = target_rewrites
                .iter()
                .find(|(parameter, _, _)| *parameter == call.target_symbol)
            {
                call.target_symbol = *target;
                call.target = name.clone();
            }
            if let (Some((receiver_symbol, _)), Some(receiver)) = (evidence_receiver, receiver) {
                call.receiver_symbol = receiver_symbol;
                call.receiver = receiver;
                call.arguments = span_without_first(call.arguments);
            }
            if let Some(rewrite) = evidence_dispatch {
                call.machine_arguments = rewrite.application_arguments.clone();
            }
            substitute_forwarded_machine_arguments(
                &mut call.machine_arguments,
                &static_argument_rewrites,
                &argument_rewrites,
            );
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
        let evidence_dispatch = match program.expression_table.expression(handle) {
            ExpressionNode::Call(call) => evidence_target_rewrites
                .iter()
                .find(|rewrite| rewrite.placeholder == call.target_symbol),
            _ => None,
        };
        let evidence_receiver = match program.expression_table.expression(handle) {
            ExpressionNode::Call(call) if evidence_dispatch.is_some() => program
                .expression_table
                .expression_handles(call.arguments)
                .first()
                .copied(),
            _ => None,
        };
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            continue;
        };
        if let Some((_, target, name)) = target_rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == call.target_symbol)
        {
            call.target_symbol = *target;
            call.target = name.clone();
        }
        if evidence_dispatch.is_some()
            && let Some(receiver) = evidence_receiver
        {
            call.receiver = receiver;
            call.arguments = span_without_first(call.arguments);
        }
        if let Some(rewrite) = evidence_dispatch {
            call.machine_arguments = rewrite.application_arguments.clone();
        }
        substitute_forwarded_machine_arguments(
            &mut call.machine_arguments,
            &static_argument_rewrites,
            &argument_rewrites,
        );
        if state_symbols
            .iter()
            .any(|(_, concrete)| *concrete == call.target_symbol)
        {
            call.machine_arguments = Box::default();
        }
    }
}

/// Const substitution changes an indexed-domain instance from binder identity
/// (`Quantity<To>`) to the selected canonical value (`Quantity<METER>`). The
/// semantic ID is derived data, so refresh every affected constraint and cast
/// before validation or checked-fact construction observes the specialized
/// graph.
pub fn refresh_closed_domain_instance_identities(
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    let mut constraint_updates = Vec::new();
    for (_, _, constraints) in program
        .type_reference_table
        .constrained_type_reference_sites()
    {
        for (offset, constraint) in program
            .type_reference_table
            .constraints(constraints)
            .iter()
            .enumerate()
        {
            let TypeConstraintNode::Domain(constraint) = constraint else {
                continue;
            };
            if constraint.arguments.is_empty() || !constraint.symbol.is_valid() {
                continue;
            }
            let Some(domain) = program
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == constraint.symbol)
                .cloned()
            else {
                continue;
            };
            let parameters = program.domain_type_parameters(&domain);
            let index_parameters = if parameters.is_empty() {
                &[][..]
            } else {
                &parameters[1..]
            };
            let identity = psi_typed_trees::domain::indexed_domain_instance_name(
                program,
                &domain,
                index_parameters,
                &constraint.arguments,
            )?;
            constraint_updates.push((constraints, offset, identity, domain.semantic_roles));
        }
    }

    for (constraints, offset, identity, roles) in constraint_updates {
        let semantic_id = program.semantic_domains.intern(&identity);
        let TypeConstraintNode::Domain(domain) =
            &mut program.type_reference_table.constraints_mut(constraints)[offset]
        else {
            unreachable!("collected domain constraint changed kind")
        };
        domain.semantic_id = semantic_id;
        domain.semantic_roles = psi_language_semantics::DomainSemanticRoles {
            denotation_dimension: roles.denotation_dimension.map(|_| semantic_id),
            arithmetic_policy: roles.arithmetic_policy.map(|_| semantic_id),
        };
    }

    let cast_sites = program
        .expression_table
        .expression_entries()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            (cast.semantic_domain_symbol.is_valid() && !cast.semantic_domain_arguments.is_empty())
                .then_some((
                    handle,
                    cast.semantic_domain_symbol,
                    cast.semantic_domain_arguments,
                ))
        })
        .collect::<Vec<_>>();
    let mut cast_updates = Vec::new();
    for (handle, symbol, arguments) in cast_sites {
        let Some(domain) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
            .cloned()
        else {
            continue;
        };
        let parameters = program.domain_type_parameters(&domain);
        let index_parameters = if parameters.is_empty() {
            &[][..]
        } else {
            &parameters[1..]
        };
        let arguments = program
            .type_reference_table
            .type_reference_handles(arguments);
        let identity = psi_typed_trees::domain::indexed_domain_instance_name(
            program,
            &domain,
            index_parameters,
            arguments,
        )?;
        cast_updates.push((handle, identity));
    }
    for (handle, identity) in cast_updates {
        let semantic_id = program.semantic_domains.intern(&identity);
        let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(handle) else {
            unreachable!("collected cast changed kind")
        };
        cast.semantic_domain_id = semantic_id;
    }

    Ok(())
}

fn remapped_symbol(symbol: SymbolHandle, symbols: &[(SymbolHandle, SymbolHandle)]) -> SymbolHandle {
    symbols
        .iter()
        .find_map(|(before, after)| (*before == symbol).then_some(*after))
        .unwrap_or(symbol)
}

fn apply_specialization(program: &mut TypedTrees, candidate: &Candidate) {
    let template_contract_fingerprint =
        template_contract_fingerprint(program, candidate.machine_index);
    let accepted_template_commitment =
        accepted_template_commitment(program, candidate.machine_index);
    let type_arguments: Vec<String> = candidate
        .type_bindings
        .iter()
        .map(|binding| {
            program.display_type_reference(binding.expect("complete type specialization"))
        })
        .collect();
    let type_identities: Vec<String> = candidate
        .type_bindings
        .iter()
        .map(|binding| {
            program
                .normalized_type_identity(binding.expect("complete type specialization"))
                .into_string()
        })
        .collect();
    let const_arguments: Vec<String> = candidate
        .const_bindings
        .iter()
        .map(|binding| {
            program.display_type_reference(binding.expect("complete const specialization"))
        })
        .collect();
    let const_identities: Vec<String> = candidate
        .const_bindings
        .iter()
        .map(|binding| {
            program
                .normalized_type_identity(binding.expect("complete const specialization"))
                .into_string()
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
    let evidence_paths: Vec<String> = candidate
        .evidence_bindings
        .iter()
        .map(|binding| {
            binding
                .as_ref()
                .expect("complete specialization")
                .display_name()
        })
        .collect();
    let conformance_applications = candidate
        .evidence_bindings
        .iter()
        .map(|binding| {
            crate::conformance_applications::close_conformance_application(
                program,
                binding.as_ref().expect("complete specialization"),
            )
            .expect("validated closed conformance application")
        })
        .collect();
    let fingerprint = specialization_fingerprint(
        &candidate.template_name,
        &type_identities,
        &const_identities,
        &machine_paths,
        &evidence_paths,
    );
    program
        .machine_specializations
        .push(psi_typed_trees::typed_trees::MachineSpecialization {
            template: candidate.template_symbol,
            instance: candidate.template_symbol,
            type_arguments: type_arguments.clone(),
            const_arguments,
            type_argument_identities: type_identities,
            const_argument_identities: const_identities,
            machine_arguments,
            conformance_arguments: candidate
                .evidence_bindings
                .iter()
                .map(|binding| binding.as_ref().expect("complete specialization").symbol)
                .collect(),
            conformance_applications,
            template_contract_fingerprint,
            accepted_template_commitment,
            machine_argument_contract_fingerprints: Vec::new(),
            conformance_argument_fingerprints: Vec::new(),
            fingerprint,
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

    for ((parameter_symbol, parameter_name, _), binding) in candidate
        .const_parameters
        .iter()
        .zip(candidate.const_bindings.iter())
    {
        let replacement = program
            .type_reference_table
            .type_reference(binding.expect("complete const specialization"))
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
    substitute_const_index_expression_parameters(program, candidate, None);

    let machine_rewrites: Vec<(
        SymbolHandle,
        SymbolHandle,
        psi_typed_trees::name::Identifier,
    )> = candidate
        .machine_parameters
        .iter()
        .zip(candidate.machine_bindings.iter())
        .map(|((parameter_symbol, _, _), binding)| {
            let binding = binding.as_ref().expect("complete machine specialization");
            // Preserve the authored symbol leaf for interpreter dispatch.
            // Free machines expose an internal body state named `entry`; using
            // that implementation detail here makes `F(value)` look like a
            // sibling-state call and recursively re-enters the generic helper.
            // The selected path is exact for both free (`chosen`) and attached
            // (`Card::power`) machines; the state name is only a recovery path
            // for synthetic arguments without authored path members.
            let target = binding
                .path
                .last()
                .cloned()
                .or_else(|| {
                    state_by_symbol(program, binding.symbol).map(|state| state.name.clone())
                })
                .expect("admitted static machine argument has an entry name");
            (*parameter_symbol, binding.symbol, target)
        })
        .collect();
    let evidence_target_rewrites = evidence_requirement_rewrites(program, candidate);
    let mut target_rewrites = machine_rewrites.clone();
    target_rewrites.extend(
        evidence_target_rewrites
            .iter()
            .map(|rewrite| (rewrite.placeholder, rewrite.target, rewrite.name.clone())),
    );
    let mut argument_rewrites = machine_rewrites;
    argument_rewrites.extend(evidence_argument_rewrites(program, candidate));
    let static_argument_rewrites = forwarded_static_argument_rewrites(program, candidate);

    substitute_machine_parameter_type_references(program, candidate, None);

    let state_spans: Vec<HandleSpan<StatementNode>> = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .map(|state| state.statement_nodes)
        .collect();
    for span in state_spans {
        for statement_handle in statement_span_handles(span) {
            let StatementNode::Call(snapshot) =
                program.statement_table.statement(statement_handle).clone()
            else {
                continue;
            };
            let evidence_dispatch = evidence_target_rewrites
                .iter()
                .find(|rewrite| rewrite.placeholder == snapshot.target_symbol);
            let evidence_receiver = evidence_dispatch
                .is_some()
                .then(|| {
                    program
                        .statement_table
                        .expression_handles(snapshot.arguments)
                        .first()
                        .copied()
                })
                .flatten()
                .and_then(|receiver| statement_receiver_path(program, receiver));
            let receiver = evidence_receiver.as_ref().map(|(_, members)| {
                let mut receiver = HandleSpan::empty();
                for member in members {
                    program
                        .statement_table
                        .push_name_path_member(&mut receiver, member.clone());
                }
                receiver
            });
            let StatementNode::Call(call) = program.statement_table.statement_mut(statement_handle)
            else {
                unreachable!();
            };
            if let Some((_, symbol, name)) = target_rewrites
                .iter()
                .find(|(parameter, _, _)| *parameter == call.target_symbol)
            {
                call.target_symbol = *symbol;
                call.target = name.clone();
            }
            if let (Some((receiver_symbol, _)), Some(receiver)) = (evidence_receiver, receiver) {
                call.receiver_symbol = receiver_symbol;
                call.receiver = receiver;
                call.arguments = span_without_first(call.arguments);
            }
            if let Some(rewrite) = evidence_dispatch {
                call.machine_arguments = rewrite.application_arguments.clone();
            }
            substitute_forwarded_machine_arguments(
                &mut call.machine_arguments,
                &static_argument_rewrites,
                &argument_rewrites,
            );
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
        let evidence_dispatch = match program.expression_table.expression(handle) {
            ExpressionNode::Call(call) => evidence_target_rewrites
                .iter()
                .find(|rewrite| rewrite.placeholder == call.target_symbol),
            _ => None,
        };
        let evidence_receiver = match program.expression_table.expression(handle) {
            ExpressionNode::Call(call) if evidence_dispatch.is_some() => program
                .expression_table
                .expression_handles(call.arguments)
                .first()
                .copied(),
            _ => None,
        };
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            continue;
        };
        if let Some((_, symbol, name)) = target_rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == call.target_symbol)
        {
            call.target_symbol = *symbol;
            call.target = name.clone();
        }
        if evidence_dispatch.is_some()
            && let Some(receiver) = evidence_receiver
        {
            call.receiver = receiver;
            call.arguments = span_without_first(call.arguments);
        }
        if let Some(rewrite) = evidence_dispatch {
            call.machine_arguments = rewrite.application_arguments.clone();
        }
        substitute_forwarded_machine_arguments(
            &mut call.machine_arguments,
            &static_argument_rewrites,
            &argument_rewrites,
        );
        if candidate.state_symbols.contains(&call.target_symbol) {
            call.machine_arguments = Box::default();
        }
    }

    let attached_data = {
        let template = &program.machines()[candidate.machine_index];
        specialized_attached_data(program, candidate, template)
    };
    program.machines_mut()[candidate.machine_index].attached_data = attached_data;
    let specialized = program.machines()[candidate.machine_index].clone();
    resolve_specialized_receiver_calls(program, &specialized);

    let specialized = &mut program.machines_mut()[candidate.machine_index];
    specialized.type_parameters = HandleSpan::empty();
    specialized.conformance_bounds.clear();
}

fn specialized_attached_data(
    program: &TypedTrees,
    candidate: &Candidate,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<psi_typed_trees::name::Identifier> {
    let attached = machine.attached_data.as_ref()?;
    let parameter_index = candidate
        .type_parameters
        .iter()
        .position(|(_, name)| name == attached.as_str());
    let Some(parameter_index) = parameter_index else {
        return Some(attached.clone());
    };
    let binding = candidate.type_bindings[parameter_index]?;
    match program.type_reference_table.type_reference(binding) {
        TypeReferenceNode::Named { name, .. } => Some(name.clone()),
        TypeReferenceNode::Generic { base_name, .. } => Some(base_name.clone()),
        _ => Some(attached.clone()),
    }
}

fn resolve_specialized_receiver_calls(
    program: &mut TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) {
    let states = program.machine_states(machine).to_vec();
    let mut statement_updates = Vec::new();
    let mut expression_updates = Vec::new();
    for state in &states {
        let mut expression_handles = Vec::new();
        for (index, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            collect_statement_expression_trees(program, statement, &mut expression_handles);
            let StatementNode::Call(call) = statement else {
                continue;
            };
            let target = crate::lookup::resolve_state_call_target(
                program,
                machine,
                state,
                call.receiver_symbol,
                call.target_symbol,
                crate::lookup::statement_call_receiver_members(program, call),
                &call.target,
            );
            if target.is_valid() {
                statement_updates.push((state.statement_nodes, index, target));
            }
        }
        for handle in expression_handles {
            let ExpressionNode::Call(call) = program.expression_table.expression(handle) else {
                continue;
            };
            let (receiver_symbol, receiver_path) =
                crate::lookup::call_receiver_parts(program, call.receiver);
            let target = crate::lookup::resolve_state_call_target(
                program,
                machine,
                state,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            );
            if target.is_valid() {
                expression_updates.push((handle, target));
            }
        }
    }
    for (span, index, target) in statement_updates {
        let StatementNode::Call(call) = &mut program.statement_table.statements_mut(span)[index]
        else {
            continue;
        };
        call.target_symbol = target;
    }

    expression_updates.sort_unstable_by_key(|(handle, _)| handle.arena_index());
    expression_updates.dedup_by_key(|(handle, _)| handle.arena_index());
    for (handle, target) in expression_updates {
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            continue;
        };
        call.target_symbol = target;
    }
}

/// MP5's pre-specialization template identity. The in-place specialization
/// pass necessarily consumes generic declarations, so the universal contract
/// must be captured before substitution. This encoding is binder-positional:
/// renaming a type, machine, or value parameter does not change the identity.
fn template_contract_fingerprint(program: &TypedTrees, machine_index: usize) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let machine = &program.machines()[machine_index];
    let parameters = program.machine_type_parameters(machine);
    let binders: Vec<(String, String)> = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let prefix = match parameter.kind {
                TypeParameterKind::Type => "T",
                TypeParameterKind::Const { .. } => "C",
                TypeParameterKind::Machine { .. } => "M",
                TypeParameterKind::Proposition { .. } => "P",
            };
            (
                parameter.name.as_str().to_owned(),
                format!("${prefix}{index}"),
            )
        })
        .collect();
    let type_binders: Vec<(SymbolHandle, String)> = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let prefix = match parameter.kind {
                TypeParameterKind::Type => "T",
                TypeParameterKind::Const { .. } => "C",
                TypeParameterKind::Machine { .. } => "M",
                TypeParameterKind::Proposition { .. } => "P",
            };
            (parameter.symbol, format!("${prefix}{index}"))
        })
        .collect();
    let mut bytes = Vec::new();
    bytes.extend(machine.name.as_str().as_bytes());
    bytes.push(0xff);
    bytes.push(match machine.supply_mode {
        psi_language_semantics::MachineSupplyMode::CheckedBody => 1,
        psi_language_semantics::MachineSupplyMode::Requirement => 2,
        psi_language_semantics::MachineSupplyMode::Boundary => 3,
        psi_language_semantics::MachineSupplyMode::Accepted => 4,
        psi_language_semantics::MachineSupplyMode::ExternalRealization { .. } => 5,
    });
    for (index, parameter) in parameters.iter().enumerate() {
        bytes.push(match parameter.kind {
            TypeParameterKind::Type => 1,
            TypeParameterKind::Const { .. } => 2,
            TypeParameterKind::Machine { .. } => 3,
            TypeParameterKind::Proposition { .. } => 4,
        });
        bytes.extend((index as u32).to_le_bytes());
        encode_data_properties(parameter.bounds, &mut bytes);
        match &parameter.kind {
            TypeParameterKind::Const { type_reference } => encode_normalized_text(
                program
                    .normalized_type_identity_with_binders(*type_reference, &type_binders)
                    .as_str(),
                &binders,
                &mut bytes,
            ),
            TypeParameterKind::Machine { contract } => match program
                .machine_parameter_contract_view(contract)
                .expect("typed machine-parameter contract must retain a valid requirement identity")
            {
                psi_typed_trees::data::MachineParameterContractView::Structural(signature) => {
                    bytes.push(1);
                    encode_state_signature(program, signature, &binders, &type_binders, &mut bytes);
                }
                psi_typed_trees::data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement,
                } => {
                    bytes.push(2);
                    let identity = program
                        .normalized_trait_requirement_overload_identity(
                            trait_definition,
                            requirement,
                        )
                        .identity();
                    bytes.extend((identity.len() as u64).to_le_bytes());
                    bytes.extend(identity.as_bytes());
                    encode_state_signature(
                        program,
                        requirement,
                        &binders,
                        &type_binders,
                        &mut bytes,
                    );
                }
            },
            TypeParameterKind::Proposition { contract } => {
                bytes.extend((contract.parameters.len() as u32).to_le_bytes());
                for parameter in program.state_parameters.span_or_empty(contract.parameters) {
                    bytes.push(u8::from(parameter.is_mutable));
                    bytes.push(u8::from(parameter.is_self));
                    encode_normalized_text(
                        program
                            .normalized_type_identity_with_binders(
                                parameter.type_reference,
                                &type_binders,
                            )
                            .as_str(),
                        &binders,
                        &mut bytes,
                    );
                }
            }
            TypeParameterKind::Type => {}
        }
        bytes.push(0xfe);
    }
    let mut conformance_bounds = Vec::new();
    for bound in &machine.conformance_bounds {
        let mut encoded = Vec::new();
        let subject_index = parameters
            .iter()
            .position(|parameter| parameter.symbol == bound.subject)
            .unwrap_or(usize::MAX);
        encoded.extend((subject_index as u64).to_le_bytes());
        if bound.binder.is_some() {
            encoded.push(3);
            encoded.extend(bound.carrier_name.as_str().as_bytes());
        } else if bound.conformance.is_some() {
            encoded.push(2);
            encoded.extend(bound.carrier_name.as_str().as_bytes());
            encoded.push(0);
            if let Some(name) = &bound.conformance_name {
                encoded.extend(name.as_str().as_bytes());
            }
        } else {
            encoded.push(1);
            encoded.extend(bound.carrier_name.as_str().as_bytes());
        }
        encoded.push(0);
        for argument in &bound.arguments {
            encode_normalized_text(
                program
                    .normalized_type_identity_with_binders(*argument, &type_binders)
                    .as_str(),
                &binders,
                &mut encoded,
            );
            encoded.push(0);
        }
        conformance_bounds.push(encoded);
    }
    conformance_bounds.sort();
    for bound in conformance_bounds {
        bytes.extend(bound);
        bytes.push(0xfb);
    }
    let mut state_shapes = Vec::new();
    for state in program.machine_states(machine) {
        let mut shape = Vec::new();
        encode_state_shape(program, state, &binders, &type_binders, &mut shape);
        state_shapes.push(shape);
    }
    state_shapes.sort();
    for shape in state_shapes {
        bytes.extend(shape);
        bytes.push(0xfd);
    }
    let mut service_reaches: Vec<_> = program
        .service_reach_rows
        .services(machine.service_reach_row)
        .iter()
        .map(|service| {
            program
                .service_reaches
                .definition(*service)
                .expect("normalized service row references a registered service")
        })
        .map(|service| service.name.as_str())
        .collect();
    service_reaches.sort_unstable();
    service_reaches.dedup();
    for service in service_reaches {
        bytes.extend(service.as_bytes());
        bytes.push(0);
    }
    bytes.push(u8::from(machine.suspends));
    bytes.push(u8::from(machine.blocks));
    let mut contract_binders = binders.clone();
    if let Some(state) = program.machine_states(machine).first() {
        contract_binders.extend(
            program
                .state_parameters(state)
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    (parameter.name.as_str().to_owned(), format!("$P{index}"))
                }),
        );
    }
    let contracts = encode_contract_set(
        program,
        program.machine_contracts(machine),
        &contract_binders,
    );
    for contract in contracts {
        bytes.extend(contract);
        bytes.push(0xfc);
    }
    match &machine.termination_plan.interface {
        psi_language_semantics::TerminationInterface::InternalDerived => bytes.push(0),
        psi_language_semantics::TerminationInterface::Published(
            psi_language_semantics::TerminationGuarantee::NoGuarantee,
        ) => bytes.push(1),
        psi_language_semantics::TerminationInterface::Published(
            psi_language_semantics::TerminationGuarantee::Terminates { premises },
        ) => {
            bytes.push(2);
            let parameter_symbols = program
                .machine_states(machine)
                .first()
                .map(|state| {
                    program
                        .state_parameters(state)
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            encode_progress_premises(premises, &parameter_symbols, &mut bytes);
        }
    }
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Deterministic identity of an authored generic machine declaration before
/// monomorphization consumes its binders. Trust receipts and separate-
/// compilation caches use this same identity; callers cannot substitute the
/// identity of one concrete instance for the universal template grant.
pub fn generic_machine_template_fingerprint(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
) -> Option<u64> {
    let machine_index = program
        .machines()
        .iter()
        .position(|machine| machine.symbol == machine_symbol)?;
    (!program
        .machine_type_parameters(&program.machines()[machine_index])
        .is_empty())
    .then(|| template_contract_fingerprint(program, machine_index))
}

fn accepted_template_commitment(program: &TypedTrees, machine_index: usize) -> Option<String> {
    let machine = &program.machines()[machine_index];
    (machine.supply_mode == psi_language_semantics::MachineSupplyMode::Accepted)
        .then(|| machine.name.as_str().to_owned())
}

pub(crate) fn bind_specialization_contract_identities(
    program: &mut TypedTrees,
    contracts: &psi_checked_trees::MachineContractPlans,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let updates: Vec<(Vec<u64>, Vec<u64>)> = program
        .machine_specializations
        .iter()
        .map(|specialization| {
            let template = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template);
            match (template.map(|machine| machine.supply_mode), &specialization.accepted_template_commitment) {
                (Some(psi_language_semantics::MachineSupplyMode::Accepted), None) => {
                    diagnostics.push(Diagnostic::error(
                        "accepted generic specialization lost its template trust commitment",
                    ));
                }
                (Some(psi_language_semantics::MachineSupplyMode::Accepted), Some(commitment))
                    if template.is_some_and(|machine| machine.name.as_str() != commitment) =>
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "accepted generic specialization records template commitment `{commitment}`, but its template identity no longer matches"
                    )));
                }
                (Some(psi_language_semantics::MachineSupplyMode::Accepted), Some(_)) => {}
                (Some(mode), Some(commitment)) => diagnostics.push(Diagnostic::error(format!(
                    "generic specialization records accepted template commitment `{commitment}`, but the retained template supply mode is {mode:?}"
                ))),
                _ => {}
            }
            if specialization.template_contract_fingerprint == 0 {
                diagnostics.push(Diagnostic::error(
                    "generic specialization is missing its pre-substitution template contract identity",
                ));
            }

            let machine_identities = specialization
                .machine_arguments
                .iter()
                .filter_map(|state_symbol| {
                    let owner = program.machines().iter().find(|machine| {
                        program
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == *state_symbol)
                    });
                    let Some(owner) = owner else {
                        diagnostics.push(Diagnostic::error(format!(
                            "generic specialization references static machine symbol {:?}, but no owning machine exists",
                            state_symbol
                        )));
                        return None;
                    };
                    let Some(contract) = contracts.for_machine(owner.symbol) else {
                        diagnostics.push(Diagnostic::error(format!(
                            "generic specialization selected `{}`, but its normalized machine contract identity is missing",
                            owner.name
                        )));
                        return None;
                    };
                    Some(contract.fingerprint)
                })
                .collect();
            let conformance_identities = specialization
                .conformance_applications
                .iter()
                .filter_map(|application| {
                    let Some(conformance) = program
                        .conformances()
                        .iter()
                        .find(|conformance| conformance.symbol == application.declaration)
                    else {
                        diagnostics.push(Diagnostic::error(format!(
                            "generic specialization references conformance symbol {:?}, but no package conformance exists",
                            application.declaration
                        )));
                        return None;
                    };
                    if program.closed_conformance_rows(conformance).is_none() {
                        diagnostics.push(Diagnostic::error(format!(
                            "generic specialization selected `{}`, but it is not a closed conformance map",
                            conformance
                                .alias
                                .as_ref()
                                .map(|name| name.as_str())
                                .unwrap_or("<unnamed-conformance>")
                        )));
                        return None;
                    }
                    Some(application.fingerprint)
                })
                .collect();
            (machine_identities, conformance_identities)
        })
        .collect();

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for (specialization, (machine_identities, conformance_identities)) in
        program.machine_specializations.iter_mut().zip(updates)
    {
        if !specialization
            .machine_argument_contract_fingerprints
            .is_empty()
            && specialization.machine_argument_contract_fingerprints != machine_identities
        {
            return Err(vec![Diagnostic::error(
                "generic specialization cache entry no longer matches its selected machine contract identities",
            )]);
        }
        if !specialization.conformance_argument_fingerprints.is_empty()
            && specialization.conformance_argument_fingerprints != conformance_identities
        {
            return Err(vec![Diagnostic::error(
                "generic specialization cache entry no longer matches its selected conformance-map identities",
            )]);
        }
        specialization.machine_argument_contract_fingerprints = machine_identities;
        specialization.conformance_argument_fingerprints = conformance_identities;
        specialization.fingerprint = specialization_contract_fingerprint(
            specialization.fingerprint,
            specialization.template_contract_fingerprint,
            &specialization.machine_argument_contract_fingerprints,
            &specialization.conformance_argument_fingerprints,
        );
    }
    Ok(())
}

fn specialization_contract_fingerprint(
    selection_fingerprint: u64,
    template_contract_fingerprint: u64,
    selected_contracts: &[u64],
    selected_conformances: &[u64],
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    let mut bytes = Vec::new();
    bytes.extend(selection_fingerprint.to_le_bytes());
    bytes.extend(template_contract_fingerprint.to_le_bytes());
    bytes.push(0xf1);
    bytes.extend((selected_contracts.len() as u64).to_le_bytes());
    for identity in selected_contracts {
        bytes.extend(identity.to_le_bytes());
    }
    bytes.push(0xf2);
    bytes.extend((selected_conformances.len() as u64).to_le_bytes());
    for identity in selected_conformances {
        bytes.extend(identity.to_le_bytes());
    }
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn encode_data_properties(properties: psi_typed_trees::data::DataProperties, output: &mut Vec<u8>) {
    output.push(match properties.multiplicity {
        psi_language_semantics::Multiplicity::Unrestricted => 1,
        psi_language_semantics::Multiplicity::Affine => 2,
        psi_language_semantics::Multiplicity::Linear => 3,
    });
    if let Some(carry) = properties.carry {
        output.extend(format!("{carry}").as_bytes());
    }
    output.push(0);
}

fn encode_state_signature(
    program: &TypedTrees,
    signature: &psi_typed_trees::signature::StateSignature,
    binders: &[(String, String)],
    type_binders: &[(SymbolHandle, String)],
    output: &mut Vec<u8>,
) {
    for parameter in program.state_signature_parameters(signature) {
        encode_parameter(program, parameter, binders, type_binders, output);
    }
    encode_normalized_text(
        program
            .normalized_type_identity_with_binders(signature.return_type, type_binders)
            .as_str(),
        binders,
        output,
    );
    for service in program
        .service_reach_rows
        .services(signature.service_reach_row)
    {
        let service = program
            .service_reaches
            .definition(*service)
            .expect("normalized signature service row references a registered service");
        output.extend(service.name.as_bytes());
        output.push(0);
    }
    output.push(u8::from(signature.suspends));
    output.push(u8::from(signature.blocks));
    let mut contract_binders = binders.to_vec();
    contract_binders.extend(
        program
            .state_signature_parameters(signature)
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.name.as_str().to_owned(), format!("$P{index}"))),
    );
    let contracts = encode_contract_set(
        program,
        program.state_signature_contracts(signature),
        &contract_binders,
    );
    for contract in contracts {
        output.extend(contract);
        output.push(0xfc);
    }
    match &signature.termination_guarantee {
        psi_language_semantics::TerminationGuarantee::NoGuarantee => output.push(0),
        psi_language_semantics::TerminationGuarantee::Terminates { premises } => {
            output.push(1);
            let parameter_symbols = program
                .state_signature_parameters(signature)
                .iter()
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>();
            encode_progress_premises(premises, &parameter_symbols, output);
        }
    }
}

fn encode_progress_premises(
    premises: &[psi_language_semantics::ProgressPremise],
    parameter_symbols: &[psi_symbols::SymbolHandle],
    output: &mut Vec<u8>,
) {
    let mut encoded = premises
        .iter()
        .map(|premise| {
            let mut bytes = Vec::new();
            bytes.extend(premise.profile.0.to_le_bytes());
            if let Some(index) = parameter_symbols
                .iter()
                .position(|symbol| *symbol == premise.subject.root)
            {
                bytes.push(0);
                bytes.extend(index.to_le_bytes());
            } else {
                bytes.push(1);
                bytes.extend(premise.subject.root.arena_index().to_le_bytes());
            }
            for projection in &premise.subject.projections {
                bytes.extend(projection.arena_index().to_le_bytes());
            }
            bytes
        })
        .collect::<Vec<_>>();
    encoded.sort();
    for premise in encoded {
        output.extend(premise);
        output.push(0xfa);
    }
}

pub(crate) fn canonical_state_signature_fingerprint(
    program: &TypedTrees,
    signature: &psi_typed_trees::signature::StateSignature,
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut bytes = Vec::new();
    encode_state_signature(program, signature, &[], &[], &mut bytes);
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn encode_state_shape(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    binders: &[(String, String)],
    type_binders: &[(SymbolHandle, String)],
    output: &mut Vec<u8>,
) {
    for parameter in program.state_parameters(state) {
        encode_parameter(program, parameter, binders, type_binders, output);
    }
    encode_normalized_text(
        program
            .normalized_type_identity_with_binders(state.return_type, type_binders)
            .as_str(),
        binders,
        output,
    );
    let mut contract_binders = binders.to_vec();
    contract_binders.extend(
        program
            .state_parameters(state)
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.name.as_str().to_owned(), format!("$P{index}"))),
    );
    let contracts = encode_contract_set(program, program.state_contracts(state), &contract_binders);
    for contract in contracts {
        output.extend(contract);
        output.push(0xfc);
    }
}

fn encode_parameter(
    program: &TypedTrees,
    parameter: &psi_typed_trees::signature::StateParameter,
    binders: &[(String, String)],
    type_binders: &[(SymbolHandle, String)],
    output: &mut Vec<u8>,
) {
    output.push(u8::from(parameter.is_self));
    output.push(u8::from(parameter.is_mutable));
    output.push(u8::from(parameter.is_const));
    encode_normalized_text(
        program
            .normalized_type_identity_with_binders(parameter.type_reference, type_binders)
            .as_str(),
        binders,
        output,
    );
}

fn encode_contract(
    program: &TypedTrees,
    contract: &psi_typed_trees::signature::SignatureContract,
    binders: &[(String, String)],
    output: &mut Vec<u8>,
) {
    encode_contract_kind(&contract.kind, binders, output);
    let mut facts: Vec<String> = program
        .proof_facts
        .span_or_empty(contract.facts)
        .iter()
        .map(|fact| contract_fact_text(program, fact))
        .collect();
    facts.sort();
    for fact in facts {
        encode_normalized_text(&fact, binders, output);
    }
}

fn encode_contract_kind(
    kind: &psi_typed_trees::signature::SignatureContractKind,
    _binders: &[(String, String)],
    output: &mut Vec<u8>,
) {
    output.push(match kind {
        psi_typed_trees::signature::SignatureContractKind::Requires => 1,
        psi_typed_trees::signature::SignatureContractKind::Ensures => 2,
        psi_typed_trees::signature::SignatureContractKind::Boundary => 3,
        psi_typed_trees::signature::SignatureContractKind::Crashes { .. } => 4,
    });
    if let psi_typed_trees::signature::SignatureContractKind::Crashes { cause } = kind {
        output.push(match cause {
            psi_typed_trees::signature::CrashCause::Trap => 1,
            psi_typed_trees::signature::CrashCause::Abort => 2,
        });
    }
}

fn contract_fact_text(program: &TypedTrees, fact: &psi_typed_trees::domain::ProofFact) -> String {
    match fact {
        psi_typed_trees::domain::ProofFact::Expression(expression) => {
            program.expression_table.display_name(*expression)
        }
        psi_typed_trees::domain::ProofFact::Membership(membership) => format!(
            "{} in {}",
            program.expression_table.display_name(membership.value),
            program
                .domain_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        ),
        psi_typed_trees::domain::ProofFact::Proposition(application) => format!(
            "{}({})",
            application.name.as_str(),
            program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .map(|argument| program.expression_table.display_name(*argument))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Template and specialization identities use the same crash-bucket algebra
/// as public contract plans: route clauses merge by cause, routes form a set,
/// and an unconditional route subsumes guarded alternatives.
fn encode_contract_set(
    program: &TypedTrees,
    contracts: &[psi_typed_trees::signature::SignatureContract],
    binders: &[(String, String)],
) -> Vec<Vec<u8>> {
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct CrashBucket {
        unconditional: bool,
        routes: Vec<Vec<u8>>,
    }

    let mut encoded = Vec::new();
    let mut crash_buckets = BTreeMap::<Vec<u8>, CrashBucket>::new();
    for contract in contracts {
        if matches!(
            contract.kind,
            psi_typed_trees::signature::SignatureContractKind::Crashes { .. }
        ) {
            let mut header = Vec::new();
            encode_contract_kind(&contract.kind, binders, &mut header);
            let bucket = crash_buckets.entry(header).or_default();
            let facts = program.proof_facts.span_or_empty(contract.facts);
            if facts.is_empty()
                || facts.iter().any(|fact| {
                    matches!(
                        fact,
                        psi_typed_trees::domain::ProofFact::Expression(expression)
                            if matches!(
                                program.expression_table.expression(*expression),
                                psi_typed_trees::expression::ExpressionNode::Boolean(true)
                            )
                    )
                })
            {
                bucket.unconditional = true;
            } else {
                for fact in facts {
                    let mut route = Vec::new();
                    encode_normalized_text(&contract_fact_text(program, fact), binders, &mut route);
                    bucket.routes.push(route);
                }
            }
            continue;
        }

        let mut contract_bytes = Vec::new();
        encode_contract(program, contract, binders, &mut contract_bytes);
        encoded.push(contract_bytes);
    }

    for (header, mut bucket) in crash_buckets {
        if bucket.unconditional {
            let mut contract = header;
            contract.push(0);
            encoded.push(contract);
            continue;
        }
        bucket.routes.sort();
        bucket.routes.dedup();
        for route in bucket.routes {
            let mut contract = header.clone();
            contract.push(1);
            contract.extend(route);
            encoded.push(contract);
        }
    }
    encoded.sort();
    encoded
}

fn encode_normalized_text(text: &str, binders: &[(String, String)], output: &mut Vec<u8>) {
    let mut word = String::new();
    let flush = |word: &mut String, output: &mut Vec<u8>| {
        if word.is_empty() {
            return;
        }
        if let Some((_, replacement)) = binders.iter().find(|(name, _)| name == word) {
            output.extend(replacement.as_bytes());
        } else {
            output.extend(word.as_bytes());
        }
        word.clear();
    };
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            word.push(character);
        } else {
            flush(&mut word, output);
            output.extend(character.to_string().as_bytes());
        }
    }
    flush(&mut word, output);
    output.push(0);
}

fn specialization_fingerprint(
    template: &str,
    type_arguments: &[String],
    const_arguments: &[String],
    machine_arguments: &[String],
    evidence_arguments: &[String],
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for part in std::iter::once(template)
        .chain(type_arguments.iter().map(String::as_str))
        .chain(const_arguments.iter().map(String::as_str))
        .chain(machine_arguments.iter().map(String::as_str))
        .chain(evidence_arguments.iter().map(String::as_str))
    {
        for byte in part.as_bytes().iter().copied().chain(std::iter::once(0xff)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    hash
}
