/*
    Proof-plan obligation checking, integer and floating-point range derivation,
    transition and assignment guard narrowing, arrival stability, and
    `proof-admission` certificate synthesis.

    We decide every bounded assignment, call argument, initializer, transition
    argument, and state-return obligation in a `ProofPlan`, emitting `ProofNode`
    certificates that the `proof-admission` kernel independently re-decides and
    caching accepted derivations by semantic obligation identity.
*/

mod arrival_stability {
    use crate::checker::assignment_stability::resolved_writes_overlap_reads;
    use crate::obligations::ProofPlan;
    use crate::typed_expressions::statement_table;
    use typed_trees::statement::StatementNode;

    pub(super) struct ArrivalPrefix<'program> {
        written_paths: Vec<String>,
        local_definitions: Vec<&'program str>,
    }

    impl ArrivalPrefix<'_> {
        pub(super) fn preserves_reads(
            &self,
            premise_reads: &[Vec<String>],
            value_reads: &[Vec<String>],
        ) -> bool {
            !resolved_writes_overlap_reads(&self.written_paths, value_reads)
                && !self.local_definitions.iter().any(|name| {
                    premise_reads
                        .iter()
                        .any(|read| read.first().is_none_or(|root| root == name))
                })
        }
    }

    pub(super) fn prefix_preserves_reads<'program>(
        proof_plan: &ProofPlan<'program>,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
        statement_index: usize,
        premise_reads: &[Vec<String>],
        value_reads: &[Vec<String>],
        call_frames: &validation::CallFrameResolver<'program>,
    ) -> bool {
        prepare_prefix(proof_plan, machine, state, statement_index, call_frames)
            .is_some_and(|prefix| prefix.preserves_reads(premise_reads, value_reads))
    }

    pub(super) fn prepare_prefix<'program>(
        proof_plan: &ProofPlan<'program>,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
        statement_index: usize,
        call_frames: &validation::CallFrameResolver<'program>,
    ) -> Option<ArrivalPrefix<'program>> {
        let statements = statement_table(proof_plan.program).statements(state.statement_nodes);
        if statement_index >= statements.len() {
            return None;
        }
        let mut prefix = ArrivalPrefix {
            written_paths: Vec::new(),
            local_definitions: Vec::new(),
        };
        for (index, statement) in statements.iter().enumerate().take(statement_index + 1) {
            prefix
                .written_paths
                .extend(call_frames.statement_value_may_write_paths(machine, statement)?);
            if let Some(call) = statement.as_call() {
                prefix
                    .written_paths
                    .extend(call_frames.may_write_paths(machine, call)?);
            }
            if index == statement_index {
                return Some(prefix);
            }
            match statement {
                StatementNode::Assignment(_) => {
                    prefix.written_paths.extend(
                        call_frames
                            .assignment_write_frame(machine, statement)
                            .into_complete_paths()?,
                    );
                }
                StatementNode::LocalData(local) => {
                    prefix.local_definitions.push(local.name.as_str());
                }
                StatementNode::Call(_) | StatementNode::Expression(_) => {}
                _ => return None,
            }
        }
        None
    }

    #[cfg(test)]
    mod tests {
        use super::ArrivalPrefix;

        fn path(members: &[&str]) -> Vec<String> {
            members.iter().map(|member| (*member).to_owned()).collect()
        }

        #[test]
        fn local_definition_can_supply_a_value_but_cannot_shadow_a_premise() {
            let prefix = ArrivalPrefix {
                written_paths: Vec::new(),
                local_definitions: vec!["operand"],
            };
            let operand = [path(&["operand"])];
            assert!(prefix.preserves_reads(&[], &operand));
            assert!(!prefix.preserves_reads(&operand, &operand));
            assert!(!prefix.preserves_reads(&[Vec::new()], &[]));
        }

        #[test]
        fn prepared_prefix_preserves_parent_child_alias_rejection() {
            let prefix = ArrivalPrefix {
                written_paths: vec!["self.record.value".into()],
                local_definitions: Vec::new(),
            };
            assert!(!prefix.preserves_reads(&[], &[path(&["self", "record"])]));
            assert!(!prefix.preserves_reads(&[], &[path(&["self", "record", "value"])]));
            assert!(prefix.preserves_reads(&[], &[path(&["self", "record", "other"])]));
        }
    }
}
mod assignment_stability {
    use crate::checker::AssignmentRangeContext;
    use crate::checker::arrival_stability;
    use crate::checker::guards::unwrap_true_guard_condition;
    use crate::obligations::{BoundedAssignmentObligation, ProofPlan};
    use crate::typed_expressions::expression_table;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::statement::StatementNode;

    pub(crate) fn collect_stable_assignment_conditions(
        proof_plan: &ProofPlan,
        obligation: &BoundedAssignmentObligation,
        condition: ExpressionHandle,
        context: &AssignmentRangeContext<'_>,
        conditions: &mut Vec<ExpressionHandle>,
    ) {
        let Some(prefix) = prepare_assignment_prefix(proof_plan, obligation, context) else {
            return;
        };
        collect_stable_conditions(proof_plan, condition, &prefix, conditions);
    }

    fn collect_stable_conditions(
        proof_plan: &ProofPlan<'_>,
        condition: ExpressionHandle,
        prefix: &arrival_stability::ArrivalPrefix<'_>,
        conditions: &mut Vec<ExpressionHandle>,
    ) {
        let unwrapped = unwrap_true_guard_condition(proof_plan, condition);
        if unwrapped != condition {
            collect_stable_conditions(proof_plan, unwrapped, prefix, conditions);
            return;
        }
        if let Some(binary) = proof_plan.program.expression_table.binary_node(condition)
            && binary.operator == BinaryOperator::And
        {
            collect_stable_conditions(proof_plan, binary.left, prefix, conditions);
            collect_stable_conditions(proof_plan, binary.right, prefix, conditions);
        } else {
            let mut reads = Vec::new();
            collect_read_place_paths(proof_plan, condition, &mut reads);
            if prefix.preserves_reads(&reads, &reads) {
                conditions.push(condition);
            }
        }
    }

    fn prepare_assignment_prefix<'query>(
        proof_plan: &'query ProofPlan<'_>,
        obligation: &BoundedAssignmentObligation,
        context: &'query AssignmentRangeContext<'_>,
    ) -> Option<arrival_stability::ArrivalPrefix<'query>> {
        let program = proof_plan.program;
        let (machine, state) =
            super::machine_and_state(program, obligation.machine_symbol, obligation.state_symbol)?;

        let mut read_paths: Vec<Vec<String>> = Vec::new();
        collect_read_place_paths(proof_plan, obligation.value, &mut read_paths);
        if let Some(operands) = &obligation.binary_operands {
            collect_read_place_paths(proof_plan, operands.left, &mut read_paths);
            collect_read_place_paths(proof_plan, operands.right, &mut read_paths);
        }
        let call_frames = context.call_frames()?;

        let statements = program.statement_table.statements(state.statement_nodes);
        if !matches!(statements.get(obligation.statement_index),
        Some(StatementNode::Assignment(assignment))
            if assignment.target == obligation.target && assignment.value == obligation.value)
        {
            return None;
        }
        let prefix = arrival_stability::prepare_prefix(
            proof_plan,
            machine,
            state,
            obligation.statement_index,
            call_frames,
        )?;
        prefix.preserves_reads(&[], &read_paths).then_some(prefix)
    }

    pub(crate) fn resolved_writes_overlap_reads(written: &[String], reads: &[Vec<String>]) -> bool {
        reads.iter().any(|read| {
            let read = read.join(".");
            written
                .iter()
                .any(|write| validation::frame_paths_overlap(&read, write))
        })
    }

    pub(crate) fn written_place_path(
        proof_plan: &ProofPlan,
        expression: ExpressionHandle,
    ) -> Option<Vec<String>> {
        match proof_plan.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner) => written_place_path(proof_plan, inner.target),
            ExpressionNode::Indexed(indexed) => written_place_path(proof_plan, indexed.collection),
            ExpressionNode::Member(member) => {
                let mut path = written_place_path(proof_plan, member.receiver)?;
                path.push(member.member.as_str().to_owned());
                Some(path)
            }
            ExpressionNode::Name(path) => Some(
                expression_table(proof_plan.program)
                    .name_path_members(path.members)
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect(),
            ),
            _ => None,
        }
    }

    pub(crate) fn collect_read_place_paths(
        proof_plan: &ProofPlan,
        expression: ExpressionHandle,
        paths: &mut Vec<Vec<String>>,
    ) {
        if !expression.is_valid() {
            return;
        }
        match proof_plan.program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                expression_table(proof_plan.program).match_deep_all(dispatch, |handle| {
                    collect_read_place_paths(proof_plan, handle, paths);
                    true
                });
            }
            ExpressionNode::Binary(binary) => {
                collect_read_place_paths(proof_plan, binary.left, paths);
                collect_read_place_paths(proof_plan, binary.right, paths);
            }
            ExpressionNode::Unary(unary) => {
                collect_read_place_paths(proof_plan, unary.operand, paths);
            }
            ExpressionNode::Cast(cast) => collect_read_place_paths(proof_plan, cast.value, paths),
            ExpressionNode::Borrow(inner) => {
                collect_read_place_paths(proof_plan, inner.target, paths)
            }
            ExpressionNode::Indexed(indexed) => {
                collect_read_place_paths(proof_plan, indexed.collection, paths);
                collect_read_place_paths(proof_plan, indexed.index, paths);
            }
            ExpressionNode::Member(_) | ExpressionNode::Name(_) => {
                if let Some(path) = written_place_path(proof_plan, expression) {
                    paths.push(path);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn member_paths_may_alias(left: &[String], right: &[String]) -> bool {
        let shared = left.len().min(right.len());
        left[..shared] == right[..shared]
    }
}
mod bounded_checks {
    use crate::checker::AssignmentRangeContext;
    use crate::checker::assignment_stability::{
        collect_read_place_paths, member_paths_may_alias, written_place_path,
    };
    use crate::checker::certificate::{
        CertificateVerdict, bounded_integer_value_verdict,
        guarded_transition_integer_verdict_measured, state_return_integer_verdict,
    };

    use crate::checker::derivation_cache::DerivationConsultation;
    use crate::checker::diagnostics::{
        cannot_prove_bounded_assignment_float, cannot_prove_bounded_assignment_integer,
        cannot_prove_bounded_call_float, cannot_prove_bounded_call_integer,
        cannot_prove_bounded_initializer_float, cannot_prove_bounded_initializer_integer,
        cannot_prove_bounded_return_float, cannot_prove_bounded_return_integer,
        cannot_prove_bounded_transition_float, cannot_prove_bounded_transition_integer,
    };
    use crate::checker::float_ranges::{
        float_range_for_assignment, float_range_for_call_argument, float_range_for_initializer,
        float_range_for_return_value, float_range_from_constraints,
        guarded_float_range_for_transition_argument,
    };
    use crate::checker::integer_ranges::{
        guarded_integer_range_for_assignment_with_context,
        guarded_integer_range_for_transition_argument, integer_range_for_call_argument,
        integer_range_for_initializer, integer_range_from_constraints, type_constraints,
    };
    use crate::checker::measurement::ProofPlanMeasurements;
    use crate::checker::named_constraints::{
        check_assignment_named_constraints, check_call_named_constraints,
        check_initializer_named_constraints, check_return_named_constraints,
        check_transition_named_constraints,
    };
    use crate::checker::return_arrival::integer_range;
    use crate::obligations::{
        BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
        BoundedStateReturnObligation, BoundedTransitionArgumentObligation, ProofPlan,
    };
    use crate::obligations::{FloatRange, IntegerRange, ProofConstraint};
    use crate::typed_expressions::expression_contains_call_node;
    use diagnostics::Diagnostic;
    use numerics::bignum::BigInt;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::state::state_by_symbol;
    use typed_trees::statement::StatementNode;

    pub(crate) fn check_bounded_assignment<'program>(
        proof_plan: &ProofPlan<'program>,
        obligation: &BoundedAssignmentObligation,
        context: &AssignmentRangeContext<'program>,
        seed: u64,
        diagnostics: &mut Vec<Diagnostic>,
        _recipe_obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
        measurements: &mut ProofPlanMeasurements,
        derivations: Option<&mut DerivationConsultation<'_>>,
    ) -> Option<IntegerRange> {
        if assignment_is_overwritten_before_consumption(proof_plan, obligation) {
            return None;
        }

        let diagnostics_before = diagnostics.len();
        let mut checked_range = None;
        check_assignment_named_constraints(proof_plan, obligation, diagnostics);

        if let Some(target_range) =
            integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
        {
            checked_range = Some(target_range.clone());
            match bounded_integer_value_verdict(
                proof_plan,
                obligation.value,
                obligation.value_constraints,
                obligation.base_type,
                &target_range,
                seed,
                false,
                measurements,
                derivations,
            ) {
                CertificateVerdict::Certified => {}
                CertificateVerdict::Rejected => {
                    diagnostics.push(cannot_prove_bounded_assignment_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                }
                CertificateVerdict::Uncovered => {
                    let interval_fits = guarded_integer_range_for_assignment_with_context(
                        proof_plan, obligation, context,
                    )
                    .is_some_and(|range| {
                        range.minimum >= target_range.minimum
                            && range.maximum <= target_range.maximum
                    });
                    if !interval_fits
                        && !validation::occurrence_integer_expression_fits_range(
                            proof_plan.program,
                            obligation.machine_symbol,
                            obligation.state_symbol,
                            obligation.statement_index,
                            obligation.value,
                            &target_range.minimum,
                            &target_range.maximum,
                        )
                    {
                        diagnostics.push(cannot_prove_bounded_assignment_integer(
                            proof_plan,
                            obligation,
                            target_range,
                        ));
                    }
                }
            }
        }

        if discharge_declared_float_range(
            proof_plan,
            obligation,
            obligation.constraints,
            diagnostics,
            float_range_for_assignment,
            cannot_prove_bounded_assignment_float,
        ) {
            return None;
        }

        if diagnostics.len() == diagnostics_before {
            checked_range
        } else {
            None
        }
    }

    fn assignment_is_overwritten_before_consumption(
        proof_plan: &ProofPlan,
        obligation: &BoundedAssignmentObligation,
    ) -> bool {
        let program = proof_plan.program;
        let Some(state) = program.machines().iter().find_map(|machine| {
            state_by_symbol(program.machine_states(machine), obligation.state_symbol)
        }) else {
            return false;
        };
        let Some(window_path) = written_place_path(proof_plan, obligation.target) else {
            return false;
        };
        if matches!(
            program.expression_table.expression(obligation.target),
            ExpressionNode::Indexed(_)
        ) {
            return false;
        }
        let target_name = program.expression_table.display_name(obligation.target);
        let mut reached_assignment = false;

        for statement in program.statement_table.statements(state.statement_nodes) {
            if !reached_assignment {
                if let Some(assignment) = statement.as_assignment()
                    && assignment.target == obligation.target
                    && assignment.value == obligation.value
                {
                    reached_assignment = true;
                }
                continue;
            }

            match statement {
                StatementNode::Assembly(_) => {}
                StatementNode::AssemblyFact(_) => return false,
                StatementNode::Assignment(assignment) => {
                    if expression_contains_call_node(proof_plan.program, assignment.value) {
                        return false;
                    }
                    let mut reads = Vec::new();
                    collect_read_place_paths(proof_plan, assignment.value, &mut reads);
                    if reads
                        .iter()
                        .any(|read| member_paths_may_alias(read, &window_path))
                    {
                        return false;
                    }

                    let Some(written) = written_place_path(proof_plan, assignment.target) else {
                        return false;
                    };
                    if program.expression_table.display_name(assignment.target) == target_name {
                        return true;
                    }
                    if member_paths_may_alias(&written, &window_path) {
                        return false;
                    }
                }
                StatementNode::LocalData(local) => {
                    if local.initial_value.is_valid() {
                        if expression_contains_call_node(proof_plan.program, local.initial_value) {
                            return false;
                        }
                        let mut reads = Vec::new();
                        collect_read_place_paths(proof_plan, local.initial_value, &mut reads);
                        if reads
                            .iter()
                            .any(|read| member_paths_may_alias(read, &window_path))
                        {
                            return false;
                        }
                    }
                }
                StatementNode::Expression(expression) => {
                    if expression_contains_call_node(proof_plan.program, *expression) {
                        return false;
                    }
                    let mut reads = Vec::new();
                    collect_read_place_paths(proof_plan, *expression, &mut reads);
                    if reads
                        .iter()
                        .any(|read| member_paths_may_alias(read, &window_path))
                    {
                        return false;
                    }
                }
                StatementNode::Call(_) | StatementNode::Transition(_) => return false,
            }
        }

        false
    }

    pub(crate) fn check_bounded_initializer(
        proof_plan: &ProofPlan,
        obligation: &BoundedInitializerObligation,
        seed: u64,
        diagnostics: &mut Vec<Diagnostic>,
        _recipe_obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
        measurements: &mut ProofPlanMeasurements,
        derivations: Option<&mut DerivationConsultation<'_>>,
    ) {
        check_initializer_named_constraints(proof_plan, obligation, diagnostics);

        if let Some(target_range) =
            integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
        {
            match bounded_integer_value_verdict(
                proof_plan,
                obligation.value,
                obligation.value_constraints,
                obligation.base_type,
                &target_range,
                seed,
                false,
                measurements,
                derivations,
            ) {
                CertificateVerdict::Certified => {}
                CertificateVerdict::Rejected => {
                    diagnostics.push(cannot_prove_bounded_initializer_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                }
                CertificateVerdict::Uncovered => {
                    let value_range = integer_range_for_initializer(proof_plan, obligation);
                    let interval_fits = value_range.as_ref().is_some_and(|range| {
                        range.minimum >= target_range.minimum
                            && range.maximum <= target_range.maximum
                    });
                    let occurrence_fits = !interval_fits
                        && obligation.owner.state_statement().is_some_and(
                            |(machine_symbol, state_symbol, statement_index)| {
                                validation::occurrence_integer_expression_fits_range(
                                    proof_plan.program,
                                    machine_symbol,
                                    state_symbol,
                                    statement_index,
                                    obligation.value,
                                    &target_range.minimum,
                                    &target_range.maximum,
                                )
                            },
                        );
                    if !interval_fits && !occurrence_fits {
                        diagnostics.push(cannot_prove_bounded_initializer_integer(
                            proof_plan,
                            obligation,
                            target_range,
                        ));
                        if value_range.is_none() {
                            return;
                        }
                    }
                }
            }
        }

        if discharge_declared_float_range(
            proof_plan,
            obligation,
            obligation.constraints,
            diagnostics,
            float_range_for_initializer,
            cannot_prove_bounded_initializer_float,
        ) {}
    }

    pub(crate) fn check_bounded_state_return<'program>(
        proof_plan: &ProofPlan<'program>,
        obligation: &BoundedStateReturnObligation,
        context: &AssignmentRangeContext<'program>,
        seed: u64,
        diagnostics: &mut Vec<Diagnostic>,
        _recipe_obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
        measurements: &mut ProofPlanMeasurements,
        derivations: Option<&mut DerivationConsultation<'_>>,
    ) {
        check_return_named_constraints(proof_plan, obligation, diagnostics);

        if let Some(target_range) =
            integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
        {
            match state_return_integer_verdict(
                proof_plan,
                obligation,
                context,
                &target_range,
                seed,
                measurements,
                derivations,
            ) {
                CertificateVerdict::Certified => {}
                CertificateVerdict::Rejected => {
                    diagnostics.push(cannot_prove_bounded_return_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                }
                CertificateVerdict::Uncovered => {
                    let Some(value_range) = integer_range(proof_plan, obligation, context) else {
                        diagnostics.push(cannot_prove_bounded_return_integer(
                            proof_plan,
                            obligation,
                            target_range,
                        ));
                        return;
                    };

                    if value_range.minimum < target_range.minimum
                        || value_range.maximum > target_range.maximum
                    {
                        diagnostics.push(cannot_prove_bounded_return_integer(
                            proof_plan,
                            obligation,
                            target_range,
                        ));
                    }
                }
            }
        }

        if discharge_declared_float_range(
            proof_plan,
            obligation,
            obligation.constraints,
            diagnostics,
            float_range_for_return_value,
            cannot_prove_bounded_return_float,
        ) {}
    }

    pub(crate) fn check_bounded_call_argument(
        proof_plan: &ProofPlan,
        obligation: &BoundedCallArgumentObligation,
        seed: u64,
        diagnostics: &mut Vec<Diagnostic>,
        _recipe_obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
        measurements: &mut ProofPlanMeasurements,
        derivations: Option<&mut DerivationConsultation<'_>>,
    ) {
        check_call_named_constraints(proof_plan, obligation, diagnostics);

        if let Some(target_range) =
            integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
        {
            match bounded_integer_value_verdict(
                proof_plan,
                obligation.argument,
                obligation.argument_constraints,
                obligation.base_type,
                &target_range,
                seed,
                true,
                measurements,
                derivations,
            ) {
                CertificateVerdict::Certified => {}
                CertificateVerdict::Rejected => {
                    diagnostics.push(cannot_prove_bounded_call_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                }
                CertificateVerdict::Uncovered => {
                    let argument_range = integer_range_for_call_argument(proof_plan, obligation);
                    let interval_fits = argument_range.as_ref().is_some_and(|range| {
                        range.minimum >= target_range.minimum
                            && range.maximum <= target_range.maximum
                    });
                    if !interval_fits
                        && !validation::occurrence_integer_expression_fits_range(
                            proof_plan.program,
                            obligation.machine_symbol,
                            obligation.state_symbol,
                            obligation.statement_index,
                            obligation.argument,
                            &target_range.minimum,
                            &target_range.maximum,
                        )
                    {
                        diagnostics.push(cannot_prove_bounded_call_integer(
                            proof_plan,
                            obligation,
                            target_range,
                        ));
                        if argument_range.is_none() {
                            return;
                        }
                    }
                }
            }
        }

        if discharge_declared_float_range(
            proof_plan,
            obligation,
            obligation.constraints,
            diagnostics,
            float_range_for_call_argument,
            cannot_prove_bounded_call_float,
        ) {}
    }

    pub(crate) fn check_bounded_transition_argument(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
        seed: u64,
        diagnostics: &mut Vec<Diagnostic>,
        _recipe_obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
        measurements: &mut ProofPlanMeasurements,
        mut derivations: Option<&mut DerivationConsultation<'_>>,
    ) {
        check_transition_named_constraints(proof_plan, obligation, diagnostics);

        if let Some(target_range) =
            integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
        {
            match bounded_integer_value_verdict(
                proof_plan,
                obligation.argument,
                obligation.argument_constraints,
                obligation.base_type,
                &target_range,
                seed,
                true,
                measurements,
                derivations.as_deref_mut(),
            ) {
                CertificateVerdict::Certified => {}
                CertificateVerdict::Rejected => {
                    diagnostics.push(cannot_prove_bounded_transition_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                }
                CertificateVerdict::Uncovered => {
                    match guarded_transition_integer_verdict_measured(
                        proof_plan,
                        obligation,
                        &target_range,
                        seed,
                        measurements,
                        derivations,
                    ) {
                        CertificateVerdict::Certified => {}
                        CertificateVerdict::Rejected => {
                            diagnostics.push(cannot_prove_bounded_transition_integer(
                                proof_plan,
                                obligation,
                                target_range,
                            ));
                        }
                        CertificateVerdict::Uncovered => {
                            let argument_range = guarded_integer_range_for_transition_argument(
                                proof_plan, obligation,
                            );

                            if argument_range.minimum < target_range.minimum
                                || argument_range.maximum > target_range.maximum
                            {
                                let arrival_fits = validation::arrival_integer_expression_bounds(
                                    proof_plan.program,
                                    obligation.machine_symbol,
                                    obligation.state_symbol,
                                    obligation.statement_index,
                                    obligation.argument,
                                )
                                .is_some_and(
                                    |(minimum, maximum)| {
                                        BigInt::from_i64(minimum) >= target_range.minimum
                                            && BigInt::from_i64(maximum) <= target_range.maximum
                                    },
                                );
                                if !arrival_fits {
                                    diagnostics.push(cannot_prove_bounded_transition_integer(
                                        proof_plan,
                                        obligation,
                                        target_range,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        if discharge_declared_float_range(
            proof_plan,
            obligation,
            obligation.constraints,
            diagnostics,
            guarded_float_range_for_transition_argument,
            cannot_prove_bounded_transition_float,
        ) {}
    }

    fn discharge_declared_float_range<O>(
        proof_plan: &ProofPlan,
        obligation: &O,
        constraints: arena::HandleSpan<ProofConstraint>,
        diagnostics: &mut Vec<Diagnostic>,
        range: fn(&ProofPlan, &O) -> Option<FloatRange>,
        refuse: fn(&ProofPlan, &O, FloatRange) -> Diagnostic,
    ) -> bool {
        let Some(target_range) =
            float_range_from_constraints(type_constraints(proof_plan, constraints))
        else {
            return false;
        };
        let Some(value_range) = range(proof_plan, obligation) else {
            diagnostics.push(refuse(proof_plan, obligation, target_range));
            return true;
        };
        if !target_range.contains_range(&value_range) {
            diagnostics.push(refuse(proof_plan, obligation, target_range));
        }
        false
    }
}
mod certificate {
    use arena::HandleSpan;
    use numerics::bignum::BigInt;
    use proof_admission::IntegerAffineWitness;
    use proof_admission::{
        AcceptedFact, CertificateEnvelope, EvidenceError, EvidenceRoute, Obligation,
        ObligationClass, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
        verify_obligation,
    };
    use semantic_vocabulary::{
        EvidenceIdentity, IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType,
        IntegerValue, ObligationId, Proposition, PropositionContext, ScalarTerm, ScalarType,
        ValueId,
    };
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
    };
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TransitionGuardNode};
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    use crate::checker::AssignmentRangeContext;
    use crate::checker::derivation_cache::DerivationConsultation;
    use crate::checker::guards::{expressions_equivalent_for_proof, unwrap_true_guard_condition};
    use crate::checker::integer_ranges::{
        integer_literal_handle, integer_range_for_return_value, integer_range_from_constraints,
        type_constraints,
    };
    use crate::checker::measurement::ProofPlanMeasurements;
    use crate::checker::requires_conditions;
    use crate::obligations::{
        BoundedStateReturnObligation, BoundedTransitionArgumentObligation, IntegerRange,
        ProofConstraint, ProofPlan,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CertificateVerdict {
        Certified,
        Rejected,
        Uncovered,
    }

    #[derive(Debug)]
    pub(crate) struct BoundedValueCertificate {
        context: PropositionContext,
        obligation: Obligation,
        assumptions: Vec<Proposition>,
        envelope: CertificateEnvelope,
    }

    impl BoundedValueCertificate {
        pub(crate) fn verify(&self) -> Result<AcceptedFact, EvidenceError> {
            verify_obligation(
                &self.context,
                &self.obligation,
                &self.assumptions,
                &[],
                EvidenceRoute::CertificateDerived(self.envelope.clone()),
            )
        }
    }

    pub(crate) fn bounded_integer_value_verdict(
        proof_plan: &ProofPlan,
        value: ExpressionHandle,
        value_constraints: HandleSpan<ProofConstraint>,
        base_type: TypeReferenceHandle,
        target: &IntegerRange,
        seed: u64,
        anonymous: bool,
        measurements: &mut ProofPlanMeasurements,
        derivations: Option<&mut DerivationConsultation<'_>>,
    ) -> CertificateVerdict {
        certificate_verdict_measured(measurements, derivations, || {
            bounded_integer_value(
                proof_plan,
                value,
                value_constraints,
                base_type,
                target,
                seed,
                anonymous,
            )
        })
    }

    fn certificate_verdict_measured(
        measurements: &mut ProofPlanMeasurements,
        mut derivations: Option<&mut DerivationConsultation<'_>>,
        produce: impl FnOnce() -> Option<BoundedValueCertificate>,
    ) -> CertificateVerdict {
        if let Some(consultation) = derivations.as_deref_mut()
            && let Some(fact) = consultation.recheck()
        {
            measurements.record_accepted_fact(&fact);
            measurements.record_certificate_verdict(CertificateVerdict::Certified);
            return CertificateVerdict::Certified;
        }
        let Some(certificate) = produce() else {
            measurements.record_certificate_verdict(CertificateVerdict::Uncovered);
            return CertificateVerdict::Uncovered;
        };
        measurements.record_emitted_certificate(&certificate.envelope.proof);
        let verdict = match certificate.verify() {
            Ok(fact) => {
                measurements.record_accepted_fact(&fact);
                CertificateVerdict::Certified
            }
            Err(_) => CertificateVerdict::Rejected,
        };
        measurements.record_certificate_verdict(verdict);
        if verdict == CertificateVerdict::Certified
            && let Some(consultation) = derivations
        {
            consultation.retain(certificate);
        }
        verdict
    }

    pub(crate) fn state_return_integer_verdict<'program>(
        proof_plan: &ProofPlan<'program>,
        obligation: &BoundedStateReturnObligation,
        context: &AssignmentRangeContext<'program>,
        target: &IntegerRange,
        seed: u64,
        measurements: &mut ProofPlanMeasurements,
        derivations: Option<&mut DerivationConsultation<'_>>,
    ) -> CertificateVerdict {
        certificate_verdict_measured(measurements, derivations, || {
            state_return_certificate(proof_plan, obligation, context, target, seed)
        })
    }

    fn state_return_certificate<'program>(
        proof_plan: &ProofPlan<'program>,
        obligation: &BoundedStateReturnObligation,
        context: &AssignmentRangeContext<'program>,
        target: &IntegerRange,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let program = proof_plan.program;
        let (machine, state) =
            super::machine_and_state(program, obligation.machine_symbol, obligation.state_symbol)?;
        let statements = program.statement_table.statements(state.statement_nodes);
        if !std::ptr::eq(program, context.program)
            || !matches!(
                statements.get(obligation.statement_index),
                Some(StatementNode::Expression(value)) if *value == obligation.value
            )
        {
            return None;
        }
        if let Some((minimum, maximum)) = validation::arrival_integer_expression_bounds(
            program,
            obligation.machine_symbol,
            obligation.state_symbol,
            obligation.statement_index,
            obligation.value,
        ) {
            return interval_premise_certificate(
                proof_plan,
                obligation.base_type,
                minimum,
                maximum,
                target,
                seed,
            );
        }
        bounded_integer_value(
            proof_plan,
            obligation.value,
            obligation.value_constraints,
            obligation.base_type,
            target,
            seed,
            false,
        )
        .or_else(|| {
            contract_refined_certificate(
                proof_plan, obligation, machine, state, context, target, seed,
            )
        })
    }

    fn interval_premise_certificate(
        proof_plan: &ProofPlan,
        base_type: TypeReferenceHandle,
        minimum: i64,
        maximum: i64,
        target: &IntegerRange,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let interval = IntegerRange {
            minimum: BigInt::from_i64(minimum),
            maximum: BigInt::from_i64(maximum),
        };
        if interval.minimum < target.minimum || interval.maximum > target.maximum {
            return None;
        }
        let integer_type =
            fixed_integer_type(proof_plan.program.primitive_type_reference(base_type)?)?;
        declared_interval_certificate(
            &interval,
            integer_type,
            bigint_math_literal(&target.minimum)?,
            bigint_math_literal(&target.maximum)?,
            seed,
        )
    }

    fn contract_refined_certificate<'program>(
        proof_plan: &ProofPlan<'program>,
        obligation: &BoundedStateReturnObligation,
        machine: &'program Machine,
        state: &'program State,
        context: &AssignmentRangeContext<'program>,
        target: &IntegerRange,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let program = proof_plan.program;
        let declared = integer_range_for_return_value(proof_plan, obligation)?;
        let integer_type =
            fixed_integer_type(program.primitive_type_reference(obligation.base_type)?)?;

        let conditions = requires_conditions::surviving_conditions(
            proof_plan,
            context,
            machine.symbol,
            state.symbol,
            obligation.statement_index,
            obligation.value,
            obligation.binary_operands.as_ref(),
        );
        if conditions.is_empty() {
            return None;
        }

        let mut facts = Vec::new();
        push_bound(&mut facts, declared.minimum.clone(), true);
        push_bound(&mut facts, declared.maximum.clone(), false);
        for condition in &conditions {
            collect_contract_bounds(proof_plan, obligation.value, *condition, &mut facts);
        }
        let mut narrowed = declared.clone();
        for fact in &facts[2..] {
            if fact.lower {
                narrowed.minimum = narrowed.minimum.max(fact.bound.clone());
            } else {
                narrowed.maximum = narrowed.maximum.min(fact.bound.clone());
            }
        }
        if narrowed.minimum > narrowed.maximum {
            facts.truncate(2);
        }

        let value = ValueId::new(seed)?;
        let (atom, proposition_context) = integer_math_atom(integer_type, value)?;
        let mut assumptions = Vec::with_capacity(facts.len());
        for fact in &facts {
            push_math_premise(&mut assumptions, &fact.bound, &atom, fact.lower);
        }
        guarded_bounds_certificate(atom, assumptions, proposition_context, target, seed)
    }

    fn integer_math_atom(
        integer_type: IntegerType,
        value: ValueId,
    ) -> Option<(IntegerMathTerm, PropositionContext)> {
        let atom = IntegerMathTerm::MathValue {
            source_type: integer_type,
            value,
        };
        let context =
            PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))])
                .ok()?;
        Some((atom, context))
    }

    fn collect_contract_bounds(
        proof_plan: &ProofPlan,
        subject: ExpressionHandle,
        condition: ExpressionHandle,
        facts: &mut Vec<GuardBound>,
    ) {
        let program = proof_plan.program;
        let Some(binary) = program.expression_table.binary_node(condition) else {
            return;
        };
        if binary.operator == BinaryOperator::Equal {
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) {
                return collect_contract_bounds(proof_plan, subject, binary.left, facts);
            }
            if matches!(
                program.expression_table.expression(binary.left),
                ExpressionNode::Boolean(true)
            ) {
                return collect_contract_bounds(proof_plan, subject, binary.right, facts);
            }
        }
        if binary.operator == BinaryOperator::And {
            collect_contract_bounds(proof_plan, subject, binary.left, facts);
            collect_contract_bounds(proof_plan, subject, binary.right, facts);
            return;
        }
        if requires_conditions::same_place(proof_plan, binary.left, subject) {
            if let Some(value) = integer_literal_handle(proof_plan, binary.right) {
                push_right_literal_bounds(binary.operator, BigInt::from_i64(value), facts);
            }
            return;
        }
        if requires_conditions::same_place(proof_plan, binary.right, subject)
            && let Some(value) = integer_literal_handle(proof_plan, binary.left)
        {
            push_left_literal_bounds(binary.operator, BigInt::from_i64(value), facts);
        }
    }

    pub(crate) fn guarded_transition_integer_verdict_measured(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
        target: &IntegerRange,
        seed: u64,
        measurements: &mut ProofPlanMeasurements,
        derivations: Option<&mut DerivationConsultation<'_>>,
    ) -> CertificateVerdict {
        certificate_verdict_measured(measurements, derivations, || {
            guarded_transition_certificate(proof_plan, obligation, target, seed)
        })
    }

    fn guarded_transition_certificate(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
        target: &IntegerRange,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let anonymous_owned = proof_plan
            .program
            .primitive_type_reference(obligation.base_type)
            .and_then(|primitive| {
                validation::land_anonymous_integer_expression(
                    proof_plan.program,
                    obligation.argument,
                    primitive,
                    &[],
                    |expression| {
                        validation::has_anonymous_operator_meaning(proof_plan.program, expression)
                    },
                )
            })
            .is_some();
        if anonymous_owned {
            return None;
        }
        guarded_direct_certificate(proof_plan, obligation, target, seed)
            .or_else(|| guarded_refold_certificate(proof_plan, obligation, target, seed))
            .or_else(|| arrival_argument_certificate(proof_plan, obligation, target, seed))
    }

    fn arrival_argument_certificate(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
        target: &IntegerRange,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let (minimum, maximum) = validation::arrival_integer_expression_bounds(
            proof_plan.program,
            obligation.machine_symbol,
            obligation.state_symbol,
            obligation.statement_index,
            obligation.argument,
        )?;
        interval_premise_certificate(
            proof_plan,
            obligation.base_type,
            minimum,
            maximum,
            target,
            seed,
        )
    }

    fn guarded_direct_certificate(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
        target: &IntegerRange,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let integer_type = fixed_integer_type(
            proof_plan
                .program
                .primitive_type_reference(obligation.base_type)?,
        )?;
        let value = ValueId::new(seed)?;
        let (atom, context) = integer_math_atom(integer_type, value)?;

        let mut facts = Vec::new();
        if let Some(declared) = integer_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )) {
            push_bound(&mut facts, declared.minimum.clone(), true);
            push_bound(&mut facts, declared.maximum.clone(), false);
        }
        if let TransitionGuardNode::When(condition) = &obligation.guard {
            collect_guard_bounds(proof_plan, obligation.argument, *condition, &mut facts);
        }
        for refuted in &obligation.refuted_exit_guards {
            collect_refuted_bounds(proof_plan, obligation.argument, *refuted, &mut facts);
        }
        let mut assumptions = Vec::with_capacity(facts.len());
        for fact in &facts {
            push_math_premise(&mut assumptions, &fact.bound, &atom, fact.lower);
        }
        guarded_bounds_certificate(atom, assumptions, context, target, seed)
    }

    fn guarded_bounds_certificate(
        atom: IntegerMathTerm,
        assumptions: Vec<Proposition>,
        context: PropositionContext,
        target: &IntegerRange,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let mut lower_anchor: Option<(usize, BigInt)> = None;
        let mut upper_anchor: Option<(usize, BigInt)> = None;
        for (index, assumption) in assumptions.iter().enumerate() {
            let Proposition::IntegerMathLessOrEqual(left, right) = assumption else {
                continue;
            };
            if *right == atom
                && let IntegerMathTerm::IntegerLiteral(bound) = left
            {
                let bound = proof_admission::big_integer_literal(*bound);
                if lower_anchor
                    .as_ref()
                    .is_none_or(|(_, strongest)| bound > *strongest)
                {
                    lower_anchor = Some((index, bound));
                }
            }
            if *left == atom
                && let IntegerMathTerm::IntegerLiteral(bound) = right
            {
                let bound = proof_admission::big_integer_literal(*bound);
                if upper_anchor
                    .as_ref()
                    .is_none_or(|(_, strongest)| bound < *strongest)
                {
                    upper_anchor = Some((index, bound));
                }
            }
        }
        let (lower_index, lower_bound) = lower_anchor?;
        let (upper_index, upper_bound) = upper_anchor?;
        if target.minimum > lower_bound || target.maximum < upper_bound {
            return None;
        }
        let lower_leg = transitivity_leg(
            closed_literal_relation(
                bigint_math_literal(&target.minimum)?,
                bigint_math_literal(&lower_bound)?,
            ),
            assumption_node(assumptions[lower_index].clone(), lower_index),
        );
        let upper_leg = transitivity_leg(
            assumption_node(assumptions[upper_index].clone(), upper_index),
            closed_literal_relation(
                bigint_math_literal(&upper_bound)?,
                bigint_math_literal(&target.maximum)?,
            ),
        );
        bounded_conjunction_certificate(context, seed, lower_leg, upper_leg, assumptions)
    }

    struct GuardBound {
        bound: BigInt,
        lower: bool,
    }

    fn push_bound(facts: &mut Vec<GuardBound>, bound: BigInt, lower: bool) {
        facts.push(GuardBound { bound, lower });
    }

    fn collect_guard_bounds(
        proof_plan: &ProofPlan,
        subject: ExpressionHandle,
        condition: ExpressionHandle,
        facts: &mut Vec<GuardBound>,
    ) {
        let program = proof_plan.program;
        if let ExpressionNode::Unary(unary) = program.expression_table.expression(condition)
            && unary.operator == UnaryOperator::LogicalNot
        {
            collect_refuted_bounds(proof_plan, subject, unary.operand, facts);
            return;
        }
        let Some(binary) = program.expression_table.binary_node(condition) else {
            return;
        };
        if binary.operator == BinaryOperator::Equal {
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) {
                return collect_guard_bounds(proof_plan, subject, binary.left, facts);
            }
            if matches!(
                program.expression_table.expression(binary.left),
                ExpressionNode::Boolean(true)
            ) {
                return collect_guard_bounds(proof_plan, subject, binary.right, facts);
            }
        }
        if binary.operator == BinaryOperator::And {
            collect_guard_bounds(proof_plan, subject, binary.left, facts);
            collect_guard_bounds(proof_plan, subject, binary.right, facts);
            return;
        }
        if expressions_equivalent_for_proof(proof_plan, binary.left, subject) {
            if let Some(value) = integer_literal_handle(proof_plan, binary.right) {
                push_right_literal_bounds(binary.operator, BigInt::from_i64(value), facts);
            }
            return;
        }
        if expressions_equivalent_for_proof(proof_plan, binary.right, subject)
            && let Some(value) = integer_literal_handle(proof_plan, binary.left)
        {
            push_left_literal_bounds(binary.operator, BigInt::from_i64(value), facts);
        }
    }

    fn collect_refuted_bounds(
        proof_plan: &ProofPlan,
        subject: ExpressionHandle,
        condition: ExpressionHandle,
        facts: &mut Vec<GuardBound>,
    ) {
        let program = proof_plan.program;
        let condition = unwrap_true_guard_condition(proof_plan, condition);
        if let ExpressionNode::Unary(unary) = program.expression_table.expression(condition)
            && unary.operator == UnaryOperator::LogicalNot
        {
            collect_guard_bounds(proof_plan, subject, unary.operand, facts);
            return;
        }
        let Some(binary) = program.expression_table.binary_node(condition) else {
            return;
        };
        if !expressions_equivalent_for_proof(proof_plan, binary.left, subject) {
            return;
        }
        let Some(value) = integer_literal_handle(proof_plan, binary.right) else {
            return;
        };
        if let Some(interval) = validation::BigIntInterval::from_refuted_comparison(
            binary.operator,
            true,
            BigInt::from_i64(value),
        ) {
            push_interval_bounds(interval, facts);
        }
    }

    fn push_right_literal_bounds(
        operator: BinaryOperator,
        value: BigInt,
        facts: &mut Vec<GuardBound>,
    ) {
        if let Some(interval) = validation::BigIntInterval::from_comparison(operator, true, value) {
            push_interval_bounds(interval, facts);
        }
    }

    fn push_interval_bounds(interval: validation::BigIntInterval, facts: &mut Vec<GuardBound>) {
        if let Some(low) = interval.low {
            push_bound(facts, low, true);
        }
        if let Some(high) = interval.high {
            push_bound(facts, high, false);
        }
    }

    fn push_left_literal_bounds(
        operator: BinaryOperator,
        value: BigInt,
        facts: &mut Vec<GuardBound>,
    ) {
        push_right_literal_bounds(
            crate::checker::guards::mirrored_comparison(operator),
            value,
            facts,
        )
    }

    fn push_math_premise(
        assumptions: &mut Vec<Proposition>,
        bound: &BigInt,
        atom: &IntegerMathTerm,
        lower_side: bool,
    ) {
        let Some(bound) = bigint_math_literal(bound) else {
            return;
        };
        let bound = IntegerMathTerm::IntegerLiteral(bound);
        assumptions.push(if lower_side {
            Proposition::IntegerMathLessOrEqual(bound, atom.clone())
        } else {
            Proposition::IntegerMathLessOrEqual(atom.clone(), bound)
        });
    }

    fn guarded_refold_certificate(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
        target: &IntegerRange,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let program = proof_plan.program;
        let binary = program.expression_table.binary_node(obligation.argument)?;
        let (place, literal, subtract) = match binary.operator {
            BinaryOperator::Add => {
                if let Some(literal) = integer_literal_handle(proof_plan, binary.right) {
                    (binary.left, literal, false)
                } else {
                    let literal = integer_literal_handle(proof_plan, binary.left)?;
                    (binary.right, literal, false)
                }
            }
            BinaryOperator::Subtract => {
                let literal = integer_literal_handle(proof_plan, binary.right)?;
                (binary.left, literal, true)
            }
            _ => return None,
        };
        if integer_literal_handle(proof_plan, place).is_some() {
            return None;
        }
        let exact = type_constraints(proof_plan, obligation.argument_constraints)
            .iter()
            .all(|constraint| match constraint {
                ProofConstraint::ArithmeticDomain(domain) => {
                    *domain == numerics::arithmetic::ArithmeticDomain::Exact
                }
                ProofConstraint::Named(name) => name.as_str() != "wrapping",
                _ => true,
            });
        if !exact {
            return None;
        }

        let (machine, state) =
            super::machine_and_state(program, obligation.machine_symbol, obligation.state_symbol)?;
        let place_type =
            crate::typed_expressions::expression_type_reference(program, machine, state, place)?;
        let integer_type = fixed_integer_type(program.primitive_type_reference(place_type)?)?;
        let literal_bigint = BigInt::from_i64(literal);

        let place_constraints: Vec<ProofConstraint> =
            crate::obligations::expression_constraints(program, machine, state, place)
                .into_iter()
                .collect();
        let mut facts = Vec::new();
        if let Some(declared) = integer_range_from_constraints(&place_constraints) {
            push_bound(&mut facts, declared.minimum.clone(), true);
            push_bound(&mut facts, declared.maximum.clone(), false);
        }
        if let TransitionGuardNode::When(condition) = &obligation.guard {
            collect_guard_bounds(proof_plan, place, *condition, &mut facts);
        }
        for refuted in &obligation.refuted_exit_guards {
            collect_refuted_bounds(proof_plan, place, *refuted, &mut facts);
        }

        let value = ValueId::new(seed)?;
        let place_scalar = ScalarTerm::value(value, ScalarType::Integer(integer_type));
        let (place_math, context) = integer_math_atom(integer_type, value)?;

        let mut assumptions = Vec::with_capacity(facts.len());
        let mut lower_anchor: Option<(usize, BigInt)> = None;
        let mut upper_anchor: Option<(usize, BigInt)> = None;
        for fact in &facts {
            let index = assumptions.len();
            if !push_scalar_premise(
                &mut assumptions,
                &fact.bound,
                &place_scalar,
                fact.lower,
                integer_type,
            ) {
                continue;
            }
            if fact.lower {
                if lower_anchor
                    .as_ref()
                    .is_none_or(|(_, strongest)| fact.bound > *strongest)
                {
                    lower_anchor = Some((index, fact.bound.clone()));
                }
            } else if upper_anchor
                .as_ref()
                .is_none_or(|(_, strongest)| fact.bound < *strongest)
            {
                upper_anchor = Some((index, fact.bound.clone()));
            }
        }
        let (lower_index, lower_bound) = lower_anchor?;
        let (upper_index, upper_bound) = upper_anchor?;
        refold_bounds_certificate(
            place_scalar,
            place_math,
            integer_type,
            literal_bigint,
            subtract,
            assumptions,
            (lower_index, lower_bound),
            (upper_index, upper_bound),
            context,
            target,
            seed,
        )
    }

    fn refold_bounds_certificate(
        place_scalar: ScalarTerm,
        place_math: IntegerMathTerm,
        integer_type: IntegerType,
        literal: BigInt,
        subtract: bool,
        assumptions: Vec<Proposition>,
        lower_anchor: (usize, BigInt),
        upper_anchor: (usize, BigInt),
        context: PropositionContext,
        target: &IntegerRange,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let (lower_index, lower_bound) = lower_anchor;
        let (upper_index, upper_bound) = upper_anchor;

        let mapped_lower = if subtract {
            lower_bound.sub(&literal)
        } else {
            lower_bound.add(&literal)
        };
        let mapped_upper = if subtract {
            upper_bound.sub(&literal)
        } else {
            upper_bound.add(&literal)
        };
        if target.minimum > mapped_lower || target.maximum < mapped_upper {
            return None;
        }

        let literal_math = IntegerMathTerm::IntegerLiteral(bigint_math_literal(&literal)?);
        let argument_math = if subtract {
            IntegerMathTerm::Subtract(Box::new(place_math), Box::new(literal_math))
        } else {
            IntegerMathTerm::Add(Box::new(place_math), Box::new(literal_math))
        };
        let literal_scalar = scalar_integer_term(integer_type, &literal)?;
        let target_scalar = if subtract {
            ScalarTerm::exact_integer_subtract(integer_type, place_scalar.clone(), literal_scalar)
        } else {
            ScalarTerm::exact_integer_add(integer_type, place_scalar.clone(), literal_scalar)
        }
        .ok()?;
        let witness = IntegerAffineWitness {
            root: place_scalar,
            target: target_scalar,
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        };

        let lower_leg = transitivity_leg(
            closed_literal_relation(
                bigint_math_literal(&target.minimum)?,
                bigint_math_literal(&mapped_lower)?,
            ),
            affine_leg(
                &witness,
                &assumptions,
                lower_index,
                &mapped_lower,
                &argument_math,
                true,
            )?,
        );
        let upper_leg = transitivity_leg(
            affine_leg(
                &witness,
                &assumptions,
                upper_index,
                &mapped_upper,
                &argument_math,
                false,
            )?,
            closed_literal_relation(
                bigint_math_literal(&mapped_upper)?,
                bigint_math_literal(&target.maximum)?,
            ),
        );
        bounded_conjunction_certificate(context, seed, lower_leg, upper_leg, assumptions)
    }

    fn affine_leg(
        witness: &IntegerAffineWitness,
        assumptions: &[Proposition],
        anchor_index: usize,
        mapped: &BigInt,
        argument_math: &IntegerMathTerm,
        lower: bool,
    ) -> Option<ProofNode> {
        let premise = assumptions.get(anchor_index)?.clone();
        let root_bound = ProofNode {
            conclusion: Proposition::Conjunction(vec![premise.clone(), Proposition::Truth]),
            rule: ProofRule::ConjunctionIntroduction(vec![
                assumption_node(premise, anchor_index),
                ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                },
            ]),
        };
        let mapped = IntegerMathTerm::IntegerLiteral(bigint_math_literal(mapped)?);
        let conclusion = if lower {
            Proposition::IntegerMathLessOrEqual(mapped, argument_math.clone())
        } else {
            Proposition::IntegerMathLessOrEqual(argument_math.clone(), mapped)
        };
        Some(ProofNode {
            conclusion,
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(root_bound),
                witness: witness.clone(),
            },
        })
    }

    fn push_scalar_premise(
        assumptions: &mut Vec<Proposition>,
        bound: &BigInt,
        atom: &ScalarTerm,
        lower_side: bool,
        integer_type: IntegerType,
    ) -> bool {
        let Some(bound) = scalar_integer_term(integer_type, bound) else {
            return false;
        };
        assumptions.push(if lower_side {
            Proposition::LessOrEqual(bound, atom.clone())
        } else {
            Proposition::LessOrEqual(atom.clone(), bound)
        });
        true
    }

    fn scalar_integer_term(integer_type: IntegerType, bound: &BigInt) -> Option<ScalarTerm> {
        let value = match integer_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(bound.to_string().parse().ok()?),
            IntegerSign::Unsigned => {
                if bound.is_negative() {
                    return None;
                }
                IntegerValue::Unsigned(bound.to_string().parse().ok()?)
            }
        };
        ScalarTerm::integer(integer_type, value).ok()
    }

    fn bounded_integer_value(
        proof_plan: &ProofPlan,
        value: ExpressionHandle,
        value_constraints: HandleSpan<ProofConstraint>,
        base_type: TypeReferenceHandle,
        target: &IntegerRange,
        seed: u64,
        anonymous: bool,
    ) -> Option<BoundedValueCertificate> {
        let lower = bigint_math_literal(&target.minimum)?;
        let upper = bigint_math_literal(&target.maximum)?;
        let node = proof_plan.program.expression_table.expression(value);
        if anonymous {
            let program = proof_plan.program;
            let landed = program
                .primitive_type_reference(base_type)
                .and_then(|primitive| {
                    validation::land_anonymous_integer_expression(
                        program,
                        value,
                        primitive,
                        &[],
                        |expression| {
                            validation::has_anonymous_operator_meaning(program, expression)
                        },
                    )
                });
            if let Some((landed, _)) = landed {
                let term = closed_math_term(proof_plan, value)?;
                if eval_math_term(&term)? != landed.value_bignum()? {
                    return None;
                }
                return closed_bounds_certificate(term, lower, upper, seed);
            }
        }
        if let ExpressionNode::Integer(literal) = node {
            let term = literal_math_term(literal)?;
            return closed_bounds_certificate(term, lower, upper, seed);
        }
        declared_bounds_certificate(
            proof_plan,
            value_constraints,
            base_type,
            target,
            lower,
            upper,
            seed,
        )
    }

    fn closed_bounds_certificate(
        term: IntegerMathTerm,
        lower: IntegerMathLiteral,
        upper: IntegerMathLiteral,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let lower_conjunct = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::IntegerLiteral(lower),
            term.clone(),
        );
        let upper_conjunct =
            Proposition::IntegerMathLessOrEqual(term, IntegerMathTerm::IntegerLiteral(upper));
        bounded_conjunction_certificate(
            PropositionContext::default(),
            seed,
            closed_relation_node(lower_conjunct),
            closed_relation_node(upper_conjunct),
            Vec::new(),
        )
    }

    fn declared_bounds_certificate(
        proof_plan: &ProofPlan,
        value_constraints: HandleSpan<ProofConstraint>,
        base_type: TypeReferenceHandle,
        target: &IntegerRange,
        lower: IntegerMathLiteral,
        upper: IntegerMathLiteral,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let declared =
            integer_range_from_constraints(type_constraints(proof_plan, value_constraints))?;
        if declared.minimum < target.minimum || declared.maximum > target.maximum {
            return None;
        }
        let integer_type =
            fixed_integer_type(proof_plan.program.primitive_type_reference(base_type)?)?;
        declared_interval_certificate(&declared, integer_type, lower, upper, seed)
    }

    fn declared_interval_certificate(
        declared: &IntegerRange,
        integer_type: IntegerType,
        lower: IntegerMathLiteral,
        upper: IntegerMathLiteral,
        seed: u64,
    ) -> Option<BoundedValueCertificate> {
        let value = ValueId::new(seed)?;
        let (atom, context) = integer_math_atom(integer_type, value)?;
        let declared_lower = bigint_math_literal(&declared.minimum)?;
        let declared_upper = bigint_math_literal(&declared.maximum)?;

        let lower_premise = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::IntegerLiteral(declared_lower),
            atom.clone(),
        );
        let upper_premise = Proposition::IntegerMathLessOrEqual(
            atom.clone(),
            IntegerMathTerm::IntegerLiteral(declared_upper),
        );

        let lower_leg = transitivity_leg(
            closed_literal_relation(lower, declared_lower),
            assumption_node(lower_premise.clone(), 0),
        );
        let upper_leg = transitivity_leg(
            assumption_node(upper_premise.clone(), 1),
            closed_literal_relation(declared_upper, upper),
        );
        bounded_conjunction_certificate(
            context,
            seed,
            lower_leg,
            upper_leg,
            vec![lower_premise, upper_premise],
        )
    }

    fn bounded_conjunction_certificate(
        context: PropositionContext,
        seed: u64,
        lower_leg: ProofNode,
        upper_leg: ProofNode,
        assumptions: Vec<Proposition>,
    ) -> Option<BoundedValueCertificate> {
        let conclusion = Proposition::Conjunction(vec![
            lower_leg.conclusion.clone(),
            upper_leg.conclusion.clone(),
        ]);
        let proof = ProofNode {
            conclusion: conclusion.clone(),
            rule: ProofRule::ConjunctionIntroduction(vec![lower_leg, upper_leg]),
        };
        Some(BoundedValueCertificate {
            context,
            obligation: Obligation {
                id: ObligationId::new(seed)?,
                proposition: conclusion,
                class: ObligationClass::Derivable,
            },
            assumptions,
            envelope: Some(CertificateEnvelope {
                identity: EvidenceIdentity::new(seed)?,
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            })?,
        })
    }

    fn assumption_node(conclusion: Proposition, index: usize) -> ProofNode {
        ProofNode {
            conclusion,
            rule: ProofRule::Assumption { index },
        }
    }

    fn closed_literal_relation(left: IntegerMathLiteral, right: IntegerMathLiteral) -> ProofNode {
        closed_relation_node(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::IntegerLiteral(left),
            IntegerMathTerm::IntegerLiteral(right),
        ))
    }

    fn closed_relation_node(conclusion: Proposition) -> ProofNode {
        ProofNode {
            conclusion,
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        }
    }

    fn transitivity_leg(
        left_less_or_equal_middle: ProofNode,
        middle_less_or_equal_right: ProofNode,
    ) -> ProofNode {
        let (
            Proposition::IntegerMathLessOrEqual(left, _),
            Proposition::IntegerMathLessOrEqual(_, right),
        ) = (
            &left_less_or_equal_middle.conclusion,
            &middle_less_or_equal_right.conclusion,
        )
        else {
            unreachable!("bounded-value legs only chain mathematical <= premises")
        };
        ProofNode {
            conclusion: Proposition::IntegerMathLessOrEqual(left.clone(), right.clone()),
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(left_less_or_equal_middle),
                middle_less_or_equal_right: Box::new(middle_less_or_equal_right),
            },
        }
    }

    fn closed_math_term(
        proof_plan: &ProofPlan,
        expression: ExpressionHandle,
    ) -> Option<IntegerMathTerm> {
        let program = proof_plan.program;
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(literal) => literal_math_term(literal),
            ExpressionNode::Name(_) => {
                let value = integer_literal_handle(proof_plan, expression)?;
                math_literal_term(BigInt::from_i64(value))
            }
            ExpressionNode::Binary(binary) => {
                if !validation::has_anonymous_operator_meaning(program, expression) {
                    return None;
                }
                let left = closed_math_term(proof_plan, binary.left)?;
                let right = closed_math_term(proof_plan, binary.right)?;
                match binary.operator {
                    BinaryOperator::Add => {
                        Some(IntegerMathTerm::Add(Box::new(left), Box::new(right)))
                    }
                    BinaryOperator::Subtract => {
                        Some(IntegerMathTerm::Subtract(Box::new(left), Box::new(right)))
                    }
                    BinaryOperator::Multiply => {
                        Some(IntegerMathTerm::Multiply(Box::new(left), Box::new(right)))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn eval_math_term(term: &IntegerMathTerm) -> Option<BigInt> {
        match term {
            IntegerMathTerm::IntegerLiteral(literal) => {
                Some(proof_admission::big_integer_literal(*literal))
            }
            IntegerMathTerm::Add(left, right) => {
                Some(eval_math_term(left)?.add(&eval_math_term(right)?))
            }
            IntegerMathTerm::Subtract(left, right) => {
                Some(eval_math_term(left)?.sub(&eval_math_term(right)?))
            }
            IntegerMathTerm::Multiply(left, right) => {
                Some(eval_math_term(left)?.mul(&eval_math_term(right)?))
            }
            IntegerMathTerm::ShiftLeft { .. }
            | IntegerMathTerm::MathValue { .. }
            | IntegerMathTerm::Embed(_) => None,
        }
    }

    fn literal_math_term(literal: &numerics::literals::IntegerLiteral) -> Option<IntegerMathTerm> {
        math_literal_term(literal.value_bignum()?)
    }

    fn math_literal_term(value: BigInt) -> Option<IntegerMathTerm> {
        Some(IntegerMathTerm::IntegerLiteral(bigint_math_literal(
            &value,
        )?))
    }

    fn bigint_math_literal(value: &BigInt) -> Option<IntegerMathLiteral> {
        let magnitude: u128 = value.abs().to_string().parse().ok()?;
        IntegerMathLiteral::new(value.is_negative(), magnitude).ok()
    }

    fn fixed_integer_type(primitive: PrimitiveType) -> Option<IntegerType> {
        let (sign, bits) = match primitive {
            PrimitiveType::I8 => (IntegerSign::Signed, 8),
            PrimitiveType::I16 => (IntegerSign::Signed, 16),
            PrimitiveType::I32 => (IntegerSign::Signed, 32),
            PrimitiveType::I64 => (IntegerSign::Signed, 64),
            PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
            PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
            PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
            PrimitiveType::U64 => (IntegerSign::Unsigned, 64),
            PrimitiveType::Selector
            | PrimitiveType::Bool
            | PrimitiveType::F32
            | PrimitiveType::F64
            | PrimitiveType::Addr => {
                return None;
            }
        };
        IntegerType::new(sign, bits).ok()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            bigint_math_literal, closed_bounds_certificate, declared_interval_certificate,
            guarded_bounds_certificate, refold_bounds_certificate, scalar_integer_term,
        };
        use crate::integer_range;
        use crate::test_program::{bounded_value, program};
        use numerics::bignum::BigInt;
        use proof_admission::{
            AcceptedFactRoute, AcceptedProofRule, EvidenceRoute, Obligation, ObligationClass,
            PrimitiveJudgment, ProofNode, ProofRule, verify_obligation,
        };
        use semantic_vocabulary::{
            IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, ObligationId,
            Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
        };

        fn math_literal(value: i64) -> IntegerMathLiteral {
            bigint_math_literal(&BigInt::from_i64(value)).expect("literal bound")
        }

        fn literal_term(value: i64) -> IntegerMathTerm {
            IntegerMathTerm::IntegerLiteral(math_literal(value))
        }

        fn u8_type() -> IntegerType {
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")
        }

        fn declared_u8_certificate(seed: u64) -> super::BoundedValueCertificate {
            declared_interval_certificate(
                &integer_range(0, 255),
                u8_type(),
                math_literal(0),
                math_literal(300),
                seed,
            )
            .expect("certificate")
        }

        fn middle_leg(certificate: &mut super::BoundedValueCertificate) -> &mut ProofNode {
            let ProofRule::ConjunctionIntroduction(conjuncts) =
                &mut certificate.envelope.proof.rule
            else {
                panic!("conjunction root");
            };
            let ProofRule::IntegerLessOrEqualTransitivity {
                middle_less_or_equal_right,
                ..
            } = &mut conjuncts[0].rule
            else {
                panic!("transitivity leg");
            };
            middle_less_or_equal_right
        }

        #[test]
        fn closed_arithmetic_term_verifies_under_kernel_evaluation() {
            let term = IntegerMathTerm::Add(Box::new(literal_term(2)), Box::new(literal_term(3)));
            let certificate = closed_bounds_certificate(term, math_literal(0), math_literal(10), 8)
                .expect("certificate");
            certificate.verify().expect("kernel accepts");
        }

        #[test]
        fn closed_literal_outside_target_is_rejected_by_the_kernel() {
            let certificate =
                closed_bounds_certificate(literal_term(300), math_literal(0), math_literal(5), 9)
                    .expect("certificate");
            assert!(certificate.verify().is_err());
        }

        #[test]
        fn closed_arithmetic_term_outside_target_is_rejected() {
            let term =
                IntegerMathTerm::Add(Box::new(literal_term(200)), Box::new(literal_term(100)));
            let certificate = closed_bounds_certificate(term, math_literal(0), math_literal(5), 10)
                .expect("certificate");
            assert!(certificate.verify().is_err());
        }

        #[test]
        fn declared_interval_certificate_rejects_an_uncited_premise() {
            let mut certificate = declared_u8_certificate(12);
            middle_leg(&mut certificate).rule = ProofRule::Assumption { index: 9 };
            assert!(certificate.verify().is_err());
        }

        #[test]
        fn declared_interval_certificate_rejects_a_withheld_premise() {
            let mut certificate = declared_u8_certificate(13);
            certificate.assumptions.remove(0);
            assert!(certificate.verify().is_err());
        }

        #[test]
        fn declared_interval_certificate_rejects_a_false_premise_swap() {
            let mut certificate = declared_u8_certificate(14);
            certificate.assumptions.swap(0, 1);
            assert!(certificate.verify().is_err());
        }

        #[test]
        fn tampered_obligation_conclusion_is_rejected() {
            let mut certificate =
                closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 15)
                    .expect("certificate");
            certificate.obligation.proposition = Proposition::Conjunction(vec![
                Proposition::IntegerMathLessOrEqual(literal_term(0), literal_term(5)),
                Proposition::IntegerMathLessOrEqual(literal_term(5), literal_term(400)),
            ]);
            assert!(certificate.verify().is_err());
        }

        #[test]
        fn transitivity_middle_mismatch_is_rejected() {
            let mut certificate = declared_u8_certificate(16);
            let ProofRule::ConjunctionIntroduction(conjuncts) =
                &mut certificate.envelope.proof.rule
            else {
                panic!("conjunction root");
            };
            let ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                ..
            } = &mut conjuncts[0].rule
            else {
                panic!("transitivity leg");
            };
            left_less_or_equal_middle.conclusion =
                Proposition::IntegerMathLessOrEqual(literal_term(0), literal_term(200));
            assert!(certificate.verify().is_err());
        }

        #[test]
        fn kernel_derived_route_still_decides_the_same_closed_claim() {
            let conclusion = Proposition::IntegerMathLessOrEqual(literal_term(3), literal_term(9));
            let fact = verify_obligation(
                &PropositionContext::default(),
                &Obligation {
                    id: ObligationId::new(21).expect("obligation id"),
                    proposition: conclusion.clone(),
                    class: ObligationClass::Derivable,
                },
                &[],
                &[],
                EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
            )
            .expect("kernel accepts a true closed relation");
            assert_eq!(fact.proposition, conclusion);
        }

        fn math_atom(integer_type: IntegerType, seed: u64) -> (ValueId, IntegerMathTerm) {
            let value = ValueId::new(seed).expect("value id");
            (
                value,
                IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value,
                },
            )
        }

        fn guarded_argument_certificate(seed: u64) -> super::BoundedValueCertificate {
            let integer_type = u8_type();
            let (value, atom) = math_atom(integer_type, seed);
            let context =
                PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))])
                    .expect("context");
            let assumptions = vec![
                Proposition::IntegerMathLessOrEqual(literal_term(0), atom.clone()),
                Proposition::IntegerMathLessOrEqual(atom.clone(), literal_term(255)),
                Proposition::IntegerMathLessOrEqual(literal_term(1), atom.clone()),
                Proposition::IntegerMathLessOrEqual(atom.clone(), literal_term(100)),
            ];
            guarded_bounds_certificate(atom, assumptions, context, &integer_range(0, 200), seed)
                .expect("certificate")
        }

        #[test]
        fn guarded_argument_premises_verify_under_the_kernel() {
            let certificate = guarded_argument_certificate(31);
            let fact = certificate.verify().expect("kernel accepts");
            assert_eq!(fact.proposition, certificate.obligation.proposition);
            let AcceptedFactRoute::CertificateDerived { acceptance, .. } = fact.route else {
                panic!("certificate-derived route");
            };
            assert_eq!(acceptance.assumptions.len(), 2);
            assert!(
                acceptance
                    .rules
                    .contains(&AcceptedProofRule::IntegerLessOrEqualTransitivity)
            );
        }

        #[test]
        fn guarded_argument_gap_stays_uncovered() {
            let integer_type = u8_type();
            let (value, atom) = math_atom(integer_type, 32);
            let context =
                PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))])
                    .expect("context");
            let assumptions = vec![
                Proposition::IntegerMathLessOrEqual(literal_term(1), atom.clone()),
                Proposition::IntegerMathLessOrEqual(atom.clone(), literal_term(100)),
            ];
            assert!(
                guarded_bounds_certificate(atom, assumptions, context, &integer_range(50, 200), 32)
                    .is_none()
            );
        }

        #[test]
        fn guarded_argument_certificate_rejects_a_miscited_premise() {
            let mut certificate = guarded_argument_certificate(33);
            middle_leg(&mut certificate).rule = ProofRule::Assumption { index: 0 };
            assert!(certificate.verify().is_err());
        }

        #[test]
        fn guarded_argument_certificate_rejects_a_dropped_premise() {
            let mut certificate = guarded_argument_certificate(34);
            certificate.assumptions.remove(3);
            assert!(certificate.verify().is_err());
        }

        fn work_decrement_certificate(seed: u64) -> super::BoundedValueCertificate {
            let integer_type = u8_type();
            let value = ValueId::new(seed).expect("value id");
            let place = ScalarTerm::value(value, ScalarType::Integer(integer_type));
            let place_math = IntegerMathTerm::MathValue {
                source_type: integer_type,
                value,
            };
            let context =
                PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))])
                    .expect("context");
            let scalar_bound = |bound| scalar_integer_term(integer_type, &BigInt::from_i64(bound));
            let assumptions = vec![
                Proposition::LessOrEqual(scalar_bound(2).expect("scalar"), place.clone()),
                Proposition::LessOrEqual(place.clone(), scalar_bound(128).expect("scalar")),
            ];
            refold_bounds_certificate(
                place,
                place_math,
                integer_type,
                BigInt::from_i64(1),
                true,
                assumptions,
                (0, BigInt::from_i64(2)),
                (1, BigInt::from_i64(128)),
                context,
                &integer_range(0, 127),
                seed,
            )
            .expect("certificate")
        }

        #[test]
        fn affine_refold_bound_verifies_under_the_kernel() {
            let certificate = work_decrement_certificate(41);
            let fact = certificate.verify().expect("kernel accepts");
            assert_eq!(fact.proposition, certificate.obligation.proposition);
            let AcceptedFactRoute::CertificateDerived { acceptance, .. } = fact.route else {
                panic!("certificate-derived route");
            };
            assert_eq!(acceptance.assumptions.len(), 2);
            assert!(
                acceptance
                    .rules
                    .contains(&AcceptedProofRule::IntegerAffineBound)
            );
        }

        #[test]
        fn affine_refold_rejects_a_wrong_mapped_endpoint() {
            let mut certificate = work_decrement_certificate(42);
            let step = middle_leg(&mut certificate);
            let ProofRule::IntegerAffineBound { .. } = step.rule else {
                panic!("affine leg");
            };
            let Proposition::IntegerMathLessOrEqual(_, argument) = &step.conclusion else {
                panic!("integer <= conclusion");
            };
            let argument = argument.clone();
            step.conclusion = Proposition::IntegerMathLessOrEqual(literal_term(5), argument);
            assert!(certificate.verify().is_err());
        }

        #[test]
        fn affine_refold_rejects_a_foreign_witness_root() {
            let mut certificate = work_decrement_certificate(43);
            let step = middle_leg(&mut certificate);
            let ProofRule::IntegerAffineBound { witness, .. } = &mut step.rule else {
                panic!("affine leg");
            };
            let foreign = ValueId::new(999).expect("value id");
            witness.root = ScalarTerm::value(foreign, ScalarType::Integer(u8_type()));
            assert!(certificate.verify().is_err());
        }

        #[test]
        fn affine_refold_gap_stays_uncovered() {
            let integer_type = u8_type();
            let value = ValueId::new(44).expect("value id");
            let place = ScalarTerm::value(value, ScalarType::Integer(integer_type));
            let place_math = IntegerMathTerm::MathValue {
                source_type: integer_type,
                value,
            };
            let context =
                PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))])
                    .expect("context");
            let scalar_bound = |bound| scalar_integer_term(integer_type, &BigInt::from_i64(bound));
            let assumptions = vec![
                Proposition::LessOrEqual(scalar_bound(1).expect("scalar"), place.clone()),
                Proposition::LessOrEqual(place.clone(), scalar_bound(128).expect("scalar")),
            ];
            assert!(
                refold_bounds_certificate(
                    place,
                    place_math,
                    integer_type,
                    BigInt::from_i64(1),
                    true,
                    assumptions,
                    (0, BigInt::from_i64(1)),
                    (1, BigInt::from_i64(128)),
                    context,
                    &integer_range(1, 127),
                    44,
                )
                .is_none()
            );
        }

        #[test]
        fn plan_measurement_records_the_kernel_receipt() {
            use crate::checker::measurement::ProofPlanMeasurements;
            use proof_admission::MathematicalCoreDecision;

            let certificate =
                closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 55)
                    .expect("certificate");
            let fact = certificate.verify().expect("kernel accepts");

            let mut measurements = ProofPlanMeasurements::default();
            measurements.record_certificate_verdict(super::CertificateVerdict::Certified);
            measurements.record_accepted_fact(&fact);

            let AcceptedFactRoute::CertificateDerived { acceptance, .. } = fact.route else {
                panic!("certificate-derived route");
            };
            let MathematicalCoreDecision::Judged(receipt) = acceptance.mathematical_core else {
                panic!("a closed literal certificate judges in the mathematical core");
            };
            assert_eq!(measurements.certificate_certified, 1);
            assert_eq!(measurements.certificate_attempts(), 1);
            assert_eq!(measurements.kernel_judgments, 1);
            assert_eq!(measurements.kernel_refusals, 0);
            assert_eq!(
                measurements.kernel_declarations,
                u64::from(receipt.declarations)
            );
            assert_eq!(
                measurements.kernel_assumption_closure,
                u64::from(receipt.assumption_closure)
            );
            assert_eq!(
                measurements.kernel_context_depth,
                u64::from(receipt.context_depth)
            );
            assert_eq!(
                measurements.kernel_arena_slots,
                u64::from(receipt.arena_slots)
            );
        }

        use crate::checker::derivation_cache::{DerivationConsultation, ProofDerivationCache};
        use crate::obligations::{ProofPlan, proof_obligation_key};

        #[test]
        fn kernel_rejected_candidates_are_passed_over_not_accepted() {
            let mut program = program();
            let (_base, obligation) = program.count_value();
            let plan = ProofPlan::new(&program.typed_trees);
            let mut cache = ProofDerivationCache::new();

            let forged =
                closed_bounds_certificate(literal_term(300), math_literal(0), math_literal(5), 63)
                    .expect("certificate builds; the kernel rejects it");
            DerivationConsultation::new(&plan, &obligation, &mut cache).retain(forged);
            let sound =
                closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 64)
                    .expect("certificate");
            DerivationConsultation::new(&plan, &obligation, &mut cache).retain(sound);

            let fact = DerivationConsultation::new(&plan, &obligation, &mut cache)
                .recheck()
                .expect("the sound candidate still discharges the leg");
            assert!(matches!(
                fact.route,
                AcceptedFactRoute::CertificateDerived { .. }
            ));
            let report = cache.report();
            assert_eq!(report.consultations, 1);
            assert_eq!(report.rejected_candidates, 1);
            assert_eq!(report.reused, 1);
        }

        #[test]
        fn capacity_refusal_is_explicit_and_keeps_the_verdict_path() {
            let mut program = program();
            let (_base, obligation) = program.count_value();
            let plan = ProofPlan::new(&program.typed_trees);
            let mut cache = ProofDerivationCache::with_capacity(0);

            let certificate =
                closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 65)
                    .expect("certificate");
            DerivationConsultation::new(&plan, &obligation, &mut cache).retain(certificate);

            assert!(cache.is_empty());
            assert_eq!(cache.report().capacity_refusals, 1);
        }

        #[test]
        fn dependency_change_misses_and_invalidate_reclaims_the_stale_row() {
            let mut program = program();
            let (base, count) = program.count_value();
            let plan = ProofPlan::new(&program.typed_trees);
            let mut cache = ProofDerivationCache::new();

            let certificate =
                closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 66)
                    .expect("certificate");
            DerivationConsultation::new(&plan, &count, &mut cache).retain(certificate);

            let moved = program.data_value(1, "count", base);
            assert!(
                DerivationConsultation::new(&plan, &moved, &mut cache)
                    .recheck()
                    .is_none()
            );

            let total = program.data_value(1, "total", base);
            let neighbor =
                closed_bounds_certificate(literal_term(3), math_literal(0), math_literal(10), 67)
                    .expect("certificate");
            DerivationConsultation::new(&plan, &total, &mut cache).retain(neighbor);

            let stale_key =
                proof_obligation_key(&plan, &count).expect("formed count obligation key");
            assert_eq!(cache.invalidate(&stale_key), 1);
            assert_eq!(cache.len(), 1);
            assert_eq!(cache.key_count(), 1);

            assert!(
                DerivationConsultation::new(&plan, &count, &mut cache)
                    .recheck()
                    .is_none()
            );
            assert!(
                DerivationConsultation::new(&plan, &total, &mut cache)
                    .recheck()
                    .is_some(),
                "the surviving row still discharges its leg"
            );
            let ghost = bounded_value(
                program.machine,
                "Main",
                program.int_type,
                "ghost",
                base,
                arena::HandleSpan::empty(),
            );
            assert_eq!(
                cache.invalidate(
                    &proof_obligation_key(&plan, &ghost).expect("formed ghost obligation key")
                ),
                0
            );

            let report = cache.report();
            assert_eq!(report.consultations, 3);
            assert_eq!(report.reused, 1);
            assert_eq!(report.retained, 2);
        }

        #[test]
        fn dependency_invalidation_drops_affected_entries_and_is_tallied() {
            let mut program = program();
            let (base, count_obligation) = program.count_value();
            let plan = ProofPlan::new(&program.typed_trees);
            let mut cache = ProofDerivationCache::new();

            let total_obligation = program.data_value(1, "total", base);
            DerivationConsultation::new(&plan, &count_obligation, &mut cache).retain(
                closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 66)
                    .expect("certificate"),
            );
            DerivationConsultation::new(&plan, &count_obligation, &mut cache).retain(
                closed_bounds_certificate(literal_term(6), math_literal(0), math_literal(10), 67)
                    .expect("certificate"),
            );
            DerivationConsultation::new(&plan, &total_obligation, &mut cache).retain(
                closed_bounds_certificate(literal_term(7), math_literal(0), math_literal(10), 68)
                    .expect("certificate"),
            );
            assert_eq!(cache.len(), 3);

            assert_eq!(
                cache.invalidate_where(|key| key.as_str().contains("::count")),
                2
            );
            assert_eq!(cache.len(), 1);
            assert_eq!(cache.report().invalidated, 2);

            let renamed = bounded_value(
                program.machine,
                "Renamed",
                program.data[0],
                "renamed",
                base,
                arena::HandleSpan::empty(),
            );
            assert!(
                DerivationConsultation::new(&plan, &renamed, &mut cache)
                    .recheck()
                    .is_none()
            );

            let total_again = program.data_value(1, "total", base);
            assert!(
                DerivationConsultation::new(&plan, &total_again, &mut cache)
                    .recheck()
                    .is_some()
            );
            let report = cache.report();
            assert_eq!(report.consultations, 2);
            assert_eq!(report.reused, 1);
        }
    }

    #[cfg(test)]
    mod coverage_tests {
        use super::{
            GuardBound, collect_contract_bounds, contract_refined_certificate,
            interval_premise_certificate,
        };
        use crate::checker::AssignmentRangeContext;
        use crate::integer_range;
        use crate::obligations::{BoundedStateReturnObligation, ProofConstraint, ProofPlan};
        use arena::HandleSpan;
        use numerics::bignum::BigInt;
        use numerics::literals::IntegerLiteral;
        use proof_admission::AcceptedFactRoute;
        use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::TypedTrees;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableNamePath,
        };
        use typed_trees::machine::Machine;
        use typed_trees::name::Identifier;
        use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
        use typed_trees::state::State;
        use typed_trees::statement::StatementNode;
        use typed_trees::statement::TransitionGuardNode;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        struct ReturnProgram {
            program: TypedTrees,
            machine: SymbolHandle,
            state: SymbolHandle,
            carrier: TypeReferenceHandle,
            value: ExpressionHandle,
            condition: ExpressionHandle,
        }

        fn name_path(program: &mut TypedTrees, symbol: SymbolHandle) -> TableNamePath {
            let mut members = HandleSpan::empty();
            program
                .expression_table
                .push_name_path_member(&mut members, Identifier::generated("pending"));
            let mut member_symbols = HandleSpan::empty();
            program
                .expression_table
                .push_name_path_member_symbol(&mut member_symbols, symbol);
            TableNamePath::new(members, member_symbols, symbol, symbol)
        }

        fn binary(
            program: &mut TypedTrees,
            left: ExpressionHandle,
            operator: BinaryOperator,
            right: ExpressionHandle,
        ) -> ExpressionHandle {
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator,
                    right,
                }))
        }

        fn return_program() -> ReturnProgram {
            let mut builder = SymbolTableBuilder::new();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let root_children = symbols::builtin_type_symbols()
                .into_iter()
                .chain([(SymbolKind::Machine, SymbolNameRef::Static("Main"))]);
            let handles =
                SymbolTableBuilder::child_handles(builder.insert_children(root, root_children))
                    .collect::<Vec<_>>();
            let u8_symbol = handles[symbols::BuiltinTypeAtom::U8.ordinal()];
            let machine = *handles.last().expect("machine");
            let state = SymbolTableBuilder::child_handles(builder.insert_children(
                machine,
                [(SymbolKind::State, SymbolNameRef::Static("entry"))],
            ))
            .next()
            .expect("state");
            let pending = SymbolTableBuilder::child_handles(builder.insert_children(
                state,
                [(SymbolKind::Parameter, SymbolNameRef::Static("pending"))],
            ))
            .next()
            .expect("parameter");

            let mut program = TypedTrees {
                symbols: builder.finish(),
                ..TypedTrees::default()
            };
            let carrier = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: u8_symbol,
                    name: Identifier::generated("u8"),
                });
            let pending_path = name_path(&mut program, pending);
            let value = program
                .expression_table
                .insert(ExpressionNode::Name(pending_path));
            let bound = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(200)));
            let condition = binary(&mut program, value, BinaryOperator::LessOrEqual, bound);

            let mut state_node = State {
                symbol: state,
                name: Identifier::generated("entry"),
                ..State::default()
            };
            program.push_state_parameter(
                &mut state_node,
                StateParameter {
                    symbol: pending,
                    name: Identifier::generated("pending"),
                    type_reference: carrier,
                    ..StateParameter::default()
                },
            );
            let statement = program
                .statement_table
                .insert(StatementNode::Expression(value));
            state_node.statement_nodes = HandleSpan::from_parts(statement, 1);
            let facts = program
                .proof_facts
                .insert_many([ProofFact::Expression(condition)]);
            program.push_state_contract(
                &mut state_node,
                SignatureContract {
                    kind: SignatureContractKind::Requires,
                    facts,
                    ..SignatureContract::default()
                },
            );
            let mut machine_node = Machine {
                symbol: machine,
                name: Identifier::generated("Main"),
                ..Machine::default()
            };
            program.push_machine_state(&mut machine_node, state_node);
            program.push_machine(machine_node);
            ReturnProgram {
                program,
                machine,
                state,
                carrier,
                value,
                condition,
            }
        }

        fn return_obligation(
            plan: &mut ProofPlan,
            fixture: &ReturnProgram,
        ) -> BoundedStateReturnObligation {
            let value_constraints =
                plan.type_constraints
                    .insert_many([ProofConstraint::IntegerRange {
                        minimum: BigInt::from_i64(0),
                        maximum: BigInt::from_i64(255),
                    }]);
            BoundedStateReturnObligation {
                machine_symbol: fixture.machine,
                machine: Identifier::generated("Main"),
                state_symbol: fixture.state,
                state: Identifier::generated("entry"),
                statement_index: 0,
                value: fixture.value,
                value_constraints,
                base_type: fixture.carrier,
                constraints: HandleSpan::empty(),
                binary_operands: None,
                guard: TransitionGuardNode::Always,
                refuted_exit_guards: Vec::new(),
            }
        }

        #[test]
        fn arrival_interval_certificate_verifies_under_the_kernel() {
            let fixture = return_program();
            let plan = ProofPlan::new(&fixture.program);
            let certificate = interval_premise_certificate(
                &plan,
                fixture.carrier,
                0,
                200,
                &integer_range(0, 220),
                71,
            )
            .expect("certificate");
            let fact = certificate.verify().expect("kernel accepts");
            assert!(matches!(
                fact.route,
                AcceptedFactRoute::CertificateDerived { .. }
            ));
        }

        #[test]
        fn arrival_interval_gap_stays_uncovered() {
            let fixture = return_program();
            let plan = ProofPlan::new(&fixture.program);
            assert!(
                interval_premise_certificate(
                    &plan,
                    fixture.carrier,
                    0,
                    255,
                    &integer_range(0, 10),
                    72,
                )
                .is_none()
            );
        }

        #[test]
        fn contract_condition_contributes_its_literal_facts() {
            let mut fixture = return_program();
            let less = fixture
                .program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(7)));
            let conjunct = binary(
                &mut fixture.program,
                fixture.value,
                BinaryOperator::GreaterOrEqual,
                less,
            );
            let both = binary(
                &mut fixture.program,
                conjunct,
                BinaryOperator::And,
                fixture.condition,
            );
            let plan = ProofPlan::new(&fixture.program);
            let mut facts: Vec<GuardBound> = Vec::new();
            collect_contract_bounds(&plan, fixture.value, both, &mut facts);
            assert!(
                matches!(
                    facts.as_slice(),
                    [GuardBound { bound: lower, lower: true }, GuardBound { bound: upper, lower: false }]
                        if *lower == BigInt::from_i64(7) && *upper == BigInt::from_i64(200)
                ),
                "pending >= 7 && pending <= 200 contributes `7 <= pending` and `pending <= 200`"
            );
        }

        #[test]
        fn contract_point_exclusion_contributes_no_fact() {
            let mut fixture = return_program();
            let five = fixture
                .program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(5)));
            let exclusion = binary(
                &mut fixture.program,
                fixture.value,
                BinaryOperator::NotEqual,
                five,
            );
            let equality = binary(
                &mut fixture.program,
                fixture.value,
                BinaryOperator::Equal,
                five,
            );
            let plan = ProofPlan::new(&fixture.program);
            let mut facts: Vec<GuardBound> = Vec::new();
            collect_contract_bounds(&plan, fixture.value, exclusion, &mut facts);
            assert!(facts.is_empty(), "a `!=` exclusion has no `<=` restatement");
            collect_contract_bounds(&plan, fixture.value, equality, &mut facts);
            assert_eq!(facts.len(), 2);
        }

        #[test]
        fn contract_refined_return_gap_stays_uncovered() {
            let fixture = return_program();
            let mut plan = ProofPlan::new(&fixture.program);
            let obligation = return_obligation(&mut plan, &fixture);
            let context = AssignmentRangeContext::new(&plan);
            let machine = &fixture.program.machines()[0];
            let state = &fixture.program.machine_states(machine)[0];
            assert!(
                contract_refined_certificate(
                    &plan,
                    &obligation,
                    machine,
                    state,
                    &context,
                    &integer_range(0, 100),
                    74,
                )
                .is_none(),
                "neither declared 255 nor the `pending <= 200` fact reaches a [0..=100] target"
            );
        }
    }
}
mod derivation_cache {
    use proof_admission::AcceptedFact;

    use crate::checker::certificate::BoundedValueCertificate;
    use crate::derivation_store::{DerivationStore, DerivationStoreFull};
    use crate::obligations::{
        ProofObligation, ProofObligationKey, ProofPlan, proof_obligation_key,
    };

    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct DerivationCacheReport {
        pub consultations: u64,
        pub reused: u64,
        pub rejected_candidates: u64,
        pub retained: u64,
        pub capacity_refusals: u64,
        pub invalidated: u64,
    }

    #[derive(Debug, Default)]
    pub struct ProofDerivationCache {
        store: DerivationStore<BoundedValueCertificate>,
        report: DerivationCacheReport,
    }

    impl ProofDerivationCache {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_capacity(capacity: usize) -> Self {
            Self {
                store: DerivationStore::with_capacity(capacity),
                report: DerivationCacheReport::default(),
            }
        }

        pub fn len(&self) -> usize {
            self.store.len()
        }

        pub fn is_empty(&self) -> bool {
            self.store.is_empty()
        }

        pub fn key_count(&self) -> usize {
            self.store.key_count()
        }

        pub fn report(&self) -> DerivationCacheReport {
            self.report
        }

        pub fn invalidate(&mut self, key: &ProofObligationKey) -> usize {
            let removed = self.store.invalidate(key);
            self.report.invalidated += removed as u64;
            removed
        }

        pub fn invalidate_where(
            &mut self,
            affected: impl FnMut(&ProofObligationKey) -> bool,
        ) -> usize {
            let removed = self.store.invalidate_where(affected);
            self.report.invalidated += removed as u64;
            removed
        }
    }

    pub(crate) struct DerivationConsultation<'a> {
        key: Option<ProofObligationKey>,
        cache: &'a mut ProofDerivationCache,
    }

    impl DerivationConsultation<'_> {
        pub(crate) fn new<'cache>(
            proof_plan: &ProofPlan<'_>,
            obligation: &ProofObligation,
            cache: &'cache mut ProofDerivationCache,
        ) -> DerivationConsultation<'cache> {
            DerivationConsultation {
                key: proof_obligation_key(proof_plan, obligation),
                cache,
            }
        }

        pub(crate) fn recheck(&mut self) -> Option<AcceptedFact> {
            self.cache.report.consultations += 1;
            let key = self.key.as_ref()?;
            let candidates: Vec<_> = self.cache.store.candidates(key).map(|(id, _)| id).collect();
            for id in candidates {
                let Some(candidate) = self.cache.store.derivation(id) else {
                    continue;
                };
                match candidate.verify() {
                    Ok(fact) => {
                        self.cache.report.reused += 1;
                        return Some(fact);
                    }
                    Err(_) => {
                        self.cache.report.rejected_candidates += 1;
                    }
                }
            }
            None
        }

        pub(crate) fn retain(&mut self, certificate: BoundedValueCertificate) {
            let Some(key) = self.key.clone() else {
                return;
            };
            match self.cache.store.store(key, certificate) {
                Ok(_) => {
                    self.cache.report.retained += 1;
                }
                Err(DerivationStoreFull) => {
                    self.cache.report.capacity_refusals += 1;
                }
            }
        }
    }
}
mod diagnostics {
    use crate::checker::float_ranges::float_range_display;
    use crate::obligations::{
        BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
        BoundedStateReturnObligation, BoundedTransitionArgumentObligation, FloatRange,
        IntegerRange, ProofPlan,
    };
    pub(crate) use diagnostics::Diagnostic;
    use typed_trees::expression::ExpressionHandle;

    pub(crate) fn cannot_prove_bounded_transition_integer(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
        target_range: IntegerRange,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove transition argument `{}` satisfies bounded parameter `{}` in `{}.{}`; expected {}..={}",
            expression_display_name(proof_plan, obligation.argument),
            obligation.parameter,
            obligation.machine,
            obligation.state,
            target_range.minimum,
            target_range.maximum
        ))
    }

    pub(crate) fn cannot_prove_bounded_assignment_integer(
        proof_plan: &ProofPlan,
        obligation: &BoundedAssignmentObligation,
        target_range: IntegerRange,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove assignment value `{}` satisfies bounded target `{}` in `{}.{}`; expected {}..={}",
            expression_display_name(proof_plan, obligation.value),
            expression_display_name(proof_plan, obligation.target),
            obligation.machine,
            obligation.state,
            target_range.minimum,
            target_range.maximum
        ))
    }

    pub(crate) fn cannot_prove_bounded_return_integer(
        proof_plan: &ProofPlan,
        obligation: &BoundedStateReturnObligation,
        target_range: IntegerRange,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove return value `{}` satisfies bounded return type in `{}.{}`; expected {}..={}",
            expression_display_name(proof_plan, obligation.value),
            obligation.machine,
            obligation.state,
            target_range.minimum,
            target_range.maximum
        ))
    }

    pub(crate) fn cannot_prove_bounded_initializer_integer(
        proof_plan: &ProofPlan,
        obligation: &BoundedInitializerObligation,
        target_range: IntegerRange,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove initializer `{}` satisfies the bounded value of {}; expected {}..={}",
            expression_display_name(proof_plan, obligation.value),
            obligation.owner,
            target_range.minimum,
            target_range.maximum
        ))
    }

    pub(crate) fn cannot_prove_bounded_transition_float(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
        target_range: FloatRange,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove transition argument `{}` satisfies bounded parameter `{}` in `{}.{}`; {}",
            expression_display_name(proof_plan, obligation.argument),
            obligation.parameter,
            obligation.machine,
            obligation.state,
            float_range_display(target_range)
        ))
    }

    pub(crate) fn cannot_prove_bounded_assignment_float(
        proof_plan: &ProofPlan,
        obligation: &BoundedAssignmentObligation,
        target_range: FloatRange,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove assignment value `{}` satisfies bounded target `{}` in `{}.{}`; {}",
            expression_display_name(proof_plan, obligation.value),
            expression_display_name(proof_plan, obligation.target),
            obligation.machine,
            obligation.state,
            float_range_display(target_range)
        ))
    }

    pub(crate) fn cannot_prove_bounded_return_float(
        proof_plan: &ProofPlan,
        obligation: &BoundedStateReturnObligation,
        target_range: FloatRange,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove return value `{}` satisfies bounded return type in `{}.{}`; {}",
            expression_display_name(proof_plan, obligation.value),
            obligation.machine,
            obligation.state,
            float_range_display(target_range)
        ))
    }

    pub(crate) fn cannot_prove_bounded_initializer_float(
        proof_plan: &ProofPlan,
        obligation: &BoundedInitializerObligation,
        target_range: FloatRange,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove initializer `{}` satisfies the bounded value of {}; {}",
            expression_display_name(proof_plan, obligation.value),
            obligation.owner,
            float_range_display(target_range)
        ))
    }

    pub(crate) fn cannot_prove_transition_named_constraint(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
        constraint: &str,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove transition argument `{}` satisfies `{}` for bounded parameter `{}` in `{}.{}`",
            expression_display_name(proof_plan, obligation.argument),
            constraint,
            obligation.parameter,
            obligation.machine,
            obligation.state
        ))
    }

    pub(crate) fn cannot_prove_assignment_named_constraint(
        proof_plan: &ProofPlan,
        obligation: &BoundedAssignmentObligation,
        constraint: &str,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove assignment value `{}` satisfies `{}` for bounded target `{}` in `{}.{}`",
            expression_display_name(proof_plan, obligation.value),
            constraint,
            expression_display_name(proof_plan, obligation.target),
            obligation.machine,
            obligation.state
        ))
    }

    pub(crate) fn cannot_prove_return_named_constraint(
        proof_plan: &ProofPlan,
        obligation: &BoundedStateReturnObligation,
        constraint: &str,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove return value `{}` satisfies `{}` for bounded return type in `{}.{}`",
            expression_display_name(proof_plan, obligation.value),
            constraint,
            obligation.machine,
            obligation.state
        ))
    }

    pub(crate) fn cannot_prove_initializer_named_constraint(
        proof_plan: &ProofPlan,
        obligation: &BoundedInitializerObligation,
        constraint: &str,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot prove initializer `{}` satisfies `{}` for the bounded value of {}",
            expression_display_name(proof_plan, obligation.value),
            constraint,
            obligation.owner
        ))
    }

    pub(crate) fn expression_display_name(
        proof_plan: &ProofPlan,
        expression: ExpressionHandle,
    ) -> String {
        proof_plan.program.expression_table.display_name(expression)
    }

    fn call_argument_target(obligation: &BoundedCallArgumentObligation) -> String {
        obligation
            .receiver
            .as_ref()
            .map(|receiver| format!("{receiver}.{}", obligation.target))
            .unwrap_or_else(|| obligation.target.to_string())
    }

    pub(crate) fn cannot_prove_bounded_call_integer(
        proof_plan: &ProofPlan,
        obligation: &BoundedCallArgumentObligation,
        target_range: IntegerRange,
    ) -> Diagnostic {
        let target = call_argument_target(obligation);

        Diagnostic::error(format!(
            "cannot prove call argument `{}` satisfies bounded parameter `{}` for `{}` in `{}.{}`; expected {}..={}",
            expression_display_name(proof_plan, obligation.argument),
            obligation.parameter,
            target,
            obligation.machine,
            obligation.state,
            target_range.minimum,
            target_range.maximum
        ))
    }

    pub(crate) fn cannot_prove_bounded_call_float(
        proof_plan: &ProofPlan,
        obligation: &BoundedCallArgumentObligation,
        target_range: FloatRange,
    ) -> Diagnostic {
        let target = call_argument_target(obligation);

        Diagnostic::error(format!(
            "cannot prove call argument `{}` satisfies bounded parameter `{}` for `{}` in `{}.{}`; {}",
            expression_display_name(proof_plan, obligation.argument),
            obligation.parameter,
            target,
            obligation.machine,
            obligation.state,
            float_range_display(target_range)
        ))
    }

    pub(crate) fn cannot_prove_call_named_constraint(
        proof_plan: &ProofPlan,
        obligation: &BoundedCallArgumentObligation,
        constraint: &str,
    ) -> Diagnostic {
        let target = call_argument_target(obligation);

        Diagnostic::error(format!(
            "cannot prove call argument `{}` satisfies `{}` for bounded parameter `{}` for `{}` in `{}.{}`",
            expression_display_name(proof_plan, obligation.argument),
            constraint,
            obligation.parameter,
            target,
            obligation.machine,
            obligation.state
        ))
    }
}
mod float_ranges {
    use super::assignment_stability::{collect_read_place_paths, resolved_writes_overlap_reads};
    use crate::checker::guards::{false_comparison_operand, unwrap_true_guard_condition};
    use crate::checker::integer_ranges::type_constraints;
    use crate::obligations::{
        BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
        BoundedStateReturnObligation, BoundedTransitionArgumentObligation, FloatRange,
        ProofConstraint, ProofPlan,
    };
    use crate::typed_expressions::{dehoisted_condition, dehoisted_operand, expression_table};
    use arena::HandleSpan;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
    };
    use typed_trees::statement::{StatementNode, TransitionGuardNode};
    use typed_trees::types::PrimitiveType;
    use validation::{
        CallFrameResolver, CheckingInput, canonical_place_from_expression, declared_place_type_raw,
        expression_result_type_reference, frame_paths_overlap,
        has_builtin_bound_expression_meaning, has_builtin_decomposed_guard_meaning,
        index_place_segment, is_single_segment_name_path, place_has_builtin_coordinates,
        sealed_float_comparison,
    };

    macro_rules! float_range_for {
        ($name:ident, $obligation:ty, $expression:ident, $constraints:ident) => {
            pub(crate) fn $name(
                proof_plan: &ProofPlan,
                obligation: &$obligation,
            ) -> Option<FloatRange> {
                float_range(
                    proof_plan,
                    obligation.machine_symbol,
                    obligation.state_symbol,
                    obligation.statement_index,
                    obligation.$expression,
                    obligation.$constraints,
                )
            }
        };
    }

    float_range_for!(
        float_range_for_transition_argument,
        BoundedTransitionArgumentObligation,
        argument,
        argument_constraints
    );
    float_range_for!(
        float_range_for_assignment,
        BoundedAssignmentObligation,
        value,
        value_constraints
    );
    float_range_for!(
        float_range_for_call_argument,
        BoundedCallArgumentObligation,
        argument,
        argument_constraints
    );
    float_range_for!(
        float_range_for_return_value,
        BoundedStateReturnObligation,
        value,
        value_constraints
    );

    fn float_range(
        proof_plan: &ProofPlan,
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        statement_index: usize,
        operand: ExpressionHandle,
        constraints: HandleSpan<ProofConstraint>,
    ) -> Option<FloatRange> {
        match proof_plan.program.expression_table.expression(operand) {
            ExpressionNode::Float(value) => {
                let value = finite_float_literal(value)?;
                Some(FloatRange::closed(value))
            }
            _ => float_range_at(
                proof_plan,
                machine_symbol,
                state_symbol,
                statement_index,
                operand,
            )
            .or_else(|| float_range_from_constraints(type_constraints(proof_plan, constraints))),
        }
    }

    fn float_statement_preserves(
        frames: &CallFrameResolver<'_>,
        machine: &typed_trees::machine::Machine,
        statement: &StatementNode,
        preserves: &impl Fn(Option<Vec<String>>) -> bool,
        allow_transition: bool,
    ) -> bool {
        if !preserves(frames.statement_value_may_write_paths(machine, statement)) {
            return false;
        }
        match statement {
            StatementNode::Assignment(_) => preserves(
                frames
                    .assignment_write_frame(machine, statement)
                    .into_complete_paths(),
            ),
            StatementNode::Call(call) => preserves(frames.may_write_paths(machine, call)),
            StatementNode::Transition(_) if allow_transition => true,
            StatementNode::LocalData(_) | StatementNode::Expression(_) => true,
            _ => false,
        }
    }

    fn float_range_at(
        proof_plan: &ProofPlan,
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<FloatRange> {
        let program = proof_plan.program;
        let (machine, state) = program.machine_state(machine_symbol, state_symbol)?;
        let identity = resolved_place_identity(program, expression)?;
        let place_type = declared_place_type_raw(program, machine, Some(state), expression)?;
        let primitive = program.primitive_type_reference(place_type)?;
        let format = match primitive {
            PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
            PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
            _ => return None,
        };
        let path =
            super::assignment_stability::written_place_path(proof_plan, expression)?.join(".");
        let frames = CallFrameResolver::new(program)?;
        let statements = program.statement_table.statements(state.statement_nodes);
        let current = statements.get(statement_index)?;
        let root = *identity.first()?;
        let reference_local = statements.iter().find_map(|statement| {
            let local = statement.as_local_data()?;
            if local.symbol != root {
                return None;
            }
            let mut reference = local.type_reference;
            while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
                program.type_reference_table.type_reference(reference)
            {
                reference = *base_type;
            }
            matches!(
                program.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Reference { .. }
            )
            .then_some(local.symbol)
        });
        let observed_origin = match reference_local {
            Some(local) => {
                Some(frames.local_reference_origin_before_statement(machine, current, local)?)
            }
            None => None,
        };
        let preserves = |written: Option<Vec<String>>| {
            written.is_some_and(|written| {
                written
                    .iter()
                    .all(|written| !frame_paths_overlap(&path, written))
            })
        };
        if !preserves(frames.statement_value_may_write_paths(machine, current)) {
            return None;
        }
        for statement in statements[..statement_index].iter().rev() {
            if let Some(local) = statement.as_local_data()
                && identity.as_slice() == [local.symbol]
            {
                if program.primitive_type_reference(local.type_reference) != Some(primitive) {
                    return None;
                }
                let ExpressionNode::Float(literal) =
                    program.expression_table.expression(local.initial_value)
                else {
                    return None;
                };
                return (literal.landing() == Some(format))
                    .then(|| finite_float_literal(literal))
                    .flatten()
                    .map(FloatRange::closed);
            }
            if let Some(assignment) = statement.as_assignment()
                && resolved_place_identity(program, assignment.target).as_ref() == Some(&identity)
            {
                if let Some(local) = reference_local
                    && frames.local_reference_origin_before_statement(machine, statement, local)
                        != observed_origin
                {
                    return None;
                }
                let target_type =
                    declared_place_type_raw(program, machine, Some(state), assignment.target)?;
                if program.primitive_type_reference(target_type) != Some(primitive) {
                    return None;
                }
                let ExpressionNode::Float(literal) =
                    program.expression_table.expression(assignment.value)
                else {
                    return None;
                };
                return (literal.landing() == Some(format))
                    .then(|| finite_float_literal(literal))
                    .flatten()
                    .map(FloatRange::closed);
            }
            if !float_statement_preserves(&frames, machine, statement, &preserves, false) {
                return None;
            }
        }
        None
    }

    fn resolved_place_identity(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<Vec<symbols::SymbolHandle>> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) if is_single_segment_name_path(program, path) => {
                Some(vec![path.symbol])
            }
            ExpressionNode::Member(member) if member.member_symbol.is_valid() => {
                let mut identity = resolved_place_identity(program, member.receiver)?;
                identity.push(member.member_symbol);
                Some(identity)
            }
            _ => None,
        }
    }

    pub(crate) fn guarded_float_range_for_transition_argument(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
    ) -> Option<FloatRange> {
        guarded_float_range(
            proof_plan,
            float_range_for_transition_argument(proof_plan, obligation),
            &GuardedOperand {
                machine_symbol: obligation.machine_symbol,
                state_symbol: obligation.state_symbol,
                statement_index: obligation.statement_index,
                argument: obligation.argument,
                argument_constraints: obligation.argument_constraints,
                guard: obligation.guard,
                refuted_exit_guards: &obligation.refuted_exit_guards,
            },
        )
    }

    pub(crate) fn guarded_float_range_for_return_value(
        proof_plan: &ProofPlan,
        obligation: &BoundedStateReturnObligation,
    ) -> Option<FloatRange> {
        guarded_float_range(
            proof_plan,
            float_range_for_return_value(proof_plan, obligation),
            &GuardedOperand {
                machine_symbol: obligation.machine_symbol,
                state_symbol: obligation.state_symbol,
                statement_index: obligation.statement_index,
                argument: obligation.value,
                argument_constraints: obligation.value_constraints,
                guard: obligation.guard,
                refuted_exit_guards: &obligation.refuted_exit_guards,
            },
        )
    }

    struct GuardedOperand<'obligation> {
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        statement_index: usize,
        argument: ExpressionHandle,
        argument_constraints: HandleSpan<ProofConstraint>,
        guard: TransitionGuardNode,
        refuted_exit_guards: &'obligation [ExpressionHandle],
    }

    fn guarded_float_range(
        proof_plan: &ProofPlan,
        base: Option<FloatRange>,
        obligation: &GuardedOperand<'_>,
    ) -> Option<FloatRange> {
        let program = proof_plan.program;
        let (machine, state) =
            program.machine_state(obligation.machine_symbol, obligation.state_symbol)?;
        let statements = program.statement_table.statements(state.statement_nodes);
        let current = statements.get(obligation.statement_index)?;
        let frames = CallFrameResolver::new(program)?;
        let mut reads = Vec::new();
        collect_read_place_paths(proof_plan, obligation.argument, &mut reads);
        collect_read_place_paths(
            proof_plan,
            dehoisted_operand(program, state, obligation.argument),
            &mut reads,
        );
        let preserves = |written: Option<Vec<String>>| {
            written.is_some_and(|written| !resolved_writes_overlap_reads(&written, &reads))
        };
        if !preserves(frames.statement_value_may_write_paths(machine, current)) {
            return None;
        }
        let mut range = FloatGuardRange {
            ordered: base.unwrap_or(FloatRange {
                minimum: f64::NEG_INFINITY,
                maximum: f64::INFINITY,
                maximum_inclusive: true,
            }),
            may_be_nan: base.is_none()
                && !type_constraints(proof_plan, obligation.argument_constraints)
                    .iter()
                    .any(|constraint| {
                        matches!(constraint,
                    ProofConstraint::Named(name) if name.as_str() == "finite")
                    }),
        };
        range = apply_float_guard(
            proof_plan,
            range,
            obligation.argument,
            &obligation.guard,
            obligation.machine_symbol,
            obligation.state_symbol,
            obligation.statement_index,
        );
        for statement in statements[..obligation.statement_index].iter().rev() {
            if !preserves(frames.statement_value_may_write_paths(machine, statement)) {
                break;
            }
            match statement {
                StatementNode::Assignment(_) => {
                    if !preserves(
                        frames
                            .assignment_write_frame(machine, statement)
                            .into_complete_paths(),
                    ) {
                        break;
                    }
                }
                StatementNode::Call(call) => {
                    if !preserves(frames.may_write_paths(machine, call)) {
                        break;
                    }
                }
                StatementNode::Transition(transition) => {
                    if let TransitionGuardNode::When(condition) = transition.guard
                        && obligation.refuted_exit_guards.contains(&condition)
                    {
                        range = apply_float_condition(
                            proof_plan,
                            range,
                            obligation.argument,
                            condition,
                            obligation.machine_symbol,
                            obligation.state_symbol,
                            obligation.statement_index,
                            true,
                        );
                    }
                }
                StatementNode::LocalData(_) | StatementNode::Expression(_) => {}
                _ => break,
            }
        }
        (!range.may_be_nan && inhabited(range.ordered)).then_some(range.ordered)
    }

    #[derive(Clone, Copy)]
    struct FloatGuardRange {
        ordered: FloatRange,
        may_be_nan: bool,
    }

    fn inhabited(range: FloatRange) -> bool {
        range.minimum < range.maximum || (range.minimum == range.maximum && range.maximum_inclusive)
    }

    fn apply_float_guard(
        proof_plan: &ProofPlan,
        range: FloatGuardRange,
        argument: ExpressionHandle,
        guard: &TransitionGuardNode,
        machine_symbol: symbols::SymbolHandle,
        source_state: symbols::SymbolHandle,
        statement_index: usize,
    ) -> FloatGuardRange {
        match guard {
            TransitionGuardNode::Always => range,
            TransitionGuardNode::When(condition) => apply_float_condition(
                proof_plan,
                range,
                argument,
                *condition,
                machine_symbol,
                source_state,
                statement_index,
                false,
            ),
        }
    }

    fn apply_float_condition(
        proof_plan: &ProofPlan,
        range: FloatGuardRange,
        argument: ExpressionHandle,
        condition: ExpressionHandle,
        machine_symbol: symbols::SymbolHandle,
        source_state: symbols::SymbolHandle,
        statement_index: usize,
        refute: bool,
    ) -> FloatGuardRange {
        let condition = dehoisted_guard_condition(
            proof_plan,
            machine_symbol,
            source_state,
            statement_index,
            condition,
        );
        if let ExpressionNode::Unary(unary) =
            proof_plan.program.expression_table.expression(condition)
            && unary.operator == UnaryOperator::LogicalNot
        {
            return apply_float_condition(
                proof_plan,
                range,
                argument,
                unary.operand,
                machine_symbol,
                source_state,
                statement_index,
                !refute,
            );
        }
        if let Some(operand) = false_comparison_operand(proof_plan, condition)
            && let Some((machine, state)) = proof_plan
                .program
                .machine_state(machine_symbol, source_state)
            && has_builtin_decomposed_guard_meaning(
                proof_plan.program,
                machine,
                Some(state),
                condition,
            )
        {
            return apply_float_condition(
                proof_plan,
                range,
                argument,
                operand,
                machine_symbol,
                source_state,
                statement_index,
                !refute,
            );
        }
        let Some(binary) = proof_plan.program.expression_table.binary_node(condition) else {
            return range;
        };
        if binary.operator == BinaryOperator::And {
            if refute {
                return range;
            }
            let range = apply_float_condition(
                proof_plan,
                range,
                argument,
                binary.left,
                machine_symbol,
                source_state,
                statement_index,
                false,
            );
            return apply_float_condition(
                proof_plan,
                range,
                argument,
                binary.right,
                machine_symbol,
                source_state,
                statement_index,
                false,
            );
        }
        let Some((machine, state)) = proof_plan
            .program
            .machine_state(machine_symbol, source_state)
        else {
            return range;
        };
        if !has_builtin_bound_expression_meaning(
            proof_plan.program,
            machine,
            Some(state),
            condition,
        ) && !sealed_float_comparison(
            proof_plan.program,
            machine,
            Some(state),
            condition,
            binary,
        ) {
            return range;
        }
        apply_float_comparison(
            proof_plan,
            range,
            argument,
            binary.operator,
            binary.left,
            binary.right,
            machine_symbol,
            source_state,
            statement_index,
            refute,
        )
    }

    fn apply_float_comparison(
        proof_plan: &ProofPlan,
        mut range: FloatGuardRange,
        argument: ExpressionHandle,
        operator: BinaryOperator,
        left: ExpressionHandle,
        right: ExpressionHandle,
        machine_symbol: symbols::SymbolHandle,
        source_state: symbols::SymbolHandle,
        statement_index: usize,
        refute: bool,
    ) -> FloatGuardRange {
        let Some((machine, state)) = proof_plan
            .program
            .machine_state(machine_symbol, source_state)
        else {
            return range;
        };
        let dehoist =
            |operand| stable_scalar_operand(proof_plan, machine, state, statement_index, operand);
        let (state_argument, state_left, state_right) =
            (dehoist(argument), dehoist(left), dehoist(right));
        let same_argument = |expression| {
            same_float_place(
                proof_plan,
                machine_symbol,
                source_state,
                statement_index,
                state_argument,
                expression,
            )
        };
        let on_left = same_argument(state_left);
        let on_right = same_argument(state_right);
        if on_left && on_right {
            if matches!(
                (operator, refute),
                (
                    BinaryOperator::Equal
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::GreaterOrEqual,
                    false
                ) | (BinaryOperator::NotEqual, true)
            ) {
                range.may_be_nan = false;
            }
            return range;
        }
        let (operator, literal) = if on_left {
            (operator, float_literal_value(proof_plan, state_right))
        } else if on_right {
            (
                flip_comparison(operator),
                float_literal_value(proof_plan, state_left),
            )
        } else {
            return range;
        };
        let Some(literal) = literal else {
            return range;
        };
        let operator = if refute {
            match operator {
                BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
                BinaryOperator::LessOrEqual => BinaryOperator::Greater,
                BinaryOperator::Greater => BinaryOperator::LessOrEqual,
                BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
                BinaryOperator::NotEqual => {
                    range.may_be_nan = false;
                    BinaryOperator::Equal
                }
                _ => return range,
            }
        } else {
            if !matches!(
                operator,
                BinaryOperator::Equal
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Less
            ) {
                return range;
            }
            range.may_be_nan = false;
            operator
        };
        match operator {
            BinaryOperator::Equal => {
                raise_minimum(&mut range.ordered, literal);
                cap_maximum(&mut range.ordered, literal, true);
            }
            BinaryOperator::GreaterOrEqual => raise_minimum(&mut range.ordered, literal),
            BinaryOperator::Greater => {
                if literal == f64::INFINITY {
                    range.ordered = empty_float_range();
                } else {
                    raise_minimum(&mut range.ordered, literal.next_up());
                }
            }
            BinaryOperator::LessOrEqual => cap_maximum(&mut range.ordered, literal, true),
            BinaryOperator::Less => {
                if literal == f64::NEG_INFINITY {
                    range.ordered = empty_float_range();
                } else {
                    cap_maximum(&mut range.ordered, literal, false);
                }
            }
            _ => {}
        }
        range
    }

    fn flip_comparison(operator: BinaryOperator) -> BinaryOperator {
        match operator {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            other => other,
        }
    }

    fn dehoisted_guard_condition(
        proof_plan: &ProofPlan,
        machine_symbol: symbols::SymbolHandle,
        source_state: symbols::SymbolHandle,
        statement_index: usize,
        mut condition: ExpressionHandle,
    ) -> ExpressionHandle {
        let Some((machine, state)) = proof_plan
            .program
            .machine_state(machine_symbol, source_state)
        else {
            return condition;
        };
        let mut previous_declaration = statement_index;
        loop {
            let unwrapped = unwrap_true_guard_condition(proof_plan, condition);
            if unwrapped != condition {
                if !has_builtin_decomposed_guard_meaning(
                    proof_plan.program,
                    machine,
                    Some(state),
                    condition,
                ) {
                    return condition;
                }
                condition = unwrapped;
            }
            let Some((declaration, initializer)) =
                stable_initializer(proof_plan, machine, state, statement_index, condition)
            else {
                return condition;
            };
            if declaration >= previous_declaration {
                return condition;
            }
            previous_declaration = declaration;
            condition = initializer;
        }
    }

    fn stable_scalar_operand(
        proof_plan: &ProofPlan,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        mut operand: ExpressionHandle,
    ) -> ExpressionHandle {
        let mut previous_declaration = statement_index;
        loop {
            let Some((declaration, initializer)) =
                stable_initializer(proof_plan, machine, state, statement_index, operand)
            else {
                return operand;
            };
            if declaration >= previous_declaration
                || !matches!(
                    proof_plan.program.expression_table.expression(initializer),
                    ExpressionNode::Name(_)
                        | ExpressionNode::Member(_)
                        | ExpressionNode::Indexed(_)
                        | ExpressionNode::Integer(_)
                        | ExpressionNode::Float(_)
                )
            {
                return operand;
            }
            previous_declaration = declaration;
            operand = initializer;
        }
    }

    fn stable_initializer(
        proof_plan: &ProofPlan,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<(usize, ExpressionHandle)> {
        let program = proof_plan.program;
        let initializer = dehoisted_condition(program, state, expression);
        if initializer == expression {
            return None;
        }
        let name = program.expression_table.name_path_node(expression)?;
        let (declaration, _) = program.state_local_declaration(state, name.symbol)?;
        if declaration >= statement_index {
            return None;
        }
        let mut reads = Vec::new();
        collect_read_place_paths(proof_plan, expression, &mut reads);
        collect_read_place_paths(proof_plan, initializer, &mut reads);
        let frames = CallFrameResolver::new(program)?;
        let preserves = |written: Option<Vec<String>>| {
            written.is_some_and(|written| !resolved_writes_overlap_reads(&written, &reads))
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        for statement in statements.get(declaration + 1..=statement_index)? {
            if !float_statement_preserves(&frames, machine, statement, &preserves, true) {
                return None;
            }
        }
        Some((declaration, initializer))
    }

    fn same_float_place(
        proof_plan: &ProofPlan,
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        statement_index: usize,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let program = proof_plan.program;
        let Some((machine, state)) = program.machine_state(machine_symbol, state_symbol) else {
            return false;
        };
        let observed_primitive = |expression| {
            expression_result_type_reference(program, machine, state, expression)
                .or_else(|| declared_place_type_raw(program, machine, Some(state), expression))
                .and_then(|reference| program.primitive_type_reference(reference))
        };
        let left_primitive = observed_primitive(left);
        if !matches!(
            left_primitive,
            Some(PrimitiveType::F32 | PrimitiveType::F64)
        ) || left_primitive != observed_primitive(right)
        {
            return false;
        }
        let canonical = |expression| {
            if !place_has_builtin_coordinates(program, machine, Some(state), expression) {
                return None;
            }
            let mut place = canonical_place_from_expression(program, expression)?;
            for segment in &mut place.segments {
                if let facts::PlaceSegment::Index { expression } = segment {
                    let operand = stable_scalar_operand(
                        proof_plan,
                        machine,
                        state,
                        statement_index,
                        *expression,
                    );
                    *segment = index_place_segment(program, operand);
                }
            }
            Some(place)
        };
        let (Some(left), Some(right)) = (canonical(left), canonical(right)) else {
            return false;
        };
        left.root == right.root
            && left.segments.len() == right.segments.len()
            && left
                .segments
                .iter()
                .zip(&right.segments)
                .all(|(left, right)| match (left, right) {
                    (
                        facts::PlaceSegment::Index { expression: left },
                        facts::PlaceSegment::Index { expression: right },
                    ) => {
                        has_builtin_bound_expression_meaning(program, machine, Some(state), *left)
                            && has_builtin_bound_expression_meaning(
                                program,
                                machine,
                                Some(state),
                                *right,
                            )
                            && expression_table(program)
                                .expressions_structurally_equal(*left, *right)
                    }
                    _ => left == right,
                })
    }

    fn float_literal_value(proof_plan: &ProofPlan, expression: ExpressionHandle) -> Option<f64> {
        let ExpressionNode::Float(value) =
            proof_plan.program.expression_table.expression(expression)
        else {
            return None;
        };
        let value = value.landed_f64();
        (!value.is_nan()).then_some(value)
    }

    fn raise_minimum(range: &mut FloatRange, bound: f64) {
        if bound > range.minimum {
            range.minimum = bound;
        }
    }

    fn cap_maximum(range: &mut FloatRange, bound: f64, inclusive: bool) {
        if bound < range.maximum {
            range.maximum = bound;
            range.maximum_inclusive = inclusive;
        } else if bound == range.maximum {
            range.maximum_inclusive = range.maximum_inclusive && inclusive;
        }
    }

    fn empty_float_range() -> FloatRange {
        FloatRange {
            minimum: 1.0,
            maximum: 0.0,
            maximum_inclusive: true,
        }
    }

    pub(crate) fn float_range_for_initializer(
        proof_plan: &ProofPlan,
        obligation: &BoundedInitializerObligation,
    ) -> Option<FloatRange> {
        match expression_table(proof_plan.program).expression(obligation.value) {
            ExpressionNode::Float(value) => {
                let value = finite_float_literal(value)?;
                Some(FloatRange::closed(value))
            }
            _ => obligation
                .owner
                .state_statement()
                .and_then(|(machine_symbol, state_symbol, statement_index)| {
                    float_range_at(
                        proof_plan,
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        obligation.value,
                    )
                })
                .or_else(|| {
                    float_range_from_constraints(type_constraints(
                        proof_plan,
                        obligation.value_constraints,
                    ))
                }),
        }
    }

    pub(crate) fn float_range_from_constraints(
        constraints: &[ProofConstraint],
    ) -> Option<FloatRange> {
        crate::obligations::float_range_from_constraints(constraints)
    }

    pub(crate) fn finite_float_literal(
        value: &typed_trees::expression::FloatLiteral,
    ) -> Option<f64> {
        let value = value.landed_f64();
        value.is_finite().then_some(value)
    }

    pub(crate) fn float_range_display(range: FloatRange) -> String {
        if range.maximum_inclusive {
            format!("expected {}..={}", range.minimum, range.maximum)
        } else {
            format!("expected {}..{}", range.minimum, range.maximum)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::float_range_at;
        use crate::obligations::{FloatRange, ProofPlan};
        use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::TypedTrees;
        use typed_trees::data::{DataDefinition, DataField, DataMember};
        use typed_trees::expression::{
            Expression, ExpressionHandle, ExpressionNode, FloatLiteral, NamePath,
            TableCallExpression, TableMemberExpression,
        };
        use typed_trees::machine::Machine;
        use typed_trees::name::Identifier;
        use typed_trees::signature::StateParameter;
        use typed_trees::state::State;
        use typed_trees::statement::{StatementNode, TableAssignment, TableCall, TableLocalData};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        struct Fixture {
            program: TypedTrees,
            machine: Machine,
            state: State,
            root: ExpressionHandle,
            value: ExpressionHandle,
            other: ExpressionHandle,
            mutator: SymbolHandle,
            alias: SymbolHandle,
            replacement: ExpressionHandle,
            reference_type: TypeReferenceHandle,
        }

        impl Fixture {
            fn new() -> Self {
                let mut builder = SymbolTableBuilder::new();
                let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
                let children = SymbolTableBuilder::child_handles(builder.insert_children(
                    root,
                    symbols::builtin_type_symbols().into_iter().chain([
                        (SymbolKind::Data, SymbolNameRef::Static("Sample")),
                        (SymbolKind::Machine, SymbolNameRef::Static("update")),
                        (SymbolKind::Machine, SymbolNameRef::Static("mutate")),
                    ]),
                ))
                .collect::<Vec<_>>();
                let data_symbol = children[children.len() - 3];
                let machine_symbol = children[children.len() - 2];
                let mutator_machine = children[children.len() - 1];
                let fields = SymbolTableBuilder::child_handles(builder.insert_children(
                    data_symbol,
                    [
                        (SymbolKind::Field, SymbolNameRef::Static("value")),
                        (SymbolKind::Field, SymbolNameRef::Static("other")),
                    ],
                ))
                .collect::<Vec<_>>();
                let state_symbol = SymbolTableBuilder::child_handles(builder.insert_children(
                    machine_symbol,
                    [(SymbolKind::State, SymbolNameRef::Static("entry"))],
                ))
                .next()
                .unwrap();
                let mutator = SymbolTableBuilder::child_handles(builder.insert_children(
                    mutator_machine,
                    [(SymbolKind::State, SymbolNameRef::Static("entry"))],
                ))
                .next()
                .unwrap();
                let target_parameter = SymbolTableBuilder::child_handles(builder.insert_children(
                    mutator,
                    [(SymbolKind::Parameter, SymbolNameRef::Static("target"))],
                ))
                .next()
                .unwrap();
                let bindings = SymbolTableBuilder::child_handles(builder.insert_children(
                    state_symbol,
                    [
                        (SymbolKind::Parameter, SymbolNameRef::Static("sample")),
                        (SymbolKind::Parameter, SymbolNameRef::Static("replacement")),
                        (SymbolKind::Local, SymbolNameRef::Static("alias")),
                    ],
                ))
                .collect::<Vec<_>>();
                let parameter = bindings[0];
                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..TypedTrees::default()
                };
                let float_type = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: children[symbols::BuiltinTypeAtom::F32.ordinal()],
                        name: Identifier::generated("f32"),
                    });
                let mut definition = DataDefinition {
                    symbol: data_symbol,
                    name: Identifier::generated("Sample"),
                    ..DataDefinition::default()
                };
                for (name, symbol) in [("value", fields[0]), ("other", fields[1])] {
                    program.push_data_member(
                        &mut definition,
                        DataMember::Field(DataField {
                            symbol,
                            name: Identifier::generated(name),
                            type_reference: float_type,
                            ..DataField::default()
                        }),
                    );
                }
                program.push_data_definition(definition);
                let data_type = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: data_symbol,
                        name: Identifier::generated("Sample"),
                    });
                let reference_type =
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::Reference {
                            referee: data_type,
                            access: language_core::ReferenceAccess::Mutable,
                            lifetime: None,
                        });
                let mut state = State {
                    symbol: state_symbol,
                    name: Identifier::generated("entry"),
                    ..State::default()
                };
                program.push_state_parameter(
                    &mut state,
                    StateParameter {
                        symbol: parameter,
                        name: Identifier::generated("sample"),
                        type_reference: reference_type,
                        ..StateParameter::default()
                    },
                );
                program.push_state_parameter(
                    &mut state,
                    StateParameter {
                        symbol: bindings[1],
                        name: Identifier::generated("replacement"),
                        type_reference: reference_type,
                        ..StateParameter::default()
                    },
                );
                let replacement =
                    program
                        .expression_table
                        .insert_tree(&Expression::Name(NamePath::resolved(
                            vec![Identifier::generated("replacement")],
                            bindings[1],
                            bindings[1],
                        )));
                let root =
                    program
                        .expression_table
                        .insert_tree(&Expression::Name(NamePath::resolved(
                            vec![Identifier::generated("sample")],
                            parameter,
                            parameter,
                        )));
                let mut member = |symbol, name| {
                    program
                        .expression_table
                        .insert(ExpressionNode::Member(TableMemberExpression {
                            receiver: root,
                            member_symbol: symbol,
                            member: Identifier::generated(name),
                            case_variant: None,
                        }))
                };
                let value = member(fields[0], "value");
                let other = member(fields[1], "other");
                let target =
                    program
                        .expression_table
                        .insert_tree(&Expression::Name(NamePath::resolved(
                            vec![Identifier::generated("target")],
                            target_parameter,
                            target_parameter,
                        )));
                let target_field = program.expression_table.insert(ExpressionNode::Member(
                    TableMemberExpression {
                        receiver: target,
                        member_symbol: fields[0],
                        member: Identifier::generated("value"),
                        case_variant: None,
                    },
                ));
                let poison = program.expression_table.insert(ExpressionNode::Float(
                    FloatLiteral::parse("2.0f32").unwrap(),
                ));
                let mut mutator_state = State {
                    symbol: mutator,
                    name: Identifier::generated("entry"),
                    return_type: float_type,
                    ..State::default()
                };
                program.push_state_parameter(
                    &mut mutator_state,
                    StateParameter {
                        symbol: target_parameter,
                        name: Identifier::generated("target"),
                        type_reference: reference_type,
                        ..StateParameter::default()
                    },
                );
                program.statement_table.push_statement(
                    &mut mutator_state.statement_nodes,
                    StatementNode::Assignment(TableAssignment {
                        target: target_field,
                        value: poison,
                        ..TableAssignment::default()
                    }),
                );
                program.statement_table.push_statement(
                    &mut mutator_state.statement_nodes,
                    StatementNode::Expression(poison),
                );
                let mut mutator_definition = Machine {
                    symbol: mutator_machine,
                    name: Identifier::generated("mutate"),
                    body_is_present: true,
                    ..Machine::default()
                };
                program.push_machine_state(&mut mutator_definition, mutator_state);
                program.push_machine(mutator_definition);
                Self {
                    program,
                    state,
                    root,
                    value,
                    other,
                    mutator,
                    alias: bindings[2],
                    replacement,
                    reference_type,
                    machine: Machine {
                        symbol: machine_symbol,
                        name: Identifier::generated("update"),
                        body_is_present: true,
                        ..Machine::default()
                    },
                }
            }

            fn push(&mut self, statement: StatementNode) {
                self.program
                    .statement_table
                    .push_statement(&mut self.state.statement_nodes, statement);
            }

            fn store(&mut self, target: ExpressionHandle, value: &str) {
                let value = self
                    .program
                    .expression_table
                    .insert(ExpressionNode::Float(FloatLiteral::parse(value).unwrap()));
                self.push(StatementNode::Assignment(TableAssignment {
                    target,
                    value,
                    ..TableAssignment::default()
                }));
            }

            fn range(&self, statement_index: usize) -> Option<FloatRange> {
                let mut program = self.program.clone();
                let mut machine = self.machine.clone();
                program.push_machine_state(&mut machine, self.state.clone());
                program.push_machine(machine);
                float_range_at(
                    &ProofPlan::new(&program),
                    self.machine.symbol,
                    self.state.symbol,
                    statement_index,
                    self.value,
                )
            }

            fn mutating_call(&mut self) -> TableCall {
                TableCall {
                    authored_machine_application: Default::default(),
                    target_symbol: self.mutator,
                    target: Identifier::generated("mutate"),
                    arguments: self
                        .program
                        .statement_table
                        .insert_expression_handles([self.root]),
                    ..TableCall::default()
                }
            }
        }

        #[test]
        fn reaching_float_store_uses_latest_repair_at_the_observation() {
            let mut fixture = Fixture::new();
            fixture.store(fixture.value, "2.0f32");
            fixture.store(fixture.value, "0.5f32");
            fixture.store(fixture.other, "9.0f32");
            fixture.push(StatementNode::Expression(fixture.value));
            fixture.store(fixture.value, "7.0f32");
            assert_eq!(fixture.range(3), Some(FloatRange::closed(0.5)));
            assert_eq!(fixture.range(4), Some(FloatRange::closed(0.5)));
        }

        #[test]
        fn reaching_float_store_does_not_borrow_a_forward_repair_or_wrong_format() {
            let mut fixture = Fixture::new();
            fixture.push(StatementNode::Expression(fixture.value));
            fixture.store(fixture.value, "0.5f32");
            assert_eq!(fixture.range(0), None);
            for spelling in ["0.5f64", "0.5", "1.0e50f32"] {
                fixture.store(fixture.value, spelling);
                fixture.push(StatementNode::Expression(fixture.value));
                assert_eq!(
                    fixture.range(fixture.state.statement_nodes.len() - 1),
                    None,
                    "{spelling}"
                );
            }
        }

        #[test]
        fn reaching_float_store_loses_unknown_parent_and_call_writes() {
            for invalidation in 0..3 {
                let mut fixture = Fixture::new();
                fixture.store(fixture.value, "0.5f32");
                let statement = match invalidation {
                    0 => StatementNode::Assignment(TableAssignment {
                        target: fixture.value,
                        value: fixture.other,
                        ..TableAssignment::default()
                    }),
                    1 => StatementNode::Assignment(TableAssignment {
                        target: fixture.root,
                        value: fixture.root,
                        ..TableAssignment::default()
                    }),
                    _ => StatementNode::Call(fixture.mutating_call()),
                };
                fixture.push(statement);
                fixture.push(StatementNode::Expression(fixture.value));
                assert_eq!(fixture.range(2), None, "invalidation {invalidation}");
            }
        }

        #[test]
        fn reaching_float_store_checks_sibling_argument_effects() {
            let mut fixture = Fixture::new();
            fixture.store(fixture.value, "0.5f32");
            let mutating_arguments = fixture
                .program
                .expression_table
                .insert_expression_handles([fixture.root]);
            let mutation = fixture
                .program
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    authored_call_selection: None,
                    authored_call_binding: None,
                    authored_machine_application: Default::default(),
                    requirement_application: Default::default(),
                    exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(),
                    receiver: ExpressionHandle::invalid(),
                    target_symbol: fixture.mutator,
                    static_machine_parameter: SymbolHandle::invalid(),
                    target: Identifier::generated("mutate"),
                    static_requirement_dispatch: None,
                    layout_query: None,
                    native_code_layout: None,
                    build_selection: Default::default(),
                    machine_arguments: Box::default(),
                    quotient_operation: None,
                    private_layout_operation: None,
                    installed_interrupt_operation: None,
                    arguments: mutating_arguments,
                    evidence_arguments: Box::default(),
                    operational_acknowledgement: Default::default(),
                }));
            let arguments = fixture
                .program
                .statement_table
                .insert_expression_handles([fixture.value, mutation]);
            fixture.push(StatementNode::Call(TableCall {
                authored_machine_application: Default::default(),
                arguments,
                ..TableCall::default()
            }));
            assert_eq!(fixture.range(1), None);
        }

        #[test]
        fn reaching_float_store_invalidates_alias_rebinding_and_origin_writes() {
            for rebind in [false, true] {
                let mut fixture = Fixture::new();
                let original_value = fixture.value;
                let alias = fixture
                    .program
                    .expression_table
                    .insert_tree(&Expression::Name(NamePath::resolved(
                        vec![Identifier::generated("alias")],
                        fixture.alias,
                        fixture.alias,
                    )));
                let Some(original) = fixture.program.expression_table.member_node(original_value)
                else {
                    panic!("field place");
                };
                let original = original.clone();
                fixture.value = fixture
                    .program
                    .expression_table
                    .insert(ExpressionNode::Member(TableMemberExpression {
                        receiver: alias,
                        ..original
                    }));
                fixture.push(StatementNode::LocalData(TableLocalData {
                    symbol: fixture.alias,
                    name: Identifier::generated("alias"),
                    type_reference: fixture.reference_type,
                    initial_value: fixture.root,
                    is_mutable: true,
                    ..TableLocalData::default()
                }));
                fixture.store(fixture.value, "0.5f32");
                fixture.push(StatementNode::Expression(fixture.value));
                assert_eq!(fixture.range(2), Some(FloatRange::closed(0.5)));
                if rebind {
                    fixture.push(StatementNode::Assignment(TableAssignment {
                        target: alias,
                        value: fixture.replacement,
                        ..TableAssignment::default()
                    }));
                } else {
                    fixture.store(original_value, "2.0f32");
                }
                fixture.push(StatementNode::Expression(fixture.value));
                assert_eq!(fixture.range(4), None, "rebind={rebind}");
            }
        }
    }
}
mod guards {
    use crate::checker::integer_ranges::{
        apply_handle_condition_complement, integer_literal_handle,
    };
    use crate::obligations::{IntegerRange, ProofPlan};
    use crate::typed_expressions::{dehoisted_condition, dehoisted_operand, expression_table};
    use numerics::bignum::BigInt;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
    };
    use typed_trees::statement::TransitionGuardNode;

    pub(crate) fn apply_handle_guard(
        proof_plan: &ProofPlan,
        range: IntegerRange,
        argument: ExpressionHandle,
        guard: &TransitionGuardNode,
    ) -> IntegerRange {
        match guard {
            TransitionGuardNode::Always => range,
            TransitionGuardNode::When(condition) => {
                apply_handle_condition(proof_plan, range, argument, *condition)
            }
        }
    }

    pub(crate) fn apply_assignment_guard(
        proof_plan: &ProofPlan,
        range: IntegerRange,
        value: ExpressionHandle,
        guard: &TransitionGuardNode,
    ) -> IntegerRange {
        let range = apply_handle_guard(proof_plan, range, value, guard);

        let Some(value_binary) = proof_plan.program.expression_table.binary_node(value) else {
            return range;
        };
        if value_binary.operator != BinaryOperator::Subtract {
            return range;
        }

        let TransitionGuardNode::When(condition) = guard else {
            return range;
        };
        let condition = unwrap_true_guard_condition(proof_plan, *condition);
        let Some(condition_binary) = proof_plan.program.expression_table.binary_node(condition)
        else {
            return range;
        };

        let lower_bound = if expressions_equivalent_for_proof(
            proof_plan,
            value_binary.left,
            condition_binary.left,
        ) && expressions_equivalent_for_proof(
            proof_plan,
            value_binary.right,
            condition_binary.right,
        ) {
            match condition_binary.operator {
                BinaryOperator::Greater => Some(1),
                BinaryOperator::GreaterOrEqual => Some(0),
                _ => None,
            }
        } else if expressions_equivalent_for_proof(
            proof_plan,
            value_binary.left,
            condition_binary.right,
        ) && expressions_equivalent_for_proof(
            proof_plan,
            value_binary.right,
            condition_binary.left,
        ) {
            match condition_binary.operator {
                BinaryOperator::Less => Some(1),
                BinaryOperator::LessOrEqual => Some(0),
                _ => None,
            }
        } else {
            None
        };

        let Some(lower_bound) = lower_bound else {
            return range;
        };

        IntegerRange {
            minimum: range.minimum.max(BigInt::from_i64(lower_bound)),
            maximum: range.maximum,
        }
    }

    fn non_true_operand(
        proof_plan: &ProofPlan,
        condition: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let binary = proof_plan.program.expression_table.binary_node(condition)?;
        if binary.operator != BinaryOperator::Equal {
            return None;
        }
        let is_true = |handle| {
            matches!(
                proof_plan.program.expression_table.expression(handle),
                ExpressionNode::Boolean(true)
            )
        };
        if is_true(binary.right) {
            return Some(binary.left);
        }
        is_true(binary.left).then_some(binary.right)
    }

    pub(crate) fn false_comparison_operand(
        proof_plan: &ProofPlan,
        condition: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let binary = proof_plan.program.expression_table.binary_node(condition)?;
        if binary.operator != BinaryOperator::Equal {
            return None;
        }
        let is_false = |handle| {
            matches!(
                proof_plan.program.expression_table.expression(handle),
                ExpressionNode::Boolean(false)
            )
        };
        if is_false(binary.right) {
            return Some(binary.left);
        }
        is_false(binary.left).then_some(binary.right)
    }

    pub(crate) fn unwrap_true_guard_condition(
        proof_plan: &ProofPlan,
        condition: ExpressionHandle,
    ) -> ExpressionHandle {
        non_true_operand(proof_plan, condition).unwrap_or(condition)
    }

    pub(crate) fn apply_handle_condition(
        proof_plan: &ProofPlan,
        range: IntegerRange,
        argument: ExpressionHandle,
        condition: ExpressionHandle,
    ) -> IntegerRange {
        if let ExpressionNode::Unary(unary) =
            proof_plan.program.expression_table.expression(condition)
            && unary.operator == UnaryOperator::LogicalNot
        {
            return apply_handle_condition_complement(proof_plan, range, argument, unary.operand);
        }
        let Some(binary) = proof_plan.program.expression_table.binary_node(condition) else {
            return range;
        };

        if let Some(operand) = non_true_operand(proof_plan, condition) {
            return apply_handle_condition(proof_plan, range, argument, operand);
        }

        if binary.operator == BinaryOperator::And {
            let range = apply_handle_condition(proof_plan, range, argument, binary.left);
            return apply_handle_condition(proof_plan, range, argument, binary.right);
        }

        if expressions_equivalent_for_proof(proof_plan, binary.left, argument) {
            return apply_right_literal_guard(proof_plan, range, binary.operator, binary.right);
        }

        if expressions_equivalent_for_proof(proof_plan, binary.right, argument) {
            return apply_left_literal_guard(proof_plan, range, binary.left, binary.operator);
        }

        range
    }

    pub(crate) fn apply_source_condition(
        proof_plan: &ProofPlan,
        range: IntegerRange,
        argument: ExpressionHandle,
        condition: ExpressionHandle,
        machine_symbol: symbols::SymbolHandle,
        source_state: symbols::SymbolHandle,
    ) -> IntegerRange {
        let condition = proof_plan
            .program
            .machine(machine_symbol)
            .and_then(|machine| {
                proof_plan
                    .program
                    .state_in_machine(machine, source_state)
                    .map(|state| dehoisted_condition(proof_plan.program, state, condition))
            })
            .unwrap_or(condition);
        let Some(binary) = proof_plan.program.expression_table.binary_node(condition) else {
            return range;
        };

        if let Some(operand) = non_true_operand(proof_plan, condition) {
            return apply_source_condition(
                proof_plan,
                range,
                argument,
                operand,
                machine_symbol,
                source_state,
            );
        }

        if binary.operator == BinaryOperator::And {
            let range = apply_source_condition(
                proof_plan,
                range,
                argument,
                binary.left,
                machine_symbol,
                source_state,
            );
            return apply_source_condition(
                proof_plan,
                range,
                argument,
                binary.right,
                machine_symbol,
                source_state,
            );
        }

        let dehoist = |handle: ExpressionHandle| {
            let program = proof_plan.program;
            program
                .machine(machine_symbol)
                .and_then(|machine| {
                    program
                        .state_in_machine(machine, source_state)
                        .map(|state| dehoisted_operand(program, state, handle))
                })
                .unwrap_or(handle)
        };

        let left = dehoist(binary.left);
        let right = dehoist(binary.right);
        if expressions_equivalent_for_proof(proof_plan, left, argument) {
            return apply_right_literal_guard(proof_plan, range, binary.operator, right);
        }
        if expressions_equivalent_for_proof(proof_plan, right, argument) {
            return apply_left_literal_guard(proof_plan, range, left, binary.operator);
        }

        range
    }

    pub(crate) fn expressions_equivalent_for_proof(
        proof_plan: &ProofPlan,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        if left == right {
            return true;
        }

        if let (Some(left_path), Some(right_path)) = (
            flat_place_segments(proof_plan, left),
            flat_place_segments(proof_plan, right),
        ) {
            return left_path == right_path;
        }

        match (
            proof_plan.program.expression_table.expression(left),
            proof_plan.program.expression_table.expression(right),
        ) {
            (ExpressionNode::Borrow(left), _) => {
                expressions_equivalent_for_proof(proof_plan, left.target, right)
            }
            (_, ExpressionNode::Borrow(right)) => {
                expressions_equivalent_for_proof(proof_plan, left, right.target)
            }
            (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
                expression_table(proof_plan.program).name_path_members(left.members)
                    == expression_table(proof_plan.program).name_path_members(right.members)
            }
            (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
                left.target == right.target
                    && left.target_symbol == right.target_symbol
                    && left.arguments.count() == right.arguments.count()
                    && match (left.receiver.is_valid(), right.receiver.is_valid()) {
                        (true, true) => expressions_equivalent_for_proof(
                            proof_plan,
                            left.receiver,
                            right.receiver,
                        ),
                        (false, false) => true,
                        _ => false,
                    }
                    && expression_table(proof_plan.program)
                        .expression_handles(left.arguments)
                        .iter()
                        .zip(
                            expression_table(proof_plan.program)
                                .expression_handles(right.arguments),
                        )
                        .all(|(left_argument, right_argument)| {
                            expressions_equivalent_for_proof(
                                proof_plan,
                                *left_argument,
                                *right_argument,
                            )
                        })
            }
            (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
                left.member == right.member
                    && left.member_symbol == right.member_symbol
                    && expressions_equivalent_for_proof(proof_plan, left.receiver, right.receiver)
            }
            (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
                let (left, right) = (*left, *right);
                expressions_equivalent_for_proof(proof_plan, left.collection, right.collection)
                    && expressions_equivalent_for_proof(proof_plan, left.index, right.index)
            }
            (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
            (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
            (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
            (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
            _ => false,
        }
    }

    fn flat_place_segments(
        proof_plan: &ProofPlan,
        expression: ExpressionHandle,
    ) -> Option<Vec<String>> {
        match proof_plan.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner) => flat_place_segments(proof_plan, inner.target),
            ExpressionNode::Name(path) => Some(
                expression_table(proof_plan.program)
                    .name_path_members(path.members)
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect(),
            ),
            ExpressionNode::Member(member) => {
                let mut segments = flat_place_segments(proof_plan, member.receiver)?;
                segments.push(member.member.as_str().to_owned());
                Some(segments)
            }
            _ => None,
        }
    }

    pub(crate) fn apply_right_literal_guard(
        proof_plan: &ProofPlan,
        range: IntegerRange,
        operator: BinaryOperator,
        right: ExpressionHandle,
    ) -> IntegerRange {
        let Some(value) = integer_literal_handle(proof_plan, right) else {
            return range;
        };
        match validation::BigIntInterval::from_comparison(operator, true, BigInt::from_i64(value)) {
            Some(interval) => interval.refine_closed(range),
            None => range,
        }
    }

    pub(crate) fn mirrored_comparison(operator: BinaryOperator) -> BinaryOperator {
        operator.mirrored()
    }

    pub(crate) fn apply_left_literal_guard(
        proof_plan: &ProofPlan,
        range: IntegerRange,
        left: ExpressionHandle,
        operator: BinaryOperator,
    ) -> IntegerRange {
        apply_right_literal_guard(proof_plan, range, mirrored_comparison(operator), left)
    }
}
mod integer_ranges {
    use crate::checker::assignment_stability::collect_stable_assignment_conditions;
    use crate::checker::guards::{
        apply_assignment_guard, apply_handle_condition, apply_handle_guard, apply_source_condition,
        expressions_equivalent_for_proof, unwrap_true_guard_condition,
    };
    use crate::checker::requires_conditions;
    use crate::obligations::{
        BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
        BoundedStateReturnObligation, BoundedTransitionArgumentObligation, IntegerRange,
        ProofConstraint, ProofPlan, declared_integer_range, integer_binary_range,
    };
    use crate::typed_expressions::{dehoisted_operand, expression_table};
    use arena::HandleSpan;
    use numerics::bignum::BigInt;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
    };
    use typed_trees::statement::TransitionGuardNode;
    use validation::land_anonymous_integer_expression;

    fn integer_range_for_argument(
        proof_plan: &ProofPlan,
        operand: ExpressionHandle,
        base_type: typed_trees::types::TypeReferenceHandle,
        constraints: HandleSpan<ProofConstraint>,
    ) -> Option<IntegerRange> {
        if let Some(range) = anonymous_integer_argument_range(proof_plan, operand, base_type) {
            return Some(range);
        }
        integer_range_for_value(proof_plan, operand, constraints)
    }

    fn integer_range_for_value(
        proof_plan: &ProofPlan,
        operand: ExpressionHandle,
        constraints: HandleSpan<ProofConstraint>,
    ) -> Option<IntegerRange> {
        match proof_plan.program.expression_table.expression(operand) {
            ExpressionNode::Integer(value) => integer_range_for_literal(value),
            _ => integer_range_from_constraints(type_constraints(proof_plan, constraints)),
        }
    }

    macro_rules! argument_obligation_range {
        ($name:ident, $obligation:ty) => {
            pub(crate) fn $name(
                proof_plan: &ProofPlan,
                obligation: &$obligation,
            ) -> Option<IntegerRange> {
                integer_range_for_argument(
                    proof_plan,
                    obligation.argument,
                    obligation.base_type,
                    obligation.argument_constraints,
                )
            }
        };
    }

    macro_rules! value_obligation_range {
    ($vis:vis $name:ident, $obligation:ty) => {
        $vis fn $name(
            proof_plan: &ProofPlan,
            obligation: &$obligation,
        ) -> Option<IntegerRange> {
            integer_range_for_value(proof_plan, obligation.value, obligation.value_constraints)
        }
    };
}

    argument_obligation_range!(
        integer_range_for_transition_argument,
        BoundedTransitionArgumentObligation
    );

    pub(crate) fn guarded_integer_range_for_transition_argument(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
    ) -> IntegerRange {
        let base = integer_range_for_transition_argument(proof_plan, obligation)
            .unwrap_or_else(neutral_range);

        let range = apply_handle_guard(proof_plan, base, obligation.argument, &obligation.guard);
        let mut range =
            guard_refined_binary_range(proof_plan, range, obligation.argument, &obligation.guard);
        for refuted in &obligation.refuted_exit_guards {
            range =
                apply_handle_condition_complement(proof_plan, range, obligation.argument, *refuted);
            range = place_refined_binary_range(
                proof_plan,
                range,
                obligation.argument,
                |plan, place_range, place| {
                    apply_handle_condition_complement(plan, place_range, place, *refuted)
                },
            );
        }
        range
    }

    pub(crate) fn apply_handle_condition_complement(
        proof_plan: &ProofPlan,
        mut range: IntegerRange,
        argument: ExpressionHandle,
        condition: ExpressionHandle,
    ) -> IntegerRange {
        let condition = unwrap_true_guard_condition(proof_plan, condition);
        if let ExpressionNode::Unary(unary) =
            proof_plan.program.expression_table.expression(condition)
            && unary.operator == UnaryOperator::LogicalNot
        {
            return apply_handle_condition(proof_plan, range, argument, unary.operand);
        }
        let Some(binary) = proof_plan.program.expression_table.binary_node(condition) else {
            return range;
        };
        if !expressions_equivalent_for_proof(proof_plan, binary.left, argument) {
            return range;
        }
        let Some(value) = integer_literal_handle(proof_plan, binary.right) else {
            return range;
        };
        let value = BigInt::from_i64(value);
        let one = BigInt::from_i64(1);
        match binary.operator {
            BinaryOperator::Equal => {
                if range.minimum == value {
                    range.minimum = range.minimum.add(&one);
                }
                if range.maximum == value {
                    range.maximum = range.maximum.sub(&one);
                }
            }
            operator => {
                if let Some(interval) =
                    validation::BigIntInterval::from_refuted_comparison(operator, true, value)
                {
                    range = interval.refine_closed(range);
                }
            }
        }
        range
    }

    fn place_refined_binary_range(
        proof_plan: &ProofPlan,
        range: IntegerRange,
        value: ExpressionHandle,
        narrow: impl FnOnce(&ProofPlan, IntegerRange, ExpressionHandle) -> IntegerRange,
    ) -> IntegerRange {
        let Some(binary) = proof_plan.program.expression_table.binary_node(value) else {
            return range;
        };
        let (place, literal, place_is_left) =
            if let Some(literal) = integer_literal_handle(proof_plan, binary.right) {
                (binary.left, literal, true)
            } else if let Some(literal) = integer_literal_handle(proof_plan, binary.left) {
                (binary.right, literal, false)
            } else {
                return range;
            };
        let literal = BigInt::from_i64(literal);
        let place_range = match (binary.operator, place_is_left) {
            (BinaryOperator::Add, _) => IntegerRange {
                minimum: range.minimum.sub(&literal),
                maximum: range.maximum.sub(&literal),
            },
            (BinaryOperator::Subtract, true) => IntegerRange {
                minimum: range.minimum.add(&literal),
                maximum: range.maximum.add(&literal),
            },
            (BinaryOperator::Subtract, false) => IntegerRange {
                minimum: literal.sub(&range.maximum),
                maximum: literal.sub(&range.minimum),
            },
            _ => return range,
        };
        let narrowed = narrow(proof_plan, place_range.clone(), place);
        if narrowed == place_range {
            return range;
        }
        let refolded = match (binary.operator, place_is_left) {
            (BinaryOperator::Add, _) => IntegerRange {
                minimum: narrowed.minimum.add(&literal),
                maximum: narrowed.maximum.add(&literal),
            },
            (BinaryOperator::Subtract, true) => IntegerRange {
                minimum: narrowed.minimum.sub(&literal),
                maximum: narrowed.maximum.sub(&literal),
            },
            (BinaryOperator::Subtract, false) => IntegerRange {
                minimum: literal.sub(&narrowed.maximum),
                maximum: literal.sub(&narrowed.minimum),
            },
            _ => unreachable!("classified above"),
        };
        IntegerRange {
            minimum: range.minimum.max(refolded.minimum),
            maximum: range.maximum.min(refolded.maximum),
        }
    }

    argument_obligation_range!(
        integer_range_for_call_argument,
        BoundedCallArgumentObligation
    );

    fn anonymous_integer_argument_range(
        proof_plan: &ProofPlan,
        argument: ExpressionHandle,
        destination: typed_trees::types::TypeReferenceHandle,
    ) -> Option<IntegerRange> {
        let program = proof_plan.program;
        let primitive = program.primitive_type_reference(destination)?;
        let (literal, _) =
            land_anonymous_integer_expression(program, argument, primitive, &[], |expression| {
                validation::has_anonymous_operator_meaning(program, expression)
            })?;
        integer_range_for_literal(&literal)
    }

    value_obligation_range!(integer_range_for_assignment, BoundedAssignmentObligation);

    pub(crate) fn guarded_integer_range_for_assignment(
        proof_plan: &ProofPlan,
        obligation: &BoundedAssignmentObligation,
    ) -> Option<IntegerRange> {
        let context = AssignmentRangeContext::new(proof_plan);
        guarded_integer_range_for_assignment_with_context(proof_plan, obligation, &context)
    }

    pub(crate) fn guarded_integer_range_for_assignment_with_context(
        proof_plan: &ProofPlan,
        obligation: &BoundedAssignmentObligation,
        context: &AssignmentRangeContext<'_>,
    ) -> Option<IntegerRange> {
        let declared = integer_range_for_assignment(proof_plan, obligation);
        let mut range = declared.clone().unwrap_or_else(neutral_range);

        let value_display = expression_table(proof_plan.program).display_name(obligation.value);
        for (place, bound) in &obligation.ensures_witness_bounds {
            if place == &value_display {
                let bound = BigInt::from_i64(*bound);
                if range.maximum > bound {
                    range.maximum = bound;
                }
            }
        }
        if !obligation.ensures_witness_bounds.is_empty()
            && let Some(operands) = &obligation.binary_operands
        {
            let witness_operand = |declared: &Option<IntegerRange>, handle: ExpressionHandle| {
                let mut narrowed = declared.clone().unwrap_or_else(neutral_range);
                let operand_display = proof_plan.program.expression_table.display_name(handle);
                let mut touched = false;
                for (place, bound) in &obligation.ensures_witness_bounds {
                    if place == &operand_display {
                        let bound = BigInt::from_i64(*bound);
                        if narrowed.maximum > bound {
                            narrowed.maximum = bound;
                        }
                        if narrowed.minimum < BigInt::zero()
                            && operand_is_unsigned(proof_plan, obligation, handle)
                        {
                            narrowed.minimum = BigInt::zero();
                        }
                        touched = true;
                    }
                }
                (touched || declared.is_some()).then_some(narrowed)
            };
            if let (Some(left), Some(right)) = (
                witness_operand(&operands.left_range, operands.left),
                witness_operand(&operands.right_range, operands.right),
            ) && left != neutral_range()
                && right != neutral_range()
                && let Some(folded) = integer_binary_range(operands.operator, left, right)
            {
                range = IntegerRange {
                    minimum: range.minimum.max(folded.minimum),
                    maximum: range.maximum.min(folded.maximum),
                };
            }
        }

        let mut conditions = Vec::new();
        if let Some(TransitionGuardNode::When(condition)) = &obligation.state_guard {
            collect_stable_assignment_conditions(
                proof_plan,
                obligation,
                *condition,
                context,
                &mut conditions,
            );
        }
        for condition in &conditions {
            let guard = TransitionGuardNode::When(*condition);
            range = apply_assignment_guard(proof_plan, range, obligation.value, &guard);
            range = guard_refined_binary_range(proof_plan, range, obligation.value, &guard);
        }
        if !conditions.is_empty()
            && let Some(operands) = &obligation.binary_operands
            && let (Some(left), Some(right)) = (
                guard_narrowed_operand_range(
                    proof_plan,
                    obligation,
                    &conditions,
                    operands.left,
                    operands.left_range.clone(),
                ),
                guard_narrowed_operand_range(
                    proof_plan,
                    obligation,
                    &conditions,
                    operands.right,
                    operands.right_range.clone(),
                ),
            )
            && let Some(folded) = integer_binary_range(operands.operator, left, right)
        {
            range = IntegerRange {
                minimum: range.minimum.max(folded.minimum),
                maximum: range.maximum.min(folded.maximum),
            };
        }

        let requires = requires_conditions::surviving_conditions(
            proof_plan,
            context,
            obligation.machine_symbol,
            obligation.state_symbol,
            obligation.statement_index,
            obligation.value,
            obligation.binary_operands.as_ref(),
        );
        range = requires_conditions::refine(
            proof_plan,
            range,
            obligation.value,
            obligation.binary_operands.as_ref(),
            &requires,
        );

        if declared.is_none() && range == neutral_range() {
            return None;
        }
        Some(range)
    }

    fn guard_narrowed_operand_range(
        proof_plan: &ProofPlan,
        obligation: &BoundedAssignmentObligation,
        conditions: &[ExpressionHandle],
        handle: ExpressionHandle,
        declared: Option<IntegerRange>,
    ) -> Option<IntegerRange> {
        let mut narrowed = declared.unwrap_or_else(neutral_range);
        for condition in conditions {
            narrowed = apply_source_condition(
                proof_plan,
                narrowed,
                handle,
                *condition,
                obligation.machine_symbol,
                obligation.state_guard_source,
            );
        }

        let operand_display = proof_plan.program.expression_table.display_name(handle);
        for (place, bound) in &obligation.ensures_witness_bounds {
            if place == &operand_display {
                let bound = BigInt::from_i64(*bound);
                if narrowed.maximum > bound {
                    narrowed.maximum = bound;
                }
                if narrowed.minimum < BigInt::zero()
                    && operand_is_unsigned(proof_plan, obligation, handle)
                {
                    narrowed.minimum = BigInt::zero();
                }
            }
        }

        let Some(binary) = proof_plan.program.expression_table.binary_node(handle) else {
            return (narrowed != neutral_range()).then_some(narrowed);
        };

        let Some((machine, state)) = super::machine_and_state(
            proof_plan.program,
            obligation.machine_symbol,
            obligation.state_symbol,
        ) else {
            return (narrowed != neutral_range()).then_some(narrowed);
        };

        let nested = |sub_operand: ExpressionHandle| {
            let sub_operand = dehoisted_operand(proof_plan.program, state, sub_operand);
            guard_narrowed_operand_range(
                proof_plan,
                obligation,
                conditions,
                sub_operand,
                declared_integer_range(proof_plan.program, machine, state, sub_operand),
            )
        };
        if let (Some(left), Some(right)) = (nested(binary.left), nested(binary.right))
            && let Some(folded) = integer_binary_range(binary.operator, left, right)
        {
            narrowed = IntegerRange {
                minimum: narrowed.minimum.max(folded.minimum),
                maximum: narrowed.maximum.min(folded.maximum),
            };
        }
        (narrowed != neutral_range()).then_some(narrowed)
    }

    pub struct AssignmentRangeContext<'program> {
        pub(crate) program: &'program dyn ::validation::CheckingInput,
        call_frames: std::sync::OnceLock<Option<validation::CallFrameResolver<'program>>>,
    }

    impl<'program> AssignmentRangeContext<'program> {
        pub fn new(proof_plan: &ProofPlan<'program>) -> Self {
            Self {
                program: proof_plan.program,
                call_frames: std::sync::OnceLock::new(),
            }
        }

        pub(crate) fn call_frames(&self) -> Option<&validation::CallFrameResolver<'program>> {
            self.call_frames
                .get_or_init(|| validation::CallFrameResolver::new(self.program))
                .as_ref()
        }
    }

    pub fn proved_assignment_integer_range_with_context(
        proof_plan: &ProofPlan<'_>,
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        statement_index: usize,
        context: &AssignmentRangeContext<'_>,
    ) -> Option<crate::obligations::IntegerRange> {
        if !std::ptr::eq(context.program, proof_plan.program) {
            return None;
        }
        let obligation =
            assignment_range_obligation(proof_plan, machine_symbol, state_symbol, statement_index)?;
        guarded_integer_range_for_assignment_with_context(proof_plan, obligation, context)
    }

    fn assignment_range_obligation<'plan>(
        proof_plan: &'plan ProofPlan<'_>,
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        statement_index: usize,
    ) -> Option<&'plan BoundedAssignmentObligation> {
        proof_plan
            .assignment_value_ranges
            .iter()
            .map(|(_, obligation)| obligation)
            .find(|obligation| {
                obligation.machine_symbol == machine_symbol
                    && obligation.state_symbol == state_symbol
                    && obligation.statement_index == statement_index
            })
    }

    fn operand_is_unsigned(
        proof_plan: &ProofPlan,
        obligation: &BoundedAssignmentObligation,
        handle: ExpressionHandle,
    ) -> bool {
        let _ = obligation;
        let Some(constraints) = operand_declared_primitive(proof_plan, handle) else {
            return false;
        };
        matches!(
            constraints,
            typed_trees::types::PrimitiveType::U8
                | typed_trees::types::PrimitiveType::U16
                | typed_trees::types::PrimitiveType::U32
                | typed_trees::types::PrimitiveType::U64
                | typed_trees::types::PrimitiveType::Addr
        )
    }

    fn operand_declared_primitive(
        proof_plan: &ProofPlan,
        handle: ExpressionHandle,
    ) -> Option<typed_trees::types::PrimitiveType> {
        let program = proof_plan.program;
        let member = program.expression_table.member_node(handle)?;
        let path = program.expression_table.name_path_node(member.receiver)?;
        let [receiver] = program.expression_table.name_path_members(path.members) else {
            return None;
        };
        if !receiver.is_self_receiver() {
            return None;
        }
        for machine in program.machines() {
            let Some(attached) = machine.attached_data.as_ref() else {
                continue;
            };
            let Some(data) = program.data_definition_by_name_str(attached.as_str()) else {
                continue;
            };
            if let Some(field_type) = ::validation::field_domains::data_field_type_by_name(
                program,
                data,
                member.member.as_str(),
            ) {
                return program.primitive_type_reference(field_type);
            }
        }
        None
    }

    fn guard_refined_binary_range(
        proof_plan: &ProofPlan,
        range: IntegerRange,
        value: ExpressionHandle,
        guard: &TransitionGuardNode,
    ) -> IntegerRange {
        let TransitionGuardNode::When(condition) = guard else {
            return range;
        };
        place_refined_binary_range(proof_plan, range, value, |plan, place_range, place| {
            apply_handle_condition(plan, place_range, place, *condition)
        })
    }

    value_obligation_range!(
        pub(crate) integer_range_for_return_value,
        BoundedStateReturnObligation
    );

    value_obligation_range!(
        pub(crate) integer_range_for_initializer,
        BoundedInitializerObligation
    );

    fn neutral_range() -> IntegerRange {
        IntegerRange {
            minimum: BigInt::from_i64(i64::MIN),
            maximum: BigInt::from_i64(i64::MAX),
        }
    }

    fn integer_range_for_literal(
        literal: &numerics::literals::IntegerLiteral,
    ) -> Option<IntegerRange> {
        let value = literal.value_bignum()?;
        Some(IntegerRange {
            minimum: value.clone(),
            maximum: value,
        })
    }

    pub(crate) fn integer_range_from_constraints(
        constraints: &[ProofConstraint],
    ) -> Option<IntegerRange> {
        crate::obligations::integer_range_from_constraint_iter(constraints.iter())
    }

    pub(crate) fn type_constraints<'proof>(
        proof_plan: &'proof ProofPlan<'_>,
        constraints: HandleSpan<ProofConstraint>,
    ) -> &'proof [ProofConstraint] {
        proof_plan.type_constraints.span(constraints).unwrap_or(&[])
    }

    pub(crate) fn integer_literal_handle(
        proof_plan: &ProofPlan,
        expression: ExpressionHandle,
    ) -> Option<i64> {
        match proof_plan.program.expression_table.expression(expression) {
            ExpressionNode::Integer(value) => value.value_i64(),
            ExpressionNode::Binary(binary) => {
                let left = integer_literal_handle(proof_plan, binary.left)?;
                let right = integer_literal_handle(proof_plan, binary.right)?;
                let folded = integer_binary_range(
                    binary.operator,
                    IntegerRange {
                        minimum: BigInt::from_i64(left),
                        maximum: BigInt::from_i64(left),
                    },
                    IntegerRange {
                        minimum: BigInt::from_i64(right),
                        maximum: BigInt::from_i64(right),
                    },
                )?;
                if folded.minimum == folded.maximum {
                    folded.minimum.to_i64()
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
mod measurement {
    use proof_admission::{
        AcceptedFact, AcceptedFactRoute, MathematicalCoreDecision, ProofNode, ProofRule,
    };

    use crate::checker::certificate::CertificateVerdict;
    use crate::obligations::ProofObligation;

    #[derive(Debug, Default, Clone, PartialEq, Eq)]
    pub struct ProofPlanMeasurements {
        pub bounded_assignments: u32,
        pub bounded_call_arguments: u32,
        pub bounded_initializers: u32,
        pub bounded_state_returns: u32,
        pub bounded_transition_arguments: u32,
        pub decided_elsewhere: u32,
        pub discharged: u32,
        pub diagnosed: u32,
        pub certificate_certified: u32,
        pub certificate_rejected: u32,
        pub certificate_uncovered: u32,
        pub kernel_judgments: u32,
        pub kernel_refusals: u32,
        pub kernel_declarations: u64,
        pub kernel_assumption_closure: u64,
        pub kernel_context_depth: u64,
        pub kernel_arena_slots: u64,
        pub certificate_proof_nodes: u64,
        pub check_elapsed_microseconds: u64,
        /// Obligations by `ROUTE_KINDS` row and `ROUTES` column.
        pub routes: [[u32; ROUTES.len()]; ROUTE_KINDS.len()],
    }

    /// The census in `check_on_lowered_form.md` counts an obligation as
    /// evidence-backed only when proof admission decided it again from a
    /// certificate. The columns say how each obligation was settled, weakest
    /// route first when one obligation made several certificate attempts:
    /// `interval_fallback` (a certificate was uncovered and interval reasoning
    /// accepted), `certificate_core_refused` (admission checked the certificate
    /// rules but the mathematical core refused the family),
    /// `certificate_kernel_judged` (admission and the core both decided it),
    /// `no_certificate` (accepted with no attempt: skipped as overwritten, a
    /// float or named-constraint check, or no integer range), `diagnosed`, and
    /// `decided_elsewhere` (not decided by this checker at all).
    pub const ROUTES: [&str; 6] = [
        "certificate_kernel_judged",
        "certificate_core_refused",
        "interval_fallback",
        "no_certificate",
        "diagnosed",
        "decided_elsewhere",
    ];

    pub const ROUTE_KINDS: [&str; 7] = [
        "bounded_assignment",
        "bounded_call_argument",
        "bounded_initializer",
        "bounded_state_return",
        "bounded_value",
        "bounded_transition_argument",
        "guarded_transition",
    ];

    /// Certificate counters before one obligation, compared after it.
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct CertificateAttempts {
        certified: u32,
        uncovered: u32,
        refusals: u32,
    }

    impl ProofPlanMeasurements {
        pub(crate) const fn attempts(&self) -> CertificateAttempts {
            CertificateAttempts {
                certified: self.certificate_certified,
                uncovered: self.certificate_uncovered,
                refusals: self.kernel_refusals,
            }
        }

        pub(crate) fn record_route(
            &mut self,
            obligation: &ProofObligation,
            decided_here: bool,
            diagnosed: bool,
            before: CertificateAttempts,
        ) {
            let kind = match obligation {
                ProofObligation::BoundedAssignment(_) => 0,
                ProofObligation::BoundedCallArgument(_) => 1,
                ProofObligation::BoundedInitializer(_) => 2,
                ProofObligation::BoundedStateReturn(_) => 3,
                ProofObligation::BoundedValue(_) => 4,
                ProofObligation::BoundedTransitionArgument(_) => 5,
                ProofObligation::GuardedTransition(_) => 6,
            };
            let route = if !decided_here {
                5
            } else if diagnosed {
                4
            } else if self.certificate_uncovered > before.uncovered {
                2
            } else if self.kernel_refusals > before.refusals {
                1
            } else if self.certificate_certified > before.certified {
                0
            } else {
                3
            };
            self.routes[kind][route] += 1;
        }

        pub fn render_routes(&self) -> String {
            let mut rendered = String::new();
            for (kind, row) in ROUTE_KINDS.iter().zip(&self.routes) {
                for (route, count) in ROUTES.iter().zip(row) {
                    if *count != 0 {
                        rendered.push_str(&format!(
                            "proof_obligation_route kind={kind} route={route} count={count}\n"
                        ));
                    }
                }
            }
            rendered
        }

        pub const fn obligations(&self) -> u32 {
            self.bounded_assignments
                + self.bounded_call_arguments
                + self.bounded_initializers
                + self.bounded_state_returns
                + self.bounded_transition_arguments
                + self.decided_elsewhere
        }

        pub const fn certificate_attempts(&self) -> u32 {
            self.certificate_certified + self.certificate_rejected + self.certificate_uncovered
        }

        pub(crate) fn record_obligation(&mut self, obligation: &ProofObligation) {
            match obligation {
                ProofObligation::BoundedAssignment(_) => self.bounded_assignments += 1,
                ProofObligation::BoundedCallArgument(_) => self.bounded_call_arguments += 1,
                ProofObligation::BoundedInitializer(_) => self.bounded_initializers += 1,
                ProofObligation::BoundedStateReturn(_) => self.bounded_state_returns += 1,
                ProofObligation::BoundedTransitionArgument(_) => {
                    self.bounded_transition_arguments += 1
                }
                ProofObligation::BoundedValue(_) | ProofObligation::GuardedTransition(_) => {
                    self.decided_elsewhere += 1;
                }
            }
        }

        pub(crate) fn record_outcome(&mut self, produced_diagnostic: bool) {
            if produced_diagnostic {
                self.diagnosed += 1;
            } else {
                self.discharged += 1;
            }
        }

        pub(crate) fn record_certificate_verdict(&mut self, verdict: CertificateVerdict) {
            match verdict {
                CertificateVerdict::Certified => self.certificate_certified += 1,
                CertificateVerdict::Rejected => self.certificate_rejected += 1,
                CertificateVerdict::Uncovered => self.certificate_uncovered += 1,
            }
        }

        pub(crate) fn record_emitted_certificate(&mut self, proof: &ProofNode) {
            self.certificate_proof_nodes += proof_node_count(proof);
        }

        pub fn render(&self) -> String {
            format!(
                "obligations={} bounded_assignments={} bounded_call_arguments={} \
             bounded_initializers={} bounded_state_returns={} \
             bounded_transition_arguments={} decided_elsewhere={} discharged={} \
             diagnosed={} certificate_certified={} certificate_rejected={} \
             certificate_uncovered={} kernel_judgments={} kernel_refusals={} \
             kernel_declarations={} kernel_assumption_closure={} \
             kernel_context_depth={} kernel_arena_slots={} \
             certificate_proof_nodes={} check_elapsed_microseconds={}",
                self.obligations(),
                self.bounded_assignments,
                self.bounded_call_arguments,
                self.bounded_initializers,
                self.bounded_state_returns,
                self.bounded_transition_arguments,
                self.decided_elsewhere,
                self.discharged,
                self.diagnosed,
                self.certificate_certified,
                self.certificate_rejected,
                self.certificate_uncovered,
                self.kernel_judgments,
                self.kernel_refusals,
                self.kernel_declarations,
                self.kernel_assumption_closure,
                self.kernel_context_depth,
                self.kernel_arena_slots,
                self.certificate_proof_nodes,
                self.check_elapsed_microseconds,
            )
        }

        pub(crate) fn emit_if_requested(&self) {
            if std::env::var_os("OMEGA_PROOF_MEASUREMENTS").is_some() {
                eprintln!("proof_plan_measurements {}", self.render());
                eprint!("{}", self.render_routes());
            }
        }

        pub(crate) fn record_accepted_fact(&mut self, fact: &AcceptedFact) {
            let AcceptedFactRoute::CertificateDerived { acceptance, .. } = &fact.route else {
                return;
            };
            match &acceptance.mathematical_core {
                MathematicalCoreDecision::Judged(receipt) => {
                    self.kernel_judgments += 1;
                    self.kernel_declarations += u64::from(receipt.declarations);
                    self.kernel_assumption_closure += u64::from(receipt.assumption_closure);
                    self.kernel_context_depth += u64::from(receipt.context_depth);
                    self.kernel_arena_slots += u64::from(receipt.arena_slots);
                }
                MathematicalCoreDecision::Refused(_) => self.kernel_refusals += 1,
            }
        }
    }

    fn proof_node_count(node: &ProofNode) -> u64 {
        let children: u64 = match &node.rule {
            ProofRule::ValueEqualityTransport {
                premise,
                equalities,
            } => proof_node_count(premise) + equalities.iter().map(proof_node_count).sum::<u64>(),
            ProofRule::PredicateDenotation { premise } => proof_node_count(premise),
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. }
            | ProofRule::IntegerCorrelatedForbiddenRoots { .. } => 0,
            ProofRule::ConjunctionIntroduction(conjuncts) => {
                conjuncts.iter().map(proof_node_count).sum()
            }
            ProofRule::ConjunctionElimination { conjunction, .. } => proof_node_count(conjunction),
            ProofRule::DisjunctionIntroduction { disjunct, .. } => proof_node_count(disjunct),
            ProofRule::DisjunctionElimination {
                disjunction,
                branches,
            } => proof_node_count(disjunction) + branches.iter().map(proof_node_count).sum::<u64>(),
            ProofRule::ImplicationIntroduction { body } => proof_node_count(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_node_count(implication) + proof_node_count(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => proof_node_count(left_equals_middle) + proof_node_count(middle_equals_right),
            ProofRule::EqualitySymmetry { equality } => proof_node_count(equality),
            ProofRule::IntegerOrderWeakening { relation }
            | ProofRule::IntegerOrderDiscreteness { relation }
            | ProofRule::IntegerMathOrderTranslation { relation, .. } => proof_node_count(relation),
            ProofRule::IntegerOrderAntisymmetry { forward, backward } => {
                proof_node_count(forward) + proof_node_count(backward)
            }
            ProofRule::IntegerSubtractOrder {
                difference,
                positive,
            } => proof_node_count(difference) + proof_node_count(positive),
            ProofRule::IntegerAddOrder { sum, positive } => {
                proof_node_count(sum) + proof_node_count(positive)
            }
            ProofRule::IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => proof_node_count(smaller) + proof_node_count(larger) + proof_node_count(order),
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                middle_less_or_equal_right,
            } => {
                proof_node_count(left_less_or_equal_middle)
                    + proof_node_count(middle_less_or_equal_right)
            }
            ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle,
                middle_to_right,
            } => proof_node_count(left_to_middle) + proof_node_count(middle_to_right),
            ProofRule::IntegerOrderSubstitution {
                relation, equality, ..
            } => proof_node_count(relation) + proof_node_count(equality),
            ProofRule::IntegerAffineBound { root_bound, .. } => proof_node_count(root_bound),
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound,
                right_bound,
                ..
            }
            | ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound,
                right_bound,
                ..
            } => proof_node_count(left_bound) + proof_node_count(right_bound),
            ProofRule::IntegerCastBound { root_bound, .. } => proof_node_count(root_bound),
        };
        1 + children
    }

    #[cfg(test)]
    mod tests {
        use super::{
            AcceptedFact, AcceptedFactRoute, MathematicalCoreDecision, ProofPlanMeasurements,
        };
        use proof_admission::{
            CertificateAcceptance, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
        };
        use semantic_vocabulary::{
            EvidenceIdentity, IntegerMathLiteral, IntegerMathTerm, ObligationId, Proposition,
        };

        fn sample_proposition() -> Proposition {
            let zero = IntegerMathTerm::IntegerLiteral(
                IntegerMathLiteral::new(false, 0).expect("zero literal"),
            );
            Proposition::IntegerMathLessOrEqual(zero.clone(), zero)
        }

        fn certificate_derived_fact(mathematical_core: MathematicalCoreDecision) -> AcceptedFact {
            AcceptedFact {
                obligation: ObligationId::new(1).expect("obligation id"),
                proposition: sample_proposition(),
                route: AcceptedFactRoute::CertificateDerived {
                    identity: EvidenceIdentity::new(1).expect("identity"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    acceptance: CertificateAcceptance {
                        rules: Vec::new(),
                        assumptions: Vec::new(),
                        semantic_axioms: Vec::new(),
                        mathematical_core,
                    },
                },
            }
        }

        #[test]
        fn refused_and_kernel_derived_routes_carry_no_receipt() {
            let mut measurements = ProofPlanMeasurements::default();
            measurements.record_accepted_fact(&certificate_derived_fact(
                MathematicalCoreDecision::Refused("uncovered family"),
            ));
            assert_eq!(measurements.kernel_refusals, 1);
            assert_eq!(measurements.kernel_judgments, 0);

            measurements.record_accepted_fact(&AcceptedFact {
                obligation: ObligationId::new(2).expect("obligation id"),
                proposition: sample_proposition(),
                route: AcceptedFactRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
            });
            assert_eq!(measurements.kernel_judgments, 0);
            assert_eq!(measurements.kernel_refusals, 1);
        }

        #[test]
        fn emitted_certificates_count_whole_proof_trees() {
            let leaf = || ProofNode {
                conclusion: sample_proposition(),
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            };
            let mut measurements = ProofPlanMeasurements::default();
            measurements.record_emitted_certificate(&ProofNode {
                conclusion: sample_proposition(),
                rule: ProofRule::ConjunctionIntroduction(vec![leaf(), leaf()]),
            });
            assert_eq!(measurements.certificate_proof_nodes, 3);
            measurements.record_emitted_certificate(&ProofNode {
                conclusion: sample_proposition(),
                rule: ProofRule::ImplicationElimination {
                    implication: Box::new(ProofNode {
                        conclusion: sample_proposition(),
                        rule: ProofRule::Assumption { index: 0 },
                    }),
                    premise: Box::new(leaf()),
                },
            });
            assert_eq!(measurements.certificate_proof_nodes, 6);
        }

        #[test]
        fn render_lists_every_figure_on_one_line() {
            let measurements = ProofPlanMeasurements {
                discharged: 3,
                diagnosed: 1,
                certificate_proof_nodes: 7,
                check_elapsed_microseconds: 42,
                ..ProofPlanMeasurements::default()
            };
            let line = measurements.render();
            assert!(line.contains("obligations=0"));
            assert!(line.contains("discharged=3"));
            assert!(line.contains("diagnosed=1"));
            assert!(line.contains("certificate_proof_nodes=7"));
            assert!(line.ends_with("check_elapsed_microseconds=42"));
            assert!(!line.contains('\n'));
        }
    }
}
mod named_constraints {
    use crate::checker::diagnostics::{
        cannot_prove_assignment_named_constraint, cannot_prove_call_named_constraint,
        cannot_prove_initializer_named_constraint, cannot_prove_return_named_constraint,
        cannot_prove_transition_named_constraint,
    };
    use crate::checker::float_ranges::{
        finite_float_literal, float_range_for_assignment, float_range_for_call_argument,
        float_range_for_initializer, float_range_for_transition_argument,
        float_range_from_constraints, guarded_float_range_for_return_value,
    };
    use crate::checker::integer_ranges::{
        guarded_integer_range_for_assignment, guarded_integer_range_for_transition_argument,
        integer_range_for_transition_argument, integer_range_from_constraints, type_constraints,
    };
    use crate::obligations::{
        BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
        BoundedStateReturnObligation, BoundedTransitionArgumentObligation, IntegerRange,
        ProofConstraint, ProofPlan,
    };
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    pub(crate) fn check_call_named_constraints(
        proof_plan: &ProofPlan,
        obligation: &BoundedCallArgumentObligation,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
            if constraint == "finite"
                && float_range_for_call_argument(proof_plan, obligation)
                    .is_some_and(|range| range.proves_finite())
            {
                continue;
            }
            if !argument_handle_satisfies_named_constraint(
                proof_plan,
                obligation.argument,
                obligation.argument_constraints,
                constraint,
            ) {
                diagnostics.push(cannot_prove_call_named_constraint(
                    proof_plan, obligation, constraint,
                ));
            }
        }
    }

    macro_rules! check_named_constraints {
        ($name:ident, $obligation:ty, $satisfies:ident, $diagnostic:ident) => {
            pub(crate) fn $name(
                proof_plan: &ProofPlan,
                obligation: &$obligation,
                diagnostics: &mut Vec<Diagnostic>,
            ) {
                for constraint in
                    named_constraints(type_constraints(proof_plan, obligation.constraints))
                {
                    if !$satisfies(proof_plan, obligation, constraint) {
                        diagnostics.push($diagnostic(proof_plan, obligation, constraint));
                    }
                }
            }
        };
    }

    check_named_constraints!(
        check_assignment_named_constraints,
        BoundedAssignmentObligation,
        assignment_satisfies_named_constraint,
        cannot_prove_assignment_named_constraint
    );

    check_named_constraints!(
        check_transition_named_constraints,
        BoundedTransitionArgumentObligation,
        transition_argument_satisfies_named_constraint,
        cannot_prove_transition_named_constraint
    );

    pub(crate) fn check_return_named_constraints(
        proof_plan: &ProofPlan,
        obligation: &BoundedStateReturnObligation,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
            if constraint == "finite"
                && guarded_float_range_for_return_value(proof_plan, obligation)
                    .is_some_and(|range| range.proves_finite())
            {
                continue;
            }
            if !argument_handle_satisfies_named_constraint(
                proof_plan,
                obligation.value,
                obligation.value_constraints,
                constraint,
            ) {
                diagnostics.push(cannot_prove_return_named_constraint(
                    proof_plan, obligation, constraint,
                ));
            }
        }
    }

    check_named_constraints!(
        check_initializer_named_constraints,
        BoundedInitializerObligation,
        initializer_satisfies_named_constraint,
        cannot_prove_initializer_named_constraint
    );

    fn named_constraints(constraints: &[ProofConstraint]) -> impl Iterator<Item = &str> {
        constraints.iter().filter_map(|constraint| {
            let ProofConstraint::Named(name) = constraint else {
                return None;
            };

            Some(name.as_str())
        })
    }

    fn argument_handle_satisfies_named_constraint(
        proof_plan: &ProofPlan,
        argument: ExpressionHandle,
        argument_constraints: HandleSpan<ProofConstraint>,
        constraint: &str,
    ) -> bool {
        let constraints = type_constraints(proof_plan, argument_constraints);

        constraints_satisfy_named_constraint(constraints, constraint)
            || match (
                constraint,
                proof_plan.program.expression_table.expression(argument),
            ) {
                ("exact", ExpressionNode::Integer(_)) => true,
                ("finite", ExpressionNode::Float(value)) => finite_float_literal(value).is_some(),
                ("finite", ExpressionNode::Integer(_)) => true,
                ("non_negative", ExpressionNode::Integer(value)) => {
                    value.value_i64().is_some_and(|value| value >= 0)
                }
                ("positive", ExpressionNode::Integer(value)) => {
                    value.value_i64().is_some_and(|value| value > 0)
                }
                ("wrapping", ExpressionNode::Integer(_)) => true,
                _ => false,
            }
    }

    fn positive_range(constraint: &str, range: &IntegerRange) -> bool {
        match constraint {
            "positive" => !range.minimum.is_negative() && !range.minimum.is_zero(),
            "non_negative" => !range.minimum.is_negative(),
            _ => false,
        }
    }

    fn transition_argument_satisfies_named_constraint(
        proof_plan: &ProofPlan,
        obligation: &BoundedTransitionArgumentObligation,
        constraint: &str,
    ) -> bool {
        let constraints = type_constraints(proof_plan, obligation.argument_constraints);

        if constraints_satisfy_named_constraint(constraints, constraint) {
            return true;
        }

        if constraint == "finite"
            && float_range_for_transition_argument(proof_plan, obligation)
                .is_some_and(|range| range.proves_finite())
        {
            return true;
        }

        if matches!(constraint, "positive" | "non_negative") {
            let range = guarded_integer_range_for_transition_argument(proof_plan, obligation);
            return positive_range(constraint, &range);
        }

        if matches!(constraint, "exact")
            && integer_range_for_transition_argument(proof_plan, obligation)
                .is_some_and(|range| range.minimum == range.maximum)
        {
            return true;
        }

        argument_handle_satisfies_named_constraint(
            proof_plan,
            obligation.argument,
            obligation.argument_constraints,
            constraint,
        )
    }

    fn assignment_satisfies_named_constraint(
        proof_plan: &ProofPlan,
        obligation: &BoundedAssignmentObligation,
        constraint: &str,
    ) -> bool {
        let constraints = type_constraints(proof_plan, obligation.value_constraints);

        if constraints_satisfy_named_constraint(constraints, constraint) {
            return true;
        }

        if constraint == "finite"
            && float_range_for_assignment(proof_plan, obligation)
                .is_some_and(|range| range.proves_finite())
        {
            return true;
        }

        if matches!(constraint, "positive" | "non_negative")
            && let Some(range) = guarded_integer_range_for_assignment(proof_plan, obligation)
        {
            return positive_range(constraint, &range);
        }

        if matches!(constraint, "exact")
            && guarded_integer_range_for_assignment(proof_plan, obligation)
                .is_some_and(|range| range.minimum == range.maximum)
        {
            return true;
        }

        argument_handle_satisfies_named_constraint(
            proof_plan,
            obligation.value,
            obligation.value_constraints,
            constraint,
        )
    }

    fn initializer_satisfies_named_constraint(
        proof_plan: &ProofPlan,
        obligation: &BoundedInitializerObligation,
        constraint: &str,
    ) -> bool {
        if constraint == "finite"
            && float_range_for_initializer(proof_plan, obligation)
                .is_some_and(|range| range.proves_finite())
        {
            return true;
        }
        argument_handle_satisfies_named_constraint(
            proof_plan,
            obligation.value,
            obligation.value_constraints,
            constraint,
        )
    }

    fn constraints_satisfy_named_constraint(
        constraints: &[ProofConstraint],
        constraint: &str,
    ) -> bool {
        if constraints.iter().any(|argument_constraint| {
            matches!(
                argument_constraint,
                ProofConstraint::Named(argument_constraint)
                    if argument_constraint.as_str() == constraint
            )
        }) {
            return true;
        }

        match constraint {
            "exact" => integer_range_from_constraints(constraints).is_some(),
            "finite" => {
                integer_range_from_constraints(constraints).is_some()
                    || float_range_from_constraints(constraints)
                        .is_some_and(|range| range.proves_finite())
            }
            "non_negative" => integer_range_from_constraints(constraints)
                .is_some_and(|range| !range.minimum.is_negative()),
            "positive" => integer_range_from_constraints(constraints)
                .is_some_and(|range| !range.minimum.is_negative() && !range.minimum.is_zero()),
            "wrapping" => false,
            _ => false,
        }
    }
}
mod requires_conditions {
    use crate::checker::arrival_stability;
    use crate::checker::assignment_stability::collect_read_place_paths;
    use crate::checker::guards::{apply_left_literal_guard, apply_right_literal_guard};
    use crate::checker::integer_ranges::AssignmentRangeContext;
    use crate::obligations::{BinaryValueOperands, IntegerRange, ProofPlan, integer_binary_range};
    use symbols::SymbolHandle;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::signature::SignatureContractKind;

    pub(super) fn surviving_conditions(
        proof_plan: &ProofPlan<'_>,
        context: &AssignmentRangeContext<'_>,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        value: ExpressionHandle,
        binary_operands: Option<&BinaryValueOperands>,
    ) -> Vec<ExpressionHandle> {
        let program = proof_plan.program;
        let Some(machine) = program.machine(machine_symbol) else {
            return Vec::new();
        };
        let states = program.machine_states(machine);
        let Some(state) = typed_trees::state::state_by_symbol(states, state_symbol) else {
            return Vec::new();
        };
        let Some(call_frames) = context.call_frames() else {
            return Vec::new();
        };
        let is_entry = states
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol);
        let mut conditions = Vec::new();
        for contract in program
            .machine_contracts(machine)
            .iter()
            .filter(|_| is_entry)
            .chain(program.state_contracts(state))
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(condition) = fact.as_expression() else {
                    continue;
                };
                if !stable_expression(proof_plan, *condition) {
                    continue;
                }
                let mut premise_reads = Vec::new();
                collect_read_place_paths(proof_plan, *condition, &mut premise_reads);
                let mut value_reads = premise_reads.clone();
                collect_read_place_paths(proof_plan, value, &mut value_reads);
                if let Some(operands) = binary_operands {
                    collect_read_place_paths(proof_plan, operands.left, &mut value_reads);
                    collect_read_place_paths(proof_plan, operands.right, &mut value_reads);
                }
                if arrival_stability::prefix_preserves_reads(
                    proof_plan,
                    machine,
                    state,
                    statement_index,
                    &premise_reads,
                    &value_reads,
                    call_frames,
                ) {
                    conditions.push(*condition);
                }
            }
        }
        conditions
    }

    pub(super) fn refine(
        proof_plan: &ProofPlan<'_>,
        range: IntegerRange,
        value: ExpressionHandle,
        binary_operands: Option<&BinaryValueOperands>,
        conditions: &[ExpressionHandle],
    ) -> IntegerRange {
        let narrow = |mut range: IntegerRange, value: ExpressionHandle| {
            for condition in conditions {
                range = apply_condition(proof_plan, range, value, *condition);
            }
            range
        };
        let mut refined = narrow(range.clone(), value);
        if let Some(operands) = binary_operands
            && let (Some(left), Some(right)) = (&operands.left_range, &operands.right_range)
        {
            let left = narrow(left.clone(), operands.left);
            let right = narrow(right.clone(), operands.right);
            if left.minimum <= left.maximum
                && right.minimum <= right.maximum
                && let Some(folded) = integer_binary_range(operands.operator, left, right)
            {
                refined.minimum = refined.minimum.max(folded.minimum);
                refined.maximum = refined.maximum.min(folded.maximum);
            }
        }
        if refined.minimum > refined.maximum {
            range
        } else {
            refined
        }
    }

    pub(super) fn stable_expression(proof_plan: &ProofPlan<'_>, value: ExpressionHandle) -> bool {
        let table = &proof_plan.program.expression_table;
        if !table.expression_is_valid(value) {
            return false;
        }
        match table.expression(value) {
            ExpressionNode::Name(path) => path.symbol.is_valid() && path.head_symbol.is_valid(),
            ExpressionNode::Member(member) => {
                member.member_symbol.is_valid() && stable_expression(proof_plan, member.receiver)
            }
            ExpressionNode::Binary(binary) => {
                stable_expression(proof_plan, binary.left)
                    && stable_expression(proof_plan, binary.right)
            }
            ExpressionNode::Unary(unary) => stable_expression(proof_plan, unary.operand),
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
            _ => false,
        }
    }

    pub(super) fn same_place(
        proof_plan: &ProofPlan<'_>,
        condition_side: ExpressionHandle,
        subject: ExpressionHandle,
    ) -> bool {
        let table = &proof_plan.program.expression_table;
        matches!(
            table.expression(condition_side),
            ExpressionNode::Name(_) | ExpressionNode::Member(_)
        ) && stable_expression(proof_plan, condition_side)
            && stable_expression(proof_plan, subject)
            && table.expressions_structurally_equal(condition_side, subject)
    }

    fn apply_condition(
        proof_plan: &ProofPlan<'_>,
        range: IntegerRange,
        value: ExpressionHandle,
        condition: ExpressionHandle,
    ) -> IntegerRange {
        let table = &proof_plan.program.expression_table;
        let Some(binary) = table.binary_node(condition) else {
            return range;
        };
        if binary.operator == BinaryOperator::And {
            let range = apply_condition(proof_plan, range, value, binary.left);
            return apply_condition(proof_plan, range, value, binary.right);
        }
        if binary.operator == BinaryOperator::Equal {
            if matches!(
                table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) {
                return apply_condition(proof_plan, range, value, binary.left);
            }
            if matches!(table.expression(binary.left), ExpressionNode::Boolean(true)) {
                return apply_condition(proof_plan, range, value, binary.right);
            }
        }
        if same_place(proof_plan, binary.left, value) {
            apply_right_literal_guard(proof_plan, range, binary.operator, binary.right)
        } else if same_place(proof_plan, binary.right, value) {
            apply_left_literal_guard(proof_plan, range, binary.left, binary.operator)
        } else if let ExpressionNode::Cast(cast) = table.expression(value) {
            if same_place(proof_plan, binary.left, cast.value) {
                apply_right_literal_guard(proof_plan, range, binary.operator, binary.right)
            } else if same_place(proof_plan, binary.right, cast.value) {
                apply_left_literal_guard(proof_plan, range, binary.left, binary.operator)
            } else {
                range
            }
        } else {
            range
        }
    }
}
mod return_arrival {
    use crate::checker::integer_ranges::{AssignmentRangeContext, integer_range_for_return_value};
    use crate::checker::requires_conditions;
    use crate::obligations::{BoundedStateReturnObligation, IntegerRange, ProofPlan};
    use numerics::bignum::BigInt;
    use typed_trees::statement::StatementNode;

    pub(super) fn integer_range(
        proof_plan: &ProofPlan<'_>,
        obligation: &BoundedStateReturnObligation,
        context: &AssignmentRangeContext<'_>,
    ) -> Option<IntegerRange> {
        let program = proof_plan.program;
        let (_, state) =
            super::machine_and_state(program, obligation.machine_symbol, obligation.state_symbol)?;
        let statements = program.statement_table.statements(state.statement_nodes);
        if !std::ptr::eq(program, context.program)
            || !matches!(statements.get(obligation.statement_index),
            Some(StatementNode::Expression(value)) if *value == obligation.value)
        {
            return None;
        }
        if let Some((minimum, maximum)) = validation::arrival_integer_expression_bounds(
            program,
            obligation.machine_symbol,
            obligation.state_symbol,
            obligation.statement_index,
            obligation.value,
        ) {
            return Some(IntegerRange {
                minimum: BigInt::from_i64(minimum),
                maximum: BigInt::from_i64(maximum),
            });
        }
        let declared = integer_range_for_return_value(proof_plan, obligation)?;
        let conditions = requires_conditions::surviving_conditions(
            proof_plan,
            context,
            obligation.machine_symbol,
            obligation.state_symbol,
            obligation.statement_index,
            obligation.value,
            obligation.binary_operands.as_ref(),
        );
        Some(requires_conditions::refine(
            proof_plan,
            declared,
            obligation.value,
            obligation.binary_operands.as_ref(),
            &conditions,
        ))
    }
}

pub use derivation_cache::{DerivationCacheReport, ProofDerivationCache};
pub use integer_ranges::{AssignmentRangeContext, proved_assignment_integer_range_with_context};
pub use measurement::ProofPlanMeasurements;

fn machine_and_state(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    program.machine_state(machine_symbol, state_symbol)
}

use crate::checker::bounded_checks::{
    check_bounded_assignment, check_bounded_call_argument, check_bounded_initializer,
    check_bounded_state_return, check_bounded_transition_argument,
};
use crate::checker::derivation_cache::DerivationConsultation;
use crate::obligations::{ProofObligation, ProofPlan};
use diagnostics::Diagnostic;

macro_rules! check_obligations {
    (
        $obligation:ident, $plan:ident, $ranges:ident, $seed:ident,
        $diagnostics:ident, $recipes:ident, $measurements:ident, $consultation:ident;
        literal { $( $pat:pat => $body:block )* }
        ranged { $( $($rvariant:ident)|+ => $rfunc:ident ),* $(,)? }
        plain { $( $($variant:ident)|+ => $func:ident ),* $(,)? }
    ) => {
        match $obligation {
            $( $pat => $body )*
            $(
                $( ProofObligation::$rvariant(obligation) )|+ => {
                    $rfunc(
                        $plan,
                        obligation,
                        &$ranges,
                        $seed,
                        &mut $diagnostics,
                        &mut $recipes,
                        $measurements,
                        $consultation.as_mut(),
                    );
                }
            )*
            $(
                $( ProofObligation::$variant(obligation) )|+ => {
                    $func(
                        $plan,
                        obligation,
                        $seed,
                        &mut $diagnostics,
                        &mut $recipes,
                        $measurements,
                        $consultation.as_mut(),
                    );
                }
            )*
        }
    };
}

pub fn check_proof_plan(proof_plan: &ProofPlan) -> Result<(), Vec<Diagnostic>> {
    check_proof_plan_with_decisions(proof_plan).map(|_| ())
}

pub struct CheckedIntegerAssignment {
    pub obligation: arena::Handle<ProofObligation>,
    pub range: crate::obligations::IntegerRange,
}

pub struct ProofDecisions {
    pub integer_assignments: Vec<CheckedIntegerAssignment>,
    pub constant_recipe_obligations: Vec<checked_trees::ConstantRecipeObligation>,
}

pub fn check_proof_plan_with_decisions(
    proof_plan: &ProofPlan,
) -> Result<ProofDecisions, Vec<Diagnostic>> {
    let mut measurements = ProofPlanMeasurements::default();
    check_proof_plan_inner(proof_plan, &mut measurements, None)
}

fn check_proof_plan_inner(
    proof_plan: &ProofPlan<'_>,
    measurements: &mut ProofPlanMeasurements,
    mut derivation_cache: Option<&mut ProofDerivationCache>,
) -> Result<ProofDecisions, Vec<Diagnostic>> {
    let started = std::time::Instant::now();
    let mut diagnostics = Vec::new();
    let mut recipe_obligations = Vec::new();
    let range_context = AssignmentRangeContext::new(proof_plan);
    let mut checked_assignments = Vec::new();

    for (index, (obligation_handle, obligation)) in proof_plan.obligations.iter().enumerate() {
        let seed = index as u64 + 1;
        measurements.record_obligation(obligation);
        let decided_here = !matches!(
            obligation,
            ProofObligation::BoundedValue(_) | ProofObligation::GuardedTransition(_)
        );
        let diagnostics_before = diagnostics.len();
        let attempts_before = measurements.attempts();
        let mut consultation = if decided_here {
            derivation_cache
                .as_deref_mut()
                .map(|cache| DerivationConsultation::new(proof_plan, obligation, cache))
        } else {
            None
        };
        check_obligations!(
            obligation, proof_plan, range_context, seed,
            diagnostics, recipe_obligations, measurements, consultation;
            literal {
                ProofObligation::BoundedAssignment(obligation) => {
                    if let Some(range) = check_bounded_assignment(
                        proof_plan,
                        obligation,
                        &range_context,
                        seed,
                        &mut diagnostics,
                        &mut recipe_obligations,
                        measurements,
                        consultation.as_mut(),
                    ) {
                        checked_assignments.push(CheckedIntegerAssignment {
                            obligation: obligation_handle,
                            range,
                        });
                    }
                }
                ProofObligation::BoundedValue(_) | ProofObligation::GuardedTransition(_) => {}
            }
            ranged {
                BoundedStateReturn => check_bounded_state_return
            }
            plain {
                BoundedCallArgument => check_bounded_call_argument,
                BoundedInitializer => check_bounded_initializer,
                BoundedTransitionArgument => check_bounded_transition_argument
            }
        );
        let diagnosed = diagnostics.len() != diagnostics_before;
        if decided_here {
            measurements.record_outcome(diagnosed);
        }
        measurements.record_route(obligation, decided_here, diagnosed, attempts_before);
    }

    measurements.check_elapsed_microseconds =
        u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    measurements.emit_if_requested();

    if diagnostics.is_empty() {
        Ok(ProofDecisions {
            integer_assignments: checked_assignments,
            constant_recipe_obligations: recipe_obligations,
        })
    } else {
        Err(diagnostics)
    }
}
