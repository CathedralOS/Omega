//! Indexed-instance collection: the compatibility key and state-call
//! index, typed-indexed instance discovery, and ensured-instance plumbing.

use super::labels::{domain_label, qualification_label};
use arena::HandleSpan;
use checked_trees::{
    CheckedOperatorFacts, ContractProofFactKind, FlowFacts, FlowSemanticContextRef, ProofFacts,
};
use facts::ProgramPoint;
use language_semantics::SemanticDomainId;
use symbols::JoinedPath;
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[derive(Debug, Clone)]
pub(super) struct IndexedInstance {
    pub(super) family: SymbolHandle,
    pub(super) semantic_id: SemanticDomainId,
    pub(super) arguments: Vec<TypeReferenceHandle>,
    pub(super) label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CompatibilityKey {
    pub(super) point: ProgramPoint,
    pub(super) value: ExpressionHandle,
    pub(super) target_type: TypeReferenceHandle,
    pub(super) family: SymbolHandle,
    pub(super) actual: SemanticDomainId,
    pub(super) expected: SemanticDomainId,
}

pub(super) struct ResolvedStateCall<'program, 'flow> {
    pub(super) fact: &'flow checked_trees::FlowCallFact,
    pub(super) site: validation::CallSite<'program>,
}

impl ResolvedStateCall<'_, '_> {
    pub(super) fn parameters<'program>(
        &self,
        program: &'program dyn ::validation::CheckingInput,
        lexical_owner: SymbolHandle,
    ) -> Option<&'program [typed_trees::signature::StateParameter]> {
        // An applied requirement's prepared signature already substitutes its
        // owning trait telescope, including inherited scopes. Reading the bare
        // target instead loses those bindings; reconstructing them here would
        // create another, incomplete application-normalization path.
        if let validation::CallSite::Expression { call, .. } = &self.site
            && call.requirement_application.is_valid()
        {
            return program
                .requirement_call_signature(lexical_owner, call)
                .map(|signature| program.state_signature_parameters(signature));
        }
        validation::call_target_parameters(program, self.fact.target_symbol)
    }
}

pub(super) struct StateCallIndex<'program, 'flow> {
    pub(super) calls: Vec<ResolvedStateCall<'program, 'flow>>,
}

impl<'program, 'flow> StateCallIndex<'program, 'flow> {
    pub(super) fn new(
        program: &'program dyn ::validation::CheckingInput,
        flow: &'flow FlowFacts,
        state_flow: &'flow checked_trees::FlowStateFact,
    ) -> Self {
        let calls = flow
            .control
            .calls
            .span_or_empty(state_flow.calls)
            .iter()
            .filter_map(|fact| {
                validation::find_call_site(
                    program,
                    state_flow.machine_symbol,
                    state_flow.state_symbol,
                    fact.statement_index,
                    fact.call_ordinal,
                )
                .map(|site| ResolvedStateCall { fact, site })
            })
            .collect();
        Self { calls }
    }

    pub(super) fn contexts_after_value(
        &self,
        statement_index: usize,
        value: ExpressionHandle,
        fallback: HandleSpan<FlowSemanticContextRef>,
    ) -> HandleSpan<FlowSemanticContextRef> {
        self.calls
            .iter()
            .filter(|call| call.fact.statement_index == statement_index)
            .find_map(|call| match &call.site {
                validation::CallSite::Expression { expression, .. } if *expression == value => {
                    Some(call.fact.exit_semantic_contexts)
                }
                _ => None,
            })
            .unwrap_or(fallback)
    }
}

pub(super) fn construction_field_type(
    program: &dyn ::validation::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    case_name: Option<&str>,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    if let Some(case_name) = case_name
        && let Some(variant) =
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name => {
                        Some(variant)
                    }
                    _ => None,
                })
    {
        for field in program.data_payload_fields(variant) {
            if field.name.as_str() == field_name && field.type_reference.is_valid() {
                return Some(field.type_reference);
            }
        }
    }
    program.data_members(definition).iter().find_map(|member| {
        let field = member.as_field()?;
        (field.name.as_str() == field_name && field.type_reference.is_valid())
            .then_some(field.type_reference)
    })
}

pub(super) fn literal_element_type(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => literal_element_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            literal_element_type(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some(*element_type),
        _ => None,
    }
}

pub(super) fn expression_indexed_instances(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Vec<IndexedInstance> {
    let mut instances = Vec::new();
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            return expression_indexed_instances(
                program,
                operators,
                machine,
                state,
                statement_index,
                inner.target,
            );
        }
        ExpressionNode::Atomic(atomic) => {
            return expression_indexed_instances(
                program,
                operators,
                machine,
                state,
                statement_index,
                atomic.result,
            );
        }
        ExpressionNode::Cast(cast) => {
            collect_type_indexed_instances(
                program,
                cast.target_type,
                &mut instances,
                &mut Vec::new(),
            );
            if cast.semantic_domain_symbol.is_valid() && !cast.semantic_domain_arguments.is_empty()
            {
                instances.push(IndexedInstance {
                    family: cast.semantic_domain_symbol,
                    semantic_id: cast.semantic_domain_id,
                    arguments: program
                        .type_reference_table
                        .type_reference_handles(cast.semantic_domain_arguments)
                        .to_vec(),
                    label: qualification_label(program, cast),
                });
            }
            return instances;
        }
        ExpressionNode::Call(call) => {
            let return_type = if call.requirement_application.is_valid() {
                program
                    .requirement_call_signature(machine.symbol, call)
                    .map(|signature| signature.return_type)
            } else {
                call_return_type(program, call.target_symbol)
            };
            if let Some(return_type) = return_type {
                collect_type_indexed_instances(
                    program,
                    return_type,
                    &mut instances,
                    &mut Vec::new(),
                );
            } else if let Some(operator_use) = operators
                .named_uses()
                .find(|operator_use| operator_use.expression == expression)
                && let Some(operator) =
                    program.operator_definition_by_symbol(operator_use.selected_operator_symbol)
            {
                collect_type_indexed_instances(
                    program,
                    operator.return_type,
                    &mut instances,
                    &mut Vec::new(),
                );
            }
            return instances;
        }
        ExpressionNode::Binary(_) | ExpressionNode::Unary(_) | ExpressionNode::Indexed(_) => {
            if let Some(operator_use) = operators.expression_use(expression)
                && let Some(operator) =
                    program.operator_definition_by_symbol(operator_use.selected_operator_symbol)
            {
                collect_type_indexed_instances(
                    program,
                    operator.return_type,
                    &mut instances,
                    &mut Vec::new(),
                );
                return instances;
            }
        }
        _ => {}
    }
    if let Some(type_reference) = validation::expression_type_reference_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    ) {
        collect_type_indexed_instances(program, type_reference, &mut instances, &mut Vec::new());
    } else if let Some(type_reference) =
        validation::declared_place_type_raw(program, machine, Some(state), expression)
    {
        collect_type_indexed_instances(program, type_reference, &mut instances, &mut Vec::new());
    }
    instances
}

/// The indexed instances a value-position call's own `ensures` declare on
/// its reserved `result`. A declared return type `-> Extent` names no
/// instance, so `expression_indexed_instances` finds none there; the
/// requirement's `ensures result in Granted & Resident<SlotPlacement, Slot>`
/// does, and the typed membership fact attached to this call carries the
/// identity the typer interned on it. A restating `let` (or store, or
/// return) then compares against that instance exactly as it compares
/// against a declared field's, instead of joining the family by symbol and
/// keeping whichever index the restatement chose. The facts are read from
/// the call's contract row, before `call_contract_evidence` decides whether
/// the promise is admitted as establishment: like the declared return type,
/// the declared instance is what the value is, whatever its evidence.
pub(super) fn append_call_result_ensured_instances(
    program: &dyn ::validation::CheckingInput,
    proof: &ProofFacts,
    state_calls: &StateCallIndex<'_, '_>,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
    instances: &mut Vec<IndexedInstance>,
) {
    if !matches!(
        program.expression_table.expression(value),
        ExpressionNode::Call(_)
    ) {
        return;
    }
    let Some(call) = state_calls.calls.iter().find(|call| {
        call.fact.statement_index == statement_index
            && matches!(
                &call.site,
                validation::CallSite::Expression { expression, .. }
                    if *expression == value
            )
    }) else {
        return;
    };
    let Some((_, contract_call)) = proof.contract_calls.iter().find(|(_, contract_call)| {
        contract_call.caller_machine_symbol == machine.symbol
            && contract_call.caller_state_symbol == state.symbol
            && contract_call.statement_index == statement_index
            && contract_call.call_ordinal == call.fact.call_ordinal
    }) else {
        return;
    };
    for reference in proof
        .contract_fact_refs
        .span_or_empty(contract_call.ensures)
    {
        let contract = proof.contract_facts.get(reference.fact);
        if contract.kind != ContractProofFactKind::Ensures {
            continue;
        }
        let mut selected_fact = program.proof_facts.get(contract.fact);
        if let validation::CallSite::Expression {
            call: expression, ..
        } = &call.site
            && let checked_trees::ContractProofFactOwner::StateSignature { state_symbol, .. } =
                contract.owner
            && let Some((_, declared)) = program.trait_machine_signature(state_symbol)
            && let Some(selected) = program.requirement_call_signature(machine.symbol, expression)
        {
            let Some(applied_fact) =
                program.applied_signature_contract_fact(declared, selected, contract.fact)
            else {
                continue;
            };
            selected_fact = applied_fact;
        }
        let Some(membership) = selected_fact.as_membership() else {
            continue;
        };
        // The subject must be the whole reserved result: a projection such
        // as `result.storage` belongs to that field's declared type.
        if !membership.semantic_domain.is_valid()
            || !program
                .expression_table
                .names_reserved_result(membership.value)
        {
            continue;
        }
        let arguments = program
            .type_reference_table
            .type_reference_handles(membership.domain_arguments)
            .to_vec();
        if arguments.is_empty()
            || instances.iter().any(|instance| {
                instance.family == membership.domain_symbol
                    && instance.semantic_id == membership.semantic_domain
            })
        {
            continue;
        }
        let name = program.domain_path_members(membership.domain).joined_path();
        let label = domain_label(program, &name, &arguments);
        instances.push(IndexedInstance {
            family: membership.domain_symbol,
            semantic_id: membership.semantic_domain,
            arguments,
            label,
        });
    }
}
pub(super) fn call_return_type(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if let Some(state) = program.state_by_symbol(state_symbol) {
        return Some(state.return_type);
    }
    if let Some((_, signature)) = program.machine_parameter_signature(state_symbol) {
        return Some(signature.return_type);
    }
    program
        .trait_machine_signature(state_symbol)
        .map(|(_, signature)| signature.return_type)
}

pub(super) fn collect_type_indexed_instances(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    instances: &mut Vec<IndexedInstance>,
    visited: &mut Vec<TypeReferenceHandle>,
) {
    if !type_reference.is_valid() || visited.contains(&type_reference) {
        return;
    }
    visited.push(type_reference);
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_indexed_instances(program, *referee, instances, visited)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_indexed_instances(program, *base_type, instances, visited);
            for constraint in program.type_reference_table.constraints(*constraints) {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    continue;
                };
                if domain.symbol.is_valid() && !domain.arguments.is_empty() {
                    instances.push(instance_from_constraint(program, domain));
                }
            }
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => {}
    }
}

pub(super) fn instance_from_constraint(
    program: &dyn ::validation::CheckingInput,
    domain: &DomainConstraint,
) -> IndexedInstance {
    IndexedInstance {
        family: domain.symbol,
        semantic_id: domain.semantic_id,
        arguments: domain.arguments.clone(),
        label: domain_label(program, domain.name.as_str(), &domain.arguments),
    }
}

pub(super) fn selected_operation_count(
    program: &dyn ::validation::CheckingInput,
    arguments: impl Iterator<Item = TypeReferenceHandle>,
) -> usize {
    let mut expressions = Vec::new();
    for argument in arguments {
        let TypeReferenceNode::ConstExpression(expression) =
            program.type_reference_table.type_reference(argument)
        else {
            continue;
        };
        if !expressions.contains(expression) {
            expressions.push(*expression);
        }
    }
    expressions
        .into_iter()
        .map(|expression| {
            program
                .expression_table
                .open_index_operations(expression)
                .count()
        })
        .sum()
}
