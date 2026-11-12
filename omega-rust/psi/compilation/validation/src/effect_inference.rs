/*
Target-neutral effect, operational-may, synchronous-invocation, and
service-reach fixed-point inference (`machine_calls::effect_inference`).
*/

type ContractDependencyWork<Contribution> =
    flow_effects::ContractDependency<Contribution, Vec<symbols::SymbolHandle>>;

pub(crate) mod call_sites {
    use arena::HandleSpan;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
    use typed_trees::state::State;
    use typed_trees::statement::{
        StatementNode, TableCall, TableNamePath, TransitionGuardNode, TransitionTargetNode,
    };

    pub(crate) trait CallSiteVisitor {
        fn statement_call(&mut self, call: &TableCall, statement_index: usize, ordinal: usize);

        fn expression_call(
            &mut self,
            call: &TableCallExpression,
            statement_index: usize,
            ordinal: usize,
        );

        fn named_transfer(
            &mut self,
            target: SymbolHandle,
            path: &TableNamePath,
            arguments: HandleSpan<ExpressionHandle>,
            static_machine_parameter: SymbolHandle,
            acknowledgement: language_semantics::CallOperationalAcknowledgement,
            statement_index: usize,
            ordinal: usize,
        );
    }

    pub(crate) fn visit_state_calls(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        state: &State,
        visitor: &mut impl CallSiteVisitor,
    ) {
        for (statement_index, statement) in program
            .statement_table
            .indexed_statements(state.statement_nodes)
        {
            if program.static_statement_type_context_is_unreachable(
                state.statement_nodes.handle_at(statement_index as u32),
            ) {
                continue;
            }
            let mut ordinal = 0usize;
            visit_statement(
                program,
                machine,
                statement,
                statement_index,
                &mut ordinal,
                visitor,
            );
        }
    }

    fn visit_statement(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        statement: &StatementNode,
        statement_index: usize,
        ordinal: &mut usize,
        visitor: &mut impl CallSiteVisitor,
    ) {
        match statement {
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
            StatementNode::Assignment(assignment) => {
                visit_expression(
                    program,
                    assignment.target,
                    statement_index,
                    ordinal,
                    visitor,
                );
                visit_expression(program, assignment.value, statement_index, ordinal, visitor);
            }
            StatementNode::Call(call) => {
                visitor.statement_call(call, statement_index, *ordinal);
                reserve_ordinal(ordinal);
                for argument in program.statement_table.expression_handles(call.arguments) {
                    visit_expression(program, *argument, statement_index, ordinal, visitor);
                }
            }
            StatementNode::Expression(expression) => {
                visit_expression(program, *expression, statement_index, ordinal, visitor);
            }
            StatementNode::LocalData(local_data) => {
                if local_data.initial_value.is_valid() {
                    visit_expression(
                        program,
                        local_data.initial_value,
                        statement_index,
                        ordinal,
                        visitor,
                    );
                }
            }
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    visit_expression(program, guard, statement_index, ordinal, visitor);
                }
                visit_transition_target(
                    program,
                    machine,
                    transition.target,
                    statement_index,
                    ordinal,
                    visitor,
                );
                if transition.continuation.is_valid() {
                    visit_transition_target(
                        program,
                        machine,
                        transition.continuation,
                        statement_index,
                        ordinal,
                        visitor,
                    );
                }
            }
        }
    }

    fn visit_transition_target(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        target: typed_trees::statement::TransitionTargetHandle,
        statement_index: usize,
        ordinal: &mut usize,
        visitor: &mut impl CallSiteVisitor,
    ) {
        if !target.is_valid() {
            return;
        }
        match program.statement_table.transition_target(target) {
            TransitionTargetNode::Named {
                path,
                arguments,
                static_machine_parameter,
                operational_acknowledgement,
                ..
            } => {
                let target_symbol = crate::declarations::transitions::named_transition_call_symbol(
                    program,
                    machine,
                    path.symbol,
                );
                if target_symbol.is_valid()
                    && !program.static_transition_type_context_is_unreachable(target)
                {
                    visitor.named_transfer(
                        target_symbol,
                        path,
                        *arguments,
                        *static_machine_parameter,
                        *operational_acknowledgement,
                        statement_index,
                        *ordinal,
                    );
                }
                reserve_ordinal(ordinal);
                for argument in program.statement_table.expression_handles(*arguments) {
                    visit_expression(program, *argument, statement_index, ordinal, visitor);
                }
            }
            TransitionTargetNode::Value(expression) => {
                visit_expression(program, *expression, statement_index, ordinal, visitor);
            }
            TransitionTargetNode::Terminal => {}
        }
    }

    fn visit_expression(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        statement_index: usize,
        ordinal: &mut usize,
        visitor: &mut impl CallSiteVisitor,
    ) {
        if !expression.is_valid() {
            return;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Atomic(atomic) => {
                let inputs = crate::value_custody::atomic_operations::atomic_execution_children(
                    program, atomic,
                );
                for input in inputs {
                    visit_expression(program, input, statement_index, ordinal, visitor);
                }
            }
            ExpressionNode::Call(call) => {
                if !program.static_type_context_is_unreachable(expression) {
                    visitor.expression_call(call, statement_index, *ordinal);
                }
                reserve_ordinal(ordinal);
                visit_expression(program, call.receiver, statement_index, ordinal, visitor);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    visit_expression(program, *argument, statement_index, ordinal, visitor);
                }
            }
            other => crate::queries::expression_traversal::children(program, other, |child| {
                visit_expression(program, child, statement_index, ordinal, visitor);
            }),
        }
    }

    fn reserve_ordinal(ordinal: &mut usize) {
        *ordinal = ordinal.checked_add(1).expect("call ordinal overflow");
    }
}
pub(crate) mod callback_requirements {
    use flow_effects::InvocationTarget;
    use typed_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
    use typed_trees::signature::StateSignature;

    pub(crate) fn opaque_machine_parameters<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> &'program [TypeParameter] {
        match program.machine_specialization(machine.symbol) {
            Some(specialization) => program
                .data_type_parameters
                .span_or_empty(specialization.template_parameters),
            None => program.machine_type_parameters(machine),
        }
    }

    pub(crate) fn opaque_signature_parameters<'program>(
        program: &'program dyn crate::CheckingInput,
        signature: &'program StateSignature,
    ) -> &'program [TypeParameter] {
        let declaration = program
            .closed_requirement_application(signature.symbol)
            .and_then(|application| program.trait_machine_signature(application.template))
            .map_or(signature, |(_, declaration)| declaration);
        program.state_signature_type_parameters(declaration)
    }

    pub(crate) fn visit(
        program: &dyn crate::CheckingInput,
        parameters: &[TypeParameter],
        invocations: &[InvocationTarget],
        mut consume: impl FnMut(&StateSignature),
    ) {
        let mut pending = vec![(parameters, invocations.to_vec())];
        let mut visited = Vec::new();
        while let Some((parameters, invocations)) = pending.pop() {
            for parameter in parameters {
                let TypeParameterKind::Machine { contract } = &parameter.kind else {
                    continue;
                };
                let MachineParameterContract::Nominal {
                    trait_definition, ..
                } = contract
                else {
                    continue;
                };
                if !invocations.contains(&InvocationTarget::Service(*trait_definition))
                    || visited.contains(&parameter.symbol)
                {
                    continue;
                }
                visited.push(parameter.symbol);
                let Some(contract) = program.machine_parameter_contract_view(contract) else {
                    continue;
                };
                let signature = contract.signature();
                consume(signature);
                pending.push((
                    opaque_signature_parameters(program, signature),
                    super::declared_signature_invocations(program, signature),
                ));
            }
        }
    }
}
pub(crate) mod invocations {
    use crate::declarations::data_queries::data_definition_by_symbol;
    use crate::frozen_program::expression_table;
    use arena::HandleSpan;
    use symbols::{SymbolHandle, SymbolMap};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
    use typed_trees::machine::Machine;
    use typed_trees::signature::{StateParameter, StateSignature};
    use typed_trees::state::State;
    use typed_trees::statement::{TableCall, TableNamePath};

    use super::call_sites::{CallSiteVisitor, visit_state_calls};
    use flow_effects::{
        InvocationInferencePlan, InvocationTarget, MachineInvocationInference,
        RealizedInvocationTarget,
    };

    use crate::proof_contracts::default_domains::call_summaries::{
        StateToMachine, state_to_machine_index,
    };

    #[derive(Debug, Clone)]
    pub(crate) struct MachineWork {
        symbol: SymbolHandle,
        family_members: Vec<SymbolHandle>,
        published: Vec<InvocationTarget>,
        uses_published: bool,
        direct: Vec<InvocationTarget>,
        transitive: Vec<InvocationTarget>,
        realized_direct: Vec<RealizedInvocationTarget>,
        realized: Vec<RealizedInvocationTarget>,
        calls: Vec<CallWork>,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct CallWork {
        target_machine: SymbolHandle,
        arguments: Vec<Option<InvocationTarget>>,
        declared: Vec<InvocationTarget>,
    }

    pub(crate) struct MachineInvocationLinks {
        family_targets: Vec<usize>,
        call_targets: Vec<Option<usize>>,
    }

    pub fn infer_synchronous_invocations(
        program: &dyn crate::CheckingInput,
    ) -> InvocationInferencePlan {
        let state_to_machine = state_to_machine_index(program);
        let mut work = program
            .machines()
            .iter()
            .map(|machine| build_machine_work(program, machine, &state_to_machine))
            .collect::<Vec<_>>();
        let mut machine_indices = SymbolMap::default();
        for (index, machine) in work.iter().enumerate() {
            machine_indices.entry(machine.symbol).or_insert(index);
        }

        let operational = super::infer_operational_may(program);
        for (_, dependency) in operational.cleanup_dependencies.iter() {
            if let Some(&caller) = machine_indices.get(&dependency.caller_machine) {
                work[caller].calls.push(CallWork {
                    target_machine: dependency.target_machine,
                    arguments: Vec::new(),
                    declared: Vec::new(),
                });
            }
        }
        let links = work
            .iter()
            .map(|machine| MachineInvocationLinks {
                family_targets: machine
                    .family_members
                    .iter()
                    .filter_map(|member| machine_indices.get(member).copied())
                    .collect(),
                call_targets: machine
                    .calls
                    .iter()
                    .map(|call| machine_indices.get(&call.target_machine).copied())
                    .collect(),
            })
            .collect::<Vec<_>>();

        loop {
            let previous = work
                .iter()
                .map(|machine| (machine.transitive.clone(), machine.realized.clone()))
                .collect::<Vec<_>>();
            for index in 0..work.len() {
                let mut transitive = work[index].direct.clone();
                let mut realized = work[index].realized_direct.clone();
                for &member in &links[index].family_targets {
                    let callee = &work[member];
                    let effective = if callee.uses_published {
                        &callee.published
                    } else {
                        &callee.transitive
                    };
                    extend_targets(&mut transitive, effective);
                    extend_realized(&mut realized, &callee.realized);
                }
                for (call, callee) in work[index].calls.iter().zip(&links[index].call_targets) {
                    extend_targets(&mut transitive, &call.declared);
                    let Some(callee) = callee.map(|callee| &work[callee]) else {
                        for target in &call.declared {
                            extend_realized(
                                &mut realized,
                                &[RealizedInvocationTarget::Abstract(*target)],
                            );
                        }
                        continue;
                    };
                    let effective = if callee.uses_published {
                        &callee.published
                    } else {
                        &callee.transitive
                    };
                    for target in effective {
                        if let Some(target) = substitute_target(*target, &call.arguments) {
                            typed_trees::invocations::insert_target(&mut transitive, target);
                        }
                    }
                    for target in &callee.realized {
                        let target = match *target {
                            RealizedInvocationTarget::Abstract(target) => {
                                substitute_target(target, &call.arguments)
                                    .map(RealizedInvocationTarget::Abstract)
                            }
                            target => Some(target),
                        };
                        if let Some(target) = target {
                            extend_realized(&mut realized, &[target]);
                        }
                    }
                }
                work[index].transitive = transitive;
                work[index].realized = realized;
            }
            if work
                .iter()
                .zip(&previous)
                .all(|(machine, (transitive, realized))| {
                    machine.transitive == *transitive && machine.realized == *realized
                })
            {
                break;
            }
        }

        for machine in &mut work {
            if !machine.family_members.is_empty() {
                machine.published = machine.transitive.clone();
                machine.uses_published = true;
            }
        }

        InvocationInferencePlan {
            machines: work
                .into_iter()
                .map(|machine| {
                    let effective = if machine.uses_published {
                        machine.published.clone()
                    } else {
                        machine.transitive.clone()
                    };
                    MachineInvocationInference {
                        machine: machine.symbol,
                        published: machine.published,
                        inferred_direct: machine.direct,
                        inferred_transitive: machine.transitive,
                        effective,
                        realized: machine.realized,
                    }
                })
                .collect(),
        }
    }

    pub fn declared_signature_invocations(
        program: &dyn crate::CheckingInput,
        signature: &StateSignature,
    ) -> Vec<InvocationTarget> {
        typed_trees::invocations::declared_signature_invocations(program.typed_program(), signature)
    }

    pub(crate) fn full_parameter_invocation(
        target: InvocationTarget,
        parameters: &[StateParameter],
    ) -> Option<InvocationTarget> {
        match target {
            InvocationTarget::Parameter(ordinal) => parameters
                .iter()
                .enumerate()
                .nth(ordinal as usize)
                .and_then(|(position, _)| u32::try_from(position).ok())
                .map(InvocationTarget::Parameter),
            target => Some(target),
        }
    }

    fn explicit_signature_invocations(
        program: &dyn crate::CheckingInput,
        signature: &StateSignature,
    ) -> Vec<InvocationTarget> {
        declared_signature_invocations(program, signature)
            .into_iter()
            .filter_map(|target| {
                full_parameter_invocation(target, program.state_signature_parameters(signature))
            })
            .collect()
    }

    pub fn declared_machine_invocations(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
    ) -> Vec<InvocationTarget> {
        typed_trees::invocations::declared_machine_invocations(program.typed_program(), machine)
    }

    pub fn invocation_target_label(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        target: InvocationTarget,
    ) -> String {
        match target {
            InvocationTarget::Parameter(index) => {
                typed_trees::invocations::machine_entry_parameters(program.typed_program(), machine)
                    .get(index as usize)
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .unwrap_or_else(|| format!("parameter#{index}"))
            }
            InvocationTarget::Service(symbol) => program
                .trait_definition_by_symbol(symbol)
                .map(|definition| definition.name.as_str().to_owned())
                .unwrap_or_else(|| format!("service#{}", symbol.arena_index())),
        }
    }

    pub fn has_self_forwarded_boundary_parameter(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        boundary: SymbolHandle,
        requirement_parameter_count: usize,
    ) -> bool {
        let Some(state) = program.machine_states(machine).first() else {
            return false;
        };
        let parameters = program.state_parameters(state).iter().collect::<Vec<_>>();
        parameters.len() == requirement_parameter_count + 1
            && parameters.first().is_some_and(|parameter| {
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                    .type_symbol(&program.type_reference_table)
                    == boundary
            })
    }

    fn build_machine_work(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state_to_machine: &StateToMachine,
    ) -> MachineWork {
        let family_members = super::family_interface_members(program, machine)
            .iter()
            .map(|member| member.machine)
            .collect::<Vec<_>>();
        let published = if family_members.is_empty() {
            declared_machine_invocations(program, machine)
        } else {
            Vec::new()
        };
        let mut direct = Vec::new();
        let mut realized_direct =
            if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
                published
                    .iter()
                    .copied()
                    .map(RealizedInvocationTarget::Abstract)
                    .collect()
            } else {
                Vec::new()
            };
        let mut calls = Vec::new();
        for state in program.machine_states(machine) {
            let mut sites = InvocationCallSites {
                program,
                machine,
                state,
                state_to_machine,
                direct: &mut direct,
                realized: &mut realized_direct,
                calls: &mut calls,
            };
            visit_state_calls(program, machine.symbol, state, &mut sites);
        }
        MachineWork {
            symbol: machine.symbol,
            uses_published: family_members.is_empty()
                && (machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                    || machine.is_public
                    || !program.machine_invokes(machine).is_empty()),
            family_members,
            published: published.clone(),
            direct: direct.clone(),
            transitive: direct,
            realized: realized_direct.clone(),
            realized_direct,
            calls,
        }
    }

    pub(crate) struct InvocationCallSites<'a> {
        program: &'a dyn crate::CheckingInput,
        machine: &'a Machine,
        state: &'a State,
        state_to_machine: &'a StateToMachine,
        direct: &'a mut Vec<InvocationTarget>,
        realized: &'a mut Vec<RealizedInvocationTarget>,
        calls: &'a mut Vec<CallWork>,
    }

    impl CallSiteVisitor for InvocationCallSites<'_> {
        fn statement_call(&mut self, call: &TableCall, _statement_index: usize, _ordinal: usize) {
            let receiver =
                origin_for_statement_receiver(self.program, self.machine, self.state, call);
            self.resolved_call(
                call.target_symbol,
                call.static_machine_parameter,
                receiver,
                self.program
                    .statement_table
                    .expression_handles(call.arguments),
            );
        }

        fn expression_call(
            &mut self,
            call: &TableCallExpression,
            _statement_index: usize,
            _ordinal: usize,
        ) {
            if expression_table(self.program)
                .build_selection_request(call.build_selection)
                .is_some()
            {
                return;
            }
            let receiver =
                origin_for_expression(self.program, self.machine, self.state, call.receiver);
            self.resolved_call(
                call.target_symbol,
                call.static_machine_parameter,
                receiver,
                expression_table(self.program).expression_handles(call.arguments),
            );
        }

        fn named_transfer(
            &mut self,
            target: SymbolHandle,
            path: &TableNamePath,
            arguments: HandleSpan<ExpressionHandle>,
            static_machine_parameter: SymbolHandle,
            _acknowledgement: language_semantics::CallOperationalAcknowledgement,
            _statement_index: usize,
            _ordinal: usize,
        ) {
            let members = self.program.statement_table.name_path_members(path.members);
            let receiver = members.split_last().and_then(|(_, receiver)| {
                if receiver.is_empty() {
                    return None;
                }
                origin_for_symbol(self.program, self.machine, self.state, path.head_symbol).or_else(
                    || {
                        boundary_service_for_receiver_path(
                            self.program,
                            self.machine,
                            self.state,
                            path.head_symbol,
                            receiver,
                        )
                        .map(InvocationTarget::Service)
                    },
                )
            });
            let arguments = self
                .program
                .statement_table
                .expression_handles(arguments)
                .iter()
                .map(|argument| {
                    origin_for_expression(self.program, self.machine, self.state, *argument)
                })
                .collect();
            let direct = &mut *self.direct;
            let calls = &mut *self.calls;
            push_call(
                self.program,
                target,
                static_machine_parameter,
                receiver,
                arguments,
                direct,
                self.realized,
                calls,
                self.state_to_machine,
            );
        }
    }

    impl InvocationCallSites<'_> {
        fn resolved_call(
            &mut self,
            target_symbol: SymbolHandle,
            static_machine_parameter: SymbolHandle,
            receiver: Option<InvocationTarget>,
            argument_handles: &[ExpressionHandle],
        ) {
            let arguments = argument_handles
                .iter()
                .map(|argument| {
                    origin_for_expression(self.program, self.machine, self.state, *argument)
                })
                .collect::<Vec<_>>();
            let direct = &mut *self.direct;
            let calls = &mut *self.calls;
            push_call(
                self.program,
                target_symbol,
                static_machine_parameter,
                receiver,
                arguments,
                direct,
                self.realized,
                calls,
                self.state_to_machine,
            );
        }
    }

    fn origin_for_statement_receiver(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        call: &TableCall,
    ) -> Option<InvocationTarget> {
        let path = program.statement_table.name_path_members(call.receiver);
        if let Some(origin) = origin_for_symbol(program, machine, state, call.receiver_symbol) {
            return Some(origin);
        }
        if let Some(service) =
            boundary_service_for_receiver_path(program, machine, state, call.receiver_symbol, path)
        {
            return Some(InvocationTarget::Service(service));
        }
        None
    }

    fn boundary_service_for_receiver_path(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        head_symbol: SymbolHandle,
        path: &[typed_trees::name::Identifier],
    ) -> Option<SymbolHandle> {
        let mut type_reference = type_reference_for_symbol(program, machine, state, head_symbol)
            .or_else(|| {
                let head = path.first()?.as_str();
                program
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.name.as_str() == head)
                    .map(|parameter| parameter.type_reference)
                    .or_else(|| {
                        program
                            .machine_owned_data(machine)
                            .iter()
                            .find(|owned| owned.name.as_str() == head)
                            .map(|owned| owned.type_reference)
                    })
            })?;
        for member_name in path.iter().skip(1) {
            let owner_symbol = program
                .type_reference_table
                .type_reference(type_reference)
                .type_symbol(&program.type_reference_table);
            let definition = data_definition_by_symbol(program, owner_symbol)?;
            let field = program.data_members(definition).iter().find_map(|member| {
                let field = member.as_field()?;
                (field.name.as_str() == member_name.as_str()).then_some(field)
            })?;
            type_reference = field.type_reference;
        }
        typed_trees::invocations::boundary_service_for_type(program.typed_program(), type_reference)
    }

    fn type_reference_for_symbol(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        symbol: SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        program
            .state_parameter_by_symbol(state, symbol)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                program
                    .machine_owned_data_by_symbol(machine, symbol)
                    .map(|owned| owned.type_reference)
            })
            .or_else(|| {
                let definition = data_definition_by_symbol(program, machine.attached_data_symbol)?;
                program.data_members(definition).iter().find_map(|member| {
                    let field = member.as_field()?;
                    (field.symbol == symbol).then_some(field.type_reference)
                })
            })
    }

    fn push_call(
        program: &dyn crate::CheckingInput,
        target: SymbolHandle,
        static_machine_parameter: SymbolHandle,
        receiver: Option<InvocationTarget>,
        arguments: Vec<Option<InvocationTarget>>,
        direct: &mut Vec<InvocationTarget>,
        realized: &mut Vec<RealizedInvocationTarget>,
        calls: &mut Vec<CallWork>,
        state_to_machine: &StateToMachine,
    ) {
        let boundary_signature = boundary_trait_for_signature(program, target);
        let crosses_boundary = receiver.is_some() || boundary_signature.is_some();
        if boundary_signature.is_some() {
            extend_realized(realized, &[RealizedInvocationTarget::Requirement(target)]);
        } else if let Some(receiver) = receiver {
            extend_realized(realized, &[RealizedInvocationTarget::Abstract(receiver)]);
        }
        if let Some(receiver) = receiver {
            typed_trees::invocations::insert_target(direct, receiver);
        } else if let Some(service) = boundary_signature {
            typed_trees::invocations::insert_target(direct, InvocationTarget::Service(service));
        }

        let binder_signature = program
            .retained_static_machine_contract(static_machine_parameter)
            .map(typed_trees::data::MachineParameterContractView::signature)
            .or_else(|| {
                program
                    .machine_parameter_signature(target)
                    .map(|(_, signature)| signature)
            });
        let declared = if crosses_boundary {
            Vec::new()
        } else if let Some(signature) = binder_signature {
            explicit_signature_invocations(program, signature)
        } else {
            signature_for_symbol(program, target)
                .map(|signature| declared_signature_invocations(program, signature))
                .unwrap_or_default()
        }
        .into_iter()
        .filter_map(|target| substitute_target(target, &arguments))
        .collect::<Vec<_>>();
        calls.push(CallWork {
            target_machine: if crosses_boundary {
                SymbolHandle::invalid()
            } else {
                state_to_machine
                    .get(&target)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid)
            },
            arguments,
            declared,
        });
    }

    fn signature_for_symbol(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&StateSignature> {
        if let Some((_, signature)) = program.machine_parameter_signature(symbol) {
            return Some(signature);
        }
        program
            .trait_machine_signature(symbol)
            .map(|(_, signature)| signature)
    }

    fn boundary_trait_for_signature(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        let symbol = program
            .closed_requirement_application(symbol)
            .map_or(symbol, |application| application.template);
        program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .find(|definition| {
                program
                    .trait_machine_signature_by_symbol(definition, symbol)
                    .is_some()
            })
            .map(|definition| definition.symbol)
    }

    fn origin_for_expression(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<InvocationTarget> {
        if !expression.is_valid() {
            return None;
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::Name(path) => origin_for_symbol(program, machine, state, path.symbol),
            ExpressionNode::Member(member) => {
                origin_for_symbol(program, machine, state, member.member_symbol)
                    .or_else(|| origin_for_expression(program, machine, state, member.receiver))
            }
            ExpressionNode::Borrow(inner) => {
                origin_for_expression(program, machine, state, inner.target)
            }
            _ => None,
        }
    }

    fn origin_for_symbol(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        symbol: SymbolHandle,
    ) -> Option<InvocationTarget> {
        if !symbol.is_valid() {
            return None;
        }
        if let Some(parameter) = program.state_parameter_by_symbol(state, symbol) {
            let service = typed_trees::invocations::boundary_service_for_type(
                program.typed_program(),
                parameter.type_reference,
            )?;
            if let Some(index) =
                typed_trees::invocations::machine_entry_parameters(program.typed_program(), machine)
                    .iter()
                    .position(|entry| {
                        entry.symbol == parameter.symbol
                            || entry.name.as_str() == parameter.name.as_str()
                    })
            {
                return Some(InvocationTarget::Parameter(index as u32));
            }
            return Some(InvocationTarget::Service(service));
        }
        if let Some(owned) = program.machine_owned_data_by_symbol(machine, symbol) {
            return typed_trees::invocations::boundary_service_for_type(
                program.typed_program(),
                owned.type_reference,
            )
            .map(InvocationTarget::Service);
        }
        let attached = crate::declarations::data_queries::data_definition_by_symbol(
            program,
            machine.attached_data_symbol,
        );
        attached
            .and_then(|definition| {
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                            Some(field)
                        }
                        _ => None,
                    })
            })
            .and_then(|member| {
                typed_trees::invocations::boundary_service_for_type(
                    program.typed_program(),
                    member.type_reference,
                )
            })
            .map(InvocationTarget::Service)
    }

    fn substitute_target(
        target: InvocationTarget,
        arguments: &[Option<InvocationTarget>],
    ) -> Option<InvocationTarget> {
        match target {
            InvocationTarget::Parameter(index) => arguments.get(index as usize).copied().flatten(),
            InvocationTarget::Service(_) => Some(target),
        }
    }

    fn extend_targets(targets: &mut Vec<InvocationTarget>, source: &[InvocationTarget]) {
        for target in source {
            typed_trees::invocations::insert_target(targets, *target);
        }
    }

    fn extend_realized(
        targets: &mut Vec<RealizedInvocationTarget>,
        source: &[RealizedInvocationTarget],
    ) {
        let previous_length = targets.len();
        for target in source {
            if !targets.contains(target) {
                targets.push(*target);
            }
        }
        if targets.len() != previous_length {
            targets.sort_unstable_by_key(|target| match *target {
                RealizedInvocationTarget::Requirement(symbol) => {
                    (0u8, symbol.arena_index(), symbol.generation())
                }
                RealizedInvocationTarget::Abstract(InvocationTarget::Parameter(ordinal)) => {
                    (1, ordinal, 0)
                }
                RealizedInvocationTarget::Abstract(InvocationTarget::Service(symbol)) => {
                    (2, symbol.arena_index(), symbol.generation())
                }
            });
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::{InvocationTarget, RealizedInvocationTarget, SymbolHandle, extend_realized};

        #[test]
        fn realized_invocation_union_is_order_independent_without_merging_identities() {
            use RealizedInvocationTarget::{Abstract, Requirement};
            let first = SymbolHandle::from_parts(3, 1);
            let replacement = SymbolHandle::from_parts(3, 2);
            let second = SymbolHandle::from_parts(5, 1);
            let groups = [
                vec![Requirement(first), Abstract(InvocationTarget::Parameter(1))],
                vec![
                    Requirement(replacement),
                    Abstract(InvocationTarget::Service(first)),
                    Requirement(first),
                ],
                vec![
                    Abstract(InvocationTarget::Parameter(0)),
                    Requirement(second),
                ],
            ];
            let mut expected = Vec::new();
            for group in &groups {
                extend_realized(&mut expected, group);
            }
            assert_eq!(expected.len(), 6);
            for order in [[2, 1, 0], [1, 0, 2], [1, 2, 0], [0, 2, 1], [2, 0, 1]] {
                let mut observed = Vec::new();
                for index in order {
                    extend_realized(&mut observed, &groups[index]);
                }
                assert_eq!(observed, expected);
                extend_realized(&mut observed, &expected);
                assert_eq!(observed, expected);
            }
            assert!(
                groups
                    .iter()
                    .flatten()
                    .all(|target| expected.contains(target))
            );
        }
    }
}
pub(crate) use invocations::full_parameter_invocation;
#[derive(Debug, Clone, PartialEq, Eq)]
struct FamilyMemberDependency {
    machine: symbols::SymbolHandle,
    parameters: Vec<(symbols::SymbolHandle, symbols::SymbolHandle)>,
}

// Target bodies rename the same callback slots. Both effect analyses retain
// that correspondence rather than leaking a member's binder into its family.
fn family_dependencies(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<FamilyMemberDependency> {
    family_interface_members(program, machine)
        .iter()
        .map(|member| FamilyMemberDependency {
            machine: member.machine,
            parameters: program
                .machine_by_symbol(member.machine)
                .map(|member| {
                    program
                        .machine_type_parameters(member)
                        .iter()
                        .zip(program.machine_type_parameters(machine))
                        .map(|(member, interface)| (member.symbol, interface.symbol))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect()
}

pub(crate) mod operational {
    use super::ContractDependencyWork;
    use crate::CheckingInput;
    use arena::HandleSpan;
    use std::collections::HashMap;
    use symbols::JoinedPath;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, TableCallExpression};
    use typed_trees::state::State;
    use typed_trees::statement::{TableCall, TableNamePath};

    use super::call_sites::{CallSiteVisitor, visit_state_calls};

    use flow_effects::{CallOperational, MachineOperational, OperationalPlan, StateOperational};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct MachineWork {
        symbol: SymbolHandle,
        family_members: Vec<super::FamilyMemberDependency>,
        parameters: Vec<SymbolHandle>,
        uses_published_contract: bool,
        checked_generic: bool,
        template_application: Option<(
            SymbolHandle,
            HandleSpan<flow_effects::StaticMachineCallBinding>,
        )>,
        published_may_suspend: bool,
        published_may_block: bool,
        transitive_may_suspend: bool,
        transitive_may_block: bool,
        body_may_suspend: bool,
        body_may_block: bool,
        suspension_dependency: ContractDependencyWork<bool>,
        blocking_dependency: ContractDependencyWork<bool>,
        states: Vec<StateWork>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct StateWork {
        symbol: SymbolHandle,
        direct_may_suspend: bool,
        direct_may_block: bool,
        transitive_may_suspend: bool,
        transitive_may_block: bool,
        calls: Vec<CallWork>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CallWork {
        statement_index: usize,
        call_ordinal: usize,
        target_name: String,
        target_state_symbol: SymbolHandle,
        requirement_symbol: SymbolHandle,
        static_machine_parameter: SymbolHandle,
        pending_static_machine_arguments: Box<[typed_trees::expression::StaticMachineArgument]>,
        static_machine_bindings: HandleSpan<flow_effects::StaticMachineCallBinding>,
        target_machine_symbol: SymbolHandle,

        target_operator_symbol: SymbolHandle,
        direct_may_suspend: bool,
        direct_may_block: bool,
        transitive_may_suspend: bool,
        transitive_may_block: bool,
        suspension_dependency: ContractDependencyWork<bool>,
        blocking_dependency: ContractDependencyWork<bool>,
        acknowledgement: language_semantics::CallOperationalAcknowledgement,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct DirectCallOperational {
        pub may_suspend: bool,
        pub may_block: bool,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub(crate) struct OperationalDependencies {
        may_suspend: ContractDependencyWork<bool>,
        may_block: ContractDependencyWork<bool>,
    }

    pub fn infer_operational_may(program: &dyn CheckingInput) -> OperationalPlan {
        super::prepared_effects::operational_plan(program)
    }

    pub(crate) fn infer_operational_may_uncached(program: &dyn CheckingInput) -> OperationalPlan {
        infer_operational_may_for_dependencies(
            program,
            &arena::Arena::default(),
            &flow_effects::CapturedSelectorCalls::default(),
        )
    }

    pub fn infer_operational_may_for_dependencies(
        program: &dyn CheckingInput,
        dependencies: &arena::Arena<flow_effects::CleanupDependency>,
        captured: &flow_effects::CapturedSelectorCalls,
    ) -> OperationalPlan {
        let signature_effects = DirectSignatureEffects::of(program);
        let mut machines = build_machine_work(&signature_effects, program);
        let mut plan = OperationalPlan::default();
        prepare_static_bindings(program, &mut machines, &mut plan.static_machine_bindings);
        let selected_call_contexts = propagate_operational_may(
            program,
            &mut machines,
            dependencies,
            captured,
            &plan.static_machine_bindings,
        );
        let mut plan = build_plan(machines, plan);
        plan.selected_call_contexts = selected_call_contexts;
        plan.cleanup_dependencies = dependencies.clone();
        plan.captured_selector_calls = captured.clone();
        plan
    }

    fn build_machine_work(
        signature_effects: &DirectSignatureEffects,
        program: &dyn CheckingInput,
    ) -> Vec<MachineWork> {
        let mut machines = Vec::with_capacity(program.machines().len());

        for machine in program.machines() {
            let template = program
                .machine_specializations
                .iter()
                .find(|specialization| specialization.instance == machine.symbol)
                .and_then(|specialization| program.machine_by_symbol(specialization.template))
                .unwrap_or(machine);
            let checked_generic = template.supply_mode
                == language_semantics::MachineSupplyMode::CheckedBody
                && template.body_is_present
                && program
                    .machine_kind_type_parameters(template)
                    .next()
                    .is_some();
            let family_members = super::family_dependencies(program, machine);
            let uses_published_contract = family_members.is_empty()
                && (machine.is_public
                    || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody);
            let states = program
                .machine_states(machine)
                .iter()
                .map(|state| StateWork {
                    symbol: state.symbol,
                    direct_may_suspend: false,
                    direct_may_block: false,
                    transitive_may_suspend: false,
                    transitive_may_block: false,
                    calls: collect_state_calls(signature_effects, program, machine.symbol, state),
                })
                .collect();

            let published = machine_operational(program, machine);
            machines.push(MachineWork {
                symbol: machine.symbol,
                family_members,
                parameters: program
                    .machine_states(machine)
                    .first()
                    .map(|entry| {
                        program
                            .state_parameters(entry)
                            .iter()
                            .map(|parameter| parameter.symbol)
                            .collect()
                    })
                    .unwrap_or_default(),
                uses_published_contract,
                checked_generic,
                template_application: None,
                published_may_suspend: published.may_suspend,
                published_may_block: published.may_block,
                transitive_may_suspend: uses_published_contract && published.may_suspend,
                transitive_may_block: uses_published_contract && published.may_block,
                body_may_suspend: false,
                body_may_block: false,
                suspension_dependency: Default::default(),
                blocking_dependency: Default::default(),
                states,
            });
        }

        machines
    }

    fn collect_state_calls(
        signature_effects: &DirectSignatureEffects,
        program: &dyn CheckingInput,
        machine: SymbolHandle,
        state: &State,
    ) -> Vec<CallWork> {
        let mut calls = Vec::new();

        let mut sites = OperationalCallSites {
            signature_effects,
            program,
            calls: &mut calls,
        };
        visit_state_calls(program, machine, state, &mut sites);

        if let Some(owner) = program.machine_by_symbol(machine) {
            for call in &mut calls {
                if let Ok(Some(requirement)) = crate::named_conformance_target_requirement(
                    program,
                    owner,
                    call.target_state_symbol,
                ) {
                    call.target_state_symbol = requirement.signature.symbol;
                    call.requirement_symbol = requirement.signature.symbol;
                    let direct = signature_effects.for_symbol(requirement.signature.symbol);
                    call.direct_may_suspend = direct.may_suspend;
                    call.direct_may_block = direct.may_block;
                }
            }
        }

        calls
    }

    pub(crate) struct OperationalCallSites<'a> {
        signature_effects: &'a DirectSignatureEffects,
        program: &'a dyn CheckingInput,
        calls: &'a mut Vec<CallWork>,
    }

    impl CallSiteVisitor for OperationalCallSites<'_> {
        fn statement_call(&mut self, call: &TableCall, statement_index: usize, ordinal: usize) {
            let target_operator_symbol = {
                let candidates =
                    typed_trees::operator::named_statement_call_candidates(self.program, call);
                match candidates.as_slice() {
                    [operator] => operator.symbol,
                    _ => SymbolHandle::invalid(),
                }
            };
            push_call(
                self.signature_effects,
                self.program,
                call.target.as_str(),
                call.target_symbol,
                call.static_requirement_dispatch
                    .as_ref()
                    .map_or(SymbolHandle::invalid(), |dispatch| {
                        dispatch.contract_symbol()
                    }),
                call.static_machine_parameter,
                &call.machine_arguments,
                target_operator_symbol,
                call.operational_acknowledgement,
                statement_index,
                ordinal,
                self.calls,
            );
        }

        fn expression_call(
            &mut self,
            call: &TableCallExpression,
            statement_index: usize,
            ordinal: usize,
        ) {
            if crate::proof_contracts::proof_embeddings::is_exact_embed_call(self.program, call) {
                return;
            }
            let named_operator =
                typed_trees::operator::resolve_named_expression_call(self.program, call);
            let target_operator_symbol = named_operator
                .map(|operator| operator.symbol)
                .unwrap_or_else(SymbolHandle::invalid);
            let target_state_symbol = named_operator
                .filter(|operator| operator.is_boundary)
                .map_or(call.target_symbol, |operator| operator.symbol);
            push_call(
                self.signature_effects,
                self.program,
                call.target.as_str(),
                target_state_symbol,
                call.static_requirement_dispatch
                    .as_ref()
                    .map_or(SymbolHandle::invalid(), |dispatch| {
                        dispatch.contract_symbol()
                    }),
                call.static_machine_parameter,
                &call.machine_arguments,
                target_operator_symbol,
                call.operational_acknowledgement,
                statement_index,
                ordinal,
                self.calls,
            );
        }

        fn named_transfer(
            &mut self,
            target: SymbolHandle,
            path: &TableNamePath,
            _arguments: HandleSpan<ExpressionHandle>,
            static_machine_parameter: SymbolHandle,
            acknowledgement: language_semantics::CallOperationalAcknowledgement,
            statement_index: usize,
            ordinal: usize,
        ) {
            let target_name = self
                .program
                .statement_table
                .name_path_members(path.members)
                .joined_path();
            push_call(
                self.signature_effects,
                self.program,
                &target_name,
                target,
                SymbolHandle::invalid(),
                static_machine_parameter,
                &[],
                SymbolHandle::invalid(),
                acknowledgement,
                statement_index,
                ordinal,
                self.calls,
            );
        }
    }

    fn push_call(
        signature_effects: &DirectSignatureEffects,
        program: &dyn CheckingInput,
        target_name: &str,
        target_state_symbol: SymbolHandle,
        requirement_symbol: SymbolHandle,
        static_machine_parameter: SymbolHandle,
        static_machine_arguments: &[typed_trees::expression::StaticMachineArgument],
        target_operator_symbol: SymbolHandle,
        acknowledgement: language_semantics::CallOperationalAcknowledgement,
        statement_index: usize,
        call_ordinal: usize,
        calls: &mut Vec<CallWork>,
    ) {
        let authored_target = target_state_symbol;
        let target_state_symbol = program
            .machine_by_symbol(authored_target)
            .and_then(|machine| program.machine_states(machine).first())
            .map_or(authored_target, |state| state.symbol);
        let target_machine_symbol = program
            .machine_holding_state(target_state_symbol)
            .map(|machine| machine.symbol)
            .unwrap_or_else(SymbolHandle::invalid);
        let direct = program
            .retained_static_machine_contract(static_machine_parameter)
            .map(|contract| signature_operational(program, contract.signature()))
            .unwrap_or_else(|| {
                signature_effects.for_symbol(if requirement_symbol.is_valid() {
                    requirement_symbol
                } else {
                    authored_target
                })
            });
        calls.push(CallWork {
            statement_index,
            call_ordinal,
            target_name: target_name.to_owned(),
            target_state_symbol,
            requirement_symbol,
            static_machine_parameter,
            pending_static_machine_arguments: static_machine_arguments.into(),
            static_machine_bindings: HandleSpan::empty(),
            target_machine_symbol,

            target_operator_symbol,
            direct_may_suspend: direct.may_suspend,
            direct_may_block: direct.may_block,
            transitive_may_suspend: false,
            transitive_may_block: false,
            suspension_dependency: Default::default(),
            blocking_dependency: Default::default(),
            acknowledgement,
        });
    }

    pub(crate) struct DirectSignatureEffects {
        rows: Vec<(SymbolHandle, DirectCallOperational)>,
    }

    impl DirectSignatureEffects {
        fn of(program: &dyn CheckingInput) -> Self {
            let mut symbols: Vec<_> = program
                .data_type_parameters
                .iter()
                .map(|(_, parameter)| parameter.symbol)
                .chain(
                    program
                        .trait_machine_signatures
                        .iter()
                        .map(|(_, signature)| signature.symbol),
                )
                .chain(
                    program
                        .closed_requirement_applications()
                        .map(|application| application.signature.symbol),
                )
                .chain(program.dynamic_slots.iter().map(|slot| slot.symbol))
                .filter(|symbol| symbol.is_valid())
                .collect();
            symbols.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
            symbols.dedup();
            let mut parameter_signatures = program.machine_parameter_signature_queries();
            let rows = symbols
                .into_iter()
                .map(|symbol| {
                    (
                        symbol,
                        direct_operational_for_signature_symbol_uncached(
                            program,
                            symbol,
                            &mut parameter_signatures,
                        ),
                    )
                })
                .collect();
            Self { rows }
        }

        fn for_symbol(&self, symbol: SymbolHandle) -> DirectCallOperational {
            self.rows
                .binary_search_by_key(
                    &(symbol.arena_index(), symbol.generation()),
                    |(symbol, _)| (symbol.arena_index(), symbol.generation()),
                )
                .ok()
                .map(|position| self.rows[position].1)
                .unwrap_or_default()
        }
    }

    fn direct_operational_for_signature_symbol_uncached(
        program: &dyn CheckingInput,
        symbol: SymbolHandle,
        parameter_signatures: &mut typed_trees::MachineParameterSignatureQueries<'_>,
    ) -> DirectCallOperational {
        if let Some(requirement) = program
            .dynamic_slot_by_symbol(symbol)
            .and_then(|slot| program.dynamic_slot_requirement(slot))
        {
            return signature_operational(program, requirement);
        }
        if let Some((_, signature)) = parameter_signatures.get(symbol) {
            return signature_operational(program, signature);
        }

        if let Some((_, signature)) = program.trait_machine_signature(symbol) {
            return signature_operational(program, signature);
        }

        DirectCallOperational::default()
    }

    pub fn signature_operational(
        program: &dyn CheckingInput,
        signature: &typed_trees::signature::StateSignature,
    ) -> DirectCallOperational {
        let mut operational = DirectCallOperational {
            may_suspend: signature.suspends,
            may_block: signature.blocks,
        };
        super::callback_requirements::visit(
            program,
            super::callback_requirements::opaque_signature_parameters(program, signature),
            &super::declared_signature_invocations(program, signature),
            |invoked| {
                operational.may_suspend |= invoked.suspends;
                operational.may_block |= invoked.blocks;
            },
        );
        operational
    }

    pub(crate) fn machine_operational(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> DirectCallOperational {
        let mut operational = DirectCallOperational {
            may_suspend: machine.suspends,
            may_block: machine.blocks,
        };
        for governing in super::opaque_governing_ceilings(program, machine) {
            operational.may_suspend |= governing.suspends;
            operational.may_block |= governing.blocks;
        }
        if machine.supply_mode.is_boundary_declaration() && machine.has_bodyless_signature() {
            super::callback_requirements::visit(
                program,
                super::callback_requirements::opaque_machine_parameters(program, machine),
                &super::declared_machine_invocations(program, machine),
                |invoked| {
                    operational.may_suspend |= invoked.suspends;
                    operational.may_block |= invoked.blocks;
                },
            );
        }
        operational
    }

    #[derive(Clone, PartialEq, Eq)]
    pub(crate) struct OperationalContext {
        machine: usize,
        selectors: Vec<Option<Vec<SymbolHandle>>>,
        body: OperationalDependencies,
        summary: OperationalDependencies,
    }

    pub(crate) type SelectorArguments = Vec<Option<Vec<SymbolHandle>>>;

    pub(crate) struct MachineContextInputs {
        parameter_count: usize,
        initial_summary: OperationalDependencies,
        unknown_context: Option<usize>,
        selected_contexts: HashMap<SelectorArguments, usize>,
    }

    pub(crate) struct OperationalContexts {
        entries: Vec<OperationalContext>,
        machines: Vec<MachineContextInputs>,
        machine_positions: HashMap<SymbolHandle, usize>,
        checked_dependencies: Option<Vec<(bool, bool)>>,
    }

    impl OperationalContexts {
        fn new(machines: &[MachineWork]) -> Self {
            let mut machine_positions = HashMap::with_capacity(machines.len());
            let inputs = machines
                .iter()
                .enumerate()
                .map(|(position, machine)| {
                    machine_positions.entry(machine.symbol).or_insert(position);
                    MachineContextInputs {
                        parameter_count: machine.parameters.len(),
                        initial_summary: OperationalDependencies {
                            may_suspend: (machine.uses_published_contract
                                && !machine.checked_generic
                                && machine.published_may_suspend)
                                .into(),
                            may_block: (machine.uses_published_contract
                                && !machine.checked_generic
                                && machine.published_may_block)
                                .into(),
                        },
                        unknown_context: None,
                        selected_contexts: HashMap::new(),
                    }
                })
                .collect();
            Self {
                entries: Vec::with_capacity(machines.len()),
                machines: inputs,
                machine_positions,
                checked_dependencies: None,
            }
        }

        fn is_unknown(&self, position: usize) -> bool {
            self.entries[position].selectors.iter().all(Option::is_none)
        }

        fn position_of_machine(&self, symbol: SymbolHandle) -> Option<usize> {
            self.machine_positions.get(&symbol).copied()
        }

        fn intern(&mut self, machine: usize, mut selectors: SelectorArguments) -> usize {
            let inputs = &mut self.machines[machine];
            if selectors.is_empty()
                && let Some(position) = inputs.unknown_context
            {
                return position;
            }
            selectors.resize(inputs.parameter_count, None);
            let is_unknown = selectors.iter().all(Option::is_none);
            if is_unknown {
                if let Some(position) = inputs.unknown_context {
                    return position;
                }
            } else if let Some(position) = inputs.selected_contexts.get(selectors.as_slice()) {
                return *position;
            }
            let position = self.entries.len();
            if is_unknown {
                inputs.unknown_context = Some(position);
            } else {
                inputs.selected_contexts.insert(selectors.clone(), position);
            }
            self.entries.push(OperationalContext {
                machine,
                selectors,
                body: OperationalDependencies::default(),
                summary: inputs.initial_summary.clone(),
            });
            position
        }
    }

    impl std::ops::Deref for OperationalContexts {
        type Target = [OperationalContext];
        fn deref(&self) -> &Self::Target {
            &self.entries
        }
    }

    fn selector_actual(
        captured: &flow_effects::CapturedSelectorCalls,
        call: Option<&flow_effects::CapturedSelectorCall>,
        position: usize,
        owner: &MachineWork,
        selectors: &[Option<Vec<SymbolHandle>>],
    ) -> Option<Vec<SymbolHandle>> {
        let argument = captured
            .arguments
            .span_or_empty(call?.arguments)
            .iter()
            .find(|argument| argument.source_position == position)?;
        let mut implementations = Vec::new();
        for origin in captured.origins.span_or_empty(argument.origins) {
            match origin {
                flow_effects::CapturedSelectorOrigin::Unresolved => return None,
                flow_effects::CapturedSelectorOrigin::Implementation(implementation) => {
                    implementations.push(*implementation)
                }
                flow_effects::CapturedSelectorOrigin::Parameter(parameter) => {
                    let position = owner
                        .parameters
                        .iter()
                        .position(|symbol| symbol == parameter)?;
                    implementations.extend_from_slice(selectors.get(position)?.as_ref()?);
                }
            }
        }
        implementations.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        implementations.dedup();
        (!implementations.is_empty()).then_some(implementations)
    }

    pub(crate) struct ResolvedCall {
        envelope: OperationalDependencies,
        selected_dispatch: bool,
        targets: Vec<(SymbolHandle, usize)>,
    }

    #[allow(clippy::too_many_arguments)]
    fn call_envelope(
        program: &dyn CheckingInput,
        machines: &[MachineWork],
        bindings: &arena::Arena<flow_effects::StaticMachineCallBinding>,
        captured: &flow_effects::CapturedSelectorCalls,
        contexts: &mut OperationalContexts,
        owner: &MachineWork,
        selectors: &[Option<Vec<SymbolHandle>>],
        state: SymbolHandle,
        call: &CallWork,
    ) -> OperationalDependencies {
        resolve_call(
            program, machines, bindings, captured, contexts, owner, selectors, state, call,
        )
        .envelope
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_call(
        program: &dyn CheckingInput,
        machines: &[MachineWork],
        bindings: &arena::Arena<flow_effects::StaticMachineCallBinding>,
        captured: &flow_effects::CapturedSelectorCalls,
        contexts: &mut OperationalContexts,
        owner: &MachineWork,
        selectors: &[Option<Vec<SymbolHandle>>],
        state: SymbolHandle,
        call: &CallWork,
    ) -> ResolvedCall {
        let mut records = captured.calls.iter().filter_map(|(_, record)| {
            (record.caller_state == state
                && record.statement_index == call.statement_index
                && record.call_ordinal == call.call_ordinal
                && record.target == call.target_state_symbol)
                .then_some(record)
        });
        let record = records.next().filter(|_| records.next().is_none());
        let mut envelope = OperationalDependencies {
            may_suspend: (call.direct_may_suspend).into(),
            may_block: (call.direct_may_block).into(),
        };
        let parameter = if program
            .retained_static_machine_contract(call.target_state_symbol)
            .is_some()
        {
            call.target_state_symbol
        } else if !call.target_machine_symbol.is_valid() {
            call.static_machine_parameter
        } else {
            SymbolHandle::invalid()
        };
        if parameter.is_valid() {
            envelope = OperationalDependencies {
                may_suspend: ContractDependencyWork::parameter(parameter, call.direct_may_suspend),
                may_block: ContractDependencyWork::parameter(parameter, call.direct_may_block),
            };
        } else if !call.requirement_symbol.is_valid()
            && contexts
                .position_of_machine(call.target_machine_symbol)
                .is_some()
        {
            // The callee's summary already includes its published ceiling or
            // its checked callback dependency. Adding the old ceiling again
            // would erase the dependency before selections can substitute it.
            envelope = OperationalDependencies::default();
        }
        let mut selected_dispatch = false;
        let (targets, offset) = if let Some(slot) =
            program.dynamic_slot_by_symbol(call.target_state_symbol)
        {
            let selected = selector_actual(captured, record, 0, owner, selectors).and_then(
                |implementations| {
                    implementations
                        .into_iter()
                        .map(|implementation| {
                            let implementation = program.conformance_by_symbol(implementation)?;
                            implementation
                                .rows
                                .iter()
                                .find(|row| {
                                    row.declaring_trait == slot.declaring_trait
                                        && row.requirement == slot.requirement
                                })
                                .map(|row| row.realization_machine)
                        })
                        .collect::<Option<Vec<_>>>()
                },
            );
            match selected.filter(|targets| {
                !targets.is_empty()
                    && targets
                        .iter()
                        .all(|target| contexts.position_of_machine(*target).is_some())
            }) {
                Some(targets) => {
                    envelope = OperationalDependencies::default();
                    selected_dispatch = true;
                    (targets, 1)
                }
                None => (Vec::new(), 1),
            }
        } else {
            (vec![call.target_machine_symbol], 0)
        };
        let mut resolved = Vec::with_capacity(targets.len());
        for target in targets {
            let Some(machine) = contexts.position_of_machine(target) else {
                continue;
            };
            let arguments = if record.is_some() {
                (0..machines[machine].parameters.len())
                    .map(|position| {
                        selector_actual(captured, record, position + offset, owner, selectors)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let context = contexts.intern(machine, arguments);
            let summary = if call.static_machine_parameter.is_valid() {
                let published = contexts.intern(machine, Vec::new());
                selected_dependencies(machines, contexts, published)
            } else {
                contexts[context].summary.clone()
            };
            let summary = substitute_operational_dependencies(
                program,
                machines,
                contexts,
                &summary,
                call.static_machine_bindings,
                bindings,
            );
            envelope.may_suspend |= summary.may_suspend;
            envelope.may_block |= summary.may_block;
            resolved.push((target, context));
        }
        ResolvedCall {
            envelope,
            selected_dispatch,
            targets: resolved,
        }
    }

    fn export_selected_call_contexts(
        program: &dyn CheckingInput,
        machines: &[MachineWork],
        bindings: &arena::Arena<flow_effects::StaticMachineCallBinding>,
        captured: &flow_effects::CapturedSelectorCalls,
        contexts: &mut OperationalContexts,
    ) -> flow_effects::SelectedCallContexts {
        let mut resolved = Vec::new();
        let mut position = 0;
        while position < contexts.len() {
            let owner = &machines[contexts[position].machine];
            let selectors = contexts[position].selectors.clone();
            let mut calls = Vec::new();
            for state in &owner.states {
                for call in &state.calls {
                    let resolution = resolve_call(
                        program,
                        machines,
                        bindings,
                        captured,
                        contexts,
                        owner,
                        &selectors,
                        state.symbol,
                        call,
                    );
                    let known_target = resolution
                        .targets
                        .iter()
                        .any(|(_, context)| !contexts.is_unknown(*context));
                    if resolution.selected_dispatch || known_target {
                        calls.push((state.symbol, call, resolution));
                    }
                }
            }
            resolved.push(calls);
            position += 1;
        }
        let mut exported = flow_effects::SelectedCallContexts::default();
        let handles = (0..contexts.len())
            .map(|position| {
                exported.contexts.append(flow_effects::SelectedCallContext {
                    machine: machines[contexts[position].machine].symbol,
                    unknown: contexts.is_unknown(position),
                    published: machines[contexts[position].machine].uses_published_contract,
                    calls: HandleSpan::empty(),
                })
            })
            .collect::<Vec<_>>();
        for (position, calls) in resolved.into_iter().enumerate() {
            let mut span = HandleSpan::empty();
            for (state, call, resolution) in calls {
                let mut targets = HandleSpan::empty();
                for (_, context) in resolution.targets {
                    exported.targets.append_to_span(
                        &mut targets,
                        flow_effects::SelectedContextTarget {
                            context: handles[context],
                        },
                    );
                }
                exported.calls.append_to_span(
                    &mut span,
                    flow_effects::SelectedContextCall {
                        caller_state: state,
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_state_symbol: call.target_state_symbol,
                        selected_dispatch: resolution.selected_dispatch,
                        targets,
                    },
                );
            }
            exported.contexts.get_mut(handles[position]).calls = span;
        }
        exported
    }

    fn prepare_static_bindings(
        program: &dyn CheckingInput,
        machines: &mut [MachineWork],
        bindings: &mut arena::Arena<flow_effects::StaticMachineCallBinding>,
    ) {
        for machine in machines {
            machine.template_application = program
                .machine_specializations
                .iter()
                .find(|specialization| specialization.instance == machine.symbol)
                .map(|specialization| {
                    let mut rows = Vec::new();
                    for binding in program
                        .expression_table
                        .static_machine_application_bindings(specialization.binding_application)
                    {
                        let typed_trees::static_applications::StaticMachineBindingValue::Machine(
                            argument,
                        ) = &binding.value
                        else {
                            continue;
                        };
                        let nested = argument
                            .application
                            .as_ref()
                            .map_or(&[][..], |application| application.arguments.as_ref());
                        let arguments = retain_static_machine_bindings(
                            program,
                            argument.symbol,
                            nested,
                            bindings,
                        );
                        rows.push(flow_effects::StaticMachineCallBinding {
                            parameter: binding.parameter,
                            selected: argument.symbol,
                            arguments,
                        });
                    }
                    (specialization.template, bindings.insert_many(rows))
                });
            for state in &mut machine.states {
                for call in &mut state.calls {
                    let arguments = std::mem::take(&mut call.pending_static_machine_arguments);
                    call.static_machine_bindings = retain_static_machine_bindings(
                        program,
                        call.target_state_symbol,
                        &arguments,
                        bindings,
                    );
                }
            }
        }
    }

    fn published_dependency(
        published: bool,
        body: ContractDependencyWork<bool>,
        checked_dependency: bool,
    ) -> ContractDependencyWork<bool> {
        if published && checked_dependency {
            body
        } else {
            published.into()
        }
    }

    fn selected_dependencies(
        machines: &[MachineWork],
        contexts: &OperationalContexts,
        context: usize,
    ) -> OperationalDependencies {
        // A selected callback keeps its written ceiling even when its private
        // body finishes immediately. Only its checked generic dependency may
        // specialize that ceiling; runtime selector observations do not.
        let owner = &machines[contexts[context].machine];
        let template = owner
            .template_application
            .and_then(|(template, _)| contexts.position_of_machine(template))
            .unwrap_or(contexts[context].machine);
        let dependency = contexts
            .checked_dependencies
            .as_ref()
            .map_or((true, true), |dependencies| dependencies[template]);
        let summary = &contexts[context].summary;
        OperationalDependencies {
            may_suspend: if owner.published_may_suspend && !(owner.checked_generic && dependency.0)
            {
                true.into()
            } else {
                summary.may_suspend.clone()
            },
            may_block: if owner.published_may_block && !(owner.checked_generic && dependency.1) {
                true.into()
            } else {
                summary.may_block.clone()
            },
        }
    }

    fn substitute_operational_dependencies(
        program: &dyn CheckingInput,
        machines: &[MachineWork],
        contexts: &mut OperationalContexts,
        dependency: &OperationalDependencies,
        application: HandleSpan<flow_effects::StaticMachineCallBinding>,
        bindings: &arena::Arena<flow_effects::StaticMachineCallBinding>,
    ) -> OperationalDependencies {
        fn substitute(
            program: &dyn CheckingInput,
            machines: &[MachineWork],
            contexts: &mut OperationalContexts,
            dependency: &ContractDependencyWork<bool>,
            application: HandleSpan<flow_effects::StaticMachineCallBinding>,
            bindings: &arena::Arena<flow_effects::StaticMachineCallBinding>,
            suspend: bool,
        ) -> ContractDependencyWork<bool> {
            let mut result = ContractDependencyWork::from(dependency.concrete);
            for parameter in &dependency.parameters {
                let selected = bindings
                    .span_or_empty(application)
                    .iter()
                    .find(|binding| binding.parameter == *parameter)
                    .copied();
                if let Some(binding) = selected {
                    if let Some(contract) =
                        program.retained_static_machine_contract(binding.selected)
                    {
                        let signature = contract.signature();
                        result |= ContractDependencyWork::parameter(
                            binding.selected,
                            if suspend {
                                signature.suspends
                            } else {
                                signature.blocks
                            },
                        );
                    } else if let Some((machine, _)) =
                        crate::declarations::transitions::resolved_transition_target_state(
                            program,
                            binding.selected,
                        )
                        && let Some(machine) = contexts.position_of_machine(machine.symbol)
                    {
                        let selected = contexts.intern(machine, Vec::new());
                        let summary = selected_dependencies(machines, contexts, selected);
                        let dependency = if suspend {
                            &summary.may_suspend
                        } else {
                            &summary.may_block
                        };
                        let mut contribution = substitute(
                            program,
                            machines,
                            contexts,
                            dependency,
                            binding.arguments,
                            bindings,
                            suspend,
                        );
                        if contribution.parameters.iter().any(|parameter| {
                            program
                                .declaration_type_parameters(binding.selected)
                                .iter()
                                .any(|owned| owned.symbol == *parameter)
                        }) {
                            // An unapplied family keeps its published upper bound.
                            // Its method-local binders are not caller binders.
                            contribution = true.into();
                        }
                        result |= contribution;
                    } else {
                        // Keep an unresolved projection visible. A missing
                        // selection cannot establish that the call is quiet.
                        result |= ContractDependencyWork::parameter(*parameter, true);
                    }
                } else {
                    result |= ContractDependencyWork::parameter(*parameter, true);
                }
            }
            result
        }
        OperationalDependencies {
            may_suspend: substitute(
                program,
                machines,
                contexts,
                &dependency.may_suspend,
                application,
                bindings,
                true,
            ),
            may_block: substitute(
                program,
                machines,
                contexts,
                &dependency.may_block,
                application,
                bindings,
                false,
            ),
        }
    }

    fn propagate_operational_may(
        program: &dyn CheckingInput,
        machines: &mut [MachineWork],
        dependencies: &arena::Arena<flow_effects::CleanupDependency>,
        captured: &flow_effects::CapturedSelectorCalls,
        bindings: &arena::Arena<flow_effects::StaticMachineCallBinding>,
    ) -> flow_effects::SelectedCallContexts {
        let mut contexts = OperationalContexts::new(machines);
        let roots = (0..machines.len())
            .map(|machine| contexts.intern(machine, Vec::new()))
            .collect::<Vec<_>>();
        // Recursive helpers must first solve their own callback equations.
        // Seeding them with an opaque ceiling would turn A(F) = F OR A(F)
        // into an unconditional effect. Once those equations settle, axes
        // without a checked callback dependency keep their authored ceiling.
        loop {
            let previous = contexts.entries.clone();
            for position in 0..previous.len() {
                let owner = &machines[contexts[position].machine];
                let selectors = contexts[position].selectors.clone();
                let mut body = OperationalDependencies::default();
                for state in &owner.states {
                    body.may_suspend |= state.direct_may_suspend;
                    body.may_block |= state.direct_may_block;
                    for call in &state.calls {
                        let envelope = call_envelope(
                            program,
                            machines,
                            bindings,
                            captured,
                            &mut contexts,
                            owner,
                            &selectors,
                            state.symbol,
                            call,
                        );
                        body.may_suspend |= envelope.may_suspend;
                        body.may_block |= envelope.may_block;
                    }
                }
                for target in dependencies.iter().filter_map(|(_, dependency)| {
                    (dependency.caller_machine == owner.symbol).then_some(dependency.target_machine)
                }) {
                    if let Some(machine) = contexts.position_of_machine(target) {
                        let target = contexts.intern(machine, Vec::new());
                        body.may_suspend |= &contexts[target].summary.may_suspend;
                        body.may_block |= &contexts[target].summary.may_block;
                    }
                }
                let mut family = OperationalDependencies::default();
                for member in &owner.family_members {
                    if let Some(machine) = contexts.position_of_machine(member.machine) {
                        let target = contexts.intern(machine, selectors.clone());
                        let rename = |dependency: &ContractDependencyWork<bool>| {
                            let mut result = ContractDependencyWork::from(dependency.concrete);
                            for parameter in &dependency.parameters {
                                let parameter = member
                                    .parameters
                                    .iter()
                                    .find(|(source, _)| source == parameter)
                                    .map_or(*parameter, |(_, interface)| *interface);
                                result |= ContractDependencyWork::parameter(parameter, true);
                            }
                            result
                        };
                        family.may_suspend |= rename(&contexts[target].summary.may_suspend);
                        family.may_block |= rename(&contexts[target].summary.may_block);
                    }
                }
                contexts.entries[position].body = body.clone();
                contexts.entries[position].summary = if let Some((template, application)) =
                    owner.template_application
                    && let Some(template) = contexts.position_of_machine(template)
                {
                    let template = contexts.intern(template, Vec::new());
                    let template = contexts.entries[template].summary.clone();
                    substitute_operational_dependencies(
                        program,
                        machines,
                        &mut contexts,
                        &template,
                        application,
                        bindings,
                    )
                } else if owner.uses_published_contract {
                    OperationalDependencies {
                        may_suspend: published_dependency(
                            owner.published_may_suspend,
                            body.may_suspend,
                            owner.checked_generic
                                && contexts.checked_dependencies.as_ref().is_none_or(
                                    |dependencies| dependencies[contexts[position].machine].0,
                                ),
                        ),
                        may_block: published_dependency(
                            owner.published_may_block,
                            body.may_block,
                            owner.checked_generic
                                && contexts.checked_dependencies.as_ref().is_none_or(
                                    |dependencies| dependencies[contexts[position].machine].1,
                                ),
                        ),
                    }
                } else {
                    OperationalDependencies {
                        may_suspend: body.may_suspend.clone() | family.may_suspend,
                        may_block: body.may_block.clone() | family.may_block,
                    }
                };
            }
            if contexts.entries == previous {
                if contexts.checked_dependencies.is_some() {
                    break;
                }
                contexts.checked_dependencies = Some(
                    roots
                        .iter()
                        .map(|root| {
                            let summary = &contexts[*root].summary;
                            (
                                !summary.may_suspend.parameters.is_empty(),
                                !summary.may_block.parameters.is_empty(),
                            )
                        })
                        .collect(),
                );
            }
        }
        let selected_call_contexts =
            export_selected_call_contexts(program, machines, bindings, captured, &mut contexts);
        let declaration_summaries = machines
            .iter()
            .enumerate()
            .map(|(position, machine)| (machine.symbol, contexts[roots[position]].summary.clone()))
            .collect::<Vec<_>>();
        for machine_index in 0..machines.len() {
            let root = roots[machine_index];
            let summary = contexts[root].clone();
            let mut call_summaries = Vec::new();
            for state in &machines[machine_index].states {
                for call in &state.calls {
                    call_summaries.push(call_envelope(
                        program,
                        machines,
                        bindings,
                        captured,
                        &mut contexts,
                        &machines[machine_index],
                        &summary.selectors,
                        state.symbol,
                        call,
                    ));
                }
            }
            let published = selected_dependencies(machines, &contexts, root);
            let machine = &mut machines[machine_index];
            machine.body_may_suspend = summary.body.may_suspend.possible();
            machine.body_may_block = summary.body.may_block.possible();
            machine.transitive_may_suspend = summary.summary.may_suspend.possible();
            machine.transitive_may_block = summary.summary.may_block.possible();
            machine.suspension_dependency = published.may_suspend;
            machine.blocking_dependency = published.may_block;
            let mut summaries = call_summaries.into_iter();
            for state in &mut machine.states {
                state.transitive_may_suspend = state.direct_may_suspend;
                state.transitive_may_block = state.direct_may_block;
                for call in &mut state.calls {
                    if let Some(envelope) = summaries.next() {
                        call.direct_may_suspend &= envelope.may_suspend.possible();
                        call.direct_may_block &= envelope.may_block.possible();
                        call.transitive_may_suspend = envelope.may_suspend.possible();
                        call.transitive_may_block = envelope.may_block.possible();
                        call.suspension_dependency = envelope.may_suspend;
                        call.blocking_dependency = envelope.may_block;
                    }
                    state.transitive_may_suspend |= call.transitive_may_suspend;
                    state.transitive_may_block |= call.transitive_may_block;
                }
                for (_, dependency) in dependencies.iter().filter(|(_, dependency)| {
                    dependency.caller_machine == machine.symbol
                        && dependency.caller_state == state.symbol
                }) {
                    if let Some(target) =
                        declaration_summaries.iter().find_map(|(symbol, summary)| {
                            (*symbol == dependency.target_machine).then_some(summary.clone())
                        })
                    {
                        state.transitive_may_suspend |= target.may_suspend.possible();
                        state.transitive_may_block |= target.may_block.possible();
                    }
                }
            }
        }
        selected_call_contexts
    }

    fn retain_static_machine_bindings(
        program: &dyn CheckingInput,
        target: SymbolHandle,
        arguments: &[typed_trees::expression::StaticMachineArgument],
        bindings: &mut arena::Arena<flow_effects::StaticMachineCallBinding>,
    ) -> HandleSpan<flow_effects::StaticMachineCallBinding> {
        let mut result = HandleSpan::empty();
        if !target.is_valid() || arguments.is_empty() {
            return result;
        }
        let parameters = program
            .retained_static_machine_contract(target)
            .map_or_else(
                || program.declaration_type_parameters(target),
                |contract| program.state_signature_type_parameters(contract.signature()),
            )
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind,
                    typed_trees::data::TypeParameterKind::Machine { .. }
                )
            });
        let selected = arguments.iter().filter(|argument| {
            argument.symbol.is_valid()
                && matches!(
                    program.symbols.get(argument.symbol).kind,
                    symbols::SymbolKind::State | symbols::SymbolKind::MachineParameter
                )
        });
        for (parameter, selected) in parameters.zip(selected) {
            let nested = selected
                .application
                .as_ref()
                .map_or(&[][..], |application| application.arguments.as_ref());
            let arguments =
                retain_static_machine_bindings(program, selected.symbol, nested, bindings);
            bindings.append_to_span(
                &mut result,
                flow_effects::StaticMachineCallBinding {
                    parameter: parameter.symbol,
                    selected: selected.symbol,
                    arguments,
                },
            );
        }
        result
    }

    fn build_plan(machines: Vec<MachineWork>, mut plan: OperationalPlan) -> OperationalPlan {
        for machine in machines {
            let mut states = HandleSpan::empty();
            for state in machine.states {
                let mut calls = HandleSpan::empty();
                for call in state.calls {
                    let static_machine_bindings = call.static_machine_bindings;
                    let acknowledgement = if call.acknowledgement.origin
                    == language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
                {
                    language_semantics::CallOperationalAcknowledgement {
                        acknowledges_suspend: call.transitive_may_suspend,
                        acknowledges_block: call.transitive_may_block,
                        ..call.acknowledgement
                    }
                } else {
                    call.acknowledgement
                };
                    plan.calls.append_to_span(
                        &mut calls,
                        CallOperational {
                            statement_index: call.statement_index,
                            call_ordinal: call.call_ordinal,
                            target_name: call.target_name,
                            target_state_symbol: call.target_state_symbol,
                            static_machine_parameter: call.static_machine_parameter,
                            static_machine_bindings,
                            target_machine_symbol: call.target_machine_symbol,
                            target_operator_symbol: call.target_operator_symbol,
                            suspension_dependency: call
                                .suspension_dependency
                                .retain(&mut plan.dependency_parameters),
                            blocking_dependency: call
                                .blocking_dependency
                                .retain(&mut plan.dependency_parameters),
                            direct_may_suspend: call.direct_may_suspend,
                            direct_may_block: call.direct_may_block,
                            transitive_may_suspend: call.transitive_may_suspend,
                            transitive_may_block: call.transitive_may_block,
                            acknowledgement,
                        },
                    );
                }
                plan.states.append_to_span(
                    &mut states,
                    StateOperational {
                        symbol: state.symbol,
                        direct_may_suspend: state.direct_may_suspend,
                        direct_may_block: state.direct_may_block,
                        transitive_may_suspend: state.transitive_may_suspend,
                        transitive_may_block: state.transitive_may_block,
                        calls,
                    },
                );
            }
            plan.machines.append_to_span(
                &mut plan.root_machines,
                MachineOperational {
                    symbol: machine.symbol,
                    suspension_dependency: machine
                        .suspension_dependency
                        .retain(&mut plan.dependency_parameters),
                    blocking_dependency: machine
                        .blocking_dependency
                        .retain(&mut plan.dependency_parameters),
                    published_may_suspend: machine.suspension_dependency.possible(),
                    published_may_block: machine.blocking_dependency.possible(),
                    transitive_may_suspend: machine.transitive_may_suspend,
                    transitive_may_block: machine.transitive_may_block,
                    body_may_suspend: machine.body_may_suspend,
                    body_may_block: machine.body_may_block,
                    states,
                },
            );
        }

        plan
    }

    #[cfg(test)]
    mod context_tests {
        use super::*;

        fn machine(symbol: SymbolHandle, parameter_count: u32, published: bool) -> MachineWork {
            MachineWork {
                symbol,
                family_members: Vec::new(),
                parameters: (1..=parameter_count)
                    .map(|position| SymbolHandle::from_parts(position, 1))
                    .collect(),
                uses_published_contract: published,
                checked_generic: false,
                template_application: None,
                published_may_suspend: published,
                published_may_block: published,
                transitive_may_suspend: false,
                transitive_may_block: false,
                body_may_suspend: false,
                body_may_block: false,
                suspension_dependency: Default::default(),
                blocking_dependency: Default::default(),
                states: Vec::new(),
            }
        }

        #[test]
        fn operational_context_identity_preserves_arity_order_and_generations() {
            let implementation = SymbolHandle::from_parts(40, 1);
            let next_generation = SymbolHandle::from_parts(40, 2);
            let owner = SymbolHandle::from_parts(10, 1);
            let machines = [
                machine(owner, 2, false),
                machine(owner, 0, true),
                machine(SymbolHandle::from_parts(10, 2), 1, false),
            ];
            let mut contexts = OperationalContexts::new(&machines);
            assert_eq!(contexts.position_of_machine(owner), Some(0));
            assert_eq!(
                contexts.position_of_machine(SymbolHandle::from_parts(10, 2)),
                Some(2)
            );
            assert_eq!(
                contexts.position_of_machine(SymbolHandle::from_parts(11, 1)),
                None
            );
            let queries = [
                (0, vec![]),
                (0, vec![None, None]),
                (0, vec![Some(vec![implementation])]),
                (0, vec![Some(vec![implementation]), None]),
                (0, vec![Some(vec![next_generation])]),
                (0, vec![Some(vec![])]),
                (0, vec![None, Some(vec![implementation])]),
                (1, vec![Some(vec![implementation])]),
                (1, vec![]),
                (2, vec![Some(vec![implementation, next_generation])]),
                (2, vec![Some(vec![next_generation, implementation])]),
            ];
            let mut reference: Vec<(usize, SelectorArguments)> = Vec::new();
            for (machine, selectors) in queries {
                let mut normalized = selectors.clone();
                normalized.resize(machines[machine].parameters.len(), None);
                let expected = reference
                    .iter()
                    .position(|entry| *entry == (machine, normalized.clone()))
                    .unwrap_or_else(|| {
                        let position = reference.len();
                        reference.push((machine, normalized));
                        position
                    });
                assert_eq!(contexts.intern(machine, selectors), expected);
            }
            assert_eq!(contexts.len(), reference.len());
        }

        #[test]
        fn operational_context_summaries_do_not_change_selector_identity() {
            let machines = [machine(SymbolHandle::from_parts(10, 1), 1, true)];
            let mut contexts = OperationalContexts::new(&machines);
            let unknown = contexts.intern(0, Vec::new());
            assert_eq!(
                contexts[unknown].summary,
                OperationalDependencies {
                    may_suspend: (true).into(),
                    may_block: true.into()
                }
            );
            let actual = vec![Some(vec![SymbolHandle::from_parts(20, 1)])];
            let selected = contexts.intern(0, actual.clone());
            assert_ne!(unknown, selected);
            contexts.entries[selected].body = OperationalDependencies {
                may_suspend: (true).into(),
                may_block: (false).into(),
            };
            contexts.entries[selected].summary = OperationalDependencies::default();
            assert_eq!(contexts.intern(0, actual), selected);
            assert_eq!(contexts.intern(0, vec![None]), unknown);
            assert_eq!(
                contexts[unknown].summary,
                OperationalDependencies {
                    may_suspend: (true).into(),
                    may_block: true.into()
                }
            );
            assert_eq!(contexts.len(), 2);
        }
    }
}
pub(crate) mod prepared_effects {
    use flow_effects::{OperationalPlan, ServiceReachInferencePlan};

    #[derive(Debug)]
    pub struct PreparedEffectPlans {
        pub operational: OperationalPlan,
        pub service_reaches: ServiceReachInferencePlan,
    }

    pub(crate) fn with_operational_plan<Result>(
        program: &dyn crate::CheckingInput,
        query: impl FnOnce(&OperationalPlan) -> Result,
    ) -> Result {
        if let Some(plans) = program.prepared_effect_plans() {
            return query(&plans.operational);
        }
        let plan = crate::frozen_program::frozen_memo(
            program,
            |memos| &memos.operational,
            || super::operational::infer_operational_may_uncached(program),
        );
        query(&plan)
    }

    pub(crate) fn operational_plan(program: &dyn crate::CheckingInput) -> OperationalPlan {
        with_operational_plan(program, Clone::clone)
    }

    pub(crate) fn service_reach_plan(
        program: &dyn crate::CheckingInput,
        operational: &OperationalPlan,
    ) -> ServiceReachInferencePlan {
        if let Some(plans) = program.prepared_effect_plans() {
            return if &plans.operational == operational {
                plans.service_reaches.clone()
            } else {
                super::service_reach::infer_service_reaches_uncached(program, operational)
            };
        }
        let Some(memos) = program.checking_memos() else {
            return super::service_reach::infer_service_reaches_uncached(program, operational);
        };
        let (input, plan) = memos.service_reaches.get_or_init(|| {
            (
                operational.clone(),
                super::service_reach::infer_service_reaches_uncached(program, operational),
            )
        });
        if input == operational {
            plan.clone()
        } else {
            super::service_reach::infer_service_reaches_uncached(program, operational)
        }
    }
}
pub use prepared_effects::PreparedEffectPlans;
pub(crate) mod service_reach {
    use arena::{Handle, HandleSpan};
    use language_semantics::{ServiceReachId, ServiceReachInterface};
    use symbols::SymbolHandle;
    use typed_trees::declaration_index::CallTargetIndex;

    use flow_effects::{
        CallServiceReachInference, InstallationReachRequirement, MachineServiceReachInference,
        OperationalPlan, SelectedCallContext, SelectedCallContexts, ServiceReachInferencePlan,
        StateServiceReachInference,
    };

    #[derive(Debug, Clone)]
    pub(crate) struct MachineReachWork {
        symbol: SymbolHandle,
        context: Handle<SelectedCallContext>,
        family_members: Vec<FamilyMemberReach>,
        published: Vec<ServiceReachId>,
        checked_body: bool,
        authored_ceiling: bool,
        uses_published: bool,
        concrete_declared: Vec<ServiceReachId>,
        direct: Vec<ServiceReachId>,
        transitive: Vec<ServiceReachId>,
        concrete_direct: Vec<ServiceReachId>,
        concrete_transitive: Vec<ServiceReachId>,
        concrete_effective: Vec<ServiceReachId>,
        unresolved_installation_reaches: Vec<InstallationReachRequirement>,
        dependency_direct: DependencyWork,
        dependency: DependencyWork,
        cleanup_contribution: Vec<ServiceReachId>,
        calls: Vec<MachineReachCall>,
    }

    type FamilyMemberReach = super::FamilyMemberDependency;

    #[derive(Debug, Clone)]
    pub(crate) struct MachineReachCall {
        target: SymbolHandle,
        selected: Option<Vec<Handle<SelectedCallContext>>>,
        direct: DirectServiceReach,
        bindings: HandleSpan<flow_effects::StaticMachineCallBinding>,
        cleanup: CleanupContribution,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum CleanupContribution {
        Callback,
        Ordinary,
        SelectedCleanup,
    }

    type DependencyWork = super::ContractDependencyWork<Vec<ServiceReachId>>;

    #[derive(Debug, Clone, Default)]
    pub(crate) struct DirectServiceReach {
        services: Vec<ServiceReachId>,
        concrete_services: Vec<ServiceReachId>,
        unresolved_installation_reaches: Vec<InstallationReachRequirement>,
    }

    fn with_call_target_index<R>(
        program: &dyn crate::CheckingInput,
        run: impl FnOnce(&CallTargetIndex) -> R,
    ) -> R {
        program.with_call_target_index(run)
    }

    pub fn infer_service_reaches(
        program: &dyn crate::CheckingInput,
        operational: &OperationalPlan,
    ) -> ServiceReachInferencePlan {
        super::prepared_effects::service_reach_plan(program, operational)
    }

    pub(crate) fn infer_service_reaches_uncached(
        program: &dyn crate::CheckingInput,
        operational: &OperationalPlan,
    ) -> ServiceReachInferencePlan {
        let assembly_services =
            crate::machine_calls::assembly_services::AssemblyServices::new(program);
        let contexts = &operational.selected_call_contexts;
        let mut declaration_contexts = symbols::SymbolKeyMap::default();
        for (handle, context) in contexts.contexts.iter() {
            if context.unknown {
                declaration_contexts
                    .entry(context.machine)
                    .or_insert(handle);
            }
        }
        let mut work = Vec::new();
        for machine in program.machines() {
            let context = declaration_contexts
                .get(&machine.symbol)
                .copied()
                .unwrap_or_else(Handle::invalid);
            work.push(machine_reach_work(
                program,
                operational,
                machine,
                context,
                &assembly_services,
            ));
        }

        let work_index = work
            .iter()
            .enumerate()
            .map(|(index, machine)| (machine.symbol, index))
            .collect::<symbols::SymbolKeyMap<_, _>>();
        let mut context_work = vec![None; contexts.contexts.len() + 1];
        for (index, machine) in work.iter().enumerate() {
            if machine.context.is_valid() {
                set_context_work(&mut context_work, machine.context, index);
            }
        }
        for (handle, context) in contexts.contexts.iter() {
            if context.unknown
                || context.published
                || context_work_index(&context_work, handle).is_some()
            {
                continue;
            }
            let Some(machine) = work_index
                .get(&context.machine)
                .map(|index| &program.machines()[*index])
            else {
                continue;
            };
            set_context_work(&mut context_work, handle, work.len());
            work.push(machine_reach_work(
                program,
                operational,
                machine,
                handle,
                &assembly_services,
            ));
        }
        let call_targets = |call: &MachineReachCall| -> Vec<usize> {
            match &call.selected {
                Some(selected) => selected
                    .iter()
                    .filter_map(|context| {
                        context_work_index(&context_work, *context).or_else(|| {
                            work_index
                                .get(&contexts.contexts.get(*context).machine)
                                .copied()
                        })
                    })
                    .collect(),
                None => work_index.get(&call.target).copied().into_iter().collect(),
            }
        };
        loop {
            let previous = work
                .iter()
                .map(|machine| {
                    (
                        machine.transitive.clone(),
                        machine.concrete_transitive.clone(),
                        machine.unresolved_installation_reaches.clone(),
                        machine.published.clone(),
                        machine.concrete_effective.clone(),
                        machine.dependency.clone(),
                        machine.cleanup_contribution.clone(),
                    )
                })
                .collect::<Vec<_>>();
            for machine_index in 0..work.len() {
                let mut transitive = work[machine_index].direct.clone();
                let mut concrete_transitive = work[machine_index].concrete_direct.clone();
                let mut unresolved = work[machine_index].unresolved_installation_reaches.clone();
                let mut dependency = work[machine_index].dependency_direct.clone();
                let mut cleanup_contribution = Vec::new();
                for member in &work[machine_index].family_members {
                    let Some(target) = work_index.get(&member.machine).map(|index| &work[*index])
                    else {
                        continue;
                    };
                    extend_service_set(&mut transitive, effective_services(target));
                    extend_service_set(
                        &mut concrete_transitive,
                        concrete_effective_services(target),
                    );
                    extend_installation_reaches(
                        &mut unresolved,
                        &target.unresolved_installation_reaches,
                    );
                    extend_service_set(&mut dependency.concrete, &target.dependency.concrete);
                    for parameter in &target.dependency.parameters {
                        let interface_parameter = member
                            .parameters
                            .iter()
                            .find(|(member_parameter, _)| member_parameter == parameter)
                            .map_or(*parameter, |(_, interface_parameter)| *interface_parameter);
                        extend_parameter_set(&mut dependency.parameters, &[interface_parameter]);
                    }
                    extend_service_set(&mut cleanup_contribution, &target.cleanup_contribution);
                }
                for call in &work[machine_index].calls {
                    extend_service_set(&mut transitive, &call.direct.services);
                    extend_service_set(&mut concrete_transitive, &call.direct.concrete_services);
                    for target in call_targets(call).into_iter().map(|index| &work[index]) {
                        if work[machine_index].checked_body {
                            substitute_dependency(
                                program,
                                operational,
                                &work,
                                &target.dependency,
                                call.bindings,
                                &mut dependency,
                            );
                            match call.cleanup {
                                CleanupContribution::Callback => {}
                                CleanupContribution::Ordinary => extend_service_set(
                                    &mut cleanup_contribution,
                                    &target.cleanup_contribution,
                                ),
                                CleanupContribution::SelectedCleanup => extend_service_set(
                                    &mut cleanup_contribution,
                                    effective_services(target),
                                ),
                            }
                        }
                        extend_service_set(&mut transitive, effective_services(target));
                        extend_service_set(
                            &mut concrete_transitive,
                            concrete_effective_services(target),
                        );
                        extend_installation_reaches(
                            &mut unresolved,
                            &target.unresolved_installation_reaches,
                        );
                    }
                }
                work[machine_index].transitive = transitive;
                work[machine_index].concrete_transitive = concrete_transitive;
                work[machine_index].unresolved_installation_reaches = unresolved;
                work[machine_index].dependency = dependency;
                work[machine_index].cleanup_contribution = cleanup_contribution;
                let machine = &mut work[machine_index];
                if machine.checked_body && !machine.authored_ceiling {
                    machine.published = machine.transitive.clone();
                }
                machine.concrete_effective = if machine.checked_body && !machine.authored_ceiling
                    || !machine.family_members.is_empty()
                {
                    machine.concrete_transitive.clone()
                } else {
                    machine.concrete_declared.clone()
                };
            }
            if work.iter().zip(&previous).all(|(machine, previous)| {
                machine.transitive == previous.0
                    && machine.concrete_transitive == previous.1
                    && machine.unresolved_installation_reaches == previous.2
                    && machine.published == previous.3
                    && machine.concrete_effective == previous.4
                    && machine.dependency == previous.5
                    && machine.cleanup_contribution == previous.6
            }) {
                break;
            }
        }

        for machine in &mut work {
            if !machine.family_members.is_empty() {
                machine.published = machine.transitive.clone();
                machine.uses_published = true;
            }
        }

        let mut plan = ServiceReachInferencePlan {
            rows: program.service_reach_rows.clone(),
            ..Default::default()
        };
        for machine in program.machines() {
            let Some(machine_work) = work_index.get(&machine.symbol).map(|index| &work[*index])
            else {
                continue;
            };
            let machine_calls = work_index
                .get(&machine.symbol)
                .map(|index| &work[*index].calls)
                .map_or(&[][..], Vec::as_slice);
            let mut machine_call = machine_calls.iter();
            let mut states = HandleSpan::empty();
            if let Some(machine_summary) =
                with_operational_index(operational, |index| index.get(&machine.symbol).copied())
                    .map(|index| &operational.machines()[index])
            {
                for state_summary in operational.states.span_or_empty(machine_summary.states) {
                    let mut calls = HandleSpan::empty();
                    let mut state_direct = Vec::new();
                    let mut state_transitive = Vec::new();
                    let mut state_concrete_direct = Vec::new();
                    let mut state_concrete_transitive = Vec::new();
                    let mut state_unresolved = Vec::new();
                    for call_summary in operational.calls.span_or_empty(state_summary.calls) {
                        let call_work = machine_call.next().expect("operational call work");
                        let call_direct = call_work.direct.clone();
                        let mut call_transitive = call_direct.services.clone();
                        let mut call_concrete_transitive = call_direct.concrete_services.clone();
                        let mut call_unresolved = call_direct.unresolved_installation_reaches;
                        for target in call_targets(call_work)
                            .into_iter()
                            .map(|index| &work[index])
                        {
                            extend_service_set(&mut call_transitive, effective_services(target));
                            extend_service_set(
                                &mut call_concrete_transitive,
                                concrete_effective_services(target),
                            );
                            extend_installation_reaches(
                                &mut call_unresolved,
                                &target.unresolved_installation_reaches,
                            );
                        }
                        extend_service_set(&mut state_direct, &call_direct.services);
                        extend_service_set(&mut state_transitive, &call_transitive);
                        extend_service_set(
                            &mut state_concrete_direct,
                            &call_direct.concrete_services,
                        );
                        extend_service_set(
                            &mut state_concrete_transitive,
                            &call_concrete_transitive,
                        );
                        extend_installation_reaches(&mut state_unresolved, &call_unresolved);
                        let inferred_direct = plan.rows.intern(call_direct.services);
                        let inferred_transitive = plan.rows.intern(call_transitive);
                        let concrete_direct = plan.rows.intern(call_direct.concrete_services);
                        let concrete_transitive = plan.rows.intern(call_concrete_transitive);
                        plan.calls.append_to_span(
                            &mut calls,
                            CallServiceReachInference {
                                statement_index: call_summary.statement_index,
                                call_ordinal: call_summary.call_ordinal,
                                target_state: call_summary.target_state_symbol,
                                target_machine: call_summary.target_machine_symbol,
                                inferred_direct,
                                inferred_transitive,
                                concrete_direct,
                                concrete_transitive,
                                unresolved_installation_reaches: call_unresolved,
                            },
                        );
                    }
                    for (_, dependency) in
                        operational
                            .cleanup_dependencies
                            .iter()
                            .filter(|(_, dependency)| {
                                dependency.caller_machine == machine.symbol
                                    && dependency.caller_state == state_summary.symbol
                            })
                    {
                        if let Some(target) = work_index
                            .get(&dependency.target_machine)
                            .map(|index| &work[*index])
                        {
                            extend_service_set(&mut state_transitive, effective_services(target));
                            extend_service_set(
                                &mut state_concrete_transitive,
                                concrete_effective_services(target),
                            );
                            extend_installation_reaches(
                                &mut state_unresolved,
                                &target.unresolved_installation_reaches,
                            );
                        }
                    }
                    let inferred_direct = plan.rows.intern(state_direct);
                    let inferred_transitive = plan.rows.intern(state_transitive);
                    let concrete_direct = plan.rows.intern(state_concrete_direct);
                    let concrete_transitive = plan.rows.intern(state_concrete_transitive);
                    plan.states.append_to_span(
                        &mut states,
                        StateServiceReachInference {
                            state: state_summary.symbol,
                            inferred_direct,
                            inferred_transitive,
                            concrete_direct,
                            concrete_transitive,
                            unresolved_installation_reaches: state_unresolved,
                            calls,
                        },
                    );
                }
            }

            let cleanup_contribution = plan.rows.intern(machine_work.cleanup_contribution.clone());
            let published = plan.rows.intern(machine_work.published.clone());
            let inferred_direct = plan.rows.intern(machine_work.direct.clone());
            let inferred_transitive = plan.rows.intern(machine_work.transitive.clone());
            let concrete_transitive = plan.rows.intern(machine_work.concrete_transitive.clone());
            let effective = plan.rows.intern(effective_services(machine_work).to_vec());
            let concrete_effective = plan
                .rows
                .intern(concrete_effective_services(machine_work).to_vec());
            let mut parameters = HandleSpan::empty();
            for parameter in &machine_work.dependency.parameters {
                plan.dependency_parameters
                    .append_to_span(&mut parameters, *parameter);
            }
            let dependency = flow_effects::ServiceReachDependency {
                concrete: plan.rows.intern(machine_work.dependency.concrete.clone()),
                parameters,
            };
            plan.machines.append_to_span(
                &mut plan.root_machines,
                MachineServiceReachInference {
                    machine: machine.symbol,
                    dependency,
                    cleanup_contribution,
                    interface: if machine_work.uses_published {
                        ServiceReachInterface::PublishedCeiling(published)
                    } else {
                        ServiceReachInterface::InternalInferred
                    },
                    published,
                    inferred_direct,
                    inferred_transitive,
                    concrete_transitive,
                    concrete_effective,
                    unresolved_installation_reaches: machine_work
                        .unresolved_installation_reaches
                        .clone(),
                    effective,
                    states,
                },
            );
        }
        plan
    }
    fn machine_reach_work(
        program: &dyn crate::CheckingInput,
        operational: &OperationalPlan,
        machine: &typed_trees::machine::Machine,
        context: Handle<SelectedCallContext>,
        assembly_services: &crate::machine_calls::assembly_services::AssemblyServices,
    ) -> MachineReachWork {
        let contexts = &operational.selected_call_contexts;
        let family_members = super::family_dependencies(program, machine);
        let checked_body =
            machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody;
        let authored_ceiling = program
            .authored_service_reach_rows_for(machine.symbol)
            .next()
            .is_some();
        let mut published = if family_members.is_empty() {
            program
                .service_reach_rows
                .services(machine.service_reach_row)
                .to_vec()
        } else {
            Vec::new()
        };
        let mut concrete_declared = if machine.service_reach_is_installation_bound {
            Vec::new()
        } else {
            published.clone()
        };
        for governing in super::opaque_governing_ceilings(program, machine) {
            let services = program
                .service_reach_rows
                .services(governing.service_reach_row);
            extend_service_set(&mut published, services);
            if !governing.service_reach_is_installation_bound {
                extend_service_set(&mut concrete_declared, services);
            }
        }
        let mut invoked = DirectServiceReach::default();
        if machine.supply_mode.is_boundary_declaration() {
            extend_machine_invoked_binding_services(program, machine, &mut invoked);
            extend_service_set(&mut published, &invoked.services);
            extend_service_set(&mut concrete_declared, &invoked.concrete_services);
        }
        let mut calls = Vec::new();
        let mut direct = Vec::new();
        let mut concrete_direct = Vec::new();
        let mut dependency_direct = DependencyWork {
            concrete: published.clone(),
            ..Default::default()
        };
        if let Some(summary) =
            with_operational_index(operational, |index| index.get(&machine.symbol).copied())
                .map(|index| &operational.machines()[index])
        {
            for state in operational.states.span_or_empty(summary.states) {
                for call in operational.calls.span_or_empty(state.calls) {
                    let resolved = contexts.call(
                        context,
                        state.symbol,
                        call.statement_index,
                        call.call_ordinal,
                        call.target_state_symbol,
                    );
                    let selected_dispatch = resolved.is_some_and(|call| call.selected_dispatch);
                    let call_reach = if selected_dispatch {
                        DirectServiceReach::default()
                    } else {
                        direct_service_reach_for_operational_call(
                            program,
                            machine.symbol,
                            call,
                            assembly_services,
                        )
                    };
                    extend_service_set(&mut direct, &call_reach.services);
                    extend_service_set(&mut concrete_direct, &call_reach.concrete_services);
                    if checked_body
                        && !selected_dispatch
                        && !call.static_machine_parameter.is_valid()
                        && is_nominal_parameter(program, call.target_state_symbol)
                    {
                        extend_parameter_set(
                            &mut dependency_direct.parameters,
                            &[call.target_state_symbol],
                        );
                    } else if checked_body && !selected_dispatch {
                        extend_service_set(&mut dependency_direct.concrete, &call_reach.services);
                    }
                    calls.push(MachineReachCall {
                        target: call.target_machine_symbol,
                        selected: resolved.map(|call| selected_targets(contexts, call)),
                        direct: call_reach,
                        bindings: call.static_machine_bindings,
                        cleanup: if call.static_machine_parameter.is_valid()
                            || is_nominal_parameter(program, call.target_state_symbol)
                        {
                            CleanupContribution::Callback
                        } else {
                            CleanupContribution::Ordinary
                        },
                    });
                }
            }
        }
        for (_, dependency) in operational
            .cleanup_dependencies
            .iter()
            .filter(|(_, dependency)| dependency.caller_machine == machine.symbol)
        {
            calls.push(MachineReachCall {
                target: dependency.target_machine,
                selected: None,
                direct: DirectServiceReach::default(),
                bindings: HandleSpan::empty(),
                cleanup: CleanupContribution::SelectedCleanup,
            });
        }
        let mut work =
            MachineReachWork {
                symbol: machine.symbol,
                context,
                uses_published: family_members.is_empty()
                    && (machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                        || machine.is_public
                        || authored_ceiling
                        || !program
                            .service_reach_rows
                            .services(machine.service_reach_row)
                            .is_empty()),
                family_members,
                published: published.clone(),
                checked_body,
                authored_ceiling,
                concrete_declared: concrete_declared.clone(),
                direct: direct.clone(),
                transitive: direct,
                concrete_direct: concrete_direct.clone(),
                concrete_transitive: concrete_direct,
                concrete_effective: concrete_declared,
                unresolved_installation_reaches: machine
                    .service_reach_is_installation_bound
                    .then_some(InstallationReachRequirement {
                        requirement: machine.symbol,
                        upper_bound: machine.service_reach_row,
                    })
                    .into_iter()
                    .chain(invoked.unresolved_installation_reaches)
                    .chain(calls.iter().flat_map(|call| {
                        call.direct.unresolved_installation_reaches.iter().copied()
                    }))
                    .collect(),
                cleanup_contribution: Vec::new(),
                dependency: dependency_direct.clone(),
                dependency_direct,
                calls,
            };
        normalize_installation_reaches(&mut work.unresolved_installation_reaches);
        work
    }

    fn selected_targets(
        contexts: &SelectedCallContexts,
        call: &flow_effects::SelectedContextCall,
    ) -> Vec<Handle<SelectedCallContext>> {
        contexts
            .targets
            .span_or_empty(call.targets)
            .iter()
            .map(|target| target.context)
            .collect()
    }

    fn set_context_work(
        context_work: &mut Vec<Option<usize>>,
        context: Handle<SelectedCallContext>,
        work: usize,
    ) {
        let slot = context.arena_index() as usize;
        if context_work.len() <= slot {
            context_work.resize(slot + 1, None);
        }
        context_work[slot] = Some(work);
    }

    fn context_work_index(
        context_work: &[Option<usize>],
        context: Handle<SelectedCallContext>,
    ) -> Option<usize> {
        context_work
            .get(context.arena_index() as usize)
            .copied()
            .flatten()
    }

    fn effective_services(machine: &MachineReachWork) -> &[ServiceReachId] {
        if machine.uses_published {
            &machine.published
        } else {
            &machine.transitive
        }
    }

    fn concrete_effective_services(machine: &MachineReachWork) -> &[ServiceReachId] {
        &machine.concrete_effective
    }

    fn with_operational_index<R>(
        operational: &OperationalPlan,
        run: impl FnOnce(&symbols::SymbolKeyMap<SymbolHandle, usize>) -> R,
    ) -> R {
        operational.with_machine_index(run)
    }

    fn indexed_contract_view<'program>(
        program: &'program dyn crate::CheckingInput,
        index: &CallTargetIndex,
        contract: &'program typed_trees::data::MachineParameterContract,
    ) -> Option<typed_trees::data::MachineParameterContractView<'program>> {
        match contract {
            typed_trees::data::MachineParameterContract::RequirementIdentity => None,
            typed_trees::data::MachineParameterContract::Structural(signature) => Some(
                typed_trees::data::MachineParameterContractView::Structural(signature),
            ),
            typed_trees::data::MachineParameterContract::Nominal {
                trait_definition,
                requirement,
                instantiated,
                ..
            } => {
                let trait_definition =
                    &program.traits()[*index.trait_by_symbol.get(trait_definition)? as usize];
                let declared = &program.trait_machine_signatures(trait_definition)
                    [index.trait_signature.get(requirement)?.1 as usize];
                let requirement = instantiated.as_deref().unwrap_or(declared);
                Some(typed_trees::data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement,
                })
            }
        }
    }

    fn retained_contract_view<'program>(
        program: &'program dyn crate::CheckingInput,
        index: &CallTargetIndex,
        symbol: SymbolHandle,
    ) -> Option<typed_trees::data::MachineParameterContractView<'program>> {
        if !symbol.is_valid() {
            return None;
        }
        let handle = index.parameter_contract.get(&symbol)?;
        let parameter = program.data_type_parameters.get(*handle);
        let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
            return None;
        };
        indexed_contract_view(program, index, contract)
    }

    fn direct_service_reach_for_operational_call(
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        call: &flow_effects::CallOperational,
        assembly_services: &crate::machine_calls::assembly_services::AssemblyServices,
    ) -> DirectServiceReach {
        if let Some(function) = program
            .symbols
            .builtin_function_for_symbol(call.target_state_symbol)
            && function.is_asm_intrinsic()
        {
            let mut reach = DirectServiceReach::default();
            if let Some(service_name) = function.asm_intrinsic_service_name()
                && let Ok(service) = assembly_services.resolve(program, owner, service_name)
            {
                program
                    .service_reaches
                    .extend_closure(service, &mut reach.services);
                reach.services.sort_by_key(|service| service.0);
                reach.services.dedup();
                reach.concrete_services = reach.services.clone();
            }
            return reach;
        }
        let mut reach = direct_service_reach_for_call(program, call.target_state_symbol);
        if let Some(typed_trees::data::MachineParameterContractView::Structural(signature)) =
            with_call_target_index(program, |index| {
                retained_contract_view(program, index, call.static_machine_parameter)
            })
        {
            extend_signature_service_rows(program, signature, &mut reach);
            record_signature_reach(program, signature, &mut reach);
        }
        reach
    }

    fn is_nominal_parameter(program: &dyn crate::CheckingInput, parameter: SymbolHandle) -> bool {
        with_call_target_index(program, |index| {
            matches!(
                retained_contract_view(program, index, parameter),
                Some(typed_trees::data::MachineParameterContractView::Nominal { .. })
            )
        })
    }

    fn extend_parameter_set(destination: &mut Vec<SymbolHandle>, source: &[SymbolHandle]) {
        destination.extend_from_slice(source);
        destination.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        destination.dedup();
    }

    fn substitute_dependency(
        program: &dyn crate::CheckingInput,
        operational: &OperationalPlan,
        machines: &[MachineReachWork],
        dependency: &DependencyWork,
        bindings: HandleSpan<flow_effects::StaticMachineCallBinding>,
        result: &mut DependencyWork,
    ) {
        extend_service_set(&mut result.concrete, &dependency.concrete);
        for parameter in &dependency.parameters {
            let Some(binding) = operational
                .static_machine_bindings
                .span_or_empty(bindings)
                .iter()
                .find(|binding| binding.parameter == *parameter)
            else {
                extend_parameter_set(&mut result.parameters, &[*parameter]);
                continue;
            };
            if is_nominal_parameter(program, binding.selected) {
                extend_parameter_set(&mut result.parameters, &[binding.selected]);
            } else if let Some((selected, _)) =
                crate::declarations::transitions::resolved_transition_target_state(
                    program,
                    binding.selected,
                )
                && let Some(selected) = machines
                    .iter()
                    .find(|machine| machine.symbol == selected.symbol)
            {
                substitute_dependency(
                    program,
                    operational,
                    machines,
                    &selected.dependency,
                    binding.arguments,
                    result,
                );
            } else {
                extend_service_set(
                    &mut result.concrete,
                    &direct_service_reach_for_call(program, binding.selected).services,
                );
            }
        }
    }

    pub fn fixed_installation_boundary_service_reach(
        program: &dyn crate::CheckingInput,
        target: SymbolHandle,
    ) -> Option<Vec<ServiceReachId>> {
        let trait_requirements = program
            .traits()
            .iter()
            .flat_map(|owner| program.trait_machine_instances(owner))
            .filter(|signature| signature.symbol == target)
            .map(|signature| signature.service_reach_is_installation_bound);
        let machine_requirements = program
            .machines()
            .iter()
            .filter(|machine| {
                !machine.supplies_checked_body()
                    && typed_trees::state::state_by_symbol(program.machine_states(machine), target)
                        .is_some()
            })
            .map(|machine| machine.service_reach_is_installation_bound);
        let operator_requirements = program
            .operators()
            .iter()
            .filter(|operator| operator.is_boundary && operator.symbol == target)
            .map(|_| false);
        let mut requirements = trait_requirements
            .chain(machine_requirements)
            .chain(operator_requirements);
        let installation_bound = requirements.next()?;
        if requirements.next().is_some() {
            return None;
        }
        if !installation_bound {
            return Some(Vec::new());
        }
        Some(direct_service_reach_for_call(program, target).concrete_services)
    }

    pub fn trait_signature_service_reach(
        program: &dyn crate::CheckingInput,
        signature: SymbolHandle,
    ) -> Option<Vec<ServiceReachId>> {
        let mut declarations = program
            .traits()
            .iter()
            .flat_map(|owner| program.trait_machine_instances(owner))
            .filter(|declaration| declaration.symbol == signature);
        let signature = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        Some(signature_service_reach(program, signature))
    }

    pub(crate) fn signature_service_reach(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
    ) -> Vec<ServiceReachId> {
        let mut reach = DirectServiceReach::default();
        extend_signature_service_rows(program, signature, &mut reach);
        record_signature_reach(program, signature, &mut reach);
        reach.services
    }

    fn direct_service_reach_for_call(
        program: &dyn crate::CheckingInput,
        target: SymbolHandle,
    ) -> DirectServiceReach {
        let intrinsic = crate::exact_compiler_intrinsic_boundary_requirement(program, target);
        if intrinsic.is_none()
            && let Some((requirement, _)) =
                crate::exact_external_leaf_boundary_requirement(program, target)
        {
            return direct_service_reach_for_call(program, requirement);
        }
        let target = intrinsic.map_or(target, |(requirement, _)| requirement);
        let target = program
            .dynamic_slot_by_symbol(target)
            .map_or(target, |slot| slot.requirement);
        let mut reach = DirectServiceReach::default();
        if !target.is_valid() {
            return reach;
        }

        let parameter_signature = with_call_target_index(program, |index| {
            index
                .parameter_signature
                .get(&target)
                .and_then(|&(machine_index, parameter_index)| {
                    let machine = &program.machines()[machine_index as usize];
                    let parameter =
                        &program.machine_type_parameters(machine)[parameter_index as usize];
                    let typed_trees::data::TypeParameterKind::Machine { contract } =
                        &parameter.kind
                    else {
                        return None;
                    };
                    indexed_contract_view(program, index, contract)
                        .map(typed_trees::data::MachineParameterContractView::signature)
                        .map(|signature| (machine, signature))
                })
        });
        if let Some((_, signature)) = parameter_signature {
            extend_signature_service_rows(program, signature, &mut reach);
            record_signature_reach(program, signature, &mut reach);
            return reach;
        }

        if let Some((_, signature)) = program.trait_machine_signature(target) {
            extend_signature_service_rows(program, signature, &mut reach);
            record_signature_reach(program, signature, &mut reach);
            return reach;
        }
        if let Some(machine) = with_call_target_index(program, |index| {
            index.boundary_machine_by_state.get(&target).copied()
        })
        .map(|machine_index| &program.machines()[machine_index as usize])
        {
            extend_service_set(
                &mut reach.services,
                program
                    .service_reach_rows
                    .services(machine.service_reach_row),
            );
            if !machine.service_reach_is_installation_bound {
                reach.concrete_services = reach.services.clone();
            }
            extend_machine_invoked_binding_services(program, machine, &mut reach);
        }
        reach
    }

    fn extend_signature_service_rows(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        reach: &mut DirectServiceReach,
    ) {
        extend_service_set(
            &mut reach.services,
            program
                .service_reach_rows
                .services(signature.service_reach_row),
        );
        if !signature.service_reach_is_installation_bound {
            extend_service_set(
                &mut reach.concrete_services,
                program
                    .service_reach_rows
                    .services(signature.service_reach_row),
            );
        }
    }

    fn record_signature_reach(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        reach: &mut DirectServiceReach,
    ) {
        record_installation_reach(signature, reach);
        extend_invoked_binding_services(program, signature, reach);
    }

    fn record_installation_reach(
        signature: &typed_trees::signature::StateSignature,
        reach: &mut DirectServiceReach,
    ) {
        if signature.service_reach_is_installation_bound {
            reach
                .unresolved_installation_reaches
                .push(InstallationReachRequirement {
                    requirement: signature.symbol,
                    upper_bound: signature.service_reach_row,
                });
        }
    }

    fn extend_installation_reaches(
        destination: &mut Vec<InstallationReachRequirement>,
        source: &[InstallationReachRequirement],
    ) {
        destination.extend_from_slice(source);
        normalize_installation_reaches(destination);
    }

    fn normalize_installation_reaches(rows: &mut Vec<InstallationReachRequirement>) {
        rows.sort_by_key(|row| {
            (
                row.requirement.arena_index(),
                row.requirement.generation(),
                row.upper_bound.0,
            )
        });
        rows.dedup();
    }

    fn extend_invoked_binding_services(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        reach: &mut DirectServiceReach,
    ) {
        extend_invoked_callback_services(
            program,
            super::callback_requirements::opaque_signature_parameters(program, signature),
            &super::declared_signature_invocations(program, signature),
            reach,
        );
    }

    fn extend_machine_invoked_binding_services(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        reach: &mut DirectServiceReach,
    ) {
        if !machine.has_bodyless_signature() {
            return;
        }
        extend_invoked_callback_services(
            program,
            super::callback_requirements::opaque_machine_parameters(program, machine),
            &super::declared_machine_invocations(program, machine),
            reach,
        );
    }

    fn extend_invoked_callback_services(
        program: &dyn crate::CheckingInput,
        parameters: &[typed_trees::data::TypeParameter],
        invocations: &[flow_effects::InvocationTarget],
        reach: &mut DirectServiceReach,
    ) {
        super::callback_requirements::visit(program, parameters, invocations, |signature| {
            extend_signature_service_rows(program, signature, reach);
            record_installation_reach(signature, reach);
        });
    }

    fn extend_service_set(destination: &mut Vec<ServiceReachId>, source: &[ServiceReachId]) {
        destination.extend_from_slice(source);
        destination.sort_by_key(|service| service.0);
        destination.dedup();
    }
}

pub use invocations::{
    declared_machine_invocations, declared_signature_invocations,
    has_self_forwarded_boundary_parameter, infer_synchronous_invocations, invocation_target_label,
};
pub use operational::{DirectCallOperational, signature_operational};
pub use operational::{infer_operational_may, infer_operational_may_for_dependencies};
pub(crate) use service_reach::signature_service_reach;
pub use service_reach::{
    fixed_installation_boundary_service_reach, infer_service_reaches, trait_signature_service_reach,
};

pub(crate) struct GoverningCeiling {
    suspends: bool,
    blocks: bool,
    service_reach_row: language_semantics::ServiceReachRowId,
    service_reach_is_installation_bound: bool,
}

fn opaque_governing_ceilings(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<GoverningCeiling> {
    if machine.body_is_present
        || !matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::ExternalRealization { .. }
        )
    {
        return Vec::new();
    }
    program
        .machine_trait_conformances(machine)
        .iter()
        .filter_map(|conformance| {
            match typed_trees::machine::resolve_satisfied_declaration(
                program.typed_program(),
                machine,
                conformance,
            )? {
                typed_trees::machine::SatisfiedDeclaration::Trait { requirement, .. } => {
                    Some(GoverningCeiling {
                        suspends: requirement.suspends,
                        blocks: requirement.blocks,
                        service_reach_row: requirement.service_reach_row,
                        service_reach_is_installation_bound: requirement
                            .service_reach_is_installation_bound,
                    })
                }
                typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(requirement) => {
                    Some(GoverningCeiling {
                        suspends: requirement.suspends,
                        blocks: requirement.blocks,
                        service_reach_row: requirement.service_reach_row,
                        service_reach_is_installation_bound: requirement
                            .service_reach_is_installation_bound,
                    })
                }
                typed_trees::machine::SatisfiedDeclaration::Operator(_) => None,
            }
        })
        .collect()
}

fn family_interface_members<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> &'program [typed_trees::machine::TargetMachineFamilyMember] {
    if machine.body_is_present {
        return &[];
    }
    program
        .target_machine_families
        .iter()
        .find(|family| family.canonical == machine.symbol)
        .map_or(&[], |family| family.members.as_slice())
}
