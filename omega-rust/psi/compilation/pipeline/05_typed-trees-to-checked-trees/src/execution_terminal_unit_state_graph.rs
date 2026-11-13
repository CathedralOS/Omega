/*
    State-local calls and explicit successor bindings, independent of graph shape.

    `build_traced` plans a state machine as a composed Unit control
    graph. It fixes the machine's result signature and natural ranks, then
    the signature and entry claims of every state. Each state then plans its
    prefix initializers and statement sequence (`control::statement_sequence`),
    admits the sequence's operations (`operation_custody`), plans its
    terminator (`terminator`, with `closed_sum` and `returns`), and accounts
    for every structural result it produces (`result_custody`, with
    `local_results`). Finally the parameter custody of live states and the
    provider attachment requirements are checked.
*/

use super::{
    CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedGuardedJumpPlan, CheckedScalarExpressionRole,
    CheckedStructuralAccess, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralScalarArgumentPlan,
    CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, ExpressionNode,
    Multiplicity, PermissionEventKind, PermissionEventSource, StatementNode, TransitionExit,
    TransitionGuardNode, TransitionTargetNode, TypeReferenceNode,
};
use crate::execution::terminal_unit::ScalarCalleeClosure;
use crate::execution::terminal_unit::calls::{
    entry_claims, free_structural_scalar_entry_signature_traced,
};
use crate::execution::terminal_unit::structural_value_calls::PreparedCalls;
use crate::execution::terminal_unit::types::erased_proof_parameter_plans;
use crate::execution::terminal_unit::types::erased_scalar_parameter_plans;
use crate::execution::terminal_unit::types::parameter_storage_plans;
use crate::execution::terminal_unit::types::{
    ShapeCollector, machine_binders, retain_parameter_arrival_domains, state_flow,
};
use crate::execution::terminal_unit::{composed_control, control};
use crate::values::lower_state_domain_predicate;
use crate::values::lower_state_scalar_contract_predicate;

mod closed_sum {

    /*
        Case terminators inspect exact sums. Each selected arm transfers or disposes
        owned custody through normal-edge cleanup; a shared or mutable loan is read in place.
        Captured locals retain their real descriptor result and cross its referee,
        never re-evaluating the reference's formation or borrowing its owned custody.
        A field or fixed-index subject observes a subtree of that same carrier;
        it neither transfers the carrier nor turns a one-case sum into a guard.

        The dispatched sum is a parameter, an earlier immutable local, or a
        computed call subject. A computed subject's call is planned once at the
        first arm's statement; the parser's per-arm copies of it (each guard's
        subject and each payload read's receiver) all read that one result.
        Shared saved-dispatch resolution rejoins those exact authored occurrences
        to the producer's expression-root storage. Do not whitelist receivers by
        arm syntax here: ordinary scalar returns, calls and successor arguments
        all observe that storage. Call scheduling still requires its unique prior
        producer; independently authored calls keep their own occurrence and run.
    */
    use super::super::{
        CheckedClosedSumCaseOutcomePlan, CheckedClosedSumCasePlan as CasePlan,
        CheckedClosedSumPayloadTransferPlan, DataMember,
    };
    use super::{
        CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedScalarExpressionRole,
        CheckedStructuralAccess as StructuralAccess,
        CheckedUnitEffectOperationPlan as OperationPlan,
        CheckedUnitStructuralArgumentPlan as ArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan as SourcePlan, Multiplicity, ScalarCalleeClosure,
        ShapeCollector, Signature, TransitionExit, TransitionGuardNode, TransitionTargetNode,
        TypeReferenceNode, returns, terminal_calls,
    };
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::state_graph::{
        EdgeCalls, FusedStates, SuccessorEdge, successor_bindings,
    };
    use crate::execution::terminal_unit::types::base_type_identity;
    use crate::published::places::place_at;
    use crate::published::proof::exact_outcome_case_test;
    use crate::published::semantic::calls::dispatch_subjects;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn build(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        declarations: &super::loan_backing::DeclaredStoragePlans<'_>,
        records: &super::loan_backing::RetainedBorrowRecords<'_>,
        shapes: &mut ShapeCollector<'_>,
        fused: &FusedStates<'_>,
        state_index: usize,
        signatures: &[Signature],
        edge_calls: EdgeCalls<'_>,
        place_loans: &[checked_trees::CheckedPlaceLoanPlan],
        result: &checked_trees::CheckedControlResultPlan,
        next_scalar_binding: usize,
        operations: &mut Vec<OperationPlan>,
        transition_start: usize,
        trace: &LocalConstructionTrace,
    ) -> Option<CheckedComposedUnitControlTerminatorPlan> {
        let EdgeCalls {
            scalar_callees,
            entry_claims,
            local_results,
        } = edge_calls;
        let &(machine, state) = fused.get(state_index)?;
        let (structural, _) = &signatures[state_index];
        let statements = program.statement_table.statements(state.statement_nodes);
        let first = statements.get(transition_start)?.as_transition()?;
        let TransitionGuardNode::When(guard) = first.guard else {
            return None;
        };
        let (expression, _) = exact_outcome_case_test(program, guard)?;
        trace.phase("state graph: case subject storage");
        let subject_place = validation::contextual_canonical_place_from_expression(
            program,
            state.symbol,
            transition_start,
            expression,
        );
        let captured_place = crate::flow::captured_case_subject(
            program,
            crate::flow::CapturedSubjectFacts::from(facts),
            machine,
            state,
            transition_start,
            guard,
        )
        .and_then(|subject| {
            let source = facts.flow.control.operands.get(subject.source);
            place_at(
                program,
                source.state_symbol,
                source.statement_index,
                source.expression,
            )
        });
        let dispatch = match subject_place.as_ref() {
            Some(place) if place.root.as_symbol().is_some() => DispatchSubject::Place {
                subject: place,
                captured_source: captured_place.as_ref(),
            },
            _ => DispatchSubject::Computed(dispatch_subjects::evaluated_case_subject(
                program,
                statements,
                transition_start,
            )?),
        };
        let (mut subject, mut type_reference) = if let DispatchSubject::Computed(
            subject_expression,
        ) = dispatch
        {
            trace.phase("state graph: terminator: case subject call");
            trace.statement(u32::try_from(transition_start).ok());
            let (result, type_reference) = subject_call(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                state,
                structural,
                entry_claims,
                operations,
                transition_start,
                subject_expression,
                trace,
            )?;
            (
                ArgumentPlan::structural_result(
                    result.binding_ordinal,
                    result.type_identity,
                    StructuralAccess::Owned,
                ),
                type_reference,
            )
        } else if let Some(loan) = place_loans
            .iter()
            .find(|loan| dispatch.is_symbol(loan.symbol))
        {
            if loan.access != StructuralAccess::SharedBorrow
                || place_loans
                    .iter()
                    .filter(|candidate| candidate.symbol == loan.symbol)
                    .count()
                    != 1
            {
                return None;
            }
            let (formation, local) = program.state_local_declaration(state, loan.symbol)?;
            if crate::execution::terminal_unit::control::statement_sequence::dispatch_observation(
                program,
                facts,
                machine,
                state,
                structural,
                u32::try_from(formation).ok()?,
                local,
            )
            .as_ref()
                != Some(loan)
            {
                return None;
            }
            let source = match loan.root {
                checked_trees::CheckedPlaceLoanRoot::StructuralParameter { parameter_index } => {
                    SourcePlan::Parameter { parameter_index }
                }
                checked_trees::CheckedPlaceLoanRoot::Local { symbol } => {
                    SourcePlan::StructuralLocal { symbol }
                }
                checked_trees::CheckedPlaceLoanRoot::ScalarParameter { .. } => return None,
            };
            (
                ArgumentPlan {
                    projection_evaluations: Default::default(),
                    source,
                    path: loan.path.clone(),
                    type_identity: shapes.add_type(loan.referee, &[], &[])?,
                    access: loan.access,
                },
                loan.referee,
            )
        } else if let Some((index, parameter)) = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .find(|(_, parameter)| dispatch.is_symbol(parameter.symbol))
        {
            let (position, retained) = structural
                .iter()
                .enumerate()
                .find(|(_, retained)| retained.position as usize == index)?;
            let inspected = match retained.access {
                StructuralAccess::Owned => parameter.type_reference,
                checked_trees::checked_borrowed_access_pair_pattern!() => {
                    let TypeReferenceNode::Reference { referee, .. } = program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    else {
                        return None;
                    };
                    *referee
                }
                StructuralAccess::WriteOnlyBorrow => return None,
            };
            if !matches!(
                retained.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) || retained.multiplicity != program.type_multiplicity(parameter.type_reference)
                || !retained.qualifications.is_empty()
                || retained.primitive_referent.is_some()
            {
                return None;
            }
            (
                ArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: SourcePlan::Parameter {
                        parameter_index: u32::try_from(position).ok()?,
                    },
                    path: Vec::new(),
                    type_identity: retained.type_identity,
                    access: retained.access,
                },
                inspected,
            )
        } else {
            let mut matching = operations
                .iter()
                .filter_map(super::retained_structural_result)
                .filter_map(|result| {
                    let local = statements
                        .get(result.statement_index as usize)?
                        .as_local_data()?;
                    (dispatch.is_symbol(local.symbol)
                        && !local.is_mutable
                        && result.statement_index < transition_start as u32)
                        .then_some((result, local))
                });
            let (result, local) = matching.next()?;
            if matching.next().is_some()
                || !matches!(
                    result.multiplicity,
                    Multiplicity::Affine | Multiplicity::Unrestricted
                )
            {
                return None;
            }
            let (subject, inspected) =
                if let Some(shell) = validation::reference_shell(program, local.type_reference) {
                    if result.multiplicity != Multiplicity::Affine
                        || shell.access == language_semantics::ReferenceAccess::WriteOnly
                    {
                        return None;
                    }
                    shapes.add_type(shell.referee, &[], &[])?;
                    (
                        ArgumentPlan {
                            projection_evaluations: Default::default(),
                            source: SourcePlan::StructuralResult {
                                binding_ordinal: result.binding_ordinal,
                            },
                            path: vec![checked_trees::CheckedUnitStructuralPathSegment::Referent],
                            type_identity: base_type_identity(program, shell.referee, &[], &[])?,
                            access: StructuralAccess::SharedBorrow,
                        },
                        shell.referee,
                    )
                } else {
                    if result.multiplicity != program.type_multiplicity(local.type_reference) {
                        return None;
                    }
                    (
                        ArgumentPlan::structural_result(
                            result.binding_ordinal,
                            result.type_identity,
                            StructuralAccess::Owned,
                        ),
                        local.type_reference,
                    )
                };
            (subject, inspected)
        };
        let mut established_results = local_results.to_vec();
        if let DispatchSubject::Computed(expression) = dispatch {
            let SourcePlan::StructuralResult { binding_ordinal } = subject.source else {
                return None;
            };
            let result = operations
                .iter()
                .filter_map(super::retained_structural_result)
                .find(|result| result.binding_ordinal == binding_ordinal)?
                .clone();
            established_results.push((result, facts::PlaceRoot::Expression(expression)));
        }
        let edge_calls = EdgeCalls {
            local_results: &established_results,
            ..edge_calls
        };
        if let DispatchSubject::Place { subject: place, .. } = dispatch
            && !place.segments.is_empty()
        {
            let ProjectedArgumentPath {
                storage_type: projected,
                path,
                ..
            } = crate::execution::terminal_unit::calls::projected_argument_path(
                program,
                state.symbol,
                transition_start,
                place,
                None,
                None,
            )?;
            subject.path.extend(path);
            subject.type_identity = base_type_identity(program, projected, &[], &[])?;
            subject.access = StructuralAccess::SharedBorrow;
            type_reference = projected;
            shapes.add_type(projected, &[], &[])?;
        }
        trace.phase("state graph: case subject contents");
        if !validation::has_contents(
            program,
            type_reference,
            &[],
            validation::ContentsRequirement::BorrowedStorage,
        ) {
            return None;
        }
        trace.phase("state graph: case subject closed declaration");
        let data = program.closed_data_definition(type_reference)?;
        let variants = program
            .data_members(data)
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(variant) => Some(variant),
                _ => None,
            })
            .filter(|variant| !::validation::impossible_variant(program, variant))
            .collect::<Vec<_>>();
        trace.phase("state graph: case coverage roster");
        if variants.is_empty() || statements.len() - transition_start > variants.len() + 1 {
            return None;
        }
        trace.phase("state graph: case selected arms");
        let mut cases = Vec::new();
        let mut count = returns::next_result_ordinal(operations.as_slice())?;
        for (index, statement) in statements[transition_start..].iter().enumerate() {
            let transition = statement.as_transition()?;
            let ordinal = u32::try_from(transition_start + index).ok()?;
            let selected_variants = match transition.guard {
                TransitionGuardNode::When(guard) => {
                    let (expression, case_symbol) = exact_outcome_case_test(program, guard)?;
                    let case_symbol = program
                        .closed_data_constructor_case(type_reference, case_symbol)?
                        .symbol;
                    if !dispatch.names(program, state, ordinal as usize, expression) {
                        return None;
                    }
                    vec![
                        variants
                            .iter()
                            .copied()
                            .find(|variant| variant.symbol == case_symbol)?,
                    ]
                }
                TransitionGuardNode::Always if transition_start + index + 1 == statements.len() => {
                    variants
                        .iter()
                        .copied()
                        .filter(|variant| {
                            let identity = checked_trees::CheckedMemberIdentity::of_member(
                                variant.identity,
                                variant.name.clone(),
                            );
                            !cases
                                .iter()
                                .any(|case: &CasePlan| case.case_identity == identity)
                        })
                        .collect()
                }
                _ => return None,
            };
            if selected_variants.is_empty() {
                if matches!(transition.guard, TransitionGuardNode::Always) {
                    continue;
                }
                return None;
            }
            let arrival = validation::completing_transition_transfer(program, transition.target)
                .and_then(|arrival| {
                    super::fused_target_index(program, fused, arrival.target)
                        .map(|index| (index, arrival))
                });
            let returning_arm = match program.statement_table.transition_target(transition.target) {
                _ if arrival.is_some() => None,
                _ if terminal_calls::external_callable(program, machine, fused, transition)
                    .is_some() =>
                {
                    trace.phase("state graph: terminator: case successors: terminal call");
                    let arm = terminal_calls::arm_call(
                        program,
                        facts,
                        scalar_callees,
                        shapes,
                        machine,
                        state,
                        structural,
                        entry_claims,
                        edge_calls.local_results,
                        operations,
                        result,
                        next_scalar_binding,
                        transition,
                        ordinal,
                        trace,
                    )?;
                    Some(arm)
                }
                TransitionTargetNode::Value(expression)
                    if transition.exit == TransitionExit::Ordinary
                        && !transition.continuation.is_valid() =>
                {
                    trace.phase("state graph: terminator: case successors: return arm value");
                    trace.statement(Some(ordinal));
                    let arm = if let checked_trees::CheckedControlResultPlan::Scalar {
                        scalar_type: primitive_type,
                    } = result
                    {
                        let role = CheckedScalarExpressionRole::Return;
                        let retained = facts
                            .values
                            .scalar_expressions
                            .expression_at(state.symbol, ordinal, role)
                            .is_some()
                            || facts
                                .values
                                .scalar_computations
                                .root_at(state.symbol, ordinal, role)
                                .is_some_and(|root| root.machine == machine.symbol);
                        if !retained {
                            return None;
                        }
                        checked_trees::CheckedConditionalReturnArm::Scalar {
                            statement_ordinal: ordinal,
                            scalar_type: *primitive_type,
                        }
                    } else if *result == checked_trees::CheckedControlResultPlan::Unit {
                        checked_trees::CheckedConditionalReturnArm::Completion(
                            super::unit_completions::build(
                                program,
                                facts,
                                scalar_callees,
                                shapes,
                                machine,
                                state,
                                structural,
                                entry_claims,
                                edge_calls.local_results,
                                operations,
                                ordinal,
                                *expression,
                                trace,
                            )?,
                        )
                    } else {
                        let (operation, operand_calls) = returns::return_value_operation(
                            program,
                            facts,
                            scalar_callees,
                            shapes,
                            machine,
                            state,
                            structural,
                            entry_claims,
                            operations,
                            &mut count,
                            ordinal,
                            *expression,
                            trace,
                        )?;
                        operations.extend(operand_calls);
                        checked_trees::CheckedConditionalReturnArm::Structural(operation)
                    };
                    Some(arm)
                }
                TransitionTargetNode::Named { .. } => None,
                _ => return None,
            };
            for variant in selected_variants {
                let identity = checked_trees::CheckedMemberIdentity::of_member(
                    variant.identity,
                    variant.name.clone(),
                );
                if cases
                    .iter()
                    .any(|case: &CasePlan| case.case_identity == identity)
                {
                    return None;
                }
                let (outcome, payloads) = if let Some(arm) = &returning_arm {
                    (
                        CheckedClosedSumCaseOutcomePlan::Return(arm.clone()),
                        Vec::new(),
                    )
                } else {
                    let (target_index, arrival) = arrival.as_ref()?;
                    let target_index = *target_index;
                    let &(_, target) = fused.get(target_index)?;
                    let arguments = arrival.arguments;
                    let target_parameters = program.state_parameters(target);
                    let mut payloads = Vec::new();
                    for (parameter_index, parameter) in
                        signatures[target_index].1.iter().enumerate()
                    {
                        let argument_index = target_parameters
                            .iter()
                            .take(parameter.source_position as usize)
                            .count();
                        let argument = *arguments.get(argument_index)?;
                        let Some(field) = dispatch.payload_field(
                            program,
                            state,
                            ordinal as usize,
                            argument,
                            variant,
                        )?
                        else {
                            continue;
                        };
                        if !checked_trees::CheckedScalarType::from_type_reference(
                            program.typed_program(),
                            field.type_reference,
                        )?
                        .same_type(program.typed_program(), parameter.scalar_type)
                        .ok()?
                        {
                            return None;
                        }
                        payloads.push(CheckedClosedSumPayloadTransferPlan {
                            field_identity: checked_trees::CheckedMemberIdentity::of_member(
                                field.identity,
                                field.name.clone(),
                            ),
                            scalar_type: parameter.scalar_type,
                            target_scalar_parameter_index: u32::try_from(parameter_index).ok()?,
                        });
                    }
                    let payload_parameters = payloads
                        .iter()
                        .map(|payload| payload.target_scalar_parameter_index)
                        .collect::<Vec<_>>();
                    let successor = successor_bindings(
                        program,
                        facts,
                        declarations,
                        records,
                        fused,
                        state_index,
                        signatures,
                        operations.as_slice(),
                        transition,
                        ordinal,
                        &payload_parameters,
                        SuccessorEdge::ClosedCase,
                        edge_calls,
                        shapes,
                        trace,
                    )?;
                    (
                        CheckedClosedSumCaseOutcomePlan::Successor(successor),
                        payloads,
                    )
                };
                cases.push(CasePlan {
                    case_identity: identity,
                    outcome,
                    payloads,
                });
            }
        }
        if cases.len() != variants.len() {
            return None;
        }
        Some(CheckedComposedUnitControlTerminatorPlan::ClosedSum {
            subject,
            subject_type: type_reference,
            subject_kind: match dispatch {
                DispatchSubject::Place { .. } => checked_trees::CheckedClosedSumSubjectKind::Place,
                DispatchSubject::Computed(_) => {
                    checked_trees::CheckedClosedSumSubjectKind::Computed
                }
            },
            cases,
        })
    }

    /* The sum a closed dispatch inspects. */
    #[derive(Clone, Copy)]
    enum DispatchSubject<'place> {
        /* A parameter or an earlier immutable local, named by every arm. */
        Place {
            subject: &'place validation::CanonicalPlace,
            /*
                Authenticated completed backing of an observed descriptor. Payload
                receivers can retain this source while the guard uses the descriptor.
            */
            captured_source: Option<&'place validation::CanonicalPlace>,
        },
        /*
            One authored call. The parser copies the dispatch subject into every
            arm's guard and payload read; each copy keeps the one authored call's
            selection occurrence. The dispatch evaluates that call once, at the
            first arm's statement, and every copy reads its single result
            (wiki/spec/language/expressions.md, "Transition dispatch").
        */
        Computed(typed_trees::expression::ExpressionHandle),
    }

    impl DispatchSubject<'_> {
        fn is_symbol(self, symbol: symbols::SymbolHandle) -> bool {
            matches!(self, Self::Place { subject, .. } if subject.root == facts::PlaceRoot::Symbol(symbol))
        }

        fn is_symbol_place(
            self,
            program: &dyn validation::CheckingInput,
            place: &validation::CanonicalPlace,
        ) -> bool {
            matches!(self, Self::Place { subject, .. }
                if crate::flow::canonical_places_agree(program, subject, place))
        }

        /* Whether `expression` names this subject at `statement_index`. */
        fn names(
            self,
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> bool {
            match self {
                Self::Place { .. } => validation::contextual_canonical_place_from_expression(
                    program,
                    state.symbol,
                    statement_index,
                    expression,
                )
                .is_some_and(|place| self.is_symbol_place(program, &place)),
                Self::Computed(subject) => {
                    dispatch_subjects::repeats_subject(program, subject, expression)
                }
            }
        }

        /*
            An exact bare payload can forward the case's prepared scalar directly.
            Other expressions use the ordinary completed operand graph, including
            projections inside a copied payload. This forwarding pattern is not an
            eligibility gate for checked scalar computations.
        */
        #[allow(clippy::too_many_arguments)]
        fn payload_field<'program>(
            self,
            program: &'program dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            argument: typed_trees::expression::ExpressionHandle,
            variant: &'program typed_trees::data::DataVariant,
        ) -> Option<Option<&'program typed_trees::data::DataField>> {
            if let Some(operand) = crate::values::scalar::semantic_casts::same_carrier_operand(
                program,
                state.symbol,
                argument,
            ) {
                return self.payload_field(program, state, statement_index, operand, variant);
            }
            match self {
                Self::Place {
                    subject,
                    captured_source,
                } => {
                    let Some(place) = validation::contextual_canonical_place_from_expression(
                        program,
                        state.symbol,
                        statement_index,
                        argument,
                    ) else {
                        return Some(None);
                    };
                    let Some(subject) = [Some(subject), captured_source]
                        .into_iter()
                        .flatten()
                        .find(|subject| {
                            place.root == subject.root
                                && place.segments.len() >= subject.segments.len()
                                && place.segments.iter().zip(&subject.segments).all(
                                    |(left, right)| {
                                        crate::flow::canonical_place_segments_equal(
                                            program, *left, *right,
                                        )
                                    },
                                )
                        })
                    else {
                        return Some(None);
                    };
                    let [
                        facts::PlaceSegment::Case { variant: selected },
                        facts::PlaceSegment::Field { symbol },
                    ] = &place.segments[subject.segments.len()..]
                    else {
                        return Some(None);
                    };
                    if *selected != variant.symbol {
                        return None;
                    }
                    let field = program.data_payload_field_by_symbol(variant, *symbol)?;
                    Some(Some(field))
                }
                Self::Computed(subject) => {
                    let Some(member) = program.expression_table.member_node(argument) else {
                        return Some(None);
                    };
                    if !dispatch_subjects::repeats_subject(program, subject, member.receiver) {
                        return Some(None);
                    }
                    if member
                        .case_variant
                        .as_ref()
                        .is_none_or(|name| name.as_str() != variant.name.as_str())
                    {
                        return None;
                    }
                    let field = program.data_payload_fields(variant).iter().find(|field| {
                        field.name.as_str() == member.member.as_str()
                            && (!member.member_symbol.is_valid()
                                || member.member_symbol == field.symbol)
                    })?;
                    Some(Some(field))
                }
            }
        }
    }

    /*
        Plan the computed subject's one call at the first arm's statement. Its
        structural result is bound like a `let` local's, anchored at the subject
        expression, and consumed by the dispatch.
    */
    #[allow(clippy::too_many_arguments)]
    fn subject_call(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural: &[checked_trees::CheckedUnitStructuralParameterPlan],
        entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
        operations: &mut Vec<OperationPlan>,
        transition_start: usize,
        subject: typed_trees::expression::ExpressionHandle,
        trace: &LocalConstructionTrace,
    ) -> Option<(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        typed_trees::types::TypeReferenceHandle,
    )> {
        let flow = crate::execution::terminal_unit::types::state_flow(
            facts,
            machine.symbol,
            state.symbol,
        )?;
        let mut matching = crate::flow::call_flows(&facts.flow.control, flow.calls)
            .iter()
            .filter(|call| {
                call.statement_index == transition_start && call.authored_expression == subject
            });
        let call = matching.next()?;
        if matching.next().is_some()
            || facts.flow.control.is_retired(state.symbol, call)
            || operations
                .iter()
                .filter_map(|operation| operation.call())
                .any(|planned| {
                    planned.coordinate.statement_index as usize == call.statement_index
                        && planned.coordinate.call_ordinal as usize == call.call_ordinal
                })
        {
            return None;
        }
        let return_type = validation::call_target_return_type(program, call.target_symbol)?;
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut structural_results = operations
            .iter()
            .filter_map(|operation| {
                let result = operation.structural_result_binding()?;
                let local = statements
                    .get(result.statement_index as usize)?
                    .as_local_data()?;
                local
                    .symbol
                    .is_valid()
                    .then(|| (result.clone(), facts::PlaceRoot::Symbol(local.symbol)))
            })
            .collect::<Vec<_>>();
        let mut structural_count = returns::next_result_ordinal(operations)?;
        crate::execution::terminal_unit::control::structural_operands::plan_call_operands(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural,
            &[],
            entry_claims,
            &mut structural_results,
            operations,
            &mut structural_count,
            call,
            trace,
            &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
        )?;
        let result = checked_trees::CheckedUnitStructuralResultBindingPlan {
            statement_index: u32::try_from(transition_start).ok()?,
            binding_ordinal: u32::try_from(structural_count).ok()?,
            type_identity: shapes.add_type(return_type, &[], &[])?,
            multiplicity: program.type_multiplicity(return_type),
            local: Default::default(),
        };
        let operation = crate::execution::terminal_unit::calls::build_call_operation(
            program,
            shapes,
            facts,
            Some(scalar_callees),
            machine,
            state,
            structural,
            &[],
            call,
            Some(
                crate::execution::terminal_unit::calls::ExpectedCallValueResult::Structural(
                    &result,
                ),
            ),
            &structural_results,
            operations,
            trace,
        )?;
        let mut operation = crate::execution::terminal_unit::control::bind_structural_call_result(
            program,
            facts,
            machine.symbol,
            state.symbol,
            operation,
            result.clone(),
        )?;
        let (_, discard_result_on_return) = operation.discardable_structural_result_mut()?;
        *discard_result_on_return = false;
        let OperationPlan::Call(planned) = &operation else {
            return None;
        };
        if (planned.is_boundary() && !planned.claim_transfers().is_empty())
            || !super::operation_custody::call_custody_admitted(planned, operations)
        {
            return None;
        }
        let coordinate = planned.coordinate;
        crate::execution::terminal_unit::control::statement_sequence::consume_results(
            operations, &operation,
        )?;
        operations.push(operation);
        crate::execution::terminal_unit::cleanup::anonymous::insert_continuation(
            program,
            facts,
            shapes,
            machine,
            state,
            coordinate,
            operations.len(),
            &structural_results,
            operations,
            trace,
        )?;
        Some((result, return_type))
    }
}
mod fused {
    /*
        The fused-state roster a composed control graph plans over.

        `fused_states` keeps recursive components and shared activation custody
        in one graph, rather than fusing every reachable callee. Recursive tail
        transfers retain their shared rank and constant-stack execution. The rest
        of this module's machinery — signatures, entry claims, successors — reads
        this roster through `FusedStates`, each state keeping its authored owner
        for signatures and proof facts.
    */
    use super::super::StatementNode;

    /*
        A composed graph's state roster: the dispatch machine's states followed by
        the foreign machines in its recursive component. Each entry pairs the
        state with its owning machine — signatures, entry claims, and every
        machine-scoped fact answer through the owner, never the dispatch machine.
    */
    pub(in crate::execution::terminal_unit) type FusedStates<'program> = Vec<(
        &'program typed_trees::machine::Machine,
        &'program typed_trees::state::State,
    )>;

    /*
        Keep recursive tail transfers in one graph so their rank and stack use
        remain shared. A tail transfer also stays here when it carries activation
        custody or overlapping borrowed storage. Other acyclic calls keep their
        ordinary operand preparation and completion cleanup. Each fused state
        retains its authored owner for signatures and proof facts.
    */
    pub(in crate::execution::terminal_unit) fn fused_states<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
    ) -> FusedStates<'program> {
        let mut fused: FusedStates<'program> = program
            .machine_states(machine)
            .iter()
            .map(|state| (machine, state))
            .collect();
        let mut cursor = 0;
        while let Some(&(_, state)) = fused.get(cursor) {
            cursor += 1;
            for target_symbol in completing_transition_targets(program, state) {
                if fused_target_index(program, &fused, target_symbol).is_some() {
                    continue;
                }
                let Some((target_owner, _)) = validation::find_machine_head(program, target_symbol)
                else {
                    continue;
                };
                if !target_owner.body_is_present {
                    continue;
                }
                for owner_state in program.machine_states(target_owner) {
                    if !fused
                        .iter()
                        .any(|(_, candidate)| candidate.symbol == owner_state.symbol)
                    {
                        fused.push((target_owner, owner_state));
                    }
                }
            }
        }
        let mut owners = fused
            .iter()
            .map(|(owner, _)| owner.symbol)
            .collect::<Vec<_>>();
        owners.dedup();
        if owners.len() <= 1 {
            return fused;
        }
        let mut predecessors = vec![Vec::new(); owners.len()];
        for (owner, state) in &fused {
            let Some(source_index) = owners.iter().position(|symbol| *symbol == owner.symbol)
            else {
                continue;
            };
            for target_symbol in completing_transition_targets(program, state) {
                let Some(target_index) = fused_target_index(program, &fused, target_symbol) else {
                    continue;
                };
                let target_owner = fused[target_index].0.symbol;
                if let Some(target_index) = owners.iter().position(|symbol| *symbol == target_owner)
                {
                    predecessors[target_index].push(source_index);
                }
            }
        }
        for incoming in &mut predecessors {
            incoming.sort_unstable();
            incoming.dedup();
        }
        let components = crate::checks::termination::strongly_connected_components(&predecessors);
        let mut component_of = vec![0; owners.len()];
        for (index, component) in components.iter().enumerate() {
            for &owner in component {
                component_of[owner] = index;
            }
        }
        let mut retained = vec![false; owners.len()];
        let mut ready = Vec::new();
        for &owner in &components[component_of[0]] {
            retained[owner] = true;
            ready.push(owner);
        }
        let mut cursor = 0;
        while let Some(&source) = ready.get(cursor) {
            cursor += 1;
            let source_states = fused
                .iter()
                .filter(|(owner, _)| owner.symbol == owners[source])
                .map(|(_, state)| *state);
            for state in source_states {
                for (statement_index, transfer) in completing_transition_transfers(program, state) {
                    let Some(target_index) = fused_target_index(program, &fused, transfer.target)
                    else {
                        continue;
                    };
                    let (target_owner, target_state) = fused[target_index];
                    let Some(target) = owners
                        .iter()
                        .position(|symbol| *symbol == target_owner.symbol)
                    else {
                        continue;
                    };
                    if !retained[target]
                        && (moves_activation_claim(program, target_state)
                            || shares_borrowed_storage(program, state, statement_index, &transfer))
                    {
                        for &owner in &components[component_of[target]] {
                            if !retained[owner] {
                                retained[owner] = true;
                                ready.push(owner);
                            }
                        }
                    }
                }
            }
        }
        fused.retain(|(owner, _)| {
            owners
                .iter()
                .position(|symbol| *symbol == owner.symbol)
                .is_some_and(|owner_index| retained[owner_index])
        });
        fused
    }

    /*
        Whether arriving at `entry` moves activation-scoped custody: one of its
        owned (by-value) parameters carries an activation claim.
    */
    fn moves_activation_claim(
        program: &dyn ::validation::CheckingInput,
        entry: &typed_trees::state::State,
    ) -> bool {
        program.state_parameters(entry).iter().any(|parameter| {
            ::validation::type_carries_activation_claim(program, parameter.type_reference)
        })
    }

    /*
        Every complete transition invocation in authored order. Applied calls
        retain their full source payload; exact resolution supplies the same
        component edge as a named arrival without rewriting the typed tree.
    */
    fn completing_transition_targets<'a>(
        program: &'a dyn ::validation::CheckingInput,
        state: &'a typed_trees::state::State,
    ) -> impl Iterator<Item = symbols::SymbolHandle> + 'a {
        completing_transition_transfers(program, state).map(|(_, transfer)| transfer.target)
    }

    fn completing_transition_transfers<'a>(
        program: &'a dyn ::validation::CheckingInput,
        state: &'a typed_trees::state::State,
    ) -> impl Iterator<Item = (usize, validation::CompletingTransfer<'a>)> + 'a {
        crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .enumerate()
            .filter_map(|(index, statement)| match statement {
                StatementNode::Transition(transition) => Some((index, transition)),
                _ => None,
            })
            .flat_map(move |(index, transition)| {
                transition.targets().filter_map(move |target| {
                    validation::completing_transition_transfer(program, target)
                        .map(|transfer| (index, transfer))
                })
            })
    }

    // Keep suspended storage and its child loan in the same activation. This
    // selects the graph only; ordinary successor planning still checks every
    // argument, captured selector, invocation contract and cleanup obligation.
    fn shares_borrowed_storage(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        transfer: &validation::CompletingTransfer<'_>,
    ) -> bool {
        let borrowed = transfer
            .arguments
            .iter()
            .zip(program.state_parameters(transfer.state))
            .filter_map(|(argument, parameter)| {
                let shell = validation::reference_shell(program, parameter.type_reference)?;
                let place = crate::published::places::place_at(
                    program,
                    state.symbol,
                    statement_index,
                    *argument,
                )?;
                Some((place, shell.access))
            })
            .collect::<Vec<_>>();
        borrowed.iter().enumerate().any(|(index, (left, access))| {
            borrowed[index + 1..].iter().any(|(right, other_access)| {
                (*access != language_semantics::ReferenceAccess::Shared
                    || *other_access != language_semantics::ReferenceAccess::Shared)
                    && left.root == right.root
                    && (left.segments.starts_with(&right.segments)
                        || right.segments.starts_with(&left.segments))
            })
        })
    }

    /*
        The fused position of a named transition target: a state symbol resolves
        directly, while a machine head resolves to that owner's entry state —
        the same normalization `activation_target_state_index` applies to
        one machine's own roster.
    */
    pub(in crate::execution::terminal_unit) fn fused_target_index(
        program: &dyn ::validation::CheckingInput,
        fused: &FusedStates<'_>,
        target: symbols::SymbolHandle,
    ) -> Option<usize> {
        if let Some(index) = fused.iter().position(|(_, state)| state.symbol == target) {
            return Some(index);
        }
        let (_, entry) = validation::find_machine_head(program, target)?;
        fused
            .iter()
            .position(|(_, state)| state.symbol == entry.symbol)
    }
}
mod guards {
    /*
        Retained guard expressions and source-checked arrival contracts.

        Checking proves every state transfer, including the implicit requirements
        from qualified parameters and their fields. Planning consumes that decision;
        it does not re-prove membership by recognizing carrier shapes. Exact operand
        transfer, ownership, and emitted qualification transport remain separate.
    */
    use super::super::{
        CheckFacts, CheckedComposedUnitControlStatePlan, CheckedScalarExpressionRole, PrimitiveType,
    };
    use checked_trees::CheckedCallScalarArgument;
    use symbols::SymbolHandle;

    /*
        Every live edge into a qualified state must have a completed call check.
        Invocation-only entry is handled by its caller; a reentered entry still
        checks all graph arrivals here. The decision summarizes all calls at its
        coordinate, not a unique operand: transfer planning retains that identity.

        A decision may retain a constant obligation. This permits planning only;
        machine lowering refuses pending obligations before Terminal production.
    */
    pub(in crate::execution::terminal_unit) fn checked_qualified_arrivals(
        facts: &CheckFacts,
        planned: &[CheckedComposedUnitControlStatePlan],
        live: &[bool],
        target_index: usize,
    ) -> bool {
        let target = planned[target_index].state;
        let mut has_arrival = false;
        for (source, live) in planned.iter().zip(live) {
            if !live {
                continue;
            }
            for successor in source.successors() {
                if successor.target_state != target {
                    continue;
                }
                has_arrival = true;
                if !facts.call_requires_decided(
                    source.state,
                    successor.statement_ordinal,
                    successor.target_state,
                ) {
                    return false;
                }
            }
        }
        has_arrival
    }

    /*
        The exact Boolean value one authored guard retains at its `Guard`
        coordinate: the pure expression, or the unique computation root this
        machine owns there (a selected comparison, a call, or any other checked
        computation). The computation producer records a root only where no pure
        expression exists, so the coordinate names at most one of the two forms.
    */
    pub(in crate::execution::terminal_unit) fn retained_guard(
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<CheckedCallScalarArgument> {
        let role = CheckedScalarExpressionRole::Guard;
        if let Some(expression) =
            facts
                .values
                .scalar_expressions
                .expression_at(state, statement_ordinal, role)
        {
            return Some(CheckedCallScalarArgument::Pure(expression.clone()));
        }
        let computations = &facts.values.scalar_computations;
        let root = computations.root_at(state, statement_ordinal, role)?;
        (root.machine == machine && computations.nodes.is_valid(root.root))
            .then_some(CheckedCallScalarArgument::Computation(root.root))
    }

    pub(in crate::execution::terminal_unit) fn guard_is_boolean(
        facts: &CheckFacts,
        guard: &CheckedCallScalarArgument,
    ) -> bool {
        match guard {
            CheckedCallScalarArgument::Pure(expression) => {
                expression.primitive_type() == Some(PrimitiveType::Bool)
            }
            CheckedCallScalarArgument::Computation(root) => {
                facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(*root)
                    .scalar_type
                    == PrimitiveType::Bool
            }
        }
    }
}
mod loan_backing {
    /*
        Captured loan resources name the actual storage retained across an arrival,
        including stored-field crossings and call-returned references. Missing
        descriptor provenance refuses retention rather than guessing a local home.
        A selected arrival may retire a descriptor and transport its owned backing
        only when no other captured value needs that backing's address. Retirement
        alone says nothing about copied descriptors. Use the checked call's complete
        operand resources and their stored dependencies, not producer spellings or
        lexical last-use indices. Unresolved coverage retains the stable-home route.
    */
    use crate::flow::literal_value_path_is_inactive;
    use crate::shared::collections::push_unique;
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        BorrowLoanFact, CheckFacts, CheckedCallBorrowLoanSource as LoanSource,
        CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
        FlowConstraintRef,
    };
    use facts::PlaceRoot;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    use crate::borrow::accesses::is_image_storage_root;
    use crate::published::places::place_at;

    mod record_selections {
        /*
            Arena-ordered candidate rows for one immutable effect-planning request.
            These selections grant no loan or retention verdict. Every consumer keeps
            its original exact predicates and validates each selected resource normally.
        */
        use arena::Handle;
        use checked_trees::{
            CheckFacts, CheckedCallBorrowLoanResource as LoanResource, FlowCallFact,
            IncomingReferenceIdentity as ReferenceIdentity, KeyedRows,
        };
        use symbols::SymbolHandle;

        type CallKey = (Handle<FlowCallFact>, SymbolHandle, SymbolHandle);

        pub(in crate::execution::terminal_unit) struct RetainedBorrowRecords<'facts> {
            facts: &'facts CheckFacts,
            calls: KeyedRows<CallKey, LoanResource>,
            incoming: KeyedRows<(SymbolHandle, SymbolHandle), ReferenceIdentity>,
        }

        impl<'facts> RetainedBorrowRecords<'facts> {
            pub(in crate::execution::terminal_unit) fn new(facts: &'facts CheckFacts) -> Self {
                Self {
                    facts,
                    calls: KeyedRows::default(),
                    incoming: KeyedRows::default(),
                }
            }

            pub(super) fn facts(&self) -> &'facts CheckFacts {
                self.facts
            }

            pub(super) fn call_resources(
                &self,
                call: Handle<FlowCallFact>,
                machine: SymbolHandle,
                state: SymbolHandle,
            ) -> Vec<(Handle<LoanResource>, &'facts LoanResource)> {
                let arena = &self.facts.borrow.call_loan_resources;
                self.calls.with_rows(
                    arena,
                    |row| (row.call, row.machine_symbol, row.state_symbol),
                    &(call, machine, state),
                    |rows| {
                        rows.iter()
                            .map(|&handle| (handle, arena.get(handle)))
                            .collect()
                    },
                )
            }

            pub(super) fn incoming_references(
                &self,
                machine: SymbolHandle,
                state: SymbolHandle,
            ) -> Vec<(Handle<ReferenceIdentity>, &'facts ReferenceIdentity)> {
                let arena = &self.facts.borrow.incoming_references;
                self.incoming.with_rows(
                    arena,
                    |row| (row.machine_symbol, row.state_symbol),
                    &(machine, state),
                    |rows| {
                        rows.iter()
                            .map(|&handle| (handle, arena.get(handle)))
                            .collect()
                    },
                )
            }
        }

        #[cfg(test)]
        mod tests {
            use super::RetainedBorrowRecords;
            use arena::Handle;
            use checked_trees::{
                CheckFacts, CheckedCallBorrowLoanResource, IncomingReferenceIdentity,
                IncomingReferenceOrigin,
            };
            use symbols::SymbolHandle;

            #[test]
            fn selections_match_complete_arena_filters_and_keep_exact_keys_and_order() {
                let machine = SymbolHandle::from_parts(11, 2);
                let state = SymbolHandle::from_parts(12, 3);
                let other_machine = SymbolHandle::from_parts(11, 4);
                let other_state = SymbolHandle::from_parts(12, 5);
                let call = Handle::from_parts(21, 6);
                let other_call = Handle::from_parts(21, 7);
                let operand = Handle::from_parts(31, 8);
                let other_operand = Handle::from_parts(31, 9);
                let mut facts = CheckFacts::default();
                let path = facts
                    .borrow
                    .owner_segments
                    .insert_many([checked_trees::BorrowLoanOwnerSegment::FixedIndex(0)]);
                let base = CheckedCallBorrowLoanResource {
                    call,
                    machine_symbol: machine,
                    state_symbol: state,
                    owner_operand: operand,
                    ..Default::default()
                };
                for row in [
                    base.clone(),
                    CheckedCallBorrowLoanResource {
                        machine_symbol: other_machine,
                        ..base.clone()
                    },
                    CheckedCallBorrowLoanResource {
                        state_symbol: other_state,
                        ..base.clone()
                    },
                    CheckedCallBorrowLoanResource {
                        call: other_call,
                        ..base.clone()
                    },
                    CheckedCallBorrowLoanResource {
                        owner_operand: other_operand,
                        ..base.clone()
                    },
                    CheckedCallBorrowLoanResource {
                        owner_path: path,
                        ..base.clone()
                    },
                    base.clone(),
                    CheckedCallBorrowLoanResource {
                        owner_operand: Handle::invalid(),
                        ..base.clone()
                    },
                ] {
                    facts.borrow.call_loan_resources.insert(row);
                }
                let entry = IncomingReferenceIdentity {
                    machine_symbol: machine,
                    state_symbol: state,
                    origin: IncomingReferenceOrigin::Parameter { symbol: machine },
                    ..Default::default()
                };
                for row in [
                    entry.clone(),
                    IncomingReferenceIdentity {
                        machine_symbol: other_machine,
                        ..entry.clone()
                    },
                    IncomingReferenceIdentity {
                        state_symbol: other_state,
                        ..entry.clone()
                    },
                    IncomingReferenceIdentity {
                        origin: IncomingReferenceOrigin::ParameterReferent { symbol: machine },
                        ..entry.clone()
                    },
                    entry.clone(),
                ] {
                    facts.borrow.incoming_references.insert(row);
                }
                let records = RetainedBorrowRecords::new(&facts);
                assert!(std::ptr::eq(records.facts(), &facts));
                for (selected_call, selected_machine, selected_state) in [
                    (call, machine, state),
                    (other_call, machine, state),
                    (call, other_machine, state),
                    (call, machine, other_state),
                    (Handle::invalid(), machine, state),
                ] {
                    let original: Vec<_> = facts
                        .borrow
                        .call_loan_resources
                        .iter()
                        .filter(|(_, row)| {
                            row.call == selected_call
                                && row.machine_symbol == selected_machine
                                && row.state_symbol == selected_state
                        })
                        .collect();
                    for _ in 0..2 {
                        let selected =
                            records.call_resources(selected_call, selected_machine, selected_state);
                        assert_eq!(selected, original);
                        for root_only in [false, true] {
                            let filter = |row: &CheckedCallBorrowLoanResource| {
                                row.owner_operand == operand
                                    && (!root_only || row.owner_path.is_empty())
                            };
                            assert_eq!(
                                selected
                                    .iter()
                                    .filter(|(_, row)| filter(row))
                                    .map(|(h, _)| *h)
                                    .collect::<Vec<_>>(),
                                original
                                    .iter()
                                    .filter(|(_, row)| filter(row))
                                    .map(|(h, _)| *h)
                                    .collect::<Vec<_>>()
                            );
                        }
                    }
                }
                for (selected_machine, selected_state) in [
                    (machine, state),
                    (other_machine, state),
                    (machine, other_state),
                ] {
                    let original: Vec<_> = facts
                        .borrow
                        .incoming_references
                        .iter()
                        .filter(|(_, row)| {
                            row.machine_symbol == selected_machine
                                && row.state_symbol == selected_state
                        })
                        .collect();
                    let selected = records.incoming_references(selected_machine, selected_state);
                    assert_eq!(selected, original);
                    assert_eq!(
                        selected
                            .iter()
                            .filter(|(_, row)| matches!(
                                row.origin,
                                IncomingReferenceOrigin::Parameter { .. }
                            ))
                            .map(|(h, _)| *h)
                            .collect::<Vec<_>>(),
                        original
                            .iter()
                            .filter(|(_, row)| matches!(
                                row.origin,
                                IncomingReferenceOrigin::Parameter { .. }
                            ))
                            .map(|(h, _)| *h)
                            .collect::<Vec<_>>()
                    );
                }
                let empty = CheckFacts::default();
                let separate = RetainedBorrowRecords::new(&empty);
                assert!(separate.call_resources(call, machine, state).is_empty());
                assert!(separate.incoming_references(machine, state).is_empty());
                assert_eq!(records.call_resources(call, machine, state).len(), 5);
            }
        }
    }
    pub(in crate::execution::terminal_unit) use record_selections::RetainedBorrowRecords;

    /*
        One graph request borrows one complete checking input. Only declaration
        geometry is shared: roots, selected paths, constraints, loans and dependency
        verdicts remain live inputs to every retention question below.
    */
    type DeclaredOwnerPaths = std::sync::Arc<[Vec<checked_trees::BorrowLoanOwnerSegment>]>;

    pub(in crate::execution::terminal_unit) struct DeclaredStoragePlans<'program> {
        program: &'program dyn validation::CheckingInput,
        owner_paths: symbols::SymbolKeyMap<
            typed_trees::types::TypeReferenceHandle,
            Option<DeclaredOwnerPaths>,
        >,
    }

    impl<'program> DeclaredStoragePlans<'program> {
        pub(super) fn new(
            program: &'program dyn validation::CheckingInput,
            carriers: impl IntoIterator<Item = typed_trees::types::TypeReferenceHandle>,
        ) -> Self {
            let mut owner_paths = symbols::SymbolKeyMap::default();
            for carrier in carriers {
                if owner_paths.contains_key(&carrier) {
                    continue;
                }
                let paths = validation::reference_result_custody::declared_carrier_references(
                    program, carrier,
                )
                .and_then(|sources| {
                    sources
                        .into_iter()
                        .map(|source| {
                            let segments =
                                validation::reference_result_custody::origin_place_segments(
                                    program,
                                    carrier,
                                    source.path,
                                )?;
                            Some(crate::borrow::owner_path_from_place_segments(
                                program, &segments,
                            ))
                        })
                        .collect::<Option<Vec<_>>>()
                })
                .map(std::sync::Arc::from);
                owner_paths.insert(carrier, paths);
            }
            Self {
                program,
                owner_paths,
            }
        }

        fn for_carrier(
            &self,
            carrier: typed_trees::types::TypeReferenceHandle,
        ) -> Option<DeclaredOwnerPaths> {
            self.owner_paths.get(&carrier).cloned().flatten()
        }
    }

    struct IngressStorageCarrier<'plan> {
        root: PlaceRoot,
        carrier: typed_trees::types::TypeReferenceHandle,
        declarations: &'plan DeclaredStoragePlans<'plan>,
    }

    #[cfg(test)]
    mod declared_storage_plan_tests {
        use super::DeclaredStoragePlans;
        use crate::borrow::owner_path_from_place_segments;
        use checked_trees::BorrowLoanOwnerSegment;
        use language_core::ReferenceAccess;
        use std::sync::Arc;
        use typed_trees::TypedTrees;
        use typed_trees::name::Identifier;
        use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

        /*
            The original uncached reader/conversion is the oracle. The association may
            retain its answer, never replace its source, type or refusal judgments.
        */
        fn original_paths(
            program: &dyn validation::CheckingInput,
            carrier: TypeReferenceHandle,
        ) -> Option<Vec<Vec<BorrowLoanOwnerSegment>>> {
            validation::reference_result_custody::declared_carrier_references(program, carrier)?
                .into_iter()
                .map(|source| {
                    let segments = validation::reference_result_custody::origin_place_segments(
                        program,
                        carrier,
                        source.path,
                    )?;
                    Some(owner_path_from_place_segments(program, &segments))
                })
                .collect()
        }

        #[test]
        fn declaration_storage_plans_retain_exact_paths_refusals_and_request_scope() {
            let mut program = TypedTrees::default();
            let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
            let reference = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: unit,
                    access: ReferenceAccess::Mutable,
                    lifetime: Some(Identifier::from("loan")),
                });
            let elements = program
                .type_reference_table
                .insert(TypeReferenceNode::Slice {
                    element_type: reference,
                });
            let array = program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: elements,
                    length: FixedArrayLength::Literal(3),
                });
            let empty = program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: reference,
                    length: FixedArrayLength::Literal(0),
                });
            let unknown = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: symbols::SymbolHandle::invalid(),
                    name: Identifier::from("unprepared"),
                });
            let carriers = [array, reference, elements, empty, unknown];
            let mut plan = DeclaredStoragePlans::new(&program, carriers);
            assert_eq!(plan.owner_paths.len(), carriers.len());
            let other_generation = TypeReferenceHandle::from_parts(
                array.arena_index(),
                array.generation().wrapping_add(1),
            );
            plan.owner_paths.insert(other_generation, None);
            for carrier in [array, reference, elements, empty, unknown] {
                let original = original_paths(&program, carrier);
                let first = plan.for_carrier(carrier);
                let second = plan.for_carrier(carrier);
                assert_eq!(first.as_deref(), original.as_deref());
                assert_eq!(second.as_deref(), original.as_deref());
                if let (Some(first), Some(second)) = (&first, &second) {
                    assert!(Arc::ptr_eq(first, second));
                }
            }
            let actual = plan.for_carrier(array).unwrap();
            assert_eq!(
                actual.as_ref(),
                &[
                    vec![
                        BorrowLoanOwnerSegment::FixedIndex(0),
                        BorrowLoanOwnerSegment::DynamicIndex
                    ],
                    vec![
                        BorrowLoanOwnerSegment::FixedIndex(1),
                        BorrowLoanOwnerSegment::DynamicIndex
                    ],
                    vec![
                        BorrowLoanOwnerSegment::FixedIndex(2),
                        BorrowLoanOwnerSegment::DynamicIndex
                    ],
                ]
            );
            assert!(plan.for_carrier(empty).unwrap().is_empty());
            assert!(matches!(plan.owner_paths.get(&unknown), Some(None)));
            assert!(matches!(
                plan.owner_paths.get(&other_generation),
                Some(None)
            ));
            let next = DeclaredStoragePlans::new(&program, carriers);
            assert!(!next.owner_paths.contains_key(&other_generation));
            let next_paths = next.for_carrier(array).unwrap();
            assert_eq!(actual, next_paths);
            assert!(!Arc::ptr_eq(&actual, &next_paths));
        }
    }

    fn selected_call(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
    ) -> Option<Handle<checked_trees::FlowCallFact>> {
        let flow = facts.flow.control.state(machine, state)?;
        let mut calls = flow.calls.handles().filter(|handle| {
            let call = facts.flow.control.calls.get(*handle);
            call.statement_index == statement
                && (call.authored_site == checked_trees::FlowCallSite::NamedTransition
                    || (call.authored_site == checked_trees::FlowCallSite::Expression
                        && program.state_by_symbol(state).and_then(|state| program.statement_table.statement_at(state, statement)).is_some_and(|statement| match statement {
                            typed_trees::statement::StatementNode::Transition(transition) => transition.targets().any(|target| matches!(program.statement_table.transition_target(target), typed_trees::statement::TransitionTargetNode::Value(expression) if *expression == call.authored_expression)),
                            typed_trees::statement::StatementNode::Expression(expression) => *expression == call.authored_expression,
                            _ => false,
                        })))
                && arguments_name_call(program, facts, call, target, arguments)
        });
        let call = calls.next()?;
        calls.next().is_none().then_some(call)
    }

    fn arguments_name_call(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        call: &checked_trees::FlowCallFact,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
    ) -> bool {
        let authored = facts
            .flow
            .control
            .call_arguments
            .span(call.authored_arguments);
        if call.authored_target == target && authored == Some(arguments) {
            return true;
        }
        let Some(slot) = program.dynamic_slot_by_symbol(call.authored_target) else {
            return false;
        };
        let Some(entry) = validation::unique_dispatched_entry(program, slot) else {
            return false;
        };
        entry.symbol == target
            && authored.is_some_and(|authored| authored.get(1..) == Some(arguments))
    }

    /*
        Rejoin captured permission to its state-local storage occurrence. The
        checker has already composed stored-field crossings and call-result
        provenance. Replaying descriptor constructors here would lose those edges.
    */
    pub(super) struct RetainedStorage {
        pub(super) backing: Option<u32>,
        pub(super) incoming: Vec<Handle<checked_trees::CheckedCallBorrowLoanResource>>,
        pub(super) parents: Vec<(
            CheckedUnitStructuralArgumentSourcePlan,
            CheckedUnitStructuralArgumentSourcePlan,
        )>,
        pub(super) operand_parents: Vec<CheckedUnitStructuralArgumentSourcePlan>,
    }

    pub(super) fn retained_storage(
        declarations: &DeclaredStoragePlans<'_>,
        records: &RetainedBorrowRecords<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        argument_position: usize,
        operations: &[CheckedUnitEffectOperationPlan],
        address_source: Option<CheckedUnitStructuralArgumentSourcePlan>,
    ) -> Result<RetainedStorage, &'static str> {
        let program = declarations.program;
        let facts = records.facts();
        let call_handle =
            selected_call(program, facts, machine, state, statement, target, arguments)
                .ok_or("state loan: exact captured call")?;
        let call_fact = facts.flow.control.calls.get(call_handle);
        let leading = usize::from(
            program
                .dynamic_slot_by_symbol(call_fact.authored_target)
                .is_some(),
        );
        let operand = call_fact
            .argument_operands()
            .handles()
            .nth(argument_position.saturating_add(leading))
            .ok_or("state loan: captured operand position")?;
        let (parents, mut operand_parents) = retained_parents(
            declarations,
            records,
            machine,
            state,
            statement,
            call_handle,
            operand,
            operations,
            address_source,
        )?;
        let mut roots = records
            .call_resources(call_handle, machine, state)
            .into_iter()
            .filter(|(_, resource)| {
                resource.call == call_handle
                    && resource.machine_symbol == machine
                    && resource.state_symbol == state
                    && resource.owner_operand == operand
                    && resource.owner_path.is_empty()
            });
        let Some((resource_handle, resource)) = roots.next() else {
            let capture = facts.flow.control.operands.get(operand);
            if validation::reference_result_custody::stored_loan_parts(
                program,
                capture.type_reference,
            )
            .is_some()
            {
                if ::validation::trace_flags::state_reference_diagnostics() {
                    eprintln!(
                        "CHECKSITE state reference resource missing: machine={machine:?} state={state:?} call={call_handle:?} operand={operand:?} type={:?} expression={:?} completion={:?} permission={:?}",
                        capture.type_reference,
                        capture.expression,
                        capture.completion,
                        capture.descriptor_permission,
                    );
                    if facts
                        .flow
                        .control
                        .operand_retentions
                        .is_valid(capture.retention)
                    {
                        eprintln!(
                            "CHECKSITE state reference retention: {:?}",
                            facts.flow.control.operand_retentions.get(capture.retention),
                        );
                    }
                    if let Some((_, borrowing)) = facts.borrow.first_state_borrow(machine, state) {
                        for (handle, row) in facts.borrow.state_loans(borrowing).take(8) {
                            eprintln!(
                                "CHECKSITE state reference loan: handle={handle:?} row={row:?}"
                            );
                        }
                    }
                    for (_, row) in records
                        .call_resources(call_handle, machine, state)
                        .into_iter()
                        .take(8)
                    {
                        eprintln!(
                            "CHECKSITE available state reference resource: owner={:?} operand={:?} path={:?} source={:?} incoming={:?}",
                            row.owner_operand,
                            row.operand,
                            facts.borrow.owner_segments.span(row.owner_path),
                            row.source,
                            row.backing_incoming_reference,
                        );
                    }
                }
                return Err("state loan: descriptor resource is absent");
            }
            return Ok(RetainedStorage {
                backing: None,
                incoming: Vec::new(),
                parents,
                operand_parents,
            });
        };
        let resources = std::iter::once((resource_handle, resource))
            .chain(roots)
            .collect::<Vec<_>>();
        let local_backing = |resource: &checked_trees::CheckedCallBorrowLoanResource| {
            if resource.backing_incoming_reference.is_valid() {
                Ok(None)
            } else {
                local_backing_binding(
                    program,
                    facts,
                    state,
                    statement,
                    operations,
                    resource.parent_lifetime.root,
                )
            }
        };
        let backing = local_backing(resource)?;
        let mut common_backing = true;
        for (_, candidate) in &resources {
            common_backing &= local_backing(candidate)? == backing;
        }
        if !common_backing {
            for (_, resource) in &resources {
                if !resource.backing_incoming_reference.is_valid()
                    && let Some(parent) = owned_parent_source(
                        program,
                        facts,
                        state,
                        statement,
                        operations,
                        resource.parent_lifetime.root,
                    )?
                    && !operand_parents.contains(&parent)
                {
                    operand_parents.push(parent);
                }
            }
        }
        let backing = common_backing.then_some(backing).flatten();
        let typed_state = program
            .state_by_symbol(state)
            .ok_or("state loan ingress: state is absent")?;
        let typed_machine = validation::find_state(program, state)
            .map(|(machine, _)| machine)
            .ok_or("state loan ingress: machine is absent")?;
        let incoming = if resources.iter().all(|(_, resource)| {
            if resource.backing_incoming_reference.is_valid() {
                return facts
                    .borrow
                    .incoming_references
                    .is_valid(resource.backing_incoming_reference)
                    && {
                        let origin = facts
                            .borrow
                            .incoming_references
                            .get(resource.backing_incoming_reference);
                        origin.machine_symbol == machine && origin.state_symbol == state
                    };
            }
            resource
                .parent_lifetime
                .root
                .as_symbol()
                .and_then(|symbol| program.state_parameter_by_symbol(typed_state, symbol))
                .is_some_and(|parameter| {
                    validation::reference_shell(program, parameter.type_reference).is_some()
                        && validation::state_reference_parameter_binding_is_stable(
                            program,
                            typed_machine,
                            typed_state,
                            parameter.symbol,
                        )
                })
        }) {
            resources.iter().map(|(handle, _)| *handle).collect()
        } else {
            Vec::new()
        };
        Ok(RetainedStorage {
            backing,
            incoming,
            parents,
            operand_parents,
        })
    }

    fn local_backing_binding(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: SymbolHandle,
        statement: usize,
        operations: &[CheckedUnitEffectOperationPlan],
        root: PlaceRoot,
    ) -> Result<Option<u32>, &'static str> {
        if is_image_storage_root(program, root) {
            return Ok(None);
        }
        let root_type = validation::canonical_place_type_reference(
            program,
            state,
            statement,
            &validation::CanonicalPlace {
                root,
                segments: Vec::new(),
            },
        )
        .ok_or("state loan: backing root type")?;
        if validation::reference_result_custody::stored_loan_parts(program, root_type).is_some() {
            return Ok(None);
        }
        local_storage_binding(program, facts, state, operations, root)
    }

    /*
        Locate owned descriptor storage as well as ordinary owned values. Unlike
        the primary referent query, this never interprets the descriptor as authority
        over the caller/provider storage it denotes.
    */
    fn local_storage_binding(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: SymbolHandle,
        operations: &[CheckedUnitEffectOperationPlan],
        root: PlaceRoot,
    ) -> Result<Option<u32>, &'static str> {
        let typed_state = program
            .state_by_symbol(state)
            .ok_or("state loan: source state")?;
        if let Some(symbol) = root.as_symbol() {
            if program
                .state_parameter_by_symbol(typed_state, symbol)
                .is_some()
            {
                return Ok(None);
            }
            let source =
                super::super::local_storage::source(program, typed_state, operations, symbol);
            return match source {
                Some(source) => source
                    .current_result_binding(operations)
                    .map(Some)
                    .ok_or("state loan: backing occurrence is not a production result"),
                None => Ok(None),
            };
        }
        let mut producers = operations.iter().filter(|operation| {
            super::super::cleanup::normal_edges::producer_root(facts, state, operation)
                == Some(root)
        });
        let Some(producer) = producers.next() else {
            return Ok(None);
        };
        if producers.next().is_some() {
            return Err("state loan: duplicate backing producers");
        }
        Ok(Some(
            producer
                .structural_result_binding()
                .ok_or("state loan: backing producer has no structural result")?
                .binding_ordinal,
        ))
    }

    /*
        The selected call's complete stored-loan frontier already names these
        dependencies, including replaced/call-produced references. Retain the
        current checked roots, never replay construction syntax or stale initializers.
        A dependency is storage lifetime only: cleanup still needs its own action.
    */
    fn retained_parents(
        declarations: &DeclaredStoragePlans<'_>,
        records: &RetainedBorrowRecords<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        call_handle: Handle<checked_trees::FlowCallFact>,
        operand: Handle<checked_trees::FlowOperandFact>,
        operations: &[CheckedUnitEffectOperationPlan],
        address_source: Option<CheckedUnitStructuralArgumentSourcePlan>,
    ) -> Result<
        (
            Vec<(
                CheckedUnitStructuralArgumentSourcePlan,
                CheckedUnitStructuralArgumentSourcePlan,
            )>,
            Vec<CheckedUnitStructuralArgumentSourcePlan>,
        ),
        &'static str,
    > {
        let program = declarations.program;
        let facts = records.facts();
        let call = facts.flow.control.calls.get(call_handle);
        let capture = facts.flow.control.operands.get(operand);
        let ingress = records
            .incoming_references(machine, state)
            .into_iter()
            .filter_map(|(_, incoming)| {
                if incoming.machine_symbol != machine || incoming.state_symbol != state {
                    return None;
                }
                let checked_trees::IncomingReferenceOrigin::Parameter { symbol } = incoming.origin
                else {
                    return None;
                };
                Some((
                    PlaceRoot::Symbol(symbol),
                    facts
                        .borrow
                        .owner_segments
                        .span_or_empty(incoming.owner_path),
                ))
            })
            .collect::<Vec<_>>();
        let typed_state = program
            .state_by_symbol(state)
            .ok_or("state loan parents: source state")?;
        let ingress_storage = program
            .state_parameters(typed_state)
            .iter()
            .filter_map(|parameter| {
                validation::reference_result_custody::input_carrier_type(
                    program,
                    parameter.type_reference,
                )
                .map(|carrier| IngressStorageCarrier {
                    root: PlaceRoot::Symbol(parameter.symbol),
                    carrier,
                    declarations,
                })
            })
            .collect::<Vec<_>>();
        let paths = validation::borrow_contracts::borrow_carrying_owner_paths(
            program,
            capture.type_reference,
        )
        .ok_or("state loan parents: unresolved operand frontier")?;
        let roots = records
            .call_resources(call_handle, machine, state)
            .into_iter()
            .filter(|(_, resource)| {
                resource.call == call_handle
                    && resource.machine_symbol == machine
                    && resource.state_symbol == state
                    && resource.owner_operand == operand
            })
            .map(|(_, resource)| resource)
            .collect::<Vec<_>>();
        let mut shared_backings = Vec::new();
        for path in &paths {
            if roots.iter().any(|resource| {
                facts
                    .borrow
                    .owner_segments
                    .span_or_empty(resource.owner_path)
                    == path.as_slice()
            }) || ingress_covers_capture_path(
                program,
                facts,
                operand,
                &crate::borrow::owner_place_segments(path),
                &ingress,
            ) {
                continue;
            }
            let retained = shared_leaf_lifetime_roots(program, facts, capture, path)
                .ok_or("state loan parents: captured reference leaves have no resources")?;
            for root in retained {
                push_unique(&mut shared_backings, root);
            }
        }
        if roots.is_empty() && shared_backings.is_empty() && address_source.is_none() {
            return Ok((Vec::new(), Vec::new()));
        }
        let borrow_state = facts
            .borrow
            .unique_state_borrow(machine, state)
            .ok_or("state loan parents: source borrow state")?;
        let mut captured = Vec::new();
        let mut operand_parents = Vec::new();
        if let Some(CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal }) =
            address_source
        {
            let operation = operations
                .iter()
                .find(|operation| {
                    operation
                        .structural_result_binding()
                        .is_some_and(|binding| binding.binding_ordinal == binding_ordinal)
                })
                .ok_or("state loan parents: address source production")?;
            let root = super::super::cleanup::normal_edges::producer_root(facts, state, operation)
                .ok_or("state loan parents: address source storage")?;
            let dependencies = Dependencies {
                program,
                facts,
                state,
                statement: capture.statement_index,
                borrow_state,
                question: StorageQuestion::RetainForArrival,
                forbidden: &[],
                ingress: &ingress,
                ingress_storage: &ingress_storage,
            };
            if !dependencies.contents(root, call.entry_constraints, &mut Vec::new(), &mut captured)
            {
                return Err("state loan parents: incomplete address storage dependencies");
            }
        }
        for resource in roots {
            if !facts.flow.control.operands.is_valid(resource.operand) {
                return Err("state loan parents: stale captured leaf");
            }
            let leaf = facts.flow.control.operands.get(resource.operand);
            if leaf.state_symbol != state {
                return Err("state loan parents: captured leaf belongs to another state");
            }
            let dependencies = Dependencies {
                program,
                facts,
                state,
                statement: leaf.statement_index,
                borrow_state,
                question: StorageQuestion::RetainForArrival,
                forbidden: &[],
                ingress: &ingress,
                ingress_storage: &ingress_storage,
            };
            let root = resource.parent_lifetime.root;
            let incoming_referent = resource.backing_incoming_reference.is_valid()
                || match resource.source {
                    LoanSource::StoredLoan { loan } => {
                        if !facts.borrow.loans.is_valid(loan) {
                            return Err("state loan parents: stale stored resource");
                        }
                        facts.borrow.loans.get(loan).incoming_reference.is_valid()
                    }
                    LoanSource::CallAccess { .. } => false,
                };
            if !resource.owner_path.is_empty()
                && !incoming_referent
                && let Some(parent) =
                    owned_parent_source(program, facts, state, statement, operations, root)?
                && !operand_parents.contains(&parent)
            {
                operand_parents.push(parent);
            }
            if !dependencies.captured_storage(resource, leaf.constraints, &mut captured) {
                return Err("state loan parents: incomplete captured storage dependencies");
            }
            let loans = match resource.source {
                LoanSource::StoredLoan { loan } => vec![loan],
                LoanSource::CallAccess { parents, .. } => facts
                    .borrow
                    .call_loan_parents
                    .span(parents)
                    .ok_or("state loan parents: stale captured parent roster")?
                    .to_vec(),
            };
            for loan in loans {
                if !dependencies.loan(
                    loan,
                    leaf.constraints,
                    &mut Vec::new(),
                    &mut Vec::new(),
                    &mut captured,
                ) {
                    return Err("state loan parents: incomplete captured loan lineage");
                }
            }
        }
        let dependencies = Dependencies {
            program,
            facts,
            state,
            statement: capture.statement_index,
            borrow_state,
            question: StorageQuestion::RetainForArrival,
            forbidden: &[],
            ingress: &ingress,
            ingress_storage: &ingress_storage,
        };
        for root in shared_backings {
            if let Some(parent) =
                owned_parent_source(program, facts, state, statement, operations, root)?
                && !operand_parents.contains(&parent)
            {
                operand_parents.push(parent);
            }
            if !dependencies.contents(root, capture.constraints, &mut Vec::new(), &mut captured) {
                return Err("state loan parents: incomplete shared storage dependencies");
            }
        }
        let mut parents = Vec::new();
        for (dependent, parent) in captured {
            let Some(parent) =
                owned_parent_source(program, facts, state, statement, operations, parent)?
            else {
                continue;
            };
            let dependent =
                owned_parent_source(program, facts, state, statement, operations, dependent)?;
            let Some(dependent) = dependent else {
                continue;
            };
            let relation = (dependent, parent);
            push_unique(&mut parents, relation);
        }
        Ok((parents, operand_parents))
    }

    /*
        A complete shared-leaf lifetime profile can retain storage after its forming
        access ends. Read every exact completed profile; neither shared type spelling
        nor a missing resource establishes external or image backing. Exclusive
        leaves still require their captured permission resource or ingress relation.
    */
    fn shared_leaf_lifetime_roots(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        capture: &checked_trees::FlowOperandFact,
        path: &[checked_trees::BorrowLoanOwnerSegment],
    ) -> Option<Vec<PlaceRoot>> {
        let path = crate::borrow::owner_place_segments(path);
        let reference = validation::project_type_reference_from_segments(
            program,
            capture.type_reference,
            &path,
        )?;
        if validation::reference_shell(program, reference)?.access
            != language_semantics::ReferenceAccess::Shared
        {
            return None;
        }
        let profiles = validation::completed_storage::captured_retention_profiles_at_path(
            program,
            &facts.flow.control,
            capture,
            &path,
        )?;
        if profiles.is_empty() {
            return None;
        }
        let mut roots = Vec::new();
        for profile in profiles {
            for root in facts.flow.control.retained_roots(profile)? {
                if !roots.contains(root) {
                    roots.push(*root);
                }
            }
        }
        Some(roots)
    }

    /*
        Reuse the completed assembly's child projection. An opaque child keeps its
        remaining path in its own parameter namespace; alternatives must each own
        that same permission. No initializer or source constructor is replayed.
    */
    fn ingress_covers_capture_path(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        operand: Handle<checked_trees::FlowOperandFact>,
        path: &[facts::PlaceSegment],
        ingress: &[(PlaceRoot, &[checked_trees::BorrowLoanOwnerSegment])],
    ) -> bool {
        if !facts.flow.control.operands.is_valid(operand) {
            return false;
        }
        ingress_covers_capture_path_inner(
            program,
            facts,
            operand,
            path,
            ingress,
            facts.flow.control.operands.get(operand).storage_snapshot,
            &mut Vec::new(),
        )
    }

    fn ingress_covers_capture_path_inner(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        operand: Handle<checked_trees::FlowOperandFact>,
        path: &[facts::PlaceSegment],
        ingress: &[(PlaceRoot, &[checked_trees::BorrowLoanOwnerSegment])],
        current: Handle<checked_trees::FlowStorageSnapshot>,
        active: &mut Vec<(
            Handle<checked_trees::FlowOperandFact>,
            Vec<facts::PlaceSegment>,
        )>,
    ) -> bool {
        let key = (operand, path.to_vec());
        if !facts.flow.control.operands.is_valid(operand) || active.contains(&key) {
            return false;
        }
        active.push(key);
        let covered = ingress_covers_capture_path_body(
            program, facts, operand, path, ingress, current, active,
        );
        active.pop();
        covered
    }

    fn ingress_covers_capture_path_body(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        operand: Handle<checked_trees::FlowOperandFact>,
        path: &[facts::PlaceSegment],
        ingress: &[(PlaceRoot, &[checked_trees::BorrowLoanOwnerSegment])],
        current: Handle<checked_trees::FlowStorageSnapshot>,
        active: &mut Vec<(
            Handle<checked_trees::FlowOperandFact>,
            Vec<facts::PlaceSegment>,
        )>,
    ) -> bool {
        let control = &facts.flow.control;
        let capture = control.operands.get(operand);
        if literal_value_path_is_inactive(program, capture.expression, capture.type_reference, path)
        {
            return true;
        }
        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion {
            let Some(rows) = control
                .operand_alternatives
                .span(alternatives)
                .filter(|rows| !rows.is_empty())
            else {
                return false;
            };
            return rows.iter().all(|row| {
                row.source_arm.is_valid()
                    && ingress_covers_capture_path_inner(
                        program, facts, row.value, path, ingress, current, active,
                    )
            });
        }
        if !path.is_empty()
            && matches!(
                capture.completion,
                checked_trees::FlowOperandCompletion::Constructed(_)
            )
        {
            let Some(children) = validation::completed_storage::captured_operand_children_at_path(
                program, control, operand, path,
            ) else {
                return false;
            };
            return children.iter().all(|child| {
                ingress_covers_capture_path_inner(
                    program,
                    facts,
                    child.capture,
                    &child.projection,
                    ingress,
                    current,
                    active,
                )
            });
        }
        if let Some(profiles) = validation::completed_storage::captured_retention_profiles_at_path(
            program, control, capture, path,
        ) {
            let suppliers = profiles
                .iter()
                .map(|profile| {
                    validation::completed_storage::descriptor_suppliers(
                        control,
                        *profile,
                        &mut Vec::new(),
                    )
                })
                .collect::<Vec<_>>();
            if suppliers.iter().any(Option::is_some) {
                return !suppliers.is_empty()
                    && suppliers.into_iter().all(|suppliers| {
                        suppliers.is_some_and(|suppliers| {
                            !suppliers.is_empty()
                                && suppliers.iter().all(|supplier| {
                                    ingress_covers_capture_path_inner(
                                        program,
                                        facts,
                                        supplier.operand,
                                        control
                                            .operand_retained_paths
                                            .span_or_empty(supplier.projection),
                                        ingress,
                                        current,
                                        active,
                                    )
                                })
                        })
                    });
            }
        }
        if let Some(stored) = validation::completed_storage::reference_captures_at_path(
            program, control, operand, path, current,
        ) {
            let Some(captures) = stored.captures() else {
                return false;
            };
            return !captures.is_empty()
                && captures.iter().all(|selected| {
                    if selected.capture == operand && selected.projection == path {
                        return unchanged_ingress_capture(program, control, capture, path, ingress);
                    }
                    ingress_covers_capture_path_inner(
                        program,
                        facts,
                        selected.capture,
                        &selected.projection,
                        ingress,
                        current,
                        active,
                    )
                });
        }
        unchanged_ingress_capture(program, control, capture, path, ingress)
    }

    fn unchanged_ingress_capture(
        program: &dyn validation::CheckingInput,
        control: &checked_trees::FlowControlFacts,
        capture: &checked_trees::FlowOperandFact,
        path: &[facts::PlaceSegment],
        ingress: &[(PlaceRoot, &[checked_trees::BorrowLoanOwnerSegment])],
    ) -> bool {
        let Some(mut source) = place_at(
            program,
            capture.state_symbol,
            capture.statement_index,
            capture.expression,
        ) else {
            return false;
        };
        source.segments.extend_from_slice(path);
        if validation::completed_storage::at_snapshot(
            program,
            control,
            capture.storage_snapshot,
            &source,
            validation::completed_storage::StoredObservation::Binding,
        )
        .is_some()
        {
            return false;
        }
        let selected = crate::borrow::owner_path_from_place_segments(program, &source.segments);
        ingress
            .iter()
            .any(|(root, path)| *root == source.root && *path == selected)
    }

    fn owned_parent_source(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: SymbolHandle,
        statement: usize,
        operations: &[CheckedUnitEffectOperationPlan],
        root: PlaceRoot,
    ) -> Result<Option<CheckedUnitStructuralArgumentSourcePlan>, &'static str> {
        if is_image_storage_root(program, root) {
            return Ok(None);
        }
        let reference = validation::canonical_place_type_reference(
            program,
            state,
            statement,
            &validation::CanonicalPlace {
                root,
                segments: Vec::new(),
            },
        )
        .ok_or("state loan parents: parent type")?;
        if validation::reference_result_custody::result_multiplicity(program, reference)
            == language_semantics::Multiplicity::Unrestricted
        {
            return Ok(None);
        }
        let typed_state = program
            .state_by_symbol(state)
            .ok_or("state loan parents: source state")?;
        if let Some(symbol) = root.as_symbol()
            && let Some(parameter) = program.state_parameter_by_symbol(typed_state, symbol)
        {
            if validation::reference_shell(program, parameter.type_reference).is_some() {
                return Ok(None);
            }
            let parameters = program.state_parameters(typed_state);
            let authored = parameters
                .iter()
                .position(|parameter| parameter.symbol == symbol)
                .ok_or("state loan parents: owned input position")?;
            let parameter_index = super::super::types::dense_structural_parameter_index(
                program, parameters, authored,
            )
            .ok_or("state loan parents: owned input namespace")?;
            return Ok(Some(CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            }));
        }
        Ok(
            local_storage_binding(program, facts, state, operations, root)?.map(
                |binding_ordinal| CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                },
            ),
        )
    }

    pub(super) fn retired_carrier_backing(
        program: &dyn validation::CheckingInput,
        records: &RetainedBorrowRecords<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        argument_position: usize,
        operations: &[CheckedUnitEffectOperationPlan],
        subject: &CheckedUnitStructuralArgumentPlan,
    ) -> Option<u32> {
        let facts = records.facts();
        if subject.path.as_slice() != [CheckedUnitStructuralPathSegment::Referent] {
            return None;
        }
        let carrier = subject.source.current_result_binding(operations)?;
        let (formation, producer) = operations.iter().enumerate().find(|(_, operation)| {
            operation
                .structural_result_binding()
                .is_some_and(|result| result.binding_ordinal == carrier)
        })?;
        let lent = producer.established_reference_source(&facts.values.structural_values)?;
        if !lent.path.is_empty() {
            return None;
        }
        let backing = lent
            .source
            .current_result_binding(&operations[..formation])?;
        let backing_producer = operations[..formation].iter().find(|operation| {
            operation
                .structural_result_binding()
                .is_some_and(|result| result.binding_ordinal == backing)
        })?;
        let backing_root =
            super::super::cleanup::normal_edges::producer_root(facts, state, backing_producer)?;
        let carrier_root =
            super::super::cleanup::normal_edges::producer_root(facts, state, producer)?;
        let backing_type = validation::canonical_place_type_reference(
            program,
            state,
            statement,
            &validation::CanonicalPlace {
                root: backing_root,
                segments: Vec::new(),
            },
        )?;
        if validation::reference_shell(program, backing_type).is_some() {
            return None;
        }
        let released = operations
            .iter()
            .enumerate()
            .find_map(|(position, operation)| match operation {
                CheckedUnitEffectOperationPlan::ReleaseReference {
                    source,
                    path,
                    case_conditions,
                    loans,
                    select_evaluated_storage: false,
                    defer_to_exit: true,
                    ..
                } if path.is_empty()
                    && case_conditions.is_empty()
                    && source.current_result_binding(&operations[..position]) == Some(carrier) =>
                {
                    Some(*loans)
                }
                _ => None,
            })?;

        let call_handle =
            selected_call(program, facts, machine, state, statement, target, arguments)?;
        let call = facts.flow.control.calls.get(call_handle);
        let selected_operand = call.argument_operands().handles().nth(argument_position)?;
        let resources = records
            .call_resources(call_handle, machine, state)
            .into_iter()
            .filter(|(_, resource)| {
                resource.call == call_handle
                    && resource.machine_symbol == machine
                    && resource.state_symbol == state
            })
            .map(|(_, resource)| resource)
            .collect::<Vec<_>>();
        let borrow_state = facts.borrow.unique_state_borrow(machine, state)?;
        let dependencies = Dependencies {
            program,
            facts,
            state,
            statement,
            borrow_state,
            question: StorageQuestion::ProveRelocationIndependence,
            forbidden: &[backing_root, carrier_root],
            ingress: &[],
            ingress_storage: &[],
        };
        let mut selected = false;
        for operand in call.argument_operands().handles() {
            let capture = facts.flow.control.operands.get(operand);
            let paths = validation::borrow_contracts::borrow_carrying_owner_paths(
                program,
                capture.type_reference,
            )?;
            let captured = resources
                .iter()
                .copied()
                .filter(|resource| resource.owner_operand == operand)
                .collect::<Vec<_>>();
            if !paths.iter().all(|path| {
                captured.iter().any(|resource| {
                    facts.borrow.owner_segments.span(resource.owner_path) == Some(path.as_slice())
                })
            }) || captured.iter().any(|resource| {
                !paths.iter().any(|path| {
                    facts.borrow.owner_segments.span(resource.owner_path) == Some(path.as_slice())
                })
            }) {
                return None;
            }
            for resource in captured {
                if operand == selected_operand {
                    if !resource.owner_path.is_empty()
                        || resource.parent_lifetime.root != backing_root
                        || resource.backing_incoming_reference.is_valid()
                        || !resource.backing_path.is_empty()
                    {
                        return None;
                    }
                    let LoanSource::CallAccess { access, parents } = resource.source else {
                        return None;
                    };
                    if facts.borrow.argument_accesses.get(access).expression != capture.expression {
                        return None;
                    }
                    let parents = facts.borrow.call_loan_parents.span(parents)?;
                    let released = facts.borrow.loan_group(released)?.collect::<Vec<_>>();
                    if released.is_empty()
                        || !released.iter().all(|(loan, _)| parents.contains(loan))
                        || parents.iter().any(|parent| {
                            !released.iter().any(|(loan, _)| loan == parent)
                                && !dependencies.loan(
                                    *parent,
                                    capture.constraints,
                                    &mut Vec::new(),
                                    &mut Vec::new(),
                                    &mut Vec::new(),
                                )
                        })
                    {
                        return None;
                    }
                    selected = true;
                    continue;
                }
                if !facts.flow.control.operands.is_valid(resource.operand) {
                    return None;
                }
                let leaf = facts.flow.control.operands.get(resource.operand);
                if leaf.state_symbol != state {
                    return None;
                }
                let captured_dependencies = Dependencies {
                    statement: leaf.statement_index,
                    ..dependencies
                };
                if !captured_dependencies.captured_storage(
                    resource,
                    leaf.constraints,
                    &mut Vec::new(),
                ) {
                    return None;
                }
                let loans = match resource.source {
                    LoanSource::StoredLoan { loan } => vec![loan],
                    LoanSource::CallAccess { parents, .. } => {
                        facts.borrow.call_loan_parents.span(parents)?.to_vec()
                    }
                };
                for loan in loans {
                    if !captured_dependencies.loan(
                        loan,
                        leaf.constraints,
                        &mut Vec::new(),
                        &mut Vec::new(),
                        &mut Vec::new(),
                    ) {
                        return None;
                    }
                }
            }
        }
        if !selected
            || !dependencies.contents(
                backing_root,
                call.entry_constraints,
                &mut Vec::new(),
                &mut Vec::new(),
            )
        {
            return None;
        }
        Some(backing)
    }

    /*
        Continuing a lifetime and proving freedom to relocate storage have
        different conclusions. The same captured graph answers both questions;
        external output provenance can justify only the former.
    */
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum StorageQuestion {
        RetainForArrival,
        ProveRelocationIndependence,
    }

    struct Dependencies<'a> {
        program: &'a dyn validation::CheckingInput,
        facts: &'a CheckFacts,
        state: SymbolHandle,
        statement: usize,
        borrow_state: &'a checked_trees::StateBorrowFact,
        question: StorageQuestion,
        forbidden: &'a [PlaceRoot],
        /*
            Exact caller-supplied leaf contracts are sufficient for continued
            lifetime, not for proving relocation independence. That judgment passes
            no ingress assumptions and retains its existing fail-closed behavior.
        */
        ingress: &'a [(PlaceRoot, &'a [checked_trees::BorrowLoanOwnerSegment])],
        ingress_storage: &'a [IngressStorageCarrier<'a>],
    }

    impl Dependencies<'_> {
        fn captured_storage(
            &self,
            resource: &checked_trees::CheckedCallBorrowLoanResource,
            constraints: HandleSpan<FlowConstraintRef>,
            captured: &mut Vec<(PlaceRoot, PlaceRoot)>,
        ) -> bool {
            let root = resource.parent_lifetime.root;
            if resource.parent_lifetime.machine_symbol != self.borrow_state.machine_symbol
                || resource.parent_lifetime.state_symbol != self.state
                || self.forbidden.contains(&root)
            {
                return false;
            }
            let Some(path) = self
                .facts
                .borrow
                .call_loan_backing_paths
                .span(resource.backing_path)
            else {
                return false;
            };
            if resource.backing_incoming_reference.is_valid() {
                if !self
                    .facts
                    .borrow
                    .incoming_references
                    .is_valid(resource.backing_incoming_reference)
                {
                    return false;
                }
                let incoming = self
                    .facts
                    .borrow
                    .incoming_references
                    .get(resource.backing_incoming_reference);
                if matches!(
                    incoming.origin,
                    checked_trees::IncomingReferenceOrigin::Output { .. }
                        | checked_trees::IncomingReferenceOrigin::DescriptorLoad { .. }
                        | checked_trees::IncomingReferenceOrigin::Invocation { .. }
                ) {
                    if self.question != StorageQuestion::RetainForArrival
                        || incoming.machine_symbol != self.borrow_state.machine_symbol
                        || incoming.state_symbol != self.state
                        || incoming.place_root() != root
                    {
                        return false;
                    }
                    let sources = match resource.source {
                        LoanSource::StoredLoan { loan } => vec![loan],
                        LoanSource::CallAccess { parents, .. } => {
                            let Some(parents) = self.facts.borrow.call_loan_parents.span(parents)
                            else {
                                return false;
                            };
                            parents.to_vec()
                        }
                    };
                    return sources.iter().any(|handle| {
                        self.facts.borrow.loans.is_valid(*handle) && {
                            let loan = self.facts.borrow.loans.get(*handle);
                            loan.incoming_reference == resource.backing_incoming_reference
                                && self
                                    .facts
                                    .borrow
                                    .access_segments
                                    .span_or_empty(loan.incoming_owner_path)
                                    == self
                                        .facts
                                        .borrow
                                        .call_loan_backing_paths
                                        .span_or_empty(resource.backing_incoming_owner_path)
                                && loan.root == root
                        }
                    }) && sources.iter().all(|handle| {
                        self.loan(
                            *handle,
                            constraints,
                            &mut Vec::new(),
                            &mut Vec::new(),
                            captured,
                        )
                    });
                }
                return incoming.machine_symbol == self.borrow_state.machine_symbol
                    && incoming.state_symbol == self.state
                    && incoming.place_root() == root
                    && self.ingress.iter().any(|(owner, owner_path)| {
                        *owner == root
                            && *owner_path
                                == self
                                    .facts
                                    .borrow
                                    .owner_segments
                                    .span_or_empty(incoming.owner_path)
                    });
            }
            self.contents_at(root, path, constraints, &mut Vec::new(), captured)
        }

        fn selected_path(
            &self,
            owner: &[checked_trees::BorrowLoanOwnerSegment],
            selected: &[facts::PlaceSegment],
        ) -> bool {
            owner.len() >= selected.len()
                && owner.iter().zip(selected).all(|(owner, selected)| {
                    crate::borrow::owner_segment_matches_place_segment(
                        self.program,
                        owner,
                        selected,
                    )
                    .unwrap_or_else(|| match (owner, selected) {
                        (
                            checked_trees::BorrowLoanOwnerSegment::FixedIndex(index),
                            facts::PlaceSegment::FixedRange { start, end },
                        ) => start <= index && index < end,
                        (
                            checked_trees::BorrowLoanOwnerSegment::FixedIndex(_),
                            facts::PlaceSegment::OpaqueRange,
                        )
                        | (
                            checked_trees::BorrowLoanOwnerSegment::DynamicIndex,
                            facts::PlaceSegment::FixedIndex { .. }
                            | facts::PlaceSegment::Index { .. }
                            | facts::PlaceSegment::FixedRange { .. }
                            | facts::PlaceSegment::OpaqueRange,
                        ) => true,
                        _ => false,
                    })
                })
        }

        fn root(
            &self,
            root: PlaceRoot,
            constraints: HandleSpan<FlowConstraintRef>,
            active: &mut Vec<PlaceRoot>,
            captured: &mut Vec<(PlaceRoot, PlaceRoot)>,
        ) -> bool {
            if self.forbidden.contains(&root) {
                return false;
            }
            if let Some(dependent) = active.last().copied()
                && !captured.contains(&(dependent, root))
            {
                captured.push((dependent, root));
            }
            self.contents(root, constraints, active, captured)
        }

        fn contents(
            &self,
            root: PlaceRoot,
            constraints: HandleSpan<FlowConstraintRef>,
            active: &mut Vec<PlaceRoot>,
            captured: &mut Vec<(PlaceRoot, PlaceRoot)>,
        ) -> bool {
            self.contents_at(root, &[], constraints, active, captured)
        }

        fn contents_at(
            &self,
            root: PlaceRoot,
            selected: &[facts::PlaceSegment],
            constraints: HandleSpan<FlowConstraintRef>,
            active: &mut Vec<PlaceRoot>,
            captured: &mut Vec<(PlaceRoot, PlaceRoot)>,
        ) -> bool {
            if active.contains(&root) {
                return false;
            }
            if is_image_storage_root(self.program, root) {
                return true;
            }
            let Some(mut reference) = validation::canonical_place_type_reference(
                self.program,
                self.state,
                self.statement,
                &validation::CanonicalPlace {
                    root,
                    segments: Vec::new(),
                },
            ) else {
                return false;
            };
            let mut opaque_referent = false;
            if root.as_symbol().is_some_and(|symbol| {
                self.program
                    .state_by_symbol(self.state)
                    .is_some_and(|state| {
                        self.program
                            .state_parameter_by_symbol(state, symbol)
                            .is_some()
                    })
            }) {
                match validation::reference_result_custody::input_carrier_type(
                    self.program,
                    reference,
                ) {
                    Some(carrier) => reference = carrier,
                    None if validation::reference_shell(self.program, reference).is_some() => {
                        opaque_referent = true;
                    }
                    None => return false,
                }
            }
            let declared;
            let paths: std::borrow::Cow<'_, [Vec<checked_trees::BorrowLoanOwnerSegment>]> =
                if opaque_referent {
                    std::borrow::Cow::Borrowed(&[])
                } else if let Some(ingress) = self
                    .ingress_storage
                    .iter()
                    .find(|ingress| ingress.root == root)
                {
                    declared = ingress.declarations.for_carrier(ingress.carrier);
                    let Some(paths) = declared.as_ref() else {
                        return false;
                    };
                    std::borrow::Cow::Borrowed(paths.as_ref())
                } else {
                    let Some(paths) = validation::borrow_contracts::borrow_carrying_owner_paths(
                        self.program,
                        reference,
                    ) else {
                        return false;
                    };
                    std::borrow::Cow::Owned(paths)
                };
            let paths = paths
                .iter()
                .filter(|path| self.selected_path(path, selected))
                .collect::<Vec<_>>();
            let loans = self
                .facts
                .flow
                .borrow_loan_constraints(constraints)
                .filter(|handle| {
                    let loan = self.facts.borrow.loans.get(*handle);
                    loan.storage_owner == root
                        && self.selected_path(self.facts.borrow.loan_owner_path(loan), selected)
                })
                .collect::<Vec<_>>();
            if !paths.iter().all(|path| {
                loans.iter().any(|handle| {
                    self.facts
                        .borrow
                        .loan_owner_path(self.facts.borrow.loans.get(*handle))
                        == path.as_slice()
                }) || self
                    .ingress
                    .iter()
                    .any(|(owner, incoming)| *owner == root && *incoming == path.as_slice())
            }) || loans.iter().any(|handle| {
                !paths.iter().any(|path| {
                    self.facts
                        .borrow
                        .loan_owner_path(self.facts.borrow.loans.get(*handle))
                        == path.as_slice()
                })
            }) {
                return false;
            }
            active.push(root);
            let independent = loans
                .into_iter()
                .all(|loan| self.loan(loan, constraints, active, &mut Vec::new(), captured));
            active.pop();
            independent
        }

        /*
            A performed return supplies a pointer independently of its input
            addresses. Retain every input debt for this exact returned field, using
            the recorded supplier graph so later writes cannot retarget its capture.
        */
        fn invocation_lifetimes(
            &self,
            handle: Handle<BorrowLoanFact>,
            constraints: HandleSpan<FlowConstraintRef>,
            active_roots: &mut Vec<PlaceRoot>,
            active_loans: &mut Vec<Handle<BorrowLoanFact>>,
            captured: &mut Vec<(PlaceRoot, PlaceRoot)>,
        ) -> bool {
            let borrow = &self.facts.borrow;
            let loan = borrow.loans.get(handle);
            if !borrow.is_call_result_loan(loan)
                || loan.formation != checked_trees::BorrowLoanFormation::ReturnedDescriptor
            {
                return false;
            }
            let call = borrow.calls.get(loan.established_by_call);
            let incoming = borrow.incoming_references.get(loan.incoming_reference);
            let checked_trees::IncomingReferenceOrigin::Invocation { expression, target } =
                incoming.origin
            else {
                return false;
            };
            let Some(path) = borrow.owner_segments.span(loan.owner_path) else {
                return false;
            };
            if borrow.call_at(
                incoming.machine_symbol,
                incoming.state_symbol,
                call.statement_index,
                call.call_ordinal,
            ) != Some(loan.established_by_call)
                || expression != call.authored_expression
                || validation::call_target_entry_symbol(self.program, target)
                    != validation::call_target_entry_symbol(self.program, call.target_symbol)
                || loan
                    .formation_owner_path
                    .and_then(|path| borrow.owner_segments.span(path))
                    != Some(path)
                || borrow.owner_segments.span(incoming.owner_path) != Some(path)
                || !borrow.loans_contain(call.completed_reference_loans, handle)
            {
                return false;
            }
            let Some(debts) = loan
                .retained_input_lifetimes
                .and_then(|debts| borrow.loan_suppliers.span(debts))
            else {
                return false;
            };
            debts.iter().all(|source| {
                if !borrow.loans_contain(call.completed_reference_loans, *source) {
                    return false;
                }
                let row = borrow.loans.get(*source);
                row.established_by_call == loan.established_by_call
                    && row.storage_owner == loan.storage_owner
                    && borrow.owner_segments.span(row.owner_path) == Some(path)
                    && row.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency
                    && self.loan(*source, constraints, active_roots, active_loans, captured)
            })
        }

        fn loan(
            &self,
            handle: Handle<BorrowLoanFact>,
            constraints: HandleSpan<FlowConstraintRef>,
            active_roots: &mut Vec<PlaceRoot>,
            active_loans: &mut Vec<Handle<BorrowLoanFact>>,
            captured: &mut Vec<(PlaceRoot, PlaceRoot)>,
        ) -> bool {
            if !self.facts.borrow.state_owns_loan(self.borrow_state, handle)
                || active_loans.contains(&handle)
            {
                return false;
            }
            let loan = self.facts.borrow.loans.get(handle);
            if self.forbidden.contains(&loan.root) {
                return false;
            }
            if self
                .facts
                .borrow
                .incoming_references
                .is_valid(loan.incoming_reference)
                && let identity = self
                    .facts
                    .borrow
                    .incoming_references
                    .get(loan.incoming_reference)
                && let checked_trees::IncomingReferenceOrigin::DescriptorLoad { expression } =
                    identity.origin
            {
                if self.question != StorageQuestion::RetainForArrival
                    || identity.machine_symbol != self.borrow_state.machine_symbol
                    || identity.state_symbol != self.state
                    || loan.root != PlaceRoot::Expression(expression)
                    || active_loans.contains(&handle)
                {
                    return false;
                }
                let Some(state) = self
                    .facts
                    .flow
                    .control
                    .state(identity.machine_symbol, identity.state_symbol)
                else {
                    return false;
                };
                let mut completions = self
                    .facts
                    .flow
                    .borrow_lifetimes
                    .activations
                    .span_or_empty(state.borrow_activations)
                    .iter()
                    .filter(|activation| {
                        self.facts
                            .borrow
                            .loans
                            .get(activation.loan)
                            .incoming_reference
                            == loan.incoming_reference
                            && self
                                .facts
                                .flow
                                .control
                                .operands
                                .is_valid(activation.formation_value)
                            && self
                                .facts
                                .flow
                                .control
                                .operands
                                .get(activation.formation_value)
                                .descriptor_permission
                                .loaded_loan()
                                == activation.loan
                    });
                let Some(activation) = completions.next() else {
                    return false;
                };
                if completions.next().is_some()
                    || !self
                        .facts
                        .flow
                        .control
                        .operands
                        .is_valid(activation.formation_value)
                {
                    return false;
                }
                let capture = self
                    .facts
                    .flow
                    .control
                    .operands
                    .get(activation.formation_value);
                if capture.expression != expression
                    || capture.descriptor_permission.loaded_loan() != activation.loan
                {
                    return false;
                }
                let Some(roots) =
                    shared_leaf_lifetime_roots(self.program, self.facts, capture, &[])
                else {
                    return false;
                };
                active_loans.push(handle);
                let retained = roots
                    .into_iter()
                    .all(|root| self.root(root, constraints, active_roots, captured));
                active_loans.pop();
                return retained;
            }
            let incoming = self
                .facts
                .borrow
                .incoming_references
                .is_valid(loan.incoming_reference)
                && {
                    let identity = self
                        .facts
                        .borrow
                        .incoming_references
                        .get(loan.incoming_reference);
                    identity.machine_symbol == self.borrow_state.machine_symbol
                        && identity.state_symbol == self.state
                        && matches!(identity.origin,
                            checked_trees::IncomingReferenceOrigin::Parameter { symbol }
                                if loan.root == PlaceRoot::Symbol(symbol))
                };
            let output = self.question == StorageQuestion::RetainForArrival
                && self
                    .facts
                    .borrow
                    .incoming_references
                    .is_valid(loan.incoming_reference)
                && {
                    let identity = self
                        .facts
                        .borrow
                        .incoming_references
                        .get(loan.incoming_reference);
                    identity.machine_symbol == self.borrow_state.machine_symbol
                        && identity.state_symbol == self.state
                        && matches!(identity.origin,
                            checked_trees::IncomingReferenceOrigin::Output { identity }
                                if loan.root == PlaceRoot::InvocationOutput(identity))
                };
            let invocation = self.question == StorageQuestion::RetainForArrival
                && self
                    .facts
                    .borrow
                    .incoming_references
                    .is_valid(loan.incoming_reference)
                && {
                    let identity = self
                        .facts
                        .borrow
                        .incoming_references
                        .get(loan.incoming_reference);
                    identity.machine_symbol == self.borrow_state.machine_symbol
                        && identity.state_symbol == self.state
                        && matches!(identity.origin,
                            checked_trees::IncomingReferenceOrigin::Invocation { expression, .. }
                                if loan.root == PlaceRoot::Expression(expression))
                };
            if loan.incoming_reference.is_valid() && !incoming && !output && !invocation {
                return false;
            }
            if invocation && loan.lineage != checked_trees::BorrowLoanLineage::DirectRoot {
                let same_origin = |parent| {
                    self.facts.borrow.loans.is_valid(parent) && {
                        let source = self.facts.borrow.loans.get(parent);
                        source.incoming_reference == loan.incoming_reference
                            && source.root == loan.root
                    }
                };
                let parent = match loan.lineage {
                    checked_trees::BorrowLoanLineage::Reborrow { parent_loan } => {
                        same_origin(parent_loan)
                    }
                    _ => false,
                };
                if !parent
                    && !self
                        .facts
                        .borrow
                        .loan_suppliers
                        .span(loan.supplying_loans)
                        .is_some_and(|suppliers| suppliers.iter().copied().any(same_origin))
                {
                    return false;
                }
            }
            if output {
                let Some(suppliers) = self.facts.borrow.loan_suppliers.span(loan.supplying_loans)
                else {
                    return false;
                };
                let same_output_supplier = suppliers.iter().any(|parent| {
                    self.facts.borrow.loans.is_valid(*parent) && {
                        let source = self.facts.borrow.loans.get(*parent);
                        source.incoming_reference == loan.incoming_reference
                            && self
                                .facts
                                .borrow
                                .access_segments
                                .span_or_empty(source.incoming_owner_path)
                                == self
                                    .facts
                                    .borrow
                                    .access_segments
                                    .span_or_empty(loan.incoming_owner_path)
                            && source.root == loan.root
                    }
                });
                if self.facts.borrow.output_origin_call(handle).is_none() && !same_output_supplier {
                    return false;
                }
            }
            if !incoming
                && !output
                && !invocation
                && !self.root(loan.root, constraints, active_roots, captured)
            {
                return false;
            }
            active_loans.push(handle);
            let parent_independent = match loan.lineage {
                checked_trees::BorrowLoanLineage::Reborrow { parent_loan } => self.loan(
                    parent_loan,
                    constraints,
                    active_roots,
                    active_loans,
                    captured,
                ),
                checked_trees::BorrowLoanLineage::DirectRoot if invocation => self
                    .invocation_lifetimes(
                        handle,
                        constraints,
                        active_roots,
                        active_loans,
                        captured,
                    ),
                checked_trees::BorrowLoanLineage::DirectRoot => true,
                checked_trees::BorrowLoanLineage::SuppliedOutput => output,
                checked_trees::BorrowLoanLineage::LifetimeDependency => {
                    self.question == StorageQuestion::RetainForArrival
                }
                checked_trees::BorrowLoanLineage::UnretainedDerived => {
                    incoming || !loan.supplying_loans.is_empty()
                }
            };
            let independent = parent_independent
                && self
                    .facts
                    .borrow
                    .loan_suppliers
                    .span(loan.supplying_loans)
                    .is_some_and(|suppliers| {
                        suppliers.iter().all(|supplier| {
                            self.loan(*supplier, constraints, active_roots, active_loans, captured)
                        })
                    });
            active_loans.pop();
            independent
        }
    }
}
pub(super) use loan_backing::RetainedBorrowRecords;
mod local_results {
    /*
        Selected exits partition established result custody, named or anonymous.
        Source checking owns cleanup eligibility and the exact remaining complement.
        This join identifies the retained producer before matching its selected exit
        receipt or delegating a whole no-work occurrence to affine frontier closure.
        Normal cleanup materializes selected hooks; graph closure retains no-code
        births, transfers and deaths independently of an exit receipt.
    */
    use checked_trees::{CheckFacts, CheckedUnitEffectOperationPlan};
    use language_semantics::{Multiplicity, PermissionEventSource};

    pub(super) fn permits_disposal(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        operation: &CheckedUnitEffectOperationPlan,
        operations: &[CheckedUnitEffectOperationPlan],
        statement: Option<u32>,
        continuation: bool,
        no_code_completion_owner: bool,
    ) -> bool {
        validate(
            program,
            facts,
            state,
            operation,
            operations,
            statement,
            continuation,
            no_code_completion_owner,
        )
        .is_some()
    }

    fn validate(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        operation: &CheckedUnitEffectOperationPlan,
        operations: &[CheckedUnitEffectOperationPlan],
        statement: Option<u32>,
        continuation: bool,
        no_code_completion_owner: bool,
    ) -> Option<()> {
        let result = operation.structural_result_binding()?;
        let root =
            super::super::cleanup::normal_edges::producer_root(facts, state.symbol, operation)?;
        if let Some(symbol) = root.as_symbol()
            && super::super::local_storage::occupant(program, state, operations, symbol)
                != Some(result.binding_ordinal)
        {
            return None;
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        let machine = facts
            .flow
            .control
            .state_by_symbol(state.symbol)?
            .machine_symbol;
        let exit = if let Some(statement) = statement {
            checked_trees::FlowClaimJoinExit {
                state_symbol: state.symbol,
                statement_index: statement as usize,
                kind: if continuation {
                    checked_trees::FlowClaimJoinExitKind::TransitionContinuation
                } else if statements
                    .get(statement as usize)
                    .is_some_and(|statement| statement.as_transition().is_some())
                {
                    checked_trees::FlowClaimJoinExitKind::TransitionTarget
                } else {
                    checked_trees::FlowClaimJoinExitKind::FinalExpression
                },
            }
        } else {
            let exits =
                super::super::cleanup::normal_edges::state_exits(program, facts, machine, state)
                    .ok()?;
            let [exit] = exits.as_slice() else {
                return None;
            };
            *exit
        };
        let statement_index = exit.statement_index;
        if result.statement_index as usize > statement_index {
            return None;
        }
        let reference = validation::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &validation::CanonicalPlace {
                root,
                segments: Vec::new(),
            },
        )?;
        let expected_identity =
            validation::stored_structural_type_identity(program, reference, &[], &[]);
        let referent_identity = validation::reference_shell(program, reference).map(|shell| {
            validation::stored_structural_type_identity(program, shell.referee, &[], &[])
        });
        let multiplicity =
            validation::reference_result_custody::binding_multiplicity(program, reference);
        if (referent_identity.is_some() && result.multiplicity != Multiplicity::Unrestricted)
            || (result.type_identity != expected_identity
                && referent_identity != Some(result.type_identity))
            || result.multiplicity != multiplicity
        {
            return None;
        }
        if multiplicity == Multiplicity::Unrestricted {
            return Some(());
        }
        if no_code_completion_owner
            && validation::has_no_code_cleanup_contents(program, reference)
            && validation::has_contents(
                program,
                reference,
                &[],
                validation::ContentsRequirement::BorrowFree,
            )
        {
            return Some(());
        }
        facts
            .contextual_cleanup
            .dispositions
            .iter()
            .any(|(_, disposition)| {
                disposition.machine
                    == super::super::cleanup::owning_cleanup_machine(program, state.symbol)
                    && disposition.state == state.symbol
                    && disposition.root == root
                    && disposition.source == PermissionEventSource::StateExit
                    && disposition.exit == exit
            })
            .then_some(())
    }
}
mod operation_custody {
    /*
        Admitting one composed Unit state's planned operations.

        Named structural results stay live through successor operand evaluation:
        the selected edge, not one global return flag, owns their transfer or
        disposal. Every remaining operation must then carry custody a composed
        state can replay; one that does not names its kind in the trace.
        Completed earlier operations expose their retained nested value calls through
        the common dependency inventory. Admission rejoins one exact producer and
        call boundary; inventory order does not schedule evaluation or select death.
    */
    use super::result_custody::{
        call_owns_result, operand_owned_construction, retained_structural_result,
    };
    use super::{
        CheckedStructuralAccess as StructuralAccess,
        CheckedUnitEffectOperationPlan as OperationPlan,
        CheckedUnitStructuralArgumentPlan as ArgumentPlan, Multiplicity, StatementNode, control,
    };
    use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute};

    pub(super) fn admit(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        operations: &mut [OperationPlan],
        trace: &control::LocalConstructionTrace,
    ) -> Option<()> {
        let statements = program.statement_table.statements(state.statement_nodes);
        for operation in operations.iter_mut() {
            let operand_owned = operand_owned_construction(operation);
            if let Some((result, discard_result_on_return)) =
                operation.discardable_structural_result_mut()
                && (operand_owned
                    || matches!(
                        statements.get(result.statement_index as usize),
                        Some(StatementNode::LocalData(_))
                    ))
            {
                *discard_result_on_return = false
            }
        }
        for (operation_index, operation) in operations.iter().enumerate() {
            match operation {
                OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Structural {
                        initial_value: Some(value),
                        ..
                    },
                ) => {
                    if !call_argument_admitted(value.source(), &operations[..operation_index]) {
                        return None;
                    }
                }
                OperationPlan::Call(call)
                    if call_custody_admitted(call, &operations[..operation_index]) => {}
                OperationPlan::CaptureAssignmentDestination { destination, .. }
                    if call_argument_admitted(destination, &operations[..operation_index]) => {}
                OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. },
                )
                | OperationPlan::EvaluateStructuralValue { .. }
                | checked_trees::byte_sequence_establishment_pattern!()
                | OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. },
                )
                | OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. },
                )
                | OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. },
                )
                | OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. },
                )
                | OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Structural { .. },
                )
                | checked_trees::reference_hold_pattern!() => {}
                OperationPlan::Store(store)
                    if store.installed_value().is_none_or(|value| {
                        value.access == StructuralAccess::Owned
                            && call_argument_admitted(value, &operations[..operation_index])
                    }) => {}
                OperationPlan::AtomicFence { .. }
                | OperationPlan::AtomicAccess(_)
                | OperationPlan::RegisterAccess { .. }
                | OperationPlan::GlobalDescriptorTable { .. } => {}
                OperationPlan::MoveStructuralField { source, .. }
                    if source.access == StructuralAccess::Owned
                        && call_argument_admitted(source, &operations[..operation_index]) => {}
                OperationPlan::ContinuationCleanup {
                    boundary,
                    cleanup_actions,
                    ..
                } if !cleanup_actions.is_empty()
                    && cleanup_actions.iter().all(|action| {
                        call_argument_admitted(
                            &checked_trees::CheckedUnitStructuralArgumentPlan {
                                projection_evaluations: Default::default(),
                                source: action.source().clone(),
                                path: action.path().to_vec(),
                                type_identity: action.type_identity().to_owned(),
                                access: StructuralAccess::Owned,
                            },
                            &operations[..operation_index],
                        )
                    })
                    && match boundary {
                        checked_trees::CheckedUnitCleanupBoundary::Call(coordinate) => {
                            operations[..operation_index]
                                .iter()
                                .flat_map(OperationPlan::with_value_calls)
                                .filter_map(OperationPlan::invocation_coordinate)
                                .filter(|invocation| *invocation == *coordinate)
                                .count()
                                == 1
                        }
                        checked_trees::CheckedUnitCleanupBoundary::StatementEntry {
                            statement_index,
                        }
                        | checked_trees::CheckedUnitCleanupBoundary::Replacement {
                            statement_index,
                            ..
                        } => statements.get(*statement_index as usize).is_some(),
                    } => {}
                _ => {
                    trace.phase(match operation {
                        OperationPlan::Call(call) => match (call.route(), call.result()) {
                            (_, CheckedUnitCallResult::Structural { .. }) => {
                                "state graph: operation custody: discarded structural result"
                            }
                            (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::NoNormal) => {
                                "state graph: operation custody: no-normal invocation is not an opaque boundary completion"
                            }
                            (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Unit) => {
                                "state graph: operation custody: boundary call arguments"
                            }
                            (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Scalar(_)) => {
                                "state graph: operation custody: scalar call"
                            }
                            (
                                CheckedUnitCallRoute::Module { .. } | CheckedUnitCallRoute::Dynamic { .. } | CheckedUnitCallRoute::Runtime { .. } | CheckedUnitCallRoute::ActivationStorage { .. } | CheckedUnitCallRoute::DescriptorTable { .. },
                                _,
                            ) => call_custody_refusal(
                                &call.structural_arguments,
                                &operations[..operation_index],
                            ),
                        },
                        OperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. }) => {
                            "state graph: operation custody: discarded structural result"
                        }
                        OperationPlan::ContinuationCleanup { .. } => {
                            "state graph: operation custody: continuation cleanup owner"
                        }
                        checked_trees::reference_hold_pattern!() => {
                            "state graph: operation custody: reference custody"
                        }
                        OperationPlan::Complete { .. } => {
                            "state graph: operation custody: completion"
                        }
                        _ => "state graph: operation custody: other operation",
                    });
                    return None;
                }
            }
        }
        Some(())
    }

    /*
        Whether the selected edges of a composed state can carry one call's
        custody. A structural result must stay live for its edge to own; a
        boundary call completes its receipts at the call; an ordinary call consumes
        its exact argument claims through the shared call custody replay. Every
        structural operand except a structural result's must already be admitted.
    */
    pub(super) fn call_custody_admitted(
        call: &CheckedUnitCallPlan,
        earlier: &[OperationPlan],
    ) -> bool {
        let arguments_admitted = || {
            call.structural_arguments
                .iter()
                .all(|argument| call_argument_admitted(argument, earlier))
        };
        match (call.route(), call.result()) {
            (
                _,
                CheckedUnitCallResult::Structural {
                    discard_result_on_return,
                    ..
                },
            ) => !discard_result_on_return,
            (_, _) => arguments_admitted(),
        }
    }

    /*
        The call sequencer already joined each operand to its loan or consumption.
        A state edge does not change that call's argument custody. Boundary and
        ordinary calls share this rule; their receipts and claim transfers remain
        separate obligations at their respective consumers.
    */
    fn call_argument_admitted(argument: &ArgumentPlan, earlier: &[OperationPlan]) -> bool {
        if argument.access != StructuralAccess::Owned {
            return true;
        }
        let Some(ordinal) = argument.source_structural_result_binding_ordinal() else {
            return true;
        };
        let mut producers = earlier
            .iter()
            .flat_map(OperationPlan::with_value_calls)
            .filter_map(|operation| {
                let result = operation.structural_result_binding()?;
                (result.binding_ordinal == ordinal).then_some((operation, result))
            });
        let Some((producer, result)) = producers.next() else {
            return false;
        };
        producers.next().is_none()
            && retained_structural_result(producer).is_some()
            && (result.multiplicity == Multiplicity::Unrestricted || call_owns_result(producer))
    }

    /*
        Name the first unmet call requirement using the same argument admission
        as the operation guard, then distinguish its unsupported custody.
    */
    fn call_custody_refusal(
        structural_arguments: &[ArgumentPlan],
        earlier: &[OperationPlan],
    ) -> &'static str {
        for argument in structural_arguments {
            if call_argument_admitted(argument, earlier) {
                continue;
            }
            if argument.access == StructuralAccess::Owned
                && let Some(ordinal) = argument.source_structural_result_binding_ordinal()
            {
                let producer = earlier
                    .iter()
                    .rev()
                    .flat_map(OperationPlan::with_value_calls)
                    .find_map(|operation| {
                        operation
                            .structural_result_binding()
                            .filter(|result| result.binding_ordinal == ordinal)
                    });
                return match producer.map(|result| result.multiplicity) {
                    None => "state graph: operation custody: call owned result without a producer",
                    Some(_) => "state graph: operation custody: call owned local result",
                };
            }
            return "state graph: operation custody: unit call arguments";
        }
        "state graph: operation custody: unit call arguments"
    }
}
mod ranking {
    /*
        Read the source-admitted ordered rank roster, preserve every component's
        coordinate association, then bind its scalar roots into this executable
        control plan. Admission prototypes and executable spans have distinct arena
        owners; fusion changes placement, never tuple order or source meaning.
    */
    use super::{FusedStates, fused_states};

    /*
        Read one retained source observation roster for the authored owners in this
        composed graph. Fusion changes control placement, never the chosen measure
        coordinate or the source judgment. Weak call-component edges come from its
        same retained complete decision.
    */
    pub(crate) fn natural_rank_plans(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        admitted: &checked_trees::CheckedRankArrivalPlan,
    ) -> Option<(
        Vec<checked_trees::CheckedStateNaturalRank>,
        Vec<checked_trees::CheckedRankEdgeCoordinate>,
    )> {
        let fused = fused_states(program, machine);
        natural_rank_plans_for_fused(program, &fused, admitted)
    }

    pub(super) fn natural_rank_plans_for_fused(
        program: &dyn validation::CheckingInput,
        fused: &FusedStates<'_>,
        admitted: &checked_trees::CheckedRankArrivalPlan,
    ) -> Option<(
        Vec<checked_trees::CheckedStateNaturalRank>,
        Vec<checked_trees::CheckedRankEdgeCoordinate>,
    )> {
        let has_witness = fused
            .iter()
            .any(|(owner, _)| owner.termination_plan.implementation_witness.is_some());
        if !has_witness && admitted.runtime_components.is_empty() {
            return Some((Vec::new(), Vec::new()));
        }
        let members = fused
            .iter()
            .map(|(_, state)| state.symbol)
            .collect::<Vec<_>>();
        let mut ranks = Vec::new();
        for (owner, state) in fused {
            let judgment = admitted.judgment_for_body(owner.symbol, owner.states)?;
            if owner.termination_plan.implementation_witness.is_none()
                && judgment.observations.is_empty()
            {
                continue;
            }
            if !judgment.local_terminates {
                return None;
            }
            let observations = admitted.observations.span(judgment.observations)?;
            let relevance = validation::rank_relevance(program, &[owner.symbol])?;
            if relevance.requires_rank(state.symbol)
                && !observations.iter().any(|rank| rank.state == state.symbol)
            {
                return None;
            }
            ranks.extend(
                observations
                    .iter()
                    .filter(|rank| rank.state == state.symbol)
                    .cloned(),
            );
        }
        ranks.sort_by_key(|rank| (rank.state.arena_index(), rank.state.generation()));
        for same_state in ranks.windows(2) {
            if same_state[0].state == same_state[1].state && same_state[0] != same_state[1] {
                return None;
            }
        }
        ranks.dedup();
        if admitted.runtime_components.is_empty() {
            return Some((ranks, Vec::new()));
        }
        let mut weak = Vec::new();
        for component in validation::retained_runtime_natural_components(admitted) {
            if !component
                .states
                .iter()
                .any(|rank| members.contains(&rank.state))
                && !component
                    .edges
                    .iter()
                    .any(|edge| members.contains(&edge.source) || members.contains(&edge.target))
            {
                continue;
            }
            if !component
                .states
                .iter()
                .all(|rank| members.contains(&rank.state))
                || !component
                    .edges
                    .iter()
                    .all(|edge| members.contains(&edge.source) && members.contains(&edge.target))
            {
                return None;
            }
            if component.states.iter().any(|state| {
                !ranks
                    .iter()
                    .filter(|rank| rank.state == state.state)
                    .any(|root| {
                        let Some(values) = admitted.observation_components.span(root.components) else {
                            return false;
                        };
                        !values.is_empty()
                            && values.len() == state.values.len()
                            && values.iter().zip(&state.values).all(|(rank, state)| {
                                rank.parameter == state.parameter
                                    && rank.parameter_position == state.parameter_position
                                    && rank.function == state.function
                                    && match (&state.observation, rank.measure) {
                                        (
                                            validation::RuntimeRankInput::Value(
                                                validation::RankPremiseObservation::Value,
                                            ),
                                            checked_trees::CheckedNaturalRankMeasure::DeclaredMeasure {
                                                measure,
                                                bindings,
                                                primitive_type,
                                                formation,
                                                ..
                                            },
                                        ) => {
                                            let Some(retained) = admitted.bindings.span(bindings).and_then(|bindings| {
                                                bindings.iter().map(|binding| validation::RuntimeRankApplicationBinding::from_checked(admitted, binding))
                                                    .collect::<Option<Vec<_>>>()
                                            }) else { return false; };
                                            measure == state.measure
                                                && measure.is_valid()
                                                && primitive_type == state.primitive_type
                                                && retained == state.application_bindings
                                                && (!admitted.application_formations.is_valid(formation)
                                                    || admitted.application_formations.get(formation).input_home.is_none_or(|input| {
                                                    let coordinate = admitted.coordinates.get(input.actual);
                                                    coordinate.parameter == state.coordinate.parameter
                                                        && admitted.path_segments.span(coordinate.path) == Some(state.coordinate.path.as_slice())
                                                        && validation::declared_input_type_matches(program, state.input_type, input.actual_type)
                                                }))

                                        }
                                        (
                                            validation::RuntimeRankInput::Value(
                                                validation::RankPremiseObservation::Value,
                                            ),
                                            checked_trees::CheckedNaturalRankMeasure::IntegerValue {
                                                coordinate,
                                                primitive_type,
                                            },
                                        ) => {
                                            let coordinate = admitted.coordinates.get(coordinate);
                                            !state.measure.is_valid()
                                                && primitive_type == state.primitive_type
                                                && coordinate.parameter == state.coordinate.parameter
                                                && admitted.path_segments.span(coordinate.path)
                                                    == Some(state.coordinate.path.as_slice())
                                        }
                                        (
                                            validation::RuntimeRankInput::Value(
                                                validation::RankPremiseObservation::Length,
                                            ),
                                            checked_trees::CheckedNaturalRankMeasure::SequenceLength {
                                                coordinate,
                                            },
                                        ) => {
                                            let coordinate = admitted.coordinates.get(coordinate);
                                            coordinate.parameter == state.coordinate.parameter
                                                && admitted.path_segments.span(coordinate.path)
                                                    == Some(state.coordinate.path.as_slice())
                                        }
                                        (
                                            validation::RuntimeRankInput::Distance {
                                                lower_observation,
                                                lower_primitive,
                                                upper: expected_upper,
                                            },
                                            checked_trees::CheckedNaturalRankMeasure::Distance {
                                                lower,
                                                upper,
                                            },
                                        ) => {
                                            let expected_lower =
                                                validation::RuntimeRankOperand::Coordinate {
                                                    parameter: state.parameter,
                                                    coordinate: state.coordinate.clone(),
                                                    entry_indexes: state.entry_indexes.clone(),
                                                    primitive_type: *lower_primitive,
                                                    observation: *lower_observation,
                                                };
                                            !state.measure.is_valid()
                                                && expected_lower.matches_checked(admitted, lower)
                                                && expected_upper.matches_checked(admitted, upper)
                                        }
                                        _ => false,
                                    }
                            })
                    })
            }) {
                return None;
            }
            weak.extend(
                component
                    .edges
                    .iter()
                    .filter(|edge| !edge.strict)
                    .map(|edge| checked_trees::CheckedRankEdgeCoordinate {
                        source_state: edge.source,
                        target_state: edge.target,
                        statement_ordinal: edge.statement_ordinal,
                    }),
            );
        }
        weak.sort_by_key(|edge| {
            (
                edge.source_state.arena_index(),
                edge.source_state.generation(),
                edge.statement_ordinal,
                edge.target_state.arena_index(),
                edge.target_state.generation(),
            )
        });
        weak.dedup();
        Some((ranks, weak))
    }

    /*
        Rebind every component into the composed plan's own arena. A missing root
        rejects the whole tuple; it cannot shorten the relation to a passing prefix.
        Cloning each admitted component preserves parameter, position, measure and
        function identity in order; only its executable root and arena owner change.
        This checking-time construction owns tuple completeness, so lowering reads
        the published tuple without comparing it to the source prototype again.
    */
    pub(super) fn bind_observations(
        program: &dyn validation::CheckingInput,
        admitted: &checked_trees::CheckedRankArrivalPlan,
        computations: &checked_trees::CheckedScalarComputationPlans,
        ranks: &mut [checked_trees::CheckedStateNaturalRank],
    ) -> Option<arena::Arena<checked_trees::CheckedIntegerRankObservation>> {
        let mut components = arena::Arena::new();
        for rank in ranks {
            let (owner, _) = validation::find_state(program, rank.state)?;
            let prototype = admitted.observation_components.span(rank.components)?;
            if prototype.is_empty() {
                return None;
            }
            let values = prototype
                .iter()
                .enumerate()
                .map(|(component, value)| {
                    let root = computations.root_at(
                        rank.state,
                        0,
                        checked_trees::CheckedScalarExpressionRole::RankObservation {
                            component: u32::try_from(component).ok()?,
                        },
                    )?;
                    if root.machine != owner.symbol || !root.root.is_valid() {
                        return None;
                    }
                    let mut value = value.clone();
                    value.observation = root.root;
                    Some(value)
                })
                .collect::<Option<Vec<_>>>()?;
            rank.components = components.insert_many(values);
        }
        Some(components)
    }

    /*
        Publish the complete witness roster before it crosses the checking boundary.
        The selector and binder above preserve tuple identity by construction. Keep
        completeness here so a future producer change cannot publish a shortened or
        substituted tuple and defer the source obligation to lowering.
    */
    pub(super) fn complete_witness_ranks(
        fused: &FusedStates<'_>,
        admitted: &checked_trees::CheckedRankArrivalPlan,
        ranks: &[checked_trees::CheckedStateNaturalRank],
        components: &arena::Arena<checked_trees::CheckedIntegerRankObservation>,
    ) -> Option<()> {
        for (owner, state) in fused {
            if owner.termination_plan.implementation_witness.is_none() {
                continue;
            }
            let judgment = admitted.judgment_for_body(owner.symbol, owner.states)?;
            if !judgment.local_terminates {
                return None;
            }
            for required in admitted
                .observations
                .span(judgment.observations)?
                .iter()
                .filter(|rank| rank.state == state.symbol)
            {
                let wanted = admitted.observation_components.span(required.components)?;
                let retained = ranks
                    .iter()
                    .find(|rank| rank.state == required.state)
                    .and_then(|rank| components.span(rank.components))?;
                if wanted.is_empty()
                    || wanted.len() != retained.len()
                    || !wanted.iter().zip(retained).all(|(wanted, actual)| {
                        wanted.parameter == actual.parameter
                            && wanted.parameter_position == actual.parameter_position
                            && wanted.measure == actual.measure
                            && wanted.function == actual.function
                            && actual.observation.is_valid()
                    })
                {
                    return None;
                }
            }
        }
        Some(())
    }
}
mod result_custody {

    /*
        Result custody accounting for one composed Unit state.

        Every structural result a state's operations produce has exactly one
        owner on each path out of the state: a later operation that consumes it,
        a store that writes it, a successor edge that transfers it, or a cleanup
        row that discards it. A domain-ending or all-crash exit abandons custody without
        manufacturing a disposition. Normal paths with no owner and every path
        with duplicate dispositions refuse; closed sums account per case.
        Named homes are not permanent aliases of their first result. Cleanup and
        construction resolve the value at their operation prefix, sharing operand
        storage selection so replacement neither loses nor revives an occurrence.
        Anonymous input views and returned children can share a statement without
        sharing custody. Loan dispositions join the result's captured holder, never
        every claim established by that statement. An anonymous borrowed window
        lent whole into the one call that consumes it has that call as its
        disposition; the loan schedule retires the descriptor, so the state does
        not also demand a return-discard.
        For a holder with captured loans, whole physical cleanup realizes its
        checked release. Both inventories must close; they are one result death.
        Projected transfers and residual descriptor releases can jointly exhaust an
        array. Their exact typed frontier replaces store-only partition recognition;
        covering a nominal owner's fields still cannot discharge its own cleanup.
        Scalar and structural return tails account each selected guard prefix and
        value together with that exit's exact disposition. A consuming alternative
        cannot stand in for cleanup on a sibling, and terminal local aliases name
        only the current occupant after any body-level replacement.
    */
    use super::super::local_storage::occupant;
    use super::super::storage_transfers::has_computation_transfer;
    use super::super::types::borrowed_slice_result;
    use super::guards::retained_guard;
    use super::{
        CheckFacts, CheckedComposedUnitControlTerminatorPlan as TerminatorPlan,
        CheckedStructuralAccess as StructuralAccess, CheckedStructuralControlSuccessorPlan,
        CheckedUnitEffectOperationPlan as OperationPlan,
        CheckedUnitStructuralArgumentSourcePlan as SourcePlan, Multiplicity, PermissionEventKind,
        PermissionEventSource, StatementNode, control, local_results,
    };
    use crate::execution::permission_events;
    use checked_trees::{
        CheckedConditionalReturnArm as ReturnArm,
        CheckedUnitStructuralArgumentPlan as ArgumentPlan, CheckedUnitStructuralParameterPlan,
    };

    pub(super) fn account<'calls>(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        call_schedules: &dyn Fn(
            checked_trees::CheckedStructuralValueHandle,
        )
            -> Option<&'calls [checked_trees::CheckedStructuralValueCall]>,
        shapes: &super::ShapeCollector<'_>,
        prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        structural_results: &[(
            checked_trees::CheckedUnitStructuralResultBindingPlan,
            facts::PlaceRoot,
        )],
        operations: &mut [OperationPlan],
        terminator: &TerminatorPlan,
        selected_exit: Option<checked_trees::FlowClaimJoinExit>,
        completion: checked_trees::CheckedNormalCompletion,
        trace: &control::LocalConstructionTrace,
    ) -> Option<()> {
        if selected_exit.is_some_and(|exit| {
            exit.state_symbol != state.symbol
                || exit.kind != checked_trees::FlowClaimJoinExitKind::TransitionTarget
        }) {
            return None;
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        trace.phase("state graph: result custody accounting");
        let has_normal_continuation =
            completion == checked_trees::CheckedNormalCompletion::Possible;
        let entry_guard = match terminator {
            TerminatorPlan::Conditional { guard, .. }
            | TerminatorPlan::ConditionalReturn { guard, .. }
            | TerminatorPlan::ConditionalCompletion { guard, .. } => Some(guard.clone()),
            TerminatorPlan::GuardedJumps { arms, .. } => {
                arms.first().map(|arm| arm.guard().clone())
            }
            TerminatorPlan::Guarded { .. } => None,
            _ => None,
        };
        let construction_uses = construction_local_uses(facts, state.symbol, operations.iter())?;
        let successors = terminator.successors();
        let successor_construction_uses = successors
            .iter()
            .map(|edge| construction_local_uses(facts, state.symbol, edge.operand_operations()))
            .collect::<Option<Vec<_>>>()?;
        let return_arms = terminator.return_arms();
        let has_normal_caller_completion = has_normal_continuation
            && (successors.is_empty()
                || successors
                    .iter()
                    .any(|edge| edge.has_normal_caller_completion())
                || return_arms.iter().any(|arm| arm.has_normal_continuation()));
        let return_construction_uses = return_arms
            .iter()
            .map(|arm| construction_local_uses(facts, state.symbol, arm.operation_dependencies()))
            .collect::<Option<Vec<_>>>()?;
        let guarded_return_construction_uses = match terminator {
            TerminatorPlan::Guarded { return_values, .. } => return_values
                .iter()
                .map(|value| {
                    construction_local_uses(facts, state.symbol, std::iter::once(&value.operation))
                })
                .collect::<Option<Vec<_>>>()?,
            _ => Vec::new(),
        };
        let mut guarded_discards = Vec::new();
        for (producer_index, operation) in operations.iter().enumerate() {
            if matches!(
                operation,
                OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Structural { .. }
                )
            ) {
                continue;
            }
            let Some((result, _)) = operation.discardable_structural_result() else {
                continue;
            };
            let result_local = operation
                .statement_structural_result_binding()
                .and_then(|binding| statements.get(binding.statement_index as usize))
                .and_then(|statement| match statement {
                    StatementNode::LocalData(local) => Some(local.symbol),
                    _ => None,
                })
                .unwrap_or_default();
            let consumers = &operations[producer_index + 1..];
            let occurrence_end = consumers
                .iter()
                .position(|consumer| consumer.ends_local_occurrence(result_local));
            let terminal_local = if occupant(program, state, operations, result_local)
                == Some(result.binding_ordinal)
            {
                result_local
            } else {
                Default::default()
            };
            let occupies_result = |source: &SourcePlan, before: usize| {
                let source = super::super::local_storage::operand_source(
                    program,
                    state,
                    &operations[..before],
                    structural_results,
                    source,
                );
                matches!(source,
                    Some(SourcePlan::StructuralResult { binding_ordinal })
                        if binding_ordinal == result.binding_ordinal)
            };
            let construction_owns_result =
                |argument: &checked_trees::CheckedUnitStructuralArgumentPlan, before: usize| {
                    occupies_result(&argument.source, before)
                        && argument.type_identity == result.type_identity
                        && (result.multiplicity == Multiplicity::Unrestricted
                            || argument.access == StructuralAccess::Owned)
                        && argument.path.is_empty()
                };
            let successor_moves = successors
                .iter()
                .zip(&successor_construction_uses)
                .map(|(edge, constructions)| {
                    let direct = edge.transfers.iter().filter(|transfer| matches!(
                        transfer.source,
                        checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal }
                            if binding_ordinal == result.binding_ordinal
                    )).count();
                    let operands = if result.multiplicity == Multiplicity::Unrestricted {
                        0
                    } else {
                        edge.operand_calls()
                            .flat_map(OperationPlan::structural_operands)
                            .filter(|argument| construction_owns_result(argument, operations.len()))
                            .count()
                            + constructions.iter().filter(|(_, argument)| {
                                construction_owns_result(argument, operations.len())
                            }).count()
                    };
                    (*edge, direct + operands)
                })
                .collect::<Vec<_>>();
            let (cleanup_discards, retired_whole) = operations
                .iter()
                .enumerate()
                .skip(producer_index + 1)
                .filter_map(|(operation_index, operation)| match operation {
                    OperationPlan::ContinuationCleanup {
                        boundary,
                        cleanup_actions,
                        ..
                    } => Some((operation_index, boundary, cleanup_actions)),
                    _ => None,
                })
                .fold(
                    (0, false),
                    |(count, retired_whole), (operation_index, boundary, discards)| {
                        if let checked_trees::CheckedUnitCleanupBoundary::Replacement {
                            old_value,
                            ..
                        } = boundary
                            && (!old_value.path.is_empty()
                                || old_value.type_identity != result.type_identity
                                || !occupies_result(&old_value.source, operation_index))
                        {
                            return (count, retired_whole);
                        }
                        let mut matched = false;
                        let mut whole = false;
                        for discard in discards {
                            if occupies_result(discard.source(), operation_index) {
                                matched = true;
                                whole |= discard.path().is_empty()
                                    && discard.type_identity() == result.type_identity;
                            }
                        }
                        (count + usize::from(matched), retired_whole || whole)
                    },
                );
            if retired_whole
                && (successor_moves.iter().any(|(_, moves)| *moves != 0)
                    || terminator.return_arms().iter().any(|arm| {
                        arm.operation_dependencies().any(|operation| {
                            operation.with_value_calls().any(|operation| {
                                terminator_call_consumes_result(operation, result.binding_ordinal)
                            })
                        })
                    }))
            {
                return None;
            }
            let (reference_releases, deferred_reference_releases) = if result.multiplicity
                != Multiplicity::Unrestricted
                && shapes.types.get(&result.type_identity)?.descriptor_cleanup
            {
                let captured = control::statement_sequence::captured_reference_result(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    structural_results,
                    operation,
                    operations,
                );
                let (released, loans) = captured.map_or((result, Default::default()), |captured| {
                    (captured.result, captured.loans)
                });
                operations[producer_index + 1..]
                    .iter()
                    .flat_map(OperationPlan::with_value_calls)
                    .filter(|operation| {
                        control::statement_sequence::release_matches(operation, released, loans)
                    })
                    .fold((0, 0), |(immediate, deferred), operation| {
                        if matches!(
                            operation,
                            OperationPlan::ReleaseReference {
                                defer_to_exit: true,
                                ..
                            }
                        ) {
                            (immediate, deferred + 1)
                        } else {
                            (immediate + 1, deferred)
                        }
                    })
            } else {
                (0, 0)
            };
            if deferred_reference_releases > 1 {
                trace.phase("state graph: duplicate deferred reference dispositions");
                trace.statement(Some(result.statement_index));
                return None;
            }
            let selection_source_disposition =
                facts
                    .flow
                    .ownership
                    .with_state_owned_selections(state.symbol, |handles| {
                        handles.iter().copied().any(|handle| {
                            let receipt = facts.flow.ownership.owned_selections.get(handle);
                            receipt.machine == machine.symbol
                                && receipt.state == state.symbol
                                && facts
                                    .flow
                                    .ownership
                                    .selection_sources
                                    .span_or_empty(receipt.sources)
                                    .iter()
                                    .any(|source| {
                                        if source.retains_storage
                                            && (result.multiplicity != Multiplicity::Affine
                                                || receipt.death
                                                    != PermissionEventSource::StateExit)
                                        {
                                            return false;
                                        }
                                        let boundary = operations
                                            .iter()
                                            .position(|operation| {
                                                operation.source_statement_index().is_some_and(
                                                    |statement| {
                                                        statement >= receipt.statement_ordinal
                                                    },
                                                )
                                            })
                                            .unwrap_or(operations.len());
                                        super::super::local_storage::source(
                                            program,
                                            state,
                                            &operations[..boundary],
                                            source.symbol,
                                        ) == Some(SourcePlan::StructuralResult {
                                            binding_ordinal: result.binding_ordinal,
                                        })
                                    })
                        })
                    });
            let view_source = match operation {
                OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
                ) if facts.values.structural_values.nodes.is_valid(*value) => {
                    let node = facts.values.structural_values.nodes.get(*value);
                    match &node.kind {
                        checked_trees::CheckedStructuralValueKind::BorrowedSliceView {
                            source,
                            ..
                        }
                        | checked_trees::CheckedStructuralValueKind::MutableSliceView {
                            source,
                            ..
                        }
                        | checked_trees::CheckedStructuralValueKind::Reference { source, .. }
                            if borrowed_slice_result(program, node.type_reference) =>
                        {
                            Some(source)
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            let parameter_symbol = |parameter_index: u32| {
                let parameter = structural_parameters.get(parameter_index as usize)?;
                let authored = program
                    .state_parameters(state)
                    .get(parameter.position as usize)?;
                false.then_some(machine.symbol).or(Some(authored.symbol))
            };
            let same_view_root = |argument: &checked_trees::CheckedUnitStructuralArgumentPlan| {
                let Some(view_source) = view_source else {
                    return false;
                };
                match (&argument.source, &view_source.source) {
                    (
                        SourcePlan::Parameter { parameter_index },
                        SourcePlan::StructuralLocal { symbol },
                    ) => parameter_symbol(*parameter_index) == Some(*symbol),
                    (
                        SourcePlan::StructuralLocal { symbol },
                        SourcePlan::Parameter { parameter_index },
                    ) => Some(*symbol) == parameter_symbol(*parameter_index),
                    _ => argument.source == view_source.source,
                }
            };
            let exact_view_reborrow =
                |source: &checked_trees::CheckedUnitStructuralArgumentPlan, before: usize| {
                    let Some(view_source) = view_source else {
                        return false;
                    };
                    let observed_source = match source.source {
                        SourcePlan::StructuralLocalBorrow { symbol, .. } => {
                            SourcePlan::StructuralLocal { symbol }
                        }
                        _ => source.source.clone(),
                    };
                    let through_carrier =
                        occupies_result(&observed_source, before) && !source.path.is_empty();
                    let through_backing = same_view_root(source)
                        && source.path.starts_with(&view_source.path)
                        && (source.path.len() > view_source.path.len()
                            || source.type_identity == view_source.type_identity);
                    (through_carrier || through_backing)
                        && matches!(
                            (view_source.access, source.access),
                            (
                                StructuralAccess::MutableBorrow,
                                checked_trees::checked_borrowed_access_pair_pattern!(),
                            ) | (
                                StructuralAccess::SharedBorrow,
                                StructuralAccess::SharedBorrow,
                            )
                        )
                };
            let affine_view = result.multiplicity == Multiplicity::Affine
                && matches!(operation,
                OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. }
                ) if facts.values.structural_values.nodes.is_valid(*value)
                    && borrowed_slice_result(
                        program,
                        facts.values.structural_values.nodes.get(*value).type_reference,
                    ));
            let installed_view_claims = affine_view
                .then(|| {
                    installed_view_claims(
                        facts,
                        machine.symbol,
                        state.symbol,
                        operation,
                        result,
                        &operations[producer_index + 1..],
                    )
                })
                .flatten()
                .unwrap_or_default();
            let (has_captured_view_loans, owns_local_view_claim, view_releases) = if affine_view {
                view_loan_releases(
                    facts,
                    machine.symbol,
                    state.symbol,
                    result,
                    operation,
                    &installed_view_claims,
                )
                .map_err(|reason| {
                    trace.phase(reason);
                    trace.statement(Some(result.statement_index));
                })
                .ok()?
            } else {
                (false, false, 0)
            };
            let release_owned_view = affine_view && owns_local_view_claim;
            let has_captured_view_loans = release_owned_view && has_captured_view_loans;
            let storage_transfers = if result.multiplicity == Multiplicity::Unrestricted {
                None
            } else {
                Some(super::super::storage_transfers::transfers(
                    program,
                    facts,
                    prepared_calls,
                    structural_results,
                    result.binding_ordinal,
                    operations,
                )?)
            };
            let moves_occurrence = |argument: &ArgumentPlan| {
                argument.access == StructuralAccess::Owned
                    && occupies_result(&argument.source, operations.len())
            };
            let no_code_completion_owner = result.multiplicity == Multiplicity::Affine
                && result.local.access == StructuralAccess::Owned
                && shapes
                    .types
                    .get(&result.type_identity)
                    .is_some_and(|declaration| {
                        declaration.no_code_affine_cleanup && !declaration.owns_linear_claim
                    })
                && producer_claim_paths(program, facts, operation)
                    .is_some_and(|claims| claims.is_empty())
                && storage_transfers
                    .as_ref()
                    .is_some_and(|transfers| transfers.possible.is_empty())
                && matches!(
                    has_computation_transfer(
                        program,
                        facts,
                        state.symbol,
                        structural_results,
                        result,
                        operations,
                    ),
                    Some(false)
                )
                && successor_moves.iter().all(|(_, moves)| *moves == 0)
                && !successors.iter().any(|edge| {
                    edge.transfers.iter().any(|transfer| matches!(
                        &transfer.source,
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedValue { subject }
                            if moves_occurrence(subject)
                    ))
                })
                && !construction_uses
                    .iter()
                    .chain(successor_construction_uses.iter().flatten())
                    .chain(return_construction_uses.iter().flatten())
                    .chain(guarded_return_construction_uses.iter().flatten())
                    .any(|(_, argument)| moves_occurrence(argument))
                && !successors
                    .iter()
                    .flat_map(|edge| edge.operand_calls())
                    .chain(return_arms.iter().flat_map(|arm| {
                        arm.operation_dependencies()
                            .flat_map(OperationPlan::with_value_calls)
                    }))
                    .flat_map(OperationPlan::structural_operands)
                    .any(&moves_occurrence);
            let call_transfers = if result.multiplicity == Multiplicity::Unrestricted {
                0
            } else {
                consumers[..occurrence_end.map_or(consumers.len(), |position| position + 1)]
                    .iter()
                    .map(|consumer| {
                        consumer.definite_call_transfers(
                            &facts.values,
                            call_schedules,
                            operation,
                            result_local,
                        )
                    })
                    .sum::<usize>()
                    + if occurrence_end.is_none() {
                        entry_guard.as_ref().map_or(0, |guard| {
                            guard.definite_call_transfers(
                                &facts.values,
                                call_schedules,
                                operation,
                                terminal_local,
                            )
                        })
                    } else {
                        0
                    }
            };
            let construction_count = construction_uses
                .iter()
                .filter(|(consumer_index, argument)| {
                    *consumer_index > producer_index
                        && construction_owns_result(argument, *consumer_index)
                })
                .count();
            let construction_transfers = if result.multiplicity == Multiplicity::Unrestricted {
                0
            } else {
                construction_count
            };
            let permits_disposal = |statement: Option<u32>, continuation: bool| {
                let statement = statement.or_else(|| {
                    selected_exit.and_then(|exit| u32::try_from(exit.statement_index).ok())
                });
                !retired_whole
                    && call_transfers == 0
                    && construction_transfers == 0
                    && reference_releases == 0
                    && view_releases == 0
                    && (deferred_reference_releases == 1
                        || local_results::permits_disposal(
                            program,
                            facts,
                            state,
                            operation,
                            operations,
                            statement,
                            continuation,
                            no_code_completion_owner,
                        ))
            };
            if result.multiplicity != Multiplicity::Unrestricted
                && successor_moves.iter().any(|(_, moves)| *moves > 1)
            {
                trace.phase("state graph: structural result has duplicate successor transfers");
                trace.statement(Some(result.statement_index));
                return None;
            }
            let transferred = |edge: &CheckedStructuralControlSuccessorPlan| {
                !release_owned_view
                    && successor_moves.iter().any(|(candidate, transfers)| {
                        std::ptr::eq(*candidate, edge)
                            && if result.multiplicity == Multiplicity::Unrestricted {
                                *transfers != 0
                            } else {
                                *transfers == 1
                            }
                    })
            };
            let stored_parts = operations
                .iter()
                .skip(producer_index + 1)
                .filter_map(|operation| {
                    operation.installed_value().filter(|value| {
                        value.access == StructuralAccess::Owned
                            && value.source_structural_result_binding_ordinal()
                                == Some(result.binding_ordinal)
                    })
                })
                .collect::<Vec<_>>();
            let stored = if result.multiplicity == Multiplicity::Unrestricted {
                !stored_parts.is_empty()
            } else {
                matches!(stored_parts.as_slice(), [whole]
                    if whole.path.is_empty() && whole.type_identity == result.type_identity)
            };
            let projected_disposition = if result.multiplicity == Multiplicity::Unrestricted {
                false
            } else {
                let mut paths = storage_transfers.as_ref()?.definite.clone();
                paths.sort();
                paths.dedup();
                paths.extend(consumers.iter().filter_map(|operation| {
                    let OperationPlan::ReleaseReference {
                        source,
                        path,
                        defer_to_exit: false,
                        ..
                    } = operation
                    else {
                        return None;
                    };
                    (source
                        == &(SourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        })
                        && !path.is_empty())
                    .then(|| path.clone())
                }));
                paths.sort();
                let mut remaining = paths.as_slice();
                !paths.is_empty()
                    && paths.iter().all(|path| !path.is_empty())
                    && producer_claim_paths(program, facts, operation).is_some_and(|claims| {
                        paths.iter().all(|path| {
                            claims
                                .iter()
                                .all(|claim| path.len() <= claim.len() || !path.starts_with(claim))
                        })
                    })
                    && disposed_partition(
                        shapes,
                        result.type_identity,
                        &mut Vec::new(),
                        &mut remaining,
                    )
                    && remaining.is_empty()
            };
            let return_arm_owns = |arm: &ReturnArm, uses: &[(usize, ArgumentPlan)]| {
                !arm.has_normal_continuation()
                    || arm.definite_call_transfers(
                        &facts.values,
                        call_schedules,
                        state.symbol,
                        operation,
                        terminal_local,
                    ) == 1
                    || (!matches!(arm, ReturnArm::Completion(_))
                        && uses.iter().any(|(_, argument)| {
                            construction_owns_result(argument, operations.len())
                        }))
                    || permits_disposal(arm.statement_ordinal(), false)
            };
            let has_normal_exit = successors
                .iter()
                .any(|edge| edge.has_normal_caller_completion())
                || return_arms.iter().any(|arm| arm.has_normal_continuation());
            let has_selected_disposition = has_normal_exit
                || (!release_owned_view && successor_moves.iter().any(|(_, moves)| *moves != 0));
            let mut closed_sum_all_reference_returns = false;
            let completion_owns = |transfers: usize, statement: u32, continuation: bool| {
                (result.multiplicity == Multiplicity::Unrestricted && transfers > 0)
                    || transfers == 1
                    || (transfers == 0 && permits_disposal(Some(statement), continuation))
            };
            let consumed = match terminator {
                TerminatorPlan::Guarded {
                    arms,
                    fallback,
                    return_values,
                } => {
                    let arms = facts.flow.guarded_exits.exits.span(*arms)?;
                    if arms.len() + usize::from(fallback.is_some()) != return_values.len() {
                        return None;
                    }
                    let mut prefix_transfers = 0;
                    let mut complete = !return_values.is_empty();
                    for (((guard, destination), value), uses) in arms
                        .iter()
                        .map(|arm| (Some(arm), &arm.destination))
                        .chain(fallback.iter().map(|destination| (None, destination)))
                        .zip(return_values)
                        .zip(&guarded_return_construction_uses)
                    {
                        if let Some(guard) = guard {
                            prefix_transfers += retained_guard(
                                facts,
                                machine.symbol,
                                state.symbol,
                                guard.guard_statement_ordinal,
                            )?
                            .definite_call_transfers(
                                &facts.values,
                                call_schedules,
                                operation,
                                terminal_local,
                            );
                        }
                        let checked_trees::CheckedScalarBranchDestination::Return {
                            statement_ordinal,
                            is_continuation,
                        } = destination
                        else {
                            return None;
                        };
                        let transfers = prefix_transfers
                            + value.operation.definite_call_transfers(
                                &facts.values,
                                call_schedules,
                                operation,
                                terminal_local,
                            )
                            + uses
                                .iter()
                                .filter(|(_, argument)| {
                                    construction_owns_result(argument, operations.len())
                                })
                                .count();
                        complete &=
                            completion_owns(transfers, *statement_ordinal, *is_continuation);
                    }
                    complete
                }
                TerminatorPlan::ReturnScalar {
                    completion: completion @ checked_trees::CheckedScalarReturnPlan::Exits(_),
                } => {
                    let exits = completion.selected_exit_call_transfers(
                        &facts.values,
                        call_schedules,
                        &facts.flow.guarded_exits,
                        state.symbol,
                        operation,
                        terminal_local,
                    )?;
                    !exits.is_empty()
                        && exits.iter().all(|(destination, transfers)| {
                            let checked_trees::CheckedScalarBranchDestination::Return {
                                statement_ordinal,
                                is_continuation,
                            } = destination
                            else {
                                return false;
                            };
                            completion_owns(*transfers, *statement_ordinal, *is_continuation)
                        })
                }
                TerminatorPlan::CompleteExpression { completion } => {
                    // A selection that cannot return supplies no normal disposal.
                    // Earlier transfers and releases keep their own counts.
                    completion.expression.normal_continuation
                        && (completion.definite_call_transfers(
                            &facts.values,
                            call_schedules,
                            operation,
                            terminal_local,
                        ) == 1
                            || permits_disposal(Some(completion.statement_ordinal), false))
                }
                TerminatorPlan::ReturnUnit | TerminatorPlan::ReturnScalar { .. } => {
                    permits_disposal(None, false)
                }
                TerminatorPlan::ReturnStructural { result: returned } => {
                    let returns_this = matches!(
                        returned.source,
                        SourcePlan::StructuralResult { binding_ordinal }
                            if binding_ordinal == result.binding_ordinal
                    );
                    let release_is_separate = reference_releases == 0 && !retired_whole;
                    if returns_this
                        && release_owned_view
                        && view_releases != 0
                        && release_is_separate
                    {
                        false
                    } else {
                        returns_this || permits_disposal(None, false)
                    }
                }
                TerminatorPlan::Jump { .. }
                | TerminatorPlan::Conditional { .. }
                | TerminatorPlan::ConditionalReturn { .. }
                | TerminatorPlan::ConditionalCompletion { .. }
                | TerminatorPlan::GuardedJumps { .. } => {
                    has_selected_disposition
                        && terminator.successors().iter().all(|edge| {
                            transferred(edge)
                                || !edge.has_normal_caller_completion()
                                || permits_disposal(
                                    Some(edge.statement_ordinal),
                                    edge.is_continuation,
                                )
                        })
                        && return_arms
                            .iter()
                            .zip(&return_construction_uses)
                            .all(|(arm, uses)| return_arm_owns(arm, uses))
                }
                TerminatorPlan::ClosedSum { cases, .. } => {
                    let selected_case_exhausted =
                        |case: &checked_trees::CheckedClosedSumCasePlan, arm: &ReturnArm| {
                            // A union of alternative moves cannot exhaust one selected case.
                            if matches!(arm, ReturnArm::Completion(_)) {
                                return false;
                            }
                            let no_nominal_cleanup = shapes
                                .types
                                .get(&result.type_identity)
                                .is_some_and(|declaration| !declaration.owns_linear_claim);
                            let selects =
                                |path: &[checked_trees::CheckedUnitStructuralPathSegment]| {
                                    matches!(
                                        path.first(),
                                        Some(checked_trees::CheckedUnitStructuralPathSegment::Case(identity))
                                            if *identity == case.case_identity
                                    )
                                };
                            let moved = arm
                                .operation_dependencies()
                                .flat_map(OperationPlan::with_value_calls)
                                .flat_map(OperationPlan::structural_operands)
                                .filter(|argument| {
                                    moves_occurrence(argument)
                                        && !argument.path.is_empty()
                                        && selects(&argument.path)
                                })
                                .map(|argument| argument.path.clone())
                                .collect::<Vec<_>>();
                            no_nominal_cleanup
                                && !moved.is_empty()
                                && producer_claim_paths(program, facts, operation).is_some_and(
                                    |claims| {
                                        claims.iter().filter(|claim| selects(claim)).all(|claim| {
                                            moved.iter().any(|path| claim.starts_with(path))
                                        })
                                    },
                                )
                        };
                    let mut return_uses = return_construction_uses.iter();
                    let complete_partition = !cases.is_empty()
                            && cases.iter().all(|case| match &case.outcome {
                                checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(
                                    successor,
                                ) => !successor.has_normal_caller_completion() || transferred(successor) || permits_disposal(Some(successor.statement_ordinal), successor.is_continuation),
                                checked_trees::CheckedClosedSumCaseOutcomePlan::Return(arm) => {
                                    let exact_reborrow = matches!(arm,
                                        ReturnArm::Structural(returned)
                                            if returned
                                                .established_reference_source(&facts.values.structural_values)
                                                .is_some_and(|source| exact_view_reborrow(source, operations.len())));
                                    return_uses.next().is_some_and(|uses| {
                                        exact_reborrow
                                            || return_arm_owns(arm, uses)
                                            || selected_case_exhausted(case, arm)
                                    })
                                }
                            });
                    if view_source.is_some() {
                        if !complete_partition && occurrence_end.is_none() && !stored {
                            let consumed_by_forming_call = !result_local.is_valid()
                                && operations.iter().any(|consumer| {
                                    consumer.structural_operands().iter().any(|argument| {
                                        argument.source_structural_result_binding_ordinal()
                                            == Some(result.binding_ordinal)
                                    })
                                });
                            if !consumed_by_forming_call {
                                trace.phase(
                                    "state graph: closed-sum exit has no exact view disposition",
                                );
                                trace.statement(Some(result.statement_index));
                                return None;
                            }
                        }
                        closed_sum_all_reference_returns = cases.iter().all(|case| {
                            let checked_trees::CheckedClosedSumCaseOutcomePlan::Return(
                                checked_trees::CheckedConditionalReturnArm::Structural(returned),
                            ) = &case.outcome
                            else {
                                return false;
                            };
                            returned
                                .established_reference_source(&facts.values.structural_values)
                                .is_some_and(|source| exact_view_reborrow(source, operations.len()))
                        });
                        has_normal_exit
                            && occurrence_end.is_none()
                            && !stored
                            && result.multiplicity == Multiplicity::Unrestricted
                    } else {
                        has_selected_disposition && complete_partition
                    }
                }
                TerminatorPlan::Crash { .. }
                | TerminatorPlan::InvokeNoNormal { .. }
                | TerminatorPlan::EvaluateNoNormal => false,
            } || selection_source_disposition;
            let copied_into_construction =
                result.multiplicity == Multiplicity::Unrestricted && construction_count != 0;
            let retirement_realizes_view_release =
                (retired_whole || reference_releases != 0) && has_captured_view_loans;
            if retirement_realizes_view_release && view_releases != 1 {
                trace.phase("state graph: view cleanup has no unique captured loan disposition");
                trace.statement(Some(result.statement_index));
                return None;
            }
            let independent_view_releases = if retirement_realizes_view_release {
                0
            } else {
                view_releases
            };
            let returning_view_disposal = (matches!(terminator, TerminatorPlan::Guarded { .. })
                || closed_sum_all_reference_returns)
                && view_releases == 1;
            let copied_into_byte_store = result.multiplicity == Multiplicity::Unrestricted
                && operations[producer_index + 1..].iter().any(|operation| {
                    operation.stored_bytes().is_some_and(|value| {
                        value.completed_call_binding_ordinal() == Some(result.binding_ordinal)
                    })
                });
            let observed_by_call = result.multiplicity == Multiplicity::Unrestricted
                && operations[producer_index + 1..]
                    .iter()
                    .flat_map(OperationPlan::structural_operands)
                    .any(|argument| {
                        argument.source_structural_result_binding_ordinal()
                            == Some(result.binding_ordinal)
                            && argument.path.is_empty()
                    });
            let replaced_unrestricted =
                result.multiplicity == Multiplicity::Unrestricted && occurrence_end.is_some();
            let implicit_disposition = consumed
                || stored
                || projected_disposition
                || copied_into_construction
                || copied_into_byte_store
                || observed_by_call
                || replaced_unrestricted;
            let implicit_disposition = implicit_disposition
                && !(result.multiplicity == Multiplicity::Unrestricted && retired_whole);
            let total = usize::from(implicit_disposition)
                + call_transfers
                + cleanup_discards
                + construction_transfers
                + reference_releases
                + independent_view_releases;
            if total != 1 && !(total == 0 && !has_normal_caller_completion) {
                if ::validation::trace_flags::struct_trace() {
                    eprintln!(
                        "result custody: machine={:?} state={:?} result={result:?} local={result_local:?} construction={construction_transfers} cleanup={cleanup_discards} implicit={implicit_disposition} consumed={consumed} stored={stored} projected={projected_disposition} retired={retired_whole} calls={call_transfers} release={reference_releases} views={independent_view_releases} operations={operations:#?}",
                        machine.symbol, state.symbol
                    );
                    for (_, establishment) in facts.contextual_cleanup.storage_establishments.iter()
                    {
                        if establishment.machine == machine.symbol
                            && establishment.state == state.symbol
                        {
                            eprintln!("result custody receiving storage: {establishment:?}");
                        }
                    }
                }
                trace.phase(if total == 0 {
                    "state graph: structural result has no selected disposition"
                } else if construction_transfers != 0 && reference_releases != 0 {
                    "state graph: structural result has duplicate dispositions (owned construction and physical reference release)"
                } else if construction_transfers != 0 && implicit_disposition {
                    "state graph: structural result has duplicate dispositions (owned construction and selected exit)"
                } else if construction_transfers != 0 && cleanup_discards != 0 {
                    "state graph: structural result has duplicate dispositions (owned construction and continuation cleanup)"
                } else if construction_transfers != 0 && call_transfers != 0 {
                    "state graph: structural result has duplicate dispositions (owned construction and call transfer)"
                } else if construction_transfers != 0 && independent_view_releases != 0 {
                    "state graph: structural result has duplicate dispositions (owned construction and captured loan release)"
                } else {
                    "state graph: structural result has duplicate dispositions"
                });
                trace.statement(Some(result.statement_index));
                trace.result_custody_failure(control::ResultCustodyFailure {
                    binding: result.binding_ordinal,
                    local: result_local,
                    producer_operation: producer_index,
                    dispositions: [
                        usize::from(implicit_disposition),
                        call_transfers,
                        cleanup_discards,
                        construction_transfers,
                        reference_releases,
                        independent_view_releases,
                    ],
                    normal_caller_completion: has_normal_caller_completion,
                });
                return None;
            }
            if returning_view_disposal {
                guarded_discards.push(producer_index);
            }
        }
        for producer_index in guarded_discards {
            let (_, discard_result_on_return) = operations
                .get_mut(producer_index)?
                .discardable_structural_result_mut()?;
            *discard_result_on_return = true;
        }
        Some(())
    }

    /*
        Use the actual producer's claim frontier; storage shape alone cannot tell
        whether a containing claim forbids moving an otherwise ordinary child.
    */
    fn producer_claim_paths(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        producer: &OperationPlan,
    ) -> Option<Vec<Vec<checked_trees::CheckedUnitStructuralPathSegment>>> {
        if let Some(call) = producer.call() {
            return Some(
                call.structural_custody()?
                    .result_claims
                    .iter()
                    .map(|claim| claim.path.clone())
                    .collect(),
            );
        }
        let OperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
        ) = producer
        else {
            return None;
        };
        if !facts.values.structural_values.nodes.is_valid(*value) {
            return None;
        }
        let actual_type = facts
            .values
            .structural_values
            .nodes
            .get(*value)
            .type_reference;
        validation::linear_claim_frontier(program, actual_type)
            .into_iter()
            .map(|claim| validation::structural_claim_path(program, actual_type, &claim.path).ok())
            .collect()
    }

    /*
        A completed disposition partition accounts for exactly one whole source value.
        Arrays own their elements, so all of an array's children can transfer its
        custody. Other values remain indivisible here: covering a record's fields
        does not discharge its own nominal cleanup. The caller separately preserves
        containing claims. These paths come from typed owned operands, not from
        receiver shape or source syntax. Consume their sorted guaranteed coverage,
        rejecting missing siblings and overlapping ancestor/descendant coverage.
    */
    fn disposed_partition(
        shapes: &super::ShapeCollector<'_>,
        identity: typed_trees::type_identity::TypeIdentityId,
        prefix: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        remaining: &mut &[Vec<checked_trees::CheckedUnitStructuralPathSegment>],
    ) -> bool {
        let Some(first) = remaining.first() else {
            return false;
        };
        if first == prefix {
            *remaining = &remaining[1..];
            return true;
        }
        let Some(checked_trees::CheckedUnitStructuralTypeShape::FixedArray {
            element_type_identity,
            length,
        }) = shapes.types.get(&identity).map(|plan| &plan.shape)
        else {
            return false;
        };
        let Some(length) = length.literal() else {
            return false;
        };
        if length == 0 || u64::try_from(remaining.len()).map_or(true, |count| count < length) {
            return false;
        }
        for index in 0..length {
            prefix.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                index,
            ));
            let complete = disposed_partition(shapes, *element_type_identity, prefix, remaining);
            prefix.pop();
            if !complete {
                return false;
            }
        }
        true
    }

    /*
        Statement coordinates can contain several independent formations: an
        anonymous input descriptor, a returned child, or another operand's view.
        Only the loans captured by this exact holder can dispose its result.
        A temporary descriptor with no source loan still needs its own physical
        cleanup; another holder's Consume cannot stand in for that disposition.
        Captured loan handles rejoin their original claims: equal backing paths can
        belong to different holders even within one statement.
    */
    fn view_loan_releases(
        facts: &CheckFacts,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        producer: &OperationPlan,
        installed_claims: &[language_semantics::PermissionClaimIdentity],
    ) -> Result<(bool, bool, usize), &'static str> {
        let owner = if result.local.symbol.is_valid() {
            facts::PlaceRoot::Symbol(result.local.symbol)
        } else {
            let OperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
            ) = producer
            else {
                return Err("state graph: view result has no captured formation");
            };
            let formation = facts.values.structural_values.nodes.get(*value).expression;
            if !formation.is_valid() {
                return Err("state graph: view result has an invalid captured formation");
            }
            facts::PlaceRoot::Expression(formation)
        };
        let rows = facts
            .borrow
            .with_state_borrows(machine, state, |rows| rows.to_vec());
        let borrowing = match rows.as_slice() {
            [] => return Ok((false, false, 0)),
            [row] => facts.borrow.states.get(*row),
            _ => return Err("state graph: view result has ambiguous borrow states"),
        };
        let loans = facts
            .borrow
            .storage_loans(borrowing, owner, result.statement_index as usize)
            .ok_or("state graph: view holder has no complete captured loan group")?;
        let captured_loans = facts
            .borrow
            .loans
            .span(loans)
            .ok_or("state graph: view holder has an invalid captured loan group")?;
        let mut claims = Vec::new();
        let mut inherited = 0usize;
        for (handle, loan) in facts
            .borrow
            .loan_group(loans)
            .ok_or("state graph: view holder has an invalid captured loan group")?
        {
            if !facts.borrow.loan_owner_path(loan).is_empty() {
                return Err("state graph: view root contains a projected loan claim");
            }
            let mut matching = facts
                .flow
                .ownership
                .borrow_claims
                .iter()
                .filter_map(|(_, claim)| (claim.loan == handle).then_some(claim.claim_identity));
            let claim = matching
                .next()
                .ok_or("state graph: view loan has no claim mapping")?;
            if matching.next().is_some() {
                return Err("state graph: view loan has duplicate claim mappings");
            }
            let claim @ language_semantics::PermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source,
                ..
            } = claim
            else {
                return Err("state graph: view loan has no established activation identity");
            };
            if machine_symbol != machine
                || state_symbol != state
                || source
                    != (PermissionEventSource::Statement {
                        statement_index: result.statement_index as usize,
                    })
            {
                inherited += 1;
                continue;
            }
            if claims.contains(&claim) {
                return Err("state graph: distinct view loans share one activation claim");
            }
            let mut establishments = permission_events(facts).filter_map(|(_, event)| {
                (event.machine_symbol == machine
                    && event.state_symbol == state
                    && event.claim_identity == claim
                    && event.kind == PermissionEventKind::Establish)
                    .then_some(event)
            });
            let establishment = establishments
                .next()
                .ok_or("state graph: view activation has no establishment")?;
            if establishments.next().is_some() {
                return Err("state graph: view activation has duplicate establishments");
            }
            if establishment.source != source
                || establishment.access != language_semantics::PermissionAccess::Exclusive
                || establishment.multiplicity != Multiplicity::Affine
                || establishment.evaluation != checked_trees::FlowPermissionEvaluation::Statement
            {
                return Err("state graph: view activation has invalid establishment evidence");
            }
            claims.push(claim);
        }
        if !claims.is_empty() && inherited != 0 {
            return Err("state graph: view activation roster mixes local and inherited claims");
        }
        let owns_local_claim = !claims.is_empty();
        let mut closure = None;
        for claim in claims
            .iter()
            .filter(|claim| !installed_claims.contains(claim))
        {
            let mut releases = permission_events(facts).filter_map(|(_, event)| {
                (event.machine_symbol == machine
                    && event.state_symbol == state
                    && event.claim_identity == *claim
                    && event.kind == PermissionEventKind::Consume)
                    .then_some(event)
            });
            let release = releases.next();
            if releases.next().is_some() {
                return Err("state graph: view activation has duplicate releases");
            }
            if let Some(release) = release {
                let after_formation = match release.source {
                    PermissionEventSource::StateExit
                    | PermissionEventSource::InstructionExitDisposition { .. } => true,
                    PermissionEventSource::Statement { statement_index }
                    | PermissionEventSource::InstructionDisposition { statement_index }
                    | PermissionEventSource::Call {
                        statement_index, ..
                    } => statement_index >= result.statement_index as usize,
                    PermissionEventSource::StateEntry => false,
                };
                if !after_formation
                    || release.access != language_semantics::PermissionAccess::Exclusive
                    || release.multiplicity != Multiplicity::Affine
                    || release.evaluation != checked_trees::FlowPermissionEvaluation::Statement
                {
                    return Err("state graph: view activation has invalid release evidence");
                }
            }
            let released = release.is_some();
            if closure.is_some_and(|previous| previous != released) {
                return Err("state graph: view activation roster has incomplete release coverage");
            }
            closure = Some(released);
        }
        Ok((
            !captured_loans.is_empty(),
            owns_local_claim,
            usize::from(closure == Some(true)),
        ))
    }

    /*
        A statement can form several unrelated descriptors. A permission release
        belongs to this result only when its exact formation expression rejoins the
        checked loan and that loan's establishment claim. Statement coincidence
        alone used to charge an anonymous argument for the returned child's release.
    */
    fn view_formation_claims(
        facts: &CheckFacts,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        producer: &OperationPlan,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
    ) -> Option<Vec<language_semantics::PermissionClaimIdentity>> {
        let OperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
        ) = producer
        else {
            return None;
        };
        let formation = facts.values.structural_values.nodes.get(*value).expression;
        if !formation.is_valid() {
            return None;
        }
        let borrowing = facts.borrow.unique_state_borrow(machine, state)?;
        let loans = facts
            .borrow
            .loan_group(borrowing.loans)?
            .filter(|(_, loan)| {
                loan.statement_index == result.statement_index as usize
                    && loan.formation_expression == formation
                    && loan.kind == checked_trees::BorrowAccessKind::Mutable
            })
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        Some(
            facts
                .flow
                .ownership
                .borrow_claims
                .iter()
                .filter(|(_, claim)| loans.contains(&claim.loan))
                .map(|(_, claim)| claim.claim_identity)
                .collect(),
        )
    }

    /*
        Installation transfers the completed descriptor, regardless of whether its
        receiving place is a binding, field or indexed slot. The source checker has
        already joined formation to exact loan claims; rejoining by backing or by
        an empty owner path confuses the producer's death with the resident loan's
        later release. Require one whole-value installation and one later release
        per exact claim. Destination access and replacement cleanup remain the
        store's obligations, not permission granted by this accounting join.
    */
    fn installed_view_claims(
        facts: &CheckFacts,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        producer: &OperationPlan,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        following: &[OperationPlan],
    ) -> Option<Vec<language_semantics::PermissionClaimIdentity>> {
        let mut installations = following.iter().filter_map(|operation| {
            let (type_identity, statement_index) = match operation {
                OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Structural {
                        result,
                        initial_value: Some(_),
                        ..
                    },
                ) => (result.type_identity, result.statement_index),
                OperationPlan::Store(store) => {
                    let (checked_trees::CheckedStoreLeaf::OwnedRebind { type_identity }
                    | checked_trees::CheckedStoreLeaf::StructuralStore {
                        type_identity, ..
                    }) = &store.destination.leaf
                    else {
                        return None;
                    };
                    (*type_identity, store.statement_index)
                }
                _ => return None,
            };
            let supplied = operation.installed_value()?;
            (type_identity == result.type_identity
                && supplied.type_identity == result.type_identity
                && supplied.source_structural_result_binding_ordinal()
                    == Some(result.binding_ordinal)
                && supplied.access == StructuralAccess::Owned
                && supplied.path.is_empty())
            .then_some(statement_index)
        });
        if installations.next()? != result.statement_index || installations.next().is_some() {
            return None;
        }
        let claims = view_formation_claims(facts, machine, state, producer, result)?;
        if claims.is_empty() {
            return None;
        }
        let source = PermissionEventSource::Statement {
            statement_index: result.statement_index as usize,
        };
        for claim in &claims {
            let matching = |event: &checked_trees::FlowPermissionEventFact| {
                event.claim_identity == *claim
                    && event.machine_symbol == machine
                    && event.state_symbol == state
                    && event.access == language_semantics::PermissionAccess::Exclusive
                    && event.multiplicity == Multiplicity::Affine
            };
            let mut establishments = permission_events(facts).filter_map(|(_, event)| {
                (matching(event) && event.kind == PermissionEventKind::Establish).then_some(event)
            });
            let establishment = establishments.next()?;
            if establishments.next().is_some()
                || establishment.source != source
                || establishment.evaluation != checked_trees::FlowPermissionEvaluation::Statement
            {
                return None;
            }
            let mut releases = permission_events(facts).filter_map(|(_, event)| {
                (matching(event) && event.kind == PermissionEventKind::Consume).then_some(event)
            });
            let release = releases.next()?;
            let after_installation = match release.source {
                PermissionEventSource::StateExit
                | PermissionEventSource::InstructionExitDisposition { .. } => true,
                PermissionEventSource::Statement { statement_index }
                | PermissionEventSource::InstructionDisposition { statement_index }
                | PermissionEventSource::Call {
                    statement_index, ..
                } => statement_index >= result.statement_index as usize,
                PermissionEventSource::StateEntry => false,
            };
            if releases.next().is_some()
                || !after_installation
                || release.evaluation != checked_trees::FlowPermissionEvaluation::Statement
            {
                return None;
            }
        }
        Some(claims)
    }

    /*
        Borrow whole local operands once per supplied construction sequence. The
        sequence mutation walker also enters dispatch/projection nodes to update
        disposal flags; accounting cannot flatten those per-arm/residual receipts.
        Content authority derivation retains its consumed parents on the same
        completed value. They are owned operands even when absent from its fields.
    */
    fn construction_local_uses<'operation>(
        facts: &CheckFacts,
        state: symbols::SymbolHandle,
        operations: impl IntoIterator<Item = &'operation OperationPlan>,
    ) -> Option<Vec<(usize, checked_trees::CheckedUnitStructuralArgumentPlan)>> {
        use checked_trees::{CheckedStructuralRecordFieldValue, CheckedStructuralValueKind};

        let values = &facts.values.structural_values;
        let mut uses = Vec::new();
        let mut pending = Vec::new();
        let mut visited = Vec::new();
        for (index, operation) in operations.into_iter().enumerate() {
            let OperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                    result, value, ..
                },
            ) = operation
            else {
                continue;
            };
            if facts
                .flow
                .ownership
                .owned_selection_at(state, result.statement_index)
                .is_some()
            {
                continue;
            }
            pending.push(*value);
            visited.clear();
            while let Some(value) = pending.pop() {
                if !values.nodes.is_valid(value) {
                    return None;
                }
                if visited.contains(&value) {
                    continue;
                }
                visited.push(value);
                let node = values.nodes.get(value);
                for parent in values.content_inputs.span_or_empty(node.content_inputs) {
                    uses.push((index, parent.clone()));
                }
                match &node.kind {
                    CheckedStructuralValueKind::Place(argument)
                    | CheckedStructuralValueKind::CopiedStructuralPlace {
                        source: argument, ..
                    } => {
                        uses.push((index, argument.clone()));
                    }
                    checked_trees::record_or_structural_case_pattern!(fields, ..) => {
                        for field in values.record_fields.span(*fields)? {
                            if let CheckedStructuralRecordFieldValue::Structural(value) =
                                field.value
                            {
                                pending.push(value);
                            }
                        }
                    }
                    CheckedStructuralValueKind::FixedArray { elements } => {
                        pending.extend(elements.iter().copied());
                    }
                    CheckedStructuralValueKind::Reference { source_value, .. }
                        if source_value.is_valid() =>
                    {
                        pending.push(*source_value);
                    }
                    CheckedStructuralValueKind::Projection { source, path, .. }
                        if path.is_empty() =>
                    {
                        pending.push(*source)
                    }
                    _ => {}
                }
            }
        }
        Some(uses)
    }

    /*
        A call retained inside a value-returning terminator (`Guarded` arm
        payloads, `ConditionalReturn`'s value arm) consumes its producers through
        owned whole arguments the same way an in-sequence call does: the argument
        names the exact result binding it moves into the call's custody.
    */
    pub(in super::super) fn terminator_call_consumes_result(
        operation: &OperationPlan,
        binding_ordinal: u32,
    ) -> bool {
        operation.structural_operands().iter().any(|argument| {
            argument.source_structural_result_binding_ordinal() == Some(binding_ordinal)
                && argument.access == StructuralAccess::Owned
                && argument.path.is_empty()
        })
    }

    /*
        Whole owned call arguments consume affine and linear results. Accounting
        requires one disposition, and lowering rejoins a moved local's exact
        establishment and call-transfer receipts instead of state-exit cleanup.
        An inline argument construction also belongs to its consuming call when
        copyable; other unrestricted locals retain their reusable statement owner.
    */
    pub(in super::super) fn call_owns_result(producer: &OperationPlan) -> bool {
        producer
            .structural_result_binding()
            .is_some_and(|result| result.multiplicity != Multiplicity::Unrestricted)
            || matches!(
                producer,
                OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                        operand_source: Some(
                            checked_trees::CheckedArrayConstructionSource::CallArgument { .. }
                        ),
                        ..
                    }
                )
            )
    }

    /*
        An available structural binding, independently of whether its producer
        carries disposal debt. An actual automatic return discard retires it.
    */
    pub(in super::super) fn retained_structural_result(
        operation: &OperationPlan,
    ) -> Option<&checked_trees::CheckedUnitStructuralResultBindingPlan> {
        if operation
            .discardable_structural_result()
            .is_some_and(|(_, discard)| discard)
        {
            return None;
        }
        operation.structural_result_binding()
    }

    /*
        An operand-owned construction retained beside the call that consumes it:
        `operand_source` names the consuming call's coordinate and formal
        position, so its statement is the call's — any local declared on that
        statement is a sibling, never this construction's binding. Such rows must
        not stand in for a local's unique producer.
    */
    pub(in super::super) fn operand_owned_construction(operation: &OperationPlan) -> bool {
        matches!(
            operation,
            OperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                    operand_source: Some(_),
                    ..
                }
            )
        )
    }
}
pub(crate) use ranking::natural_rank_plans;
use ranking::natural_rank_plans_for_fused;
pub(super) mod returns {

    /*
        Normal graph results through the shared structural value producer. A
        primitive scalar result completes each returning state with the same
        binding or exit roster an ordinary single-state body retains.
    */
    use super::super::CheckedUnitEntryClaimPlan;
    use super::super::calls::structural_result_owner;
    use super::super::types::state_flow;
    use super::fused::FusedStates;
    use super::fused::fused_target_index;
    use super::{
        CheckFacts, CheckedComposedUnitControlTerminatorPlan,
        CheckedUnitEffectOperationPlan as OperationPlan, CheckedUnitStructuralParameterPlan,
        Multiplicity, StatementNode, control,
    };
    use crate::execution::terminal_unit::ScalarCalleeClosure;
    use crate::execution::terminal_unit::types::{
        ShapeCollector, is_unit, parameter_qualifications,
    };

    /*
        How a returning state of a scalar-result graph completes its value: the
        same two completions an ordinary single-state body retains. A final
        expression is the sequence's own returned binding; value-only transition
        exits are the shared scalar exit roster, which must begin exactly at the
        state's terminator. A crash keeps the graph's own `Crash` terminator, a
        named successor keeps its edge custody plan, and a closing terminal machine
        call completes with every other result kind (`terminal_calls::completion`),
        so none of them completes here.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn scalar_completion(
        program: &dyn ::validation::CheckingInput,
        fused: &FusedStates<'_>,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        terminator_index: usize,
        scalar_type: checked_trees::CheckedScalarType,
        returned_binding: Option<&checked_trees::CheckedUnitScalarResultBindingPlan>,
        trace: &control::LocalConstructionTrace,
    ) -> Option<checked_trees::CheckedScalarReturnPlan> {
        trace.phase("state graph: terminator: scalar completion");
        let statements = program.statement_table.statements(state.statement_nodes);
        if statements[terminator_index..]
            .iter()
            .any(|statement| match statement {
                StatementNode::Transition(transition) => transition.targets().any(|target| {
                    validation::completing_transition_transfer(program, target).is_some_and(
                        |transfer| fused_target_index(program, fused, transfer.target).is_some(),
                    )
                }),
                _ => false,
            })
        {
            return None;
        }
        if let Some(binding) = returned_binding {
            return (terminator_index.checked_add(1)? == statements.len()
                && control::completion_expression(program, machine, state, terminator_index)
                    .is_some()
                && usize::try_from(binding.statement_index).ok()? == terminator_index
                && binding.scalar_type == scalar_type)
                .then_some(checked_trees::CheckedScalarReturnPlan::Binding(*binding));
        }
        let (exits, prefix) =
            control::statement_sequence::scalar_control(program, facts, machine, state)?;
        if prefix != terminator_index
            || exits.scalar_type != scalar_type
            || matches!(
                exits.terminator,
                checked_trees::CheckedScalarStateTerminator::Crash { .. }
            )
        {
            return None;
        }
        Some(checked_trees::CheckedScalarReturnPlan::Exits(exits))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn guarded(
        program: &dyn ::validation::CheckingInput,
        fused: &FusedStates<'_>,
        facts: &CheckFacts,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[CheckedUnitStructuralParameterPlan],
        claims: &[CheckedUnitEntryClaimPlan],
        operations: &[OperationPlan],
        start: usize,
        trace: &control::LocalConstructionTrace,
    ) -> Option<(CheckedComposedUnitControlTerminatorPlan, Vec<OperationPlan>)> {
        if is_unit(program, state.return_type)
            || program.type_multiplicity(state.return_type) == Multiplicity::Linear
        {
            return None;
        }
        let mut tails = facts
            .flow
            .guarded_exits
            .tails
            .iter()
            .filter(|tail| tail.state == state.symbol);
        let tail = tails.next()?;
        if tails.next().is_some() {
            return None;
        }
        let arms = facts.flow.guarded_exits.exits.span(tail.arms)?;
        if arms.first()?.guard_statement_ordinal as usize != start {
            return None;
        }
        let selected_copies =
            control::selected_copies::SelectedCopies::new(program, facts, machine.symbol, state)?;
        let mut count = next_result_ordinal(operations)?;
        let mut return_values = Vec::new();
        let mut preamble = Vec::new();
        for destination in arms
            .iter()
            .map(|arm| &arm.destination)
            .chain(tail.fallback.iter())
        {
            let checked_trees::CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } = destination
            else {
                return None;
            };
            let (expression, copies) = selected_copies.for_destination(destination)?;
            if validation::completing_call_transfer(program, expression).is_some_and(|transfer| {
                fused_target_index(program, fused, transfer.target).is_some()
            }) {
                return None;
            }
            let (operation, operand_calls) = return_value_operation(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                state,
                parameters,
                claims,
                operations,
                &mut count,
                *statement_ordinal,
                expression,
                trace,
            )?;
            return_values.push(checked_trees::CheckedGuardedReturnValue {
                statement_ordinal: *statement_ordinal,
                is_continuation: *is_continuation,
                transition_target: copies.transition_target,
                operation,
                copied_payloads: copies.copied_payloads,
            });
            preamble.extend(operand_calls);
        }
        Some((
            CheckedComposedUnitControlTerminatorPlan::Guarded {
                arms: tail.arms,
                fallback: tail.fallback.clone(),
                return_values,
            },
            preamble,
        ))
    }

    /* First free structural binding ordinal after every producer in `operations`. Terminator-side return values number into the same roster. */
    pub(super) fn next_result_ordinal(operations: &[OperationPlan]) -> Option<usize> {
        operations
            .iter()
            .flat_map(OperationPlan::with_value_calls)
            .filter_map(OperationPlan::structural_result_binding)
            .map(|result| result.binding_ordinal)
            .max()
            .map(|ordinal| usize::try_from(ordinal).ok()?.checked_add(1))
            .unwrap_or(Some(0))
    }

    /*
        The `StructuralValue` local-binding producer for one authored `(expression)`
        return target: its structural root must already be retained by the value
        fact for this exact statement, carry this machine's result type, and lower
        through the same call decomposition the guarded-return roster uses.
        Preceding operations supply the established result namespace, not a second
        execution of that prefix. The companion operation list is empty: nested
        calls and their preparation stay in the returned value's operand schedule,
        executed inside the selected arm, never in the state's common preamble.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn return_value_operation(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[CheckedUnitStructuralParameterPlan],
        claims: &[CheckedUnitEntryClaimPlan],
        preamble: &[OperationPlan],
        count: &mut usize,
        statement_ordinal: u32,
        expression: typed_trees::expression::ExpressionHandle,
        trace: &control::LocalConstructionTrace,
    ) -> Option<(OperationPlan, Vec<OperationPlan>)> {
        if let Some(operation) = literal_result_operation(
            program,
            shapes,
            state,
            &crate::execution::terminal_unit::types::machine_binders(program, machine),
            count,
            statement_ordinal,
            expression,
        ) {
            return Some((operation, Vec::new()));
        }
        trace.phase("state graph: structural return value: retained expression root");
        let root = facts.values.structural_values.root_for_expression(
            state.symbol,
            statement_ordinal,
            expression,
        )?;
        trace.phase("state graph: structural return value: exact result type");
        if root.machine != machine.symbol || root.type_reference != state.return_type {
            return None;
        }
        trace.phase("state graph: structural return value: operand calls");
        let flow = state_flow(facts, machine.symbol, state.symbol)?;
        let source_calls = facts.flow.control.calls.span(flow.calls)?;
        let results = preamble
            .iter()
            .filter_map(|operation| {
                let result = operation.structural_result_binding()?;
                let owner =
                    structural_result_owner(program, facts, state.symbol, source_calls, operation)?;
                Some((result.clone(), owner))
            })
            .collect::<Vec<_>>();
        let mut operand_calls = Vec::new();
        let (calls, completion) = control::structural_operands::value_calls(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            parameters,
            &[],
            claims,
            &results,
            preamble,
            count,
            root.root,
            trace,
            &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
        )?;
        if completion == checked_trees::CheckedValueCompletion::NoNormal {
            return Some((
                OperationPlan::EvaluateStructuralValue {
                    statement_index: statement_ordinal,
                    value: root.root,
                    calls,
                    operand_source: None,
                },
                operand_calls,
            ));
        }
        trace.phase("state graph: structural return value: returned type");
        trace.statement(Some(statement_ordinal));
        let result = control::checked_structural_result_type(
            program,
            shapes,
            state.return_type,
            &crate::execution::terminal_unit::types::machine_binders(program, machine),
        );
        if result.is_none() && ::validation::trace_flags::trace_completion() {
            eprintln!(
                "CHECKSITE completion-return-type machine={} state={} type={:?} declaration={:?} stored_loan={:?} slice={:?}",
                machine.name,
                state.name,
                state.return_type,
                program
                    .type_reference_table
                    .type_reference(state.return_type),
                validation::reference_result_custody::stored_loan_parts(program, state.return_type),
                validation::slice_view_shell(program, state.return_type, &[]),
            );
        }
        let mut result = result?;
        result.statement_index = statement_ordinal;
        result.binding_ordinal = u32::try_from(*count).ok()?;
        *count = count.checked_add(1)?;
        trace.phase("state graph: structural return value: producer");
        trace.statement(Some(statement_ordinal));
        let operation = control::structural_operands::establish_value(
            facts,
            result,
            root.root,
            calls,
            None,
            false,
            &mut operand_calls,
        );
        if operation.is_none() && ::validation::trace_flags::trace_completion() {
            eprintln!(
                "CHECKSITE completion-return-producer machine={} state={} root={:?} value={:?}",
                machine.name,
                state.name,
                root.root,
                facts.values.structural_values.nodes.get(root.root),
            );
        }
        let operation = operation?;
        trace.phase("state graph: structural return value: finished");
        trace.statement(Some(statement_ordinal));
        Some((operation, operand_calls))
    }

    /*
        Materialize exact authored literal octets into fresh bounded ownership,
        or lend a literal's immutable image storage as a borrowed byte view. The
        result ordinal shares the ordinary producer roster; only the selected
        return arm executes this operation. A body's final expression and a
        returning state's value arm complete a literal through this one producer.
    */
    pub(in crate::execution::terminal_unit) fn literal_result_operation(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        state: &typed_trees::state::State,
        binders: &[(symbols::SymbolHandle, String)],
        count: &mut usize,
        statement_ordinal: u32,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<OperationPlan> {
        let bytes = match control::statement_sequence::bounded_byte_values::expression_octets(
            program,
            state.return_type,
            expression,
        ) {
            Some(bytes) => bytes,
            None if crate::execution::terminal_unit::types::borrowed_slice_view(
                program,
                state.return_type,
            ) && validation::field_domains::string_literal_grants_view_domains(
                program,
                state.return_type,
                expression,
            ) =>
            {
                let literal = program.expression_table.string_node(expression)?;
                literal.to_vec()
            }
            None => return None,
        };
        let mut result =
            control::checked_structural_result_type(program, shapes, state.return_type, binders)?;
        result.statement_index = statement_ordinal;
        result.binding_ordinal = u32::try_from(*count).ok()?;
        let qualifications = parameter_qualifications(program, shapes, state.return_type, binders)?;
        *count = count.checked_add(1)?;
        Some(OperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
                result,
                value: checked_trees::CheckedByteSequenceValuePlan::literal(bytes),
                qualifications,
            },
        ))
    }
}
mod successors {
    /*
        Successor-edge construction for one composed state.

        `successor` (ordinary edges) and `successor_bindings` (the shared operand
        planner) build each `CheckedStructuralControlSuccessorPlan`: the exact
        parameter/local result transfers, projected loans and case payloads the
        edge carries, under the same argument custody an in-sequence call takes.
        The terminator and closed-sum modules call these through `super::`.
    */
    use super::super::calls;
    use super::Signature;
    use super::fused::{FusedStates, fused_target_index};
    use super::result_custody::{operand_owned_construction, retained_structural_result};
    use super::{
        CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
        CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
        CheckedStructuralScalarArgumentPlan, CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, ExpressionNode,
        Multiplicity, TransitionExit, TransitionGuardNode, TypeReferenceNode,
    };
    use crate::execution::terminal_unit::ScalarCalleeClosure;
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::control;
    use crate::execution::terminal_unit::types::ShapeCollector;
    use crate::published::places::place_at;
    use checked_trees::expression::ExpressionHandle;

    /*
        The terminator family an edge is built for. The trace names the guard
        that declined the edge beneath that family's phase; the edge's statement
        ordinal separates the true and false successors of one conditional.
    */
    #[derive(Clone, Copy)]
    pub(in crate::execution::terminal_unit) enum SuccessorEdge {
        Jump,
        Conditional,
        GuardedJump,
        ClosedCase,
    }

    #[derive(Clone, Copy)]
    pub(in crate::execution::terminal_unit) enum SuccessorGuard {
        TransitionForm,
        TargetState,
        ArgumentCount,
        StructuralArgument,
        SubsliceTransfer,
        LiteralTransfer,
        ResultTransfer,
        CasePayloadTransfer,
        EstablishedArgument,
        ProjectedValue,
        LoanTransfer,
        ParameterTransfer,
        ScalarArguments,
        ErasedArguments,
        ErasedProofArguments,
        EdgeCleanup,
    }

    impl SuccessorEdge {
        /*
            The trace phase naming `guard` on this successor edge. Every phase is
            `state graph: terminator: <edge>: <guard>`, spelled at compile time.
        */
        pub(in crate::execution::terminal_unit) fn phase(
            self,
            guard: SuccessorGuard,
        ) -> &'static str {
            macro_rules! guard_phase {
                ($edge:literal, $guard:literal) => {
                    concat!("state graph: terminator: ", $edge, ": ", $guard)
                };
                ($edge:literal) => {
                    match guard {
                        SuccessorGuard::TransitionForm => guard_phase!($edge, "transition form"),
                        SuccessorGuard::TargetState => guard_phase!($edge, "target state"),
                        SuccessorGuard::ArgumentCount => guard_phase!($edge, "argument count"),
                        SuccessorGuard::StructuralArgument => {
                            guard_phase!($edge, "structural argument")
                        }
                        SuccessorGuard::SubsliceTransfer => {
                            guard_phase!($edge, "byte-subslice transfer")
                        }
                        SuccessorGuard::LiteralTransfer => guard_phase!($edge, "literal transfer"),
                        SuccessorGuard::ResultTransfer => {
                            guard_phase!($edge, "result-local transfer")
                        }
                        SuccessorGuard::CasePayloadTransfer => {
                            guard_phase!($edge, "case-payload transfer")
                        }
                        SuccessorGuard::EstablishedArgument => {
                            guard_phase!($edge, "established argument")
                        }
                        SuccessorGuard::ProjectedValue => guard_phase!($edge, "projected value"),
                        SuccessorGuard::LoanTransfer => guard_phase!($edge, "loan transfer"),
                        SuccessorGuard::ParameterTransfer => {
                            guard_phase!($edge, "parameter transfer")
                        }
                        SuccessorGuard::ScalarArguments => guard_phase!($edge, "scalar arguments"),
                        SuccessorGuard::ErasedArguments => guard_phase!($edge, "erased arguments"),
                        SuccessorGuard::ErasedProofArguments => {
                            guard_phase!($edge, "erased proof arguments")
                        }
                        SuccessorGuard::EdgeCleanup => guard_phase!($edge, "edge cleanup"),
                    }
                };
            }
            match self {
                Self::Jump => guard_phase!("jump successor"),
                Self::Conditional => guard_phase!("conditional successors"),
                Self::GuardedJump => guard_phase!("guarded jump successors"),
                Self::ClosedCase => guard_phase!("closed-sum case successor"),
            }
        }
    }

    /*
        The callee catalog, source-state entry claims and source-state local
        results an edge plans its own operand calls against. A composite
        argument's call runs only on the selected edge, under the same argument
        custody an in-sequence call takes: an operand naming one of the state's
        `let` locals (or an anonymous result it established) resolves to that
        result binding exactly as a later statement's call would.
    */
    #[derive(Clone, Copy)]
    pub(in crate::execution::terminal_unit) struct EdgeCalls<'a> {
        pub(super) scalar_callees: ScalarCalleeClosure,
        pub(super) entry_claims: &'a [checked_trees::CheckedUnitEntryClaimPlan],
        pub(super) local_results: &'a [(
            checked_trees::CheckedUnitStructuralResultBindingPlan,
            facts::PlaceRoot,
        )],
    }

    pub(in crate::execution::terminal_unit) fn successor(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        declarations: &super::loan_backing::DeclaredStoragePlans<'_>,
        records: &super::loan_backing::RetainedBorrowRecords<'_>,
        fused: &FusedStates<'_>,
        source_index: usize,
        signatures: &[Signature],
        operations: &[CheckedUnitEffectOperationPlan],
        transition: &typed_trees::statement::TableTransition,
        ordinal: u32,
        edge: SuccessorEdge,
        edge_calls: EdgeCalls<'_>,
        shapes: &mut ShapeCollector<'_>,
        trace: &control::LocalConstructionTrace,
    ) -> Option<CheckedStructuralControlSuccessorPlan> {
        let successor = successor_bindings(
            program,
            facts,
            declarations,
            records,
            fused,
            source_index,
            signatures,
            operations,
            transition,
            ordinal,
            &[],
            edge,
            edge_calls,
            shapes,
            trace,
        )?;
        if !successor.has_normal_continuation() {
            return Some(successor);
        }
        trace.phase(edge.phase(SuccessorGuard::EdgeCleanup));
        trace.statement(Some(ordinal));
        let &(machine, source) = fused.get(source_index)?;
        let cleanups = &facts.flow.terminal_structural_control_cleanups;
        let mut successor = successor;
        if let Some(cleanup) = cleanups.for_edge(machine.symbol, source.symbol, ordinal) {
            successor.trivial_affine_discard_parameter_positions =
                cleanup.trivial_affine_discard_parameter_positions.clone();
        }
        Some(successor)
    }

    /*
        Operand identity is shared by ordinary and closed-case edges. The caller
        separately admits either whole-parameter cleanup or exact result-local
        cleanup; constructing bindings establishes neither cleanup contract.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::execution::terminal_unit) fn successor_bindings(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        declarations: &super::loan_backing::DeclaredStoragePlans<'_>,
        records: &super::loan_backing::RetainedBorrowRecords<'_>,
        fused: &FusedStates<'_>,
        source_index: usize,
        signatures: &[Signature],
        operations: &[CheckedUnitEffectOperationPlan],
        transition: &typed_trees::statement::TableTransition,
        ordinal: u32,
        payload_parameters: &[u32],
        edge: SuccessorEdge,
        edge_calls: EdgeCalls<'_>,
        shapes: &mut ShapeCollector<'_>,
        trace: &control::LocalConstructionTrace,
    ) -> Option<CheckedStructuralControlSuccessorPlan> {
        let mark = |guard| {
            trace.phase(edge.phase(guard));
            trace.statement(Some(ordinal));
        };
        mark(SuccessorGuard::TransitionForm);
        if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
            return None;
        }
        mark(SuccessorGuard::TargetState);
        let arrival = validation::completing_transition_transfer(program, transition.target)?;
        let target_index = fused_target_index(program, fused, arrival.target)?;
        let &(machine, source) = fused.get(source_index)?;
        let &(_, target) = fused.get(target_index)?;
        let arguments = arrival.arguments;
        let target_parameters = program.state_parameters(target);
        mark(SuccessorGuard::ArgumentCount);
        if arguments.len() != target_parameters.len() {
            return None;
        }
        let (source_structural, source_scalar) = &signatures[source_index];
        let (target_structural, target_scalar) = &signatures[target_index];
        let case_test = match transition.guard {
            TransitionGuardNode::When(guard) => crate::published::proof::exact_outcome_case_test(
                program, guard,
            )
            .and_then(|(subject, case)| {
                place_at(program, source.symbol, ordinal as usize, subject)
                    .map(|place| (place, case))
            }),
            _ => None,
        };
        let argument_at = |position: u32| arguments.get(position as usize).copied();
        let source_position = |position: u32| {
            let mut argument = argument_at(position)?;
            while let ExpressionNode::Cast(cast) = program.expression_table.expression(argument)
                && program.primitive_type_reference(cast.target_type).is_none()
            {
                argument = cast.value;
            }
            let place = place_at(program, source.symbol, ordinal as usize, argument)?;
            let symbol = place.root.as_symbol()?;
            if !place.segments.is_empty() {
                return None;
            }
            typed_trees::signature::state_parameter_position(
                program.state_parameters(source),
                symbol,
            )
        };
        let mut transfers = target_structural
            .iter()
            .enumerate()
            .scan(false, |stopped, (target_index, target)| {
                if *stopped { return None; }
                let transfer = (|| {
                mark(SuccessorGuard::StructuralArgument);
                let expression = argument_at(target.position)?;
                if let Some((source_index, source_parameter)) = source_position(target.position)
                    .and_then(|position| source_structural.iter().enumerate()
                        .find(|(_, parameter)| parameter.position as usize == position))
                    && source_parameter.type_identity == target.type_identity
                    && source_parameter.access == target.access
                    && (target.access != CheckedStructuralAccess::Owned
                        || program.state_parameters(source).get(source_parameter.position as usize)
                            .zip(target_parameters.get(target.position as usize))
                            .is_some_and(|(source, target)| {
                                program.normalized_type_identity(source.type_reference)
                                    == program.normalized_type_identity(target.type_reference)
                            }))
                {
                    mark(SuccessorGuard::ParameterTransfer);
                    return structural_transfer_plan(
                        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                            index: u32::try_from(source_index).ok()?,
                        },
                        target_index,
                    );
                }
                if let Some(range_expression) = validation::subslice_expression(program, expression)
                    && let ExpressionNode::Indexed(indexed) = program.expression_table.expression(range_expression)
                    && matches!(program.expression_table.expression(indexed.index), ExpressionNode::Range(_))
                {
                    mark(SuccessorGuard::SubsliceTransfer);
                    let target_parameter = target_parameters.get(target.position as usize)?;
                    let subslice = calls::view_subslice::admit(
                        program, facts, machine, source, source_structural,
                        target_parameter.type_reference, expression, ordinal as usize,
                        checked_trees::CheckedSubsliceSite::TransitionArgument {
                            argument_ordinal: target.position,
                        },
                    )?;
                    if subslice.range.type_identity != target.type_identity {
                        return None;
                    }
                    return structural_transfer_plan(subslice.transfer(), target_index);
                }
                if matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::String(_)
                ) {
                    mark(SuccessorGuard::LiteralTransfer);
                    let target_parameter = target_parameters.get(target.position as usize)?;
                    let literal = calls::byte_sequence_literal_argument(
                        program,
                        target_parameter.type_reference,
                        expression,
                    )?;
                    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } =
                        literal.source
                    else {
                        return None;
                    };
                    if target.access != CheckedStructuralAccess::SharedBorrow
                        || target.multiplicity != Multiplicity::Unrestricted
                        || literal.access != CheckedStructuralAccess::SharedBorrow
                        || literal.type_identity != target.type_identity
                    {
                        return None;
                    }
                    let source =
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral {
                            bytes,
                        };
                    return structural_transfer_plan(source, target_index);
                }
                let shared_view = target.access == CheckedStructuralAccess::SharedBorrow
                    && target_parameters.get(target.position as usize).is_some_and(|parameter| {
                        calls::view_subslice::view_kind(program, parameter.type_reference).is_some()
                    });
                let mutable_view = target.access == CheckedStructuralAccess::MutableBorrow
                    && target_parameters.get(target.position as usize).is_some_and(|parameter| {
                        validation::slice_view_shell(program, parameter.type_reference, &[])
                            .is_some_and(|view| view.access == language_core::ReferenceAccess::Mutable)
                    });
                if (target.access == CheckedStructuralAccess::Owned || shared_view || mutable_view)
                    && let Some(place) = place_at(program, source.symbol, ordinal as usize, expression)
                    && (target.access == CheckedStructuralAccess::Owned
                        || matches!(place.root, facts::PlaceRoot::Symbol(symbol)
                            if program.state_local_declaration(source, symbol)
                                .is_some_and(|(_, local)| validation::slice_view_shell(
                                    program, local.type_reference, &[],
                                ).is_some())))
                {
                    mark(SuccessorGuard::ResultTransfer);
                    if place.segments.is_empty()
                        && let Some(symbol) = place.root.as_symbol()
                        && let Some(binding_ordinal) =
                            super::super::local_storage::occupant(program, source, operations, symbol)
                        && (target.access != CheckedStructuralAccess::Owned
                            || program.state_local_declaration(source, symbol)
                                .zip(target_parameters.get(target.position as usize))
                                .is_some_and(|((_, local), target)| {
                                    program.normalized_type_identity(local.type_reference)
                                        == program.normalized_type_identity(target.type_reference)
                                }))
                    {
                        let mut matches = operations.iter()
                            .filter(|operation| !operand_owned_construction(operation))
                            .filter_map(retained_structural_result)
                            .filter(|result| result.binding_ordinal == binding_ordinal && result.statement_index < ordinal);
                        if let Some(result) = matches.next() {
                            if matches.next().is_some() { return None; }
                            let presents_target = result.type_identity == target.type_identity
                                || (target.access != CheckedStructuralAccess::Owned
                                    && program.state_local_declaration(source, symbol).is_some_and(|(_, local)| {
                                        super::super::types::stored_type_presents_borrowed(
                                            program, local.type_reference, result.type_identity, target.type_identity,
                                        )
                                    }));
                            if !presents_target
                                || (!mutable_view && result.multiplicity != target.multiplicity) { return None; }
                            if mutable_view {
                                let local = program.statement_table.statements(source.statement_nodes).get(result.statement_index as usize)?.as_local_data()?;
                                if local.is_mutable
                                    || result.multiplicity != Multiplicity::Affine
                                    || !validation::slice_view_shell(program, local.type_reference, &[])
                                        .is_some_and(|view| view.access == language_core::ReferenceAccess::Mutable)
                                { return None; }
                            }
                            return structural_transfer_plan(
                                checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
                                    binding_ordinal: result.binding_ordinal,
                                },
                                target_index,
                            );
                        }
                    }
                }
                if let Some((subject_place, selected_case)) = &case_test
                    && let Some(place) = place_at(program, source.symbol, ordinal as usize, expression)
                {
                    mark(SuccessorGuard::CasePayloadTransfer);
                    if let Some(source) = case_payload_transfer(
                        program,
                        place,
                        subject_place,
                        *selected_case,
                        source,
                        source_structural,
                        operations,
                        ordinal,
                        target,
                    ) {
                        return structural_transfer_plan(source, target_index);
                    }
                }
                mark(SuccessorGuard::ProjectedValue);
                if let Some(subject) = projected_value_subject(
                        program,
                        machine,
                        source,
                        source_structural,
                        edge_calls.local_results,
                        ordinal,
                        expression,
                        target,
                    )
                    && subject.access == target.access
                {
                    return structural_transfer_plan(
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedValue { subject },
                        target_index,
                    );
                }
                let evaluated_argument = control::structural_operands::evaluated_argument_root(
                    program,
                    facts,
                    machine.symbol,
                    source.symbol,
                    ordinal as usize,
                    expression,
                    target_parameters.get(target.position as usize)?,
                )
                .filter(|root| {
                    target.access == CheckedStructuralAccess::Owned
                        || validation::slice_view_shell(program, root.type_reference, &[]).is_some()
                        || !matches!(
                            facts.values.structural_values.nodes.get(root.root).kind,
                            checked_trees::CheckedStructuralValueKind::Projection {
                                copies_selected_value: false,
                                ..
                            }
                        )
                });
                mark(SuccessorGuard::LoanTransfer);
                if evaluated_argument.is_none()
                    && matches!(target.access,
                        CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
                            | CheckedStructuralAccess::WriteOnlyBorrow)
                    && validation::checked_argument_matches_type_reference(
                        program,
                        expression,
                        target_parameters.get(target.position as usize)?.type_reference,
                    )
                {
                    let expected_access = target.access;
                    if let Some(place) = place_at(program, source.symbol, ordinal as usize, expression)
                        && let Some(root_symbol) = place.root.as_symbol()
                    {
                        let parameter_subject = program.state_by_symbol(source.symbol)
                        .and_then(|typed_state| {
                            program
                                .state_parameters(typed_state)
                                .iter()
                                .enumerate()
                                .find(|(_, parameter)| {
                                    super::super::types::parameter_is_root(parameter, root_symbol)
                                })
                                .and_then(|(position, _)| {
                                    source_structural
                                        .iter()
                                        .enumerate()
                                        .find(|(_, candidate)| {
                                            candidate.position as usize == position
                                        })
                                })
                        });
                        let result_subject = |local_index: usize| {
                            let mut matches = operations.iter().filter(|operation| {
                                !operand_owned_construction(operation)
                            }).filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
                            .filter(|result| {
                                result.statement_index == local_index as u32
                            });
                            let result = matches.next()?;
                            if matches.next().is_some() {
                                return None;
                            }
                            Some(result)
                        };
                        let local_subject = || {
                            let (index, local) =
                                program.state_local_declaration(source, root_symbol)?;
                            super::super::local_storage::source(program, source, operations, root_symbol)?;
                            Some((local, result_subject(index)?))
                        };
                        let loan_permitted = |root_access| match expected_access {
                            CheckedStructuralAccess::SharedBorrow => matches!(
                                root_access,
                                CheckedStructuralAccess::Owned
                                    | checked_trees::checked_borrowed_access_pair_pattern!()),
                            CheckedStructuralAccess::MutableBorrow => matches!(
                                root_access,
                                CheckedStructuralAccess::Owned | CheckedStructuralAccess::MutableBorrow
                            ),
                            CheckedStructuralAccess::WriteOnlyBorrow => matches!(
                                root_access,
                                CheckedStructuralAccess::Owned
                                    | CheckedStructuralAccess::MutableBorrow
                                    | CheckedStructuralAccess::WriteOnlyBorrow
                            ),
                            CheckedStructuralAccess::Owned => false,
                        };
                        if let Some((source_index, source_parameter)) = parameter_subject
                        {
                            if loan_permitted(source_parameter.access)
                                && let Some(transfer) = projected_loan_transfer(
                                    program,
                                    facts,
                                    shapes,
                                    machine.symbol,
                                    source.symbol,
                                    ordinal,
                                    expression,
                                    &place,
                                    CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                        parameter_index: u32::try_from(source_index).ok()?,
                                    },
                                    expected_access,
                                    target,
                                    target_parameters,
                                    target_index,
                                )
                            {
                                return Some(transfer);
                            }
                        } else if let Some((local, result)) = local_subject() {
                            if calls::is_stored_reference(program, local.type_reference) {
                                let mut subject = calls::retained_reference_argument(
                                    program,
                                    machine.symbol,
                                    source,
                                    ordinal as usize,
                                    &place,
                                    facts::PlaceRoot::Symbol(local.symbol),
                                    local.type_reference,
                                    result,
                                    target_parameters.get(target.position as usize)?.type_reference,
                                    target.type_identity,
                                    trace,
                                    Some(&|expression| transition_selector(
                                        facts, machine.symbol, source.symbol, ordinal, target.position, expression,
                                    )),
                                )?;
                                if matches!(subject.source, CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }) {
                                    subject.source = super::super::local_storage::source(
                                        program, source, operations, root_symbol,
                                    )?;
                                }
                                trace.phase("stored reference arrival: access");
                                trace.statement(Some(ordinal));
                                if subject.access != expected_access {
                                    return None;
                                }
                                let carrier_type_identity = shapes.add_reference_type(
                                    target_parameters.get(target.position as usize)?.type_reference,
                                    &super::super::types::machine_binders(program, machine),
                                )?;
                                let (backing_type, source_contract) = crate::values::reference_sources::qualification_contract_at_site(
                                    program, source.symbol, ordinal as usize, expression,
                                    target_parameters.get(target.position as usize)?.type_reference,
                                    subject.type_identity,
                                );
                                return structural_transfer_plan(
                                    checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                                        backing_type, source_contract,
                                        retired_carrier_backing: super::loan_backing::retired_carrier_backing(
                                            program, records, machine.symbol, source.symbol,
                                            ordinal as usize, arrival.target, arguments,
                                            target.position as usize, operations, &subject,
                                        ),
                                        subject,
                                        formation_expression: expression,
                                        carrier_type_identity,
                                    },
                                    target_index,
                                );
                            }
                            let stored_leaf_access = validation::canonical_place_type_reference(
                                program, source.symbol, ordinal as usize, &place,
                            ).and_then(|reference| {
                                validation::reference_result_custody::stored_loan_parts(program, reference)
                            }).map(|(_, access)| access);
                            let mutable_local_loan = stored_leaf_access
                                .is_none_or(loan_permitted);
                            if mutable_local_loan
                                && let Some(transfer) = projected_loan_transfer(
                                    program,
                                    facts,
                                    shapes,
                                    machine.symbol,
                                    source.symbol,
                                    ordinal,
                                    expression,
                                    &place,
                                    super::super::local_storage::source(
                                        program, source, operations, root_symbol,
                                    )?,
                                    expected_access,
                                    target,
                                    target_parameters,
                                    target_index,
                                )
                            {
                                return Some(transfer);
                            }
                        }
                    }
                }
                mark(SuccessorGuard::EstablishedArgument);
                if let Some(root) = evaluated_argument {
                    let stored_reference = target.access != CheckedStructuralAccess::Owned
                        && validation::reference_result_custody::stored_loan_parts(program, root.type_reference).is_some();
                    let multiplicity = if stored_reference {
                        validation::reference_result_custody::result_multiplicity(program, root.type_reference)
                    } else if target.access == CheckedStructuralAccess::Owned {
                        if super::super::types::base_type_identity(program, root.type_reference, &[], &[])?
                            != target.type_identity
                        {
                            return None;
                        }
                        target.multiplicity
                    } else if validation::slice_view_shell(program, root.type_reference, &[])
                        .is_some()
                    {
                        let view = validation::slice_view_shell(program, root.type_reference, &[])?;
                        let access: CheckedStructuralAccess = view.access.into();
                        let parameter = target_parameters.get(target.position as usize)?;
                        if access != target.access
                            || target.multiplicity != Multiplicity::Unrestricted
                            || program.normalized_type_identity(root.type_reference)
                                != program.normalized_type_identity(parameter.type_reference)
                            || super::super::types::base_type_identity(program, root.type_reference, &[],
            &[])?
                                != target.type_identity
                        {
                            return None;
                        }
                        match access {
                            CheckedStructuralAccess::SharedBorrow => Multiplicity::Unrestricted,
                            CheckedStructuralAccess::MutableBorrow => Multiplicity::Affine,
                            _ => return None,
                        }
                    } else {
                        let multiplicity = validation::reference_result_custody::result_multiplicity(
                            program,
                            root.type_reference,
                        );
                        if target.access != CheckedStructuralAccess::SharedBorrow
                            || target.multiplicity != Multiplicity::Unrestricted
                            || !matches!(multiplicity, Multiplicity::Unrestricted | Multiplicity::Affine)
                            || (multiplicity == Multiplicity::Affine
                                && (!validation::has_contents(
                                    program,
                                    root.type_reference, &[],
                                    validation::ContentsRequirement::OwnedOrSharedView,
                                ) || super::super::types::type_graph_requires_nominal_drop(
                                    program,
                                    root.type_reference,
                                    &[],
                                    &mut std::collections::BTreeSet::new(),
                                )))
                            || super::super::types::base_type_identity(
                                program,
                                root.type_reference,
                                &[],
                                &[],
                            )? != target.type_identity
                        {
                            return None;
                        }
                        multiplicity
                    };
                    let mut operand_calls = operations.to_vec();
                    let mut result_count = super::returns::next_result_ordinal(operations)?;
                    let (calls, completion) = control::structural_operands::value_calls(
                        program,
                        facts,
                        edge_calls.scalar_callees,
                        shapes,
                        machine,
                        source,
                        source_structural,
                        &[],
                        edge_calls.entry_claims,
                        edge_calls.local_results,
                        &operand_calls,
                        &mut result_count,
                        root.root,
                        trace,
                        &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
                    )?;
                    if completion == checked_trees::CheckedValueCompletion::NoNormal {
                        let evaluation = CheckedUnitEffectOperationPlan::EvaluateStructuralValue {
                            statement_index: ordinal,
                            value: root.root,
                            calls,
                            operand_source: None,
                        };
                        operand_calls.push(evaluation);
                        return structural_transfer_plan(
                            checked_trees::CheckedStructuralControlTransferSourcePlan::EvaluateNoNormal {
                                operations: checked_trees::CheckedOperandSequence::new(
                                    operand_calls.split_off(operations.len()),
                                ),
                            },
                            target_index,
                        );
                    }
                    mark(SuccessorGuard::EstablishedArgument);
                    let result = checked_trees::CheckedUnitStructuralResultBindingPlan {
                        statement_index: ordinal,
                        binding_ordinal: u32::try_from(result_count).ok()?,
                        type_identity: if stored_reference {
                            shapes.add_reference_type(root.type_reference, &super::super::types::machine_binders(program, machine))?
                        } else { target.type_identity },
                        multiplicity,
                        local: Default::default(),
                    };
                    let presentation = if stored_reference {
                        let owner = facts::PlaceRoot::Expression(expression);
                        calls::retained_reference_argument(
                            program, machine.symbol, source, ordinal as usize,
                            &validation::CanonicalPlace { root: owner, segments: Vec::new() },
                            owner, root.type_reference, &result,
                            target_parameters.get(target.position as usize)?.type_reference, target.type_identity,
                            trace, None,
                        )?
                    } else {
                        CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal: result.binding_ordinal,
                            },
                            path: Vec::new(),
                            type_identity: target.type_identity,
                            access: target.access,
                        }
                    };
                    let establishment = control::structural_operands::establish_value(
                        facts, result, root.root, calls, None, false, &mut operand_calls,
                    )?;
                    let operand_calls = operand_calls.split_off(operations.len());
                    return structural_transfer_plan(
                        checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                            operands: checked_trees::CheckedOperandSequence::new(operand_calls),
                            establishment: Box::new(establishment),
                            presentation,
                        },
                        target_index,
                    );
                }
                None
                })();
                *stopped = transfer.as_ref().is_some_and(|transfer| matches!(transfer.source,
                    checked_trees::CheckedStructuralControlTransferSourcePlan::EvaluateNoNormal { .. }));
                Some(transfer)
            })
            .collect::<Option<Vec<_>>>()?;
        for (transfer, parameter) in transfers.iter_mut().zip(target_structural) {
            if matches!(
                transfer.source,
                checked_trees::CheckedStructuralControlTransferSourcePlan::EvaluateNoNormal { .. }
            ) {
                break;
            }

            trace.phase("state loan: captured backing");
            trace.statement(Some(ordinal));
            let storage = super::loan_backing::retained_storage(
                declarations,
                records,
                machine.symbol,
                source.symbol,
                ordinal as usize,
                arrival.target,
                arguments,
                parameter.position as usize,
                operations,
                match &transfer.source {
                    checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                        subject,
                        ..
                    } => Some(subject.source.clone()),
                    _ => None,
                },
            )
            .map_err(|reason| {
                trace.phase(reason);
                trace.statement(Some(ordinal));
            })
            .ok()?;
            if parameter.access != CheckedStructuralAccess::Owned {
                transfer.retained_loan_backing = storage.backing;
                transfer.incoming_loan_resources = storage.incoming;
            }
            transfer.retained_loan_parents = storage.parents;
            transfer.retained_operand_parents = storage.operand_parents;
        }
        let stopped_position = transfers.last().and_then(|transfer| {
            matches!(
                transfer.source,
                checked_trees::CheckedStructuralControlTransferSourcePlan::EvaluateNoNormal { .. }
            )
            .then(|| {
                target_structural
                    .get(transfer.target_parameter_index as usize)
                    .map(|target| target.position)
            })
            .flatten()
        });
        mark(SuccessorGuard::ScalarArguments);
        let scalar_arguments = target_scalar
            .iter()
            .enumerate()
            .filter(|(index, _)| !payload_parameters.contains(&(*index as u32)))
            .filter(|(_, target)| {
                stopped_position.is_none_or(|position| target.source_position < position)
            })
            .map(|(target_index, target)| {
                let argument = argument_at(target.source_position)?;
                let role = CheckedScalarExpressionRole::TransitionArgument {
                    argument_ordinal: target.source_position,
                };
                let pure = &facts.values.scalar_expressions;
                let computations = &facts.values.scalar_computations;
                let mut roots = computations
                    .roots
                    .iter()
                    .map(|(_, root)| root)
                    .filter(|root| {
                        root.state == source.symbol
                            && root.statement_ordinal == ordinal
                            && root.role == role
                    });
                let root = roots.next();
                if roots.next().is_some() {
                    return None;
                }
                if let Some(root) = root {
                    let has_pure = pure.source_bindings.iter().any(|(_, binding)| {
                        binding.state == source.symbol
                            && binding.statement_ordinal == ordinal
                            && binding.role == role
                    }) || pure.expressions.iter().any(|expression| {
                        expression.state == source.symbol
                            && expression.statement_ordinal == ordinal
                            && expression.role == role
                    });
                    if has_pure
                        || !computations.is_authored_root(
                            root,
                            machine.symbol,
                            argument,
                            target.scalar_type,
                        )
                    {
                        return None;
                    }
                } else {
                    let (custody, expression) =
                        pure.bound_expression_at(source.symbol, ordinal, role)?;
                    if custody.expression != argument
                        || custody.destination
                            != target_parameters
                                .get(target.source_position as usize)?
                                .symbol
                        || expression.scalar_type() != Some(target.scalar_type)
                    {
                        return None;
                    }
                }
                let immutable_source_position = source_position(target.source_position)
                    .filter(|position| !program.state_parameters(source)[*position].is_mutable);
                let forwarded_scalar = immutable_source_position.and_then(|source_position| {
                    source_scalar.iter().position(|parameter| {
                        parameter.source_position as usize == source_position
                            && parameter.scalar_type == target.scalar_type
                    })
                });
                let source = if let Some(source_index) = forwarded_scalar {
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter {
                        index: u32::try_from(source_index).ok()?,
                    }
                } else {
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
                };
                Some(CheckedStructuralScalarArgumentPlan {
                    argument_ordinal: target.source_position,
                    source,
                    target_scalar_parameter_index: u32::try_from(target_index).ok()?,
                    scalar_type: target.scalar_type,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        mark(SuccessorGuard::ErasedArguments);
        let erased_parameter_plans =
            crate::execution::terminal_unit::types::erased_scalar_parameter_plans(program, target)?;
        let erased_arguments = erased_parameter_plans
            .iter()
            .enumerate()
            .map(|(erased_index, target)| {
                // A transition argument row is keyed by its authored
                // position and so carries one erased scalar. A record
                // formal with several leaves would bind every leaf to the
                // same row; refuse it instead.
                if erased_parameter_plans
                    .iter()
                    .filter(|plan| plan.source_position == target.source_position)
                    .count()
                    != 1
                {
                    return None;
                }
                let argument = argument_at(target.source_position)?;
                let (custody, expression) = facts.values.scalar_expressions.bound_expression_at(
                    source.symbol,
                    ordinal,
                    CheckedScalarExpressionRole::TransitionArgument {
                        argument_ordinal: target.source_position,
                    },
                )?;
                if custody.expression != argument
                    || custody.destination
                        != target_parameters
                            .get(target.source_position as usize)?
                            .symbol
                    || expression.scalar_type() != Some(target.scalar_type)
                {
                    return None;
                }
                Some(CheckedStructuralScalarArgumentPlan {
                    argument_ordinal: target.source_position,
                    source: checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression,
                    target_scalar_parameter_index: u32::try_from(erased_index).ok()?,
                    scalar_type: target.scalar_type,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        mark(SuccessorGuard::ErasedProofArguments);
        let erased_proof_arguments =
            crate::execution::terminal_unit::types::erased_proof_parameter_plans(program, target)?
                .iter()
                .map(|target| {
                    facts
                        .values
                        .proof_terms
                        .term_at(
                            source.symbol,
                            ordinal,
                            checked_trees::CheckedProofTermRole::TransitionArgument {
                                argument_ordinal: target.source_position,
                            },
                        )
                        .cloned()
                })
                .collect::<Option<Vec<_>>>()?;
        Some(CheckedStructuralControlSuccessorPlan::new(
            ordinal,
            target.symbol,
            crate::execution::successor_completion::classify(program, machine, arrival.target),
            false,
            transfers,
            scalar_arguments,
            erased_arguments,
            erased_proof_arguments,
            Vec::new(),
        ))
    }

    /*
        Resolve a finite field/index path below an established source root.
        Unrestricted observations may read a borrowed root; transferring affine or
        linear custody requires an owned root. Parameter and local establishments
        share that rule and the target's exact subtree type.
    */
    fn projected_value_subject(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        source: &typed_trees::state::State,
        source_structural: &[CheckedUnitStructuralParameterPlan],
        local_results: &[(
            checked_trees::CheckedUnitStructuralResultBindingPlan,
            facts::PlaceRoot,
        )],
        ordinal: u32,
        expression: typed_trees::expression::ExpressionHandle,
        target: &CheckedUnitStructuralParameterPlan,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        if target.access != CheckedStructuralAccess::Owned {
            return None;
        }
        let place = place_at(program, source.symbol, ordinal as usize, expression)?;
        if place.segments.is_empty() {
            return None;
        }
        let parameter =
            program
                .state_parameters(source)
                .iter()
                .enumerate()
                .find(|(_, parameter)| {
                    place.root.as_symbol().is_some_and(|symbol| {
                        super::super::types::parameter_is_root(parameter, symbol)
                    })
                });
        let (argument_source, root_access) = if let Some((position, _)) = parameter {
            let (parameter_index, parameter) = source_structural
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.position as usize == position)?;
            (
                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(parameter_index).ok()?,
                },
                parameter.access,
            )
        } else {
            let mut candidates = local_results
                .iter()
                .filter(|(result, root)| *root == place.root && result.statement_index <= ordinal);
            let (result, _) = candidates.next()?;
            let root_type = validation::canonical_place_type_reference(
                program,
                source.symbol,
                ordinal as usize,
                &validation::CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                },
            )?;
            if candidates.next().is_some()
                || super::super::types::base_type_identity(program, root_type, &[], &[])?
                    != result.type_identity
            {
                return None;
            }
            (
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                },
                super::super::types::structural_access_for_type_reference(program, root_type)?,
            )
        };
        if root_access == CheckedStructuralAccess::WriteOnlyBorrow {
            return None;
        }
        let ProjectedArgumentPath {
            storage_type: projected_type,
            path,
            ..
        } = calls::projected_borrowed_path(
            program,
            machine.symbol,
            source.symbol,
            ordinal as usize,
            &place,
        )?;
        if path.is_empty()
            || !path.iter().all(|segment| {
                matches!(
                    segment,
                    CheckedUnitStructuralPathSegment::Field(_)
                        | CheckedUnitStructuralPathSegment::FixedIndex(_)
                        | CheckedUnitStructuralPathSegment::Case(_)
                )
            })
            || program.type_multiplicity(projected_type) != target.multiplicity
            || (target.multiplicity != Multiplicity::Unrestricted
                && root_access != CheckedStructuralAccess::Owned)
            || super::super::types::base_type_identity(program, projected_type, &[], &[])
                .is_none_or(|identity| identity != target.type_identity)
        {
            return None;
        }
        let access =
            super::super::types::structural_access_for_type_reference(program, projected_type)?;
        Some(CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source: argument_source,
            path,
            type_identity: target.type_identity,
            access,
        })
    }

    /*
        A projected-loan edge transfer against an already-admitted place:
        Resolve one occurrence-owned selector shared by every successor loan route.
    */
    fn transition_selector(
        facts: &CheckFacts,
        machine: symbols::SymbolHandle,
        source: symbols::SymbolHandle,
        ordinal: u32,
        argument_ordinal: u32,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<CheckedUnitStructuralPathSegment> {
        let computations = &facts.values.scalar_computations;
        let root = computations.root_at(
            source,
            ordinal,
            CheckedScalarExpressionRole::TransitionSelector {
                argument_ordinal,
                expression,
            },
        )?;
        if !computations.nodes.is_valid(root.root) {
            return None;
        }
        let primitive = computations.nodes.get(root.root).scalar_type.primitive()?;
        (primitive.is_fixed_integer()
            && computations.is_authored_root(root, machine, expression, primitive))
        .then_some(CheckedUnitStructuralPathSegment::ComputedIndex(root.root))
    }

    /*
        Build a structural control-transfer plan for one admitted argument.

        Every admitted edge supplies its own `source`; the remaining fields name
        the target parameter position and stay empty until a later phase assigns
        custody, so they are built here rather than repeated at each admission.
    */
    fn structural_transfer_plan(
        source: checked_trees::CheckedStructuralControlTransferSourcePlan,
        target_index: usize,
    ) -> Option<CheckedStructuralControlTransferPlan> {
        Some(CheckedStructuralControlTransferPlan {
            source,
            target_parameter_index: u32::try_from(target_index).ok()?,
            retained_loan_backing: None,
            incoming_loan_resources: Vec::new(),
            retained_loan_parents: Vec::new(),
            retained_operand_parents: Vec::new(),
        })
    }

    /*
        `subject_source` is the parameter or earlier result the loan rides on.

        A projected loan keeps its original storage root and exact path across
        the edge. Case steps use this same route, including deeper payload
        fields; receiving verification must prove each selected case active
        before rebinding the loan. Stored reference leaves cross their exact
        descriptor under the same endpoint judgment as call arguments. Carrier
        custody remains with the checked source; the selected referent, rather
        than an unrelated enclosing record, determines the incoming storage type.
        Reference-bearing referents still need their separate carrier custody.
        A state argument has the same
        presentation as a call argument; access, live-home and reference-content
        checks remain independent.
    */
    #[allow(clippy::too_many_arguments)]
    fn projected_loan_transfer(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &mut ShapeCollector<'_>,
        machine: symbols::SymbolHandle,
        source: symbols::SymbolHandle,
        ordinal: u32,
        formation_expression: ExpressionHandle,
        place: &validation::CanonicalPlace,
        subject_source: CheckedUnitStructuralArgumentSourcePlan,
        expected_access: CheckedStructuralAccess,
        target: &CheckedUnitStructuralParameterPlan,
        target_parameters: &[typed_trees::signature::StateParameter],
        target_index: usize,
    ) -> Option<CheckedStructuralControlTransferPlan> {
        let selector = |expression| {
            transition_selector(facts, machine, source, ordinal, target.position, expression)
        };
        let ProjectedArgumentPath {
            storage_type: projected_type,
            path,
            ..
        } = calls::projected_evaluated_path(
            program,
            machine,
            source,
            ordinal as usize,
            place,
            Some(&selector),
        )?;
        let target_type_reference = target_parameters
            .get(target.position as usize)?
            .type_reference;
        let (projected_type, path) = calls::borrowed_argument_endpoint(
            program,
            projected_type,
            path,
            target_type_reference,
        )?;
        if !path.iter().all(|segment| {
            matches!(
                segment,
                CheckedUnitStructuralPathSegment::Field(_)
                    | CheckedUnitStructuralPathSegment::Case(_)
                    | CheckedUnitStructuralPathSegment::FixedIndex(_)
                    | CheckedUnitStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::Parameter { .. }
                            | checked_trees::CheckedRuntimeIndex::Local { .. }
                    )
                    | CheckedUnitStructuralPathSegment::ComputedIndex(_)
                    | CheckedUnitStructuralPathSegment::Referent
            )
        }) || !calls::argument_presentation::classify(
            program,
            projected_type,
            target_type_reference,
            target.type_identity,
            &[],
        )
        .is_some_and(|presentation| {
            presentation != calls::argument_presentation::ArgumentPresentation::FixedArraySlice
        }) {
            return None;
        }
        let has_dynamic_selector = path.iter().any(|segment| {
            matches!(
                segment,
                CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                    | CheckedUnitStructuralPathSegment::ComputedIndex(_)
            )
        });
        let carrier_type_identity = if matches!(
            super::super::types::byte_sequence_carrier(program, target_type_reference, &[]),
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. })
        ) && !has_dynamic_selector
        {
            Default::default()
        } else {
            match shapes.add_reference_type(target_type_reference, &[]) {
                Some(identity) => identity,
                None if !has_dynamic_selector => Default::default(),
                None => return None,
            }
        };
        let (backing_type, source_contract) =
            crate::values::reference_sources::qualification_contract_at_site(
                program,
                source,
                ordinal as usize,
                formation_expression,
                target_type_reference,
                target.type_identity,
            );
        structural_transfer_plan(
            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                backing_type,
                source_contract,
                retired_carrier_backing: None,
                formation_expression,
                carrier_type_identity,
                subject: CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: subject_source,
                    path,
                    type_identity: target.type_identity,
                    access: expected_access,
                },
            },
            target_index,
        )
    }

    /*
        Admit a `[copy]` case-payload subtree transfer on a case-tested edge.

        The edge's guard selected `selected_case` on `subject_place`, so an
        argument whose place extends that subject through exactly that case
        into one declared payload field carries proven membership into the
        target's custody — the same authorization an authored case arm gives
        its destructure bindings. The checked source keeps the resolved
        subject argument plus the selected case and field identities, so a
        downstream channel can rejoin the projected subtree without
        re-deriving the destructure local.
    */
    fn case_payload_transfer(
        program: &dyn ::validation::CheckingInput,
        place: validation::CanonicalPlace,
        subject_place: &validation::CanonicalPlace,
        selected_case: symbols::SymbolHandle,
        source: &typed_trees::state::State,
        source_structural: &[CheckedUnitStructuralParameterPlan],
        operations: &[CheckedUnitEffectOperationPlan],
        ordinal: u32,
        target: &CheckedUnitStructuralParameterPlan,
    ) -> Option<checked_trees::CheckedStructuralControlTransferSourcePlan> {
        if target.access == CheckedStructuralAccess::Owned
            && target.multiplicity != Multiplicity::Unrestricted
        {
            return None;
        }
        if place.root != subject_place.root
            || place.segments.len() != subject_place.segments.len() + 2
            || place.segments[..subject_place.segments.len()] != subject_place.segments[..]
        {
            return None;
        }
        let [
            facts::PlaceSegment::Case { variant },
            facts::PlaceSegment::Field {
                symbol: field_symbol,
            },
        ] = &place.segments[subject_place.segments.len()..]
        else {
            return None;
        };
        if *variant != selected_case {
            return None;
        }
        let field = program
            .variant_owner(selected_case)
            .and_then(|(_, variant)| {
                program.data_payload_field_by_symbol(variant, *field_symbol)
            })?;
        let normalized =
            |reference| super::super::types::base_type_identity(program, reference, &[], &[]);
        let shared_referee = {
            let mut cursor = field.type_reference;
            loop {
                match program.type_reference_table.type_reference(cursor) {
                    TypeReferenceNode::Constrained { base_type, .. } => cursor = *base_type,
                    TypeReferenceNode::Reference {
                        access: language_semantics::ReferenceAccess::Shared,
                        referee,
                        ..
                    } => break Some(*referee),
                    _ => break None,
                }
            }
        };
        let admitted_shape = if target.access == CheckedStructuralAccess::Owned {
            (validation::has_contents(
                program,
                field.type_reference,
                &[],
                validation::ContentsRequirement::NumericOwned,
            ) || validation::has_contents(
                program,
                field.type_reference,
                &[],
                validation::ContentsRequirement::OwnedOrSharedView,
            )) && normalized(field.type_reference)
                .is_some_and(|identity| identity == target.type_identity)
        } else {
            target.access == CheckedStructuralAccess::SharedBorrow
                && super::super::types::structural_access_for_type_reference(
                    program,
                    field.type_reference,
                ) == Some(CheckedStructuralAccess::SharedBorrow)
                && shared_referee.is_some_and(|referee| {
                    normalized(referee).is_some_and(|identity| identity == target.type_identity)
                })
        };
        if !admitted_shape || program.type_multiplicity(field.type_reference) != target.multiplicity
        {
            return None;
        }
        let root = subject_place.root.as_symbol()?;
        let (subject_source, mut subject_type, root_identity) = if let Some(position) =
            typed_trees::signature::state_parameter_position(program.state_parameters(source), root)
        {
            let (parameter_index, parameter) = source_structural
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.position as usize == position)?;
            (
                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(parameter_index).ok()?,
                },
                program.state_parameters(source)[position].type_reference,
                parameter.type_identity,
            )
        } else {
            let mut results = operations.iter().filter_map(|operation| {
                if operand_owned_construction(operation) {
                    return None;
                }
                let result = retained_structural_result(operation)
                    .filter(|result| result.statement_index < ordinal)?;
                let local = crate::flow::state_statements(program, source.statement_nodes)
                    .get(result.statement_index as usize)?
                    .as_local_data()?;
                (local.symbol == root && !local.is_mutable).then_some((result, local))
            });
            let (result, local) = results.next()?;
            if results.next().is_some() {
                return None;
            };
            (
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                },
                local.type_reference,
                result.type_identity,
            )
        };
        let path = subject_place
            .segments
            .iter()
            .map(|segment| {
                let facts::PlaceSegment::Field { symbol } = segment else {
                    return None;
                };
                subject_type = program
                    .field_owner(*symbol)
                    .map(|(_, field)| field.type_reference)?;
                super::super::types::terminal_field_identity(program, *symbol)
                    .map(CheckedUnitStructuralPathSegment::Field)
            })
            .collect::<Option<Vec<_>>>()?;
        let variant = program
            .variant_owner(selected_case)
            .map(|(_, variant)| variant)?;
        let subject_identity = if path.is_empty() {
            root_identity
        } else {
            normalized(subject_type)?
        };
        Some(
            checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
                subject: CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: subject_source,
                    path,
                    type_identity: subject_identity,
                    access: CheckedStructuralAccess::SharedBorrow,
                },
                case_identity: checked_trees::CheckedMemberIdentity::of_member(
                    variant.identity,
                    variant.name.clone(),
                ),
                field_identity: checked_trees::CheckedMemberIdentity::of_member(
                    field.identity,
                    field.name.clone(),
                ),
                path: Vec::new(),
            },
        )
    }
}
mod terminal_calls {

    /*
        Terminal machine calls in transition arms
        ([state contracts](../../../../../../../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms)).

        An arm is classified by its resolved target: a local state is an internal
        jump (`successor` plans its edge), as is a machine entry retained in the
        recursive component. A destination outside that component is an ordinary
        invocation whose normal result completes this machine. Two placements
        exist, and neither bypasses the ordinary call plan:

        - An unconditional final arm (`transition { _ -> finish(value) }`) is the
          state's last operation. The shared statement sequence plans it through
          `call_occurrences::terminal_invocation`, exactly as a single-state body
          does, so its occurrence counts against the state's call roster and its
          value result binds as the state's returned value; `completion` then
          completes the state with that result, whatever its kind.
        - A guarded arm's call runs only when its arm is selected, so it cannot
          join the state's linear operations. `arm_call` plans it with the same
          `build_call_operation` as the arm's own operation
          (`CheckedConditionalReturnArm::Call`); emission places it in the arm's
          private completion block.

        Its subslice operands replay at the call-argument site the checker records
        for an invocation (`values::scalar::expression_plans::transition`), like an
        expression call's.

        The callee's normal result completes this machine. A scalar result is the
        call's ordinary scalar binding: the unconditional arm binds it as the
        state's returned value
        (`CheckedScalarReturnPlan::Binding` at the transition), and a guarded arm
        binds it at the next ordinal of the namespace its arm inherits and returns
        it there. A structural result is likewise the unconditional call's own
        binding, which the state returns (`ReturnStructural`). The two signatures
        must agree exactly; checking rejects a terminal call whose result differs
        from the caller's declared result.

        Scalar operand computations retain their nested calls. Structural operands
        retain the ordinary preparation sequence; the shared argument schedule
        evaluates both inside the selected arm in authored order.
        Structural results use the ordinary call binding after operand preparation;
        completion transfers that value instead of discarding it. An arm call still
        retains ordinary exact claim transfers into the callee.
    */
    use super::{
        CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
        ScalarCalleeClosure, TransitionExit, control,
    };
    use crate::execution::terminal_unit::calls::{ExpectedCallValueResult, build_call_operation};
    use crate::execution::terminal_unit::control::{
        bind_scalar_call_result, bind_structural_call_result, checked_structural_result_type,
    };
    use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders, state_flow};

    /*
        The exact callable destination an arm invokes. Local state transfers and
        entry re-entry remain state-graph edges; requirements keep their declaration
        identity and ordinary machines keep their executable state identity.
    */
    pub(super) fn invoked_callable(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        transition: &typed_trees::statement::TableTransition,
    ) -> Option<symbols::SymbolHandle> {
        if transition.exit != TransitionExit::Ordinary
            || transition.continuation.is_valid()
            || !program
                .statement_table
                .transition_target_is_valid(transition.target)
        {
            return None;
        }
        let invocation = validation::completing_transition_invocation(program, transition.target)?;
        validation::terminal_callable_target(program, machine, invocation.target)
            .map(|(target, _)| target)
    }

    /*
        Only a destination outside the retained graph needs a call operation.
        A fused machine entry still has its authored call contract, but execution
        transfers to its retained state instead of allocating another activation.
    */
    pub(super) fn external_callable(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        fused: &super::FusedStates<'_>,
        transition: &typed_trees::statement::TableTransition,
    ) -> Option<symbols::SymbolHandle> {
        let target = invoked_callable(program, machine, transition)?;
        super::fused_target_index(program, fused, target)
            .is_none()
            .then_some(target)
    }

    /*
        Complete a state whose unconditional final arm invokes a machine outside
        the retained graph. The statement sequence already planned that call as
        the state's last operation (statement `statement_ordinal`, the
        statement's only outer call) and bound its value result as the state's
        returned value. The callee's normal result completes this machine
        unchanged: Unit returns, a scalar returns its binding, and a structural
        value transfers its binding.
    */
    pub(super) fn completion(
        operations: &[CheckedUnitEffectOperationPlan],
        statement_ordinal: u32,
        result: &checked_trees::CheckedControlResultPlan,
        scalar_result: Option<&checked_trees::CheckedUnitScalarResultBindingPlan>,
        structural_result: Option<&checked_trees::CheckedUnitStructuralReturnPlan>,
    ) -> Option<CheckedComposedUnitControlTerminatorPlan> {
        let mut calls = operations
            .iter()
            .filter_map(CheckedUnitEffectOperationPlan::call)
            .filter(|call| {
                call.coordinate.statement_index == statement_ordinal
                    && call.coordinate.call_ordinal == 0
            });
        let call = calls.next()?;
        if calls.next().is_some() {
            return None;
        }
        if *call.result() == checked_trees::CheckedUnitCallResult::NoNormal {
            return Some(CheckedComposedUnitControlTerminatorPlan::InvokeNoNormal {
                coordinate: call.coordinate,
            });
        }
        match (result, call.result()) {
            (
                checked_trees::CheckedControlResultPlan::Unit,
                checked_trees::CheckedUnitCallResult::Unit,
            ) => Some(CheckedComposedUnitControlTerminatorPlan::ReturnUnit),
            (
                checked_trees::CheckedControlResultPlan::Scalar {
                    scalar_type: primitive_type,
                },
                checked_trees::CheckedUnitCallResult::Scalar(bound),
            ) if bound.scalar_type == *primitive_type
                && bound.statement_index == statement_ordinal
                && scalar_result == Some(bound) =>
            {
                Some(CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                    completion: checked_trees::CheckedScalarReturnPlan::Binding(*bound),
                })
            }
            (
                checked_trees::CheckedControlResultPlan::Structural(expected),
                checked_trees::CheckedUnitCallResult::Structural { binding, .. },
            ) if binding.statement_index == statement_ordinal
                && binding.type_identity == expected.type_identity
                && binding.multiplicity == expected.multiplicity =>
            {
                let returned = structural_result?;
                (returned.source
                    == CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: binding.binding_ordinal,
                    })
                .then(|| {
                    let mut returned = returned.clone();
                    returned.completion = checked_trees::CheckedStructuralReturnCompletion::Call {
                        coordinate: call.coordinate,
                    };
                    CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned }
                })
            }
            _ => None,
        }
    }

    /*
        Plan a guarded arm's terminal machine call as the arm's own operation.
        The outer flow call names this exact transition target after the guard's
        earlier occurrences. Operand computations and structural preparation retain
        their own calls and execute inside this arm's completion block.

        A scalar-result machine forwards the callee's result: the call binds it
        at `next_scalar_binding`, the ordinal the arm's namespace (the state's
        retained bindings) gives its next result, and the arm returns that value.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn arm_call(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural: &[CheckedUnitStructuralParameterPlan],
        entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
        local_results: &[(
            checked_trees::CheckedUnitStructuralResultBindingPlan,
            facts::PlaceRoot,
        )],
        preamble: &[checked_trees::CheckedUnitEffectOperationPlan],
        result: &checked_trees::CheckedControlResultPlan,
        next_scalar_binding: usize,
        transition: &typed_trees::statement::TableTransition,
        statement_ordinal: u32,
        trace: &control::LocalConstructionTrace,
    ) -> Option<checked_trees::CheckedConditionalReturnArm> {
        let target = invoked_callable(program, machine, transition)?;
        trace.phase("state graph: terminator: terminal call arm: completion");
        trace.statement(Some(statement_ordinal));
        // Bind the receiving result, including its checked qualifications. The
        // ordinary call builder checks the callee's carrier/access/multiplicity;
        // completed-call guarantees, not syntactic qualified-type equality,
        // justify the stronger receiving facts and exact returned claims.
        trace.phase("state graph: terminator: terminal call arm: occurrence");
        trace.statement(Some(statement_ordinal));
        let flow = state_flow(facts, machine.symbol, state.symbol)?;
        let calls = facts.flow.control.calls.span_or_empty(flow.calls);
        let mut occurrences = calls.iter().filter(|call| {
            call.statement_index == statement_ordinal as usize
                && !facts.flow.control.is_retired(state.symbol, call)
                && validation::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    call.statement_index,
                    call.call_ordinal,
                )
                .is_some_and(|site| site.completes_transition(program, transition.target))
                && validation::transition_call_target(
                    program,
                    machine,
                    state,
                    call.statement_index,
                    call.call_ordinal,
                ) == Some(transition.target)
        });
        let call = occurrences.next()?;
        if occurrences.next().is_some()
            || validation::terminal_callable_target(program, machine, call.target_symbol)
                .is_none_or(|(selected, _)| selected != target)
        {
            return None;
        }
        plan_call_arm(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural,
            entry_claims,
            local_results,
            preamble,
            result,
            next_scalar_binding,
            calls,
            None,
            call,
            target,
            statement_ordinal,
            trace,
        )
    }

    /// The exact captured invocation owns ordinary operand preparation, result
    /// delivery and loan completion, whether selected by a statement or a Match.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn plan_call_arm(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural: &[CheckedUnitStructuralParameterPlan],
        entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
        local_results: &[(
            checked_trees::CheckedUnitStructuralResultBindingPlan,
            facts::PlaceRoot,
        )],
        preamble: &[CheckedUnitEffectOperationPlan],
        result: &checked_trees::CheckedControlResultPlan,
        next_scalar_binding: usize,
        calls: &[checked_trees::FlowCallFact],
        mut selected: Option<&mut control::SelectedInvocationAccounting>,
        call: &checked_trees::FlowCallFact,
        target: symbols::SymbolHandle,
        statement_ordinal: u32,
        trace: &control::LocalConstructionTrace,
    ) -> Option<checked_trees::CheckedConditionalReturnArm> {
        let no_normal = validation::call_target_state(program, target).is_some_and(|state| {
            validation::normal_completion_possible(program, state) == Some(false)
        });
        let (scalar_result, mut structural_result) = if no_normal {
            (None, None)
        } else {
            match result {
                checked_trees::CheckedControlResultPlan::Unit => (None, None),
                checked_trees::CheckedControlResultPlan::Scalar {
                    scalar_type: primitive_type,
                } => (
                    Some(checked_trees::CheckedUnitScalarResultBindingPlan {
                        statement_index: statement_ordinal,
                        binding_ordinal: u32::try_from(next_scalar_binding).ok()?,
                        scalar_type: (*primitive_type).into(),
                    }),
                    None,
                ),
                checked_trees::CheckedControlResultPlan::Structural(expected) => {
                    let binding = checked_structural_result_type(
                        program,
                        shapes,
                        state.return_type,
                        &machine_binders(program, machine),
                    )?;
                    if binding.type_identity != expected.type_identity
                        || binding.multiplicity != expected.multiplicity
                    {
                        return None;
                    }
                    (None, Some(binding))
                }
            }
        };
        trace.phase("state graph: terminator: terminal call arm: operands");
        let mut structural_results = local_results.to_vec();
        let mut preparation = preamble.to_vec();
        let predecessor_count = preparation.len();
        let mut structural_count = super::returns::next_result_ordinal(preamble)?;
        let completion = control::structural_operands::plan_call_operands(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural,
            &[],
            entry_claims,
            &mut structural_results,
            &mut preparation,
            &mut structural_count,
            call,
            trace,
            &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
        )?;
        if completion == checked_trees::CheckedValueCompletion::NoNormal {
            if let Some(selected) = selected.as_deref_mut() {
                selected.stopped_prefix(facts, call, &preparation[predecessor_count..])?;
            }
            return Some(
                checked_trees::CheckedConditionalReturnArm::EvaluateNoNormal {
                    statement_ordinal,
                    operations: checked_trees::CheckedOperandSequence::new(
                        preparation.split_off(predecessor_count),
                    ),
                },
            );
        }
        if let Some(binding) = &mut structural_result {
            binding.statement_index = statement_ordinal;
            binding.binding_ordinal = u32::try_from(structural_count).ok()?;
        }
        trace.phase("state graph: terminator: terminal call arm: operand occurrences");
        trace.statement(Some(statement_ordinal));
        control::call_owns_statement_computations(
            program,
            facts,
            machine.symbol,
            state.symbol,
            call,
            calls,
            &preparation,
            selected,
        )?;
        trace.phase("state graph: terminator: terminal call arm: call operation");
        trace.statement(Some(statement_ordinal));
        let operation = build_call_operation(
            program,
            shapes,
            facts,
            Some(scalar_callees),
            machine,
            state,
            structural,
            &[],
            call,
            structural_result
                .as_ref()
                .map(ExpectedCallValueResult::Structural)
                .or_else(|| {
                    scalar_result.map(|result| ExpectedCallValueResult::Scalar(result.scalar_type))
                }),
            &structural_results,
            &preparation,
            trace,
        )?;
        trace.phase("state graph: terminator: terminal call arm: result binding");
        let planned = operation.call()?;
        if planned.coordinate.statement_index != statement_ordinal {
            return None;
        }
        if *planned.result() == checked_trees::CheckedUnitCallResult::NoNormal {
            control::statement_sequence::consume_results(&mut preparation, &operation)?;
            preparation.drain(..predecessor_count);
            return Some(checked_trees::CheckedConditionalReturnArm::Call {
                operands: checked_trees::CheckedOperandSequence::new(preparation),
                operation,
                completions: checked_trees::CheckedOperandSequence::new(Vec::new()),
            });
        }
        let operation = match (scalar_result, structural_result) {
            (Some(result), None) => bind_scalar_call_result(facts, operation, result, true)?,
            (None, Some(result)) => {
                let mut operation = bind_structural_call_result(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    operation,
                    result.clone(),
                )?;
                let (binding, discard) = operation.call_mut()?.structural_result_mut()?;
                if *binding != result {
                    return None;
                }
                *discard = false;
                operation
            }
            (None, None) if *planned.result() == checked_trees::CheckedUnitCallResult::Unit => {
                operation
            }
            _ => return None,
        };
        trace.phase("state graph: terminator: terminal call arm: result consumption");
        control::statement_sequence::consume_results(&mut preparation, &operation)?;
        let completions = super::super::cleanup::anonymous::normal_call_completion(
            program,
            facts,
            shapes,
            machine,
            state,
            &structural_results,
            &mut preparation,
            &operation,
            trace,
        )?;
        preparation.drain(..predecessor_count);
        Some(checked_trees::CheckedConditionalReturnArm::Call {
            operands: checked_trees::CheckedOperandSequence::new(preparation),
            operation,
            completions: checked_trees::CheckedOperandSequence::new(completions),
        })
    }
}
mod terminator {

    /*
        The terminator of one state in a composed Unit control graph.

        Tried in order: a closed-sum dispatch over the state's final case; a
        scalar return's checked completion; then the state's authored tail --
        a Unit or structural return, a crash exit, a jump, conditional or guarded
        successors, or a terminal machine call. Every successor edge is built by
        `successor` against the state signatures; a terminal call completes the
        machine instead (`terminal_calls`); an unsupported tail names its shape.
        Normal returns do not independently require whole-root no-code cleanup:
        `cleanup::normal_edges` retains each selected exit's exact disposition
        after the graph is built, before it can enter the published call closure.
        That owner admits residual and nominal actions and refuses missing plans.
    */
    use super::{
        CheckFacts, CheckedComposedUnitControlTerminatorPlan as TerminatorPlan,
        CheckedGuardedJumpPlan, CheckedScalarExpressionRole, CheckedStructuralControlSuccessorPlan,
        CheckedUnitEffectOperationPlan as OperationPlan, CheckedUnitStructuralParameterPlan,
        EdgeCalls, FusedStates, ScalarCalleeClosure, ShapeCollector, Signature, StatementNode,
        SuccessorEdge, SuccessorGuard, TransitionExit, TransitionGuardNode, TransitionTargetNode,
        closed_sum, control, guard_is_boolean, retained_guard, returns, successor, terminal_calls,
    };
    use crate::execution::guard_complement::complementary;
    use crate::execution::selected_transitions::selected_transition;
    use checked_trees::CheckedControlResultPlan;
    use checked_trees::CheckedGuardedJumpArmPlan;
    use checked_trees::CheckedGuardedJumpFallbackPlan;
    use checked_trees::CheckedGuardedJumpReturnPlan;
    use checked_trees::CheckedScalarBranchDestination;
    use checked_trees::CheckedScalarStateTerminator;
    use checked_trees::CheckedUnitEntryClaimPlan;
    use checked_trees::CheckedUnitScalarControlPlan;
    use checked_trees::CheckedUnitScalarResultBindingPlan;
    use checked_trees::CheckedUnitStructuralResultBindingPlan;
    use checked_trees::FlowClaimJoinExit;
    use checked_trees::FlowClaimJoinExitKind;
    use checked_trees::FlowPermissionEvaluation;
    use facts::PlaceRoot;
    use symbols::SymbolHandle;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::TableTransition;
    use typed_trees::statement::reachable_transition_prefix;
    use validation::completing_transition_transfer;

    /*
        The terminator one state's authored tail plans, after its operations.
        `fused` carries the whole roster this graph spans: a successor naming a
        foreign machine's entry resolves to that owner's fused states, so the
        edge stays inside the one dispatch loop.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        declarations: &super::loan_backing::DeclaredStoragePlans<'_>,
        records: &super::loan_backing::RetainedBorrowRecords<'_>,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        fused: &FusedStates<'_>,
        state_index: usize,
        state: &State,
        result: &CheckedControlResultPlan,
        signatures: &[Signature],
        entry_claims: &[CheckedUnitEntryClaimPlan],
        local_results: &[(CheckedUnitStructuralResultBindingPlan, PlaceRoot)],
        place_loans: &[checked_trees::CheckedPlaceLoanPlan],
        scalar_result: Option<&CheckedUnitScalarResultBindingPlan>,
        structural_result: Option<&checked_trees::CheckedUnitStructuralReturnPlan>,
        next_scalar_binding: usize,
        operations: &mut Vec<OperationPlan>,
        terminator_index: usize,
        trace: &control::LocalConstructionTrace,
    ) -> Option<(TerminatorPlan, Option<FlowClaimJoinExit>)> {
        if let Some(call) = operations
            .iter()
            .filter_map(OperationPlan::call)
            .find(|call| *call.result() == checked_trees::CheckedUnitCallResult::NoNormal)
        {
            return Some((
                TerminatorPlan::InvokeNoNormal {
                    coordinate: call.coordinate,
                },
                None,
            ));
        }
        if operations
            .iter()
            .any(|operation| !operation.has_normal_continuation())
        {
            return Some((TerminatorPlan::EvaluateNoNormal, None));
        }
        let result = result.clone();
        let &(machine, _) = fused.get(state_index)?;
        let (structural, _) = &signatures[state_index];
        let statements = program.statement_table.statements(state.statement_nodes);
        trace.phase("state graph: terminator");
        let ordinal = u32::try_from(terminator_index).ok()?;
        let edge_calls = EdgeCalls {
            scalar_callees,
            entry_claims,
            local_results,
        };
        let edge = |operations: &[OperationPlan],
                    transition,
                    edge_ordinal,
                    kind,
                    shapes: &mut ShapeCollector<'_>| {
            successor(
                program,
                facts,
                declarations,
                records,
                fused,
                state_index,
                signatures,
                operations,
                transition,
                edge_ordinal,
                kind,
                edge_calls,
                shapes,
                trace,
            )
        };
        let complete_expression =
            |expression, shapes: &mut ShapeCollector<'_>, operations: &mut Vec<OperationPlan>| {
                trace.phase("state graph: terminator: return expression");
                let evaluation = match statements.get(terminator_index)? {
                    StatementNode::Expression(_) => FlowPermissionEvaluation::Statement,
                    StatementNode::Transition(transition) => {
                        FlowPermissionEvaluation::TransitionTarget(transition.target)
                    }
                    _ => return None,
                };
                let completion = checked_trees::CheckedStructuralReturnCompletion::Value {
                    statement_index: ordinal,
                    expression,
                    evaluation,
                };
                if let Some(mut result) = structural_result.cloned() {
                    result.completion = completion;
                    return Some(TerminatorPlan::ReturnStructural { result });
                }
                let mut count = returns::next_result_ordinal(operations)?;
                let (operation, operand_calls) = returns::return_value_operation(
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    machine,
                    state,
                    structural,
                    entry_claims,
                    operations,
                    &mut count,
                    ordinal,
                    expression,
                    trace,
                )?;
                if !operation.has_normal_continuation() {
                    operations.extend(operand_calls);
                    operations.push(operation);
                    return Some(TerminatorPlan::EvaluateNoNormal);
                }
                let mut result = checked_trees::CheckedUnitStructuralReturnPlan::from_binding(
                    operation.structural_result_binding()?.clone(),
                    completion,
                );
                if validation::reference_shell(program, state.return_type).is_some()
                    || validation::reference_result_custody::is_reference_record(
                        program,
                        state.return_type,
                    )
                {
                    trace.phase("state graph: terminator: returned reference sources");
                    trace.statement(Some(ordinal));
                    let sources = validation::reference_result_custody::captured_return_sources(
                        program, facts, state,
                    );
                    if sources.is_none() && ::validation::trace_flags::trace_completion() {
                        eprintln!(
                            "CHECKSITE completion-return-sources-missing machine={} state={} type={:?}",
                            machine.name, state.name, state.return_type
                        );
                    }
                    result.reference_sources = sources?;
                }
                operations.extend(operand_calls);
                operations.push(operation);
                trace.phase("state graph: terminator: return expression finished");
                trace.statement(Some(ordinal));
                Some(TerminatorPlan::ReturnStructural { result })
            };
        if statements.get(terminator_index).is_some_and(|statement| {
            matches!(statement, StatementNode::Transition(transition)
                if matches!(transition.guard, TransitionGuardNode::When(_)))
        }) && let Some(selected) = selected_transition(program, statements, terminator_index)
        {
            trace.phase("state graph: terminator: selected static destination");
            let transition = selected.transition;
            let selected_ordinal = selected.statement_ordinal;
            let selected_exit = FlowClaimJoinExit {
                state_symbol: state.symbol,
                statement_index: selected_ordinal as usize,
                kind: FlowClaimJoinExitKind::TransitionTarget,
            };
            if completing_transition_transfer(program, transition.target).is_some_and(|transfer| {
                super::fused::fused_target_index(program, fused, transfer.target).is_some()
            }) {
                return Some((
                    TerminatorPlan::Jump {
                        successor: edge(
                            operations,
                            transition,
                            selected_ordinal,
                            SuccessorEdge::Jump,
                            shapes,
                        )?,
                    },
                    Some(selected_exit),
                ));
            }
            let arm = guarded_return_arm(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                state,
                structural,
                entry_claims,
                local_results,
                &result,
                next_scalar_binding,
                operations,
                selected_ordinal,
                transition,
                trace,
            )?;
            return Some((
                complete_selected_return(
                    program, facts, machine, state, transition, arm, operations,
                )?,
                Some(selected_exit),
            ));
        }
        let mut unit_completion_failure = trace.deferred_failure();
        if result == CheckedControlResultPlan::Unit {
            // A completed statement prefix has no expression left. Its empty
            // tail still reaches the ordinary implicit Unit return below.
            let expression = match statements.get(terminator_index..)?.first() {
                Some(StatementNode::Expression(expression)) => Some(*expression),
                Some(StatementNode::Transition(transition))
                    if transition.guard == TransitionGuardNode::Always
                        && transition.exit == TransitionExit::Ordinary
                        && !transition.continuation.is_valid() =>
                {
                    match program.statement_table.transition_target(transition.target) {
                        TransitionTargetNode::Value(expression) => Some(*expression),
                        _ => None,
                    }
                }
                _ => None,
            };
            let before_unit_completion = trace.mark();
            if let Some(completion) = expression.and_then(|expression| {
                super::unit_completions::build(
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    machine,
                    state,
                    structural,
                    entry_claims,
                    local_results,
                    operations,
                    ordinal,
                    expression,
                    trace,
                )
            }) {
                return Some((TerminatorPlan::CompleteExpression { completion }, None));
            }
            if trace.mark() != before_unit_completion {
                unit_completion_failure.retain();
            }
        }
        let before_closed_sum = trace.mark();
        let closed_sum = closed_sum::build(
            program,
            facts,
            declarations,
            records,
            shapes,
            fused,
            state_index,
            signatures,
            edge_calls,
            place_loans,
            &result,
            next_scalar_binding,
            operations,
            terminator_index,
            trace,
        );
        let closed_sum_failure =
            (closed_sum.is_none() && trace.mark() != before_closed_sum).then(|| trace.mark());
        let terminator = if let Some(terminator) = closed_sum {
            terminator
        } else if let CheckedControlResultPlan::Scalar {
            scalar_type: primitive_type,
        } = result
            && let Some(completion) = returns::scalar_completion(
                program,
                fused,
                facts,
                machine,
                state,
                terminator_index,
                primitive_type,
                scalar_result,
                trace,
            )
        {
            trace.phase("state graph: terminator: scalar return cleanup");
            TerminatorPlan::ReturnScalar { completion }
        } else if let Some((terminator, operand_calls)) = returns::guarded(
            program,
            fused,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural,
            entry_claims,
            operations,
            terminator_index,
            trace,
        ) {
            operations.extend(operand_calls);
            terminator
        } else {
            match reachable_transition_prefix(&statements[terminator_index..]) {
                [] if result == CheckedControlResultPlan::Unit => {
                    trace.phase("state graph: terminator: unit tail cleanup");
                    TerminatorPlan::ReturnUnit
                }
                [StatementNode::Expression(expression)]
                    if result != CheckedControlResultPlan::Unit =>
                {
                    complete_expression(*expression, shapes, operations)?
                }
                [StatementNode::Transition(transition)]
                    if matches!(transition.exit, TransitionExit::Crash(_)) =>
                {
                    trace.phase("state graph: terminator: crash exit");
                    TerminatorPlan::Crash {
                        statement_ordinal: ordinal,
                    }
                }
                [StatementNode::Transition(transition)]
                    if transition.guard == TransitionGuardNode::Always =>
                {
                    match program.statement_table.transition_target(transition.target) {
                        TransitionTargetNode::Terminal
                            if result == CheckedControlResultPlan::Unit
                                && transition.exit == TransitionExit::Ordinary
                                && !transition.continuation.is_valid() =>
                        {
                            trace.phase("state graph: terminator: explicit unit completion");
                            TerminatorPlan::ReturnUnit
                        }
                        TransitionTargetNode::Value(_)
                            if completing_transition_transfer(program, transition.target)
                                .is_some_and(|transfer| {
                                    super::fused::fused_target_index(
                                        program,
                                        fused,
                                        transfer.target,
                                    )
                                    .is_some()
                                }) =>
                        {
                            TerminatorPlan::Jump {
                                successor: edge(
                                    operations,
                                    transition,
                                    ordinal,
                                    SuccessorEdge::Jump,
                                    shapes,
                                )?,
                            }
                        }
                        TransitionTargetNode::Named { .. } | TransitionTargetNode::Value(_)
                            if terminal_calls::external_callable(
                                program, machine, fused, transition,
                            )
                            .is_some()
                                && terminal_calls::completion(
                                    operations,
                                    ordinal,
                                    &result,
                                    scalar_result,
                                    structural_result,
                                )
                                .is_some() =>
                        {
                            trace.phase("state graph: terminator: terminal call completion");
                            terminal_calls::completion(
                                operations,
                                ordinal,
                                &result,
                                scalar_result,
                                structural_result,
                            )?
                        }
                        TransitionTargetNode::Value(expression)
                            if result != CheckedControlResultPlan::Unit
                                && transition.exit == TransitionExit::Ordinary
                                && !transition.continuation.is_valid() =>
                        {
                            complete_expression(*expression, shapes, operations)?
                        }
                        _ => {
                            trace.phase("state graph: terminator: jump successor");
                            TerminatorPlan::Jump {
                                successor: edge(
                                    operations,
                                    transition,
                                    ordinal,
                                    SuccessorEdge::Jump,
                                    shapes,
                                )?,
                            }
                        }
                    }
                }
                [
                    StatementNode::Transition(when_true),
                    StatementNode::Transition(when_false),
                ] if matches!(when_true.guard, TransitionGuardNode::When(_))
                    && (when_false.guard == TransitionGuardNode::Always
                        || complementary(
                            program,
                            &facts.values.scalar_expressions,
                            state,
                            ordinal,
                        )
                        || covering_pair(facts, state.symbol, ordinal).is_some()) =>
                {
                    if result == CheckedControlResultPlan::Unit
                        && transition_on_erased_dispatch_subject(program, state, when_true)
                        && transition_on_erased_dispatch_subject(program, state, when_false)
                    {
                        unit_completion_failure.succeeded();
                        return Some(erased_proof_completion(state, statements, terminator_index));
                    }
                    trace
                        .phase("state graph: terminator: conditional successors: guard expression");
                    let guard = retained_guard(facts, machine.symbol, state.symbol, ordinal)?;
                    if !guard_is_boolean(facts, &guard) {
                        trace.phase("state graph: terminator: conditional successors: guard type");
                        return None;
                    }
                    let mut return_count = returns::next_result_ordinal(operations)?;
                    let mut branch = |transition: &TableTransition,
                                      edge_ordinal: u32|
                     -> Option<
                        Result<
                            CheckedStructuralControlSuccessorPlan,
                            (
                                checked_trees::CheckedConditionalReturnArm,
                                Vec<OperationPlan>,
                            ),
                        >,
                    > {
                        if matches!(transition.exit, TransitionExit::Crash(_)) {
                            return crash_arm(program, transition, edge_ordinal)
                                .map(|arm| Err((arm, Vec::new())));
                        }
                        if matches!(
                            program.statement_table.transition_target(transition.target),
                            TransitionTargetNode::Terminal
                        ) && result == CheckedControlResultPlan::Unit
                            && transition.exit == TransitionExit::Ordinary
                            && !transition.continuation.is_valid()
                        {
                            return Some(Err((
                                checked_trees::CheckedConditionalReturnArm::Unit {
                                    statement_ordinal: edge_ordinal,
                                },
                                Vec::new(),
                            )));
                        }
                        if completing_transition_transfer(program, transition.target).is_some_and(
                            |transfer| {
                                super::fused::fused_target_index(program, fused, transfer.target)
                                    .is_some()
                            },
                        ) {
                            return successor(
                                program,
                                facts,
                                declarations,
                                records,
                                fused,
                                state_index,
                                signatures,
                                operations,
                                transition,
                                edge_ordinal,
                                SuccessorEdge::Conditional,
                                edge_calls,
                                shapes,
                                trace,
                            )
                            .map(Ok);
                        }
                        if terminal_calls::external_callable(program, machine, fused, transition)
                            .is_some()
                        {
                            return terminal_calls::arm_call(
                                program,
                                facts,
                                scalar_callees,
                                shapes,
                                machine,
                                state,
                                structural,
                                entry_claims,
                                local_results,
                                operations,
                                &result,
                                next_scalar_binding,
                                transition,
                                edge_ordinal,
                                trace,
                            )
                            .map(|arm| Err((arm, Vec::new())));
                        }
                        if let TransitionTargetNode::Value(expression) =
                            program.statement_table.transition_target(transition.target)
                        {
                            if transition.exit != TransitionExit::Ordinary
                                || transition.continuation.is_valid()
                            {
                                trace.phase(
                                    SuccessorEdge::Conditional.phase(SuccessorGuard::TargetState),
                                );
                                return None;
                            }
                            trace.phase(
                                "state graph: terminator: conditional successors: return arm value",
                            );
                            trace.statement(Some(edge_ordinal));
                            if let CheckedControlResultPlan::Scalar {
                                scalar_type: primitive_type,
                            } = result
                            {
                                let role = CheckedScalarExpressionRole::Return;
                                let retained = facts
                                    .values
                                    .scalar_expressions
                                    .expression_at(state.symbol, edge_ordinal, role)
                                    .is_some()
                                    || facts
                                        .values
                                        .scalar_computations
                                        .root_at(state.symbol, edge_ordinal, role)
                                        .is_some_and(|root| root.machine == machine.symbol);
                                return retained.then_some(Err((
                                    checked_trees::CheckedConditionalReturnArm::Scalar {
                                        statement_ordinal: edge_ordinal,
                                        scalar_type: primitive_type.into(),
                                    },
                                    Vec::new(),
                                )));
                            }
                            return returns::return_value_operation(
                                program,
                                facts,
                                scalar_callees,
                                shapes,
                                machine,
                                state,
                                structural,
                                entry_claims,
                                operations,
                                &mut return_count,
                                edge_ordinal,
                                *expression,
                                trace,
                            )
                            .map(|(operation, operand_calls)| {
                                Err((
                                    checked_trees::CheckedConditionalReturnArm::Structural(
                                        operation,
                                    ),
                                    operand_calls,
                                ))
                            });
                        }
                        successor(
                            program,
                            facts,
                            declarations,
                            records,
                            fused,
                            state_index,
                            signatures,
                            operations,
                            transition,
                            edge_ordinal,
                            SuccessorEdge::Conditional,
                            edge_calls,
                            shapes,
                            trace,
                        )
                        .map(Ok)
                    };
                    match (
                        branch(when_true, ordinal)?,
                        branch(when_false, ordinal.checked_add(1)?)?,
                    ) {
                        (Ok(when_true), Ok(when_false)) => TerminatorPlan::Conditional {
                            guard,
                            when_true,
                            when_false,
                        },
                        (Ok(jump), Err((return_arm, operand_calls))) => {
                            operations.extend(operand_calls);
                            TerminatorPlan::ConditionalReturn {
                                guard,
                                jump,
                                return_arm,
                                return_when_true: false,
                            }
                        }
                        (Err((return_arm, operand_calls)), Ok(jump)) => {
                            operations.extend(operand_calls);
                            TerminatorPlan::ConditionalReturn {
                                guard,
                                jump,
                                return_arm,
                                return_when_true: true,
                            }
                        }
                        (Err((when_true, true_operands)), Err((when_false, false_operands))) => {
                            operations.extend(true_operands);
                            operations.extend(false_operands);
                            TerminatorPlan::ConditionalCompletion {
                                guard,
                                when_true,
                                when_false,
                            }
                        }
                    }
                }
                tail @ [
                    StatementNode::Transition(_),
                    StatementNode::Transition(_),
                    StatementNode::Transition(_),
                    ..,
                ] if tail
                    .iter()
                    .all(|statement| matches!(statement, StatementNode::Transition(_)))
                    && tail[..tail.len() - 1].iter().all(|statement| {
                        matches!(statement, StatementNode::Transition(transition)
                            if matches!(transition.guard, TransitionGuardNode::When(_)))
                    })
                    && matches!(tail.last(), Some(StatementNode::Transition(transition))
                    if matches!(
                        transition.guard,
                        TransitionGuardNode::Always | TransitionGuardNode::When(_)
                    )) =>
                {
                    if result == CheckedControlResultPlan::Unit
                        && tail.iter().all(|statement| {
                            statement.as_transition().is_some_and(|transition| {
                                transition_on_erased_dispatch_subject(program, state, transition)
                            })
                        })
                    {
                        unit_completion_failure.succeeded();
                        return Some(erased_proof_completion(state, statements, terminator_index));
                    }
                    trace.phase("state graph: terminator: guarded jump successors: roster");
                    let last_guarded = matches!(
                        tail.last(),
                        Some(StatementNode::Transition(transition))
                            if matches!(transition.guard, TransitionGuardNode::When(_))
                    );
                    let retained = facts
                        .flow
                        .guarded_exits
                        .tails
                        .iter()
                        .filter(|tail| tail.state == state.symbol)
                        .collect::<Vec<_>>();
                    let [retained] = retained.as_slice() else {
                        return None;
                    };
                    let exits = facts.flow.guarded_exits.exits.span(retained.arms)?;
                    if exits.len() != tail.len() - usize::from(!last_guarded)
                        || last_guarded != retained.fallback.is_none()
                    {
                        return None;
                    }
                    let mut arms = Vec::with_capacity(exits.len());
                    for (index, (statement, exit)) in
                        tail[..tail.len() - 1].iter().zip(exits.iter()).enumerate()
                    {
                        let transition = statement.as_transition()?;
                        let arm_ordinal = ordinal.checked_add(u32::try_from(index).ok()?)?;
                        if exit.guard_statement_ordinal != arm_ordinal {
                            return None;
                        }
                        let jump_target = internal_destination(
                            program,
                            machine,
                            fused,
                            transition,
                            &exit.destination,
                            arm_ordinal,
                        )?;
                        trace.phase(
                            "state graph: terminator: guarded jump successors: guard expression",
                        );
                        trace.statement(Some(arm_ordinal));
                        let guard =
                            retained_guard(facts, machine.symbol, state.symbol, arm_ordinal)?;
                        if !guard_is_boolean(facts, &guard) {
                            return None;
                        }
                        let arm = if jump_target.is_valid() {
                            let target = jump_target;
                            let successor = edge(
                                operations,
                                transition,
                                arm_ordinal,
                                SuccessorEdge::GuardedJump,
                                shapes,
                            )?;
                            if successor.target_state != target {
                                trace.phase(
                                    SuccessorEdge::GuardedJump.phase(SuccessorGuard::TargetState),
                                );
                                trace.statement(Some(arm_ordinal));
                                return None;
                            }
                            CheckedGuardedJumpArmPlan::Jump(CheckedGuardedJumpPlan {
                                guard,
                                successor,
                            })
                        } else {
                            let return_arm = guarded_return_arm(
                                program,
                                facts,
                                scalar_callees,
                                shapes,
                                machine,
                                state,
                                structural,
                                entry_claims,
                                local_results,
                                &result,
                                next_scalar_binding,
                                operations,
                                arm_ordinal,
                                transition,
                                trace,
                            )?;
                            CheckedGuardedJumpArmPlan::Return(CheckedGuardedJumpReturnPlan {
                                guard,
                                statement_ordinal: arm_ordinal,
                                return_arm,
                            })
                        };
                        arms.push(arm);
                    }
                    let Some(StatementNode::Transition(fallback_transition)) = tail.last() else {
                        return None;
                    };
                    let fallback_ordinal =
                        ordinal.checked_add(u32::try_from(tail.len() - 1).ok()?)?;
                    let fallback_destination = if last_guarded {
                        exits.last().map(|exit| &exit.destination)
                    } else {
                        retained.fallback.as_ref()
                    };
                    if last_guarded
                        && exits
                            .last()
                            .is_none_or(|exit| exit.guard_statement_ordinal != fallback_ordinal)
                    {
                        return None;
                    }
                    let fallback = match internal_destination(
                        program,
                        machine,
                        fused,
                        fallback_transition,
                        fallback_destination?,
                        fallback_ordinal,
                    )? {
                        target if target.is_valid() => {
                            let fallback = edge(
                                operations,
                                fallback_transition,
                                fallback_ordinal,
                                SuccessorEdge::GuardedJump,
                                shapes,
                            )?;
                            if fallback.target_state != target {
                                trace.phase(
                                    SuccessorEdge::GuardedJump.phase(SuccessorGuard::TargetState),
                                );
                                trace.statement(Some(fallback_ordinal));
                                return None;
                            }
                            CheckedGuardedJumpFallbackPlan::Successor(fallback)
                        }
                        _ => {
                            trace.phase(
                                "state graph: terminator: guarded jump successors: return completion",
                            );
                            trace.statement(Some(fallback_ordinal));
                            CheckedGuardedJumpFallbackPlan::Return(guarded_return_arm(
                                program,
                                facts,
                                scalar_callees,
                                shapes,
                                machine,
                                state,
                                structural,
                                entry_claims,
                                local_results,
                                &result,
                                next_scalar_binding,
                                operations,
                                fallback_ordinal,
                                fallback_transition,
                                trace,
                            )?)
                        }
                    };
                    TerminatorPlan::GuardedJumps { arms, fallback }
                }
                _ => {
                    trace.phase(match &statements[terminator_index..] {
                        [] => "state graph: terminator: unsupported tail: missing return value",
                        [StatementNode::Expression(_)] => {
                            "state graph: terminator: unsupported tail: expression statement with unit result"
                        }
                        [StatementNode::Transition(transition)] => match transition.exit {
                            TransitionExit::Ordinary => {
                                "state graph: terminator: unsupported tail: single guarded transition"
                            }
                            TransitionExit::Crash(_) => {
                                "state graph: terminator: unsupported tail: single guarded crash exit"
                            }
                        },
                        [StatementNode::Transition(first), StatementNode::Transition(_)]
                            if first.guard == TransitionGuardNode::Always =>
                        {
                            "state graph: terminator: unsupported tail: jump followed by a transition"
                        }
                        [StatementNode::Transition(_), StatementNode::Transition(_)] => {
                            "state graph: terminator: unsupported tail: guarded pair without exact false fallback"
                        }
                        tail @ [
                            StatementNode::Transition(_),
                            StatementNode::Transition(_),
                            StatementNode::Transition(_),
                            ..,
                        ] if tail
                            .iter()
                            .all(|statement| matches!(statement, StatementNode::Transition(_))) =>
                        {
                            "state graph: terminator: unsupported tail: transition chain"
                        }
                        [StatementNode::Transition(_), ..] => {
                            "state graph: terminator: unsupported tail: transition followed by statements"
                        }
                        [StatementNode::Expression(_), ..] => {
                            "state graph: terminator: unsupported tail: expression followed by statements"
                        }
                        _ => "state graph: terminator: unsupported tail: other statement tail",
                    });
                    if let Some(mark) = closed_sum_failure {
                        trace.restore(&mark);
                    }
                    return None;
                }
            }
        };
        unit_completion_failure.succeeded();
        Some((terminator, None))
    }

    /*
        Rejoin the shared roster's authored destination, distinguishing a local
        state edge from completion through a value or another machine's result.
        Only the composed graph's explicit fused roster makes a foreign machine
        entry an internal edge; the scalar branch roster alone cannot do so.
    */
    fn internal_destination(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        fused: &FusedStates<'_>,
        transition: &TableTransition,
        destination: &CheckedScalarBranchDestination,
        ordinal: u32,
    ) -> Option<SymbolHandle> {
        if let CheckedScalarBranchDestination::Crash { statement_ordinal } = destination {
            crash_arm(program, transition, ordinal)?;
            return (*statement_ordinal == ordinal).then_some(SymbolHandle::default());
        }
        if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
            return None;
        }
        if let Some(transfer) = completing_transition_transfer(program, transition.target)
            && super::fused::fused_target_index(program, fused, transfer.target).is_some()
        {
            return Some(transfer.state.symbol);
        }
        match destination {
            CheckedScalarBranchDestination::Jump(selected)
                if selected.statement_ordinal == ordinal && !selected.is_continuation =>
            {
                match terminal_calls::external_callable(program, machine, fused, transition) {
                    Some(target) => {
                        (target == selected.target_state).then_some(SymbolHandle::default())
                    }
                    None => Some(selected.target_state),
                }
            }
            CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation: false,
            }
            | CheckedScalarBranchDestination::InvokeNoNormal {
                statement_ordinal,
                is_continuation: false,
            } if *statement_ordinal == ordinal => Some(SymbolHandle::default()),
            _ => None,
        }
    }

    /*
        The completion an ordered-chain arm selects: a mid-chain arm and the closing
        fallback use this same join. Unit exits preserve ordinary return cleanup,
        without manufacturing a value. A scalar arm rejoins the
        `Return`-role value checking retained at its exact statement coordinate; a
        structural arm mints the `StructuralValue` local-binding producer (its operand
        calls join the state's operations; the producer itself stays inside the
        arm so `operation_dependencies` reports it exactly once).
    */
    #[allow(clippy::too_many_arguments)]
    fn guarded_return_arm(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        machine: &Machine,
        state: &State,
        structural: &[CheckedUnitStructuralParameterPlan],
        entry_claims: &[CheckedUnitEntryClaimPlan],
        local_results: &[(CheckedUnitStructuralResultBindingPlan, PlaceRoot)],
        result: &CheckedControlResultPlan,
        next_scalar_binding: usize,
        operations: &mut Vec<OperationPlan>,
        statement_ordinal: u32,
        transition: &TableTransition,
        trace: &control::LocalConstructionTrace,
    ) -> Option<checked_trees::CheckedConditionalReturnArm> {
        if matches!(transition.exit, TransitionExit::Crash(_)) {
            return crash_arm(program, transition, statement_ordinal);
        }
        if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
            return None;
        }
        if terminal_calls::invoked_callable(program, machine, transition).is_some() {
            return terminal_calls::arm_call(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                state,
                structural,
                entry_claims,
                local_results,
                operations,
                result,
                next_scalar_binding,
                transition,
                statement_ordinal,
                trace,
            );
        }
        let expression = match program.statement_table.transition_target(transition.target) {
            TransitionTargetNode::Terminal if *result == CheckedControlResultPlan::Unit => {
                return Some(checked_trees::CheckedConditionalReturnArm::Unit {
                    statement_ordinal,
                });
            }
            TransitionTargetNode::Value(expression) => *expression,
            _ => return None,
        };
        if *result == CheckedControlResultPlan::Unit {
            return super::unit_completions::build(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                state,
                structural,
                entry_claims,
                local_results,
                operations,
                statement_ordinal,
                expression,
                trace,
            )
            .map(checked_trees::CheckedConditionalReturnArm::Completion);
        }
        if let CheckedControlResultPlan::Scalar {
            scalar_type: primitive_type,
        } = result
        {
            let role = CheckedScalarExpressionRole::Return;
            let retained = facts
                .values
                .scalar_expressions
                .expression_at(state.symbol, statement_ordinal, role)
                .is_some()
                || facts
                    .values
                    .scalar_computations
                    .root_at(state.symbol, statement_ordinal, role)
                    .is_some_and(|root| root.machine == machine.symbol);
            if !retained {
                return None;
            }
            Some(checked_trees::CheckedConditionalReturnArm::Scalar {
                statement_ordinal,
                scalar_type: *primitive_type,
            })
        } else {
            let mut return_count = returns::next_result_ordinal(operations)?;
            let (operation, operand_calls) = returns::return_value_operation(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                state,
                structural,
                entry_claims,
                operations,
                &mut return_count,
                statement_ordinal,
                expression,
                trace,
            )?;
            operations.extend(operand_calls);
            Some(checked_trees::CheckedConditionalReturnArm::Structural(
                operation,
            ))
        }
    }

    /*
        The shared scalar roster's coverage evidence for a two-statement guarded
        tail at `ordinal`: a retained pair records only when its guards provably
        close the dispatch (an exact complement or complete case coverage of one
        subject), so the second guard is the vacuous else the terminator admits.
    */
    fn covering_pair(facts: &CheckFacts, state: SymbolHandle, ordinal: u32) -> Option<()> {
        let retained = facts
            .flow
            .guarded_exits
            .tails
            .iter()
            .filter(|tail| tail.state == state)
            .collect::<Vec<_>>();
        let [retained] = retained.as_slice() else {
            return None;
        };
        let exits = facts.flow.guarded_exits.exits.span(retained.arms)?;
        (retained.fallback.is_none()
            && exits.len() == 2
            && exits[0].guard_statement_ordinal == ordinal
            && exits[1].guard_statement_ordinal == ordinal + 1)
            .then_some(())
    }

    /*
        Crash completion retains its authored occurrence without fabricating a
        value, successor, or ordinary return-cleanup obligation.
    */
    fn crash_arm(
        program: &dyn ::validation::CheckingInput,
        transition: &TableTransition,
        statement_ordinal: u32,
    ) -> Option<checked_trees::CheckedConditionalReturnArm> {
        (matches!(transition.exit, TransitionExit::Crash(_))
            && !transition.continuation.is_valid()
            && matches!(
                program.statement_table.transition_target(transition.target),
                TransitionTargetNode::Terminal
            ))
        .then_some(checked_trees::CheckedConditionalReturnArm::Crash { statement_ordinal })
    }

    /*
        Once selection is closed, the existing arm transaction becomes this state's
        completion. Preserve its operand order, call coordinate and post-call loan
        transaction; normal-edge cleanup still runs on the resulting terminator.
    */
    fn complete_selected_return(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        transition: &TableTransition,
        arm: checked_trees::CheckedConditionalReturnArm,
        operations: &mut Vec<OperationPlan>,
    ) -> Option<TerminatorPlan> {
        use checked_trees::{
            CheckedConditionalReturnArm as Arm, CheckedScalarReturnPlan,
            CheckedStructuralReturnCompletion, CheckedUnitCallResult,
            CheckedUnitStructuralReturnPlan,
        };
        Some(match arm {
            Arm::EvaluateNoNormal {
                operations: prefix, ..
            } => {
                operations.extend(prefix.iter().cloned());
                TerminatorPlan::EvaluateNoNormal
            }
            Arm::EvaluateUnit {
                operations: prefix, ..
            } => {
                operations.extend(prefix.iter().cloned());
                TerminatorPlan::ReturnUnit
            }
            Arm::Crash { statement_ordinal } => TerminatorPlan::Crash { statement_ordinal },
            Arm::Completion(completion) => TerminatorPlan::CompleteExpression { completion },
            Arm::Unit { .. } => TerminatorPlan::ReturnUnit,
            Arm::Scalar {
                statement_ordinal,
                scalar_type,
            } => TerminatorPlan::ReturnScalar {
                completion: CheckedScalarReturnPlan::Exits(CheckedUnitScalarControlPlan {
                    scalar_type,
                    terminator: CheckedScalarStateTerminator::Return { statement_ordinal },
                    selected_copies: vec![
                        control::selected_copies::SelectedCopies::new(
                            program,
                            facts,
                            machine.symbol,
                            state,
                        )?
                        .for_destination(&checked_trees::CheckedScalarBranchDestination::Return {
                            statement_ordinal,
                            is_continuation: false,
                        })?
                        .1,
                    ],
                }),
            },
            Arm::Structural(operation) => {
                if !operation.has_normal_continuation() {
                    operations.push(operation);
                    return Some(TerminatorPlan::EvaluateNoNormal);
                }
                let binding = operation.structural_result_binding()?.clone();
                let TransitionTargetNode::Value(expression) =
                    program.statement_table.transition_target(transition.target)
                else {
                    return None;
                };
                let completion = CheckedStructuralReturnCompletion::Value {
                    statement_index: binding.statement_index,
                    expression: *expression,
                    evaluation: FlowPermissionEvaluation::TransitionTarget(transition.target),
                };
                let result = CheckedUnitStructuralReturnPlan::from_binding(binding, completion);
                operations.push(operation);
                TerminatorPlan::ReturnStructural { result }
            }
            Arm::Call {
                operands,
                operation,
                completions,
            } => {
                let call = operation.call()?;
                let coordinate = call.coordinate;
                let completion = match call.result() {
                    CheckedUnitCallResult::NoNormal => {
                        TerminatorPlan::InvokeNoNormal { coordinate }
                    }
                    CheckedUnitCallResult::Unit => TerminatorPlan::ReturnUnit,
                    CheckedUnitCallResult::Scalar(binding) => TerminatorPlan::ReturnScalar {
                        completion: CheckedScalarReturnPlan::Binding(*binding),
                    },
                    CheckedUnitCallResult::Structural { binding, .. } => {
                        TerminatorPlan::ReturnStructural {
                            result: CheckedUnitStructuralReturnPlan::from_binding(
                                binding.clone(),
                                CheckedStructuralReturnCompletion::Call { coordinate },
                            ),
                        }
                    }
                };
                operations.extend(operands.iter().cloned());
                operations.push(operation);
                operations.extend(completions.iter().cloned());
                completion
            }
        })
    }

    /*
        Unit completion of a proof transition. Cleanup keeps the first arm's
        coordinate so the return has one source exit; the arms are not executed.
    */
    fn erased_proof_completion(
        state: &State,
        statements: &[StatementNode],
        terminator_index: usize,
    ) -> (TerminatorPlan, Option<FlowClaimJoinExit>) {
        let selected_exit = statements[terminator_index..]
            .iter()
            .position(|statement| statement.as_transition().is_some())
            .map(|offset| FlowClaimJoinExit {
                state_symbol: state.symbol,
                statement_index: terminator_index + offset,
                kind: FlowClaimJoinExitKind::TransitionTarget,
            });
        (TerminatorPlan::ReturnUnit, selected_exit)
    }

    /*
        A `When` guard that only classifies an erased dispatch subject, or the
        `Always` fallback of that same tail. The subject is proof identity.
    */
    fn transition_on_erased_dispatch_subject(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        transition: &TableTransition,
    ) -> bool {
        let TransitionGuardNode::When(expression) = transition.guard else {
            return true;
        };
        expression_names_erased_dispatch_subject(program, state, expression)
    }

    fn expression_names_erased_dispatch_subject(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        mut expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        use typed_trees::expression::ExpressionNode;
        for _ in 0..8 {
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(path) => {
                    let Some((_, local)) = program.state_local_declaration(state, path.symbol)
                    else {
                        return false;
                    };
                    return validation::dispatch_subject_of_erased_binding(program, state, local);
                }
                ExpressionNode::Borrow(borrow) => expression = borrow.target,
                ExpressionNode::Member(member) => expression = member.receiver,
                ExpressionNode::Binary(binary) => expression = binary.left,
                ExpressionNode::Unary(unary) => expression = unary.operand,
                ExpressionNode::SavedSubject(_) => return false,
                _ => return false,
            }
        }
        false
    }
}
mod unit_completions {
    //! Unit completion retains a selected invocation graph without result storage.
    //! Every leaf uses the same ordinary call transaction as a transition arm.

    use super::{CheckFacts, ScalarCalleeClosure, ShapeCollector, control};
    use checked_trees::{
        CheckedCompletionExpressionKind as Kind, CheckedConditionalReturnArm as Arm,
        CheckedControlResultPlan, CheckedUnitCompletionExpression, CheckedUnitCompletionKind,
        CheckedUnitCompletionPlan, CheckedUnitCompletionSelection, CheckedUnitEffectOperationPlan,
        CheckedUnitScalarCompletionArm, CheckedUnitScalarCompletionCondition,
        CheckedUnitStructuralCompletionArm, CheckedUnitStructuralParameterPlan,
    };
    use std::collections::BTreeSet;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn build(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural: &[CheckedUnitStructuralParameterPlan],
        entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
        local_results: &[(
            checked_trees::CheckedUnitStructuralResultBindingPlan,
            facts::PlaceRoot,
        )],
        preamble: &[CheckedUnitEffectOperationPlan],
        statement: u32,
        expression: typed_trees::expression::ExpressionHandle,
        trace: &control::LocalConstructionTrace,
    ) -> Option<CheckedUnitCompletionPlan> {
        let plans = &facts.values.completions;
        let root = plans.root_at(machine.symbol, state.symbol, statement, expression)?;
        if let Some(refusal) = root.refusal {
            trace.phase(refusal.reason.phase());
            trace.statement(Some(statement));
            return None;
        }
        trace.phase("Unit completion: checked dispatch root");
        trace.statement(Some(statement));
        if !matches!(plans.nodes.get(root.root).kind, Kind::Dispatch { .. }) {
            return None;
        }
        let flow = super::super::types::state_flow(facts, machine.symbol, state.symbol)?;
        let call_roster = facts.flow.control.calls.span_or_empty(flow.calls);
        trace.phase("Unit completion: shared occurrence prefix");
        trace.statement(Some(statement));
        let mut selected =
            control::SelectedInvocationAccounting::new(program, facts, root, preamble)?;
        let mut bind_call =
            |node: &checked_trees::CheckedCompletionExpression,
             source_call: arena::Handle<checked_trees::FlowCallFact>| {
                trace.phase("Unit completion: exact selected call occurrence");
                trace.statement(Some(statement));
                let offset = source_call
                    .arena_index()
                    .checked_sub(flow.calls.start().arena_index())?;
                if flow.calls.handles().nth(offset as usize) != Some(source_call) {
                    return None;
                }
                let call = facts.flow.control.calls.get(source_call);
                if call.statement_index != statement as usize
                    || call.authored_expression != node.expression
                {
                    return None;
                }
                let arm = super::terminal_calls::plan_call_arm(
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    machine,
                    state,
                    structural,
                    entry_claims,
                    local_results,
                    preamble,
                    &CheckedControlResultPlan::Unit,
                    0,
                    call_roster,
                    Some(&mut selected),
                    call,
                    call.target_symbol,
                    statement,
                    trace,
                )?;
                let operations = match arm {
                    Arm::EvaluateNoNormal { operations, .. } => operations,
                    Arm::Call {
                        operands,
                        operation,
                        completions,
                    } => {
                        if !matches!(
                            operation.call()?.result(),
                            checked_trees::CheckedUnitCallResult::Unit
                                | checked_trees::CheckedUnitCallResult::NoNormal
                        ) {
                            return None;
                        }
                        checked_trees::CheckedOperandSequence::new(
                            operands
                                .iter()
                                .chain(std::iter::once(&operation))
                                .chain(completions.iter())
                                .cloned(),
                        )
                    }
                    _ => return None,
                };
                Some(operations)
            };
        let expression = bind_expression(
            plans,
            root.root,
            &mut BTreeSet::new(),
            &mut bind_call,
            trace,
            statement,
        )?;
        trace.phase("Unit completion: complete occurrence inventory");
        trace.statement(Some(statement));
        selected.finish(program, facts, preamble)?;
        Some(CheckedUnitCompletionPlan {
            statement_ordinal: statement,
            expression,
            // A union of leaf outcomes supplies no abandonment proof for earlier
            // subject or pattern execution. Each call retains its exact outcome.
            abandons_custody: false,
        })
    }

    fn bind_expression(
        plans: &checked_trees::CheckedCompletionPlans,
        handle: checked_trees::CheckedCompletionExpressionHandle,
        active: &mut BTreeSet<(u32, u32)>,
        bind_call: &mut impl FnMut(
            &checked_trees::CheckedCompletionExpression,
            arena::Handle<checked_trees::FlowCallFact>,
        ) -> Option<checked_trees::CheckedOperandSequence>,
        trace: &control::LocalConstructionTrace,
        statement: u32,
    ) -> Option<CheckedUnitCompletionExpression> {
        trace.phase("Unit completion: selected expression binding");
        trace.statement(Some(statement));
        let key = (handle.arena_index(), handle.generation());
        if !handle.is_valid() || !active.insert(key) {
            return None;
        }
        let node = plans.nodes.get(handle);
        let kind = match &node.kind {
            Kind::Missing => return None,
            Kind::Invocation { source_call } => {
                CheckedUnitCompletionKind::Invocation(bind_call(node, *source_call)?)
            }
            Kind::Dispatch { subject, arms } => {
                let arms = plans.arms.span(*arms)?;
                if arms.is_empty() {
                    return None;
                }
                match subject {
                    checked_trees::CheckedDispatchSubject::Scalar(subject) => {
                        let mut selected = Vec::with_capacity(arms.len());
                        for arm in arms {
                            use checked_trees::{
                                CheckedCompletionComparison as Comparison,
                                CheckedScalarDispatchPattern as Pattern,
                            };
                            let condition = match (&arm.pattern, arm.comparison) {
                                (Pattern::Value(value), Comparison::Boolean) => {
                                    CheckedUnitScalarCompletionCondition::Boolean(*value)
                                }
                                (Pattern::Value(value), Comparison::Integer) => {
                                    CheckedUnitScalarCompletionCondition::Integer(*value)
                                }
                                (Pattern::Predicate(value), Comparison::Predicate) => {
                                    CheckedUnitScalarCompletionCondition::Predicate(*value)
                                }
                                (Pattern::Wildcard, Comparison::Predicate) => {
                                    CheckedUnitScalarCompletionCondition::Default
                                }
                                _ => return None,
                            };
                            selected.push(CheckedUnitScalarCompletionArm {
                                source_arm: arm.source_arm,
                                condition,
                                completion: bind_expression(
                                    plans,
                                    arm.completion,
                                    active,
                                    bind_call,
                                    trace,
                                    statement,
                                )?,
                            });
                        }
                        CheckedUnitCompletionKind::Selection(
                            CheckedUnitCompletionSelection::Scalar {
                                subject: *subject,
                                arms: selected,
                            },
                        )
                    }
                    checked_trees::CheckedDispatchSubject::Structural {
                        expression,
                        value,
                        place,
                        alias,
                    } => {
                        let mut selected = Vec::with_capacity(arms.len());
                        for arm in arms {
                            let predicate = match (&arm.pattern, arm.comparison) {
                                (
                                    checked_trees::CheckedScalarDispatchPattern::Predicate(value),
                                    checked_trees::CheckedCompletionComparison::Predicate,
                                ) => Some(*value),
                                (
                                    checked_trees::CheckedScalarDispatchPattern::Wildcard,
                                    checked_trees::CheckedCompletionComparison::Predicate,
                                ) => None,
                                _ => return None,
                            };
                            selected.push(CheckedUnitStructuralCompletionArm {
                                source_arm: arm.source_arm,
                                predicate,
                                completion: bind_expression(
                                    plans,
                                    arm.completion,
                                    active,
                                    bind_call,
                                    trace,
                                    statement,
                                )?,
                            });
                        }
                        CheckedUnitCompletionKind::Selection(
                            CheckedUnitCompletionSelection::Structural {
                                expression: *expression,
                                value: *value,
                                place: place.clone(),
                                alias: *alias,
                                arms: selected,
                            },
                        )
                    }
                }
            }
        };
        // Operand execution can stop before a normally returning callee. That
        // checked transaction must not donate a normal arrival to its parent.
        let normal_continuation = node.normal_continuation
            && match &kind {
                CheckedUnitCompletionKind::Invocation(operations) => operations
                    .iter()
                    .all(|operation| operation.has_normal_continuation()),
                CheckedUnitCompletionKind::Selection(CheckedUnitCompletionSelection::Scalar {
                    arms,
                    ..
                }) => arms.iter().any(|arm| arm.completion.normal_continuation),
                CheckedUnitCompletionKind::Selection(
                    CheckedUnitCompletionSelection::Structural { arms, .. },
                ) => arms.iter().any(|arm| arm.completion.normal_continuation),
            };
        active.remove(&key);
        Some(CheckedUnitCompletionExpression {
            source: handle,
            expression: node.expression,
            normal_continuation,
            kind,
        })
    }
}

use fused::fused_states;
pub(super) use fused::{FusedStates, fused_target_index};
use guards::checked_qualified_arrivals;
pub(super) use guards::{guard_is_boolean, retained_guard};
pub(super) use result_custody::retained_structural_result;
pub(super) use successors::{
    EdgeCalls, SuccessorEdge, SuccessorGuard, successor, successor_bindings,
};

/* Plans explicit state signatures and transitions, tracing the first declined operation. */
pub(super) fn build_traced(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    records: &RetainedBorrowRecords<'_>,
    scalar_callees: ScalarCalleeClosure,
    shapes: &mut ShapeCollector<'_>,
    prepared_calls: &mut PreparedCalls,
    machine: &typed_trees::machine::Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    trace.phase("state graph: shape");
    trace.state(None);
    let states = program.machine_states(machine);
    if states.is_empty() || !machine_binders(program, machine).is_empty() {
        return None;
    }
    trace.phase("state graph: result signature");
    let result = super::types::declared_result_plan(program, shapes, machine)?;
    if let checked_trees::CheckedControlResultPlan::Scalar { .. } = result {
        trace.phase("state graph: result signature: scalar result guarantee");
        // A named invocation clause (`requires incoming: above(n)`) is an erased
        // evidence lane, not part of the scalar result. The signature owner
        // decides which named clauses a scalar body can carry, so this route
        // and the statement-sequence route admit the same signatures.
        if !crate::execution::terminal_unit::types::checked_machine_signature_has_plan(
            program, facts, machine, &states[0],
        ) || states[1..]
            .iter()
            .flat_map(|state| program.state_contracts(state))
            .any(|contract| {
                !matches!(
                    contract.kind,
                    super::SignatureContractKind::Requires
                        | super::SignatureContractKind::Crashes { .. }
                )
            })
        {
            return None;
        }
    }
    let fused_states = fused_states(program, machine);
    let fused = &fused_states;
    let declarations = loan_backing::DeclaredStoragePlans::new(
        program,
        fused
            .iter()
            .flat_map(|(_, state)| program.state_parameters(state))
            .filter_map(|parameter| {
                validation::reference_result_custody::input_carrier_type(
                    program,
                    parameter.type_reference,
                )
            }),
    );
    trace.phase("state graph: natural ranks");
    let (mut natural_ranks, preserving_rank_edges) =
        natural_rank_plans_for_fused(program, fused, &facts.termination.rank_arrivals)?;
    trace.phase("state graph: rank observations");
    let rank_components = ranking::bind_observations(
        program,
        &facts.termination.rank_arrivals,
        &facts.values.scalar_computations,
        &mut natural_ranks,
    )?;
    ranking::complete_witness_ranks(
        fused,
        &facts.termination.rank_arrivals,
        &natural_ranks,
        &rank_components,
    )?;
    let mut signatures = Vec::new();
    let mut state_entry_claims = Vec::new();
    let mut state_requires = Vec::new();
    /* Shared declaration requirement sets; see `declaration_requirements`. */
    let mut declaration_requirements = Vec::new();
    for &(owner, state) in fused.iter() {
        trace.phase("state graph: state signature: result and contracts");
        trace.state(Some(state.symbol));
        if super::types::declared_result_type(program, shapes, state.return_type)? != result {
            trace.phase("state graph: state signature: result differs");
            return None;
        }
        let Some(contract_predicates) =
            validation::structural_state_contract_predicates(program, state)
        else {
            trace.phase("state graph: state signature: contract shape unadmitted");
            return None;
        };
        let mut requires = Vec::with_capacity(contract_predicates.len());
        for row in contract_predicates {
            if let validation::StateContractRow::SourceFact(fact) = &row {
                if signatures.is_empty()
                    || facts
                        .proof
                        .state_requires_fact(
                            program.typed_program(),
                            owner.symbol,
                            state.symbol,
                            *fact,
                        )
                        .is_none()
                {
                    trace.phase("state graph: source requirement has no exact arrival proof owner");
                    return None;
                }
                requires.push(checked_trees::CheckedStateContractRequirement::SourceFact(
                    *fact,
                ));
                continue;
            }
            let source_expression = match &row {
                validation::StateContractRow::Expression(expression) => Some(*expression),
                _ => None,
            };
            let predicate = match row {
                validation::StateContractRow::Expression(expression) => {
                    lower_state_scalar_contract_predicate(
                        program,
                        &facts.operators,
                        owner,
                        state,
                        expression,
                        &mut 4096,
                    )
                }

                validation::StateContractRow::Domain {
                    parameter,
                    constraint,
                } => lower_state_domain_predicate(program, state, parameter, &constraint),
                validation::StateContractRow::SourceFact(_) => {
                    unreachable!("source fact handled above")
                }
                validation::StateContractRow::Finite { parameter } => {
                    validation::state_finite_predicate(program, state, parameter)
                }
            };
            let requirement = match (predicate, source_expression) {
                (Some(predicate), _) => checked_trees::CheckedStateContractRequirement::Closed(
                    checked_trees::ClosedScalarContractValue::Predicate(predicate),
                ),
                (None, Some(expression)) => {
                    checked_trees::CheckedStateContractRequirement::SourceExpression(expression)
                }
                (None, None) => {
                    trace.phase(
                        "state graph: state signature: declaration predicate lowering failed",
                    );
                    return None;
                }
            };
            requires.push(requirement);
        }
        let declaration = checked_trees::CheckedDeclarationRequirementSet {
            clauses: validation::state_declaration_predicates(program, &facts.operators, state)
                .into_iter()
                .map(|row| checked_trees::ClosedScalarContractValue::Predicate(row.predicate))
                .collect(),
        };
        let declaration_requires = if declaration.clauses.is_empty() {
            None
        } else {
            let set = match declaration_requirements
                .iter()
                .position(|known| *known == declaration)
            {
                Some(set) => set,
                None => {
                    declaration_requirements.push(declaration);
                    declaration_requirements.len() - 1
                }
            };
            Some(u32::try_from(set).ok()?)
        };
        state_requires.push((requires, declaration_requires));
        trace.phase("state graph: state signature: parameter signature");
        let (mut structural, scalar) =
            free_structural_scalar_entry_signature_traced(program, shapes, state, &[], trace)?;
        trace.phase("state graph: state signature: entry domains");
        if let Err(reason) =
            retain_parameter_arrival_domains(program, facts, shapes, owner, state, &mut structural)
        {
            trace.phase(reason);
            return None;
        }
        trace.phase("state graph: state signature: entry claims");
        let claims = entry_claims(
            program,
            facts,
            owner.symbol,
            state.symbol,
            &structural,
            program.state_parameters(state),
        )?;
        state_entry_claims.push(claims);
        signatures.push((structural, scalar));
    }
    let completion_signatures = fused
        .iter()
        .zip(&signatures)
        .map(
            |((_, state), (structural, scalar))| super::storage_transfers::CompletionSignature {
                state: state.symbol,
                structural,
                scalar,
            },
        )
        .collect::<Vec<_>>();
    let mut planned = Vec::new();
    for (state_index, &(owner, state)) in fused.iter().enumerate() {
        trace.state(Some(state.symbol));
        let (structural, scalar) = &signatures[state_index];
        let statements = program.statement_table.statements(state.statement_nodes);
        let terminator_index =
            control::statement_sequence::terminator_statement_index(program, state);
        let terminator_statement_ordinal = u32::try_from(terminator_index).ok()?;
        trace.phase("state graph: state flow");
        let flow = state_flow(facts, owner.symbol, state.symbol)?;
        let source_calls = facts.flow.control.calls.span_or_empty(flow.calls);
        let operation_end = if matches!(
            statements.get(terminator_index),
            Some(StatementNode::Expression(_))
        ) && facts
            .values
            .structural_values
            .root_at(state.symbol, u32::try_from(terminator_index).ok()?)
            .is_some()
            || matches!(statements.get(terminator_index), Some(StatementNode::Expression(expression))
                if matches!(program.expression_table.expression(*expression), ExpressionNode::Call(_)))
            || control::terminal_invocation(program, owner, state, terminator_index)
                .is_some_and(|target| fused_target_index(program, fused, target).is_none())
        {
            std::ops::Bound::Included(terminator_index)
        } else {
            std::ops::Bound::Excluded(terminator_index)
        };
        trace.phase("state graph: outer calls");
        let calls = control::outer_calls_in_prefix_traced(
            program,
            facts,
            owner.symbol,
            state,
            source_calls,
            operation_end,
            trace,
        )?;
        let mut sequence_structural = structural.clone();
        trace.phase("state graph: parameter storage");
        let parameter_storage = parameter_storage_plans(program, shapes, state, &[], scalar)?;
        let mut sequence = control::statement_sequence::build(
            program,
            facts,
            scalar_callees,
            shapes,
            prepared_calls,
            owner,
            state,
            &mut sequence_structural,
            scalar,
            &parameter_storage,
            &state_entry_claims[state_index],
            &calls,
            &[],
            0,
            operation_end,
            call_frames,
            trace,
        )?;
        trace.phase("state graph: operation custody");
        operation_custody::admit(program, state, &mut sequence.operations, trace)?;
        let (terminator, selected_exit) = terminator::build(
            program,
            facts,
            &declarations,
            records,
            scalar_callees,
            shapes,
            fused,
            state_index,
            state,
            &result,
            &signatures,
            &state_entry_claims[state_index],
            &sequence.structural_results,
            &sequence.place_loans,
            sequence.scalar_result.as_ref(),
            sequence.structural_result.as_ref(),
            sequence.next_scalar_binding,
            &mut sequence.operations,
            terminator_index,
            trace,
        )?;
        trace.phase("state graph: borrowed windows at selected exits");
        if !sequence.windows.is_closed()
            && !terminator.all_paths_crash(&sequence.operations, &facts.flow.guarded_exits)?
        {
            return None;
        }
        super::structural_value_calls::prepare_state(
            program,
            facts,
            shapes,
            prepared_calls,
            owner.symbol,
            state.symbol,
            &super::structural_value_calls::StateCallInputs {
                parameters: structural,
                entry_claims: &state_entry_claims[state_index],
                operations: &sequence.operations,
                trivial_locals: &[],
            },
        )?;
        let returned = match &terminator {
            checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnStructural {
                result,
            } => Some(result),
            _ => None,
        };
        sequence.finish_reference_retirement(
            program,
            facts,
            prepared_calls,
            shapes,
            structural,
            &[],
            &state_entry_claims[state_index],
            owner,
            state,
            returned,
            Some(super::storage_transfers::ComposedCompletion {
                state: state.symbol,
                terminator: &terminator,
                selected_exit,
                signatures: &completion_signatures,
            }),
            trace,
        )?;
        let mut operations = sequence.operations;
        let place_loans = sequence.place_loans;
        trace.phase("state graph: value call source custody");
        super::structural_value_calls::consume_caller_results(
            program,
            owner.symbol,
            state.symbol,
            &mut operations,
            prepared_calls.state_schedules(
                super::cleanup::owning_cleanup_machine(program, state.symbol),
                state.symbol,
            ),
        )?;
        let call_schedules = |value| prepared_calls.schedule(value);
        let completion = super::cleanup::normal_edges::completed_body_outcome(
            program,
            state,
            terminator.has_normal_continuation(&operations, &facts.flow.guarded_exits)?,
        );
        result_custody::account(
            program,
            facts,
            &call_schedules,
            shapes,
            prepared_calls,
            owner,
            state,
            structural,
            &sequence.structural_results,
            &mut operations,
            &terminator,
            selected_exit,
            completion,
            trace,
        )?;
        planned.push(CheckedComposedUnitControlStatePlan {
            selected_copies: control::selected_copies::SelectedCopies::new(
                program,
                facts,
                owner.symbol,
                state,
            )?
            .into_rows()?,
            normal_exit_cleanups: checked_trees::CheckedNormalExitCleanups {
                completion,
                ..Default::default()
            },
            selected_exit,
            state: state.symbol,
            structural_parameters: structural.clone(),
            scalar_parameters: scalar.clone(),
            parameter_storage,
            erased_scalar_parameters: erased_scalar_parameter_plans(program, state)?,
            erased_proof_parameters: erased_proof_parameter_plans(program, state)?,
            requires: state_requires[state_index].0.clone(),
            declaration_requires: state_requires[state_index].1,
            entry_claims: state_entry_claims[state_index].clone(),
            operations,
            place_loans,
            terminator,
            terminator_statement_ordinal,
            statement_count: u32::try_from(statements.len()).ok()?,
        });
    }
    trace.phase("state graph: state signature: parameter custody shape");
    let Some(live_states) = CheckedComposedUnitControlStatePlan::live_mask(&planned) else {
        trace.phase("state graph: state signature: parameter custody shape: edge target missing");
        return None;
    };
    let invocation_only_entry = !planned
        .iter()
        .zip(&live_states)
        .filter(|(_, live)| **live)
        .flat_map(|(state, _)| state.successors())
        .any(|successor| successor.target_state == planned[0].state);
    for (state_index, &(owner, state)) in fused.iter().enumerate() {
        if !live_states[state_index] {
            continue;
        }
        trace.state(Some(state.symbol));
        let qualified_arrival =
            planned[state_index]
                .structural_parameters
                .iter()
                .any(|parameter| {
                    !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                });
        if qualified_arrival
            && !(state_index == 0 && invocation_only_entry)
            && !checked_qualified_arrivals(facts, &planned, &live_states, state_index)
        {
            trace
                .phase("state graph: state signature: qualified parameter lacks a checked arrival");
            return None;
        }
        for parameter in &planned[state_index].structural_parameters {
            let reference =
                || program.state_parameters(state)[parameter.position as usize].type_reference;
            match parameter.access {
                CheckedStructuralAccess::Owned => {
                    let source_parameter =
                        &program.state_parameters(state)[parameter.position as usize];
                    let view_carrier = validation::has_contents(
                        program,
                        reference(),
                        &[],
                        validation::ContentsRequirement::OwnedOrSharedView,
                    )
                        && validation::reference_result_custody::stored_carrier_ingress_is_complete(
                            program,
                            facts.view(),
                            owner.symbol,
                            state.symbol,
                            source_parameter.symbol,
                            reference(),
                        );
                    if parameter.multiplicity != Multiplicity::Linear
                        && ![
                            validation::ContentsRequirement::CleanupOwned,
                            validation::ContentsRequirement::RepresentedOwned,
                            validation::ContentsRequirement::StructuralConstruction,
                        ]
                        .into_iter()
                        .any(|requirement| {
                            validation::has_contents(program, reference(), &[], requirement)
                        })
                        && !view_carrier
                        && !validation::reference_result_custody::is_reference_record(
                            program,
                            reference(),
                        )
                    {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: owned non-linear record contents",
                        );
                        return None;
                    }
                }
                _ => {
                    if parameter.multiplicity != Multiplicity::Unrestricted {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: borrowed restricted parameter",
                        );
                        return None;
                    }
                    if !shapes.types.contains_key(&parameter.type_identity) {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: unregistered borrowed carrier",
                        );
                        return None;
                    }
                }
            }
        }
    }
    trace.phase("state graph: finish");
    let mut plan = composed_control::finish_state_graph(
        program,
        facts,
        machine,
        planned,
        declaration_requirements,
    )
    .map_err(|reason| trace.phase(reason))
    .ok()?;
    plan.natural_ranks = natural_ranks;
    plan.rank_components = rank_components;
    plan.preserving_rank_edges = preserving_rank_edges;
    let whole_reference = match &result {
        checked_trees::CheckedControlResultPlan::Structural(result) => shapes
            .types
            .get(&result.type_identity)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    checked_trees::CheckedUnitStructuralTypeShape::Reference { .. }
                )
            }),
        _ => false,
    };
    if whole_reference
        || (matches!(
            result,
            checked_trees::CheckedControlResultPlan::Structural(_)
        ) && super::reference_results::is_reference_record(program, states[0].return_type))
    {
        trace.phase("state graph: reference result origins");
        plan.result_reference_sources =
            validation::reference_result_custody::captured_return_sources(
                program, facts, &states[0],
            )?;
    }
    plan.result = result;
    Some(plan)
}

type Signature = (
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
);
