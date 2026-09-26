//! Exact machine dependency extraction and conservative interface retention.

use std::collections::{HashMap, HashSet};

use checked_trees::CheckFacts;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    domain::ProofFact,
    expression::{
        ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
        TableMatchArm, TableNamePath,
    },
    machine::TraitConformance,
    proposition::{PropositionApplication, PropositionBinderArgumentKind, PropositionFormula},
    signature::{AuthoredInvocationTarget, SignatureContract},
    statement::{StatementHandle, StatementNode, TransitionGuardNode, TransitionTargetNode},
    trait_definition::ConformanceImplementation,
    types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode},
};

/// Exact declaration indices used to resolve machine references and to key
/// machine-owner lookups without scanning the declaration table.
pub(super) struct MachineIndex {
    /// Membership of every declared machine symbol; order comes from the program.
    pub(super) machine_symbols: HashSet<SymbolHandle>,
    /// Owning machine for every machine state symbol.
    pub(super) state_machine: HashMap<SymbolHandle, SymbolHandle>,
    /// Owning machine for every statement handle inside a machine state.
    pub(super) statement_machine: HashMap<StatementHandle, SymbolHandle>,
}

impl MachineIndex {
    pub(super) fn build(program: &TypedTrees) -> Self {
        let mut machine_symbols = HashSet::new();
        let mut state_machine = HashMap::new();
        let mut statement_machine = HashMap::new();
        for machine in program.machines() {
            machine_symbols.insert(machine.symbol);
            for state in program.machine_states(machine) {
                state_machine.insert(state.symbol, machine.symbol);
                for offset in 0..state.statement_nodes.count() {
                    statement_machine.insert(
                        StatementHandle::from_parts(
                            state.statement_nodes.start().arena_index() + offset,
                            state.statement_nodes.start().generation(),
                        ),
                        machine.symbol,
                    );
                }
            }
        }
        Self {
            machine_symbols,
            state_machine,
            statement_machine,
        }
    }

    /// Resolve any symbol that names a machine or a state owned by a machine
    /// to the machine's declaration symbol. Anything else (data, traits,
    /// parameters, intrinsics, evidence terms) is not a machine edge.
    pub(super) fn machine_of(&self, symbol: SymbolHandle) -> Option<SymbolHandle> {
        if !symbol.is_valid() {
            return None;
        }
        if self.machine_symbols.contains(&symbol) {
            return Some(symbol);
        }
        self.state_machine.get(&symbol).copied()
    }
}

/// Machines that are always retained independent of reachability: every
/// non-`CheckedBody` supply mode plus the provider/authority surface whose
/// machine references no root can bound.
pub(super) fn interface_retention_set(
    program: &TypedTrees,
    facts: &CheckFacts,
    index: &MachineIndex,
) -> HashSet<SymbolHandle> {
    let mut retained = HashSet::new();

    // Requirement, boundary, admission-claim, and external-realization
    // machines are product interface surface consumed by Omega provider
    // selection and admission; they are never pruning candidates.
    for machine in program.machines() {
        if !machine.supply_mode.is_checked_body() {
            retained.insert(machine.symbol);
        }
    }

    // Closed conformances name the exact realization machine Omega may
    // select for any retained dispatch surface. Because provider selection
    // happens outside Psi, every realization named by a retained conformance
    // row is interface surface.
    for conformance in program.conformances() {
        retain_symbol(&mut retained, index, conformance.carrier_symbol);
        if let ConformanceImplementation::Closed { rows } = &conformance.implementation {
            for row in rows {
                retain_symbol(&mut retained, index, row.realization_machine);
                retain_symbol(&mut retained, index, row.realization_state);
            }
        }
    }

    // Boundary calling plans bind an exact requirement machine to a validated
    // policy; their requirement surface is product interface.
    for plan in &program.boundary_calling_plans {
        retain_symbol(&mut retained, index, plan.requirement_machine);
        retain_symbol(&mut retained, index, plan.boundary_trait);
    }

    // Selected nominal machine uses (registration/callback selections) are
    // Omega-bound callables; the selected machine is always product surface.
    for nominal_use in &facts.nominal_machine_uses.uses {
        retain_symbol(&mut retained, index, nominal_use.selected_machine);
        retain_symbol(&mut retained, index, nominal_use.selected_entry);
        retain_symbol(&mut retained, index, nominal_use.registration_operation);
        retain_symbol(&mut retained, index, nominal_use.satisfaction_requirement);
        retain_symbol(&mut retained, index, nominal_use.satisfaction_trait);
    }

    // Contract evidence calls may be owned by trait/operator machinery whose
    // contract is always retained, so every evidence target is retained.
    for call in &facts.proof.contract_expression_evidence_calls {
        retained.insert(call.target_machine_symbol);
        retain_symbol(&mut retained, index, call.target_state_symbol);
    }
    for application in &facts
        .proof
        .contract_expression_static_conformance_applications
    {
        retain_machine_argument(index, &mut retained, &application.application);
    }

    // Denotational call projections do not carry an owner key; their targets
    // are conservative interface surface.
    for projection in &facts.fact_call_projections {
        retained.insert(projection.target_machine);
        retain_symbol(&mut retained, index, projection.target_state);
        for argument in projection.machine_arguments.iter() {
            retain_static_machine_argument(&mut retained, index, argument);
        }
    }

    // Boundary adapter dispatch selects an exact realization state for each
    // routed boundary requirement.
    for dispatch in &facts.boundary_adapter_dispatch {
        retain_symbol(&mut retained, index, dispatch.realization_state);
        retain_symbol(&mut retained, index, dispatch.requirement);
        retain_symbol(&mut retained, index, dispatch.receiver);
    }

    // Placed-view plans retain the exact `Policy::plan` machine whose
    // evaluated output produced the placement.
    for plan in &program.placed_view_plans {
        retain_symbol(&mut retained, index, plan.policy_plan_machine_symbol);
    }
    for input in &facts.placed_view_inputs {
        retain_symbol(&mut retained, index, input.policy_plan_machine);
    }

    // Compiler-owned service-erasure authorizations and open-index operation
    // selections name exact requirements/providers that must survive pruning.
    for erasure in &program.fused_service_erasures {
        retain_symbol(&mut retained, index, erasure.requirement);
    }
    for normalization in &program.open_index_normalizations {
        for operation in &normalization.operations {
            retain_symbol(&mut retained, index, operation.provider);
            retain_symbol(&mut retained, index, operation.operator);
        }
    }

    // Every machine referenced by a retained non-machine declaration stays
    // reachable: declarations other than machines are not pruned, so any
    // machine they name would otherwise become a stale reference.
    retain_declaration_surface_machines(program, index, &mut retained);

    retained.retain(|symbol| index.machine_symbols.contains(symbol));
    retained
}

/// Retain a symbol when it resolves to a machine declaration.
fn retain_symbol(
    retained: &mut impl Extend<SymbolHandle>,
    index: &MachineIndex,
    symbol: SymbolHandle,
) {
    if let Some(machine) = index.machine_of(symbol) {
        retained.extend([machine]);
    }
}

/// Retain every machine referenced by a `StaticMachineArgument`: its selected
/// entry-state symbol plus any nested application arguments.
fn retain_static_machine_argument(
    retained: &mut impl Extend<SymbolHandle>,
    index: &MachineIndex,
    argument: &StaticMachineArgument,
) {
    retain_symbol(retained, index, argument.symbol);
    if let Some(application) = &argument.application {
        for nested in application.arguments.iter() {
            retain_static_machine_argument(retained, index, nested);
        }
    }
}

/// Retain every machine named inside one closed conformance application:
/// machine arguments, static argument applications, and row realizations.
fn retain_machine_argument(
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
) {
    for machine in application.machine_arguments.iter() {
        retain_symbol(retained, index, *machine);
    }
    for argument in application.arguments.iter() {
        retain_static_machine_argument(retained, index, argument);
    }
    for row in &application.rows {
        retain_symbol(retained, index, row.realization_machine);
        retain_symbol(retained, index, row.realization_state);
    }
}

/// Machines referenced by retained non-machine declarations: data fields and
/// `where` facts, trait requirement signatures, conformance carriers,
/// proposition binders/bodies, measure bodies, const initializers, operator
/// signatures, domain facts, and wire field types.
fn retain_declaration_surface_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
) {
    for data in program.data_definitions() {
        retain_symbol(retained, index, data.symbol);
        if let Some(instance) = data.generic_instance {
            retain_type_machines(program, index, retained, instance);
        }
        if let Some(quotient) = &data.quotient {
            retain_type_machines(program, index, retained, quotient.carrier);
            retain_symbol(retained, index, quotient.relation_symbol);
        }
        for fact in program.tables.proof_facts.span_or_empty(data.where_facts) {
            retain_proof_fact_machines(program, index, retained, fact);
        }
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field) => {
                    retain_type_machines(program, index, retained, field.type_reference);
                }
                typed_trees::data::DataMember::Variant(variant) => {
                    for field in program.data_payload_fields(variant) {
                        retain_type_machines(program, index, retained, field.type_reference);
                    }
                    for fact in program
                        .tables
                        .proof_facts
                        .span_or_empty(variant.where_facts)
                    {
                        retain_proof_fact_machines(program, index, retained, fact);
                    }
                }
            }
        }
    }

    for definition in program.traits() {
        retain_symbol(retained, index, definition.symbol);
        for bound in &definition.conformance_bounds {
            if let Some(selected) = &bound.selected_conformance {
                retain_static_machine_argument(retained, index, selected);
            }
        }
        for signature in program.trait_machine_signatures(definition) {
            retain_signature_machines(program, index, retained, signature);
        }
    }

    for conformance in program.conformances() {
        retain_symbol(retained, index, conformance.symbol);
        retain_symbol(retained, index, conformance.trait_symbol);
        for argument in program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
        {
            retain_type_machines(program, index, retained, *argument);
        }
    }

    for proposition in program.propositions() {
        retain_symbol(retained, index, proposition.symbol);
        for binder in program.proposition_binders(proposition) {
            retain_symbol(retained, index, binder.symbol);
            if let typed_trees::proposition::PropositionBinderKind::Const { type_reference } =
                binder.kind
            {
                retain_type_machines(program, index, retained, type_reference);
            }
        }
        for parameter in program.proposition_parameters(proposition) {
            retain_type_machines(program, index, retained, parameter.type_reference);
        }
        match &proposition.body {
            typed_trees::proposition::PropositionBody::Witness { evidence } => {
                retain_type_machines(program, index, retained, *evidence);
            }
            typed_trees::proposition::PropositionBody::Transparent { proposition } => {
                retain_proposition_formula_machines(program, index, retained, proposition);
            }
            typed_trees::proposition::PropositionBody::Primitive => {}
        }
    }

    for measure in program.measures() {
        retain_symbol(retained, index, measure.symbol);
        if let Some(parameter) = &measure.parameter {
            retain_type_machines(program, index, retained, parameter.type_reference);
        }
        retain_type_machines(program, index, retained, measure.return_type);
        for expression in program.expression_table.expression_handles(measure.body) {
            retain_expression_machines(program, index, retained, *expression);
        }
    }

    for declaration in program.const_declarations() {
        retain_symbol(retained, index, declaration.symbol);
        retain_type_machines(program, index, retained, declaration.declared_type);
        retain_expression_machines(program, index, retained, declaration.authored_initializer);
        retain_expression_machines(
            program,
            index,
            retained,
            declaration.materialized_initializer,
        );
    }

    for operator in program.operators() {
        retain_symbol(retained, index, operator.symbol);
        for parameter in program
            .tables
            .state_parameters
            .span_or_empty(operator.parameters)
        {
            retain_type_machines(program, index, retained, parameter.type_reference);
        }
        retain_type_machines(program, index, retained, operator.return_type);
        for contract in program
            .tables
            .signature_contracts
            .span_or_empty(operator.contracts)
        {
            retain_contract_machines(program, index, retained, contract);
        }
    }

    for domain in program.domain_definitions() {
        retain_symbol(retained, index, domain.symbol);
        retain_type_machines(program, index, retained, domain.target_type);
        for argument in &domain.index_arguments {
            retain_type_machines(program, index, retained, *argument);
        }
        for fact in program.proof_facts(domain) {
            retain_proof_fact_machines(program, index, retained, fact);
        }
    }

    for schema in program.wire_schemas() {
        retain_symbol(retained, index, schema.symbol);
        for member in program.tables.wire_members.span_or_empty(schema.members) {
            if let typed_trees::wire::WireMember::Field(field) = member {
                retain_type_machines(program, index, retained, field.type_reference);
            }
        }
    }

    // Exact typed ranking-witness expressions are compiler custody for
    // termination evidence; they are machine-keyed rows so retention here is
    // conservative (reachable machines also produce their own edges).
    for custody in &program.ranking_expression_custody {
        for subject in &custody.subjects {
            retain_expression_machines(program, index, retained, *subject);
        }
        for argument in &custody.view_arguments {
            retain_expression_machines(program, index, retained, *argument);
        }
        if let Some(rank_range) = custody.rank_range {
            retain_expression_machines(program, index, retained, rank_range);
        }
    }
}

/// Retain machines referenced by one proof fact: embedded expressions,
/// membership values and domain arguments, or proposition applications.
fn retain_proof_fact_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
    fact: &ProofFact,
) {
    match fact {
        ProofFact::Expression(expression) => {
            retain_expression_machines(program, index, retained, *expression);
        }
        ProofFact::Membership(membership) => {
            retain_expression_machines(program, index, retained, membership.value);
            retain_symbol(retained, index, membership.domain_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(membership.domain_arguments)
            {
                retain_type_machines(program, index, retained, *argument);
            }
        }
        ProofFact::Proposition(application) => {
            retain_proposition_application_machines(program, index, retained, application);
        }
    }
}

/// Retain machines named by one proposition application: machine-kind binder
/// arguments plus every expression argument.
fn retain_proposition_application_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
    application: &PropositionApplication,
) {
    retain_symbol(retained, index, application.proposition);
    for binder in application.binder_arguments.iter() {
        if binder.kind == PropositionBinderArgumentKind::Machine {
            retain_symbol(retained, index, binder.symbol);
        }
    }
    for argument in program
        .expression_table
        .expression_handles(application.arguments)
    {
        retain_expression_machines(program, index, retained, *argument);
    }
}

/// Retain machines named by one proposition formula body.
fn retain_proposition_formula_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
    formula: &PropositionFormula,
) {
    match formula {
        PropositionFormula::Application(application) => {
            retain_proposition_application_machines(program, index, retained, application);
        }
        PropositionFormula::BooleanExpression(expression) => {
            retain_expression_machines(program, index, retained, *expression);
        }
    }
}

/// Retain machines referenced by one state signature or operator signature:
/// parameter types, return type, invokes targets, and contract facts.
fn retain_signature_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
    signature: &typed_trees::signature::StateSignature,
) {
    for parameter in program
        .tables
        .state_parameters
        .span_or_empty(signature.parameters)
    {
        retain_type_machines(program, index, retained, parameter.type_reference);
    }
    retain_type_machines(program, index, retained, signature.return_type);
    for invocation in program
        .tables
        .signature_invokes
        .span_or_empty(signature.invokes)
    {
        if let AuthoredInvocationTarget::Service(target) = invocation.target {
            retain_symbol(retained, index, target);
        }
    }
    for contract in program
        .tables
        .signature_contracts
        .span_or_empty(signature.contracts)
    {
        retain_contract_machines(program, index, retained, contract);
    }
}

/// Retain machines referenced by one signature contract's proof facts.
fn retain_contract_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
    contract: &SignatureContract,
) {
    for fact in program.tables.proof_facts.span_or_empty(contract.facts) {
        retain_proof_fact_machines(program, index, retained, fact);
    }
}

/// Retain every machine symbol appearing in one type reference, including
/// const-expression index payloads that can call machines.
fn retain_type_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
    type_reference: TypeReferenceHandle,
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            retain_type_machines(program, index, retained, *referee);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            retain_type_machines(program, index, retained, *base_type);
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    typed_trees::types::TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } => {
                        retain_expression_machines(program, index, retained, *minimum);
                        retain_expression_machines(program, index, retained, *maximum);
                    }
                    typed_trees::types::TypeConstraintNode::Domain(domain) => {
                        retain_symbol(retained, index, domain.symbol);
                        for argument in &domain.arguments {
                            retain_type_machines(program, index, retained, *argument);
                        }
                    }
                    typed_trees::types::TypeConstraintNode::Named(_)
                    | typed_trees::types::TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            retain_type_machines(program, index, retained, *element_type);
            match length {
                FixedArrayLength::ConstParameter { symbol, .. } => {
                    retain_symbol(retained, index, *symbol);
                }
                FixedArrayLength::Literal(_) | FixedArrayLength::ConstCall { .. } => {}
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            retain_type_machines(program, index, retained, *element_type);
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            retain_symbol(retained, index, *base_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                retain_type_machines(program, index, retained, *argument);
            }
        }
        TypeReferenceNode::ConstExpression(expression) => {
            retain_expression_machines(program, index, retained, *expression);
        }
        TypeReferenceNode::DynamicTrait {
            symbol,
            conformance,
            ..
        } => {
            retain_symbol(retained, index, *symbol);
            if let Some(conformance) = conformance {
                retain_symbol(retained, index, *conformance);
            }
        }
        TypeReferenceNode::Named { symbol, .. } => {
            retain_symbol(retained, index, *symbol);
        }
        TypeReferenceNode::Unit => {}
    }
}

/// Retain every machine referenced by one typed expression node, recursively.
fn retain_expression_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
    expression: ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(match_expression) => {
            retain_expression_machines(program, index, retained, match_expression.subject);
            for arm in program.expression_table.match_arms(match_expression.arms) {
                retain_match_arm_machines(program, index, retained, arm);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                retain_expression_machines(program, index, retained, *element);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            retain_expression_machines(program, index, retained, atomic.value);
            retain_expression_machines(program, index, retained, atomic.result);
        }
        ExpressionNode::Binary(binary) => {
            retain_expression_machines(program, index, retained, binary.left);
            retain_expression_machines(program, index, retained, binary.right);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
        ExpressionNode::Cast(cast) => {
            retain_expression_machines(program, index, retained, cast.value);
            retain_type_machines(program, index, retained, cast.target_type);
            retain_type_machines(program, index, retained, cast.result_type);
            retain_symbol(retained, index, cast.semantic_domain_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(cast.semantic_domain_arguments)
            {
                retain_type_machines(program, index, retained, *argument);
            }
        }
        ExpressionNode::Call(call) => {
            retain_call_expression_machines(program, index, retained, call);
        }
        ExpressionNode::Indexed(indexed) => {
            retain_expression_machines(program, index, retained, indexed.collection);
            retain_expression_machines(program, index, retained, indexed.index);
        }
        ExpressionNode::Member(member) => {
            retain_expression_machines(program, index, retained, member.receiver);
            retain_symbol(retained, index, member.member_symbol);
        }
        ExpressionNode::Borrow(borrow) => {
            retain_expression_machines(program, index, retained, borrow.target);
        }
        ExpressionNode::Name(path) => {
            retain_name_path_machines(program, index, retained, path);
        }
        ExpressionNode::Range(range) => {
            retain_expression_machines(program, index, retained, range.start);
            retain_expression_machines(program, index, retained, range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            retain_symbol(retained, index, literal.type_symbol);
            if let Some(case_symbol) = literal.case_symbol {
                retain_symbol(retained, index, case_symbol);
            }
            for field in program.expression_table.struct_fields(literal.fields) {
                retain_expression_machines(program, index, retained, field.value);
            }
        }
        ExpressionNode::Unary(unary) => {
            retain_expression_machines(program, index, retained, unary.operand);
        }
        ExpressionNode::ZeroValue(type_reference) => {
            retain_type_machines(program, index, retained, *type_reference);
        }
    }
}

/// Retain machines referenced by one match arm pattern and value.
fn retain_match_arm_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
    arm: &TableMatchArm,
) {
    if let typed_trees::expression::MatchPattern::Value(pattern) = &arm.pattern {
        retain_expression_machines(program, index, retained, *pattern);
    }
    retain_expression_machines(program, index, retained, arm.value);
}

/// Retain machines named by a resolved name path (head, member, and final
/// symbols can all resolve to machines).
fn retain_name_path_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
    path: &TableNamePath,
) {
    retain_symbol(retained, index, path.head_symbol);
    retain_symbol(retained, index, path.symbol);
    for member in program
        .expression_table
        .name_path_member_symbols(path.member_symbols)
    {
        retain_symbol(retained, index, *member);
    }
}

/// Retain the call target and every static argument of one typed call
/// expression, including static requirement dispatch and sealed-operation
/// applications.
fn retain_call_expression_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut impl Extend<SymbolHandle>,
    call: &TableCallExpression,
) {
    retain_symbol(retained, index, call.target_symbol);
    retain_symbol(retained, index, call.static_machine_parameter);
    retain_expression_machines(program, index, retained, call.receiver);
    if let Some(dispatch) = &call.static_requirement_dispatch {
        retain_symbol(retained, index, dispatch.realization_machine);
        retain_symbol(retained, index, dispatch.realization_state);
        retain_symbol(retained, index, dispatch.requirement);
        retain_symbol(retained, index, dispatch.declaring_trait);
    }
    for argument in call.machine_arguments.iter() {
        retain_static_machine_argument(retained, index, argument);
    }
    if let Some(quotient) = &call.quotient_operation {
        retain_static_machine_argument(retained, index, &quotient.representative_operation);
        for theorem in quotient.theorem_evidence.iter() {
            retain_static_machine_argument(retained, index, &theorem.application);
        }
    }
    if let Some(layout) = &call.private_layout_operation {
        retain_static_machine_argument(retained, index, &layout.selected_slot);
    }
    for argument in program.expression_table.expression_handles(call.arguments) {
        retain_expression_machines(program, index, retained, *argument);
    }
}

/// The checked machine edge relation: `source -> machines it can reach`.
/// Every entry is an exact symbol identity captured from the checked
/// representation; no edge is derived from names or spellings.
pub(super) fn collect_machine_edges(
    program: &TypedTrees,
    facts: &CheckFacts,
    index: &MachineIndex,
) -> HashMap<SymbolHandle, Vec<SymbolHandle>> {
    let mut edges: HashMap<SymbolHandle, Vec<SymbolHandle>> = HashMap::new();

    // Authoritative checked call facts: every call the semantic traversal
    // captured, keyed by the owning machine.
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            push_edge(
                &mut edges,
                state.machine_symbol,
                index.machine_of(call.target_symbol),
            );
        }
    }

    for machine in program.machines() {
        let from = machine.symbol;

        for invocation in program.machine_invokes(machine) {
            if let AuthoredInvocationTarget::Service(target) = invocation.target {
                push_edge(&mut edges, from, index.machine_of(target));
            }
        }

        for row in program
            .authored_service_reach_rows
            .iter()
            .filter(|row| row.owner == from)
        {
            for target in &row.targets {
                push_edge(&mut edges, from, index.machine_of(target.service));
            }
        }

        for conformance in program.machine_trait_conformances(machine) {
            push_edge(
                &mut edges,
                from,
                index.machine_of(conformance.requirement_symbol),
            );
            retain_expression_machines_into(program, index, &mut edges, from, conformance);
        }

        for owned in program.machine_owned_data(machine) {
            retain_type_machines(
                program,
                index,
                edges.entry(from).or_default(),
                owned.type_reference,
            );
            retain_expression_machines(
                program,
                index,
                edges.entry(from).or_default(),
                owned.initial_value,
            );
        }

        for bound in &machine.conformance_bounds {
            if let Some(selected) = &bound.selected_conformance {
                collect_static_argument_edges(index, &mut edges, from, selected);
            }
        }

        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                retain_type_machines(
                    program,
                    index,
                    edges.entry(from).or_default(),
                    parameter.type_reference,
                );
            }
            retain_type_machines(
                program,
                index,
                edges.entry(from).or_default(),
                state.return_type,
            );
            for contract in program.state_contracts(state) {
                for fact in program.tables.proof_facts.span_or_empty(contract.facts) {
                    retain_proof_fact_machines(
                        program,
                        index,
                        edges.entry(from).or_default(),
                        fact,
                    );
                }
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                collect_statement_edges(program, index, &mut edges, from, statement);
            }
        }
    }

    // Machine-specialization arguments bind a retained instance to the exact
    // machines it may invoke through its static telescope.
    for specialization in &program.machine_specializations {
        for argument in &specialization.machine_arguments {
            push_edge(
                &mut edges,
                specialization.instance,
                index.machine_of(*argument),
            );
        }
        for application in &specialization.conformance_applications {
            for argument in application.machine_arguments.iter() {
                push_edge(
                    &mut edges,
                    specialization.instance,
                    index.machine_of(*argument),
                );
            }
            for row in &application.rows {
                push_edge(
                    &mut edges,
                    specialization.instance,
                    index.machine_of(row.realization_machine),
                );
            }
        }
    }

    // Contract evidence calls owned by a machine are call edges into the
    // exact evidence target.
    for call in &facts.proof.contract_expression_evidence_calls {
        if let checked_trees::ContractProofFactOwner::Machine { machine_symbol }
        | checked_trees::ContractProofFactOwner::MachineState { machine_symbol, .. } = call.owner
        {
            push_edge(
                &mut edges,
                machine_symbol,
                index.machine_of(call.target_machine_symbol),
            );
        }
    }
    for application in &facts
        .proof
        .contract_expression_static_conformance_applications
    {
        if let checked_trees::ContractProofFactOwner::Machine { machine_symbol }
        | checked_trees::ContractProofFactOwner::MachineState { machine_symbol, .. } =
            application.owner
        {
            for argument in application.application.machine_arguments.iter() {
                push_edge(&mut edges, machine_symbol, index.machine_of(*argument));
            }
            for row in &application.application.rows {
                push_edge(
                    &mut edges,
                    machine_symbol,
                    index.machine_of(row.realization_machine),
                );
            }
        }
    }

    // Proof-position recursive components retain their exact internal call
    // edges even where the ordinary call traversal does not observe them.
    for component in &facts.termination.proof_recursive_components {
        for edge in &component.edges {
            push_edge(&mut edges, edge.caller, index.machine_of(edge.callee));
        }
    }

    // Machines contained in a carried record field are reachable through the
    // carrier's ownership topology.
    for (_, topology) in facts.carry.machine_topologies.iter() {
        for field in facts.carry.contained_fields.span_or_empty(topology.fields) {
            for target in facts.carry.contained_targets.span_or_empty(field.targets) {
                push_edge(
                    &mut edges,
                    topology.machine,
                    index.machine_of(target.machine),
                );
            }
        }
    }

    // Suspension-crossing rows retain the exact call target they may suspend
    // across.
    for crossing in &facts.carry.suspension_crossings {
        push_edge(
            &mut edges,
            crossing.machine,
            index.machine_of(crossing.target),
        );
    }

    // Checked semantic dependencies include `AutomaticCleanupMachine` rows
    // (consumer -> cleanup machine it implicitly invokes) and any
    // machine-valued nominal/layout dependency.
    for row in facts.flow.semantic_dependencies.iter() {
        push_edge(
            &mut edges,
            row.consumer_machine,
            index.machine_of(row.dependency),
        );
    }

    // Dynamic-dispatch plan rows bind each retained caller to its selected
    // realization machine.
    let dispatch = &facts.flow.terminal_unit_effects.dynamic_dispatch;
    for call in &dispatch.calls {
        for realization in call.realization_machines() {
            push_edge(
                &mut edges,
                call.caller_machine(),
                index.machine_of(realization),
            );
        }
    }
    for transfer in &dispatch.transfers {
        push_edge(
            &mut edges,
            transfer.caller_machine,
            index.machine_of(transfer.target_machine),
        );
    }

    // Traversal writes directly into each owner's final dependency storage.
    // Normalize once: repeated expression, proof, and checked-fact edges have
    // the same reachability meaning, and traversal order is not product order.
    for targets in edges.values_mut() {
        targets.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        targets.dedup();
    }
    edges
}

/// Append one resolved machine edge; unresolved symbols are non-machine
/// identities and contribute nothing.
fn push_edge(
    edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
    from: SymbolHandle,
    to: Option<SymbolHandle>,
) {
    if let Some(to) = to {
        edges.entry(from).or_default().push(to);
    }
}

/// Retain `via` expressions on machine satisfies rows as call edges.
fn retain_expression_machines_into(
    program: &TypedTrees,
    index: &MachineIndex,
    edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
    from: SymbolHandle,
    conformance: &TraitConformance,
) {
    retain_expression_machines(
        program,
        index,
        edges.entry(from).or_default(),
        conformance.via_expression,
    );
    for argument in program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    {
        retain_type_machines(program, index, edges.entry(from).or_default(), *argument);
    }
}

/// Edges contributed by one statement node inside a retained machine.
fn collect_statement_edges(
    program: &TypedTrees,
    index: &MachineIndex,
    edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
    from: SymbolHandle,
    statement: &StatementNode,
) {
    match statement {
        StatementNode::RootBinding(binding) => {
            retain_expression_machines(
                program,
                index,
                edges.entry(from).or_default(),
                binding.receiver,
            );
            if binding.implementation_operand.is_valid() {
                retain_expression_machines(
                    program,
                    index,
                    edges.entry(from).or_default(),
                    binding.implementation_operand,
                );
            }
        }
        StatementNode::AssemblyFact(fact) => {
            retain_expression_machines(
                program,
                index,
                edges.entry(from).or_default(),
                fact.expression,
            );
        }
        StatementNode::Assignment(assignment) => {
            retain_expression_machines(
                program,
                index,
                edges.entry(from).or_default(),
                assignment.target,
            );
            retain_expression_machines(
                program,
                index,
                edges.entry(from).or_default(),
                assignment.value,
            );
        }
        StatementNode::Call(call) => {
            if let Some(target) = index.machine_of(call.target_symbol) {
                edges.entry(from).or_default().push(target);
            }
            if let Some(target) = index.machine_of(call.static_machine_parameter) {
                edges.entry(from).or_default().push(target);
            }
            if let Some(dispatch) = &call.static_requirement_dispatch {
                if let Some(target) = index.machine_of(dispatch.realization_machine) {
                    edges.entry(from).or_default().push(target);
                }
                if let Some(target) = index.machine_of(dispatch.realization_state) {
                    edges.entry(from).or_default().push(target);
                }
            }
            for argument in call.machine_arguments.iter() {
                collect_static_argument_edges(index, edges, from, argument);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                retain_expression_machines(
                    program,
                    index,
                    edges.entry(from).or_default(),
                    *argument,
                );
            }
        }
        StatementNode::Expression(expression) => {
            retain_expression_machines(program, index, edges.entry(from).or_default(), *expression);
        }
        StatementNode::LocalData(local) => {
            retain_type_machines(
                program,
                index,
                edges.entry(from).or_default(),
                local.type_reference,
            );
            retain_expression_machines(
                program,
                index,
                edges.entry(from).or_default(),
                local.initial_value,
            );
        }
        StatementNode::Transition(transition) => {
            for target_handle in [transition.target, transition.continuation] {
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        if let Some(target) = index.machine_of(path.symbol) {
                            edges.entry(from).or_default().push(target);
                        }
                        if let Some(target) = index.machine_of(path.head_symbol) {
                            edges.entry(from).or_default().push(target);
                        }
                        for argument in program.expression_table.expression_handles(*arguments) {
                            retain_expression_machines(
                                program,
                                index,
                                edges.entry(from).or_default(),
                                *argument,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        retain_expression_machines(
                            program,
                            index,
                            edges.entry(from).or_default(),
                            *expression,
                        );
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
            if let TransitionGuardNode::When(guard) = transition.guard {
                retain_expression_machines(program, index, edges.entry(from).or_default(), guard);
            }
        }
    }
}

/// Edges contributed by machines named inside a static machine argument.
fn collect_static_argument_edges(
    index: &MachineIndex,
    edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
    from: SymbolHandle,
    argument: &StaticMachineArgument,
) {
    if let Some(target) = index.machine_of(argument.symbol) {
        edges.entry(from).or_default().push(target);
    }
    if let Some(application) = &argument.application {
        for nested in application.arguments.iter() {
            collect_static_argument_edges(index, edges, from, nested);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExpressionNode, HashSet, ProofFact, SymbolHandle, TableNamePath, TypeReferenceNode,
        TypedTrees,
    };
    use crate::product_pruning::MachineIndex;
    use crate::product_pruning::dependencies::retain_expression_machines;
    use crate::product_pruning::dependencies::retain_proof_fact_machines;
    use crate::product_pruning::dependencies::retain_type_machines;
    use typed_trees::machine::Machine;

    #[test]
    fn nested_expression_type_and_proof_dependencies_share_collection() {
        let first = SymbolHandle::from_arena_index(4);
        let second = SymbolHandle::from_arena_index(7);
        let mut program = TypedTrees::default();
        for symbol in [first, second] {
            program.push_machine(Machine {
                symbol,
                ..Machine::default()
            });
        }
        let index = MachineIndex::build(&program);
        let first_name = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                head_symbol: first,
                symbol: first,
                ..TableNamePath::default()
            }));
        let second_name = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                head_symbol: second,
                symbol: second,
                ..TableNamePath::default()
            }));
        let children = program.expression_table.insert_expression_handles([
            second_name,
            first_name,
            second_name,
        ]);
        let expression = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(children));
        let expression_type = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(expression));
        let nested_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice {
                element_type: expression_type,
            });
        let proof = ProofFact::Expression(expression);

        // The traversal writes occurrences directly into caller-owned storage.
        // A graph normalizes once after every dependency source has contributed;
        // conservative interface retention deduplicates directly in its set.
        let expected = [second, second, first, first, second, second];
        let mut expression_targets = Vec::new();
        retain_expression_machines(&program, &index, &mut expression_targets, expression);
        assert_eq!(expression_targets, expected);
        let mut type_targets = Vec::new();
        retain_type_machines(&program, &index, &mut type_targets, nested_type);
        assert_eq!(type_targets, expected);
        let mut proof_targets = Vec::new();
        retain_proof_fact_machines(&program, &index, &mut proof_targets, &proof);
        assert_eq!(proof_targets, expected);
        let mut interface_targets = HashSet::new();
        retain_type_machines(&program, &index, &mut interface_targets, nested_type);
        retain_proof_fact_machines(&program, &index, &mut interface_targets, &proof);
        assert_eq!(interface_targets, HashSet::from([first, second]));
    }
}
