//! Activation-wide CPU/thread carry analysis.
//!
//! This scans all typed storage and transient values because strict values can
//! exist outside semantic suspension crossings. It derives obligations for an
//! activation plan; it does not model or select a runtime preemption mode.

use psi_language_semantics::CarryPolicy;
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use psi_typed_trees::types::TypeReferenceHandle;

pub(super) fn build_machine_activation_carry_facts(
    program: &psi_typed_trees::TypedTrees,
    carry: &psi_checked_trees::CarryFacts,
    semantic: &psi_facts::FactPlan,
) -> Vec<psi_checked_trees::MachineActivationCarryFact> {
    let direct = program
        .machines()
        .iter()
        .map(|machine| build_machine_activation_carry_fact(program, semantic, machine))
        .collect::<Vec<_>>();

    program
        .machines()
        .iter()
        .map(|machine| join_machine_subtree_activation_carry(machine.symbol, carry, &direct))
        .collect()
}

fn join_machine_subtree_activation_carry(
    root: SymbolHandle,
    carry: &psi_checked_trees::CarryFacts,
    direct: &[psi_checked_trees::MachineActivationCarryFact],
) -> psi_checked_trees::MachineActivationCarryFact {
    let mut joined = psi_checked_trees::MachineActivationCarryFact {
        machine: root,
        effective: CarryPolicy::PERMISSIVE,
        analysis_complete: true,
        contributing_types: Vec::new(),
        unnamed_strict_values: 0,
    };

    for machine in carry.machine_subtree_symbols(root) {
        let Some(fact) = direct.iter().find(|fact| fact.machine == machine) else {
            joined.analysis_complete = false;
            continue;
        };
        joined.effective = joined.effective.intersect(fact.effective);
        joined.analysis_complete &= fact.analysis_complete;
        joined.unnamed_strict_values = joined
            .unnamed_strict_values
            .saturating_add(fact.unnamed_strict_values);
        for type_reference in &fact.contributing_types {
            if !joined.contributing_types.contains(type_reference) {
                joined.contributing_types.push(*type_reference);
            }
        }
    }

    joined
}

fn build_machine_activation_carry_fact(
    program: &psi_typed_trees::TypedTrees,
    semantic: &psi_facts::FactPlan,
    machine: &psi_typed_trees::machine::Machine,
) -> psi_checked_trees::MachineActivationCarryFact {
    let mut accumulator = ActivationCarryAccumulator {
        program,
        machine_type_parameters: program.machine_type_parameters(machine),
        effective: CarryPolicy::PERMISSIVE,
        analysis_complete: true,
        contributing_types: Vec::new(),
        unnamed_strict_values: 0,
    };

    // Persistent activation storage exists independently of lexical use and
    // contributes to the activation-wide preservation requirement.
    if let Some(attached_name) = machine.attached_data.as_ref()
        && let Some(attached) = program
            .data_definitions()
            .iter()
            .find(|definition| same_semantic_name(definition.name.as_str(), attached_name.as_str()))
    {
        for member in program.data_members(attached) {
            match member {
                psi_typed_trees::data::DataMember::Field(field) => {
                    accumulator.add_machine_type(field.type_reference);
                }
                psi_typed_trees::data::DataMember::Variant(variant) => {
                    for field in program.data_payload_fields(variant) {
                        accumulator.add_machine_type(field.type_reference);
                    }
                }
            }
        }
    }
    for owned in program.machine_owned_data(machine) {
        accumulator.add_machine_type(owned.type_reference);
        accumulator.visit_expression(owned.initial_value);
    }

    for state in program.machine_states(machine) {
        for parameter in program.state_parameters(state) {
            accumulator.add_machine_type(parameter.type_reference);
        }
        accumulator.add_machine_return_type(state.return_type);

        for statement in program.statement_table.statements(state.statement_nodes) {
            if let StatementNode::LocalData(local) = statement {
                accumulator.add_machine_type(local.type_reference);
            }
            accumulator.visit_statement(statement);
        }
    }
    if let Some(claim_policy) = established_claim_carry_policy(program, semantic, machine.symbol) {
        accumulator.effective = accumulator.effective.intersect(claim_policy);
    }

    psi_checked_trees::MachineActivationCarryFact {
        machine: machine.symbol,
        effective: accumulator.effective,
        analysis_complete: accumulator.analysis_complete,
        contributing_types: accumulator.contributing_types,
        unnamed_strict_values: accumulator.unnamed_strict_values,
    }
}

fn established_claim_carry_policy(
    program: &psi_typed_trees::TypedTrees,
    semantic: &psi_facts::FactPlan,
    machine: SymbolHandle,
) -> Option<CarryPolicy> {
    let mut claims = Vec::<(String, psi_facts::QualificationEvidence, CarryPolicy)>::new();

    for (_, fact) in semantic.facts.iter() {
        if fact.evidence.origin == psi_language_semantics::QualificationEvidenceOrigin::None
            || fact_point_machine(fact.point) != Some(machine)
        {
            continue;
        }
        let permission = match fact.payload {
            psi_facts::FactPayload::CarryPermission { permission, .. }
            | psi_facts::FactPayload::ContractCarryPermission { permission, .. } => {
                Some(permission)
            }
            psi_facts::FactPayload::CarryOrigin { .. } => None,
            _ => continue,
        };
        let psi_facts::FactPlace::Place(place) = fact.place else {
            continue;
        };
        let place =
            crate::labels::canonical_place_label(program, semantic, semantic.places.get(place));
        if let Some((_, _, policy)) = claims
            .iter_mut()
            .find(|(candidate, evidence, _)| *candidate == place && *evidence == fact.evidence)
        {
            if let Some(permission) = permission {
                *policy = permission.relax(*policy);
            }
        } else {
            claims.push((
                place,
                fact.evidence,
                permission
                    .map(|permission| permission.relax(CarryPolicy::STRICT))
                    .unwrap_or(CarryPolicy::STRICT),
            ));
        }
    }

    (!claims.is_empty()).then(|| {
        claims
            .into_iter()
            .fold(CarryPolicy::PERMISSIVE, |combined, (_, _, policy)| {
                combined.intersect(policy)
            })
    })
}

fn fact_point_machine(point: psi_facts::ProgramPoint) -> Option<SymbolHandle> {
    match point {
        psi_facts::ProgramPoint::Machine { machine_symbol }
        | psi_facts::ProgramPoint::State { machine_symbol, .. }
        | psi_facts::ProgramPoint::Statement { machine_symbol, .. }
        | psi_facts::ProgramPoint::Call { machine_symbol, .. }
        | psi_facts::ProgramPoint::CallRequires { machine_symbol, .. }
        | psi_facts::ProgramPoint::CallEnsures { machine_symbol, .. }
        | psi_facts::ProgramPoint::Exit { machine_symbol, .. }
        | psi_facts::ProgramPoint::TransitionArm { machine_symbol, .. } => Some(machine_symbol),
        psi_facts::ProgramPoint::Global | psi_facts::ProgramPoint::Definition { .. } => None,
    }
}

struct ActivationCarryAccumulator<'program> {
    program: &'program psi_typed_trees::TypedTrees,
    machine_type_parameters: &'program [psi_typed_trees::data::TypeParameter],
    effective: CarryPolicy,
    analysis_complete: bool,
    contributing_types: Vec<TypeReferenceHandle>,
    unnamed_strict_values: usize,
}

impl ActivationCarryAccumulator<'_> {
    fn add_machine_type(&mut self, type_reference: TypeReferenceHandle) {
        self.add_type(self.machine_type_parameters, type_reference);
    }

    fn add_machine_return_type(&mut self, type_reference: TypeReferenceHandle) {
        self.add_return_type(self.machine_type_parameters, type_reference);
    }

    fn add_return_type(
        &mut self,
        type_parameters: &[psi_typed_trees::data::TypeParameter],
        type_reference: TypeReferenceHandle,
    ) {
        // An absent authored return type is the resolved unit result, whose
        // carry policy is permissive; it is not a hole in checked type data.
        if type_reference.is_valid() {
            self.add_type(type_parameters, type_reference);
        }
    }

    fn add_type(
        &mut self,
        type_parameters: &[psi_typed_trees::data::TypeParameter],
        type_reference: TypeReferenceHandle,
    ) {
        if !type_reference.is_valid() {
            self.analysis_complete = false;
            return;
        }
        let policy = psi_validation::effective_type_carry_policy(
            self.program,
            type_parameters,
            type_reference,
        );
        self.effective = self.effective.intersect(policy);
        if !self.contributing_types.contains(&type_reference) {
            self.contributing_types.push(type_reference);
        }
    }

    fn add_unnamed_policy(&mut self, policy: CarryPolicy) {
        self.effective = self.effective.intersect(policy);
        if policy != CarryPolicy::PERMISSIVE {
            self.unnamed_strict_values += 1;
        }
    }

    fn visit_statement(&mut self, statement: &StatementNode) {
        match statement {
            StatementNode::AssemblyFact(fact) => self.visit_expression(fact.expression),
            StatementNode::Assignment(assignment) => {
                self.visit_expression(assignment.target);
                self.visit_expression(assignment.value);
            }
            StatementNode::Call(call) => {
                self.add_call_signature(call.target_symbol);
                for argument in self
                    .program
                    .statement_table
                    .expression_handles(call.arguments)
                {
                    self.visit_expression(*argument);
                }
            }
            StatementNode::Expression(expression) => self.visit_expression(*expression),
            StatementNode::LocalData(local) => self.visit_expression(local.initial_value),
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    self.visit_expression(guard);
                }
                self.visit_transition_target(transition.target);
                if transition.continuation.is_valid() {
                    self.visit_transition_target(transition.continuation);
                }
            }
        }
    }

    fn visit_transition_target(
        &mut self,
        target: psi_typed_trees::statement::TransitionTargetHandle,
    ) {
        if !target.is_valid() {
            return;
        }
        match self.program.statement_table.transition_target(target) {
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                self.add_call_signature(path.symbol);
                for argument in self.program.statement_table.expression_handles(*arguments) {
                    self.visit_expression(*argument);
                }
            }
            TransitionTargetNode::Value(expression) => self.visit_expression(*expression),
            TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
        }
    }

    fn visit_expression(&mut self, expression: ExpressionHandle) {
        if !expression.is_valid() {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Atomic(atomic) => {
                self.visit_expression(atomic.value);
                self.visit_expression(atomic.result);
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in self.program.expression_table.expression_handles(*values) {
                    self.visit_expression(*value);
                }
            }
            ExpressionNode::Binary(binary) => {
                self.visit_expression(binary.left);
                self.visit_expression(binary.right);
            }
            ExpressionNode::Call(call) => {
                self.add_call_signature(call.target_symbol);
                self.visit_expression(call.receiver);
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                {
                    self.visit_expression(*argument);
                }
            }
            ExpressionNode::Cast(cast) => {
                self.visit_expression(cast.value);
                let target_name = self
                    .program
                    .named_type_reference(cast.target_type)
                    .map(|name| name.as_str());
                self.add_unnamed_named_type(target_name);
            }
            ExpressionNode::Indexed(indexed) => {
                self.visit_expression(indexed.collection);
                self.visit_expression(indexed.index);
            }
            ExpressionNode::Member(member) => self.visit_expression(member.receiver),
            ExpressionNode::Borrow(inner) => {
                self.visit_expression(inner.target);
                // The typed expression graph does not allocate a standalone
                // `&T` handle for borrow formation. Its per-value provenance
                // is not yet strong enough to relax any carry axis.
                self.add_unnamed_policy(CarryPolicy::STRICT);
            }
            ExpressionNode::Range(range) => {
                self.visit_expression(range.start);
                self.visit_expression(range.end);
            }
            ExpressionNode::StructLiteral(literal) => {
                self.add_unnamed_named_type(Some(literal.type_name.as_str()));
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    self.visit_expression(field.value);
                }
            }
            ExpressionNode::Unary(unary) => self.visit_expression(unary.operand),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }

    fn add_unnamed_named_type(&mut self, name: Option<&str>) {
        let Some(name) = name else {
            self.analysis_complete = false;
            return;
        };
        if psi_typed_trees::types::PrimitiveType::from_name(name).is_some() {
            return;
        }
        let Some(definition) = self
            .program
            .data_definitions()
            .iter()
            .find(|definition| same_semantic_name(definition.name.as_str(), name))
        else {
            self.analysis_complete = false;
            return;
        };
        self.add_unnamed_policy(psi_validation::effective_data_carry_policy(
            self.program,
            definition,
        ));
    }

    fn add_call_signature(&mut self, target: SymbolHandle) {
        if !target.is_valid() {
            self.analysis_complete = false;
            return;
        }

        if let Some((machine, state)) = self.program.machines().iter().find_map(|machine| {
            self.program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == target)
                .map(|state| (machine, state))
        }) {
            let parameters = self.program.machine_type_parameters(machine);
            for parameter in self.program.state_parameters(state) {
                self.add_type(parameters, parameter.type_reference);
            }
            self.add_return_type(parameters, state.return_type);
            return;
        }

        if let Some(operator) = find_operator(self.program, target) {
            let parameters = self.program.operator_type_parameters(operator);
            for parameter in self.program.operator_parameters(operator) {
                self.add_type(parameters, parameter.type_reference);
            }
            self.add_return_type(parameters, operator.return_type);
            return;
        }

        if let Some((machine, signature)) = self.program.machine_parameter_signature(target) {
            let parameters = self.program.machine_type_parameters(machine);
            for parameter in self.program.state_signature_parameters(signature) {
                self.add_type(parameters, parameter.type_reference);
            }
            self.add_return_type(parameters, signature.return_type);
            return;
        }

        if let Some((trait_definition, signature)) =
            self.program.traits().iter().find_map(|trait_definition| {
                self.program
                    .trait_machine_signatures(trait_definition)
                    .iter()
                    .find(|signature| signature.symbol == target)
                    .map(|signature| (trait_definition, signature))
            })
        {
            let parameters = self.program.trait_type_parameters(trait_definition);
            for parameter in self.program.state_signature_parameters(signature) {
                self.add_type(parameters, parameter.type_reference);
            }
            self.add_return_type(parameters, signature.return_type);
            return;
        }

        self.analysis_complete = false;
    }
}

fn find_operator(
    program: &psi_typed_trees::TypedTrees,
    target: SymbolHandle,
) -> Option<&psi_typed_trees::operator::OperatorDefinition> {
    program
        .operators()
        .iter()
        .find(|operator| operator.symbol == target)
        .or_else(|| {
            program.domain_definitions().iter().find_map(|domain| {
                program
                    .domain_operators(domain)
                    .iter()
                    .find(|operator| operator.symbol == target)
            })
        })
}

fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right || left.rsplit("::").next() == right.rsplit("::").next()
}
