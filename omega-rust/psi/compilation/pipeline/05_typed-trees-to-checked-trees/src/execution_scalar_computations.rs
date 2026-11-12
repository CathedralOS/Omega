/*
    Checked execution plans for call-bearing scalar writes, guards, arguments, and returns.

    A known normal-return result is not an effect-free computation. Short-circuit
    selection may omit an unreachable RHS, but must retain the evaluated left
    graph, including calls whose result becomes known only at a later selection.
    Keep exact source occurrences on retained applications and selections so
    folding an enclosing guard does not change their operand custody.
    One builder owns both plan arenas: structural members may compute scalar
    operands, and scalar observations may first establish a structural value.
*/

use crate::execution::terminal_unit::types::ShapeCollector;
use crate::flow::call_flows;
use crate::flow::expressions;
use crate::values::operator_is_builtin;
use crate::values::scalar::array_constructions::call_array_constructions_in;
use crate::values::scalar::array_constructions::unique_state_flow;
use crate::values::scalar::call_arguments::nested_call_sites;
use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::expression_plans::assignment_target_primitive_type;
use crate::values::scalar::occupies_scalar_position;
use crate::values::scalar::semantic_casts;
use crate::values::scalar::subslice_endpoints::sites;
use crate::values::scalar::subslice_endpoints::subslice_endpoints;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedScalarExpression;
use checked_trees::CheckedScalarExpressionPlans;
use checked_trees::CheckedScalarExpressionRole;
use checked_trees::{
    BorrowFacts, CheckFacts, CheckedArrayConstructionSource, CheckedBooleanExpression,
    CheckedStructuralValue, CheckedStructuralValueKind, CheckedStructuralValuePlans,
};
use checked_trees::{
    CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarComputationPlans,
    FlowFacts, ProofFacts,
};
use facts::FactPlan;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::expression::BinaryOperator;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::expression::UnaryOperator;
use typed_trees::statement::TransitionTargetNode;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};
use typed_trees::types::PrimitiveType;
use typed_trees::types::TypeReferenceNode;
use validation::CallSite;
use validation::ExactIntegerCastFact;
use validation::ScalarArrayElement;
use validation::activation_target_state_index;
use validation::assignment_stored_value_type;
use validation::assignment_target_selectors;
use validation::atomic_assignment_carrier;
use validation::atomic_once_carrier;
use validation::call_target_symbol;
use validation::completing_transition_transfer;
use validation::declared_place_type_raw;
use validation::expression_result_type_reference;
use validation::find_call_site;
use validation::find_machine_head;
use validation::is_scalar_case_value;
use validation::nested_boundary_applications_compose;
use validation::reference_shell;
use validation::result_initializer_call_is_admitted;
use validation::scalar_array_elements;
use validation::terminal_callable_target;
use validation::unit_statement_call_is_admitted;
use validation::zeroed_byte_projected_qualifications;
use validation::zeroed_structural_parts;

mod atomic_loads {
    /*
        Atomic loads share structural observation geometry, while their explicit
        mode retains the event and ordering at this computation's evaluation point.
        Select/Dispatch own whether this node executes; no parser hoist can preserve
        that distinction. The location remains an actual dedicated atomic field.
    */
    use super::{
        Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
        PrimitiveType,
    };

    impl Builder<'_, '_> {
        pub(super) fn atomic_load(
            &mut self,
            expression: ExpressionHandle,
            expected: PrimitiveType,
        ) -> Option<CheckedScalarComputationHandle> {
            let carrier = validation::atomic_load_carrier(self.program, expression)?;
            let validation::AtomicAccessOperation::Load(ordering) = carrier.operation else {
                return None;
            };
            let machine = self.program.machine_by_symbol(self.machine)?;
            let state = self.program.state_by_symbol(self.state)?;
            if !validation::place_is_atomic_storage(
                self.program,
                machine,
                Some(state),
                carrier.place,
            ) || !validation::place_has_builtin_coordinates(
                self.program,
                machine,
                Some(state),
                carrier.place,
            ) {
                return None;
            }
            let captured = self.captured_dispatch_value(carrier.place);
            if self.uses_captured_dispatch_value(carrier.place) && captured.is_none() {
                return None;
            }
            let evaluated_receiver = if let Some(copy) = captured {
                let receiver = validation::DispatchExtraction::payload_binding_receiver(
                    self.program,
                    carrier.place,
                    copy.expression,
                )?;
                Some((
                    receiver,
                    validation::StructuralSource {
                        root: facts::PlaceRoot::Expression(copy.expression),
                        receiver_value: ExpressionHandle::invalid(),
                        statement_ordinal: u32::try_from(self.statement_index).ok()?,
                        type_reference: copy.type_reference,
                    },
                ))
            } else {
                None
            };
            let field = validation::structural_atomic_field(
                self.program,
                self.machine,
                self.state,
                carrier.place,
                &[],
                evaluated_receiver,
                |receiver| {
                    if evaluated_receiver.is_some() {
                        return None;
                    }
                    validation::structural_source(
                        self.program,
                        self.machine,
                        self.state,
                        u32::try_from(self.statement_index).ok()?,
                        carrier.place,
                        receiver,
                    )
                },
            )?;
            if field.primitive_type != expected
                || field.path.iter().any(|segment| {
                    !matches!(
                        segment,
                        checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                            | checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                            | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                            | checked_trees::CheckedUnitStructuralPathSegment::Referent
                    )
                })
            {
                return None;
            }
            let computation = self.structural_field(carrier.place, field)?;
            let CheckedScalarComputationKind::StructuralField {
                source_expression,
                atomic_ordering,
                ..
            } = &mut self.plans.nodes.get_mut(computation).kind
            else {
                return None;
            };
            *source_expression = expression;
            *atomic_ordering = Some(ordering);
            Some(computation)
        }
    }
}
mod boundary_calls {
    /*
        Boundary applications as ordered computation operands.

        Whole-value and nested scalar calls share one computation owner. Its
        authority completes first, then ordinary operands complete left to right,
        and the exact source call runs once. Its scalar result feeds the enclosing
        consumer and its authority carrier retires on normal completion.
        That keeps the authored
        evaluation schedule of `wiki/spec/language/expressions.md`: ordered,
        single invocation, and no invocation on a skipped short-circuit path.

        Mutable primitive storage consumes a computed value, not a separate call
        result binding. Its initializer and whole-place assignments therefore keep
        the boundary application as a computation even when it is the entire RHS.
        Reads of the old storage complete before the same ordinary store replaces it.

        Applications share the ordinary computation operand evaluator:
        structural places, selector snapshots, and scalar computations retain their
        formal order. Their published boundary signature still owns access, claims,
        completion and contract requirements independently of evaluation.
    */
    use super::call_arguments::{CallOperandCustody, ComputedCallOperands};
    use super::{
        Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
        ExpressionNode, PrimitiveType,
    };
    use crate::flow::expressions;
    use crate::published::places::place_at;

    impl Builder<'_, '_> {
        pub(super) fn is_bodyless_boundary_call(&self, mut expression: ExpressionHandle) -> bool {
            let call = loop {
                if !self
                    .program
                    .expression_table
                    .expression_is_valid(expression)
                {
                    return false;
                }
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Atomic(atomic) => expression = atomic.value,
                    ExpressionNode::Cast(_) => return false,
                    ExpressionNode::Call(call) => break call,
                    _ => return false,
                }
            };
            bodyless_boundary_target(
                self.program,
                validation::call_target_symbol(self.program, call),
            )
            .is_some()
        }

        /*
            The resolved declaration owns the signature; having an authored body
            state is not a prerequisite for entering the shared operand evaluator.
        */
        pub(super) fn boundary_call(
            &mut self,
            expression: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            target: (symbols::SymbolHandle, symbols::SymbolHandle),
            expected_type: PrimitiveType,
        ) -> Option<CheckedScalarComputationHandle> {
            let program = self.program;
            let authored_target = validation::call_target_symbol(self.program, call);
            let mut selected_call = call.clone();
            selected_call.target_symbol = authored_target;
            let mut authority_value = checked_trees::CheckedStructuralValueHandle::invalid();
            if call.receiver.is_valid()
                && !program.call_has_no_runtime_receiver_for_target(&selected_call)
            {
                let machine = program.machine_by_symbol(self.machine)?;
                let state = validation::find_state_in_machine(program, self.machine, self.state)?;
                if !is_sealed_accessor_call(program, machine, state, call, authored_target) {
                    let mut owners = program.traits().iter().filter(|owner| {
                        owner.is_boundary
                            && program
                                .executable_trait_machine_instances(owner)
                                .any(|signature| signature.symbol == authored_target)
                    });
                    let owner = owners.next()?;
                    if owners.next().is_some() {
                        return None;
                    }
                    let (source, _) = self.call_ordinal(expression, authored_target)?;
                    let source = self.flow.control.calls.get(source);
                    let site = validation::CallSite::Expression { expression, call };
                    if !crate::execution::terminal_unit::calls::boundary_trait_receiver_is_namespace(
                        program,
                        state,
                        source,
                        &site,
                        owner.symbol,
                    ) {
                        return None;
                    }
                    let reference = validation::expression_result_type_reference(
                        program,
                        machine,
                        state,
                        call.receiver,
                    )?;
                    if place_at(program, self.state, self.statement_index, call.receiver)
                        .is_some_and(|place| matches!(place.root, facts::PlaceRoot::Expression(_)))
                    {
                        authority_value = self.structural_value(call.receiver, reference)?;
                        self.retain_structural_value_root(authority_value, reference)?;
                    }
                }
            }
            let operands = expressions(program)
                .expression_handles(call.arguments)
                .to_vec();
            self.boundary_application(
                expression,
                authored_target,
                target,
                validation::call_target_parameters(program, authored_target)?,
                validation::call_target_return_type(program, target.1)?,
                &operands,
                expected_type,
                authority_value,
            )
        }

        /*
            Build the node once the declaration is known: every formal must be an
            supported call operand and the result must be the expected scalar.
        */
        #[allow(clippy::too_many_arguments)]
        fn boundary_application(
            &mut self,
            expression: ExpressionHandle,
            call_target: symbols::SymbolHandle,
            (target_machine, target_state): (symbols::SymbolHandle, symbols::SymbolHandle),
            parameters: &[typed_trees::signature::StateParameter],
            return_type: typed_trees::types::TypeReferenceHandle,
            operands: &[ExpressionHandle],
            expected_type: PrimitiveType,
            authority_value: checked_trees::CheckedStructuralValueHandle,
        ) -> Option<CheckedScalarComputationHandle> {
            if policy_carrier(self.program, return_type)? != expected_type
                || operands.len() != parameters.len()
            {
                return None;
            }
            let (source_call, call_ordinal) = self.call_ordinal(expression, call_target)?;
            let ComputedCallOperands {
                arguments,
                structural_arguments,
                mut temporary_cleanups,
                claim_transfers,
            } = self.call_operands(
                CallOperandCustody::Statement {
                    call: source_call,
                    captures: self.flow.control.calls.get(source_call).argument_operands(),
                },
                operands,
                parameters,
            )?;
            if authority_value.is_valid() {
                let call = self.flow.control.calls.get(source_call);
                if call.operands.is_empty()
                    || !call.authored_receiver.is_valid()
                    || call.authored_receiver
                        != self.structural_values.nodes.get(authority_value).expression
                {
                    return None;
                }
                let operand = call.operands.start();
                if self.flow.control.operands.get(operand).expression != call.authored_receiver {
                    return None;
                }
                let mut cleanups = self
                    .plans
                    .temporary_cleanups
                    .span(temporary_cleanups)?
                    .to_vec();
                cleanups.push(checked_trees::CheckedScalarComputationTemporaryCleanup {
                    partitions: Default::default(),
                    argument_ordinal: None,
                    operand,
                    owner: Default::default(),
                    actions: Default::default(),
                });
                temporary_cleanups = self.plans.temporary_cleanups.insert_many(cleanups);
            }
            let instruction = self
                .program
                .machine(target_machine)
                .map(|machine| {
                    validation::compiler_operations::descriptor_table_operation(
                        self.program,
                        machine,
                    )
                })
                .transpose()
                .ok()?
                .flatten();
            let callee = match instruction {
                Some((operation, service)) => {
                    if authority_value.is_valid() || !claim_transfers.is_empty() {
                        return None;
                    }
                    let shape_matches = match operation {
                        language_core::descriptor_tables::DescriptorTableOperation::ActivateTask { .. } => {
                            expected_type == PrimitiveType::U8
                                && arguments.len() == 1
                                && structural_arguments.len() == 3
                        }
                        language_core::descriptor_tables::DescriptorTableOperation::ReloadSegments { .. } => {
                            expected_type == PrimitiveType::U8
                                && arguments.len() == 1
                                && structural_arguments.len() == 2
                        }
                        language_core::descriptor_tables::DescriptorTableOperation::ReadTask
                        | language_core::descriptor_tables::DescriptorTableOperation::ReadSegment { .. } => {
                            expected_type == PrimitiveType::U16
                                && arguments.is_empty()
                                && structural_arguments.is_empty()
                        }
                    };
                    if !shape_matches {
                        return None;
                    }
                    checked_trees::CheckedScalarCallee::DescriptorTable {
                        machine: target_machine,
                        state: target_state,
                        operation,
                        service,
                    }
                }
                None => checked_trees::CheckedScalarCallee::Boundary {
                    machine: target_machine,
                    state: target_state,
                    authority_value,
                },
            };
            Some(self.insert(
                expected_type,
                CheckedScalarComputationKind::Call {
                    source_expression: expression,
                    source_call: source_call.into(),
                    callee,
                    call_ordinal,
                    arguments,
                    structural_arguments,
                    temporary_cleanups,
                    claim_transfers,
                },
            ))
        }
    }

    /*
        Classify by callable ownership rather than syntax or a representative
        provider. Authored boundary bodies remain ordinary checked callees.
    */
    pub(super) fn bodyless_boundary_target(
        program: &dyn ::validation::CheckingInput,
        target: symbols::SymbolHandle,
    ) -> Option<(symbols::SymbolHandle, symbols::SymbolHandle)> {
        if let Some((machine, state)) = validation::find_machine_head(program, target)
            .or_else(|| validation::find_state(program, target))
        {
            return machine
                .has_bodyless_signature()
                .then_some((machine.symbol, state.symbol));
        }
        crate::values::scalar::call_lowering::call_is_boundary(program, target)
            .then_some((target, target))
    }

    /*
        The primitive carrier of a type that is bare or qualified only by an
        arithmetic policy. Declared domains and ranges keep their own custody.
    */
    fn policy_carrier(
        program: &dyn ::validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        if let Some(primitive) = program.primitive_type_reference(reference) {
            return Some(primitive);
        }
        let typed_trees::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .all(|constraint| {
                matches!(
                    constraint,
                    typed_trees::types::TypeConstraintNode::ArithmeticDomain(_)
                )
            })
            .then(|| program.primitive_type_reference(*base_type))
            .flatten()
    }

    /*
        An accessor is selected by its compiler-owned plan, not by methods on a
        data type. When a placed-field accessor call qualifies the operation on
        the field place while passing its explicit reference operand in `arguments`,
        the receiver only selects that sealed accessor declaration.
    */
    fn is_sealed_accessor_call(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        call: &typed_trees::expression::TableCallExpression,
        authored_target: symbols::SymbolHandle,
    ) -> bool {
        typed_trees::typed_trees::ScalarAccessorDeclaration::authenticate(
            program.typed_program(),
            authored_target,
        )
        .ok()
        .flatten()
        .is_some()
            && validation::declared_place_type_raw(program, machine, Some(state), call.receiver)
                .or_else(|| {
                    validation::expression_result_type_reference(
                        program,
                        machine,
                        state,
                        call.receiver,
                    )
                })
                .and_then(|reference| program.access_view_field_plan_for_type_reference(reference))
                .is_some_and(|(_, field)| {
                    field.accessor_targets.iter().any(|target| {
                        target.state_symbol == authored_target
                            && target.operation == call.target.as_str()
                    })
                })
    }
}
mod byte_comparisons {

    /*
        Content comparison observes a completed structural operand. Pure places
        and computed descriptors share the same query; descriptor establishment
        stays at the operand's selective evaluation point.
    */
    use super::{
        BinaryOperator, Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind,
        ExpressionHandle, ExpressionNode, PrimitiveType, semantic_casts,
    };
    use checked_trees::CheckedUnitStructuralArgumentPlan;

    impl Builder<'_, '_> {
        pub(super) fn byte_literal_comparison(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<CheckedScalarComputationHandle> {
            let binary = self.program.expression_table.binary_node(expression)?;
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) || !super::operator_is_builtin(self.operators, expression)
            {
                return None;
            }
            let (operand, literal) = match (
                self.program.expression_table.expression(binary.left),
                self.program.expression_table.expression(binary.right),
            ) {
                (_, ExpressionNode::String(bytes)) => (binary.left, bytes.clone()),
                (ExpressionNode::String(bytes), _) => (binary.right, bytes.clone()),
                _ => return None,
            };
            let target = semantic_casts::result_type(self.program, self.state, operand)?;
            let carrier = validation::comparable_byte_carrier_type(self.program, target)?;
            let maximum_length = match carrier {
                validation::ByteObservationCarrier::FixedArray { length } => Some(length),
                validation::ByteObservationCarrier::Sequence(
                    checked_trees::CheckedByteSequenceCarrier::BoundedView { capacity, .. }
                    | checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity },
                ) => Some(capacity),
                _ => None,
            };
            let subject = self.byte_operand_subject(operand, target, false)?;
            Some(self.insert(
                PrimitiveType::Bool,
                CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                    operand_type: target,
                    source_expression: expression,
                    literal,
                    maximum_length,
                    negated: binary.operator == BinaryOperator::NotEqual,
                    subject,
                },
            ))
        }

        /*
            A declared byte-predicate call observes the same completed structural
            operand a content comparison would: a pure place or a computed
            descriptor admitted at the same selective evaluation point. The
            predicate's reads happen only after the operand's own preparation.
        */
        pub(super) fn byte_predicate(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<CheckedScalarComputationHandle> {
            let (predicate, operand) =
                typed_trees::byte_predicates::byte_predicate_call(self.program, expression)?;
            let target = semantic_casts::result_type(self.program, self.state, operand)?;
            validation::comparable_byte_carrier_type(self.program, target)?;
            let subject = self.byte_operand_subject(operand, target, false)?;
            Some(self.insert(
                PrimitiveType::Bool,
                CheckedScalarComputationKind::ByteSequencePredicate {
                    predicate,
                    operand_type: target,
                    source_expression: expression,
                    subject,
                },
            ))
        }

        pub(super) fn storage_address(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<CheckedScalarComputationHandle> {
            let call = self.program.expression_table.call_node(expression)?;
            let operand =
                validation::compiler_operations::storage_address_operand(self.program, call)?;
            let target = semantic_casts::result_type(self.program, self.state, operand)?;
            let shell = validation::reference_shell(self.program, target)?;
            if shell.access == language_semantics::ReferenceAccess::WriteOnly {
                return None;
            }
            let subject = self.byte_operand_subject(operand, target, true)?;
            Some(self.insert(
                PrimitiveType::Addr,
                CheckedScalarComputationKind::StorageAddress {
                    operand_type: target,
                    source_expression: expression,
                    subject,
                },
            ))
        }

        /* Observations and descriptor stores prepare the same byte operand. */
        fn byte_operand_subject(
            &mut self,
            operand: ExpressionHandle,
            target: typed_trees::types::TypeReferenceHandle,
            observe_referent: bool,
        ) -> Option<CheckedUnitStructuralArgumentPlan> {
            let (machine, state) = validation::find_state(self.program, self.state)?;
            if let Some(plan) = crate::execution::terminal_unit::byte_operands::plan(
                self.program,
                self.operators,
                self.pure,
                self.plans,
                self.borrow,
                machine,
                state,
                self.statement_index,
                operand,
                target,
                observe_referent,
                &|position| self.dense_structural_parameter_index(position),
                &|selector| self.local_selector(selector),
            ) {
                return Some(plan);
            }
            if observe_referent {
                return None;
            }
            self.completed_byte_subject(operand, target)
        }

        /// Observe the exact completed byte value produced by the shared value
        /// owner. Copies, saved projections, constructions and call results retain
        /// that owner's capture, evaluation order and cleanup; none is a new read
        /// of storage at the source expression's spelling.
        fn completed_byte_subject(
            &mut self,
            operand: ExpressionHandle,
            target: typed_trees::types::TypeReferenceHandle,
        ) -> Option<CheckedUnitStructuralArgumentPlan> {
            let value = self.structural_value(operand, target)?;
            self.retain_structural_value_root(value, target)?;
            Some(CheckedUnitStructuralArgumentPlan {
                source:
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        value_dependency: Some(value),
                        expression: operand,
                        logical_root_type_identity: None,
                    },
                path: Vec::new(),
                projection_evaluations: Default::default(),
                type_identity: validation::structural_type_identity(self.program, target, &[], &[]),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            })
        }
    }
}
mod call_arguments {

    mod declarations {
        /*
            Declaration calls consume initialized whole entry homes. This joins their
            storage receipt to the ordinary argument planner without manufacturing a
            statement occurrence or reconstructing an aggregate from scalar reads.
            Shared calls lend the home until their pure, total scalar invocation
            returns; contained loans and cleanup remain with its owner. An owned
            operand separately requires Copy and disposal-free custody on that exact
            selected value, not on its containing record.
        */
        use super::super::{Builder, ExpressionHandle};

        impl Builder<'_, '_> {
            pub(super) fn declaration_structural_argument(
                &mut self,
                application: checked_trees::CheckedRankApplicationFormationHandle,
                call: arena::Handle<checked_trees::CheckedDeclarationCall>,
                argument: ExpressionHandle,
                target: &typed_trees::signature::StateParameter,
            ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
                let scope = self.declaration_scope.as_ref()?;
                if scope.application != application {
                    return None;
                }
                let (formation, call) =
                    checked_trees::CheckedCallOrigin::Declaration { application, call }
                        .declaration(scope.plan, self.machine, self.state)
                        .ok()?;
                let input = formation.input_home?;
                let declaration = self
                    .program
                    .measures()
                    .iter()
                    .find(|declaration| declaration.symbol == formation.application.owner)?;
                if declaration.parameter.as_ref()?.symbol != input.formal
                    || !self
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                        .contains(&argument)
                {
                    return None;
                }
                let authored = validation::canonical_place_from_expression(self.program, argument)?;
                if authored.root.as_symbol() != Some(input.formal) {
                    return None;
                }
                let plan = scope.plan;
                let coordinate = plan.coordinates.get(input.actual);
                let root_path = plan.path_segments.span(coordinate.path)?;
                let actual = validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(coordinate.parameter),
                    segments: root_path
                        .iter()
                        .chain(&authored.segments)
                        .cloned()
                        .collect(),
                };
                let (_, state) = validation::find_state(self.program, self.state)?;
                let parameter = self
                    .program
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.symbol == coordinate.parameter)?;
                let actual_type = validation::project_type_reference_from_segments(
                    self.program,
                    parameter.type_reference,
                    root_path,
                )?;
                if !validation::declared_input_type_matches(
                    self.program,
                    actual_type,
                    input.actual_type,
                ) {
                    return None;
                }
                let geometry = validation::structural_coordinate(
                    self.program,
                    self.authored_parameters,
                    coordinate.parameter,
                    root_path,
                )?;
                let bounds = plan.entry_indexes.span(coordinate.entry_indexes)?;
                let mut selectors = Vec::new();
                for (path_step, projection) in geometry.projections.iter().enumerate() {
                    let validation::ScalarCoordinateProjection::Element { selector, .. } =
                        projection
                    else {
                        continue;
                    };
                    let checked = self.entry_index_selector(
                        coordinate.parameter,
                        path_step,
                        projection,
                        bounds,
                    )?;
                    let checked_trees::CheckedEntryIndexSelector::Expression(expression) = selector
                    else {
                        return None;
                    };
                    let primitive =
                        crate::values::scalar::expression_facts::scalar_expression_type(
                            &checked.expression,
                        )?;
                    let value = self.insert(
                        primitive,
                        checked_trees::CheckedScalarComputationKind::Value(checked.expression),
                    );
                    self.plans.nodes.get_mut(value).authored_root = *expression;
                    selectors.push((*expression, value));
                }
                selectors.extend(self.projection_selector_evaluations(argument, argument)?);
                let mut storage_target = target.clone();
                let access = match validation::reference_shell(self.program, target.type_reference)
                {
                    Some(shell) => {
                        let typed_trees::expression::ExpressionNode::Borrow(borrow) =
                            self.program.expression_table.expression(argument)
                        else {
                            return None;
                        };
                        if shell.access != language_semantics::ReferenceAccess::Shared
                            || borrow.access != shell.access
                        {
                            return None;
                        }
                        storage_target.type_reference = shell.referee;
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                    }
                    None => {
                        if !validation::reference_result_custody::source_value_is_copied(
                            self.program,
                            target.type_reference,
                        ) || validation::reference_result_custody::result_multiplicity(
                            self.program,
                            target.type_reference,
                        ) != language_semantics::Multiplicity::Unrestricted
                        {
                            return None;
                        }
                        if matches!(
                            self.program.expression_table.expression(argument),
                            typed_trees::expression::ExpressionNode::Borrow(_)
                        ) {
                            return None;
                        }
                        checked_trees::CheckedStructuralAccess::Owned
                    }
                };
                let mut argument_plan = crate::execution::terminal_unit::calls::storage_argument(
                    self.program,
                    &mut self.shapes,
                    self.machine,
                    state,
                    0,
                    &actual,
                    &storage_target,
                    access,
                    Some(&|expression| {
                        selectors.iter().find_map(|(source, value)| {
                            (*source == expression).then_some(
                                checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(
                                    *value,
                                ),
                            )
                        })
                    }),
                )?;
                argument_plan.access = access;
                let evaluations = self
                    .plans
                    .operands
                    .insert_many(selectors.iter().map(|(_, value)| *value));
                argument_plan.retain_projection_evaluations(evaluations, &self.plans.operands)?;
                Some(argument_plan)
            }
        }
    }

    use super::{Builder, CheckedScalarExpressionRole, ExpressionHandle, SymbolHandle};
    use crate::execution::terminal_unit::ComputationCallArguments;
    use crate::execution::terminal_unit::cleanup::projected_temporary_cleanup;
    use crate::execution::terminal_unit::strips_erased_parameter;
    use crate::execution::terminal_unit::types::structural_access_for_type_reference;
    use crate::flow::CapturedSubjectFacts;
    use crate::published::semantic::calls::shared_argument_referent;
    use crate::values::mutable_scalar_parameter_type;
    use crate::values::scalar::call_lowering::call_is_boundary;
    use crate::values::scalar::scalar_lowering::land_anonymous_scalar_expression;
    use crate::values::scalar_expression_type;

    #[derive(Clone, Copy)]
    pub(super) enum CallOperandCustody {
        Statement {
            call: arena::Handle<checked_trees::FlowCallFact>,
            captures: arena::HandleSpan<checked_trees::FlowOperandFact>,
        },
        Declaration {
            application: checked_trees::CheckedRankApplicationFormationHandle,
            call: arena::Handle<checked_trees::CheckedDeclarationCall>,
        },
    }

    impl Builder<'_, '_> {
        /*
            Index operands have mathematical carriers, not necessarily stored type
            references: a binary result owns no declaration identity. Reuse the
            integer evaluator so computed selectors preserve signedness and policy;
            only an anonymous integer needs the ordinary u64 index context. Choose
            that context before materialization, which requires a landed carrier.
        */
        pub(super) fn index_selector(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<checked_trees::CheckedScalarComputationHandle> {
            let value = if let Some(value) = land_anonymous_scalar_expression(
                self.program,
                self.operators,
                expression,
                typed_trees::types::PrimitiveType::U64,
            ) {
                let value = self.insert(
                    typed_trees::types::PrimitiveType::U64,
                    checked_trees::CheckedScalarComputationKind::Value(value),
                );
                self.plans.nodes.get_mut(value).value_source = expression;
                value
            } else {
                let operand = self.integer_operand(expression, None)?;
                if !scalar_expression_type(&operand.value)
                    .is_some_and(|primitive| primitive.is_fixed_integer())
                {
                    return None;
                }
                self.materialize_integer(operand)?
            };
            self.plans.nodes.get_mut(value).authored_root = expression;
            Some(value)
        }

        pub(super) fn record_transition_selectors(
            &mut self,
            statement: u32,
            argument_ordinal: u32,
            argument: ExpressionHandle,
        ) {
            let Some(place) = self.canonical_place(argument) else {
                return;
            };
            for segment in place.segments {
                if let facts::PlaceSegment::Index { expression } = segment {
                    let role = CheckedScalarExpressionRole::TransitionSelector {
                        argument_ordinal,
                        expression,
                    };
                    if self.plans.root_at(self.state, statement, role).is_none()
                        && let Some(root) = self.index_selector(expression)
                    {
                        self.push_scalar_root(role, root);
                    }
                }
            }
        }

        pub(super) fn record_call_arguments(
            &mut self,
            statement: u32,
            call_ordinal: u32,
            target: SymbolHandle,
            arguments: &[ExpressionHandle],
        ) {
            if let Some(operation) = self
                .program
                .symbols
                .builtin_function_for_symbol(target)
                .and_then(|builtin| {
                    language_core::inline_assembly::AsmRegisterOperation::from_intrinsic_name(
                        builtin.name(),
                    )
                })
            {
                if arguments.len() != operation.input_carriers().len() {
                    return;
                }
                for (ordinal, (argument, carrier)) in
                    arguments.iter().zip(operation.input_carriers()).enumerate()
                {
                    let (Ok(argument_ordinal), Some(primitive_type)) = (
                        u32::try_from(ordinal),
                        typed_trees::types::PrimitiveType::from_name(carrier.name()),
                    ) else {
                        return;
                    };
                    self.record_root(
                        self.pure,
                        statement,
                        CheckedScalarExpressionRole::UnitCallArgument {
                            call_ordinal,
                            argument_ordinal,
                        },
                        *argument,
                        primitive_type,
                    );
                }
                return;
            }
            let Some(parameters) = validation::call_target_parameters(self.program, target) else {
                return;
            };
            let explicit_parameters = parameters.iter();
            if parameters.len() != arguments.len() {
                return;
            }
            let boundary = call_is_boundary(self.program, target);
            let mut scalar_ordinal = 0u32;
            let mut structural_ordinal = 0u32;
            for (argument, parameter) in arguments.iter().zip(explicit_parameters) {
                match strips_erased_parameter(parameter) {
                    Some(true) => continue,
                    Some(false) => {}
                    None => return,
                }
                let Some(primitive_type) = self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                else {
                    if !parameter.is_const {
                        self.retain_structural_argument(*argument, parameter.type_reference);
                        let Some(selectors) =
                            self.projection_selector_evaluations(*argument, *argument)
                        else {
                            return;
                        };
                        for (expression, root) in selectors {
                            let role = CheckedScalarExpressionRole::CallSelector {
                                call_ordinal,
                                argument_ordinal: structural_ordinal,
                                expression,
                            };
                            if self.plans.root_at(self.state, statement, role).is_none() {
                                self.push_scalar_root(role, root);
                            }
                        }
                        let Some(next) = structural_ordinal.checked_add(1) else {
                            return;
                        };
                        structural_ordinal = next;
                    }
                    continue;
                };
                if parameter.is_const
                    || (parameter.is_mutable
                        && mutable_scalar_parameter_type(self.program, parameter).is_none())
                {
                    return;
                }
                let role = if boundary {
                    CheckedScalarExpressionRole::BoundaryCallArgument {
                        call_ordinal,
                        argument_ordinal: scalar_ordinal,
                    }
                } else {
                    CheckedScalarExpressionRole::UnitCallArgument {
                        call_ordinal,
                        argument_ordinal: scalar_ordinal,
                    }
                };
                self.record_root(self.pure, statement, role, *argument, primitive_type);
                let Some(next) = scalar_ordinal.checked_add(1) else {
                    return;
                };
                scalar_ordinal = next;
            }
        }
    }

    /*
        Call operands have one evaluator regardless of whether the eventual invocation
        enters a checked body or a boundary. The exact formal roster determines the
        scalar/structural lanes; the producer retains authored order and loan custody.
    */
    impl Builder<'_, '_> {
        pub(super) fn call_operands(
            &mut self,
            custody: CallOperandCustody,
            arguments: &[ExpressionHandle],
            parameters: &[typed_trees::signature::StateParameter],
        ) -> Option<ComputedCallOperands> {
            let (statement_call, captures) = match custody {
                CallOperandCustody::Statement { call, captures } => {
                    if captures.len() != arguments.len() || !self.flow.control.calls.is_valid(call)
                    {
                        return None;
                    }
                    (Some(call), Some(captures))
                }
                CallOperandCustody::Declaration { application, call } => {
                    let scope = self.declaration_scope.as_ref()?;
                    if scope.application != application {
                        return None;
                    }
                    let (_, call) =
                        checked_trees::CheckedCallOrigin::Declaration { application, call }
                            .declaration(scope.plan, self.machine, self.state)
                            .ok()?;
                    if self
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                        != arguments
                    {
                        return None;
                    }
                    (None, None)
                }
            };
            if arguments.len() != parameters.len()
                || parameters.iter().any(|parameter| {
                    parameter.is_const
                        || (parameter.is_mutable
                            && self
                                .program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                            && mutable_scalar_parameter_type(self.program, parameter).is_none())
                })
            {
                return None;
            }
            let mut computed_arguments = Vec::with_capacity(arguments.len());
            let mut structural_arguments = Vec::new();
            let mut structural_actuals = Vec::new();
            let mut temporary_cleanups = Vec::new();
            let mut prepared_arguments = None;
            let mut captures = captures.map(|span| span.handles());
            for (argument, parameter) in arguments.iter().zip(parameters) {
                let operand = match &mut captures {
                    Some(captures) => Some(captures.next()?),
                    None => None,
                };
                if let Some(operand) = operand
                    && self.flow.control.operands.get(operand).expression != *argument
                {
                    return None;
                }
                if strips_erased_parameter(parameter)? {
                    continue;
                }
                if let Some(primitive_type) = self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                {
                    let root = self.expression(*argument, primitive_type)?;
                    self.plans.nodes.get_mut(root).authored_root = *argument;
                    computed_arguments.push(root);
                } else {
                    let mut argument_plan = if let CallOperandCustody::Declaration {
                        application,
                        call,
                    } = custody
                    {
                        self.declaration_structural_argument(
                            application,
                            call,
                            *argument,
                            parameter,
                        )?
                    } else {
                        let source_call = statement_call?;
                        let operand = operand?;
                        let state = validation::find_state_in_machine(
                            self.program,
                            self.machine,
                            self.state,
                        )?;
                        let retained =
                            self.retain_structural_argument(*argument, parameter.type_reference);
                        let argument_ordinal = u32::try_from(structural_arguments.len()).ok()?;

                        if let Some(value) = retained {
                            self.retained_call_argument(
                                source_call,
                                operand,
                                *argument,
                                parameter,
                                value,
                                argument_ordinal,
                                &mut temporary_cleanups,
                            )?
                        } else if self.uses_captured_dispatch_value(*argument) {
                            return None;
                        } else {
                            let call_arguments = prepared_arguments.get_or_insert_with(|| {
                                ComputationCallArguments::new(
                                    self.program,
                                    CapturedSubjectFacts {
                                        semantic: self.semantic,
                                        flow: self.flow,
                                        borrow: self.borrow,
                                    },
                                    self.machine,
                                    state,
                                    self.flow.control.calls.get(source_call),
                                )
                            });
                            (|| {
                            let selectors = self.projection_selector_evaluations(*argument, *argument)?;
                            let mut argument_plan = call_arguments.structural_argument(
                                &mut self.shapes,
                                *argument,
                                parameter,
                                Some(&|expression| {
                                    selectors.iter().find_map(|(source, value)| {
                                        (*source == expression).then_some(checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(*value))
                                    })
                                }),
                            )?;
                            argument_plan.retain_projection_evaluations(self.projection_selector_span(*argument, &selectors)?, &self.plans.operands)?;
                            Some(argument_plan)
                        })()
                        .or_else(|| {
                            let argument_ordinal = structural_arguments.len();
                            call_arguments.subslice_argument(
                                &mut self.shapes,
                                self.operators,
                                self.pure,
                                self.plans,
                                *argument,
                                parameter,
                                u32::try_from(argument_ordinal).ok()?,
                            )
                        })
                        .or_else(|| {
                            crate::execution::terminal_unit::calls::byte_sequence_literal_argument(
                                self.program,
                                parameter.type_reference,
                                *argument,
                            )
                        })
                        ?
                        }
                    };

                    self.retain_dispatch_argument_dependency(&mut argument_plan)?;
                    structural_actuals.push(*argument);
                    structural_arguments.push(
                        checked_trees::CheckedScalarComputationStructuralArgument::Place(
                            argument_plan,
                        ),
                    );
                }
            }
            let claim_transfers = if let Some(source_call) = statement_call {
                crate::execution::terminal_unit::calls::computed_call_claim_transfers(
                    self.program,
                    self.flow,
                    self.plans,
                    self.machine,
                    self.state,
                    self.flow.control.calls.get(source_call),
                    &structural_actuals
                        .iter()
                        .zip(&structural_arguments)
                        .map(|(actual, argument)| Some((*actual, argument.as_place()?)))
                        .collect::<Option<Vec<_>>>()?,
                )?
            } else {
                if !temporary_cleanups.is_empty() {
                    return None;
                }
                Vec::new()
            };
            let arguments = self.plans.operands.insert_many(computed_arguments);
            let structural_arguments = self
                .plans
                .structural_arguments
                .insert_many(structural_arguments);
            temporary_cleanups.reverse();
            let temporary_cleanups = self
                .plans
                .temporary_cleanups
                .insert_many(temporary_cleanups);
            let claim_transfers = self.plans.claim_transfers.insert_many(claim_transfers);
            Some(ComputedCallOperands {
                arguments,
                structural_arguments,
                temporary_cleanups,
                claim_transfers,
            })
        }

        /// A borrowed place can refer to a subject produced earlier in this match.
        /// Carry that exact producer so call preparation waits for it. The loan,
        /// path and selectors remain the already checked existing-place argument.
        fn retain_dispatch_argument_dependency(
            &mut self,
            argument: &mut checked_trees::CheckedUnitStructuralArgumentPlan,
        ) -> Option<()> {
            if argument.access == checked_trees::CheckedStructuralAccess::Owned {
                return Some(());
            }
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression,
                value_dependency,
                ..
            } = &mut argument.source
            else {
                return Some(());
            };
            if value_dependency.is_some() {
                return Some(());
            }
            let anchor = *expression;
            let Some(receiver) = self
                .dispatch_receivers
                .get(&(anchor.arena_index(), anchor.generation()))
                .copied()
            else {
                return Some(());
            };
            if !receiver.value.is_valid() {
                return Some(());
            }
            if !self.structural_values.nodes.is_valid(receiver.value)
                || self
                    .structural_values
                    .root_for_value(receiver.value)
                    .is_none()
            {
                return None;
            }
            self.retain_structural_value_root(receiver.value, receiver.type_reference)?;
            // Keep the saved name attached to the existing value. This does not
            // allocate storage or add cleanup.
            *expression = self.structural_values.nodes.get(receiver.value).expression;
            *value_dependency = Some(receiver.value);
            Some(())
        }

        /*
            Consume an established producer. A failed owner or custody join cannot
            turn this operand back into a live source-place argument.
        */
        fn retained_call_argument(
            &mut self,
            source_call: arena::Handle<checked_trees::FlowCallFact>,
            operand: arena::Handle<checked_trees::FlowOperandFact>,
            argument: ExpressionHandle,
            parameter: &typed_trees::signature::StateParameter,
            value: checked_trees::CheckedStructuralValueHandle,
            argument_ordinal: u32,
            temporary_cleanups: &mut Vec<checked_trees::CheckedScalarComputationTemporaryCleanup>,
        ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
            use typed_trees::types::TypeReferenceNode;
            let descriptor =
                validation::slice_view_shell(self.program, parameter.type_reference, &[]);
            let producer_type = self.structural_values.nodes.get(value).type_reference;
            let stored = validation::reference_result_custody::stored_loan_parts(
                self.program,
                producer_type,
            )
            .map(|_| producer_type);
            let (access, referent) = if let Some(actual) = stored {
                let (_, supplied) =
                    validation::reference_result_custody::stored_loan_parts(self.program, actual)?;
                let required =
                    structural_access_for_type_reference(self.program, parameter.type_reference)?;
                if supplied != checked_trees::CheckedStructuralAccess::MutableBorrow
                    && supplied != required
                {
                    return None;
                }
                (required, actual)
            } else {
                match self
                    .program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                {
                    TypeReferenceNode::Reference { access, .. } if descriptor.is_some() => {
                        ((*access).into(), parameter.type_reference)
                    }
                    TypeReferenceNode::Reference {
                        access: language_semantics::ReferenceAccess::Shared,
                        ..
                    } => (
                        checked_trees::CheckedStructuralAccess::SharedBorrow,
                        shared_argument_referent(self.program, argument, parameter.type_reference)?
                            .1,
                    ),
                    TypeReferenceNode::Reference {
                        access, referee, ..
                    } => ((*access).into(), *referee),
                    _ => (
                        checked_trees::CheckedStructuralAccess::Owned,
                        parameter.type_reference,
                    ),
                }
            };
            let (source, owner_type, mut path, projection_evaluations) =
                self.retained_argument_owner(value, producer_type)?;
            if descriptor.is_none() && stored.is_none() {
                validation::structural_result_contract(self.program, referent).ok()?;
            }
            let selected = if let Some(actual) = stored {
                let (selected, _) =
                    validation::reference_result_custody::stored_loan_parts(self.program, actual)?;
                path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
                let (selected, selected_path) =
                    crate::execution::terminal_unit::calls::borrowed_argument_endpoint(
                        self.program,
                        selected,
                        path,
                        parameter.type_reference,
                    )?;
                path = selected_path;
                selected
            } else {
                referent
            };
            let selected_identity = if stored.is_some() {
                validation::stored_structural_type_identity(self.program, selected, &[], &[])
            } else {
                validation::structural_type_identity(self.program, selected, &[], &[])
            };
            if stored.is_some()
                && selected_identity
                    != validation::structural_type_identity(
                        self.program,
                        parameter.type_reference,
                        &[],
                        &[],
                    )
            {
                return None;
            }
            let argument_plan = checked_trees::CheckedUnitStructuralArgumentPlan {
                projection_evaluations,
                source,
                path,
                type_identity: selected_identity,
                access,
            };
            let mut discards = None;
            if access != checked_trees::CheckedStructuralAccess::Owned {
                if descriptor.is_none() && stored.is_none() {
                    crate::execution::terminal_unit::calls::validate_shared_temporary(
                        self.program,
                        self.flow,
                        self.machine,
                        self.state,
                        self.flow.control.calls.get(source_call),
                        argument_plan.source.structural_expression()?,
                        owner_type,
                    )?;
                }
                match validation::reference_result_custody::result_multiplicity(
                    self.program,
                    owner_type,
                ) {
                    language_semantics::Multiplicity::Affine
                    | language_semantics::Multiplicity::Linear => {
                        discards = Some(vec![
                        checked_trees::CheckedNormalCleanupAction::RetireHome {
                            // Retirement names the whole completed owner. Its
                            // borrowed projection is only the call's presentation.
                            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                value_dependency: argument_plan.source.structural_value_dependency(),
                                expression: argument_plan.source.structural_expression()?,
                                logical_root_type_identity: None,
                            },
                            type_identity: validation::stored_structural_type_identity(
                                self.program,
                                owner_type,
                                &[],
                                &[],
                            ),
                        },
                    ]);
                    }
                    language_semantics::Multiplicity::Unrestricted => {}
                }
            } else if !argument_plan.path.is_empty() {
                discards = Some(projected_temporary_cleanup(
                    self.program,
                    owner_type,
                    &argument_plan,
                )?);
            }
            if let Some(discards) = discards {
                temporary_cleanups.push(checked_trees::CheckedScalarComputationTemporaryCleanup {
                    partitions: Default::default(),
                    argument_ordinal: Some(argument_ordinal),
                    operand,
                    owner: Default::default(),
                    actions: self.plans.temporary_cleanup_actions.insert_many(discards),
                });
            }
            Some(argument_plan)
        }
    }

    /* One computed call's operand lanes, retained in the computation arenas. */
    pub(super) struct ComputedCallOperands {
        pub(super) arguments: arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>,
        pub(super) structural_arguments:
            arena::HandleSpan<checked_trees::CheckedScalarComputationStructuralArgument>,
        pub(super) temporary_cleanups:
            arena::HandleSpan<checked_trees::CheckedScalarComputationTemporaryCleanup>,
        pub(super) claim_transfers: arena::HandleSpan<checked_trees::CheckedUnitClaimTransferPlan>,
    }
}
mod call_result_types {
    /*
        Which call result types can be a checked structural result, decided once
        per value-plan build for every call the flow facts record.

        Planning a structural call value asks whether its substituted result type
        admits a checked structural result binding. Each ask builds a fresh shape
        collector and shapes the whole type, about 45 µs, and the product program
        asks 466 times about 231 distinct types. The answer is a pure function of
        the typed program and the type reference: the asking site always starts
        from an empty collector and passes no binders. So the build answers every
        distinct result type of every flow call up front, on the checking pool,
        and the site reads the answer here. A site whose type is not in the table
        (a call the flow facts do not record) shapes it as before.
    */
    use checked_trees::FlowFacts;
    use typed_trees::types::TypeReferenceHandle;

    use crate::execution::terminal_unit::control::checked_structural_result_type;
    use crate::execution::terminal_unit::types::{CarrierIndex, ShapeCollector};

    /*
        Every flow call's result type, ascending by handle, with whether it admits
        a checked structural result.
    */
    #[derive(Clone, Default)]
    pub(super) struct CallResultTypes(std::sync::Arc<[(TypeReferenceHandle, bool)]>);

    impl CallResultTypes {
        pub(super) fn build(
            program: &dyn ::validation::CheckingInput,
            flow: &FlowFacts,
            carriers: &CarrierIndex,
        ) -> Self {
            use rayon::prelude::*;
            let mut result_types = flow
                .control
                .states
                .iter()
                .filter_map(|(_, state_flow)| {
                    let machine = program.machine_by_symbol(state_flow.machine_symbol)?;
                    let state = program.state_by_symbol(state_flow.state_symbol)?;
                    Some(
                        crate::flow::call_flows(&flow.control, state_flow.calls)
                            .iter()
                            .filter_map(move |call| {
                                validation::expression_result_type_reference(
                                    program,
                                    machine,
                                    state,
                                    call.authored_expression,
                                )
                            }),
                    )
                })
                .flatten()
                .collect::<Vec<_>>();
            result_types.sort_unstable_by_key(|reference| handle_order(*reference));
            result_types.dedup();
            let answers = validation::checking_thread_pool().install(|| {
                result_types
                    .par_iter()
                    .map(|&reference| (reference, admits(program, carriers, reference)))
                    .collect::<Vec<_>>()
            });
            Self(answers.into())
        }

        /*
            Whether `result_type` admits a checked structural result, from the
            table when the build answered it, else by shaping it now.
        */
        pub(super) fn admits(
            &self,
            program: &dyn ::validation::CheckingInput,
            carriers: &CarrierIndex,
            result_type: TypeReferenceHandle,
        ) -> bool {
            match self
                .0
                .binary_search_by_key(&handle_order(result_type), |(reference, _)| {
                    handle_order(*reference)
                }) {
                Ok(position) => self.0[position].1,
                Err(_) => admits(program, carriers, result_type),
            }
        }
    }

    fn admits(
        program: &dyn ::validation::CheckingInput,
        carriers: &CarrierIndex,
        result_type: TypeReferenceHandle,
    ) -> bool {
        checked_structural_result_type(
            program,
            &mut ShapeCollector::with_carrier_index(program, carriers.clone()),
            result_type,
            &[],
        )
        .is_some()
    }

    fn handle_order(reference: TypeReferenceHandle) -> (u32, u32) {
        (reference.arena_index(), reference.generation())
    }
}
mod cases {

    /*
        Constructor payloads use the ordinary operand graph before tag observation.
        The same dynamic field computations retain their calls, traps and selected
        operators; knowing the constructed case does not erase their evaluation.
    */
    use super::{Builder, CheckedScalarComputationHandle, ExpressionHandle, PrimitiveType};
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::published::places::place_at;
    use checked_trees::{
        CheckedScalarCaseComputationField, CheckedScalarCaseConstruction,
        CheckedScalarComputationStructuralArgument,
    };

    impl Builder<'_, '_> {
        pub(super) fn case_membership(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<CheckedScalarComputationHandle> {
            let binary = self.program.expression_table.binary_node(expression)?;
            let machine = self.program.machine_by_symbol(self.machine)?;
            let state = self.program.state_in_machine(machine, self.state)?;
            let case = validation::applied_case_membership_variant(
                self.program,
                machine,
                Some(state),
                expression,
                binary,
            )?;
            if let Some((_, subject)) =
                self.readable_case_subject(binary.left, self.statement_index)
            {
                let receiver_value = self.case_subject_receiver(&subject)?;
                return Some(self.publish_case_membership(
                    expression,
                    expression,
                    receiver_value,
                    CheckedScalarComputationStructuralArgument::Place(subject),
                    case,
                ));
            }
            self.computed_case_membership(expression, binary.left, case)
        }

        /*
            Resolve the same readable place for membership and intrinsic
            enum equality. This route contains only place reads: calls and computed
            selectors must already have ordinary captured results before observation.
        */
        pub(super) fn readable_case_subject(
            &self,
            subject: ExpressionHandle,
            statement_index: usize,
        ) -> Option<(
            typed_trees::types::TypeReferenceHandle,
            checked_trees::CheckedUnitStructuralArgumentPlan,
        )> {
            if self.uses_captured_dispatch_value(subject) {
                return None;
            }
            let place = place_at(self.program, self.state, statement_index, subject)?;
            let source = if let Some(expression) = place.root.as_expression() {
                let typed_trees::expression::ExpressionNode::SavedSubject(saved) =
                    self.program.expression_table.expression(expression)
                else {
                    return None;
                };
                self.program.saved_value_source(saved)?;
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    value_dependency: None,
                    expression,
                    logical_root_type_identity: None,
                }
            } else {
                let root = place.root.as_symbol()?;
                let parameter = self
                    .authored_parameters
                    .iter()
                    .find(|parameter| parameter.symbol == root);
                let symbol = if let Some(parameter) = parameter {
                    parameter.symbol
                } else {
                    let machine = self.program.machine_by_symbol(self.machine)?;
                    let state = self.program.state_in_machine(machine, self.state)?;
                    let (_, local) = self
                        .program
                        .state_local_declaration(state, root)
                        .filter(|(ordinal, _)| *ordinal < statement_index)?;
                    let inspected =
                        validation::unwrapped_type_reference(self.program, local.type_reference)?;
                    if !validation::has_contents(
                        self.program,
                        inspected,
                        &[],
                        validation::ContentsRequirement::BorrowedStorage,
                    ) {
                        return None;
                    }
                    local.symbol
                };
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
            };
            let ProjectedArgumentPath {
                storage_type: projected,
                path,
                ..
            } = crate::execution::terminal_unit::calls::projected_observed_path(
                self.program,
                self.machine,
                self.state,
                statement_index,
                &place,
                &|selector| self.local_selector(selector),
            )?;
            Some((
                projected,
                checked_trees::CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: crate::execution::terminal_unit::calls::retain_local_reference_source(
                        self.program,
                        self.state,
                        source,
                        &path,
                    ),
                    path,
                    type_identity: validation::stored_structural_type_identity(
                        self.program,
                        projected,
                        &[],
                        &[],
                    ),
                    access: checked_trees::CheckedStructuralAccess::SharedBorrow,
                },
            ))
        }

        /// A tag test retains its saved receiver and exact relative path. The
        /// dispatch owns its birth; this association never replays the subject.
        pub(super) fn case_subject_receiver(
            &mut self,
            subject: &checked_trees::CheckedUnitStructuralArgumentPlan,
        ) -> Option<checked_trees::CheckedStructuralValueHandle> {
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression: anchor,
                ..
            } = subject.source
            else {
                return Some(checked_trees::CheckedStructuralValueHandle::invalid());
            };
            let typed_trees::expression::ExpressionNode::SavedSubject(saved) =
                self.program.expression_table.expression(anchor)
            else {
                return None;
            };
            if let Some(value) = self
                .dispatch_receivers
                .get(&(anchor.arena_index(), anchor.generation()))
            {
                return Some(value.value);
            }
            let capture = self.program.expression_table.value_capture(saved.capture);
            if matches!(
                capture.owner,
                typed_trees::expression::ValueCaptureOwner::Match
            ) {
                // Match production must publish the exact value/place association
                // before its patterns. Replaying its initializer cannot supply it.
                return None;
            }
            let source = self.program.saved_value_source(saved)?;
            let machine = self.program.machine_by_symbol(self.machine)?;
            let state = self.program.state_in_machine(machine, self.state)?;
            let reference =
                validation::expression_result_type_reference(self.program, machine, state, source)?;
            let value = self.structural_value(source, reference)?;
            self.retain_structural_value_root(value, reference)?;
            Some(value)
        }

        /// A runtime selector naming an immutable integer scalar local of this
        /// state: the temp symbol resolution hoists a computed subject index
        /// (`self.grid[self.r * 6 + self.c]`) into. It selects at the local's
        /// dense `Local` position, the coordinate every scalar read of that local
        /// in this computation uses.
        pub(super) fn local_selector(
            &self,
            selector: ExpressionHandle,
        ) -> Option<checked_trees::CheckedRuntimeIndex> {
            let (
                checked_trees::CheckedScalarExpression::Local {
                    position,
                    primitive_type,
                },
                _,
            ) = crate::values::scalar::scalar_lowering::lower_scalar_expression(
                self.program,
                self.operators,
                selector,
                self.parameters,
                self.authored_parameters,
                self.parameter_types,
                self.locals,
                self.exact_integer_casts,
            )?
            else {
                return None;
            };
            matches!(
                primitive_type,
                PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
            )
            .then_some(checked_trees::CheckedRuntimeIndex::Local {
                position: u32::try_from(position).ok()?,
            })
        }

        pub(super) fn dense_structural_parameter_index(
            &self,
            authored_position: usize,
        ) -> Option<u32> {
            crate::execution::terminal_unit::types::dense_structural_parameter_index(
                self.program,
                self.authored_parameters,
                authored_position,
            )
        }

        /* Membership and stored values share the same authored field computations. */
        pub(super) fn case_construction(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<CheckedScalarCaseConstruction> {
            let constructor = validation::scalar_case_constructor(self.program, expression)?;
            let mut fields = Vec::with_capacity(constructor.fields.len());
            for (symbol, source, primitive) in constructor.fields {
                let value = self.expression(source, primitive)?;
                self.plans.nodes.get_mut(value).authored_root = source;
                fields.push(CheckedScalarCaseComputationField { symbol, value });
            }
            let fields = self.plans.case_fields.insert_many(fields);
            Some(CheckedScalarCaseConstruction {
                expression,
                type_reference: constructor.type_reference,
                case: constructor.case,
                fields,
            })
        }
    }
}
mod completions {
    //! Selected Unit completions use ordinary evaluated dispatch operands and exact
    //! call occurrences. They create no value carrier or dummy result storage.

    use super::{Builder, ExpressionHandle, ExpressionNode};
    use checked_trees::{
        CheckedCompletionArm, CheckedCompletionExpression, CheckedCompletionExpressionHandle,
        CheckedCompletionExpressionKind as Kind, CheckedCompletionPlans, CheckedCompletionRefusal,
        CheckedCompletionRefusalReason as Refusal, CheckedCompletionRoot,
    };
    use std::collections::BTreeMap;

    impl Builder<'_, '_> {
        pub(super) fn record_completion(
            &mut self,
            expression: ExpressionHandle,
            plans: &mut CheckedCompletionPlans,
        ) -> Option<()> {
            if !matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::Match(_)
            ) {
                return None;
            }
            let (root, refusal) =
                match self.completion_node(expression, plans, &mut BTreeMap::new()) {
                    Ok(root) => (root, None),
                    Err(refusal) => (CheckedCompletionExpressionHandle::invalid(), Some(refusal)),
                };
            plans.roots.push(CheckedCompletionRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal: u32::try_from(self.statement_index).ok()?,
                expression,
                root,
                refusal,
            });
            refusal.is_none().then_some(())
        }

        fn completion_node(
            &mut self,
            expression: ExpressionHandle,
            plans: &mut CheckedCompletionPlans,
            visited: &mut BTreeMap<(u32, u32), Option<CheckedCompletionExpressionHandle>>,
        ) -> Result<CheckedCompletionExpressionHandle, CheckedCompletionRefusal> {
            let declined = |reason| CheckedCompletionRefusal { expression, reason };
            let coordinate = (expression.arena_index(), expression.generation());
            if let Some(previous) = visited.get(&coordinate) {
                return previous.ok_or_else(|| declined(Refusal::Cycle));
            }
            visited.insert(coordinate, None);
            let kind = match self.program.expression_table.expression(expression).clone() {
                ExpressionNode::Call(call) => {
                    let machine = self
                        .program
                        .machine_by_symbol(self.machine)
                        .ok_or_else(|| declined(Refusal::CallResult))?;
                    if !validation::call_returns_unit(self.program, machine, expression) {
                        return Err(declined(Refusal::CallResult));
                    }
                    let target = validation::call_target_symbol(self.program, &call);
                    let (source_call, _) = self
                        .call_ordinal(expression, target)
                        .ok_or_else(|| declined(Refusal::CallOccurrence))?;
                    Kind::Invocation { source_call }
                }
                ExpressionNode::Match(dispatch) => {
                    let (subject, prepared) = self
                        .prepare_dispatch(expression, &dispatch)
                        .ok_or_else(|| declined(Refusal::DispatchOperands))?;
                    let mut arms = Vec::with_capacity(prepared.len());
                    for arm in prepared {
                        let comparison = match (&subject, &arm.pattern) {
                            (
                                checked_trees::CheckedDispatchSubject::Scalar(subject),
                                checked_trees::CheckedScalarDispatchPattern::Value(_),
                            ) => {
                                let primitive = self.plans.nodes.get(*subject).primitive_type;
                                if primitive == super::PrimitiveType::Bool {
                                    checked_trees::CheckedCompletionComparison::Boolean
                                } else if primitive.is_fixed_integer()
                                    || primitive == super::PrimitiveType::Addr
                                {
                                    checked_trees::CheckedCompletionComparison::Integer
                                } else {
                                    return Err(declined(Refusal::Comparison));
                                }
                            }
                            (
                                _,
                                checked_trees::CheckedScalarDispatchPattern::Wildcard
                                | checked_trees::CheckedScalarDispatchPattern::Predicate(_),
                            ) => checked_trees::CheckedCompletionComparison::Predicate,
                            _ => return Err(declined(Refusal::Comparison)),
                        };
                        arms.push(CheckedCompletionArm {
                            source_arm: arm.source_arm,
                            equality_use: arm.equality_use,
                            comparison,
                            pattern: arm.pattern,
                            completion: self.completion_node(arm.value, plans, visited)?,
                        });
                    }
                    Kind::Dispatch {
                        subject,
                        arms: plans.arms.insert_many(arms),
                    }
                }
                _ => return Err(declined(Refusal::ExpressionKind)),
            };
            let normal_continuation = match &kind {
                Kind::Invocation { .. } => {
                    validation::expression_normal_completion_possible(self.program, expression)
                        .ok_or_else(|| declined(Refusal::NormalCompletion))?
                }
                Kind::Dispatch { arms, .. } => plans
                    .arms
                    .span(*arms)
                    .ok_or_else(|| declined(Refusal::DispatchOperands))?
                    .iter()
                    .any(|arm| plans.nodes.get(arm.completion).normal_continuation),
                Kind::Missing => return Err(declined(Refusal::ExpressionKind)),
            };
            let value = plans.nodes.append(CheckedCompletionExpression {
                expression,
                kind,
                normal_continuation,
            });
            visited.insert(coordinate, Some(value));
            Ok(value)
        }
    }
}
mod coordinate_reads {
    /*
        Entry coordinates are pure observations over an immutable formal telescope.
        The source owner proves selector formation before admitting that coordinate.
        Bind its pure scalar recipe directly to captured entry formals: scheduling an
        ordinary effectful read chain would put control before the rank exists.
        Geometry and scalar read templates remain shared with ordinary observations;
        no Indexed AST or later authored occurrence supplies entry authority.
    */
    use super::{
        Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
    };
    use checked_trees::{
        CheckedEntryIndexBound, CheckedScalarExpression, CheckedScalarReadProjection,
        CheckedScalarSelector, CheckedStorageRoot,
        CheckedStructuralPredicatePathSegment as Predicate,
        CheckedUnitStructuralPathSegment as Storage,
    };
    use typed_trees::types::PrimitiveType;

    impl Builder<'_, '_> {
        pub(super) fn scalar_coordinate(
            &mut self,
            expression: ExpressionHandle,
            coordinate: &validation::CanonicalPlace,
            expected_type: PrimitiveType,
            entry_indexes: &[CheckedEntryIndexBound],
        ) -> Option<CheckedScalarComputationHandle> {
            let facts::PlaceRoot::Symbol(symbol) = coordinate.root else {
                return None;
            };
            if coordinate.segments.is_empty() {
                let position = self
                    .parameters
                    .iter()
                    .position(|parameter| parameter.symbol == symbol)?;
                if self.parameter_types.get(position) != Some(&expected_type) {
                    return None;
                }
                return Some(self.insert(
                    expected_type,
                    CheckedScalarComputationKind::Value(CheckedScalarExpression::Parameter {
                        position,
                        primitive_type: expected_type,
                    }),
                ));
            }
            let geometry = validation::structural_scalar_coordinate(
                self.program,
                self.authored_parameters,
                symbol,
                &coordinate.segments,
            )?;
            if geometry.primitive_type != expected_type {
                return None;
            }
            let parameter_position = self
                .authored_parameters
                .iter()
                .position(|parameter| parameter.symbol == symbol)?;
            let mut path = Vec::new();
            for (path_step, step) in geometry.projections.iter().enumerate() {
                match step {
                    validation::ScalarCoordinateProjection::Storage(step) => {
                        let step = match step {
                            Storage::Referent => Predicate::Referent,
                            Storage::Field(field) => Predicate::Field(field.clone()),
                            Storage::Case(case) => Predicate::Case(case.clone()),
                            Storage::FixedIndex(index) => Predicate::FixedIndex(*index),
                            _ => return None,
                        };
                        path.push(CheckedScalarReadProjection::Static(step));
                    }
                    validation::ScalarCoordinateProjection::Element { .. } => {
                        path.push(CheckedScalarReadProjection::RuntimeIndex(
                            self.entry_index_selector(symbol, path_step, step, entry_indexes)?,
                        ));
                    }
                }
            }
            let (template, _) =
                crate::values::scalar::scalar_lowering::scalar_storage_read_projection(
                    self.program,
                    expression,
                    self.authored_parameters,
                    crate::values::scalar::scalar_lowering::ScalarStorageProjection {
                        root: CheckedStorageRoot::Parameter {
                            index: u32::try_from(parameter_position).ok()?,
                        },
                        path,
                        selected_type: geometry.result_type_reference,
                    },
                )?;
            Some(self.insert(expected_type, CheckedScalarComputationKind::Value(template)))
        }

        /*
            Bind an admitted entry selector against its immutable formal telescope.
            Scalar reads and whole-value copies consume the same exact receipt.
        */
        pub(super) fn entry_index_selector(
            &self,
            symbol: symbols::SymbolHandle,
            path_step: usize,
            projection: &validation::ScalarCoordinateProjection,
            entry_indexes: &[CheckedEntryIndexBound],
        ) -> Option<CheckedScalarSelector> {
            let validation::ScalarCoordinateProjection::Element {
                selector,
                collection_type,
                element_type,
                collection_path,
                extent,
                ..
            } = projection
            else {
                return None;
            };
            let path_step = u32::try_from(path_step).ok()?;
            let extent = extent.map(u64::try_from).transpose().ok()?;
            let mut bounds = entry_indexes.iter().filter(|bound| {
                bound.state == self.state
                    && bound.parameter == symbol
                    && bound.path_step == path_step
                    && bound.selector == *selector
                    && bound.collection_type == *collection_type
                    && bound.element_type == *element_type
                    && bound.collection_path == *collection_path
                    && bound.extent == extent
            });
            let bound = bounds.next()?;
            if bounds.next().is_some() {
                return None;
            }
            let selector_value = match selector {
                checked_trees::CheckedEntryIndexSelector::Expression(selector) => {
                    crate::values::scalar::scalar_lowering::pure_selector(
                        self.program,
                        self.operators,
                        *selector,
                        self.parameters,
                        self.authored_parameters,
                        self.parameter_types,
                        self.locals,
                        self.exact_integer_casts,
                    )?
                }
                checked_trees::CheckedEntryIndexSelector::Constant(value) => {
                    facts::u64_integer_literal(*value, bound.primitive_type)?
                }
            };
            if crate::values::scalar::expression_facts::scalar_expression_type(&selector_value)
                != Some(bound.primitive_type)
            {
                return None;
            }

            Some(CheckedScalarSelector {
                expression: selector_value,
                occurrence: ExpressionHandle::invalid(),
                entry_bound: Some(bound.clone()),
            })
        }
    }
}
mod declaration_inputs {
    /*
        Declaration inputs have one binding route in whole expressions and integer
        operands. Prefer a completed capture: its backing may already have changed.
        Otherwise observe the exact admitted destination under the source formal's
        carrier; arithmetic policy comes from that same formal type, never a hint.
    */
    use super::Builder;
    use checked_trees::CheckedScalarComputationHandle;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::TypeReferenceHandle;

    impl Builder<'_, '_> {
        pub(super) fn declaration_input(
            &mut self,
            expression: ExpressionHandle,
        ) -> Result<Option<(CheckedScalarComputationHandle, TypeReferenceHandle)>, &'static str>
        {
            if let Some(length) = self.declaration_input_length(expression)? {
                return Ok(Some(length));
            }
            let Some(place) = validation::canonical_place_from_expression(self.program, expression)
            else {
                return Ok(None);
            };
            let Some(symbol) = place.root.as_symbol() else {
                return Ok(None);
            };
            let mut captures = self
                .declaration_values
                .iter()
                .filter(|(formal, _)| *formal == place);
            if let Some((_, value)) = captures.next() {
                if captures.next().is_some() {
                    return Err("a declaration input has multiple completed captures");
                }
                let formal = self
                    .declaration_parameters
                    .iter()
                    .find(|formal| formal.symbol == symbol)
                    .ok_or("a captured declaration input has no source formal")?;
                let geometry = validation::structural_scalar_coordinate_in_type(
                    self.program,
                    place.root,
                    formal.type_reference,
                    &place.segments,
                )
                .ok_or("a captured declaration read has no exact formal geometry")?;
                if Some(geometry.primitive_type)
                    != Some(self.plans.nodes.get(*value).primitive_type)
                {
                    return Err("a completed declaration capture has the wrong formal carrier");
                }
                return Ok(Some((*value, geometry.result_type_reference)));
            }
            let mut bindings = self
                .declaration_bindings
                .iter()
                .filter(|(formal, _)| *formal == place);
            let Some((_, coordinate)) = bindings.next() else {
                return Ok(None);
            };
            if bindings.next().is_some() {
                return Err("a declaration input has multiple destination coordinates");
            }
            let coordinate = coordinate.clone();
            let formal = self
                .declaration_parameters
                .iter()
                .find(|formal| formal.symbol == symbol)
                .ok_or("a declaration coordinate has no source formal")?;
            let reference = validation::structural_scalar_coordinate_in_type(
                self.program,
                place.root,
                formal.type_reference,
                &place.segments,
            )
            .ok_or("a declaration coordinate has no exact scalar geometry")?
            .result_type_reference;
            let primitive = self
                .program
                .primitive_type_reference(reference)
                .ok_or("a declaration coordinate has no fixed scalar carrier")?;
            let value = self
                .scalar_coordinate(expression, &coordinate, primitive, &[])
                .ok_or("a declaration coordinate lacks a supported entry observation")?;
            Ok(Some((value, reference)))
        }

        /*
            A rebound sequence formal's length reads the destination's descriptor.
            The formal's own geometry has no scalar coordinate for `len`, so the
            ordinary binding route above cannot reach it.
        */
        fn declaration_input_length(
            &mut self,
            expression: ExpressionHandle,
        ) -> Result<Option<(CheckedScalarComputationHandle, TypeReferenceHandle)>, &'static str>
        {
            let Some(scope) = self.declaration_scope.as_ref() else {
                return Ok(None);
            };
            if self.declaration_bindings.is_empty() {
                return Ok(None);
            }
            let Some(member) = self.program.expression_table.member_node(expression) else {
                return Ok(None);
            };
            let Some(receiver) =
                validation::canonical_place_from_expression(self.program, member.receiver)
            else {
                return Ok(None);
            };
            let mut bindings = self
                .declaration_bindings
                .iter()
                .filter(|(formal, _)| *formal == receiver);
            let Some((_, destination)) = bindings.next() else {
                return Ok(None);
            };
            if bindings.next().is_some() {
                return Err("a declaration input has multiple destination coordinates");
            }
            let Some(symbol) = receiver.root.as_symbol() else {
                return Ok(None);
            };
            let formal = self
                .declaration_parameters
                .iter()
                .find(|formal| formal.symbol == symbol)
                .ok_or("a declaration coordinate has no source formal")?;
            let formal_type = validation::project_type_reference_from_segments(
                self.program,
                formal.type_reference,
                &receiver.segments,
            )
            .ok_or("a rebound sequence formal has no exact geometry")?;
            if validation::collection_operations::collection_measure_member(
                self.program,
                member,
                validation::collection_operations::MeasureReceiver::Declared(formal_type),
            ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
            {
                return Ok(None);
            }
            let Some(parameter) = destination.root.as_symbol() else {
                return Err("a rebound sequence length has no parameter destination");
            };
            let (_, state) = validation::find_state(self.program, self.state)
                .ok_or("a rebound sequence length has no evaluating state")?;
            let (_, _, value) = validation::ranking_slice_length_observation(
                self.program,
                state,
                &validation::RankSubjectCoordinate {
                    parameter,
                    path: destination.segments.clone(),
                },
            )
            .ok_or("a rebound sequence length has no descriptor observation")?;
            let owner = scope
                .plan
                .application_formations
                .get(scope.application)
                .application
                .owner;
            let (owner_machine, owner_state) = validation::find_state(self.program, owner)
                .ok_or("a rebound sequence length has no declaring state")?;
            let reference = validation::expression_result_type_reference(
                self.program,
                owner_machine,
                owner_state,
                expression,
            )
            .ok_or("a rebound sequence length has no result type")?;
            let value = self.insert(
                typed_trees::types::PrimitiveType::U64,
                super::CheckedScalarComputationKind::Value(value),
            );
            Ok(Some((value, reference)))
        }

        /*
            Pure lowering owns a scalar telescope, so a subtree that refers to
            an explicitly rebound declaration input must first pass through the
            computation graph. The shared typed-reference walk owns all expression
            forms; independent literals and ordinary formals retain their fast path.
            A completed capture also blocks the shortcut without a coordinate row:
            its saved value must not be replaced by the ordinary formal parameter.
        */
        pub(super) fn has_declaration_binding(&self, expression: ExpressionHandle) -> bool {
            if self.declaration_bindings.is_empty() && self.declaration_values.is_empty() {
                return false;
            }
            struct Bindings<'bindings> {
                bindings: &'bindings [(validation::CanonicalPlace, validation::CanonicalPlace)],
                captures:
                    &'bindings [(validation::CanonicalPlace, CheckedScalarComputationHandle)],
                found: bool,
                visited: Vec<ExpressionHandle>,
            }
            impl Extend<SymbolHandle> for Bindings<'_> {
                fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
                    self.found |= symbols.into_iter().any(|symbol| {
                        self.bindings
                            .iter()
                            .any(|(formal, _)| formal.root == facts::PlaceRoot::Symbol(symbol))
                            || self
                                .captures
                                .iter()
                                .any(|(formal, _)| formal.root == facts::PlaceRoot::Symbol(symbol))
                    });
                }
            }
            impl typed_trees::references::TypedReferences for Bindings<'_> {
                fn expression(&mut self, expression: ExpressionHandle) -> bool {
                    if self.visited.contains(&expression) {
                        return false;
                    }
                    self.visited.push(expression);
                    true
                }
                fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
                    false
                }
            }
            let mut bindings = Bindings {
                bindings: self.declaration_bindings,
                captures: &self.declaration_values,
                found: false,
                visited: Vec::new(),
            };
            typed_trees::references::visit_expression(self.program, &mut bindings, expression);
            bindings.found
        }
    }
}
pub(crate) use dispatch::structural_dispatch_omits_arm;
mod dispatch {

    /*
        A dispatch retains one subject computation and its covered authored prefix.
    */
    use super::{
        Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind,
        CheckedScalarExpressionRole, ExpressionHandle, ExpressionNode, PrimitiveType,
    };
    use crate::flow::expressions;
    use crate::values::operator_is_builtin;
    use crate::values::scalar::expression_facts::is_integer;
    use crate::values::scalar_expression_type;
    use checked_trees::{
        CheckedScalarComputationStructuralArgument, CheckedScalarDispatchArm,
        CheckedScalarDispatchPattern, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan,
    };
    use symbols::SymbolHandle;
    use typed_trees::expression::{MatchPattern, TableMatchExpression};

    /*
        An authored structural arm that can never run: its pattern condition is
        statically false or unreachable, or it names a case whose value sits in
        an unreachable static type context.
    */
    fn structural_dispatch_arm_is_impossible(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        dispatch: &TableMatchExpression,
        arm: &typed_trees::expression::TableMatchArm,
    ) -> bool {
        match arm.pattern {
            MatchPattern::Destructure(pattern) => {
                program.static_type_context_is_unreachable(pattern.condition)
                    || program.static_boolean_value(pattern.condition) == Some(false)
            }
            MatchPattern::Value(_) if program.static_type_context_is_unreachable(arm.value) => {
                let Some(machine) = program.machine_by_symbol(machine) else {
                    return false;
                };
                let Some(state) = program.state_in_machine(machine, state) else {
                    return false;
                };
                validation::structural_match_pattern_case(
                    program,
                    machine,
                    state,
                    dispatch,
                    arm.pattern,
                )
                .is_some_and(|case| case.is_valid())
            }
            _ => false,
        }
    }

    /*
        Whether a structural dispatch plans no arm for this authored arm. The
        dispatch roster and the normal-result tree both ask here, so an arm the
        roster leaves out is exactly an arm the result tree may find missing.
        Flow still records a completed result for a ruled-out arm it walked.
    */
    pub(crate) fn structural_dispatch_omits_arm(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        dispatch: &TableMatchExpression,
        arm: &typed_trees::expression::TableMatchArm,
    ) -> bool {
        structural_dispatch_arm_is_impossible(program, machine, state, dispatch, arm)
            || program.static_type_context_is_unreachable(arm.value)
    }

    impl Builder<'_, '_> {
        /*
            Shared operand primitive when one use is an admitted selected
            comparison. IEEE float requirements carry their canonical operation
            identity; integer boundary comparisons carry their authored spelling.
            Neither classification realizes provider semantics here.
        */
        pub(super) fn selected_comparison_primitive(
            &self,
            operator_use: arena::Handle<checked_trees::CheckedOperatorUseFact>,
        ) -> Option<PrimitiveType> {
            self.operators
                .selected_float_comparison(self.program, operator_use)
                .map(|(_, primitive)| primitive)
                .or_else(|| {
                    self.operators
                        .selected_integer_comparison(self.program, operator_use)
                        .map(|(_, primitive)| primitive)
                })
        }

        pub(super) fn comparison_use(
            &self,
            expression: ExpressionHandle,
            occurrence: checked_trees::CheckedOperatorOccurrence,
        ) -> Option<arena::Handle<checked_trees::CheckedOperatorUseFact>> {
            let mut matching = self.operators.uses.iter().filter_map(|(handle, selected)| {
                (selected.expression == expression && selected.occurrence == occurrence
                    && matches!(selected.origin, checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, state_symbol, statement_index, .. }
                        if machine_symbol == self.machine && state_symbol == self.state && statement_index == self.statement_index)
                    && self.selected_comparison_primitive(handle).is_some()).then_some(handle)
            });
            let selected = matching.next()?;
            matching.next().is_none().then_some(selected)
        }
        pub(super) fn dispatch(
            &mut self,
            source_expression: ExpressionHandle,
            dispatch: &TableMatchExpression,
            result_type: PrimitiveType,
        ) -> Option<CheckedScalarComputationHandle> {
            if let Some(selected) = validation::select_anonymous_numeric_match_arm(
                self.program,
                dispatch,
                |expression| operator_is_builtin(self.operators, expression),
            ) {
                return self.expression(selected, result_type);
            }
            let admitted_arms = self.declaration_scope.as_ref().and_then(|scope| {
                let application = scope.plan.application_formations.get(scope.application);
                let arms = application
                    .application
                    .dispatches
                    .handles()
                    .filter_map(|handle| {
                        let arm = scope.plan.application_dispatches.get(handle);
                        (arm.expression == source_expression).then_some(arm.source_arm)
                    })
                    .collect::<Vec<_>>();
                (!arms.is_empty()).then_some(arms)
            });
            let subject = self.dispatch_subject(dispatch)?;
            if let Some(admitted_arms) = admitted_arms {
                return self.declaration_dispatch(
                    source_expression,
                    dispatch,
                    subject,
                    result_type,
                    &admitted_arms,
                );
            }
            if matches!(
                subject,
                checked_trees::CheckedDispatchSubject::Structural { .. }
            ) {
                return self.structural_subject_dispatch(
                    source_expression,
                    dispatch,
                    subject,
                    result_type,
                );
            }
            let checked_trees::CheckedDispatchSubject::Scalar(subject_handle) = subject else {
                return None;
            };
            let subject_type = self.plans.nodes.get(subject_handle).primitive_type;
            let reachable = dispatch
                .arms
                .handles()
                .zip(self.program.expression_table.match_arms(dispatch.arms))
                .filter(|(_, arm)| !self.program.static_type_context_is_unreachable(arm.value))
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>();
            if reachable.len() < dispatch.arms.len()
                && !matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64)
            {
                return self.declaration_dispatch(
                    source_expression,
                    dispatch,
                    subject,
                    result_type,
                    &reachable,
                );
            }
            let subject = subject_handle;
            let authored = self.program.expression_table.match_arms(dispatch.arms);
            let mut arms = Vec::new();
            let mut boolean_coverage = [false; 2];
            let mut covered = false;
            for (ordinal, arm) in authored.iter().enumerate() {
                let source_arm = arena::Handle::from_parts(
                    dispatch
                        .arms
                        .start()
                        .arena_index()
                        .checked_add(u32::try_from(ordinal).ok()?)?,
                    dispatch.arms.start().generation(),
                );
                let equality_use = if matches!(arm.pattern, MatchPattern::Value(_))
                    && matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64)
                {
                    self.comparison_use(
                        source_expression,
                        checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm },
                    )?
                } else {
                    arena::Handle::invalid()
                };
                let pattern = match arm.pattern {
                    MatchPattern::Destructure(_) => return None,
                    MatchPattern::Wildcard => {
                        covered = true;
                        CheckedScalarDispatchPattern::Wildcard
                    }
                    MatchPattern::Value(expression) => {
                        if subject_type == PrimitiveType::Bool
                            && let ExpressionNode::Boolean(value) =
                                self.program.expression_table.expression(expression)
                        {
                            boolean_coverage[usize::from(*value)] = true;
                            covered = boolean_coverage.iter().all(|value| *value);
                        }
                        CheckedScalarDispatchPattern::Value(
                            self.expression(expression, subject_type)?,
                        )
                    }
                };
                let value = self.expression(arm.value, result_type)?;
                arms.push(CheckedScalarDispatchArm {
                    equality_use,
                    source_arm,
                    pattern,
                    value,
                });
                if covered {
                    break;
                }
            }
            if !covered {
                return None;
            }
            let arms = self.plans.dispatch_arms.insert_many(arms);
            Some(self.insert(
                result_type,
                CheckedScalarComputationKind::Dispatch {
                    source_expression,
                    subject: checked_trees::CheckedDispatchSubject::Scalar(subject),
                    arms,
                },
            ))
        }

        /*
            Consume the checked path union without importing branch assumptions or
            forming an unreachable child. Failed pattern evaluations still precede
            the next feasible pattern; the final feasible arm covers the admitted
            input, and evaluates its own pattern before producing its value.
        */
        fn declaration_dispatch(
            &mut self,
            source_expression: ExpressionHandle,
            dispatch: &TableMatchExpression,
            subject: checked_trees::CheckedDispatchSubject,
            result_type: PrimitiveType,
            admitted: &[arena::Handle<typed_trees::expression::TableMatchArm>],
        ) -> Option<CheckedScalarComputationHandle> {
            let checked_trees::CheckedDispatchSubject::Scalar(subject) = subject else {
                return None;
            };
            let authored = self.program.expression_table.match_arms(dispatch.arms);
            let handles = dispatch.arms.handles().collect::<Vec<_>>();
            if admitted.iter().any(|arm| !handles.contains(arm)) {
                return None;
            }
            let last = handles.iter().rposition(|arm| admitted.contains(arm))?;
            let subject_type = self.plans.nodes.get(subject).primitive_type;
            let mut pending = Vec::new();
            let mut arms = Vec::new();
            for (ordinal, (source_arm, arm)) in
                handles.iter().zip(authored).enumerate().take(last + 1)
            {
                match arm.pattern {
                    MatchPattern::Value(pattern) => {
                        pending.push(self.expression(pattern, subject_type)?)
                    }
                    MatchPattern::Wildcard => {}
                    MatchPattern::Destructure(_) => return None,
                }
                if !admitted.contains(source_arm) {
                    continue;
                }
                let value = self.expression(arm.value, result_type)?;
                let (pattern, value) = if ordinal == last {
                    pending.push(value);
                    (
                        CheckedScalarDispatchPattern::Wildcard,
                        self.dispatch_prefix(
                            source_expression,
                            result_type,
                            std::mem::take(&mut pending),
                        )?,
                    )
                } else {
                    if matches!(arm.pattern, MatchPattern::Wildcard) {
                        return None;
                    }
                    let pattern = self.dispatch_prefix(
                        source_expression,
                        subject_type,
                        std::mem::take(&mut pending),
                    )?;
                    (CheckedScalarDispatchPattern::Value(pattern), value)
                };
                arms.push(CheckedScalarDispatchArm {
                    equality_use: arena::Handle::invalid(),
                    source_arm: *source_arm,
                    pattern,
                    value,
                });
            }
            let arms = self.plans.dispatch_arms.insert_many(arms);
            Some(self.insert(
                result_type,
                CheckedScalarComputationKind::Dispatch {
                    source_expression,
                    subject: checked_trees::CheckedDispatchSubject::Scalar(subject),
                    arms,
                },
            ))
        }

        fn dispatch_prefix(
            &mut self,
            source_expression: ExpressionHandle,
            primitive_type: PrimitiveType,
            operands: Vec<CheckedScalarComputationHandle>,
        ) -> Option<CheckedScalarComputationHandle> {
            let position = operands.len().checked_sub(1)?;
            if position == 0 {
                return operands.first().copied();
            }
            let operands = self.plans.operands.insert_many(operands);
            Some(self.insert(
                primitive_type,
                CheckedScalarComputationKind::Apply {
                    source_expression,
                    expression: checked_trees::CheckedScalarExpression::Parameter {
                        position,
                        primitive_type,
                    },
                    operands,
                },
            ))
        }

        /* Retain one ordinary evaluated value, independent of its authored place. */
        pub(super) fn dispatch_subject(
            &mut self,
            dispatch: &TableMatchExpression,
        ) -> Option<checked_trees::CheckedDispatchSubject> {
            let machine = self.program.machine_by_symbol(self.machine)?;
            let state = self.program.state_in_machine(machine, self.state)?;
            if let Some(reference) =
                validation::structural_match_subject_type(self.program, machine, state, dispatch)
            {
                let alias = if dispatch.capture.is_valid() {
                    expressions(self.program)
                        .value_capture(dispatch.capture)
                        .value_anchor
                } else {
                    ExpressionHandle::invalid()
                };
                // Retaining a noncopy owner uses the existing operand path: it
                // keeps that exact home and access until an arm consumes it.
                // A borrowed subject still captures only its admitted view.
                let retains_owned_place = self.program.type_multiplicity(reference)
                    != language_semantics::Multiplicity::Unrestricted
                    && super::structural_values::names_owned_structural_value(
                        self.program,
                        reference,
                    )
                    && validation::ownership_place_from_expression_in_state(
                        self.program,
                        self.state,
                        self.statement_index,
                        dispatch.subject,
                    )
                    .and_then(|place| {
                        validation::canonical_owned_place_type_reference(
                            self.program,
                            self.state,
                            self.statement_index,
                            &place,
                        )
                    })
                    .is_some_and(|owned| {
                        validation::declared_carriers_match(self.program, owned, reference)
                    });
                // A source-checked place is observed under its existing custody.
                // Copy captures and stored descriptors keep their independent value
                // producer; neither path can turn an affine referent into an owner.
                if !retains_owned_place
                    && !self.uses_captured_dispatch_value(dispatch.subject)
                    && validation::reference_shell(self.program, reference).is_none()
                    && let Some(place) = self.canonical_place(dispatch.subject)
                {
                    let place = if place.root.as_symbol().is_some() {
                        Some(
                            self.borrowed_leaf_argument(
                                dispatch.subject,
                                reference,
                                dispatch.subject,
                            )?
                            .0,
                        )
                    } else {
                        self.readable_case_subject(dispatch.subject, self.statement_index)
                            .map(|(_, place)| place)
                    };
                    if let Some(place) = place {
                        // Even a tag-only selection captures its endpoint once.
                        // With no lexical binding the exact subject occurrence is
                        // its private completed-expression name.
                        let alias = if alias.is_valid() {
                            alias
                        } else {
                            dispatch.subject
                        };
                        self.retain_dispatch_receiver(
                            alias,
                            checked_trees::CheckedStructuralValueHandle::invalid(),
                            reference,
                            place.access,
                        )?;
                        return Some(checked_trees::CheckedDispatchSubject::Structural {
                            expression: dispatch.subject,
                            value: checked_trees::CheckedStructuralValueHandle::invalid(),
                            alias,
                            place,
                        });
                    }
                }
                let value = self.structural_value(dispatch.subject, reference)?;
                self.retained_argument_owner(value, reference)?;
                self.retain_structural_value_root(value, reference)?;
                self.retain_dispatch_receiver(
                    alias,
                    value,
                    reference,
                    CheckedStructuralAccess::Owned,
                )?;
                return Some(checked_trees::CheckedDispatchSubject::Structural {
                    expression: dispatch.subject,
                    value,
                    alias,
                    place: CheckedUnitStructuralArgumentPlan {
                        projection_evaluations: Default::default(),
                        source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                            value_dependency: None,
                            expression: if alias.is_valid() {
                                alias
                            } else {
                                dispatch.subject
                            },
                            logical_root_type_identity: None,
                        },
                        path: Vec::new(),
                        type_identity: validation::stored_structural_type_identity(
                            self.program,
                            reference,
                            &[],
                            &[],
                        ),
                        access: CheckedStructuralAccess::SharedBorrow,
                    },
                });
            }
            let subject = if let Some(subject_type) =
                validation::match_subject_primitive_type(self.program, dispatch)
            {
                if subject_type != PrimitiveType::Bool
                    && !is_integer(subject_type)
                    && !matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64)
                {
                    return None;
                }
                self.expression(dispatch.subject, subject_type)?
            } else if let Some(operand) = self.integer_operand(dispatch.subject, None)
                && scalar_expression_type(&operand.value).is_some_and(is_integer)
            {
                self.materialize_integer(operand)?
            } else {
                self.expression(dispatch.subject, PrimitiveType::Bool)?
            };
            Some(checked_trees::CheckedDispatchSubject::Scalar(subject))
        }

        fn retain_dispatch_receiver(
            &mut self,
            alias: ExpressionHandle,
            value: checked_trees::CheckedStructuralValueHandle,
            type_reference: typed_trees::types::TypeReferenceHandle,
            access: CheckedStructuralAccess,
        ) -> Option<()> {
            let receiver = super::builder::DispatchReceiver {
                value,
                type_reference,
                access,
            };
            if !alias.is_valid() {
                return Some(());
            }
            match self
                .dispatch_receivers
                .entry((alias.arena_index(), alias.generation()))
            {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(receiver);
                    Some(())
                }
                std::collections::btree_map::Entry::Occupied(entry) => {
                    (*entry.get() == receiver).then_some(())
                }
            }
        }

        fn structural_subject_dispatch(
            &mut self,
            source_expression: ExpressionHandle,
            dispatch: &TableMatchExpression,
            subject: checked_trees::CheckedDispatchSubject,
            result_type: PrimitiveType,
        ) -> Option<CheckedScalarComputationHandle> {
            let authored = self
                .program
                .expression_table
                .match_arms(dispatch.arms)
                .to_vec();
            let mut arms = Vec::new();
            let mut rejected_patterns = Vec::new();
            for (ordinal, arm) in authored.iter().enumerate() {
                if self.structural_dispatch_arm_is_impossible(dispatch, arm) {
                    continue;
                }
                let source_arm = dispatch.arms.handle_at(u32::try_from(ordinal).ok()?);
                let mut pattern = self.structural_dispatch_pattern(
                    source_expression,
                    dispatch,
                    &subject,
                    arm.pattern,
                )?;
                if self.program.static_type_context_is_unreachable(arm.value) {
                    rejected_patterns.push(checked_trees::CheckedScalarEvaluationOperand::Scalar(
                        pattern.computation()?,
                    ));
                    continue;
                }
                let mut value = self.expression(arm.value, result_type)?;
                if !rejected_patterns.is_empty() {
                    let operands = self
                        .plans
                        .evaluation_operands
                        .insert_many(std::mem::take(&mut rejected_patterns));
                    match pattern {
                        CheckedScalarDispatchPattern::Predicate(predicate) => {
                            pattern = CheckedScalarDispatchPattern::Predicate(self.insert(
                                PrimitiveType::Bool,
                                CheckedScalarComputationKind::WithOperands {
                                    source_expression: ExpressionHandle::invalid(),
                                    operands,
                                    operand: predicate,
                                },
                            ));
                        }
                        CheckedScalarDispatchPattern::Wildcard => {
                            value = self.insert(
                                result_type,
                                CheckedScalarComputationKind::WithOperands {
                                    source_expression: ExpressionHandle::invalid(),
                                    operands,
                                    operand: value,
                                },
                            );
                        }
                        CheckedScalarDispatchPattern::Value(_) => return None,
                    }
                }
                arms.push(CheckedScalarDispatchArm {
                    equality_use: arena::Handle::invalid(),
                    source_arm,
                    pattern,
                    value,
                });
                if matches!(arm.pattern, MatchPattern::Wildcard) {
                    break;
                }
            }
            if !rejected_patterns.is_empty() {
                return None;
            }
            let arms = self.plans.dispatch_arms.insert_many(arms);
            Some(self.insert(
                result_type,
                CheckedScalarComputationKind::Dispatch {
                    source_expression,
                    subject,
                    arms,
                },
            ))
        }

        /*
            The subject has already completed. A condition with no prefix arrival,
            or a false condition built from its pure tag test, evaluates no guard.
            A body contradiction alone cannot erase a reachable guard's effects.
        */
        pub(super) fn structural_dispatch_arm_is_impossible(
            &self,
            dispatch: &TableMatchExpression,
            arm: &typed_trees::expression::TableMatchArm,
        ) -> bool {
            structural_dispatch_arm_is_impossible(
                self.program,
                self.machine,
                self.state,
                dispatch,
                arm,
            )
        }

        /*
            Structural predicates use the same checked Boolean computation as
            authored conjunctions. Normalization owns their tag/field/guard order;
            this consumer does not reconstruct a second conditional schedule.
        */
        pub(super) fn structural_dispatch_pattern(
            &mut self,
            expression: ExpressionHandle,
            dispatch: &TableMatchExpression,
            subject: &checked_trees::CheckedDispatchSubject,
            pattern: MatchPattern,
        ) -> Option<CheckedScalarDispatchPattern> {
            if matches!(pattern, MatchPattern::Wildcard) {
                return Some(CheckedScalarDispatchPattern::Wildcard);
            }
            let checked_trees::CheckedDispatchSubject::Structural {
                place,
                value: receiver,
                alias,
                ..
            } = subject
            else {
                return None;
            };
            let machine = self.program.machine_by_symbol(self.machine)?;
            let state = self.program.state_in_machine(machine, self.state)?;
            let case = validation::structural_match_pattern_case(
                self.program,
                machine,
                state,
                dispatch,
                pattern,
            )?;
            let selected = match pattern {
                MatchPattern::Destructure(pattern) => {
                    return Some(CheckedScalarDispatchPattern::Predicate(
                        self.expression(pattern.condition, PrimitiveType::Bool)?,
                    ));
                }
                MatchPattern::Value(_) => {
                    if !case.is_valid() {
                        return None;
                    }
                    self.publish_case_membership(
                        expression,
                        ExpressionHandle::invalid(),
                        *receiver,
                        CheckedScalarComputationStructuralArgument::Place(if receiver.is_valid() {
                            place.clone()
                        } else {
                            CheckedUnitStructuralArgumentPlan {
                                source:
                                    CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                        value_dependency: None,
                                        expression: *alias,
                                        logical_root_type_identity: None,
                                    },
                                path: Vec::new(),
                                projection_evaluations: Default::default(),
                                type_identity: place.type_identity,
                                access: place.access,
                            }
                        }),
                        case,
                    )
                }
                MatchPattern::Wildcard => return None,
            };
            Some(CheckedScalarDispatchPattern::Predicate(selected))
        }
    }

    /// The comparison schedule is independent of the selected result's carrier.
    pub(super) struct PreparedDispatchArm {
        pub(super) source_arm: arena::Handle<typed_trees::expression::TableMatchArm>,
        pub(super) equality_use: arena::Handle<checked_trees::CheckedOperatorUseFact>,
        pub(super) pattern: CheckedScalarDispatchPattern,
        pub(super) value: ExpressionHandle,
    }

    impl Builder<'_, '_> {
        pub(super) fn prepare_dispatch(
            &mut self,
            expression: ExpressionHandle,
            dispatch: &TableMatchExpression,
        ) -> Option<(
            checked_trees::CheckedDispatchSubject,
            Vec<PreparedDispatchArm>,
        )> {
            let subject = self.dispatch_subject(dispatch)?;
            let subject_type = match subject {
                checked_trees::CheckedDispatchSubject::Scalar(value) => {
                    self.push_scalar_root(
                        CheckedScalarExpressionRole::StructuralValueSubject { expression },
                        value,
                    )?;
                    Some(self.plans.nodes.get(value).primitive_type)
                }
                checked_trees::CheckedDispatchSubject::Structural { .. } => None,
            };
            let authored = self
                .program
                .expression_table
                .match_arms(dispatch.arms)
                .to_vec();
            let mut arms = Vec::new();
            let mut covered = false;
            let mut boolean_coverage = [false; 2];
            for (ordinal, arm) in authored.iter().enumerate() {
                if subject_type.is_none()
                    && self.structural_dispatch_arm_is_impossible(dispatch, arm)
                {
                    continue;
                }
                let source_arm = dispatch.arms.handle_at(u32::try_from(ordinal).ok()?);
                let mut equality_use = arena::Handle::invalid();
                let pattern = if let Some(subject_type) = subject_type {
                    match arm.pattern {
                        MatchPattern::Wildcard => {
                            covered = true;
                            checked_trees::CheckedScalarDispatchPattern::Wildcard
                        }
                        MatchPattern::Value(pattern) => {
                            if subject_type == PrimitiveType::Bool
                                && let Some(value) =
                                    self.program.expression_table.boolean_node(pattern)
                            {
                                if boolean_coverage[usize::from(*value)] {
                                    continue;
                                }
                                boolean_coverage[usize::from(*value)] = true;
                                covered = boolean_coverage == [true, true];
                            }
                            if matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64) {
                                equality_use = self.comparison_use(
                                    expression,
                                    checked_trees::CheckedOperatorOccurrence::MatchEquality {
                                        source_arm,
                                    },
                                )?;
                            }
                            let computation = self.expression(pattern, subject_type)?;
                            self.plans.nodes.get_mut(computation).authored_root = pattern;
                            self.push_scalar_root(
                                CheckedScalarExpressionRole::StructuralValuePattern { source_arm },
                                computation,
                            )?;
                            checked_trees::CheckedScalarDispatchPattern::Value(computation)
                        }
                        MatchPattern::Destructure(_) => return None,
                    }
                } else {
                    self.structural_dispatch_pattern(expression, dispatch, &subject, arm.pattern)?
                };
                if let Some(computation) = pattern.computation()
                    && subject_type.is_none()
                {
                    self.push_scalar_root(
                        CheckedScalarExpressionRole::StructuralValuePattern { source_arm },
                        computation,
                    )?;
                }
                arms.push(PreparedDispatchArm {
                    source_arm,
                    equality_use,
                    pattern,
                    value: arm.value,
                });
                if matches!(arm.pattern, MatchPattern::Wildcard) || covered {
                    break;
                }
            }
            // Structural coverage was established by the common pattern
            // checker, including case completeness and guarded-arm limits.
            if (!covered && subject_type.is_some()) || arms.is_empty() {
                return None;
            }
            Some((subject, arms))
        }
    }
}
mod dispatch_copies {
    /*
        Selected structural payload captures use the ordinary copied-value producer.
        The completed-flow receipt supplies the saved receiver and entry epoch;
        the value graph retains one owned result before any target operand runs.
    */
    use super::builder::Builder;
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use checked_trees::{
        CheckedStructuralAccess, CheckedStructuralValueKind as ValueKind,
        CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralPathSegment as PathSegment,
    };

    impl Builder<'_, '_> {
        pub(super) fn retain_dispatch_copy_roots(&mut self) {
            let Some(state) = self.flow.control.unique_state(self.machine, self.state) else {
                return;
            };
            let copies = self
                .flow
                .control
                .outgoing
                .span_or_empty(state.outgoing)
                .iter()
                .filter(|outgoing| outgoing.statement_index == self.statement_index)
                .flat_map(|outgoing| {
                    self.flow
                        .control
                        .operands
                        .span_or_empty(outgoing.copied_payloads)
                        .iter()
                        .copied()
                })
                .collect::<Vec<_>>();
            for copy in copies {
                if self
                    .program
                    .primitive_type_reference(copy.type_reference)
                    .is_some()
                    && validation::reference_shell(self.program, copy.type_reference).is_none()
                {
                    continue;
                }
                let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } =
                    copy.completion
                else {
                    continue;
                };
                if self
                    .structural_values
                    .root_for_captured_copy(self.state, self.statement_index as u32, identity)
                    .is_some()
                {
                    continue;
                }
                let Some(kind) = self.dispatch_copy_kind(copy) else {
                    continue;
                };
                let Some(value) =
                    self.publish_structural_value(copy.expression, copy.type_reference, kind)
                else {
                    continue;
                };
                self.structural_values
                    .roots
                    .append(checked_trees::CheckedStructuralValueRoot {
                        captured_copy: identity,
                        machine: self.machine,
                        state: self.state,
                        statement_ordinal: self.statement_index as u32,
                        expression: copy.expression,
                        type_reference: copy.type_reference,
                        root: value,
                    });
            }
        }

        pub(super) fn captured_dispatch_value(
            &self,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<&checked_trees::CheckedStructuralValueRoot> {
            let copy = self.captured_dispatch_operand(expression)?;
            let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } =
                copy.completion
            else {
                return None;
            };
            self.structural_values.root_for_captured_copy(
                self.state,
                self.statement_index as u32,
                identity,
            )
        }

        fn captured_dispatch_operand(
            &self,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<&checked_trees::FlowOperandFact> {
            if !self.declaration_parameters.is_empty() {
                return None;
            }
            captured_dispatch_operand(
                self.program,
                self.flow,
                self.machine,
                self.state,
                self.statement_index,
                expression,
            )
        }

        pub(super) fn uses_captured_dispatch_value(
            &self,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> bool {
            uses_captured_dispatch_value(self.program, self.captured_dispatch_operand(expression))
        }

        /*
            A pure storage plan cannot stand in for a completed value occurrence.
            Formal/generated computations have no selected-copy flow use.
        */
        pub(super) fn observes_captured_dispatch_value(
            &self,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> bool {
            self.declaration_parameters.is_empty()
                && observes_captured_dispatch_value(
                    self.program,
                    self.flow,
                    self.machine,
                    self.state,
                    self.statement_index,
                    expression,
                )
        }

        fn dispatch_copy_kind(&self, copy: checked_trees::FlowOperandFact) -> Option<ValueKind> {
            let receiver = self.flow.control.operands.get(copy.backing);
            let place = self.canonical_place(receiver.expression)?;
            if !place.segments.is_empty() {
                return None;
            }
            let symbol = place.root.as_symbol()?;
            let member = self.program.expression_table.member_node(copy.expression)?;
            let field_symbol =
                validation::effective_member_symbol(self.program, member.receiver, member);
            let case_symbol = facts::payload_variant_for_field(self.program, field_symbol)?;
            let (_, case) = self.program.variant_owner(case_symbol)?;
            let field = self
                .program
                .data_payload_field_by_symbol(case, field_symbol)?;
            let mut path = Vec::new();
            let mut reference = receiver.type_reference;
            while let Some(shell) = validation::reference_shell(self.program, reference) {
                if !shell.access.is_readable() {
                    return None;
                }
                path.push(PathSegment::Referent);
                reference = shell.referee;
            }
            path.push(PathSegment::Case(
                checked_trees::CheckedMemberIdentity::of_member(case.identity, case.name.clone()),
            ));
            path.push(PathSegment::Field(
                checked_trees::CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
            ));
            Some(ValueKind::CopiedStructuralPlace {
                source: CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                    path,
                    type_identity: validation::stored_structural_type_identity(
                        self.program,
                        copy.type_reference,
                        &[],
                        &[],
                    ),
                    access: CheckedStructuralAccess::SharedBorrow,
                },
                leaf: copy.type_reference,
            })
        }
    }

    impl Builder<'_, '_> {
        /*
            Copy a projected value from its already completed owner. This is the
            same ordinary copied-place operation as a local/parameter read, with
            its relative source supplied by the authenticated flow occurrence.
        */
        pub(super) fn copied_dispatch_place(
            &mut self,
            expression: checked_trees::expression::ExpressionHandle,
            expected: typed_trees::types::TypeReferenceHandle,
        ) -> Option<ValueKind> {
            if !validation::type_satisfies_declared_property(
                self.program,
                &[],
                expected,
                validation::DeclaredPropertyRequirement::Copy,
            ) {
                return None;
            }
            let state = self.program.state_by_symbol(self.state)?;
            let transition = self
                .program
                .statement_table
                .statement_at(state, self.statement_index)?
                .as_transition()?;
            let (_, copy) = crate::flow::copied_payload_at(
                self.program,
                self.flow,
                self.machine,
                self.state,
                self.statement_index,
                checked_trees::FlowPermissionEvaluation::TransitionTarget(transition.target),
                expression,
            )?;
            let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } =
                copy.completion
            else {
                return None;
            };
            let saved = self
                .structural_values
                .root_for_captured_copy(self.state, self.statement_index as u32, identity)?
                .root;
            let source = self.canonical_place(expression)?;
            let prefix = self.canonical_place(copy.expression)?;
            let place = validation::CanonicalPlace {
                root: facts::PlaceRoot::Expression(copy.expression),
                segments: source.segments[prefix.segments.len()..].to_vec(),
            };
            let receiver = validation::DispatchExtraction::payload_binding_receiver(
                self.program,
                expression,
                copy.expression,
            )?;
            let selectors = crate::values::authored_projection_selectors_below(
                self.program,
                expression,
                receiver,
            )?;
            let selectors = self.retain_projection_selector_evaluations(expression, selectors)?;
            let ProjectedArgumentPath {
                storage_type: projected,
                mut path,
                ..
            } = self.projected_place_path(&place, &selectors)?;
            if !validation::declared_carriers_match(self.program, projected, expected) {
                return None;
            }
            if !place.segments.is_empty() {
                let mut reference = copy.type_reference;
                let mut crossings = Vec::new();
                while let Some(shell) = validation::reference_shell(self.program, reference) {
                    if !shell.access.is_readable() {
                        return None;
                    }
                    if !validation::stores_reference_shape(self.program, reference, &[]) {
                        break;
                    }
                    crossings.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
                    reference = shell.referee;
                }
                crossings.append(&mut path);
                path = crossings;
            }
            for (ordinal, value) in selectors.iter().map(|(_, value)| *value).enumerate() {
                self.push_scalar_root(
                    checked_trees::CheckedScalarExpressionRole::StructuralValueSelector {
                        expression,
                        selector_ordinal: u32::try_from(ordinal).ok()?,
                    },
                    value,
                )?;
            }
            let projection_evaluations = self.projection_selector_span(expression, &selectors)?;
            if !checked_trees::CheckedUnitStructuralArgumentPlan::projection_evaluations_match(
                &path,
                projection_evaluations,
                &self.plans.operands,
            ) {
                return None;
            }
            Some(ValueKind::Projection {
                source: saved,
                logical_root_type_identity: Default::default(),
                path,
                projection_evaluations,
                copies_selected_value: true,
                type_identity: validation::stored_structural_type_identity(
                    self.program,
                    projected,
                    &[],
                    &[],
                ),
            })
        }
    }

    /*
        The flow receipt, not equal canonical storage, selects the completed value.
        Pure-root publication and expanded-root construction share this exact query.
    */
    fn captured_dispatch_operand<'flow>(
        program: &dyn validation::CheckingInput,
        flow: &'flow checked_trees::FlowFacts,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<&'flow checked_trees::FlowOperandFact> {
        let typed_state = program.state_by_symbol(state)?;
        let transition = program
            .statement_table
            .statement_at(typed_state, statement_index)?
            .as_transition()?;
        let (_, copy) = crate::flow::copied_payload_at(
            program,
            flow,
            machine,
            state,
            statement_index,
            checked_trees::FlowPermissionEvaluation::TransitionTarget(transition.target),
            expression,
        )?;
        Some(copy)
    }

    fn uses_captured_dispatch_value(
        program: &dyn validation::CheckingInput,
        copy: Option<&checked_trees::FlowOperandFact>,
    ) -> bool {
        copy.is_some_and(|copy| {
            program
                .primitive_type_reference(copy.type_reference)
                .is_none()
                || validation::reference_shell(program, copy.type_reference).is_some()
        })
    }

    pub(super) fn observes_captured_dispatch_value(
        program: &dyn validation::CheckingInput,
        flow: &checked_trees::FlowFacts,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        let mut pending = vec![expression];
        let mut visited = Vec::new();
        while let Some(expression) = pending.pop() {
            if visited.contains(&expression) {
                continue;
            }
            visited.push(expression);
            if uses_captured_dispatch_value(
                program,
                captured_dispatch_operand(
                    program,
                    flow,
                    machine,
                    state,
                    statement_index,
                    expression,
                ),
            ) {
                return true;
            }
            if program.expression_table.expression_is_valid(expression) {
                validation::expression_traversal::children(
                    program,
                    program.expression_table.expression(expression),
                    |child| pending.push(child),
                );
            }
        }
        false
    }

    /*
        Source flow already captured its operands at their real read epochs. At the
        execution-plan boundary, an actual completed-value dependency belongs solely
        to the expanded producer. Withdraw the exact pure root even if construction
        of its replacement later fails: the old live read is never a fallback.
        Conflicting custody rows remain published so ordinary ambiguity checks refuse.
    */
    pub(super) fn withdraw_captured_pure_roots(
        program: &dyn validation::CheckingInput,
        flow: &checked_trees::FlowFacts,
        pure: &mut checked_trees::CheckedScalarExpressionPlans,
    ) {
        let mut withdrawn = Vec::new();
        for (_, binding) in pure.source_bindings.iter() {
            let Some((retained, _)) =
                pure.bound_expression_at(binding.state, binding.statement_ordinal, binding.role)
            else {
                continue;
            };
            if retained != binding {
                continue;
            }
            let Some((machine, _)) = validation::find_state(program, binding.state) else {
                continue;
            };
            if observes_captured_dispatch_value(
                program,
                flow,
                machine.symbol,
                binding.state,
                binding.statement_ordinal as usize,
                binding.expression,
            ) {
                withdrawn.push((binding.state, binding.statement_ordinal, binding.role));
            }
        }
        if withdrawn.is_empty() {
            return;
        }
        pure.expressions
            .retain(|root| !withdrawn.contains(&(root.state, root.statement_ordinal, root.role)));
        let mut bindings = arena::Arena::default();
        for (_, binding) in pure.source_bindings.iter() {
            if !withdrawn.contains(&(binding.state, binding.statement_ordinal, binding.role)) {
                bindings.append(binding.clone());
            }
        }
        pure.source_bindings = bindings;
        pure.state_index = Default::default();
        pure.seal_expressions();
    }
}
mod dynamic_calls {
    /*
        Selector calls (`Trait::member(selector, data...)`) as ordered operands.

        Checking retargets a selector call to its instantiated slot, whose roster
        is the selector followed by the requirement's instantiated formals. The
        selector evaluates first, exactly where it was authored, and becomes the
        callee's dispatch operand rather than one of the data operands; the data
        operands then complete left to right through the ordinary call operand
        evaluator against the slot's formals. Every selectable implementation is
        a checked body, so the data operands keep checked-body custody.

        The requirement's contract needs no evidence on the route: checking
        discharged its `requires` against these operands and every selectable
        implementation refines it where its conformance is checked. Suspension
        retains the same call occurrence and its independently checked frontier.
    */
    use super::call_arguments::{CallOperandCustody, ComputedCallOperands};
    use super::{
        Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
        PrimitiveType,
    };

    impl Builder<'_, '_> {
        pub(super) fn dynamic_call(
            &mut self,
            expression: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            expected_type: PrimitiveType,
        ) -> Option<CheckedScalarComputationHandle> {
            let program = self.program;
            let slot = program.dynamic_slot_by_symbol(call.target_symbol)?;
            if !call.evidence_arguments.is_empty()
                || !call.selects_only_nominal_route()
                || program.primitive_type_reference(slot.return_type)? != expected_type
            {
                return None;
            }
            let requirement = program.dynamic_slot_requirement(slot)?;
            let (source_call, call_ordinal) = self.call_ordinal(expression, call.target_symbol)?;
            let arity = match program.finite_signature_family(requirement) {
                typed_trees::finite_family::FamilyProbe::Finite { arity, .. } => arity,
                _ if call.machine_arguments.is_empty() => 0,
                _ => return None,
            };
            if call.machine_arguments.len() != arity {
                return None;
            }
            for family_ordinal in 0..arity {
                self.pure.bound_expression_at(
                    self.state,
                    u32::try_from(self.statement_index).ok()?,
                    checked_trees::CheckedScalarExpressionRole::DynamicFamilyArgument {
                        call_ordinal,
                        family_ordinal: u32::try_from(family_ordinal).ok()?,
                    },
                )?;
            }
            let arguments = program.expression_table.expression_handles(call.arguments);
            let (&selector_argument, data_arguments) = arguments.split_first()?;
            let captures = self.flow.control.calls.get(source_call).argument_operands();
            if captures.len() != arguments.len() {
                return None;
            }
            let data_captures = arena::HandleSpan::from_parts_or_empty(
                captures.handle_at(1),
                captures.count().checked_sub(1)?,
            );
            let selector = self.expression(selector_argument, PrimitiveType::Selector)?;
            self.plans.nodes.get_mut(selector).authored_root = selector_argument;
            let ComputedCallOperands {
                arguments,
                structural_arguments,
                temporary_cleanups,
                claim_transfers,
            } = self.call_operands(
                CallOperandCustody::Statement {
                    call: source_call,
                    captures: data_captures,
                },
                data_arguments,
                slot.data_parameters(),
            )?;
            Some(self.insert(
                expected_type,
                CheckedScalarComputationKind::Call {
                    source_expression: expression,
                    source_call: source_call.into(),
                    callee: checked_trees::CheckedScalarCallee::Dynamic {
                        selector,
                        slot: slot.symbol,
                        requirement: slot.requirement,
                        interface_identity: slot.interface_identity,
                    },
                    call_ordinal,
                    arguments,
                    structural_arguments,
                    temporary_cleanups,
                    claim_transfers,
                },
            ))
        }
    }
}
mod enum_equality {

    /*
        Intrinsic enum equality completes each operand once, left before right.

        Payload-free sums compare their complete case roster with ordinary Boolean
        operations. The general path owns one explicit mixed evaluation prefix:
        copyable operands complete through the ordinary value producer, while a
        non-copy place contributes scalar tag observations without copying its owner.
        Readable subjects already contain only places and captured immutable selectors.
        The generated comparison then reads those snapshots without replaying calls,
        observing a replaced source, or granting case membership to either payload.

        One Apply per case keeps the comparison's running conjunction short-lived.
        Copyable enums retain two value homes rather than every tag Boolean at once;
        non-copy carriers retain their tag snapshots through the comparison. A known
        payload-free constructor remains the existing single-membership optimization.
        Only the whole comparison owns its source normal-completion occurrence.
    */
    use super::{
        Builder, CheckedBooleanExpression, CheckedScalarComputationHandle,
        CheckedScalarComputationKind, CheckedScalarExpression, ExpressionHandle, PrimitiveType,
        operator_is_builtin,
    };
    use crate::values::scalar::structural_fields::structural_data;
    use checked_trees::CheckedScalarComputationStructuralArgument;
    use typed_trees::data::{DataDefinition, DataShapeKind};
    use typed_trees::expression::BinaryOperator;

    impl Builder<'_, '_> {
        pub(super) fn enum_equality(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<CheckedScalarComputationHandle> {
            let binary = self.program.expression_table.binary_node(expression)?;
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) || !operator_is_builtin(self.operators, expression)
            {
                return None;
            }
            let unequal = binary.operator == BinaryOperator::NotEqual;
            let (left_type, mut left, mut left_receiver) = self.enum_operand(binary.left)?;
            let (right_type, mut right, mut right_receiver) = self.enum_operand(binary.right)?;
            let left_owner = structural_data(self.program, left_type)?;
            let right_owner = structural_data(self.program, right_type)?;
            if left_owner.symbol != right_owner.symbol
                || self.program.normalized_type_identity(left_type)
                    != self.program.normalized_type_identity(right_type)
            {
                return None;
            }
            let members = self.program.data_members(left_owner);
            if DataDefinition::shape_kind_from_members(members) != DataShapeKind::Enum {
                return None;
            }
            let cases = members
                .iter()
                .map(|member| {
                    let variant = member.as_variant()?;
                    self.program
                        .data_payload_fields(variant)
                        .is_empty()
                        .then_some(variant.symbol)
                })
                .collect::<Option<Vec<_>>>()?;
            if cases.is_empty() {
                return None;
            }
            let known_case = match (&left, &right) {
                (subject, CheckedScalarComputationStructuralArgument::Case(constructor))
                    if !matches!(subject, CheckedScalarComputationStructuralArgument::Case(_)) =>
                {
                    Some((subject.clone(), left_receiver, constructor.case))
                }
                (CheckedScalarComputationStructuralArgument::Case(constructor), subject)
                    if !matches!(subject, CheckedScalarComputationStructuralArgument::Case(_)) =>
                {
                    Some((subject.clone(), right_receiver, constructor.case))
                }
                _ => None,
            };
            if let Some((subject, receiver_value, case)) = known_case {
                let receiver_value = match &subject {
                    CheckedScalarComputationStructuralArgument::Place(place)
                        if !receiver_value.is_valid() =>
                    {
                        self.case_subject_receiver(place)?
                    }
                    _ => receiver_value,
                };
                let membership = self.publish_case_membership(
                    expression,
                    if unequal {
                        ExpressionHandle::invalid()
                    } else {
                        expression
                    },
                    receiver_value,
                    subject,
                    case,
                );
                if !unequal {
                    return Some(membership);
                }
                return Some(self.boolean_negation(membership, expression));
            }
            let mut evaluation = Vec::new();
            self.capture_enum_operand(binary.left, left_type, &mut left, &mut left_receiver)?;
            self.capture_enum_operand(binary.right, right_type, &mut right, &mut right_receiver)?;
            let generated = ExpressionHandle::invalid();
            let mut memberships = Vec::new();
            for (subject, receiver) in [(&left, left_receiver), (&right, right_receiver)] {
                if receiver.is_valid() {
                    evaluation.push(checked_trees::CheckedScalarEvaluationOperand::Structural(
                        receiver,
                    ));
                }
                let snapshot_tags = !receiver.is_valid()
                    && matches!(
                        subject,
                        CheckedScalarComputationStructuralArgument::Place(_)
                    );
                let mut observed = Vec::new();
                for case in &cases {
                    let membership = self.membership_of(subject, receiver, *case, generated);
                    if snapshot_tags {
                        evaluation.push(checked_trees::CheckedScalarEvaluationOperand::Scalar(
                            membership,
                        ));
                    }
                    observed.push(membership);
                }
                memberships.push(observed);
            }
            let mut chain = None;
            for (ordinal, _) in cases.iter().enumerate() {
                let left_membership = memberships[0][ordinal];
                let right_membership = memberships[1][ordinal];
                let same = |left, right| CheckedBooleanExpression::Equal {
                    left: Box::new(CheckedBooleanExpression::Parameter { position: left }),
                    right: Box::new(CheckedBooleanExpression::Parameter { position: right }),
                };
                let (template, operands) = match chain {
                    None => (same(0, 1), vec![left_membership, right_membership]),
                    Some(previous) => (
                        CheckedBooleanExpression::And {
                            left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
                            right: Box::new(same(1, 2)),
                        },
                        vec![previous, left_membership, right_membership],
                    ),
                };
                let operands = self.plans.operands.insert_many(operands);
                chain = Some(self.insert(
                    PrimitiveType::Bool,
                    CheckedScalarComputationKind::Apply {
                        source_expression: generated,
                        expression: CheckedScalarExpression::Boolean(Box::new(template)),
                        operands,
                    },
                ));
            }
            let equal = chain?;
            let operand = if unequal {
                self.boolean_negation(equal, generated)
            } else {
                equal
            };
            let operands = self.plans.evaluation_operands.insert_many(evaluation);
            Some(self.insert(
                PrimitiveType::Bool,
                CheckedScalarComputationKind::WithOperands {
                    source_expression: expression,
                    operands,
                    operand,
                },
            ))
        }

        /*
            Ordinary readable copies use the same value producer as constructor
            children and call operands. Retaining the actual leaf once is essential
            even when its root was already resident before this comparison.
        */
        fn capture_enum_operand(
            &mut self,
            expression: ExpressionHandle,
            reference: typed_trees::types::TypeReferenceHandle,
            subject: &mut CheckedScalarComputationStructuralArgument,
            receiver: &mut checked_trees::CheckedStructuralValueHandle,
        ) -> Option<()> {
            if receiver.is_valid()
                || matches!(subject, CheckedScalarComputationStructuralArgument::Case(_))
            {
                return Some(());
            }
            if self.program.type_multiplicity(reference)
                != language_semantics::Multiplicity::Unrestricted
            {
                return Some(());
            }
            let value = self.retain_copied_place_value(expression, reference)?;
            *receiver = value;
            *subject = CheckedScalarComputationStructuralArgument::Place(
            checked_trees::CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source:
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        value_dependency: Some(value),
                        expression,
                        logical_root_type_identity: None,
                    },
                path: Vec::new(),
                type_identity: validation::stored_structural_type_identity(
                    self.program,
                    reference,
                    &[],
                    &[],
                ),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
        );
            Some(())
        }

        /*
            One operand's membership in `case`: a constant for a payloadless
            constructor, a tag observation for a place.
        */
        fn membership_of(
            &mut self,
            subject: &CheckedScalarComputationStructuralArgument,
            receiver_value: checked_trees::CheckedStructuralValueHandle,
            case: symbols::SymbolHandle,
            expression: ExpressionHandle,
        ) -> CheckedScalarComputationHandle {
            match subject {
                CheckedScalarComputationStructuralArgument::Case(constructor) => {
                    self.boolean(constructor.case == case)
                }
                _ => self.publish_case_membership(
                    expression,
                    expression,
                    receiver_value,
                    subject.clone(),
                    case,
                ),
            }
        }

        /*
            A completed value supplies the tag just as an established place does.
            Selection and evaluation remain on the ordinary structural-value graph;
            observing its tag grants neither payload membership nor ownership.
        */
        pub(super) fn computed_case_membership(
            &mut self,
            source_expression: ExpressionHandle,
            expression: ExpressionHandle,
            case: symbols::SymbolHandle,
        ) -> Option<CheckedScalarComputationHandle> {
            let (returned, subject, receiver_value) = self.completed_case_subject(expression)?;
            let owner = structural_data(self.program, returned)?;
            if !self.program.data_members(owner).iter().any(|member| {
                member
                    .as_variant()
                    .is_some_and(|variant| variant.symbol == case)
            }) {
                return None;
            }
            Some(self.publish_case_membership(
                source_expression,
                source_expression,
                receiver_value,
                subject,
                case,
            ))
        }

        fn completed_case_subject(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<(
            typed_trees::types::TypeReferenceHandle,
            CheckedScalarComputationStructuralArgument,
            checked_trees::CheckedStructuralValueHandle,
        )> {
            let reference = self.expression_type(expression)?;
            let receiver = self.retain_structural_argument(expression, reference)?;
            Some((reference, CheckedScalarComputationStructuralArgument::Place(
            checked_trees::CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { value_dependency: Some(receiver), expression, logical_root_type_identity: None },
                path: Vec::new(),
                type_identity: validation::stored_structural_type_identity(self.program, reference, &[], &[]),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            }
        ), receiver))
        }

        fn enum_operand(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<(
            typed_trees::types::TypeReferenceHandle,
            CheckedScalarComputationStructuralArgument,
            checked_trees::CheckedStructuralValueHandle,
        )> {
            if let Some((reference, place)) =
                self.readable_case_subject(expression, self.statement_index)
            {
                return Some((
                    reference,
                    CheckedScalarComputationStructuralArgument::Place(place),
                    checked_trees::CheckedStructuralValueHandle::invalid(),
                ));
            }
            if let Some(constructor) = self.case_construction(expression)
                && constructor.fields.is_empty()
            {
                return Some((
                    constructor.type_reference,
                    CheckedScalarComputationStructuralArgument::Case(constructor),
                    checked_trees::CheckedStructuralValueHandle::invalid(),
                ));
            }
            self.completed_case_subject(expression)
        }
    }
}
mod indexed_reads {
    /*
        Scalar and sequence-length observations share declaration geometry with pure reads.
        Each selector is an ordinary computation operand of a storage observation.
        Its collection carrier is retained beside it so lowering can capture that
        collection before running the selector, including at nested boundaries.
        The indexing occurrence still owns its source-checked bound; an operand
        result supplies only bits, never a substitute geometry-only evaluation.
    */
    use super::{
        ArithmeticDomain, Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind,
        CheckedScalarExpression, ExpressionHandle, PrimitiveType,
    };
    use crate::flow::expressions;
    use crate::values::scalar::expression_facts::{is_integer, scalar_expression_type};

    impl Builder<'_, '_> {
        pub(super) fn indexed_read(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<(
            CheckedScalarComputationHandle,
            ArithmeticDomain,
            PrimitiveType,
        )> {
            let program = self.program;
            let operators = self.operators;
            let parameters = self.authored_parameters;
            let mut operands = Vec::new();
            let mut collections = Vec::new();
            let mut selector = |_, selector, collection| {
                let operand = self.integer_operand(selector, Some(PrimitiveType::U64))?;
                let primitive_type = scalar_expression_type(&operand.value)?;
                if !is_integer(primitive_type) || primitive_type == PrimitiveType::Addr {
                    return None;
                }
                let position = operands.len();
                operands.push(self.materialize_integer(operand)?);
                collections.push(collection);
                Some(CheckedScalarExpression::Parameter {
                    position,
                    primitive_type,
                })
            };
            let (template, domain) = if expressions(program)
                .authored_length_receiver(expression)
                .is_some()
            {
                (
                    crate::values::scalar::structural_fields::structural_sequence_length(
                        program,
                        operators,
                        parameters,
                        expression,
                        &mut selector,
                    )?,
                    ArithmeticDomain::Exact,
                )
            } else {
                crate::values::scalar::scalar_lowering::scalar_storage_read_with_selectors(
                    program, operators, expression, parameters, selector,
                )?
            };
            let root = match &template {
                CheckedScalarExpression::StructuralParameterIndexedRead { root, .. }
                | CheckedScalarExpression::StructuralSequenceLength { root, .. } => Some(*root),
                _ => None,
            };
            let root_type = match root {
                Some(checked_trees::CheckedStorageRoot::Parameter { index }) => {
                    self.authored_parameters.get(index as usize)?.type_reference
                }
                Some(
                    checked_trees::CheckedStorageRoot::OwnedLocal { symbol }
                    | checked_trees::CheckedStorageRoot::ViewLocal { symbol },
                ) => {
                    let positions = program.local_data_positions_for_symbol(symbol);
                    let [position] = positions.as_slice() else {
                        return None;
                    };
                    let machine = &program.machines()[position.0 as usize];
                    let state = &program.machine_states(machine)[position.1 as usize];
                    crate::flow::state_statements(program, state.statement_nodes)
                        .get(position.2 as usize)?
                        .as_local_data()?
                        .type_reference
                }
                None => typed_trees::types::TypeReferenceHandle::invalid(),
            };
            let primitive_type = scalar_expression_type(&template)?;
            let operands = self.plans.operands.insert_many(operands);
            let collections = self.plans.observation_collections.insert_many(collections);
            let computation = self.insert(
                primitive_type,
                CheckedScalarComputationKind::StorageObservation {
                    root_type,
                    source_expression: expression,
                    expression: template,
                    operands,
                    collections,
                },
            );
            Some((computation, domain, primitive_type))
        }
    }
}
mod integers {
    /*
        Integer policy stays attached to each operand until its operation is selected.
    */
    use super::{
        ArithmeticDomain, BinaryOperator, Builder, CheckedBooleanExpression,
        CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarExpression,
        ExpressionHandle, ExpressionNode, PrimitiveType, UnaryOperator,
    };
    use crate::values::operator_is_builtin;
    use crate::values::scalar::boolean_lowering::construct_integer_comparison;
    use crate::values::scalar::expression_facts::combine_arithmetic_domains;
    use crate::values::scalar::expression_facts::is_integer;
    use crate::values::scalar::scalar_lowering::construct_integer_binary;
    use crate::values::scalar::scalar_lowering::construct_integer_bitwise_not;
    use crate::values::scalar::scalar_lowering::construct_integer_cast;
    use crate::values::scalar::scalar_lowering::land_anonymous_scalar_expression;
    use crate::values::scalar::scalar_lowering::lower_scalar_expression;
    use crate::values::scalar::semantic_casts;
    use crate::values::scalar_expression_type;
    use typed_trees::expression::MatchPattern;
    use typed_trees::expression::TableBinaryExpression;
    use typed_trees::operator::integer_peer_operand_carrier;
    use validation::atomic_load_carrier;
    use validation::call_target_return_type;
    use validation::call_target_symbol;
    use validation::declared_place_type_raw;

    #[derive(Clone)]
    pub(super) struct IntegerOperand {
        pub(super) value: CheckedScalarExpression,
        value_source: ExpressionHandle,
        domain: ArithmeticDomain,
        computation: CheckedScalarComputationHandle,
    }

    impl Builder<'_, '_> {
        pub(super) fn materialize_integer(
            &mut self,
            operand: IntegerOperand,
        ) -> Option<CheckedScalarComputationHandle> {
            if operand.computation.is_valid() {
                Some(operand.computation)
            } else {
                let computation = self.insert(
                    scalar_expression_type(&operand.value)?,
                    CheckedScalarComputationKind::Value(operand.value),
                );
                self.plans.nodes.get_mut(computation).value_source = operand.value_source;
                Some(computation)
            }
        }

        fn integer_application(
            &mut self,
            source_expression: ExpressionHandle,
            expression: CheckedScalarExpression,
            domain: ArithmeticDomain,
            operands: impl IntoIterator<Item = CheckedScalarComputationHandle>,
        ) -> Option<IntegerOperand> {
            let primitive_type = scalar_expression_type(&expression)?;
            let operands = self.plans.operands.insert_many(operands);
            let computation = self.insert(
                primitive_type,
                CheckedScalarComputationKind::Apply {
                    source_expression,
                    expression,
                    operands,
                },
            );
            Some(IntegerOperand {
                value: parameter(0, primitive_type),
                value_source: ExpressionHandle::invalid(),
                domain,
                computation,
            })
        }

        /*
            `expected` is the carrier the caller will store this operand into, when
            it knows one. It is evidence of last resort: every already-checked
            operand keeps its own carrier, and the hint is consulted only where the
            expression itself carries none anywhere -- see `integer_operands`.
        */
        pub(super) fn integer_operand(
            &mut self,
            expression: ExpressionHandle,
            expected: Option<PrimitiveType>,
        ) -> Option<IntegerOperand> {
            if let Some((_, _, operand)) = self
                .integer_occurrences
                .iter()
                .find(|(source, carrier, _)| *source == expression && *carrier == expected)
            {
                return Some(operand.clone());
            }
            let operand = self.build_integer_operand(expression, expected)?;
            self.integer_occurrences
                .push((expression, expected, operand.clone()));
            Some(operand)
        }

        fn build_integer_operand(
            &mut self,
            expression: ExpressionHandle,
            expected: Option<PrimitiveType>,
        ) -> Option<IntegerOperand> {
            if let Some((computation, reference)) = self.declaration_input(expression).ok()? {
                let primitive = self.program.primitive_type_reference(reference)?;
                if !is_integer(primitive) || expected.is_some_and(|expected| expected != primitive)
                {
                    return None;
                }
                return Some(IntegerOperand {
                    value: parameter(0, primitive),
                    value_source: ExpressionHandle::invalid(),
                    domain: self.program.arithmetic_domain_for_type_reference(reference),
                    computation,
                });
            }
            if let Some((computation, primitive_type)) = self.retained_constant(expression) {
                return Some(IntegerOperand {
                    value: parameter(0, primitive_type),
                    value_source: ExpressionHandle::invalid(),
                    domain: ArithmeticDomain::Exact,
                    computation,
                });
            }

            if let Some(computation) = self.storage_address(expression) {
                return Some(IntegerOperand {
                    value: parameter(0, PrimitiveType::Addr),
                    value_source: ExpressionHandle::invalid(),
                    domain: ArithmeticDomain::Exact,
                    computation,
                });
            }

            if self.uses_captured_dispatch_value(expression) {
                let reference = self.expression_type(expression)?;
                let primitive = self.program.primitive_type_reference(reference)?;
                if !is_integer(primitive) || expected.is_some_and(|expected| expected != primitive)
                {
                    return None;
                }
                return Some(IntegerOperand {
                    value: parameter(0, primitive),
                    value_source: ExpressionHandle::invalid(),
                    domain: self.program.arithmetic_domain_for_type_reference(reference),
                    computation: self.completed_scalar_observation(expression, primitive)?,
                });
            }
            if let Some(observation) = self.primitive_reference_observation_type(expression) {
                if !is_integer(observation.primitive_type) {
                    return None;
                }
                return Some(IntegerOperand {
                    value: parameter(0, observation.primitive_type),
                    value_source: ExpressionHandle::invalid(),
                    domain: observation.domain,
                    computation: self
                        .primitive_reference_observation(expression, observation.primitive_type)?,
                });
            }
            if let Some(carrier) = atomic_load_carrier(self.program, expression) {
                let declared = declared_place_type_raw(
                    self.program,
                    self.program.machine_by_symbol(self.machine)?,
                    Some(self.program.state_by_symbol(self.state)?),
                    carrier.place,
                )?;
                let primitive = self.program.primitive_type_reference(declared)?;
                if !is_integer(primitive) || expected.is_some_and(|expected| expected != primitive)
                {
                    return None;
                }
                return Some(IntegerOperand {
                    value: parameter(0, primitive),
                    value_source: ExpressionHandle::invalid(),
                    domain: ArithmeticDomain::Exact,
                    computation: self.atomic_load(expression, primitive)?,
                });
            }
            if let Some(field) = self.structural_scalar_field(expression) {
                if !is_integer(field.primitive_type) {
                    return None;
                }
                return Some(IntegerOperand {
                    value: parameter(0, field.primitive_type),
                    value_source: ExpressionHandle::invalid(),
                    domain: field.domain,
                    computation: self.structural_field(expression, field)?,
                });
            }
            if self.same_carrier_cast_operand(expression).is_some()
                && let ExpressionNode::Cast(cast) =
                    self.program.expression_table.expression(expression)
                && let Some(primitive_type) =
                    self.program.primitive_type_reference(cast.target_type)
                && is_integer(primitive_type)
            {
                let domain = cast.domain;
                let computation = self.same_carrier_cast_value(expression, primitive_type)?;
                return Some(IntegerOperand {
                    value: parameter(0, primitive_type),
                    value_source: ExpressionHandle::invalid(),
                    domain,
                    computation,
                });
            }
            if let Some(cast) = self.program.expression_table.cast_node(expression)
                && (!cast.semantic_domain.is_empty()
                    || semantic_casts::result_type(self.program, self.state, cast.value)
                        .is_some_and(|reference| {
                            semantic_casts::has_declared_domains(self.program, reference)
                        }))
            {
                let result_type =
                    semantic_casts::result_type(self.program, self.state, expression)?;
                let primitive_type = self.program.primitive_type_reference(result_type)?;
                if !is_integer(primitive_type) {
                    return None;
                }
                let computation = self.expression(expression, primitive_type)?;
                return Some(IntegerOperand {
                    value: parameter(0, primitive_type),
                    value_source: ExpressionHandle::invalid(),
                    domain: ArithmeticDomain::Exact,
                    computation,
                });
            }
            if !self.has_declaration_binding(expression)
                && !self.observes_captured_dispatch_value(expression)
                && !semantic_casts::requires_custody(self.program, self.state, expression)
                && let Some((value, domain)) = lower_scalar_expression(
                    self.program,
                    self.operators,
                    expression,
                    self.parameters,
                    self.authored_parameters,
                    self.parameter_types,
                    self.locals,
                    self.exact_integer_casts,
                )
            {
                return Some(IntegerOperand {
                    value,
                    value_source: expression,
                    domain,
                    computation: CheckedScalarComputationHandle::invalid(),
                });
            }
            if let Some((computation, domain, primitive_type)) = self.indexed_read(expression) {
                return is_integer(primitive_type).then_some(IntegerOperand {
                    value: parameter(0, primitive_type),
                    value_source: ExpressionHandle::invalid(),
                    domain,
                    computation,
                });
            }
            match self.program.expression_table.expression(expression).clone() {
                ExpressionNode::Match(dispatch) => {
                    let mut result_type = None;
                    let mut domain = ArithmeticDomain::Exact;
                    let mut boolean_coverage = [false; 2];
                    let mut contextual_results = Vec::new();
                    for arm in self.program.expression_table.match_arms(dispatch.arms) {
                        if let Some(operand) = self.integer_operand(arm.value, expected) {
                            domain = combine_arithmetic_domains(domain, operand.domain)?;
                            if let Some(primitive_type) = scalar_expression_type(&operand.value) {
                                if !is_integer(primitive_type)
                                    || result_type
                                        .is_some_and(|existing| existing != primitive_type)
                                {
                                    return None;
                                }
                                result_type = Some(primitive_type);
                            }
                        } else {
                            contextual_results.push(arm.value);
                        }
                        match arm.pattern {
                            MatchPattern::Destructure(_) => {}
                            MatchPattern::Wildcard => break,
                            MatchPattern::Value(pattern) => {
                                if let Some(value) =
                                    self.program.expression_table.boolean_node(pattern)
                                {
                                    boolean_coverage[usize::from(*value)] = true;
                                    if boolean_coverage.iter().all(|value| *value) {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    let primitive_type = result_type?;
                    if !contextual_results
                        .iter()
                        .all(|result| self.anonymous_result_lands(*result, primitive_type))
                    {
                        return None;
                    }
                    let computation = self.dispatch(expression, &dispatch, primitive_type)?;
                    Some(IntegerOperand {
                        value: parameter(0, primitive_type),
                        value_source: ExpressionHandle::invalid(),
                        domain,
                        computation,
                    })
                }
                ExpressionNode::Call(call) => {
                    let returned = call_target_return_type(
                        self.program,
                        call_target_symbol(self.program, &call),
                    )?;
                    let primitive_type = self.program.primitive_type_reference(returned)?;
                    if !is_integer(primitive_type) {
                        return None;
                    }
                    let domain = self.program.arithmetic_domain_for_type_reference(returned);
                    let computation = self.expression(expression, primitive_type)?;
                    Some(IntegerOperand {
                        value: parameter(0, primitive_type),
                        value_source: ExpressionHandle::invalid(),
                        domain,
                        computation,
                    })
                }
                ExpressionNode::Binary(binary)
                    if operator_is_builtin(self.operators, expression) =>
                {
                    let (mut left, mut right) = self.integer_operands(&binary, expected)?;
                    let (value, domain) = construct_integer_binary(
                        binary.operator,
                        left.value,
                        left.domain,
                        right.value,
                        right.domain,
                        self.operators.formation_is_decided(expression),
                        self.operators.is_trap_free(expression),
                    )?;
                    let CheckedScalarExpression::IntegerBinary {
                        kind,
                        primitive_type,
                        left: landed_left,
                        right: landed_right,
                        formation_decided,
                        trap_free,
                    } = value
                    else {
                        return None;
                    };
                    left.value = *landed_left;
                    right.value = *landed_right;
                    let left_type = scalar_expression_type(&left.value)?;
                    let right_type = scalar_expression_type(&right.value)?;
                    let left = self.materialize_integer(left)?;
                    let right = self.materialize_integer(right)?;
                    self.integer_application(
                        expression,
                        CheckedScalarExpression::IntegerBinary {
                            kind,
                            primitive_type,
                            left: Box::new(parameter(0, left_type)),
                            right: Box::new(parameter(1, right_type)),
                            formation_decided,
                            trap_free,
                        },
                        domain,
                        [left, right],
                    )
                }
                ExpressionNode::Unary(unary)
                    if unary.operator == UnaryOperator::BitwiseNot
                        && operator_is_builtin(self.operators, expression) =>
                {
                    let operand = self.integer_operand(unary.operand, expected)?;
                    let primitive_type = scalar_expression_type(&operand.value)?;
                    let (value, domain) = construct_integer_bitwise_not(
                        parameter(0, primitive_type),
                        operand.domain,
                    )?;
                    let operand = self.materialize_integer(operand)?;
                    self.integer_application(expression, value, domain, [operand])
                }
                ExpressionNode::Cast(cast) => {
                    if let Some(source) =
                        semantic_casts::result_type(self.program, self.state, cast.value)
                        && self.program.primitive_type_reference(source)
                            == Some(PrimitiveType::Bool)
                        && self
                            .program
                            .primitive_type_reference(cast.target_type)
                            .is_some_and(|destination| {
                                is_integer(destination) && destination != PrimitiveType::Addr
                            })
                        && !cast.form.is_recast()
                        && cast.semantic_domain.is_empty()
                    {
                        let destination =
                            self.program.primitive_type_reference(cast.target_type)?;
                        let operand = self.expression(cast.value, PrimitiveType::Bool)?;
                        let computation = self.insert(
                            destination,
                            CheckedScalarComputationKind::BooleanToInteger {
                                source_expression: expression,
                                operand,
                            },
                        );
                        return Some(IntegerOperand {
                            value: parameter(0, destination),
                            value_source: ExpressionHandle::invalid(),
                            domain: ArithmeticDomain::Exact,
                            computation,
                        });
                    }
                    let operand = self.integer_operand(cast.value, None).or_else(|| {
                        let destination =
                            self.program.primitive_type_reference(cast.target_type)?;
                        self.contextual_match_operand(cast.value, destination)
                    })?;
                    let (value, domain) = construct_integer_cast(
                        self.program,
                        expression,
                        operand.value.clone(),
                        self.exact_integer_casts,
                    )?;
                    if matches!(
                        value,
                        CheckedScalarExpression::IntegerWiden { .. }
                            | CheckedScalarExpression::IntegerExactCast { .. }
                    ) {
                        let source_type = scalar_expression_type(&operand.value)?;
                        let (template, _) = construct_integer_cast(
                            self.program,
                            expression,
                            parameter(0, source_type),
                            self.exact_integer_casts,
                        )?;
                        let operand = self.materialize_integer(operand)?;
                        self.integer_application(expression, template, domain, [operand])
                    } else {
                        Some(IntegerOperand {
                            value,
                            value_source: expression,
                            domain,
                            ..operand
                        })
                    }
                }
                _ => None,
            }
        }

        fn integer_operands(
            &mut self,
            binary: &TableBinaryExpression,
            expected: Option<PrimitiveType>,
        ) -> Option<(IntegerOperand, IntegerOperand)> {
            let mut left = self.integer_operand(binary.left, expected);
            let mut right = self.integer_operand(binary.right, expected);
            let land = |expression, destination| {
                land_anonymous_scalar_expression(
                    self.program,
                    self.operators,
                    expression,
                    destination,
                )
                .map(|value| IntegerOperand {
                    value,
                    value_source: expression,
                    domain: ArithmeticDomain::Exact,
                    computation: CheckedScalarComputationHandle::invalid(),
                })
            };
            let operand_type = |operand: &Option<IntegerOperand>| {
                operand
                    .as_ref()
                    .and_then(|operand| scalar_expression_type(&operand.value))
            };
            if let Some(destination) = operand_type(&right)
                && let Some(operand) = land(
                    binary.left,
                    integer_peer_operand_carrier(binary.operator, destination, false),
                )
            {
                left = Some(operand);
            }
            if let Some(destination) = operand_type(&left)
                && let Some(operand) = land(
                    binary.right,
                    integer_peer_operand_carrier(binary.operator, destination, true),
                )
            {
                right = Some(operand);
            }
            if left.is_none()
                && let Some(destination) = operand_type(&right)
            {
                left = self.contextual_match_operand(binary.left, destination);
            }
            if right.is_none()
                && let Some(destination) = operand_type(&left)
            {
                right = self.contextual_match_operand(binary.right, destination);
            }
            if operand_type(&left).is_none()
                && operand_type(&right).is_none()
                && let Some(destination) = expected
            {
                left = self.anonymous_operand(binary.left, destination);
                right = self.anonymous_operand(
                    binary.right,
                    integer_peer_operand_carrier(binary.operator, destination, true),
                );
            }
            Some((left?, right?))
        }

        /*
            An operand with no carrier of its own, landed at `destination`: an
            anonymous scalar tree directly, a match through its selected arms.
        */
        pub(super) fn anonymous_operand(
            &mut self,
            expression: ExpressionHandle,
            destination: PrimitiveType,
        ) -> Option<IntegerOperand> {
            if let Some(value) = land_anonymous_scalar_expression(
                self.program,
                self.operators,
                expression,
                destination,
            ) {
                return Some(IntegerOperand {
                    value,
                    value_source: expression,
                    domain: ArithmeticDomain::Exact,
                    computation: CheckedScalarComputationHandle::invalid(),
                });
            }
            self.contextual_match_operand(expression, destination)
        }

        fn contextual_match_operand(
            &mut self,
            expression: ExpressionHandle,
            destination: PrimitiveType,
        ) -> Option<IntegerOperand> {
            let ExpressionNode::Match(dispatch) =
                self.program.expression_table.expression(expression).clone()
            else {
                return None;
            };
            if !self.anonymous_result_lands(expression, destination) {
                return None;
            }
            let computation = self.dispatch(expression, &dispatch, destination)?;
            Some(IntegerOperand {
                value: parameter(0, destination),
                value_source: ExpressionHandle::invalid(),
                domain: ArithmeticDomain::Exact,
                computation,
            })
        }

        /*
            Result joins propagate a real destination to anonymous leaves only.
            Walking result edges never evaluates or substitutes dispatch subjects.
        */
        fn anonymous_result_lands(
            &self,
            expression: ExpressionHandle,
            destination: PrimitiveType,
        ) -> bool {
            if let Some(dispatch) = self.program.expression_table.match_node(expression) {
                let arms = self.program.expression_table.match_arms(dispatch.arms);
                return !arms.is_empty()
                    && arms
                        .iter()
                        .all(|arm| self.anonymous_result_lands(arm.value, destination));
            }
            land_anonymous_scalar_expression(self.program, self.operators, expression, destination)
                .is_some()
        }

        pub(super) fn integer_comparison(
            &mut self,
            source_expression: ExpressionHandle,
            binary: &TableBinaryExpression,
        ) -> Option<CheckedScalarComputationHandle> {
            let (mut left, mut right) = self.integer_operands(binary, None)?;
            let comparison =
                construct_integer_comparison(binary.operator, left.value, right.value)?;
            let comparison = match comparison {
                CheckedBooleanExpression::Not(value) => *value,
                value => value,
            };
            let CheckedBooleanExpression::IntegerComparison {
                left: landed_left,
                right: landed_right,
                ..
            } = comparison
            else {
                return None;
            };
            (left.value, right.value) = if matches!(
                binary.operator,
                BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
            ) {
                (*landed_right, *landed_left)
            } else {
                (*landed_left, *landed_right)
            };
            let left_type = scalar_expression_type(&left.value)?;
            let right_type = scalar_expression_type(&right.value)?;
            let template = construct_integer_comparison(
                binary.operator,
                parameter(0, left_type),
                parameter(1, right_type),
            )?;
            let left = self.materialize_integer(left)?;
            let right = self.materialize_integer(right)?;
            let operands = self.plans.operands.insert_many([left, right]);
            Some(self.insert(
                PrimitiveType::Bool,
                CheckedScalarComputationKind::Apply {
                    source_expression,
                    expression: CheckedScalarExpression::Boolean(Box::new(template)),
                    operands,
                },
            ))
        }
    }

    fn parameter(position: usize, primitive_type: PrimitiveType) -> CheckedScalarExpression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        }
    }
}
mod normal_return {
    /*
        Known results of freshly built computation DAGs, not permission to erase them.

        Calls and runtime storage stay unknown. A selection can nevertheless have
        one result on every normal-return path. Inspecting both branches here executes
        neither; the original graph still owns effects, failures, and evaluation order.
        Only Boolean selection and negation compose these results, matching flow's
        control facts. Strict operators still require closed scalar evaluation;
        interpreting a call-bearing comparison here would prune calls flow retained.
    */
    use super::{
        CheckedBooleanExpression as BooleanExpression, CheckedScalarComputationHandle,
        CheckedScalarComputationKind as ComputationKind, CheckedScalarComputationPlans,
        CheckedScalarExpression as ScalarExpression,
    };
    use facts::ScalarValue;

    pub(super) fn boolean_result(
        plans: &CheckedScalarComputationPlans,
        computation: CheckedScalarComputationHandle,
    ) -> Option<bool> {
        match &plans.nodes.get(computation).kind {
            ComputationKind::WithOperands { .. }
            | ComputationKind::StorageObservation { .. }
            | ComputationKind::SelectedComparison { .. }
            | ComputationKind::StorageAddress { .. }
            | ComputationKind::StructuralField { .. }
            | ComputationKind::ByteSequenceLiteralComparison { .. }
            | ComputationKind::ByteSequencePredicate { .. }
            | ComputationKind::CaseMembership { .. }
            | ComputationKind::BooleanToInteger { .. }
            | ComputationKind::AuthoredCast { .. } => None,
            ComputationKind::Qualification { operand, .. } => boolean_result(plans, *operand),
            ComputationKind::Value(ScalarExpression::Boolean(value)) => {
                boolean_expression_result(value)
            }
            ComputationKind::RetainedConstant { .. } | ComputationKind::Value(_) => None,
            ComputationKind::Call { .. } => None,
            ComputationKind::Dispatch { .. } => None,
            ComputationKind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => match boolean_result(plans, *condition) {
                Some(true) => boolean_result(plans, *when_true),
                Some(false) => boolean_result(plans, *when_false),
                None => {
                    let when_true = boolean_result(plans, *when_true)?;
                    let when_false = boolean_result(plans, *when_false)?;
                    (when_true == when_false).then_some(when_true)
                }
            },
            ComputationKind::Apply {
                expression,
                operands,
                ..
            } => {
                let ScalarExpression::Boolean(value) = expression else {
                    return None;
                };
                let BooleanExpression::Not(operand) = value.as_ref() else {
                    return None;
                };
                let [operand_computation] = plans.operands.span(*operands)? else {
                    return None;
                };
                if !matches!(
                    operand.as_ref(),
                    BooleanExpression::Parameter { position: 0 }
                ) {
                    return None;
                }
                Some(!boolean_result(plans, *operand_computation)?)
            }
        }
    }

    fn boolean_expression_result(expression: &BooleanExpression) -> Option<bool> {
        match expression {
            BooleanExpression::Not(operand) => Some(!boolean_expression_result(operand)?),
            BooleanExpression::And { left, right } | BooleanExpression::Or { left, right } => {
                let evaluate_when = matches!(expression, BooleanExpression::And { .. });
                let left = boolean_expression_result(left);
                if left == Some(!evaluate_when) {
                    return left;
                }
                let right = boolean_expression_result(right);
                if left == Some(evaluate_when) || right == Some(!evaluate_when) {
                    right
                } else {
                    None
                }
            }
            _ => match crate::values::evaluate_checked_scalar(
                &ScalarExpression::Boolean(Box::new(expression.clone())),
                &mut |_| None,
            )? {
                ScalarValue::Boolean(value) => Some(value),
                _ => None,
            },
        }
    }
}
mod quotient_calls {

    /*
        Explicit checked expression graph for sealed quotient requests. This does
        not admit a request: final publication consumes its separately checked row.
    */
    use super::Builder;
    use checked_trees::{
        CheckedQuotientAdaptationLicense, CheckedStructuralValueHandle, CheckedStructuralValueKind,
    };
    use typed_trees::{
        expression::{ExpressionHandle, ExpressionNode},
        types::TypeReferenceHandle,
    };

    impl Builder<'_, '_> {
        pub(super) fn quotient_scalar_call(
            &mut self,
            expression: ExpressionHandle,
            expected: typed_trees::types::PrimitiveType,
        ) -> Option<checked_trees::CheckedScalarComputationHandle> {
            let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
            else {
                return None;
            };
            let request = call.quotient_operation.as_ref()?;
            let (_, representative) = validation::find_machine_by_entry_state(
                self.program,
                request.representative_operation.symbol,
            )?;
            let result_type = representative.return_type;
            if self.program.primitive_type_reference(result_type) != Some(expected) {
                return None;
            }
            let receiver_value = self.quotient_call_value(expression, result_type)?;
            self.retain_structural_root(
                u32::try_from(self.statement_index).ok()?,
                expression,
                result_type,
                receiver_value,
            );
            Some(self.insert(expected, checked_trees::CheckedScalarComputationKind::StructuralField {
            atomic_ordering: None,
            result_type,
            source_expression: expression, receiver_expression: expression, source_type: result_type, receiver_value,
            subject: checked_trees::CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { value_dependency: Some(receiver_value), expression, logical_root_type_identity: None },
                path: Vec::new(), type_identity: validation::stored_structural_type_identity(self.program, result_type, &[], &[]),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
            field: symbols::SymbolHandle::invalid(), field_identity: Default::default(),
        }))
        }

        pub(super) fn quotient_call_value(
            &mut self,
            expression: ExpressionHandle,
            expected: TypeReferenceHandle,
        ) -> Option<CheckedStructuralValueHandle> {
            let ExpressionNode::Call(call) =
                self.program.expression_table.expression(expression).clone()
            else {
                return None;
            };
            let request = call.quotient_operation.as_ref()?;
            let (_, representative) = validation::find_machine_by_entry_state(
                self.program,
                request.representative_operation.symbol,
            )?;
            let parameters = self.program.state_parameters(representative).to_vec();
            let actuals = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if parameters.len() != actuals.len() || !call.carries_only_positional_arguments() {
                return None;
            }
            let mut arguments = Vec::new();
            for (position, (actual, formal)) in actuals.into_iter().zip(&parameters).enumerate() {
                if self
                    .program
                    .primitive_type_reference(formal.type_reference)
                    .is_some()
                {
                    arguments.push(self.structural_value(actual, formal.type_reference)?);
                    continue;
                }
                let actual_type = self.expression_type(actual)?;
                let value = self.structural_value(actual, actual_type)?;
                let value = if let Some(data) = self.program.closed_data_definition(actual_type)
                    && let Some(quotient) = data.quotient.as_ref()
                {
                    if self.program.normalized_type_identity(quotient.carrier)
                        != self.program.normalized_type_identity(formal.type_reference)
                        || self.program.type_multiplicity(actual_type)
                            != language_semantics::Multiplicity::Unrestricted
                    {
                        return None;
                    }
                    self.publish_structural_value(
                        actual,
                        formal.type_reference,
                        CheckedStructuralValueKind::QuotientAdaptation {
                            source: value,
                            quotient: actual_type,
                            license: CheckedQuotientAdaptationLicense::Input {
                                request: expression,
                                position: u32::try_from(position).ok()?,
                            },
                        },
                    )?
                } else {
                    if self.program.normalized_type_identity(actual_type)
                        != self.program.normalized_type_identity(formal.type_reference)
                    {
                        return None;
                    }
                    value
                };
                arguments.push(value);
            }
            let value = self.publish_structural_value(
                expression,
                representative.return_type,
                CheckedStructuralValueKind::QuotientCall {
                    request: expression,
                    representative_state: representative.symbol,
                    arguments,
                },
            )?;
            if let Some(data) = self.program.closed_data_definition(expected)
                && let Some(quotient) = data.quotient.as_ref()
            {
                if self.program.normalized_type_identity(quotient.carrier)
                    != self
                        .program
                        .normalized_type_identity(representative.return_type)
                    || self.program.type_multiplicity(expected)
                        != language_semantics::Multiplicity::Unrestricted
                {
                    return None;
                }
                self.publish_structural_value(
                    expression,
                    expected,
                    CheckedStructuralValueKind::QuotientAdaptation {
                        source: value,
                        quotient: expected,
                        license: CheckedQuotientAdaptationLicense::Result {
                            request: expression,
                        },
                    },
                )
            } else {
                (self.program.normalized_type_identity(expected)
                    == self
                        .program
                        .normalized_type_identity(representative.return_type))
                .then_some(value)
            }
        }
    }
}
mod rank_observations {

    /*
        Arrival ranks are ordinary checked scalar computations. The source ranking
        owner selects the view, proves formation and every cyclic edge, and retains
        its exact state telescope. This producer only binds that view's inputs to
        captured formals; it does not infer descent from the executable expression.
        A mutable formal here denotes its arrival value, never a later StorageRead.
    */
    mod operands {
        /*
            Read retained rank inputs through the ordinary scalar graph. Each operand
            keeps its own carrier; exact distance produces the common unsigned word.
        */
        use super::{Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind};
        use checked_trees::{
            CheckedNaturalRankMeasure, CheckedRankArrivalPlan, CheckedRankOperand,
            CheckedScalarExpression,
        };
        use numerics::arithmetic::ArithmeticDomain;
        use typed_trees::{
            expression::{BinaryOperator, ExpressionHandle},
            signature::StateParameter,
            types::PrimitiveType,
        };

        pub(super) fn declaration_parameters(
            program: &dyn validation::CheckingInput,
            admitted: &CheckedRankArrivalPlan,
            measure: &CheckedNaturalRankMeasure,
            inputs: &[StateParameter],
        ) -> Option<Vec<StateParameter>> {
            let mut parameters = inputs.to_vec();
            if let CheckedNaturalRankMeasure::Distance { lower, upper } = measure {
                for input in [*lower, *upper] {
                    if let CheckedRankOperand::Expression { state, .. } =
                        admitted.rank_operands.get(input)
                    {
                        let (_, state) = validation::find_state(program, *state)?;
                        for parameter in program.state_parameters(state).iter() {
                            if !parameters
                                .iter()
                                .any(|known| known.symbol == parameter.symbol)
                            {
                                parameters.push(parameter.clone());
                            }
                        }
                    }
                }
            }
            Some(parameters)
        }

        /*
            Formal-to-actual places of each expression operand declared by another
            participant and evaluated at `evaluator`'s entry.
        */
        pub(super) fn transported_bindings(
            admitted: &CheckedRankArrivalPlan,
            evaluator: symbols::SymbolHandle,
            operands: [arena::Handle<CheckedRankOperand>; 2],
        ) -> Option<Vec<(validation::CanonicalPlace, validation::CanonicalPlace)>> {
            let place = |coordinate: checked_trees::CheckedRankCoordinateHandle| {
                let coordinate = admitted.coordinates.get(coordinate);
                Some(validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(coordinate.parameter),
                    segments: admitted.path_segments.span(coordinate.path)?.to_vec(),
                })
            };
            let mut bindings = Vec::new();
            for operand in operands {
                let CheckedRankOperand::Expression {
                    state, formation, ..
                } = admitted.rank_operands.get(operand)
                else {
                    continue;
                };
                if *state == evaluator || !formation.is_valid() {
                    continue;
                }
                let formation = admitted.application_formations.get(*formation);
                for binding in admitted.bindings.span(formation.bindings)? {
                    bindings.push((place(binding.entry)?, place(binding.destination)?));
                }
            }
            Some(bindings)
        }

        pub(super) fn difference<'program>(
            builder: &mut Builder<'program, '_>,
            admitted: &'program CheckedRankArrivalPlan,
            lower: arena::Handle<CheckedRankOperand>,
            upper: arena::Handle<CheckedRankOperand>,
        ) -> Option<CheckedScalarComputationHandle> {
            let lower = read(builder, admitted, lower)?;
            let upper = read(builder, admitted, upper)?;
            let primitive_type = PrimitiveType::U64;
            let parameter = |position| CheckedScalarExpression::Parameter {
                position,
                primitive_type,
            };
            let (expression, _) = crate::values::scalar::scalar_lowering::construct_integer_binary(
                BinaryOperator::Subtract,
                parameter(0),
                ArithmeticDomain::Exact,
                parameter(1),
                ArithmeticDomain::Exact,
                true,
                false,
            )?;
            let operands = builder.plans.operands.insert_many([upper, lower]);
            Some(builder.insert(
                primitive_type,
                CheckedScalarComputationKind::Apply {
                    source_expression: ExpressionHandle::invalid(),
                    expression,
                    operands,
                },
            ))
        }

        fn read<'program>(
            builder: &mut Builder<'program, '_>,
            admitted: &'program CheckedRankArrivalPlan,
            input: arena::Handle<CheckedRankOperand>,
        ) -> Option<CheckedScalarComputationHandle> {
            let input = admitted.rank_operands.get(input);
            let (value, primitive_type) = match input {
                CheckedRankOperand::Coordinate {
                    coordinate,
                    primitive_type,
                    observation,
                } => {
                    let coordinate = admitted.coordinates.get(*coordinate);
                    let place = validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(coordinate.parameter),
                        segments: admitted.path_segments.span(coordinate.path)?.to_vec(),
                    };
                    let value = match observation {
                        checked_trees::CheckedRankObservation::Value => builder.scalar_coordinate(
                            ExpressionHandle::invalid(),
                            &place,
                            *primitive_type,
                            admitted.entry_indexes.span(coordinate.entry_indexes)?,
                        )?,
                        checked_trees::CheckedRankObservation::Length => {
                            let (_, state) =
                                validation::find_state(builder.program, builder.state)?;
                            let (_, _, value) = validation::ranking_slice_length_observation(
                                builder.program,
                                state,
                                &validation::RankSubjectCoordinate {
                                    parameter: coordinate.parameter,
                                    path: place.segments.clone(),
                                },
                            )?;
                            builder.insert(
                                PrimitiveType::U64,
                                CheckedScalarComputationKind::Value(value),
                            )
                        }
                    };
                    (value, *primitive_type)
                }
                CheckedRankOperand::Expression {
                    state,
                    expression,
                    primitive_type,
                    formation,
                } => {
                    let previous_scope = builder.declaration_scope.take();
                    let previous_casts = builder.exact_integer_casts;
                    let value = (|| {
                        if formation.is_valid() {
                            let application = admitted.application_formations.get(*formation);
                            let (machine, _) =
                                validation::find_state(builder.program, builder.state)?;
                            if application.machine != builder.machine
                                || application.machine_body != machine.states
                                || application.state != builder.state
                                || application.application.owner != *state
                                || application.application.definition_body != *expression
                            {
                                return None;
                            }
                            builder.exact_integer_casts = admitted
                                .application_casts
                                .span(application.application.casts)?;
                            builder.declaration_scope =
                                Some(super::super::builder::DeclarationScope {
                                    plan: admitted,
                                    application: *formation,
                                });
                        }
                        builder.expression(*expression, *primitive_type)
                    })();
                    builder.declaration_scope = previous_scope;
                    builder.exact_integer_casts = previous_casts;
                    (value?, *primitive_type)
                }
                CheckedRankOperand::Literal(value) => {
                    let value = builder.insert(
                        PrimitiveType::U64,
                        CheckedScalarComputationKind::Value(facts::u64_integer_literal(
                            *value,
                            PrimitiveType::U64,
                        )?),
                    );
                    (value, PrimitiveType::U64)
                }
            };
            if primitive_type == PrimitiveType::U64 {
                return Some(value);
            }
            let operand = CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type,
            };
            let expression = if matches!(
                primitive_type,
                typed_trees::unsigned_integer_primitive_pattern!()
            ) {
                CheckedScalarExpression::IntegerWiden {
                    primitive_type: PrimitiveType::U64,
                    operand: Box::new(operand),
                }
            } else {
                let (_, maximum) = primitive_type.integer_bounds()?;
                if maximum > i128::from(u64::MAX) {
                    return None;
                }
                CheckedScalarExpression::IntegerExactCast {
                    primitive_type: PrimitiveType::U64,
                    operand: Box::new(operand),
                    range: checked_trees::CheckedIntegerRange {
                        minimum: numerics::bignum::BigInt::zero(),
                        maximum: numerics::bignum::BigInt::from_i128(maximum),
                    },
                    formation_decided: true,
                }
            };
            let operands = builder.plans.operands.insert_many([value]);
            Some(builder.insert(
                PrimitiveType::U64,
                CheckedScalarComputationKind::Apply {
                    source_expression: ExpressionHandle::invalid(),
                    expression,
                    operands,
                },
            ))
        }
    }

    use super::{Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind};
    use crate::published::places::place_at;
    use checked_trees::{
        CheckedIntegerRankObservation, CheckedNaturalRankMeasure as RankMeasure,
        CheckedScalarExpression, CheckedScalarExpressionRole,
        CheckedStateNaturalRank as NaturalRank,
    };
    use numerics::arithmetic::ArithmeticDomain;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle};
    use typed_trees::types::PrimitiveType;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn record(
        program: &dyn validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &facts::FactPlan,
        flow: &checked_trees::FlowFacts,
        borrow: &checked_trees::BorrowFacts,
        proof: &checked_trees::ProofFacts,
        pure: &checked_trees::CheckedScalarExpressionPlans,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
        admitted: &checked_trees::CheckedRankArrivalPlan,
        plans: &mut checked_trees::CheckedScalarComputationPlans,
        structural_values: &mut checked_trees::CheckedStructuralValuePlans,
    ) {
        if admitted.observations.is_empty() {
            return;
        }
        let carriers = crate::execution::terminal_unit::types::carrier_index(program);
        let mut roster = Vec::<NaturalRank>::new();
        for machine in program.machines() {
            if let Some((ranks, _)) =
                crate::execution::terminal_unit::state_graph::natural_rank_plans(
                    program, machine, admitted,
                )
            {
                roster.extend(ranks);
            }
        }
        roster.sort_by_key(|rank| (rank.state.arena_index(), rank.state.generation()));
        let mut remaining = roster.as_slice();
        while let Some(rank) = remaining.first() {
            let count = remaining
                .iter()
                .take_while(|candidate| candidate.state == rank.state)
                .count();
            let candidates = &remaining[..count];
            remaining = &remaining[count..];
            if candidates.iter().any(|candidate| candidate != rank) {
                continue;
            }
            let Some((owner, state)) = validation::find_state(program, rank.state) else {
                continue;
            };
            let mut inputs = program.state_parameters(state).to_vec();
            for parameter in &mut inputs {
                parameter.is_mutable = false;
            }
            let Some(body) = admitted.for_body(owner.symbol, owner.states) else {
                continue;
            };
            let mut rows = admitted
                .states
                .span_or_empty(body.states)
                .iter()
                .filter(|row| row.state == state.symbol);
            let Some(row) = rows.next() else {
                continue;
            };
            if rows.next().is_some() {
                continue;
            }
            let Some(bindings) = validation::RankStateBindings::from_checked(admitted, row) else {
                continue;
            };
            let place =
                |coordinate: &validation::RankSubjectCoordinate| validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(coordinate.parameter),
                    segments: coordinate.path.clone(),
                };
            let Some(components) = admitted.observation_components.span(rank.components) else {
                continue;
            };
            for (component, rank) in components.iter().enumerate() {
                let Ok(component) = u32::try_from(component) else {
                    continue;
                };
                let mut declaration_bindings = bindings
                    .coordinates
                    .iter()
                    .map(|binding| (place(&binding.entry), place(&binding.destination)))
                    .collect::<Vec<_>>();
                let subject =
                    typed_trees::ranking::resolve_machine_witness_subjects(program, owner)
                        .and_then(|subjects| subjects.first().copied());
                if let RankMeasure::DeclaredMeasure {
                    bindings,
                    formation,
                    ..
                } = rank.measure
                {
                    let inputs = (|| {
                        let mut direct = Vec::new();
                        for input in admitted.bindings.span(bindings)? {
                            if !admitted
                                .application_contexts
                                .span(input.application_context)?
                                .is_empty()
                            {
                                continue;
                            }
                            let entry = admitted.coordinates.get(input.entry);
                            let destination = admitted.coordinates.get(input.destination);
                            direct.push((
                                validation::CanonicalPlace {
                                    root: facts::PlaceRoot::Symbol(entry.parameter),
                                    segments: admitted.path_segments.span(entry.path)?.to_vec(),
                                },
                                validation::CanonicalPlace {
                                    root: facts::PlaceRoot::Symbol(destination.parameter),
                                    segments: admitted
                                        .path_segments
                                        .span(destination.path)?
                                        .to_vec(),
                                },
                            ));
                        }
                        Some(direct)
                    })();
                    let Some(inputs) = inputs else {
                        continue;
                    };
                    declaration_bindings.extend(inputs);
                    if admitted.application_formations.is_valid(formation)
                        && let Some(input) =
                            admitted.application_formations.get(formation).input_home
                    {
                        let coordinate = admitted.coordinates.get(input.actual);
                        let Some(path) = admitted.path_segments.span(coordinate.path) else {
                            continue;
                        };
                        declaration_bindings.push((
                            validation::CanonicalPlace {
                                root: facts::PlaceRoot::Symbol(input.formal),
                                segments: Vec::new(),
                            },
                            validation::CanonicalPlace {
                                root: facts::PlaceRoot::Symbol(coordinate.parameter),
                                segments: path.to_vec(),
                            },
                        ));
                    }
                }
                if let RankMeasure::Distance { lower, upper } = rank.measure {
                    let Some(transported) =
                        operands::transported_bindings(admitted, state.symbol, [lower, upper])
                    else {
                        continue;
                    };
                    declaration_bindings.extend(transported);
                }
                let parameters = inputs
                    .iter()
                    .filter(|parameter| {
                        crate::values::scalar::occupies_scalar_position(program, parameter)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let Some(parameter_types) = parameters
                    .iter()
                    .map(|parameter| program.primitive_type_reference(parameter.type_reference))
                    .collect::<Option<Vec<_>>>()
                else {
                    continue;
                };
                let Some(declaration_parameters) =
                    operands::declaration_parameters(program, admitted, &rank.measure, &inputs)
                else {
                    continue;
                };
                let statement_nodes_start = plans.nodes.storage_slice().len();
                let mut builder = Builder {
                    program,
                    shapes:
                        crate::execution::terminal_unit::types::ShapeCollector::with_carrier_index(
                            program,
                            carriers.clone(),
                        ),
                    carriers: carriers.clone(),
                    call_result_types: Default::default(),
                    operators,
                    semantic,
                    flow,
                    borrow,
                    proof,
                    pure,
                    exact_integer_casts,
                    machine: owner.symbol,
                    state: state.symbol,
                    statement_index: 0,
                    parameters: &parameters,
                    authored_parameters: &inputs,
                    declaration_parameters: &declaration_parameters,
                    declaration_scope: None,
                    declaration_values: Vec::new(),
                    declaration_bindings: &declaration_bindings,
                    parameter_types: &parameter_types,
                    locals: &[],
                    plans,
                    structural_values,
                    statement_nodes_start,
                    root_scope: super::builder::RootScope::RankObservation,
                    dispatch_receivers: Default::default(),
                    scalar_occurrences: Vec::new(),
                    integer_occurrences: Vec::new(),
                };
                let root = observation(&mut builder, rank, subject, &bindings, admitted);
                if let Some(root) = root {
                    let _ = builder.push_scalar_root(
                        CheckedScalarExpressionRole::RankObservation { component },
                        root,
                    );
                }
            }
        }
    }

    fn input_coordinate(
        builder: &mut Builder<'_, '_>,
        rank: &CheckedIntegerRankObservation,
        subject: Option<ExpressionHandle>,
        bindings: &validation::RankStateBindings,
        primitive_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let (owner, _) = validation::find_state(builder.program, builder.state)?;
        let entry = builder.program.machine_states(owner).first()?;
        let source = if let Some(subject) = subject {
            place_at(builder.program, entry.symbol, 0, subject)?
        } else {
            let parameter = builder
                .program
                .state_parameters(entry)
                .get(usize::try_from(rank.parameter_position).ok()?)?;
            if parameter.symbol != rank.parameter
                || builder
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    != Some(primitive_type)
            {
                return None;
            }
            validation::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(parameter.symbol),
                segments: Vec::new(),
            }
        };
        let facts::PlaceRoot::Symbol(parameter) = source.root else {
            return None;
        };
        let coordinate = bindings.destination(&validation::RankSubjectCoordinate {
            parameter,
            path: source.segments,
        })?;
        if coordinate.parameter != rank.parameter {
            return None;
        }
        let source = validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(coordinate.parameter),
            segments: coordinate.path,
        };
        builder.scalar_coordinate(subject.unwrap_or_default(), &source, primitive_type, &[])
    }

    fn observation<'program>(
        builder: &mut Builder<'program, '_>,
        rank: &CheckedIntegerRankObservation,
        subject: Option<ExpressionHandle>,
        bindings: &validation::RankStateBindings,
        admitted: &'program checked_trees::CheckedRankArrivalPlan,
    ) -> Option<CheckedScalarComputationHandle> {
        let observed = match rank.measure {
            RankMeasure::DeclaredMeasure {
                formation,
                measure,
                bindings: application_inputs,
                primitive_type,
            } => {
                let definition = builder
                    .program
                    .measures()
                    .iter()
                    .find(|definition| definition.symbol == measure)?;
                let formal = definition.parameter.as_ref()?;
                let [body] = builder
                    .program
                    .expression_table
                    .expression_handles(definition.body)
                else {
                    return None;
                };
                for input in admitted.bindings.span(application_inputs)? {
                    if !admitted
                        .application_contexts
                        .span(input.application_context)?
                        .is_empty()
                    {
                        continue;
                    }
                    let source = admitted.coordinates.get(input.entry);
                    if source.parameter != formal.symbol {
                        return None;
                    }
                    let formal_place = validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(source.parameter),
                        segments: admitted.path_segments.span(source.path)?.to_vec(),
                    };
                    let geometry = validation::structural_scalar_coordinate_in_type(
                        builder.program,
                        formal_place.root,
                        formal.type_reference,
                        &formal_place.segments,
                    )?;
                    let input = admitted.coordinates.get(input.destination);
                    let coordinate = validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(input.parameter),
                        segments: admitted.path_segments.span(input.path)?.to_vec(),
                    };
                    let operand = builder.scalar_coordinate(
                        subject.unwrap_or_default(),
                        &coordinate,
                        geometry.primitive_type,
                        admitted.entry_indexes.span(input.entry_indexes)?,
                    )?;
                    builder.declaration_values.push((formal_place, operand));
                }
                if formation.is_valid() {
                    let proof = admitted.application_formations.get(formation);
                    let (owner, _) = validation::find_state(builder.program, builder.state)?;
                    if proof.machine != builder.machine
                        || proof.machine_body != owner.states
                        || proof.state != builder.state
                        || proof.application.owner != measure
                        || proof.application.definition_body != *body
                        || proof.bindings != application_inputs
                    {
                        return None;
                    }
                    builder.exact_integer_casts =
                        admitted.application_casts.span(proof.application.casts)?;
                    builder.declaration_scope = Some(super::builder::DeclarationScope {
                        plan: admitted,
                        application: formation,
                    });
                }
                builder.declaration_parameters = std::slice::from_ref(formal);
                builder.expression(*body, primitive_type)
            }
            RankMeasure::IntegerValue {
                coordinate,
                primitive_type,
            } => {
                let coordinate = admitted.coordinates.get(coordinate);
                let path = admitted.path_segments.span(coordinate.path)?;
                if coordinate.parameter != rank.parameter {
                    return None;
                }
                let (_, state) = validation::find_state(builder.program, builder.state)?;
                let parameter = builder
                    .program
                    .state_parameters(state)
                    .get(usize::try_from(rank.parameter_position).ok()?)?;
                if parameter.symbol != coordinate.parameter {
                    return None;
                }
                builder.scalar_coordinate(
                    subject.unwrap_or_default(),
                    &validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(coordinate.parameter),
                        segments: path.to_vec(),
                    },
                    primitive_type,
                    admitted.entry_indexes.span(coordinate.entry_indexes)?,
                )
            }
            RankMeasure::SequenceLength { coordinate } => {
                let (_, state) = validation::find_state(builder.program, builder.state)?;
                let coordinate = admitted.coordinates.get(coordinate);
                let coordinate = validation::RankSubjectCoordinate {
                    parameter: coordinate.parameter,
                    path: admitted.path_segments.span(coordinate.path)?.to_vec(),
                };
                let (parameter, position, expression) =
                    validation::ranking_slice_length_observation(
                        builder.program,
                        state,
                        &coordinate,
                    )?;
                (parameter == rank.parameter && position == rank.parameter_position)
                    .then_some(())?;
                Some(builder.insert(
                    PrimitiveType::U64,
                    CheckedScalarComputationKind::Value(expression),
                ))
            }
            RankMeasure::Distance { lower, upper } => {
                operands::difference(builder, admitted, lower, upper)
            }
            RankMeasure::IncreasingTo { primitive_type, .. } => {
                let lower = input_coordinate(builder, rank, subject, bindings, primitive_type)?;
                distance(builder, lower, primitive_type)
            }
        }?;
        match rank.function {
            checked_trees::CheckedNaturalRankFunction::Identity => Some(observed),
            checked_trees::CheckedNaturalRankFunction::Complement => {
                let primitive = match rank.measure {
                    RankMeasure::SequenceLength { .. } => PrimitiveType::U64,
                    RankMeasure::DeclaredMeasure { primitive_type, .. }
                    | RankMeasure::IntegerValue { primitive_type, .. }
                    | RankMeasure::IncreasingTo { primitive_type, .. } => primitive_type,
                    RankMeasure::Distance { .. } => return None,
                };
                distance(builder, observed, primitive)
            }
        }
    }

    /*
        Preserve the source distance's established total normalization, MAX-lower.
        The source judgment proves the bound remains invariant, so this differs
        from upper-lower by a constant and orders the same edges even at a crossed
        loop header. No new lower<=upper precondition or saturation is introduced.
    */
    fn distance(
        builder: &mut Builder<'_, '_>,
        lower: CheckedScalarComputationHandle,
        primitive_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let maximum = checked_trees::CheckedNaturalRankFunction::Complement
            .complement_ceiling(primitive_type)?;
        let ceiling = builder.insert(
            primitive_type,
            CheckedScalarComputationKind::Value(facts::u64_integer_literal(
                maximum,
                primitive_type,
            )?),
        );
        let template = |position| CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        };
        let (expression, _) = crate::values::scalar::scalar_lowering::construct_integer_binary(
            BinaryOperator::Subtract,
            template(0),
            ArithmeticDomain::Exact,
            template(1),
            ArithmeticDomain::Exact,
            true,
            false,
        )?;
        let operands = builder.plans.operands.insert_many([ceiling, lower]);
        Some(builder.insert(
            primitive_type,
            CheckedScalarComputationKind::Apply {
                source_expression: ExpressionHandle::invalid(),
                expression,
                operands,
            },
        ))
    }
}
mod reference_captures {
    /*
        Construct reference values from their recorded source and permitted access.
        A reference parameter supplies that information on entry. Locals, field
        projections and new borrows use their own borrow records. Keep the current
        pointer and slice length; rebuilding them from an earlier allocation could
        select the wrong storage or widen the view.
    */
    use super::{Builder, ExpressionHandle};
    use crate::flow::state_statements;
    use checked_trees::{BorrowAccessKind, CheckedStructuralAccess};
    use typed_trees::types::TypeReferenceHandle;

    impl Builder<'_, '_> {
        pub(super) fn captured_reference_place(
            &self,
            expression: ExpressionHandle,
            expected_access: language_semantics::ReferenceAccess,
            descriptor: Option<validation::CanonicalPlace>,
        ) -> Option<(validation::CanonicalPlace, CheckedStructuralAccess)> {
            let kind = BorrowAccessKind::from(expected_access);
            let access = CheckedStructuralAccess::from(expected_access);
            let state = self.borrow.unique_state_borrow(self.machine, self.state)?;
            let local = self
                .program
                .state_by_symbol(self.state)
                .and_then(|state| {
                    state_statements(self.program, state.statement_nodes).get(self.statement_index)
                })
                .and_then(|statement| statement.as_local_data())
                .filter(|local| local.initial_value == expression);
            let incoming = descriptor.as_ref().and_then(|place| {
                if !place.segments.is_empty()
                    || self.canonical_place(expression).as_ref() != Some(place)
                {
                    return None;
                }
                let parameter = self.program.state_parameter_by_symbol(
                    self.program.state_by_symbol(self.state)?,
                    place.root.as_symbol()?,
                )?;
                let declared = validation::reference_shell(self.program, parameter.type_reference)?;
                crate::values::reference_sources::access_can_supply(declared.access.into(), access)
                    .then_some(parameter.symbol)
            });
            let mut source = None;
            let mut retain_capture = |root, segments, captured_kind: &BorrowAccessKind| {
                if *captured_kind != kind {
                    return None;
                }
                let place = if let Some(descriptor) = &descriptor {
                    descriptor.clone()
                } else {
                    validation::CanonicalPlace {
                        root,
                        segments: self.borrow.access_segments.span(segments)?.to_vec(),
                    }
                };
                if source.as_ref().is_some_and(|prior| *prior != place) {
                    return None;
                }
                source = Some(place);
                Some(())
            };
            if let Some(parameter) = incoming {
                retain_capture(
                    facts::PlaceRoot::Symbol(parameter),
                    Default::default(),
                    &kind,
                )?;
            }
            for loan in self.borrow.loans.span(state.loans)?.iter().filter(|loan| {
                loan.statement_index == self.statement_index
                    && loan.formation_expression == expression
                    && match loan.formation_owner_path {
                        Some(path) => path.is_empty(),
                        None => local.is_some_and(|local| {
                            loan.owner_symbol == local.symbol
                                && loan.storage_owner == facts::PlaceRoot::Symbol(local.symbol)
                                && loan.owner_path.is_empty()
                        }),
                    }
            }) {
                retain_capture(loan.root, loan.segments, &loan.kind)?;
            }
            if validation::reference_formation_source(self.program, expression).is_some() {
                for call in self
                    .borrow
                    .calls
                    .span(state.calls)?
                    .iter()
                    .filter(|call| call.statement_index == self.statement_index)
                {
                    for access in self
                        .borrow
                        .argument_accesses
                        .span(call.accesses)?
                        .iter()
                        .filter(|access| {
                            access.expression == expression
                                && access.end_before_access.is_none()
                                && access.kind == kind
                        })
                    {
                        retain_capture(access.root, access.segments, &access.kind)?;
                    }
                }
            }
            let mut returned_site = None;
            for returned in self
                .borrow
                .returned_loans
                .span(state.returned_loans)?
                .iter()
                .filter(|returned| {
                    returned.statement_index == self.statement_index
                        && returned.formation_expression == expression
                        && returned
                            .formation_owner_path
                            .is_some_and(|path| path.is_empty())
                })
            {
                if returned_site.is_some_and(|site| site != returned.transition_target) {
                    return None;
                }
                returned_site = Some(returned.transition_target);
                retain_capture(returned.root, returned.segments, &returned.kind)?;
            }
            for (_, resource) in self
                .borrow
                .call_loan_resources
                .iter()
                .filter(|(_, resource)| {
                    resource.machine_symbol == self.machine
                        && resource.state_symbol == self.state
                        && resource.owner_path.is_empty()
                })
            {
                let call = self.flow.control.calls.get(resource.call);
                let operand = self.flow.control.operands.get(resource.owner_operand);
                if call.statement_index != self.statement_index
                    || operand.state_symbol != self.state
                    || operand.statement_index != self.statement_index
                    || operand.expression != expression
                    || !call
                        .operands
                        .handles()
                        .any(|handle| handle == resource.owner_operand)
                {
                    continue;
                }
                match resource.source {
                    checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                        let loan = self.borrow.loans.get(loan);
                        retain_capture(loan.root, loan.segments, &resource.access)?;
                    }
                    checked_trees::CheckedCallBorrowLoanSource::CallAccess { access, .. } => {
                        let argument = self.borrow.argument_accesses.get(access);
                        retain_capture(argument.root, argument.segments, &resource.access)?;
                    }
                }
            }
            Some((source?, access))
        }

        /*
            Borrowed storage can supply a reference value, never an owned extraction
            of its descriptor slot. Select that disposition from the declared storage
            boundary and admitted occurrence capture. Owned descriptor homes keep
            their ordinary move/copy receipts, even when they carry the same type.
        */
        pub(super) fn reference_capture_source(
            &self,
            expression: ExpressionHandle,
            expected: TypeReferenceHandle,
        ) -> Option<validation::ReferenceFormationSource> {
            if let Some(formation) =
                validation::reference_formation_source(self.program, expression)
            {
                return Some(formation);
            }
            let shell = validation::reference_shell(self.program, expected)?;
            let place = self.canonical_place(expression)?;
            let mut prefix = validation::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            };
            let mut borrowed = false;
            for segment in &place.segments {
                let carrier = validation::canonical_place_type_reference(
                    self.program,
                    self.state,
                    self.statement_index,
                    &prefix,
                )?;
                borrowed |= validation::reference_shell(self.program, carrier).is_some();
                prefix.segments.push(*segment);
            }
            let incoming = place.segments.is_empty()
                && place.root.as_symbol().is_some_and(|root| {
                    self.program
                        .state_by_symbol(self.state)
                        .and_then(|state| self.program.state_parameter_by_symbol(state, root))
                        .is_some_and(|parameter| {
                            validation::reference_shell(self.program, parameter.type_reference)
                                .is_some()
                        })
                });
            if !incoming && !borrowed {
                return None;
            }
            self.captured_reference_place(expression, shell.access, Some(place))?;
            Some(validation::ReferenceFormationSource {
                source: expression,
                access: shell.access,
                view_type: TypeReferenceHandle::invalid(),
                interpretation: validation::ReferenceFormationInterpretation::Declared,
            })
        }
    }
}
mod slice_view {
    /*
        Element-view establishment under the scalar operand owner.

        A `let view: &[T]` or `let slots: &mut [T]` local initialized by a
        collection opener lends storage that already exists, so the value rejoins
        the loan checked borrow admission recorded instead of producing a call
        result. The loan's access
        is the view's access: a shared read makes a `BorrowedSliceView`, an
        exclusive `Mutable` loan a `MutableSliceView`. The declared view's element
        equals the lent collection's element for an ordinary collection opener.
        A representation view instead retains its authored byte start and source
        extent: the existing recast judgment admits the relation, and Terminal
        independently proves the emitted region's bounds and exact target tiling.
        Reinterpreting the entire buffer before slicing is not equivalent: the
        suffix can tile even when the whole source cannot.
    */
    use super::{
        Builder, ExpressionHandle, ExpressionNode, StatementNode, TypeReferenceNode,
        structural_values::canonical_place_is_borrowable,
    };
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::calls::projected_argument_path;
    use crate::execution::terminal_unit::calls::projected_observed_path;
    use crate::execution::terminal_unit::types::byte_sequence_carrier;
    use crate::execution::terminal_unit::types::element_view_shell;
    use checked_trees::{
        CheckedByteSequenceCarrier, CheckedElementViewRegion as ViewRegion,
        CheckedReferenceStorage, CheckedScalarExpressionRole, CheckedStructuralAccess,
        CheckedStructuralValueHandle, CheckedStructuralValueKind as ValueKind, CheckedSubsliceSite,
        CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    };
    use facts::PlaceSegment;
    use typed_trees::signature::state_parameter_position;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
    use validation::call_target_symbol;
    use validation::canonical_place_from_expression;
    use validation::canonical_place_type_reference;
    use validation::expression_result_type_reference;
    use validation::indexed_operation_has_builtin_meaning;
    use validation::slice_recast_byte_region;
    use validation::slice_recast_source;
    use validation::slice_view_shell;
    use validation::stored_structural_type_identity;
    use validation::subslice_expression;

    mod formation {
        /*
            View production consumes the borrow owner's evaluated occurrence captures.
            Local storage, normal returns and call operands retain the same expression
            coordinate, but different lifecycle owners. Their captures grant access;
            the declared carrier only identifies the descriptor to transport.

            An established view's home carries its current pointer and extent. Its loan
            backing is not a substitute: reconstructing that backing can widen a narrowed
            result or revive contents replaced after the original initializer.
        */
        use super::{Builder, ExpressionHandle, TypeReferenceHandle};
        use checked_trees::CheckedStructuralAccess;

        impl Builder<'_, '_> {
            pub(super) fn view_formation(
                &self,
                expression: ExpressionHandle,
                expected: TypeReferenceHandle,
                expected_access: language_semantics::ReferenceAccess,
            ) -> Option<(validation::CanonicalPlace, CheckedStructuralAccess)> {
                if expected_access == language_semantics::ReferenceAccess::WriteOnly {
                    return None;
                }
                let descriptor = if let Some(place) = self
                    .canonical_place(expression)
                    .filter(|place| place.root.as_symbol().is_some())
                {
                    let declared = validation::canonical_place_type_reference(
                        self.program,
                        self.state,
                        self.statement_index,
                        &place,
                    )?;
                    if validation::slice_view_shell(self.program, declared, &[]).is_some() {
                        use crate::execution::terminal_unit::{calls::view_subslice, types};
                        let identity = |reference| {
                            let family =
                                if types::byte_sequence_carrier(self.program, reference, &[])
                                    .is_some_and(|carrier| carrier.is_borrowed_view())
                                {
                                    view_subslice::ViewKind::Bytes
                                } else {
                                    view_subslice::ViewKind::Elements
                                };
                            Some((
                                family,
                                view_subslice::view_identity(self.program, family, reference)?,
                            ))
                        };
                        if identity(declared)? != identity(expected)? {
                            return None;
                        }
                        Some(place)
                    } else {
                        None
                    }
                } else {
                    None
                };
                self.captured_reference_place(expression, expected_access, descriptor)
            }
        }
    }

    impl Builder<'_, '_> {
        /*
            One whole `[T]` view of the collection place this statement lends.
            Fixed arrays and vectors own contiguous storage and a slice borrows
            it, so a view local denotes the lent place under its loan rather than
            a fresh value with copied elements, and its stored length is that
            collection's own extent. Checked borrow admission already recorded the
            loan or transition argument roster names the exact lent place and
            access. Formation rejoins that lexical owner before the common
            collection/region walk; a local and an inline operand publish the same
            descriptor operation at their respective evaluation points.
        */
        pub(super) fn slice_view_place(
            &mut self,
            expression: ExpressionHandle,
            expected: TypeReferenceHandle,
        ) -> Option<ValueKind> {
            if subslice_expression(self.program, expression).is_some() {
                return None;
            }
            if let Some(call) = self.program.expression_table.call_node(expression)
                && self
                    .call_ordinal(expression, call_target_symbol(self.program, call))
                    .is_some()
            {
                return None;
            }
            let view = element_view_shell(self.program, expected, &[]).or_else(|| {
                matches!(
                    byte_sequence_carrier(self.program, expected, &[],),
                    Some(CheckedByteSequenceCarrier::BorrowedView { .. })
                )
                .then(|| slice_view_shell(self.program, expected, &[]))
                .flatten()
            })?;
            let state = self.program.state_by_symbol(self.state)?;
            let machine = self.program.machine_by_symbol(self.machine)?;
            let authored_recast = slice_recast_source(self.program, expression);
            if let Some(authored) = authored_recast
                && let Some(declared) =
                    expression_result_type_reference(self.program, machine, state, authored.source)
                && slice_view_shell(self.program, declared, &[]).is_some()
            {
                if view.access != language_semantics::ReferenceAccess::Shared
                    || authored.access != view.access
                    || self.program.normalized_type_identity(authored.element)
                        != self.program.normalized_type_identity(view.element)
                {
                    return None;
                }
                let (source, _, selectors) =
                    self.borrowed_leaf_argument(authored.source, declared, expression)?;
                self.retain_reference_selectors(expression, selectors)?;
                return Some(ValueKind::BorrowedSliceView {
                    source,
                    region: ViewRegion::WholeRepresentation,
                });
            }
            let (mut place, access) = self.view_formation(expression, expected, view.access)?;
            let recast = slice_recast_byte_region(self.program, machine, state, expression);
            if let Some(authored) = authored_recast {
                if authored.access != view.access
                    || self.program.normalized_type_identity(authored.element)
                        != self.program.normalized_type_identity(view.element)
                {
                    return None;
                }
                if recast.is_none() {
                    let collection =
                        canonical_place_from_expression(self.program, authored.source)?;

                    if collection != place {
                        return None;
                    }
                }
            }
            if let Some(region) = recast {
                if region.access != view.access
                    || self.program.normalized_type_identity(region.element)
                        != self.program.normalized_type_identity(view.element)
                {
                    return None;
                }
                let collection = canonical_place_from_expression(self.program, region.collection)?;

                let mut authored_loan = collection.clone();
                if let Some(offset) = self.program.expression_table.integer_node(region.start) {
                    authored_loan.segments.push(PlaceSegment::FixedRange {
                        start: usize::try_from(offset.value_i64()?).ok()?,
                        end: usize::try_from(region.extent).ok()?,
                    });
                }
                if authored_loan != place {
                    return None;
                }
                place = collection;
            }
            let declared = canonical_place_type_reference(
                self.program,
                self.state,
                self.statement_index,
                &place,
            )?;
            if slice_view_shell(self.program, declared, &[]).is_some() {
                let (mut source, _, selectors) =
                    self.borrowed_leaf_argument(expression, declared, expression)?;
                source.access = access;
                let selectors = self.retain_reference_selectors(expression, selectors)?;
                return Some(ValueKind::Reference {
                    source,
                    selectors,
                    storage: CheckedReferenceStorage::Declared,
                    backing_type: declared,
                    source_value: CheckedStructuralValueHandle::invalid(),
                    source_contract: checked_trees::CheckedReferenceSourceContract::SeparateStorage,
                });
            }
            if !canonical_place_is_borrowable(&place) {
                return None;
            }
            let symbol = place.root.as_symbol()?;
            let parameter_index = state_parameter_position(self.authored_parameters, symbol)
                .and_then(|authored_position| {
                    self.dense_structural_parameter_index(authored_position)
                });
            let source = match parameter_index {
                Some(parameter_index) => {
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }
                }
                None => CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
            };
            let projection = projected_argument_path(
                self.program,
                self.state,
                self.statement_index,
                &place,
                None,
                None,
            )?;
            let ProjectedArgumentPath {
                storage_type: projected,
                path,
                ..
            } = if slice_view_shell(self.program, projection.storage_type, &[]).is_some() {
                projection
            } else {
                projected_observed_path(
                    self.program,
                    self.machine,
                    self.state,
                    self.statement_index,
                    &place,
                    &|_| None,
                )?
            };
            let type_identity = stored_structural_type_identity(self.program, projected, &[], &[]);
            let lent = lent_collection_element(self.program, projected)?;
            if recast.is_none()
                && authored_recast.is_none()
                && self.program.normalized_type_identity(lent)
                    != self.program.normalized_type_identity(view.element)
            {
                return None;
            }
            let region = if let Some(region) = recast {
                let primitive =
                    super::semantic_casts::result_type(self.program, self.state, region.start)
                        .and_then(|reference| self.program.primitive_type_reference(reference))
                        .or_else(|| {
                            matches!(
                                self.program.expression_table.expression(region.start),
                                ExpressionNode::Integer(_)
                            )
                            .then_some(PrimitiveType::U64)
                        })?;
                let start = self.expression(region.start, primitive)?;
                self.plans.nodes.get_mut(start).authored_root = region.start;
                self.push_scalar_root(
                    CheckedScalarExpressionRole::RepresentationViewOffset { expression },
                    start,
                )?;
                ViewRegion::RepresentationBytes {
                    start,
                    end: region.extent,
                }
            } else {
                ViewRegion::WholeCollection
            };
            let source = CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source,
                path,
                type_identity,
                access,
            };
            Some(if access == CheckedStructuralAccess::SharedBorrow {
                ValueKind::BorrowedSliceView { source, region }
            } else {
                ValueKind::MutableSliceView { source, region }
            })
        }

        /*
            An established descriptor is an ordinary value producer, including a
            literal, call or selected expression. Its backing is not another home
            to rediscover after endpoint evaluation.
        */
        pub(super) fn subslice_view(
            &mut self,
            expression: ExpressionHandle,
            expected: TypeReferenceHandle,
        ) -> Option<ValueKind> {
            use crate::execution::terminal_unit::calls::view_subslice;
            let operation =
                view_subslice::builtin_range_operation(self.program, self.operators, expression)?;
            let machine = self.program.machine_by_symbol(self.machine)?;
            let state = self.program.state_by_symbol(self.state)?;
            let indexed = self.program.expression_table.indexed_node(operation)?;
            if self
                .canonical_place(indexed.collection)
                .is_some_and(|place| place.root.as_symbol().is_some())
            {
                return self.subslice_view_place(expression, expected);
            }
            let target = slice_view_shell(self.program, expected, &[])?;
            let source_type =
                expression_result_type_reference(self.program, machine, state, indexed.collection)
                    .unwrap_or(expected);
            if !indexed_operation_has_builtin_meaning(
                self.program,
                machine,
                Some(state),
                operation,
                Some(source_type),
            ) {
                return None;
            }
            let source_view = slice_view_shell(self.program, source_type, &[])?;
            if source_view.access == language_core::ReferenceAccess::WriteOnly
                || target.access == language_core::ReferenceAccess::WriteOnly
                || (target.access == language_core::ReferenceAccess::Mutable
                    && source_view.access != language_core::ReferenceAccess::Mutable)
                || self.program.normalized_type_identity(target.element)
                    != self.program.normalized_type_identity(source_view.element)
            {
                return None;
            }
            let site = match self
                .program
                .statement_table
                .statement_at(state, self.statement_index)
            {
                Some(StatementNode::LocalData(local)) if local.initial_value == expression => {
                    CheckedSubsliceSite::LocalBinding
                }
                _ => CheckedSubsliceSite::Expression { expression },
            };
            let (start, end, end_inclusive) = view_subslice::retained_endpoints(
                self.program,
                self.pure,
                self.plans,
                self.machine,
                self.state,
                self.statement_index,
                expression,
                site,
            )?;
            let source = self.structural_value(indexed.collection, source_type)?;
            Some(ValueKind::ViewSubslice {
                source,
                start,
                end,
                end_inclusive,
                site,
            })
        }

        /*
            Structural values use the same range admission as call operands and
            local bindings. The source carrier decides whether storage first lends
            a descriptor; endpoint computations do not select another producer.
        */
        pub(super) fn subslice_view_place(
            &self,
            expression: ExpressionHandle,
            expected: TypeReferenceHandle,
        ) -> Option<ValueKind> {
            use crate::execution::terminal_unit::{calls, types::ShapeCollector};
            subslice_expression(self.program, expression)?;
            let machine = self.program.machine_by_symbol(self.machine)?;
            let state = self.program.state_by_symbol(self.state)?;
            let parameters = calls::free_structural_scalar_signature(
                self.program,
                &mut ShapeCollector::with_carrier_index(self.program, self.carriers.clone()),
                state,
                &[],
            )?
            .0;
            let site = match self
                .program
                .statement_table
                .statement_at(state, self.statement_index)
            {
                Some(StatementNode::LocalData(local)) if local.initial_value == expression => {
                    CheckedSubsliceSite::LocalBinding
                }
                _ => CheckedSubsliceSite::Expression { expression },
            };
            let source = calls::view_subslice::admit_resolved(
                self.program,
                self.operators,
                self.pure,
                self.plans,
                self.borrow,
                machine,
                state,
                &parameters,
                expected,
                expression,
                self.statement_index,
                site,
            )?
            .argument();
            Some(match source.access {
                CheckedStructuralAccess::SharedBorrow => ValueKind::BorrowedSliceView {
                    source,
                    region: ViewRegion::WholeCollection,
                },
                CheckedStructuralAccess::MutableBorrow => ValueKind::MutableSliceView {
                    source,
                    region: ViewRegion::WholeCollection,
                },
                _ => return None,
            })
        }
    }

    /*
        The element type of an owned or borrowed contiguous collection place.
        A fixed array declares its extent and a slice carries a stored one; both
        lend the same elements, so both can back a view.
    */
    fn lent_collection_element(
        program: &dyn ::validation::CheckingInput,
        mut type_reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => return Some(*element_type),
                _ => return None,
            }
        }
    }
}
mod structural_fields {

    /*
        Field observation consumes either established storage or a complete evaluated
        receiver. Both keep the same declared projection; a fresh receiver is an
        ordinary structural value dependency, never just its selected initializer.
        A literal element of a fresh receiver's scalar array is observed the same
        way: the receiver has no storage for an indexed read to name, so its
        completed value supplies the element. Established storage keeps its
        indexed-read selection, which also serves runtime selectors.
    */
    use super::{
        Builder, CheckedScalarComputationHandle as ComputationHandle,
        CheckedScalarComputationKind as ComputationKind, ExpressionHandle, ExpressionNode,
    };
    use crate::published::places::place_at;
    use checked_trees::{
        CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan as SourcePlan,
    };

    /*
        A typed scalar observation keeps the complete source carrier and final
        qualified scalar type beside each explicit readable descriptor crossing.
        It is preparation data, not a grant to read or copy the source value.
    */
    pub(super) struct ScalarObservation {
        pub(super) type_reference: typed_trees::types::TypeReferenceHandle,
        pub(super) primitive_type: typed_trees::types::PrimitiveType,
        pub(super) domain: numerics::arithmetic::ArithmeticDomain,
        result_type: typed_trees::types::TypeReferenceHandle,
        path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    }

    impl Builder<'_, '_> {
        fn scalar_observation_type(
            &self,
            source_type: typed_trees::types::TypeReferenceHandle,
        ) -> Option<ScalarObservation> {
            use typed_trees::types::TypeReferenceNode;
            let mut reference = source_type;
            let mut result_type = source_type;
            let mut visited = Vec::new();
            let mut path = Vec::new();
            loop {
                if !self
                    .program
                    .type_reference_table
                    .contains_type_reference(reference)
                    || visited.contains(&reference)
                {
                    return None;
                }
                visited.push(reference);
                if validation::stored_view_shell(self.program, reference, &[]).is_some() {
                    return None;
                }
                match self.program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::Reference {
                        referee, access, ..
                    } if access.is_readable() => {
                        path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
                        result_type = *referee;
                        reference = *referee;
                    }
                    TypeReferenceNode::Reference { .. } => return None,
                    _ => {
                        let primitive_type = self.program.primitive_type_reference(reference)?;
                        return Some(ScalarObservation {
                            type_reference: source_type,
                            result_type,
                            primitive_type,
                            domain: self
                                .program
                                .arithmetic_domain_for_type_reference(result_type),
                            path,
                        });
                    }
                }
            }
        }

        fn publish_scalar_observation(
            &mut self,
            expression: ExpressionHandle,
            receiver_value: checked_trees::CheckedStructuralValueHandle,
            observation: ScalarObservation,
        ) -> ComputationHandle {
            self.insert(
                observation.primitive_type,
                ComputationKind::StructuralField {
                    atomic_ordering: None,
                    result_type: observation.result_type,
                    source_expression: expression,
                    receiver_expression: expression,
                    source_type: observation.type_reference,
                    receiver_value,
                    subject: CheckedUnitStructuralArgumentPlan {
                        projection_evaluations: Default::default(),
                        source: SourcePlan::StructuralExpression {
                            value_dependency: None,
                            expression,
                            logical_root_type_identity: None,
                        },
                        path: observation.path,
                        type_identity: validation::stored_structural_type_identity(
                            self.program,
                            observation.type_reference,
                            &[],
                            &[],
                        ),
                        access: CheckedStructuralAccess::SharedBorrow,
                    },
                    field: symbols::SymbolHandle::invalid(),
                    field_identity: Default::default(),
                },
            )
        }

        /*
            Read one scalar leaf `row[indices]` a runtime subvalue row contributes
            to a scalar array construction. The row's canonical place supplies the
            receiver; the leaf is an ordinary storage observation at its declared
            carrier, never a move or re-evaluation of the row's owner.
        */
        pub(super) fn projected_element_scalar(
            &mut self,
            row: ExpressionHandle,
            indices: &[u32],
            leaf_type: typed_trees::types::TypeReferenceHandle,
            primitive_type: typed_trees::types::PrimitiveType,
        ) -> Option<ComputationHandle> {
            let coordinate = self.canonical_place(row)?;
            let facts::PlaceRoot::Symbol(symbol) = coordinate.root else {
                return None;
            };
            let mut segments = coordinate.segments.clone();
            for index in indices {
                segments.push(facts::PlaceSegment::FixedIndex {
                    index: usize::try_from(*index).ok()?,
                });
            }
            let field = validation::structural_scalar_field_at_element_coordinate(
                self.program,
                self.state,
                symbol,
                &segments,
            )?;
            if field.primitive_type != primitive_type
                || self
                    .program
                    .normalized_type_identity(field.result_type_reference)
                    != self.program.normalized_type_identity(leaf_type)
            {
                return None;
            }
            self.structural_field(row, field)
        }

        /*
            A projected element leaf records at the same `ArrayElement` coordinate
            an authored leaf would, so the result consumer reads one vocabulary.
        */
        pub(super) fn record_projected_element_root(
            &mut self,
            statement_ordinal: u32,
            role: checked_trees::CheckedScalarExpressionRole,
            row: ExpressionHandle,
            indices: &[u32],
            leaf_type: typed_trees::types::TypeReferenceHandle,
            primitive_type: typed_trees::types::PrimitiveType,
        ) {
            if self
                .plans
                .root_at(self.state, statement_ordinal, role)
                .is_some()
            {
                return;
            }
            if let Some(root) =
                self.projected_element_scalar(row, indices, leaf_type, primitive_type)
            {
                self.plans.nodes.get_mut(root).authored_root = row;
                self.plans
                    .roots
                    .append(checked_trees::CheckedScalarComputationRoot {
                        machine: self.machine,
                        state: self.state,
                        statement_ordinal,
                        role,
                        root,
                    });
            }
        }

        /*
            Read the scalar endpoint of an ordinary completed value projection.
            The projection owns selection from the saved descriptor, including any
            subsequent live reference crossings. It never re-reads the old carrier.
        */
        pub(super) fn completed_scalar_observation(
            &mut self,
            expression: ExpressionHandle,
            expected: typed_trees::types::PrimitiveType,
        ) -> Option<ComputationHandle> {
            let reference = self.expression_type(expression)?;
            let observation = self.scalar_observation_type(reference)?;
            if observation.primitive_type != expected {
                return None;
            }
            let receiver_value = self.retain_structural_argument(expression, reference)?;
            Some(self.publish_scalar_observation(expression, receiver_value, observation))
        }

        /*
            A scalar field or tag read finishes its fresh reference receiver before a later
            operand can write the backing. The captured scalar is independent; the
            descriptor is not an owned referent or a receiver kept until scope exit.
            Nominal temporary owners retain their separately checked normal death.
        */
        pub(super) fn observation_completion_actions(
            &mut self,
            kind: &ComputationKind,
        ) -> arena::HandleSpan<checked_trees::CheckedNormalCleanupAction> {
            let receiver_value = kind.observed_receiver();
            if !receiver_value.is_valid() {
                return arena::HandleSpan::empty();
            }
            let receiver = self.structural_values.nodes.get(receiver_value);
            if !self
                .canonical_place(receiver.expression)
                .is_some_and(|place| {
                    place.root == facts::PlaceRoot::Expression(receiver.expression)
                        && place.segments.is_empty()
                })
            {
                return arena::HandleSpan::empty();
            }
            if !validation::reference_shell(self.program, receiver.type_reference)
                .is_some_and(|shell| shell.access.is_readable())
            {
                return arena::HandleSpan::empty();
            }
            let source = SourcePlan::StructuralExpression {
                value_dependency: None,
                expression: receiver.expression,
                logical_root_type_identity: None,
            };
            let Some(actions) = crate::execution::terminal_unit::cleanup::descriptor_retirement(
                self.program,
                source,
                receiver.type_reference,
            ) else {
                return arena::HandleSpan::empty();
            };
            self.plans.temporary_cleanup_actions.insert_many(actions)
        }

        /*
            A computed reference first produces its permission carrier; a scalar
            receiving position then observes that carrier's referent. This is the
            same explicit storage read as a field projection, not a scalar ABI for
            the producer and not an equation to its possible ingress origins.
        */
        pub(super) fn primitive_reference_observation(
            &mut self,
            expression: ExpressionHandle,
            expected: typed_trees::types::PrimitiveType,
        ) -> Option<ComputationHandle> {
            let observation = self.primitive_reference_observation_type(expression)?;
            if observation.primitive_type != expected {
                return None;
            }
            let reference = observation.type_reference;
            let receiver_value = self.structural_value(expression, reference)?;
            let statement = u32::try_from(self.statement_index).ok()?;
            self.retain_structural_root(statement, expression, reference, receiver_value);
            Some(self.publish_scalar_observation(expression, receiver_value, observation))
        }

        /*
            Receiving positions and arithmetic operands ask the same type question.
            The reference's declared referee supplies its scalar carrier and policy;
            neither a surrounding operator nor the return ABI may invent them.
        */
        pub(super) fn primitive_reference_observation_type(
            &self,
            expression: ExpressionHandle,
        ) -> Option<ScalarObservation> {
            if self
                .canonical_place(expression)
                .is_some_and(|place| place.root.as_symbol().is_some())
            {
                return None;
            }
            let reference = self.expression_type(expression)?;
            let observation = self.scalar_observation_type(reference)?;
            (!observation.path.is_empty()).then_some(observation)
        }

        pub(super) fn structural_scalar_field(
            &self,
            expression: ExpressionHandle,
        ) -> Option<validation::StructuralScalarField> {
            let element = match self.program.expression_table.expression(expression) {
                ExpressionNode::Member(_) => false,
                ExpressionNode::Indexed(_) => true,
                _ => return None,
            };
            if element {
                return self
                    .structural_receiver_scalar(expression)
                    .filter(|field| field.receiver_value.is_valid());
            }
            if !self.declaration_parameters.is_empty() {
                return validation::structural_scalar_field_in_parameters(
                    self.program,
                    self.declaration_parameters,
                    self.declaration_bindings,
                    expression,
                );
            }
            self.captured_case_scalar_field(expression)
                .or_else(|| self.structural_receiver_scalar(expression))
        }

        /*
            Pattern-bound copies belong to the selected subject's already captured
            scalar payload namespace. Rejoin its exact completed descriptor rather
            than retaining another read of a dynamic source projection. Psi 06's
            ordinary case payload bindings supply these leaves before all target
            operands; arithmetic and calls still use their shared computation graph.
        */
        fn captured_case_scalar_field(
            &self,
            expression: ExpressionHandle,
        ) -> Option<validation::StructuralScalarField> {
            let machine = self.program.machine_by_symbol(self.machine)?;
            let state = self.program.state_in_machine(machine, self.state)?;
            let observations = crate::flow::CapturedSubjectFacts {
                semantic: self.semantic,
                flow: self.flow,
                borrow: self.borrow,
            };
            let transition = self
                .program
                .statement_table
                .statement_at(state, self.statement_index)?
                .as_transition()?;
            let (_, copy) = crate::flow::copied_payload_at(
                self.program,
                self.flow,
                self.machine,
                self.state,
                self.statement_index,
                checked_trees::FlowPermissionEvaluation::TransitionTarget(transition.target),
                expression,
            )?;
            if self
                .program
                .primitive_type_reference(copy.type_reference)
                .is_none()
                || validation::reference_shell(self.program, copy.type_reference).is_some()
            {
                return None;
            }
            let flow_state = self.flow.control.unique_state(self.machine, self.state)?;
            let statement = self
                .flow
                .state_statement(flow_state, self.statement_index)?;
            let mut selected = None;
            for observation in self
                .flow
                .control
                .value_observations
                .span_or_empty(statement.value_observations)
            {
                let Some(subject) = crate::flow::captured_case_subject(
                    self.program,
                    observations,
                    machine,
                    state,
                    self.statement_index,
                    observation.expression,
                ) else {
                    continue;
                };
                if subject.receiver != copy.backing {
                    continue;
                }
                let captured = self.flow.control.operands.get(subject.source);
                let source_place = place_at(
                    self.program,
                    captured.state_symbol,
                    captured.statement_index,
                    captured.expression,
                )?;
                let mut receiver = expression;
                loop {
                    receiver = match self.program.expression_table.expression(receiver) {
                        ExpressionNode::Member(member) => member.receiver,
                        ExpressionNode::Indexed(indexed) => indexed.collection,
                        _ => break,
                    };
                    let Some(place) = self.canonical_place(receiver) else {
                        continue;
                    };
                    if !crate::flow::canonical_places_agree(self.program, &place, &source_place) {
                        continue;
                    }
                    let source = validation::structural_source(
                        self.program,
                        self.machine,
                        self.state,
                        u32::try_from(self.statement_index).ok()?,
                        expression,
                        subject.observed,
                    )?;
                    let field = validation::structural_scalar_field_of_evaluated_receiver(
                        self.program,
                        self.machine,
                        self.state,
                        expression,
                        receiver,
                        source,
                    )?;
                    if selected.as_ref().is_some_and(|previous| previous != &field) {
                        return None;
                    }
                    selected = Some(field);
                    break;
                }
            }
            selected
        }

        fn structural_receiver_scalar(
            &self,
            expression: ExpressionHandle,
        ) -> Option<validation::StructuralScalarField> {
            validation::structural_scalar_field(
                self.program,
                self.machine,
                self.state,
                expression,
                &[],
                |receiver| {
                    validation::structural_source(
                        self.program,
                        self.machine,
                        self.state,
                        u32::try_from(self.statement_index).ok()?,
                        expression,
                        receiver,
                    )
                },
            )
        }

        pub(super) fn structural_field(
            &mut self,
            expression: ExpressionHandle,
            field: validation::StructuralScalarField,
        ) -> Option<ComputationHandle> {
            if let Some(literal) = closed_descriptor_value(self.program, &field) {
                return Some(self.insert(
                    field.primitive_type,
                    ComputationKind::Value(
                        checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
                    ),
                ));
            }
            if let Some(symbol) = descriptor_parameter_symbol(self.program, &field)
                && let Some(position) = self
                    .parameters
                    .iter()
                    .position(|parameter| parameter.symbol == symbol)
                && self.parameter_types.get(position) == Some(&field.primitive_type)
            {
                return Some(self.insert(
                    field.primitive_type,
                    ComputationKind::Value(checked_trees::CheckedScalarExpression::Parameter {
                        position,
                        primitive_type: field.primitive_type,
                    }),
                ));
            }
            let receiver_value = if let Some(captured) = self.captured_dispatch_value(expression)
                && captured.type_reference == field.type_reference
                && field.root == facts::PlaceRoot::Expression(captured.expression)
            {
                captured.root
            } else if let Some(receiver) = field.root.as_expression().and_then(|anchor| {
                self.dispatch_receivers
                    .get(&(anchor.arena_index(), anchor.generation()))
            }) {
                // The dispatch has already chosen a completed value or an existing
                // place. A field read consumes that exact role, not its supplying
                // expression, which could move an affine field or replay a call.
                if !validation::declared_carriers_match(
                    self.program,
                    receiver.type_reference,
                    field.type_reference,
                ) {
                    return None;
                }
                receiver.value
            } else if field.receiver_value.is_valid() {
                if field.root.as_expression().is_some_and(|anchor| {
                matches!(self.program.expression_table.expression(anchor), ExpressionNode::SavedSubject(saved)
                    if matches!(self.program.expression_table.value_capture(saved.capture).owner,
                        typed_trees::expression::ValueCaptureOwner::Match))
            }) {
                // An unproduced Match alias cannot recover a receiver by
                // evaluating the original subject at this later observation.
                return None;
            }
                let value = self.structural_value(field.receiver_value, field.type_reference)?;
                self.retain_structural_value_root(value, field.type_reference)?;
                value
            } else {
                checked_trees::CheckedStructuralValueHandle::invalid()
            };
            let source = match field.root {
                facts::PlaceRoot::Symbol(symbol) => self.structural_source(symbol),
                facts::PlaceRoot::Expression(expression) => SourcePlan::StructuralExpression {
                    value_dependency: None,
                    expression,
                    logical_root_type_identity: None,
                },
                _ => return None,
            };
            Some(self.insert(
                field.primitive_type,
                ComputationKind::StructuralField {
                    atomic_ordering: None,
                    result_type: field.result_type_reference,
                    source_expression: expression,
                    receiver_expression: if expression.is_valid() {
                        match self.program.expression_table.expression(expression) {
                            ExpressionNode::Member(member) => self
                                .program
                                .saved_argument_source(member.receiver)
                                .unwrap_or(member.receiver),
                            ExpressionNode::Indexed(indexed) => indexed.collection,
                            _ => ExpressionHandle::invalid(),
                        }
                    } else {
                        ExpressionHandle::invalid()
                    },
                    source_type: field.type_reference,
                    receiver_value,
                    subject: CheckedUnitStructuralArgumentPlan {
                        projection_evaluations: Default::default(),
                        source,
                        path: field.path,
                        type_identity: validation::structural_type_identity(
                            self.program,
                            field.carrier_type_reference,
                            &[],
                            &[],
                        ),
                        access: CheckedStructuralAccess::SharedBorrow,
                    },
                    field: field.field,
                    field_identity: field.field_identity,
                },
            ))
        }
    }

    /*
        The closed integer a value-parameter descriptor denotes. A saved expression
        and a saved local are the same observation: erasure leaves no runtime place,
        and the application's integer operand is the field. A path below that
        descriptor is a different observation and keeps the ordinary borrow.
    */
    fn closed_descriptor_value(
        program: &dyn ::validation::CheckingInput,
        field: &validation::StructuralScalarField,
    ) -> Option<numerics::literals::IntegerLiteral> {
        if !field.path.is_empty()
            || !matches!(
                field.root,
                facts::PlaceRoot::Expression(_) | facts::PlaceRoot::Symbol(_)
            )
        {
            return None;
        }
        let (_, _, index) = descriptor_fields(program, field.type_reference)
            .into_iter()
            .find(|(_, descriptor, _)| descriptor.symbol == field.field)?;
        let literal = program
            .typed_program()
            .static_const_literal_from_type_reference(index)?;
        let landed_type = field.primitive_type.landed_landing()?;
        Some(literal.with_landing(numerics::literals::IntegerLanding {
            landed_type,
            domain: field.domain,
        }))
    }

    /*
        A descriptor whose application operand is a runtime value parameter. The
        parameter place saves that operand as the field; other places save a
        closed integer literal.
    */
    fn descriptor_parameter_symbol(
        program: &dyn ::validation::CheckingInput,
        field: &validation::StructuralScalarField,
    ) -> Option<symbols::SymbolHandle> {
        if !field.path.is_empty()
            || !matches!(
                field.root,
                facts::PlaceRoot::Expression(_) | facts::PlaceRoot::Symbol(_)
            )
        {
            return None;
        }
        let (_, _, index) = descriptor_fields(program, field.type_reference)
            .into_iter()
            .find(|(_, descriptor, _)| descriptor.symbol == field.field)?;
        match program.type_reference_table.type_reference(index) {
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                Some(*symbol)
            }
            _ => None,
        }
    }

    /*
        Value-parameter descriptors of the carrier, after the reference shells a
        borrowed local spells in front of the data application.
    */
    fn descriptor_fields(
        program: &dyn ::validation::CheckingInput,
        mut reference: typed_trees::types::TypeReferenceHandle,
    ) -> Vec<(
        &typed_trees::data::TypeParameter,
        &typed_trees::data::DataField,
        typed_trees::types::TypeReferenceHandle,
    )> {
        let mut visited = 0u32;
        loop {
            if visited > 8 {
                break;
            }
            visited += 1;
            let Some(shell) = validation::reference_shell(program, reference) else {
                break;
            };
            reference = shell.referee;
        }
        program.data_application_value_fields(reference)
    }
}
mod structural_values {

    /*
        Fresh structural establishment shares the scalar operand evaluation owner.
        Reference leaves are classified before referent-oriented normalization:
        their value is borrowed-storage custody, never a scalar pointer snapshot.
        Literals, returned values, and whole transfers share the construction-contents rule:
        exact interval-qualified scalar members use the existing bounded carrier
        and per-member establishment proof, not a constructor-specific exception.
        Closed primitive arrays select their scalar-array producer at the value
        boundary, independent of whether a record, case, or other value consumes it.
        A match is not that producer: it selects one array, and only the chosen
        arm may run. Literal constructors still own the row-major leaf roster.
        Copy permission belongs to the selected leaf, not its root: a readable
        owned or borrowed root may supply an unrestricted child without moving it.
        Exact source/path replay and ordinary liveness remain downstream checks.
    */
    use super::{
        Builder, CheckedScalarExpressionRole, ExpressionHandle, ExpressionNode, SymbolHandle,
        TypeReferenceNode,
    };
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::calls::byte_sequence_literal_argument;
    use crate::execution::terminal_unit::calls::projected_argument_path;
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use crate::published::semantic::calls::shared_argument_referent;
    use crate::values::authored_projection_selectors;
    use crate::values::operator_is_builtin;
    use crate::values::reference_sources::capture_source;
    use crate::values::reference_sources::formed_reference_type;
    use crate::values::reference_sources::transported_value_type;
    use checked_trees::{
        CheckedScalarComputationHandle, CheckedStructuralDispatchArm, CheckedStructuralValue,
        CheckedStructuralValueHandle as ValueHandle, CheckedStructuralValueKind as ValueKind,
    };
    use typed_trees::types::TypeReferenceHandle;
    use value_predicates::argument_needs_establishment;

    /* One authored selector and the scalar computation that evaluates it. */
    pub(super) type ProjectionSelector = (ExpressionHandle, CheckedScalarComputationHandle);

    mod family_values {
        /*
            Structural value production by family: fixed and scalar arrays, records,
            and case constructions each establish their own contents.
        */
        use super::super::{
            Builder, CheckedScalarExpressionRole, ExpressionHandle, ExpressionNode,
            TypeReferenceNode,
        };
        use super::value_predicates::structural_carriers_match;
        use super::value_predicates::{zeroed_integer_literal, zeroed_scalar_array_parts};
        use checked_trees::CheckedStructuralValueKind;
        use typed_trees::types::TypeReferenceHandle;

        impl Builder<'_, '_> {
            /*
                Element-wise array literal establishment: each element registers as
                its own structural value under the declared element type, exactly as
                record fields do inside a constructor.
            */
            pub(super) fn fixed_array_value(
                &mut self,
                expected: TypeReferenceHandle,
                literal: arena::HandleSpan<ExpressionHandle>,
            ) -> Option<CheckedStructuralValueKind> {
                if validation::scalar_array_preserves_memberships(self.program, expected) {
                    return None;
                }
                let reference = validation::unwrapped_type_reference(self.program, expected)?;
                let TypeReferenceNode::FixedArray { element_type, .. } =
                    self.program.type_reference_table.type_reference(reference)
                else {
                    return None;
                };
                let mut elements = Vec::new();
                for element in self.program.expression_table.expression_handles(literal) {
                    elements.push(self.structural_value(*element, *element_type)?);
                }
                Some(CheckedStructuralValueKind::FixedArray { elements })
            }

            /*
                A closed primitive array supplied as one member of a structural value.
                The ordinary scalar-array producer flattens it to its row-major leaves;
                each leaf evaluates in authored order as its own scalar computation,
                keyed by this array expression the way record fields key by their
                literal. Lowering establishes the whole roster as one scalar array, so
                the array contributes no element custody of its own. Constant-array
                projections retain their exact builtin indexing selection.
            */
            pub(super) fn scalar_array_value(
                &mut self,
                expression: ExpressionHandle,
                expected: TypeReferenceHandle,
            ) -> Option<CheckedStructuralValueKind> {
                let array = validation::scalar_array_elements(
                    self.program,
                    self.machine,
                    expression,
                    expected,
                )?;
                if array.projections.iter().any(|projection| {
                    self.operators
                        .expression_use(*projection)
                        .is_some_and(|selected| {
                            selected.spelling != language_core::OperatorSpelling::Index
                                || selected.selected_operator_symbol.is_valid()
                                || selected.candidate_count != 0
                                || !matches!(
                                    selected.status,
                                    checked_trees::CheckedOperatorResolutionStatus::Missing
                                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                                )
                        })
                }) {
                    return None;
                }
                let mut elements = Vec::with_capacity(array.elements.len());
                for (element_ordinal, element) in array.elements.into_iter().enumerate() {
                    let element_ordinal = u32::try_from(element_ordinal).ok()?;
                    let root = match element {
                        validation::ScalarArrayElement::Authored {
                            expression: leaf,
                            primitive_type,
                        } => {
                            let root = self.expression(leaf, primitive_type)?;
                            self.plans.nodes.get_mut(root).authored_root = leaf;
                            root
                        }
                        validation::ScalarArrayElement::Projected {
                            row,
                            indices,
                            leaf_type,
                            primitive_type,
                        } => {
                            self.projected_element_scalar(row, &indices, leaf_type, primitive_type)?
                        }
                    };
                    self.push_scalar_root(
                        CheckedScalarExpressionRole::StructuralArrayElement {
                            expression,
                            element_ordinal,
                        },
                        root,
                    )?;
                    elements.push(root);
                }
                Some(CheckedStructuralValueKind::ScalarArray { elements })
            }

            pub(super) fn record_value(
                &mut self,
                expression: ExpressionHandle,
                expected: TypeReferenceHandle,
            ) -> Option<CheckedStructuralValueKind> {
                let ExpressionNode::StructLiteral(literal) =
                    self.program.expression_table.expression(expression).clone()
                else {
                    return None;
                };
                let reference = validation::unwrapped_type_reference(self.program, expected)?;
                let data = self
                    .program
                    .closed_data_constructor_definition(reference, literal.type_symbol)?;
                if literal.case_name.is_some() {
                    return None;
                }
                let data_symbol = data.symbol;
                let declared = self.program.data_members(data);
                if declared
                    .iter()
                    .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
                {
                    return None;
                }
                let mut authored = self
                    .program
                    .expression_table
                    .struct_fields(literal.fields)
                    .to_vec();
                for initializer in &mut authored {
                    initializer.field_symbol = self
                        .program
                        .closed_data_constructor_field(
                            reference,
                            literal.type_symbol,
                            initializer.field_symbol,
                        )?
                        .symbol;
                }
                let relevant = self.program.live_data_fields(declared).collect::<Vec<_>>();
                let erased = declared
                    .iter()
                    .filter_map(|member| match member {
                        typed_trees::data::DataMember::Field(field)
                            if field.relevance.is_erased() =>
                        {
                            Some(field)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let omitted = relevant
                    .iter()
                    .filter(|field| {
                        authored
                            .iter()
                            .all(|initializer| initializer.field_symbol != field.symbol)
                    })
                    .copied()
                    .collect::<Vec<_>>();
                if authored.len() != relevant.len() + erased.len()
                    && (authored.len() > relevant.len() + erased.len()
                        || authored.iter().any(|initializer| {
                            declared.iter().all(|member| match member {
                                typed_trees::data::DataMember::Field(field) => {
                                    field.symbol != initializer.field_symbol
                                }
                                _ => true,
                            })
                        })
                        || omitted.iter().any(|field| {
                            zeroed_scalar_array_parts(self.program, field.type_reference).is_none()
                                && !validation::zeroed_structural_parts(
                                    self.program,
                                    field.type_reference,
                                )
                        }))
                {
                    return None;
                }
                let mut fields = Vec::new();
                for (ordinal, initializer) in authored.into_iter().enumerate() {
                    let erased_field = erased
                        .iter()
                        .find(|field| field.symbol == initializer.field_symbol);
                    let field = if let Some(field) = erased_field {
                        *field
                    } else {
                        *relevant
                            .iter()
                            .find(|field| field.symbol == initializer.field_symbol)?
                    };
                    if erased_field.is_some() {
                        let value = checked_trees::CheckedStructuralRecordFieldValue::Structural(
                            self.structural_value(initializer.value, field.type_reference)?,
                        );
                        fields.push(checked_trees::CheckedStructuralRecordField {
                            field: field.symbol,
                            expression: initializer.value,
                            type_reference: field.type_reference,
                            value,
                        });
                        continue;
                    }
                    if fields
                        .iter()
                        .any(|prior: &checked_trees::CheckedStructuralRecordField| {
                            prior.field == field.symbol
                        })
                    {
                        return None;
                    }
                    let declared = field.type_reference;
                    let value =
                        if let Some(primitive) = self.program.primitive_type_reference(declared) {
                            let root = self.expression(initializer.value, primitive)?;
                            self.plans.nodes.get_mut(root).authored_root = initializer.value;
                            self.push_scalar_root(
                                CheckedScalarExpressionRole::RecordField {
                                    expression,
                                    field_ordinal: u32::try_from(ordinal).ok()?,
                                },
                                root,
                            )?;
                            checked_trees::CheckedStructuralRecordFieldValue::Scalar(root)
                        } else {
                            checked_trees::CheckedStructuralRecordFieldValue::Structural(
                                self.structural_value(initializer.value, declared)?,
                            )
                        };
                    fields.push(checked_trees::CheckedStructuralRecordField {
                        field: field.symbol,
                        expression: initializer.value,
                        type_reference: declared,
                        value,
                    });
                }
                for field in omitted {
                    fields.push(self.zeroed_constructor_field(
                        expression,
                        field.symbol,
                        field.type_reference,
                    )?);
                }
                Some(CheckedStructuralValueKind::Record {
                    data_symbol,
                    fields: self.structural_values.record_fields.insert_many(fields),
                })
            }

            /*
                Retain an omitted member at its construction occurrence, after authored
                operands. Record and case common members share this storage producer;
                source checking still owns default-domain and zero-validity judgments.
            */
            fn zeroed_constructor_field(
                &mut self,
                expression: ExpressionHandle,
                field: symbols::SymbolHandle,
                declared: TypeReferenceHandle,
            ) -> Option<checked_trees::CheckedStructuralRecordField> {
                if zeroed_scalar_array_parts(self.program, declared).is_none()
                    && !validation::zeroed_structural_parts(self.program, declared)
                {
                    return None;
                }
                let node = if zeroed_scalar_array_parts(self.program, declared).is_none() {
                    self.publish_structural_value(
                        expression,
                        declared,
                        CheckedStructuralValueKind::ZeroedStructural {
                            projected_qualifications:
                                validation::zeroed_byte_projected_qualifications(
                                    self.program,
                                    declared,
                                )
                                .ok()?,
                        },
                    )?
                } else {
                    let (primitive_type, element_count) =
                        zeroed_scalar_array_parts(self.program, declared)?;
                    let zero = zeroed_integer_literal(primitive_type)?;
                    let element =
                        self.plans
                            .nodes
                            .append(checked_trees::CheckedScalarComputation {
                                authored_root: ExpressionHandle::invalid(),
                                value_source: ExpressionHandle::invalid(),
                                primitive_type,
                                normal_cleanup_actions: arena::HandleSpan::empty(),
                                kind: checked_trees::CheckedScalarComputationKind::Value(zero),
                            });
                    self.publish_structural_value(
                        expression,
                        declared,
                        CheckedStructuralValueKind::ZeroedScalarArray {
                            element,
                            element_count,
                        },
                    )?
                };
                Some(checked_trees::CheckedStructuralRecordField {
                    field,
                    expression,
                    type_reference: declared,
                    value: checked_trees::CheckedStructuralRecordFieldValue::Structural(node),
                })
            }

            /*
                A case literal composes its payload fields exactly like a record's:
                authored initializers pair against the selected case's declarations in
                authored order, and each operand is a scalar computation or a nested
                structural value. The case tag stays with the construction, so the
                value keeps its discriminated identity a record cannot spell.
            */
            pub(super) fn structural_case_value(
                &mut self,
                expression: ExpressionHandle,
                expected: TypeReferenceHandle,
            ) -> Option<CheckedStructuralValueKind> {
                let constructor =
                    validation::structural_case_constructor(self.program, expression, expected)?;
                let data_symbol = self
                    .program
                    .closed_data_definition(constructor.type_reference)?
                    .symbol;
                if !structural_carriers_match(self.program, constructor.type_reference, expected) {
                    return None;
                }
                let mut fields = Vec::with_capacity(constructor.fields.len());
                for (ordinal, (symbol, value_expression, declared)) in
                    constructor.fields.into_iter().enumerate()
                {
                    let value =
                        if let Some(primitive) = self.program.primitive_type_reference(declared) {
                            let root = self.expression(value_expression, primitive)?;
                            self.plans.nodes.get_mut(root).authored_root = value_expression;
                            self.push_scalar_root(
                                CheckedScalarExpressionRole::StructuralValueField {
                                    expression,
                                    field_ordinal: u32::try_from(ordinal).ok()?,
                                },
                                root,
                            )?;
                            checked_trees::CheckedStructuralRecordFieldValue::Scalar(root)
                        } else {
                            checked_trees::CheckedStructuralRecordFieldValue::Structural(
                                self.structural_value(value_expression, declared)?,
                            )
                        };
                    fields.push(checked_trees::CheckedStructuralRecordField {
                        field: symbol,
                        expression: value_expression,
                        type_reference: declared,
                        value,
                    });
                }
                for (field, declared) in constructor.omitted_fields {
                    fields.push(self.zeroed_constructor_field(expression, field, declared)?);
                }
                Some(CheckedStructuralValueKind::StructuralCase {
                    data_symbol,
                    case: constructor.case,
                    fields: self.structural_values.record_fields.insert_many(fields),
                })
            }
        }
    }
    mod place_types {

        /*
            Place-rooted leaf types and copy/borrow custody for structural
            establishment. A canonical place's borrowability and the leaf type a
            projected path selects are decided here, ahead of value production.
            Source use multiplicity is not stored-carrier custody: a reference array
            can be reusable source syntax while its descriptors must transfer or retire.
            Copy selection uses the declared Copy property; result publication and
            retirement separately retain each stored carrier's custody.
        */
        use super::super::{ExpressionHandle, ExpressionNode, SymbolHandle};
        use crate::published::places::place_at;
        use typed_trees::types::TypeReferenceHandle;

        /*
            The place shape a shared-borrow arm target must canonicalize to: a named
            root reached through record fields and literal fixed indexes only. This is
            the shape static type queries can resolve without an evaluation owner.
            Computed roots and dynamic selectors instead retain their producers in
            the structural value graph through the common reference-source capture.
        */
        pub(in super::super) fn canonical_place_is_borrowable(
            place: &validation::CanonicalPlace,
        ) -> bool {
            matches!(place.root, facts::PlaceRoot::Symbol(_))
                && place.segments.iter().all(|segment| {
                    matches!(
                        segment,
                        facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
                    )
                })
        }

        /// A copy reads named storage or an authenticated saved receiver. Its path
        /// can select a case payload or an evaluated array element; ordinary checking
        /// still proves the case, bounds, copy permission and access. This list of
        /// storage coordinates establishes none of those facts.
        pub(super) fn canonical_place_is_copyable(place: &validation::CanonicalPlace) -> bool {
            matches!(
                place.root,
                facts::PlaceRoot::Symbol(_) | facts::PlaceRoot::Expression(_)
            ) && place.segments.iter().all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. }
                        | facts::PlaceSegment::Case { .. }
                        | facts::PlaceSegment::FixedIndex { .. }
                        | facts::PlaceSegment::Index { .. }
                )
            })
        }

        /*
            Copy permission describes the stored carrier, not source-use multiplicity.
            An exclusive reference may be used repeatedly under loan checking, but it
            cannot be duplicated, including inside an array. Use the property owner's
            compositional judgment so projections of non-copy aggregates keep the
            ordinary owned-place route and its unselected siblings.
        */
        pub(super) fn copied_place_type(
            program: &dyn ::validation::CheckingInput,
            expected: TypeReferenceHandle,
        ) -> bool {
            validation::type_satisfies_declared_property(
                program,
                &[],
                expected,
                validation::DeclaredPropertyRequirement::Copy,
            )
        }

        /*
            The selected type and endpoint crossings of one readable place. Keeping
            them together prevents admitting a referent copy while emitting only its
            descriptor address. Interior and local-root crossings belong to the path
            producer; these crossings select below its final stored carrier.
        */
        pub(super) struct ReadablePlace {
            pub(super) reference: TypeReferenceHandle,
            pub(super) referent_crossings: usize,
        }

        /*
            Present the exact requested carrier from an authored readable place.
            Matching descriptors remain descriptors. A borrowed formal already denotes
            its first referent; stored references instead retain explicit crossings.
        */
        pub(super) fn readable_place(
            program: &dyn ::validation::CheckingInput,
            state: SymbolHandle,
            statement_index: usize,
            place: &validation::CanonicalPlace,
            expected: TypeReferenceHandle,
        ) -> Option<ReadablePlace> {
            let mut reference =
                validation::canonical_place_type_reference(program, state, statement_index, place)?;
            if validation::declared_carriers_match(program, reference, expected) {
                return Some(ReadablePlace {
                    reference,
                    referent_crossings: 0,
                });
            }
            let state = program.state_by_symbol(state)?;
            if place.segments.is_empty()
                && let Some(symbol) = place.root.as_symbol()
                && program.state_parameter_by_symbol(state, symbol).is_some()
            {
                let shell = validation::reference_shell(program, reference)?;
                if !shell.access.is_readable() {
                    return None;
                }
                reference = shell.referee;
            }
            for referent_crossings in 0..64 {
                if validation::declared_carriers_match(program, reference, expected) {
                    return Some(ReadablePlace {
                        reference,
                        referent_crossings,
                    });
                }
                let shell = validation::reference_shell(program, reference)?;
                if !shell.access.is_readable()
                    || !validation::stores_reference_shape(program, reference, &[])
                {
                    return None;
                }
                reference = shell.referee;
            }
            None
        }

        /*
            A member/index projection rooted at readable owned or borrowed storage
            whose leaf is a copyable `Unrestricted` structural type.
            It shares the scalar-case read's root/path contract, but retains contents
            that cannot be reconstructed by case fan-out. A bare
            whole-root name is the degenerate path: an owned root's `Unrestricted`
            contents copy out whole, no loan involved.
            Retain that snapshot at the operand's evaluation point; passing an earlier
            constructor's result would confuse mutable storage with its initial value.
        */
        pub(in super::super) fn needs_copied_place_root(
            program: &dyn ::validation::CheckingInput,
            state: SymbolHandle,
            statement_index: usize,
            expression: ExpressionHandle,
            expected: TypeReferenceHandle,
            existing_capture: impl FnOnce(ExpressionHandle) -> bool,
        ) -> bool {
            if !copied_place_type(program, expected) {
                return false;
            }
            let Some(place) = place_at(program, state, statement_index, expression) else {
                return false;
            };
            if !canonical_place_is_copyable(&place) {
                return false;
            }
            if let Some(anchor) = place.root.as_expression() {
                // The saved receiver was captured before its patterns. If its field
                // needs a copy, failure to prepare that copy cannot donate ownership
                // of the borrowed receiver to the ordinary owned-value route.
                return existing_capture(anchor);
            }
            let Some(root) = validation::canonical_place_type_reference(
                program,
                state,
                statement_index,
                &validation::CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                },
            ) else {
                return false;
            };
            if !place.segments.is_empty()
                && validation::reference_shell(program, root).is_some_and(|shell| {
                    shell.access == language_semantics::ReferenceAccess::WriteOnly
                })
            {
                return false;
            }
            readable_place(program, state, statement_index, &place, expected)
                .is_some_and(|leaf| copied_place_type(program, leaf.reference))
        }

        /*
            The terminal expression carrying a projection's storage: the whole local
            name or anonymous producer. Selecting storage does not restrict how its
            owner was produced; ordinary structural value planning admits that owner.
        */
        pub(super) fn projection_root_expression(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> ExpressionHandle {
            let mut cursor = expression;
            loop {
                match program.expression_table.expression(cursor) {
                    ExpressionNode::Member(member) => cursor = member.receiver,
                    ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
                    _ => return cursor,
                }
            }
        }
    }
    mod place_values {

        /*
            Place-rooted structural value production: each root's custody (borrowed,
            copied, projected, owned) selects its own establishment path.
            All sources publish the stored-carrier identity used by the type registry,
            not a second row for ordinary qualification. Exact declared types remain on
            nodes and carrier comparisons; descriptors and bounded-byte shapes retain
            the shells required by their representation. No lookup grants membership.
            Readable copies retain one source subtree and selector, rather than
            rebuilding records field by field and discarding non-scalar evidence.
        */
        use super::super::{Builder, ExpressionHandle, ExpressionNode, TypeReferenceNode};
        use super::place_types::{projection_root_expression, readable_place};
        use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
        use checked_trees::{
            CheckedScalarComputationHandle, CheckedScalarExpressionRole,
            CheckedStructuralAccess as StructuralAccess, CheckedStructuralValueKind as ValueKind,
            CheckedUnitStructuralArgumentPlan as ArgumentPlan,
            CheckedUnitStructuralArgumentSourcePlan as SourcePlan,
            CheckedUnitStructuralPathSegment as PathSegment,
        };
        use facts::PlaceRoot;
        use language_semantics::Multiplicity;
        use language_semantics::ReferenceAccess;
        use typed_trees::types::TypeReferenceHandle;
        use validation::CanonicalPlace;
        use validation::ContentsRequirement;
        use validation::borrowed_slice_view_result;
        use validation::canonical_place_type_reference;
        use validation::expression_result_type_reference;
        use validation::has_contents;
        use validation::logical_structural_type_identity;
        use validation::reference_result_custody::is_reference_record;
        use validation::reference_result_custody::stored_loan_parts;
        use validation::reference_shell;
        use validation::stored_structural_type_identity;
        use validation::stores_reference_shape;
        use validation::value_preserving_cast_operand;

        impl Builder<'_, '_> {
            /*
                A shared reference with the same unrestricted carrier retains
                its descriptor on reborrow. It does not reconstruct a view from the
                loan's backing: that would lose the producer's actual range and effects.
                Stored and computed sources use their ordinary value producer before
                this reborrow. Access attenuation and affine carriers remain references.
            */
            pub(super) fn shared_carrier_reborrow(
                &mut self,
                expression: ExpressionHandle,
                expected: TypeReferenceHandle,
            ) -> Option<ValueKind> {
                let borrow = self
                    .program
                    .expression_table
                    .borrow_expression(expression)?;
                if borrow.access != ReferenceAccess::Shared
                    || !matches!(
                        self.program.type_reference_table.type_reference(expected),
                        TypeReferenceNode::Reference {
                            access: ReferenceAccess::Shared,
                            ..
                        }
                    )
                    || self.program.type_multiplicity(expected) != Multiplicity::Unrestricted
                {
                    return None;
                }
                let target = borrow.target;
                let machine = self.program.machine_by_symbol(self.machine)?;
                let state = self.program.state_in_machine(machine, self.state)?;
                let actual =
                    expression_result_type_reference(self.program, machine, state, target)?;
                if self.program.normalized_type_identity(actual)
                    != self.program.normalized_type_identity(expected)
                {
                    return None;
                }
                let source = self.structural_value(target, actual)?;
                Some(ValueKind::Projection {
                    source,
                    logical_root_type_identity: Default::default(),
                    path: Vec::new(),
                    projection_evaluations: Default::default(),
                    copies_selected_value: false,
                    type_identity: stored_structural_type_identity(self.program, actual, &[], &[]),
                })
            }

            /*
                A value cast that keeps its structural carrier -- an explicit domain
                qualification or an explicit erasure -- denotes its operand unchanged
                (`as` "never silently changes denotation", and no carrier transform
                exists between equal stored carriers). Checking has already judged the
                qualification at this exact cast, so the value is its operand's
                producer viewed whole: the same whole-value projection a same-carrier
                reborrow publishes. Recasts and representation changes are not this.
            */
            pub(super) fn same_carrier_structural_cast(
                &mut self,
                expression: ExpressionHandle,
                expected: TypeReferenceHandle,
            ) -> Option<ValueKind> {
                let operand = value_preserving_cast_operand(
                    self.program,
                    self.state,
                    self.statement_index,
                    expression,
                )?;
                let machine = self.program.machine_by_symbol(self.machine)?;
                let state = self.program.state_in_machine(machine, self.state)?;
                let actual =
                    expression_result_type_reference(self.program, machine, state, operand)?;
                let carrier = stored_structural_type_identity(self.program, actual, &[], &[]);
                if carrier != stored_structural_type_identity(self.program, expected, &[], &[]) {
                    return None;
                }
                let source = self.structural_value(operand, actual)?;
                Some(ValueKind::Projection {
                    source,
                    logical_root_type_identity: Default::default(),
                    path: Vec::new(),
                    projection_evaluations: Default::default(),
                    copies_selected_value: false,
                    type_identity: carrier,
                })
            }

            /*
                Copy one copyable stored value into a fresh owned place. Shared loan
                carriers retain affine retirement custody on each independent copy;
                the source root and its ownership or loan remain unchanged.
            */
            pub(super) fn copied_place(
                &mut self,
                expression: ExpressionHandle,
                expected: TypeReferenceHandle,
            ) -> Option<ValueKind> {
                if !super::place_types::copied_place_type(self.program, expected) {
                    return None;
                }
                if let Some(kind) = self.copied_dispatch_place(expression, expected) {
                    return Some(kind);
                }
                let (source, leaf, _) =
                    self.borrowed_leaf_argument(expression, expected, expression)?;
                let computations = source
                    .computation_operands(&self.plans.operands)
                    .collect::<Vec<_>>();
                for (ordinal, value) in computations.into_iter().enumerate() {
                    self.push_scalar_root(
                        CheckedScalarExpressionRole::StructuralValueSelector {
                            expression,
                            selector_ordinal: u32::try_from(ordinal).ok()?,
                        },
                        value,
                    )?;
                }
                Some(ValueKind::CopiedStructuralPlace { source, leaf })
            }

            /// The dispatch already selected this existing endpoint before patterns.
            /// Keep its exact carrier and access ceiling; the alias is not an owned
            /// producer to reconstruct. This source names storage only, not a copied
            /// value, readable contents, or new loan authority.
            pub(super) fn retained_dispatch_place_source(
                &self,
                anchor: ExpressionHandle,
                access: StructuralAccess,
            ) -> Option<SourcePlan> {
                let receiver = self
                    .dispatch_receivers
                    .get(&(anchor.arena_index(), anchor.generation()))?;
                if receiver.value.is_valid()
                    || access == StructuralAccess::Owned
                    || !crate::values::reference_sources::access_can_supply(receiver.access, access)
                    || reference_shell(self.program, receiver.type_reference).is_some()
                {
                    return None;
                }
                let root = canonical_place_type_reference(
                    self.program,
                    self.state,
                    self.statement_index,
                    &CanonicalPlace {
                        root: PlaceRoot::Expression(anchor),
                        segments: Vec::new(),
                    },
                )?;
                if !validation::declared_carriers_match(self.program, root, receiver.type_reference)
                {
                    return None;
                }
                let identity = stored_structural_type_identity(
                    self.program,
                    receiver.type_reference,
                    &[],
                    &[],
                );
                identity
                    .is_valid()
                    .then_some(SourcePlan::StructuralExpression {
                        expression: anchor,
                        value_dependency: None,
                        logical_root_type_identity: Some(identity),
                    })
            }

            /// A readable leaf keeps its named or saved home and exact projection.
            /// Its type names the leaf; borrowing it never takes the whole receiver.
            /// The consuming operation owns selector evaluation; its expression may
            /// differ from the selected source when a recast consumes that descriptor.
            pub(in crate::execution::scalar_computations) fn borrowed_leaf_argument(
                &mut self,
                expression: ExpressionHandle,
                expected: TypeReferenceHandle,
                evaluation_owner: ExpressionHandle,
            ) -> Option<(
                ArgumentPlan,
                TypeReferenceHandle,
                Vec<(ExpressionHandle, CheckedScalarComputationHandle)>,
            )> {
                let place = self.canonical_place(expression)?;
                let source_symbol = place.root.as_symbol();
                // Value planning precedes the final structural parameter roster. Its
                // dense positions change when a borrowed receiver is retained, so an
                // authored leaf must keep its symbol until binding resolves that roster.
                // StructuralLocal already represents both live locals and named formals;
                // independent source replay still checks the exact authored root.
                let mut source = match place.root {
                    PlaceRoot::Symbol(symbol) => SourcePlan::StructuralLocal { symbol },
                    PlaceRoot::Expression(anchor) => {
                        self.retained_dispatch_place_source(anchor, StructuralAccess::SharedBorrow)?
                    }
                    _ => return None,
                };
                let selectors =
                    self.projection_selector_evaluations(evaluation_owner, expression)?;
                let ProjectedArgumentPath { mut path, .. } =
                    self.projected_place_path(&place, &selectors)?;
                if !place.segments.is_empty()
                    && let Some(source_symbol) = source_symbol
                    && let Some(state) = self.program.state_by_symbol(self.state)
                    && let Some((_, local)) =
                        self.program.state_local_declaration(state, source_symbol)
                {
                    let mut reference = local.type_reference;
                    let mut crossings = Vec::new();
                    while let Some(shell) = reference_shell(self.program, reference) {
                        if shell.access == ReferenceAccess::WriteOnly {
                            return None;
                        }
                        if !stores_reference_shape(self.program, reference, &[]) {
                            break;
                        }
                        crossings.push(PathSegment::Referent);
                        reference = shell.referee;
                    }
                    crossings.append(&mut path);
                    path = crossings;
                }
                let projected = readable_place(
                    self.program,
                    self.state,
                    self.statement_index,
                    &place,
                    expected,
                )?;
                for _ in 0..projected.referent_crossings {
                    path.push(PathSegment::Referent);
                }
                source = crate::execution::terminal_unit::calls::retain_local_reference_source(
                    self.program,
                    self.state,
                    source,
                    &path,
                );
                let type_identity =
                    stored_structural_type_identity(self.program, projected.reference, &[], &[]);
                let mut source = ArgumentPlan {
                    source,
                    path,
                    projection_evaluations: Default::default(),
                    type_identity,
                    access: StructuralAccess::SharedBorrow,
                };
                source.retain_projection_evaluations(
                    self.projection_selector_span(evaluation_owner, &selectors)?,
                    &self.plans.operands,
                )?;
                Some((source, projected.reference, selectors))
            }

            /*
                One projected owned child of an existing owner. The checked ownership
                transfer is the authoritative admission: its recorded canonical path
                must equal the authored place, and the projected type must be the
                enclosing result type. The root source is retained whole, including an
                evaluated product, and selectors keep their own checked evaluations.
            */
            pub(super) fn projected_owned_place(
                &mut self,
                expression: ExpressionHandle,
                expected: TypeReferenceHandle,
            ) -> Option<ValueKind> {
                if !matches!(
                    self.program.expression_table.expression(expression),
                    ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
                ) {
                    return None;
                }
                let place = self.canonical_place(expression)?;
                let selectors = self.projection_selector_evaluations(expression, expression)?;
                let ProjectedArgumentPath {
                    storage_type: projected,
                    mut path,
                    ..
                } = self.projected_place_path(&place, &selectors)?;
                if stored_structural_type_identity(self.program, projected, &[], &[])
                    != stored_structural_type_identity(self.program, expected, &[], &[])
                {
                    return None;
                }
                for (ordinal, (_, value)) in selectors.iter().enumerate() {
                    self.push_scalar_root(
                        CheckedScalarExpressionRole::StructuralValueSelector {
                            expression,
                            selector_ordinal: u32::try_from(ordinal).ok()?,
                        },
                        *value,
                    )?;
                }
                let root_expression = projection_root_expression(self.program, expression);
                let root_reference = canonical_place_type_reference(
                    self.program,
                    self.state,
                    self.statement_index,
                    &CanonicalPlace {
                        root: place.root,
                        segments: Vec::new(),
                    },
                )?;
                if matches!(place.root, PlaceRoot::Expression(_)) {
                    // The canonical place walk starts at borrowed storage. An
                    // evaluated value instead owns its stored descriptor, so retain
                    // each actual pointer crossing before selecting the first field.
                    // View descriptors already interpret their own element paths.
                    let mut prefix =
                        crate::execution::terminal_unit::calls::stored_reference_prefix(
                            self.program,
                            root_reference,
                            |access| access.is_readable(),
                        )?;
                    prefix.append(&mut path);
                    path = prefix;
                }
                let source = self.structural_value(root_expression, root_reference)?;
                if matches!(place.root, PlaceRoot::Expression(_)) {
                    self.retain_structural_value_root(source, root_reference)?;
                }
                let projection_evaluations =
                    self.projection_selector_span(expression, &selectors)?;
                if !ArgumentPlan::projection_evaluations_match(
                    &path,
                    projection_evaluations,
                    &self.plans.operands,
                ) {
                    return None;
                }
                Some(ValueKind::Projection {
                    source,
                    logical_root_type_identity: if matches!(place.root, PlaceRoot::Symbol(_))
                        && stores_reference_shape(self.program, root_reference, &[])
                    {
                        logical_structural_type_identity(self.program, root_reference, &[], &[])
                    } else {
                        Default::default()
                    },
                    path,
                    projection_evaluations,
                    copies_selected_value: super::place_types::copied_place_type(
                        self.program,
                        expected,
                    ),
                    type_identity: stored_structural_type_identity(
                        self.program,
                        projected,
                        &[],
                        &[],
                    ),
                })
            }

            /// Authenticate a saved subject's original carrier and capture identity.
            /// This query supplies no executable source edge or permission to replay it.
            pub(super) fn saved_subject_anchor(
                &self,
                saved: &typed_trees::expression::TableSavedSubject,
                expected: TypeReferenceHandle,
            ) -> Option<ExpressionHandle> {
                let source = self.program.saved_value_source(saved)?;
                let machine = self.program.machine_by_symbol(self.machine)?;
                let state = self.program.state_in_machine(machine, self.state)?;
                let actual =
                    expression_result_type_reference(self.program, machine, state, source)?;
                if self.program.normalized_type_identity(actual)
                    != self.program.normalized_type_identity(expected)
                {
                    return None;
                }
                Some(
                    self.program
                        .expression_table
                        .value_capture(saved.capture)
                        .value_anchor,
                )
            }

            pub(super) fn owned_structural_place(
                &self,
                expression: ExpressionHandle,
                expected: TypeReferenceHandle,
            ) -> Option<ArgumentPlan> {
                if let ExpressionNode::SavedSubject(saved) =
                    self.program.expression_table.expression(expression)
                {
                    let anchor = self.saved_subject_anchor(saved, expected)?;
                    return Some(ArgumentPlan {
                        source: SourcePlan::StructuralExpression {
                            value_dependency: None,
                            expression: anchor,
                            logical_root_type_identity: None,
                        },
                        path: Vec::new(),
                        projection_evaluations: Default::default(),
                        type_identity: stored_structural_type_identity(
                            self.program,
                            expected,
                            &[],
                            &[],
                        ),
                        access: StructuralAccess::Owned,
                    });
                }
                let name = self.program.expression_table.name_path_node(expression)?;
                if !name.symbol.is_valid()
                    || name.head_symbol != name.symbol
                    || name.members.count() != 1
                    || !names_owned_structural_value(self.program, expected)
                {
                    return None;
                }
                let owner = self.program.machine_by_symbol(self.machine)?;
                let state = self.program.state_in_machine(owner, self.state)?;
                let (source, reference) = if let Some((parameter_index, type_reference)) =
                    self.structural_parameter(name.symbol)
                {
                    if reference_shell(self.program, type_reference).is_some() {
                        return None;
                    }
                    (SourcePlan::Parameter { parameter_index }, type_reference)
                } else {
                    let (_, local) = self
                        .program
                        .state_local_declaration(state, name.symbol)
                        .filter(|&(ordinal, _)| ordinal < self.statement_index)?;
                    (
                        SourcePlan::StructuralLocal {
                            symbol: local.symbol,
                        },
                        local.type_reference,
                    )
                };
                if stored_structural_type_identity(self.program, reference, &[], &[])
                    != stored_structural_type_identity(self.program, expected, &[], &[])
                    || !names_owned_structural_value(self.program, reference)
                {
                    return None;
                }
                let type_identity =
                    stored_structural_type_identity(self.program, reference, &[], &[]);
                Some(ArgumentPlan {
                    projection_evaluations: Default::default(),
                    source,
                    path: Vec::new(),
                    type_identity,
                    access: StructuralAccess::Owned,
                })
            }
        }

        /// A declaration that names a completed structural value this transfer owns
        /// directly: a construction, linear owner or view, a stored reference
        /// descriptor or loan, or a borrowed slice view. Membership here is the
        /// authored declaration's shape, never a source spelling or a scalar lane.
        pub(in crate::execution::scalar_computations) fn names_owned_structural_value(
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            has_contents(
                program,
                reference,
                &[],
                ContentsRequirement::StructuralConstruction,
            ) || has_contents(program, reference, &[], ContentsRequirement::LinearOwned)
                || has_contents(
                    program,
                    reference,
                    &[],
                    ContentsRequirement::OwnedOrSharedView,
                )
                || is_reference_record(program, reference)
                || stored_loan_parts(program, reference).is_some()
                || borrowed_slice_view_result(program, reference)
        }
    }
    mod value_predicates {
        /*
            Structural carrier compatibility and fresh operand establishment.

            Receiving qualifications do not change a producer's storage carrier.
            Zeroed scalar-array parts and borrowed argument operands retain their own
            establishment rules; assignment planning uses the common value graph.
        */
        use super::super::{PrimitiveType, TypeReferenceNode};
        use checked_trees::CheckedStructuralValueKind;
        use typed_trees::types::TypeReferenceHandle;

        /*
            A producer supplies its declared storage carrier; the receiving qualification
            is a separate occurrence-bound checking judgment. Keeping the full receiving
            type on the value node preserves that judgment without requiring constructors
            or callees to repeat an already checked receiving membership.
        */
        pub(super) fn structural_carriers_match(
            program: &dyn ::validation::CheckingInput,
            returned: TypeReferenceHandle,
            expected: TypeReferenceHandle,
        ) -> bool {
            let (Some(returned), Some(expected)) = (
                validation::unwrapped_type_reference(program, returned),
                validation::unwrapped_type_reference(program, expected),
            ) else {
                return false;
            };
            if program.normalized_type_identity(returned)
                == program.normalized_type_identity(expected)
            {
                return true;
            }
            match (
                program.type_reference_table.type_reference(returned),
                program.type_reference_table.type_reference(expected),
            ) {
                (
                    TypeReferenceNode::Generic {
                        base_symbol: b1,
                        base_name: n1,
                        ..
                    },
                    TypeReferenceNode::Generic {
                        base_symbol: b2,
                        base_name: n2,
                        ..
                    },
                ) => {
                    (b1.is_valid() && b2.is_valid() && b1 == b2)
                        || (!n1.as_str().is_empty() && n1.as_str() == n2.as_str())
                }
                (
                    TypeReferenceNode::Generic {
                        base_symbol,
                        base_name,
                        ..
                    },
                    TypeReferenceNode::Named { symbol, name, .. },
                )
                | (
                    TypeReferenceNode::Named { symbol, name, .. },
                    TypeReferenceNode::Generic {
                        base_symbol,
                        base_name,
                        ..
                    },
                ) => {
                    (base_symbol.is_valid() && symbol.is_valid() && base_symbol == symbol)
                        || (!base_name.as_str().is_empty() && base_name.as_str() == name.as_str())
                }
                _ => false,
            }
        }

        /*
            An omitted runtime field's zero value is admitted only for a
            `[scalar; N]` leaf: a literal-length fixed array whose element resolves
            to one plain integer primitive. Anything else — borrowed carriers,
            bounded integers, nested composites — has no authored zero spelling to
            synthesize, so the field must stay authored.
        */
        pub(super) fn zeroed_scalar_array_parts(
            program: &dyn ::validation::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<(PrimitiveType, u64)> {
            let reference = validation::unwrapped_type_reference(program, type_reference)?;
            let TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } = program.type_reference_table.type_reference(reference)
            else {
                return None;
            };
            let primitive_type = program.primitive_type_reference(*element_type)?;
            if !primitive_type.is_fixed_integer() {
                return None;
            }
            Some((primitive_type, u64::try_from(*length).ok()?))
        }

        /*
            The checked literal-zero computation one synthesized zeroed field
            element contributes: a landed `0` in the declared element carrier.
        */
        pub(super) fn zeroed_integer_literal(
            primitive_type: PrimitiveType,
        ) -> Option<checked_trees::CheckedScalarExpression> {
            let landed_type = match primitive_type {
                PrimitiveType::I8 => numerics::literals::LandedIntegerType::I8,
                PrimitiveType::I16 => numerics::literals::LandedIntegerType::I16,
                PrimitiveType::I32 => numerics::literals::LandedIntegerType::I32,
                PrimitiveType::I64 => numerics::literals::LandedIntegerType::I64,
                PrimitiveType::U8 => numerics::literals::LandedIntegerType::U8,
                PrimitiveType::U16 => numerics::literals::LandedIntegerType::U16,
                PrimitiveType::U32 => numerics::literals::LandedIntegerType::U32,
                PrimitiveType::U64 => numerics::literals::LandedIntegerType::U64,
                _ => return None,
            };
            Some(checked_trees::CheckedScalarExpression::IntegerLiteral {
                literal: numerics::literals::IntegerLiteral::from_parts(
                    false,
                    numerics::literals::IntegerRadix::Decimal,
                    "0",
                )
                .ok()?
                .with_landing(numerics::literals::IntegerLanding {
                    landed_type,
                    domain: numerics::arithmetic::ArithmeticDomain::Exact,
                }),
            })
        }

        /*
            Existing storage transfers through the call's canonical source identity.
            Copies and evaluated descriptors establish values; a whole-place transfer
            must not acquire another result identity or another ownership event.
        */
        pub(super) fn argument_needs_establishment(kind: &CheckedStructuralValueKind) -> bool {
            let CheckedStructuralValueKind::Place(argument) = kind else {
                return true;
            };
            use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
            match &argument.source {
                CheckedUnitStructuralArgumentSourcePlan::CapturedStorage { .. }
                | CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
                | CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. }
                | CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
                | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
                | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { .. }
                | CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
                | CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal { .. }
                | CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. } => false,
                CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
                | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
                | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. } => true,
            }
        }
    }

    pub(super) use place_types::{canonical_place_is_borrowable, needs_copied_place_root};
    pub(super) use place_values::names_owned_structural_value;

    impl Builder<'_, '_> {
        /* Retain runtime children independently of the canonical endpoint's ordinal. */
        pub(in crate::execution::scalar_computations) fn projection_selector_evaluations(
            &mut self,
            owner: ExpressionHandle,
            source: ExpressionHandle,
        ) -> Option<Vec<ProjectionSelector>> {
            let selectors = authored_projection_selectors(self.program, source)?;
            self.retain_projection_selector_evaluations(owner, selectors)
        }

        /* The exact declared type of `expression` at this statement. */
        pub(super) fn expression_type(
            &self,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            validation::expression_type_reference_in_state(
                self.program,
                self.state,
                self.statement_index,
                expression,
            )
        }

        /* The canonical place `expression` names at this statement. */
        pub(super) fn canonical_place(
            &self,
            expression: ExpressionHandle,
        ) -> Option<validation::CanonicalPlace> {
            place_at(self.program, self.state, self.statement_index, expression)
        }

        /*
            Project a canonical place through its authored selector evaluations.
            The retained evaluation supplies each runtime index at this point, so
            the projection never re-reads the selector's original storage.
        */
        pub(super) fn projected_place_path(
            &self,
            place: &validation::CanonicalPlace,
            selectors: &[ProjectionSelector],
        ) -> Option<ProjectedArgumentPath> {
            projected_argument_path(
                self.program,
                self.state,
                self.statement_index,
                place,
                Some(self.machine),
                Some(&|expression| {
                    selectors.iter().find_map(|(selector, value)| {
                        (*selector == expression).then_some(
                            checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(*value),
                        )
                    })
                }),
            )
        }

        pub(super) fn retain_projection_selector_evaluations(
            &mut self,
            owner: ExpressionHandle,
            selectors: Vec<ExpressionHandle>,
        ) -> Option<Vec<ProjectionSelector>> {
            if selectors.is_empty() {
                return Some(Vec::new());
            }
            let statement = u32::try_from(self.statement_index).ok()?;
            if let Some(evaluations) =
                self.plans
                    .projection_selectors_at(self.machine, self.state, statement, owner)
            {
                let values = if evaluations.is_empty() {
                    &[][..]
                } else {
                    self.plans.operands.span(evaluations)?
                };
                if selectors.len() != values.len()
                    || selectors.iter().zip(values).any(|(expression, root)| {
                        !self.plans.nodes.is_valid(*root)
                            || self.plans.nodes.get(*root).authored_root != *expression
                    })
                {
                    return None;
                }
                return Some(selectors.into_iter().zip(values.iter().copied()).collect());
            }
            let values = selectors
                .into_iter()
                .enumerate()
                .map(|(ordinal, selector)| {
                    let role = CheckedScalarExpressionRole::StructuralValueSelector {
                        expression: owner,
                        selector_ordinal: u32::try_from(ordinal).ok()?,
                    };
                    let value = if let Some(root) = self.plans.root_at(
                        self.state,
                        u32::try_from(self.statement_index).ok()?,
                        role,
                    ) {
                        root.root
                    } else {
                        self.index_selector(selector)?
                    };
                    Some((selector, value))
                })
                .collect::<Option<Vec<_>>>()?;
            let evaluations = self
                .plans
                .operands
                .insert_many(values.iter().map(|(_, value)| *value));
            self.plans
                .projection_selectors
                .append(checked_trees::CheckedProjectionSelectorRoot {
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal: statement,
                    expression: owner,
                    evaluations,
                });
            Some(values)
        }

        pub(in crate::execution::scalar_computations) fn projection_selector_span(
            &self,
            expression: ExpressionHandle,
            selectors: &[(
                ExpressionHandle,
                checked_trees::CheckedScalarComputationHandle,
            )],
        ) -> Option<arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>> {
            if selectors.is_empty() {
                return Some(Default::default());
            }
            let span = self.plans.projection_selectors_at(
                self.machine,
                self.state,
                u32::try_from(self.statement_index).ok()?,
                expression,
            )?;
            let children = self.plans.operands.span(span)?;
            children
                .iter()
                .copied()
                .eq(selectors.iter().map(|(_, child)| *child))
                .then_some(span)
        }

        pub(in crate::execution::scalar_computations) fn retain_reference_selectors(
            &mut self,
            expression: ExpressionHandle,
            selectors: Vec<(
                ExpressionHandle,
                checked_trees::CheckedScalarComputationHandle,
            )>,
        ) -> Option<arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>> {
            for (ordinal, (_, value)) in selectors.iter().enumerate() {
                self.push_scalar_root(
                    CheckedScalarExpressionRole::StructuralValueSelector {
                        expression,
                        selector_ordinal: u32::try_from(ordinal).ok()?,
                    },
                    *value,
                )?;
            }
            self.projection_selector_span(expression, &selectors)
        }

        /*
            A nonempty projection names a subtree of the retained value's owner.
            Keep that owner and selection path together. Whole-value occurrences
            retain their own checking judgments and evaluation identity.
        */
        pub(super) fn retained_argument_owner(
            &mut self,
            value: ValueHandle,
            reference: TypeReferenceHandle,
        ) -> Option<(
            checked_trees::CheckedUnitStructuralArgumentSourcePlan,
            TypeReferenceHandle,
            Vec<checked_trees::CheckedUnitStructuralPathSegment>,
            arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>,
        )> {
            let node = self.structural_values.nodes.get(value);
            let (owner, path, projection_evaluations, logical_root_type_identity) = match &node.kind
            {
                // A noncopy selection retains its containing owner. A selected
                // copy owns an independent completed value, just as a whole-value
                // cast owns its checked memberships; using its source occurrence
                // would lose that new value and its exact root identity.
                ValueKind::Projection {
                    source,
                    path,
                    projection_evaluations,
                    copies_selected_value: false,
                    logical_root_type_identity,
                    ..
                } if !path.is_empty() => (
                    *source,
                    path.clone(),
                    *projection_evaluations,
                    logical_root_type_identity
                        .is_valid()
                        .then_some(*logical_root_type_identity),
                ),
                _ => (value, Vec::new(), Default::default(), None),
            };
            let owner_node = self.structural_values.nodes.get(owner);
            let expression = owner_node.expression;
            let owner_reference = owner_node.type_reference;
            let statement = u32::try_from(self.statement_index).ok()?;
            if owner_node.machine != self.machine
                || owner_node.state != self.state
                || owner_node.statement_index != statement
            {
                return None;
            }
            if owner == value
                && !validation::declared_carriers_match(self.program, owner_reference, reference)
            {
                return None;
            }
            self.retain_structural_value_root(owner, owner_reference)?;
            let retained = self.structural_values.root_for_value(owner)?;
            if self
                .program
                .normalized_type_identity(retained.type_reference)
                != self.program.normalized_type_identity(owner_reference)
            {
                return None;
            }
            Some((
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    expression,
                    value_dependency: Some(owner),
                    logical_root_type_identity,
                },
                owner_reference,
                path,
                projection_evaluations,
            ))
        }

        /*
            Authenticate a completed handle before using the common ordinary root
            publisher. Captured copies are already associated by value and cannot
            be republished through their diagnostic source expression. An occupied
            or ambiguous ordinary coordinate cannot accept a different value.
        */
        pub(super) fn retain_structural_value_root(
            &mut self,
            value: ValueHandle,
            reference: TypeReferenceHandle,
        ) -> Option<()> {
            let node = self.structural_values.nodes.get(value);
            let statement = u32::try_from(self.statement_index).ok()?;
            if node.machine != self.machine
                || node.state != self.state
                || node.statement_index != statement
                || !validation::declared_carriers_match(
                    self.program,
                    node.type_reference,
                    reference,
                )
            {
                return None;
            }
            if let Some(root) = self.structural_values.root_for_value(value) {
                return (root.machine == self.machine
                    && root.state == self.state
                    && root.statement_ordinal == statement
                    && root.expression == node.expression
                    && validation::declared_carriers_match(
                        self.program,
                        root.type_reference,
                        node.type_reference,
                    ))
                .then_some(());
            }
            if self
                .structural_values
                .roots_for_statement(self.state, statement)
                .iter()
                .any(|root| {
                    root.root == value
                        || (!root.captured_copy.is_valid() && root.expression == node.expression)
                })
            {
                return None;
            }
            self.retain_structural_root(statement, node.expression, node.type_reference, value);
            Some(())
        }

        /*
            A scalar consumer may need the value of readable storage before a later
            operand changes it. Use the ordinary copy producer and its declared
            capability; a tag observation alone never licenses copying an owner.
        */
        pub(super) fn retain_copied_place_value(
            &mut self,
            expression: ExpressionHandle,
            reference: TypeReferenceHandle,
        ) -> Option<ValueHandle> {
            let kind = self.copied_place(expression, reference)?;
            let value = self.publish_structural_value(expression, reference, kind)?;
            self.retained_argument_owner(value, reference)?;
            Some(value)
        }

        /*
            Retain the value established by an argument, independent of how the
            consumer accesses it. A shared temporary owns its referent, not a
            reference-shaped constructor. Existing places keep their loan/storage
            route; retaining a value does not itself authorize lending or cleanup.
        */
        pub(super) fn retain_structural_argument(
            &mut self,
            expression: ExpressionHandle,
            expected: TypeReferenceHandle,
        ) -> Option<ValueHandle> {
            let (expression, expected) = match self
                .program
                .type_reference_table
                .type_reference(expected)
            {
                _ if self.uses_captured_dispatch_value(expression) => {
                    let producer = if validation::reference_shell(self.program, expected).is_some()
                    {
                        validation::expression_result_type_reference(
                            self.program,
                            self.program.machine_by_symbol(self.machine)?,
                            self.program.state_by_symbol(self.state)?,
                            expression,
                        )?
                    } else {
                        expected
                    };
                    (expression, producer)
                }
                TypeReferenceNode::Reference { .. }
                    if validation::reference_result_custody::stored_loan_parts(
                        self.program,
                        expected,
                    )
                    .is_some()
                        && validation::reference_formation_source(self.program, expression)
                            .is_some() =>
                {
                    let machine = self.program.machine_by_symbol(self.machine)?;
                    let state = self.program.state_by_symbol(self.state)?;
                    let actual =
                        formed_reference_type(self.program, machine, state, expression, expected)?;
                    (expression, actual)
                }
                TypeReferenceNode::Reference { .. }
                    if validation::slice_view_shell(self.program, expected, &[]).is_some() =>
                {
                    (expression, expected)
                }
                TypeReferenceNode::Reference { .. }
                    if let Some(actual) = transported_value_type(
                        self.program,
                        self.program.machine_by_symbol(self.machine)?,
                        self.program.state_by_symbol(self.state)?,
                        self.statement_index,
                        expression,
                    ) =>
                {
                    (expression, actual)
                }
                TypeReferenceNode::Reference { .. }
                    if validation::reference_formation_source(self.program, expression)
                        .is_some() =>
                {
                    (expression, expected)
                }
                TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::Shared,
                    ..
                } => {
                    let (expression, expected) =
                        shared_argument_referent(self.program, expression, expected)?;
                    let place = self.canonical_place(expression)?;
                    if !matches!(place.root, facts::PlaceRoot::Expression(_)) {
                        return None;
                    }
                    (expression, expected)
                }
                TypeReferenceNode::Reference { .. } => return None,
                _ => (expression, expected),
            };
            let statement = u32::try_from(self.statement_index).ok()?;
            let retained = self
                .structural_values
                .root_for_expression(self.state, statement, expression);
            let root = if let Some(root) = retained {
                if root.machine != self.machine
                    || self.program.normalized_type_identity(root.type_reference)
                        != self.program.normalized_type_identity(expected)
                    || !self.structural_values.nodes.is_valid(root.root)
                    || self.structural_values.nodes.get(root.root).expression != expression
                {
                    return None;
                }
                root.root
            } else {
                self.structural_value(expression, expected)?
            };
            if !argument_needs_establishment(&self.structural_values.nodes.get(root).kind) {
                let actual = validation::expression_result_type_reference(
                    self.program,
                    self.program.machine_by_symbol(self.machine)?,
                    self.program.state_by_symbol(self.state)?,
                    expression,
                )?;
                if self.program.normalized_type_identity(actual)
                    == self.program.normalized_type_identity(expected)
                {
                    return None;
                }
            }
            if let ValueKind::Projection {
                source,
                ref path,
                copies_selected_value: false,
                ..
            } = self.structural_values.nodes.get(root).kind
            {
                let source_needs_establishment =
                    argument_needs_establishment(&self.structural_values.nodes.get(source).kind);
                if !path.is_empty() {
                    if !source_needs_establishment {
                        return None;
                    }
                    self.retained_argument_owner(root, expected)?;
                } else if !source_needs_establishment
                    && self.program.normalized_type_identity(
                        self.structural_values.nodes.get(source).type_reference,
                    ) == self.program.normalized_type_identity(expected)
                {
                    return None;
                }
            }
            self.retain_structural_value_root(root, expected)?;
            Some(root)
        }

        /*
            The dense structural position `symbol` occupies in this state's
            signature, with the parameter's declared type, or `None` when the name
            is not a carried structural formal. Dense positions count only the
            parameters that own a structural ingress slot: a `const` or `[erased]`
            binding owns none, and a primitive parameter travels the scalar roster
            instead. Consumers resolve `Parameter { parameter_index }` against the
            retained structural parameter roster, which counts the same way, so the
            index is a shared coordinate rather than a local convention. One known
            boundary: Psi 06's `parameter_declaration` omits the `[erased]`
            predicate, so a state that mixes an erased structural binding with a
            later structural formal is the one shape where the two sides disagree.
        */
        fn structural_parameter(&self, symbol: SymbolHandle) -> Option<(u32, TypeReferenceHandle)> {
            let (index, parameter) = self
                .authored_parameters
                .iter()
                .filter(|parameter| {
                    !parameter.is_const
                        && !parameter.relevance.is_erased()
                        && self
                            .program
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                })
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == symbol)?;
            Some((u32::try_from(index).ok()?, parameter.type_reference))
        }

        /*
            The source plan naming where `symbol`'s storage lives: its dense
            signature position when the name resolves to a carried structural
            formal, and the operation sequence's local slot otherwise. Every whole
            owned structural source this builder plans classifies through here,
            because a parameter root described as a local has no consumption owner:
            the returned-value walk looks for a result binding that a formal never
            mints, and the affine residual routes key on the dense position.
        */
        pub(super) fn structural_source(
            &self,
            symbol: SymbolHandle,
        ) -> checked_trees::CheckedUnitStructuralArgumentSourcePlan {
            match self.structural_parameter(symbol) {
                Some((parameter_index, _)) => {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index,
                    }
                }
                None => checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                },
            }
        }

        pub(super) fn structural_value(
            &mut self,
            expression: ExpressionHandle,
            expected: TypeReferenceHandle,
        ) -> Option<ValueHandle> {
            // Normalization can share an evaluated operand with a later projection.
            // Preserve that occurrence's value identity. A saved subject below
            // authenticates its separate lexical name against that same producer.
            if let Some(root) = self.structural_values.root_for_expression(
                self.state,
                u32::try_from(self.statement_index).ok()?,
                expression,
            ) {
                return (root.machine == self.machine
                    && self.program.normalized_type_identity(root.type_reference)
                        == self.program.normalized_type_identity(expected))
                .then_some(root.root);
            }
            if let ExpressionNode::SavedSubject(saved) =
                self.program.expression_table.expression(expression)
                && self
                    .program
                    .expression_table
                    .contains_value_capture(saved.capture)
                && !self.uses_captured_dispatch_value(expression)
                && let Some(receiver) = self
                    .dispatch_receivers
                    .get(&{
                        let anchor = self
                            .program
                            .expression_table
                            .value_capture(saved.capture)
                            .value_anchor;
                        (anchor.arena_index(), anchor.generation())
                    })
                    .copied()
                && receiver.value.is_valid()
                && self
                    .program
                    .normalized_type_identity(receiver.type_reference)
                    == self.program.normalized_type_identity(expected)
            {
                self.saved_subject_anchor(saved, expected)?;
                // This match already saved the value. Reuse its exact producer
                // when a later borrow selects a field. The saved name creates
                // neither another value home nor another cleanup obligation.
                self.retained_argument_owner(receiver.value, expected)?;
                return Some(receiver.value);
            }
            if let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
                && call.quotient_operation.is_some()
            {
                return self.quotient_call_value(expression, expected);
            }
            let reference_capture = self.reference_capture_source(expression, expected);
            let kind = if let ExpressionNode::Call(call) =
                self.program.expression_table.expression(expression)
                && let Some(query) = &call.native_code_layout
            {
                let selected_state = self
                    .program
                    .machine_by_symbol(query.operand.symbol)
                    .and_then(|machine| self.program.machine_states(machine).first())
                    .map(|state| state.symbol)
                    .unwrap_or(query.operand.symbol);
                let actual = self
                    .program
                    .expression_table
                    .constructed_value_type_reference(expression);
                if self.program.normalized_type_identity(actual)
                    != self.program.normalized_type_identity(expected)
                {
                    return None;
                }
                ValueKind::NativeCodeLayout { selected_state }
            } else if self.uses_captured_dispatch_value(expression) {
                self.copied_dispatch_place(expression, expected)?
            } else if let Some(primitive) = self.program.primitive_type_reference(expected) {
                let value = self.expression(expression, primitive)?;
                self.plans.nodes.get_mut(value).authored_root = expression;
                self.push_scalar_root(
                    CheckedScalarExpressionRole::StructuralScalarValue { expression },
                    value,
                )?;
                ValueKind::Scalar { value }
            } else if let ExpressionNode::Match(dispatch) =
                self.program.expression_table.expression(expression)
                && let Some(selected) = validation::select_anonymous_numeric_match_arm(
                    self.program,
                    dispatch,
                    |expression| operator_is_builtin(self.operators, expression),
                )
            {
                let source = self.structural_value(selected, expected)?;
                ValueKind::Projection {
                    source,
                    logical_root_type_identity: Default::default(),
                    path: Vec::new(),
                    projection_evaluations: Default::default(),
                    copies_selected_value: false,
                    type_identity: validation::stored_structural_type_identity(
                        self.program,
                        expected,
                        &[],
                        &[],
                    ),
                }
            } else if let Some(value) = self.shared_carrier_reborrow(expression, expected) {
                value
            } else if let Some(value) = self.same_carrier_structural_cast(expression, expected) {
                value
            } else if let ExpressionNode::Cast(cast) =
                self.program.expression_table.expression(expression)
                && let Some(definition) = self.program.closed_data_definition(cast.target_type)
                && let Some(quotient) = definition.quotient.as_ref()
            {
                if self.program.normalized_type_identity(expected)
                    != self.program.normalized_type_identity(cast.target_type)
                    || self.program.type_multiplicity(expected)
                        != language_semantics::Multiplicity::Unrestricted
                    || self.program.type_multiplicity(quotient.carrier)
                        != language_semantics::Multiplicity::Unrestricted
                {
                    return None;
                }
                let actual = validation::expression_type_reference_in_state(
                    self.program,
                    self.state,
                    self.statement_index,
                    cast.value,
                )?;
                if self.program.normalized_type_identity(actual)
                    != self.program.normalized_type_identity(quotient.carrier)
                {
                    return None;
                }
                let source = self.structural_value(cast.value, quotient.carrier)?;
                ValueKind::QuotientAdaptation {
                    source,
                    quotient: cast.target_type,
                    license: checked_trees::CheckedQuotientAdaptationLicense::Mint,
                }
            } else if matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::String(_)
            ) {
                let argument = byte_sequence_literal_argument(self.program, expected, expression)?;
                match argument.access {
                    checked_trees::CheckedStructuralAccess::Owned => ValueKind::Place(argument),
                    checked_trees::CheckedStructuralAccess::SharedBorrow => {
                        if !validation::field_domains::string_literal_grants_view_domains(
                            self.program,
                            expected,
                            expression,
                        ) {
                            return None;
                        }
                        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
                            bytes,
                        } = argument.source else {
                            return None;
                        };
                        ValueKind::ByteLiteralView { bytes }
                    }
                    _ => return None,
                }
            } else if validation::reference_shell(self.program, expected).is_some()
                && validation::slice_view_shell(self.program, expected, &[]).is_none()
                && reference_capture.is_some()
            {
                let state = self
                    .program
                    .machine_by_symbol(self.machine)
                    .and_then(|machine| self.program.state_in_machine(machine, self.state))?;
                let mut selectors = Vec::new();
                let mut source_value = ValueHandle::invalid();
                if let Some(formation) = reference_capture.as_ref() {
                    let place = self.canonical_place(formation.source)?;
                    if let facts::PlaceRoot::Expression(root) = place.root {
                        let root_reference = validation::canonical_place_type_reference(
                            self.program,
                            self.state,
                            self.statement_index,
                            &validation::CanonicalPlace {
                                root: place.root,
                                segments: Vec::new(),
                            },
                        )
                        .or_else(|| {
                            validation::expression_result_type_reference(
                                self.program,
                                self.program.machine_by_symbol(self.machine)?,
                                self.program.state_by_symbol(self.state)?,
                                root,
                            )
                        })
                        .or_else(|| {
                            place
                                .segments
                                .is_empty()
                                .then(|| validation::reference_shell(self.program, expected))
                                .flatten()
                                .map(|shell| shell.referee)
                        });
                        let root_reference = root_reference?;
                        source_value = if let Some(receiver) = self
                            .dispatch_receivers
                            .get(&(root.arena_index(), root.generation()))
                            .copied()
                        {
                            // A completed value and an existing place are different
                            // producer roles, even when both have expression roots.
                            // Consume the retained role, not another owned wrapper
                            // around its non-executable alias.
                            if !validation::declared_carriers_match(
                                self.program,
                                root_reference,
                                receiver.type_reference,
                            ) {
                                return None;
                            }
                            receiver.value
                        } else {
                            let value = self.structural_value(root, root_reference)?;
                            let kind = &self.structural_values.nodes.get(value).kind;
                            if argument_needs_establishment(kind) {
                                value
                            } else if let ValueKind::Place(argument) = kind {
                                // Ordinary stored sources need no second producer.
                                // Keep any already recorded real value dependency.
                                argument
                                    .source
                                    .structural_value_dependency()
                                    .unwrap_or_default()
                            } else {
                                return None;
                            }
                        };
                        if source_value.is_valid() {
                            // A real value keeps its exact producer and normal
                            // disposition. An existing place needs no value birth.
                            self.retained_argument_owner(source_value, root_reference)?;
                        }
                    }
                    selectors =
                        self.projection_selector_evaluations(expression, formation.source)?;
                }
                let selector = |expression| {
                    selectors.iter().find_map(|(selector, value)| {
                        (*selector == expression).then_some(
                            checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(*value),
                        )
                    })
                };
                let capture = capture_source(
                    self.program,
                    self.program.machine_by_symbol(self.machine)?,
                    state,
                    self.statement_index,
                    reference_capture?,
                    expected,
                    &selector,
                )?;
                let (mut source, storage, backing_type) =
                    (capture.source, capture.storage, capture.backing_type);
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression: anchor,
                ..
            } = source.source
                && self
                    .dispatch_receivers
                    .get(&(anchor.arena_index(), anchor.generation()))
                    .is_some_and(|receiver| !receiver.value.is_valid())
            {
                // Existing-place aliases are already referent-presented. Bind
                // their actual logical root type before catalog/path consumers;
                // stored descriptor values retain their separate interpretation.
                source.source = self.retained_dispatch_place_source(anchor, source.access)?;
            }
                let projection_evaluations = if selectors.is_empty() {
                    Default::default()
                } else {
                    self.projection_selector_span(expression, &selectors)?
                };
                source
                    .retain_projection_evaluations(projection_evaluations, &self.plans.operands)?;
                let selectors = self.retain_reference_selectors(expression, selectors)?;
                let source_contract = crate::values::reference_sources::qualification_contract(
                    self.program,
                    expected,
                    backing_type,
                    storage,
                );
                ValueKind::Reference {
                    source,
                    selectors,
                    storage,
                    backing_type,
                    source_contract,
                    source_value,
                }
            } else if needs_copied_place_root(
                self.program,
                self.state,
                self.statement_index,
                expression,
                expected,
                |anchor| {
                    self.dispatch_receivers
                        .get(&(anchor.arena_index(), anchor.generation()))
                        .is_some_and(|receiver| !receiver.value.is_valid())
                },
            ) {
                self.copied_place(expression, expected)?
            } else if let Some(view) = self
                .subslice_view(expression, expected)
                .or_else(|| self.slice_view_place(expression, expected))
            {
                view
            } else if let ExpressionNode::Call(call) =
                self.program.expression_table.expression(expression)
            {
                let call_target = validation::call_target_symbol(self.program, call);
                let returned = validation::expression_result_type_reference(
                    self.program,
                    self.program.machine_by_symbol(self.machine)?,
                    self.program.state_by_symbol(self.state)?,
                    expression,
                )?;
                if !value_predicates::structural_carriers_match(self.program, returned, expected)
                    || !self
                        .call_result_types
                        .admits(self.program, &self.carriers, returned)
                {
                    return None;
                }
                let (source_call, call_ordinal) = self.call_ordinal(expression, call_target)?;
                self.record_call_arguments(
                    u32::try_from(self.statement_index).ok()?,
                    call_ordinal,
                    call_target,
                    expressions(self.program).expression_handles(call.arguments),
                );
                ValueKind::Call { source_call }
            } else if matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::Name(_) | ExpressionNode::SavedSubject(_)
            ) && validation::scalar_case_constructor(self.program, expression).is_none()
                && let Some(argument) = self.owned_structural_place(expression, expected)
            {
                ValueKind::Place(argument)
            } else if let Some(projection) = self.projected_owned_place(expression, expected) {
                projection
            } else if let Some(constructor) = self.case_construction(expression) {
                if !value_predicates::structural_carriers_match(
                    self.program,
                    constructor.type_reference,
                    expected,
                ) {
                    return None;
                }
                let field_values = self
                    .plans
                    .case_fields
                    .span(constructor.fields)?
                    .iter()
                    .map(|field| field.value)
                    .collect::<Vec<_>>();
                for (field_ordinal, value) in field_values.iter().enumerate() {
                    self.push_scalar_root(
                        CheckedScalarExpressionRole::StructuralValueField {
                            expression,
                            field_ordinal: u32::try_from(field_ordinal).ok()?,
                        },
                        *value,
                    )?;
                }
                ValueKind::Case(constructor)
            } else if let ExpressionNode::StructLiteral(literal) =
                self.program.expression_table.expression(expression)
                && literal.case_symbol.is_some()
            {
                self.structural_case_value(expression, expected)?
            } else if let ExpressionNode::StructLiteral(literal) =
                self.program.expression_table.expression(expression)
                && literal.case_symbol.is_none()
            {
                self.record_value(expression, expected)?
            } else if validation::scalar_array_preserves_memberships(self.program, expected)
                && !matches!(
                    self.program.expression_table.expression(expression),
                    ExpressionNode::Match(_)
                )
            {
                self.scalar_array_value(expression, expected)?
            } else if let ExpressionNode::ArrayLiteral(literal) =
                self.program.expression_table.expression(expression)
            {
                self.fixed_array_value(expected, *literal)?
            } else if let Some(symbol) =
                validation::scalar_case_value_source(self.program, expression, expected)
            {
                ValueKind::Place(checked_trees::CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: self.structural_source(symbol),
                    path: Vec::new(),
                    type_identity: validation::stored_structural_type_identity(
                        self.program,
                        expected,
                        &[],
                        &[],
                    ),
                    access: checked_trees::CheckedStructuralAccess::Owned,
                })
            } else {
                let ExpressionNode::Match(dispatch) =
                    self.program.expression_table.expression(expression).clone()
                else {
                    return None;
                };
                let (subject, prepared) = self.prepare_dispatch(expression, &dispatch)?;
                let mut arms = Vec::with_capacity(prepared.len());
                for arm in prepared {
                    arms.push(CheckedStructuralDispatchArm {
                        source_arm: arm.source_arm,
                        equality_use: arm.equality_use,
                        pattern: arm.pattern,
                        value: self.structural_value(arm.value, expected)?,
                    });
                }
                ValueKind::Dispatch {
                    subject,
                    arms: self.structural_values.dispatch_arms.insert_many(arms),
                }
            };
            self.publish_structural_value(expression, expected, kind)
        }

        pub(super) fn publish_structural_value(
            &mut self,
            expression: ExpressionHandle,
            type_reference: TypeReferenceHandle,
            kind: ValueKind,
        ) -> Option<ValueHandle> {
            Some(self.structural_values.nodes.append(CheckedStructuralValue {
                machine: self.machine,
                state: self.state,
                statement_index: u32::try_from(self.statement_index).ok()?,
                expression,
                source_span: self.program.expression_table.source_span(expression),
                type_reference,
                qualification_decisions: Default::default(),
                content_transformation: Default::default(),
                content_inputs: Default::default(),
                kind,
            }))
        }
    }
}

use builder::Builder;
mod builder {

    /*
        Build the ordinary scalar evaluation graph from retained checked operations.
        A declaration application supplies captured formal values and its admitted
        call/cast receipts; it does not turn its body into a separate evaluator.
        Structural operands require statement-owned loan/value captures or an
        observation's independently admitted declaration-input homes. An entry
        observation owns its complete operand graph and all of its call receipts.
    */
    use super::call_arguments::{CallOperandCustody, ComputedCallOperands};
    use super::{boundary_calls, integers, normal_return};
    use crate::execution::terminal_unit::types::ShapeCollector;
    use crate::flow::call_flows;
    use crate::published::facts::domain_is_vacuous;
    use crate::values::evaluate_checked_scalar;
    use crate::values::mutable_scalar_parameter_type;
    use crate::values::operator_is_builtin;
    use crate::values::scalar::expression_facts::is_integer;
    use crate::values::scalar::expression_plans::ScalarLocal;
    use crate::values::scalar::scalar_lowering::land_anonymous_scalar_expression;
    use crate::values::scalar::scalar_lowering::lower_return_expression;
    use crate::values::scalar::semantic_casts;
    use crate::values::scalar_expression_type;
    use checked_trees::CheckedBooleanExpression;
    use checked_trees::CheckedOperatorFacts;
    use checked_trees::CheckedScalarExpression;
    use checked_trees::CheckedScalarExpressionPlans;
    use checked_trees::CheckedScalarExpressionRole;
    use checked_trees::{
        CheckedScalarComputation, CheckedScalarComputationHandle as ComputationHandle,
        CheckedScalarComputationKind as ComputationKind, CheckedScalarComputationPlans,
        CheckedScalarComputationRoot, CheckedScalarComputationStructuralArgument,
        CheckedStructuralValueHandle, CheckedStructuralValueRoot, FlowFacts,
    };
    use numerics::arithmetic::ArithmeticDomain;
    use symbols::SymbolHandle;
    use typed_trees::expression::BinaryOperator;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::expression::TableCastExpression;
    use typed_trees::expression::UnaryOperator;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::PrimitiveType;
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::types::TypeReferenceNode;

    pub(super) struct DeclarationScope<'program> {
        pub(super) plan: &'program checked_trees::CheckedRankArrivalPlan,
        pub(super) application: checked_trees::CheckedRankApplicationFormationHandle,
    }

    /*
        Operand aliases belong to a statement's execution roster only when that
        statement owns the graph. An entry observation owns its operands through
        the complete RankObservation root, including declaration call receipts.
    */
    pub(super) enum RootScope {
        Statement,
        RankObservation,
    }

    /// One actually produced saved subject. The carrier and role are published
    /// together before any pattern observes its fields.
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) struct DispatchReceiver {
        pub(super) value: CheckedStructuralValueHandle,
        pub(super) type_reference: TypeReferenceHandle,
        pub(super) access: checked_trees::CheckedStructuralAccess,
    }

    pub(super) struct Builder<'program, 'plans> {
        pub(super) program: &'program dyn ::validation::CheckingInput,
        pub(super) shapes: ShapeCollector<'program>,
        /* The declaration index every collector over this program starts from. */
        pub(super) carriers: crate::execution::terminal_unit::types::CarrierIndex,
        /* Which flow calls' result types admit a checked structural result. */
        pub(super) call_result_types: super::call_result_types::CallResultTypes,
        pub(super) operators: &'program CheckedOperatorFacts,
        pub(super) semantic: &'program facts::FactPlan,
        pub(super) flow: &'program FlowFacts,
        pub(super) borrow: &'program checked_trees::BorrowFacts,
        pub(super) proof: &'program checked_trees::ProofFacts,
        pub(super) pure: &'program CheckedScalarExpressionPlans,
        pub(super) exact_integer_casts: &'program [validation::ExactIntegerCastFact],
        pub(super) machine: SymbolHandle,
        pub(super) state: SymbolHandle,
        pub(super) statement_index: usize,
        pub(super) parameters: &'program [StateParameter],
        /* Explicit declaration inputs for an observation outside statement execution. */
        pub(super) declaration_bindings:
            &'program [(validation::CanonicalPlace, validation::CanonicalPlace)],
        pub(super) declaration_parameters: &'program [StateParameter],
        pub(super) declaration_scope: Option<DeclarationScope<'program>>,
        /* Completed declaration inputs, captured once before evaluating its body. */
        pub(super) declaration_values: Vec<(validation::CanonicalPlace, ComputationHandle)>,
        pub(super) authored_parameters: &'program [StateParameter],
        pub(super) parameter_types: &'program [PrimitiveType],
        pub(super) locals: &'program [ScalarLocal],
        pub(super) plans: &'plans mut CheckedScalarComputationPlans,
        pub(super) structural_values: &'plans mut checked_trees::CheckedStructuralValuePlans,
        /// Produced subject associations for this statement's sparse generational
        /// expression subset. A zero value retains existing storage; it is not an
        /// absent producer. No global expression interval is allocated here.
        pub(super) dispatch_receivers: std::collections::BTreeMap<(u32, u32), DispatchReceiver>,
        pub(super) statement_nodes_start: usize,
        pub(super) root_scope: RootScope,
        /*
            A normalized source operand is one evaluation. Retain that identity for
            every scalar carrier within this statement; distinct authored handles
            and distinct contextual carriers remain separate computations.
        */
        pub(super) scalar_occurrences: Vec<(ExpressionHandle, PrimitiveType, ComputationHandle)>,
        /*
            One statement's normalized source DAG may reference an operand more
            than once. Retain its computation identity, including the contextual
            carrier for anonymous integers; never merge different authored handles.
        */
        pub(super) integer_occurrences: Vec<(
            ExpressionHandle,
            Option<PrimitiveType>,
            integers::IntegerOperand,
        )>,
    }

    impl Builder<'_, '_> {
        /*
            Flow records each short-circuit occurrence's evaluation roster: the
            left capture, then the right capture only when some route selected it.
            When every evaluation of this occurrence at this statement captured
            the left operand alone, the left result already chose the skipping
            route, so the right operand never runs and owns no call fact. That
            recorded roster, not a literal spelling of the left operand, decides
            the skip; a local, field, call result, join or state input proved
            false skips exactly like `false`. No roster keeps both operands.
        */
        fn right_operand_never_evaluated(&self, expression: ExpressionHandle) -> bool {
            let control = &self.flow.control;
            control.with_expression_operands(expression, |operands| {
                let mut evaluations = operands
                    .iter()
                    .map(|operand| control.operands.get(*operand))
                    .filter(|operand| {
                        operand.state_symbol == self.state
                            && operand.statement_index == self.statement_index
                    })
                    .peekable();
                evaluations.peek().is_some()
                    && evaluations.all(|operand| {
                        operand.evaluated_inputs.len() == 1
                            && operand.conditional_inputs_from.is_none()
                    })
            })
        }

        pub(super) fn record_root(
            &mut self,
            pure: &CheckedScalarExpressionPlans,
            statement_ordinal: u32,
            role: CheckedScalarExpressionRole,
            expression: ExpressionHandle,
            expected_type: PrimitiveType,
        ) {
            if self.program.static_type_context_is_unreachable(expression) {
                return;
            }
            if pure
                .expression_at(self.state, statement_ordinal, role)
                .is_some()
                && !self.is_bodyless_boundary_call(expression)
                && !self.observes_captured_dispatch_value(expression)
            {
                return;
            }
            if let Some(existing) = self.plans.root_at(self.state, statement_ordinal, role)
                && existing.machine == self.machine
                && self.plans.nodes.is_valid(existing.root)
                && self.plans.nodes.get(existing.root).authored_root == expression
                && self.plans.nodes.get(existing.root).primitive_type == expected_type
            {
                return;
            }
            if let Some(root) = self.expression(expression, expected_type) {
                self.plans.nodes.get_mut(root).authored_root = expression;
                self.plans.roots.append(CheckedScalarComputationRoot {
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal,
                    role,
                    root,
                });
            }
        }

        /*
            An authored conversion that cannot travel the pure lane still needs an
            exact node at its operand coordinate: the qualification arms above
            admit only representation-identical transfers, while custody positions
            such as call arguments read their evidence from this graph. The
            operand lowers at its own primitive; the node carries the cast's
            destination carrier and authored domain for consumers and the
            qualification-facts rejoin.
        */
        pub(super) fn authored_cast(
            &mut self,
            expression: ExpressionHandle,
            cast: &TableCastExpression,
            expected_type: PrimitiveType,
        ) -> Option<ComputationHandle> {
            if !semantic_casts::requires_custody(self.program, self.state, expression) {
                return None;
            }
            let operand = if let Some(source_primitive) =
                semantic_casts::result_type(self.program, self.state, cast.value)
                    .and_then(|reference| self.program.primitive_type_reference(reference))
            {
                self.expression(cast.value, source_primitive)?
            } else {
                let target = self.program.primitive_type_reference(cast.target_type)?;
                let value = land_anonymous_scalar_expression(
                    self.program,
                    self.operators,
                    cast.value,
                    target,
                )?;
                let operand = self.insert(target, ComputationKind::Value(value));
                self.plans.nodes.get_mut(operand).value_source = cast.value;
                operand
            };
            let result_type = semantic_casts::result_type(self.program, self.state, expression)?;
            let source_primitive = self.plans.nodes.get(operand).primitive_type;
            let target_primitive = self.program.primitive_type_reference(result_type)?;
            let formation_decided = self.exact_integer_casts.iter().any(|fact| {
                fact.expression == expression
                    && fact.source_type == source_primitive
                    && fact.target_type == target_primitive
            });
            Some(self.insert(
                expected_type,
                ComputationKind::AuthoredCast {
                    source_expression: expression,
                    operand,
                    result_type,
                    formation_decided,
                },
            ))
        }

        /*
            Associate an expression with the structural value established for it
            at one statement. Re-publishing an association is idempotent: the
            first root stays the one consumers resolve, because
            `root_for_expression` refuses an ambiguous coordinate and an appended
            second root would hide the association instead of replacing it.
        */
        pub(super) fn retain_structural_root(
            &mut self,
            statement_ordinal: u32,
            expression: ExpressionHandle,
            type_reference: TypeReferenceHandle,
            root: CheckedStructuralValueHandle,
        ) {
            if self
                .structural_values
                .root_for_expression(self.state, statement_ordinal, expression)
                .is_some()
            {
                return;
            }
            self.structural_values
                .roots
                .append(CheckedStructuralValueRoot {
                    captured_copy: arena::Handle::invalid(),
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal,
                    expression,
                    type_reference,
                    root,
                });
        }

        /*
            A scalar root inside the current statement names its coordinate once:
            machine and state come from the builder and the ordinal from the
            statement index, so a site supplies only its role and handle.
        */
        pub(super) fn push_scalar_root(
            &mut self,
            role: CheckedScalarExpressionRole,
            root: ComputationHandle,
        ) -> Option<()> {
            if matches!(self.root_scope, RootScope::RankObservation)
                && !matches!(role, CheckedScalarExpressionRole::RankObservation { .. })
            {
                return Some(());
            }
            let statement_ordinal = u32::try_from(self.statement_index).ok()?;
            let mut published =
                self.plans
                    .roots_for_statement_role(self.state, statement_ordinal, role);
            if let Some(existing) = published.next() {
                return (published.next().is_none()
                    && existing.machine == self.machine
                    && existing.root == root)
                    .then_some(());
            }
            drop(published);
            self.plans.roots.append(CheckedScalarComputationRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal,
                role,
                root,
            });
            Some(())
        }

        pub(super) fn insert(
            &mut self,
            primitive_type: PrimitiveType,
            kind: ComputationKind,
        ) -> ComputationHandle {
            if let ComputationKind::Call {
                source_call,
                callee,
                ..
            } = &kind
                && source_call.is_valid()
                && let Some(handle) = self
                    .plans
                    .nodes
                    .storage_slice()
                    .get(self.statement_nodes_start..)
                    .and_then(|slice| {
                        slice.iter().find_map(|node| {
                            (node.primitive_type == primitive_type
                                && matches!(
                                    &node.kind,
                                    ComputationKind::Call {
                                        source_call: previous,
                                        callee: previous_callee,
                                        ..
                                    } if previous == source_call
                                        && previous_callee.called_signature()
                                            == callee.called_signature()
                                ))
                            .then(|| self.plans.nodes.handle_of(node))
                            .flatten()
                        })
                    })
            {
                return handle;
            }
            let normal_cleanup_actions = self.observation_completion_actions(&kind);
            self.plans.nodes.append(CheckedScalarComputation {
                value_source: ExpressionHandle::invalid(),
                authored_root: ExpressionHandle::invalid(),
                primitive_type,
                normal_cleanup_actions,
                kind,
            })
        }

        pub(super) fn boolean(&mut self, value: bool) -> ComputationHandle {
            self.insert(
                PrimitiveType::Bool,
                ComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::Constant(value),
                ))),
            )
        }

        /*
            The boolean `!` applied to a checked operand: one Apply node whose
            only operand is the value just appended. Enum-equality negation and
            the ordinary logical-not path share this insertion.
        */
        pub(super) fn boolean_negation(
            &mut self,
            operand: ComputationHandle,
            source_expression: ExpressionHandle,
        ) -> ComputationHandle {
            let operands = self.plans.operands.insert_many([operand]);
            self.insert(
                PrimitiveType::Bool,
                ComputationKind::Apply {
                    source_expression,
                    expression: CheckedScalarExpression::Boolean(Box::new(
                        CheckedBooleanExpression::Not(Box::new(
                            CheckedBooleanExpression::Parameter { position: 0 },
                        )),
                    )),
                    operands,
                },
            )
        }

        /*
            One Boolean tag observation of an established subject: the checked
            case-membership operation. The subject is the completed operand the
            authoring site resolved (a place argument, or the case construction it
            established); `receiver_value` is the evaluated owner when one was
            retained ahead of the read, and no receiver for an already-stored
            place; `completion_expression` is the authored occurrence whose value
            the observation completes, and none where the arm cannot complete it.
        */
        pub(super) fn publish_case_membership(
            &mut self,
            source_expression: ExpressionHandle,
            completion_expression: ExpressionHandle,
            receiver_value: CheckedStructuralValueHandle,
            subject: CheckedScalarComputationStructuralArgument,
            case: SymbolHandle,
        ) -> ComputationHandle {
            self.insert(
                PrimitiveType::Bool,
                ComputationKind::CaseMembership {
                    source_expression,
                    completion_expression,
                    receiver_value,
                    subject,
                    case,
                },
            )
        }

        /*
            A cast to the same carrier leaves its payload unchanged, whether
            it selects an arithmetic policy or changes predicate-only facts. Forward
            its already-checked operand through ordinary computation building; a
            call result must not depend on whether a policy changes. Earlier
            operations keep their selected policy. Semantic domains and carrier
            conversions retain their separate proof and qualification custody.
        */
        pub(super) fn same_carrier_cast_value(
            &mut self,
            expression: ExpressionHandle,
            expected_type: PrimitiveType,
        ) -> Option<ComputationHandle> {
            let operand = self.same_carrier_cast_operand(expression)?;
            let cast = self.program.expression_table.cast_node(expression)?;
            if self.program.primitive_type_reference(cast.target_type) != Some(expected_type) {
                return None;
            }
            self.expression(operand, expected_type)
        }

        pub(super) fn same_carrier_cast_operand(
            &self,
            expression: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            semantic_casts::same_carrier_operand(self.program, self.state, expression)
        }

        pub(super) fn retained_constant(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<(ComputationHandle, PrimitiveType)> {
            let (root, primitive) = match self.program.expression_table.expression(expression) {
                ExpressionNode::Call(call) if call.layout_query.is_some() => {
                    (expression, PrimitiveType::U64)
                }
                ExpressionNode::Name(path) => {
                    let declaration = self.program.const_declaration_by_symbol(path.symbol)?;
                    if declaration.evaluated_value.is_some()
                        || !declaration.materialized_initializer.is_valid()
                    {
                        return None;
                    }
                    (
                        declaration.materialized_initializer,
                        self.program
                            .primitive_type_reference(declaration.declared_type)?,
                    )
                }
                _ => return None,
            };
            if !is_integer(primitive) {
                return None;
            }
            Some((
                self.insert(
                    primitive,
                    ComputationKind::RetainedConstant { expression: root },
                ),
                primitive,
            ))
        }

        pub(super) fn expression(
            &mut self,
            expression: ExpressionHandle,
            expected_type: PrimitiveType,
        ) -> Option<ComputationHandle> {
            if let Some((_, _, value)) = self
                .scalar_occurrences
                .iter()
                .find(|(source, carrier, _)| *source == expression && *carrier == expected_type)
            {
                return Some(*value);
            }
            let value = self.build_expression(expression, expected_type)?;
            let prior_source = self.plans.nodes.get(value).authored_root;
            let value = if prior_source.is_valid() && prior_source != expression {
                let operands = self.plans.operands.insert_many([value]);
                self.insert(
                    expected_type,
                    ComputationKind::Apply {
                        source_expression: expression,
                        expression: CheckedScalarExpression::Parameter {
                            position: 0,
                            primitive_type: expected_type,
                        },
                        operands,
                    },
                )
            } else {
                value
            };
            self.plans.nodes.get_mut(value).authored_root = expression;
            self.scalar_occurrences
                .push((expression, expected_type, value));
            Some(value)
        }

        fn build_expression(
            &mut self,
            expression: ExpressionHandle,
            expected_type: PrimitiveType,
        ) -> Option<ComputationHandle> {
            if let Some((value, _)) = self.declaration_input(expression).ok()? {
                return (self.plans.nodes.get(value).primitive_type == expected_type)
                    .then_some(value);
            }
            if let Some((constant, primitive)) = self.retained_constant(expression) {
                return (primitive == expected_type).then_some(constant);
            }
            if is_integer(expected_type)
                && expected_type != PrimitiveType::Addr
                && semantic_casts::result_type(self.program, self.state, expression)
                    .and_then(|reference| self.program.primitive_type_reference(reference))
                    == Some(PrimitiveType::Bool)
            {
                let operand = self.expression(expression, PrimitiveType::Bool)?;
                return Some(self.insert(
                    expected_type,
                    ComputationKind::BooleanToInteger {
                        source_expression: expression,
                        operand,
                    },
                ));
            }

            if self.uses_captured_dispatch_value(expression) {
                return self.completed_scalar_observation(expression, expected_type);
            }
            if let ExpressionNode::Match(dispatch) =
                self.program.expression_table.expression(expression).clone()
            {
                return self.dispatch(expression, &dispatch, expected_type);
            }
            if let Some(observation) =
                self.primitive_reference_observation(expression, expected_type)
            {
                return Some(observation);
            }

            if validation::atomic_load_carrier(self.program, expression).is_some() {
                return self.atomic_load(expression, expected_type);
            }
            if let Some(field) = self.structural_scalar_field(expression) {
                return (field.primitive_type == expected_type)
                    .then(|| self.structural_field(expression, field))
                    .flatten();
            }
            if let Some(operand) = self.same_carrier_cast_value(expression, expected_type) {
                return Some(operand);
            }
            if let ExpressionNode::Cast(cast) =
                self.program.expression_table.expression(expression).clone()
                && cast.semantic_domain.is_empty()
                && let Some(source_type) =
                    semantic_casts::result_type(self.program, self.state, cast.value)
                && semantic_casts::has_declared_domains(self.program, source_type)
            {
                if cast.form.is_recast()
                    || cast.domain != ArithmeticDomain::Exact
                    || !matches!(
                        self.program
                            .type_reference_table
                            .type_reference(cast.target_type),
                        TypeReferenceNode::Named { .. }
                    )
                    || !semantic_casts::has_only_vacuous_tags(self.program, source_type)
                    || self.program.primitive_type_reference(source_type) != Some(expected_type)
                    || self.program.primitive_type_reference(cast.target_type)
                        != Some(expected_type)
                {
                    return self.authored_cast(expression, &cast, expected_type);
                }
                let result_type =
                    semantic_casts::result_type(self.program, self.state, expression)?;
                if semantic_casts::has_declared_domains(self.program, result_type) {
                    return self.authored_cast(expression, &cast, expected_type);
                }
                let operand = self.expression(cast.value, expected_type)?;
                return Some(self.insert(
                    expected_type,
                    ComputationKind::Qualification {
                        source_expression: expression,
                        operand,
                        result_type,
                    },
                ));
            }
            if let ExpressionNode::Cast(cast) =
                self.program.expression_table.expression(expression).clone()
                && !cast.semantic_domain.is_empty()
            {
                if cast.form.is_recast()
                    || !matches!(
                        self.program
                            .type_reference_table
                            .type_reference(cast.target_type),
                        typed_trees::types::TypeReferenceNode::Named { .. }
                    )
                    || cast.domain != ArithmeticDomain::Exact
                    || !cast.semantic_domain_id.is_valid()
                    || !domain_is_vacuous(
                        self.program,
                        cast.semantic_domain_symbol,
                        &mut Vec::new(),
                    )
                    || self.program.primitive_type_reference(cast.target_type)
                        != Some(expected_type)
                    || self.program.primitive_type_reference(cast.result_type)
                        != Some(expected_type)
                {
                    return self.authored_cast(expression, &cast, expected_type);
                }
                let operand = self.expression(cast.value, expected_type)?;
                return Some(self.insert(
                    expected_type,
                    ComputationKind::Qualification {
                        source_expression: expression,
                        operand,
                        result_type: cast.result_type,
                    },
                ));
            }
            if let ExpressionNode::Cast(cast) =
                self.program.expression_table.expression(expression).clone()
                && semantic_casts::requires_custody(self.program, self.state, expression)
            {
                return self.authored_cast(expression, &cast, expected_type);
            }
            if expected_type == PrimitiveType::Bool
                && let Some(operator_use) = self.comparison_use(
                    expression,
                    checked_trees::CheckedOperatorOccurrence::Expression,
                )
                && let Some(primitive) = self.selected_comparison_primitive(operator_use)
            {
                let operands = self
                    .operators
                    .uses
                    .get(operator_use)
                    .operands(self.program)?;
                let left = self.expression(operands[0], primitive)?;
                let right = self.expression(operands[1], primitive)?;
                return Some(self.insert(
                    PrimitiveType::Bool,
                    ComputationKind::SelectedComparison {
                        operator_use,
                        left,
                        right,
                    },
                ));
            }
            if expected_type == PrimitiveType::Bool
                && let Some(comparison) = self.byte_literal_comparison(expression)
            {
                return Some(comparison);
            }
            if expected_type == PrimitiveType::Addr
                && let Some(address) = self.storage_address(expression)
            {
                return Some(address);
            }
            if expected_type == PrimitiveType::Bool
                && let Some(predicate) = self.byte_predicate(expression)
            {
                return Some(predicate);
            }
            if !self.has_declaration_binding(expression)
                && !self.observes_captured_dispatch_value(expression)
                && !semantic_casts::requires_custody(self.program, self.state, expression)
                && let Some(value) = lower_return_expression(
                    self.program,
                    self.operators,
                    expression,
                    self.parameters,
                    self.authored_parameters,
                    self.parameter_types,
                    self.locals,
                    expected_type,
                    self.exact_integer_casts,
                )
            {
                let computation = self.insert(expected_type, ComputationKind::Value(value));
                self.plans.nodes.get_mut(computation).value_source = expression;
                return Some(computation);
            }
            if let Some((computation, _, primitive_type)) = self.indexed_read(expression) {
                return (primitive_type == expected_type).then_some(computation);
            }
            if expected_type == PrimitiveType::Bool
                && let Some(equality) = self.enum_equality(expression)
            {
                return Some(equality);
            }
            if expected_type == PrimitiveType::Bool
                && let Some(membership) = self.case_membership(expression)
            {
                return Some(membership);
            }
            if is_integer(expected_type)
                && !matches!(
                    self.program.expression_table.expression(expression),
                    ExpressionNode::Call(_)
                )
            {
                let integer = self.integer_operand(expression, Some(expected_type))?;
                if scalar_expression_type(&integer.value)? != expected_type {
                    return None;
                }
                return self.materialize_integer(integer);
            }
            match self.program.expression_table.expression(expression).clone() {
                ExpressionNode::Call(call) => {
                    if call.quotient_operation.is_some() {
                        return self.quotient_scalar_call(expression, expected_type);
                    }
                    if self
                        .program
                        .dynamic_slot_by_symbol(call.target_symbol)
                        .is_some()
                    {
                        return self.dynamic_call(expression, &call, expected_type);
                    }
                    let static_requirement_dispatch =
                        if let Some(dispatch) = &call.static_requirement_dispatch {
                            if !validation::closed_requirement_dispatch_is_valid(
                                self.program,
                                self.machine,
                                call.target_symbol,
                                dispatch,
                            ) {
                                return None;
                            }
                            let requirement = self
                                .program
                                .trait_machine_signatures(
                                    self.program
                                        .trait_definition_by_symbol(dispatch.declaring_trait)?,
                                )
                                .iter()
                                .find(|requirement| requirement.symbol == dispatch.requirement)?;
                            if self
                                .program
                                .state_signature_contracts(requirement)
                                .iter()
                                .any(|contract| contract.binding.is_some())
                            {
                                return None;
                            }
                            Some(checked_trees::StaticRequirementDispatchFact::from(dispatch))
                        } else {
                            None
                        };
                    if call.quotient_operation.is_some()
                        || call.private_layout_operation.is_some()
                        || call.layout_query.is_some()
                    {
                        return None;
                    }
                    if static_requirement_dispatch.is_none()
                        && let Some(target) = boundary_calls::bodyless_boundary_target(
                            self.program,
                            validation::call_target_symbol(self.program, &call),
                        )
                    {
                        if !call.evidence_arguments.is_empty() {
                            return None;
                        }
                        return self.boundary_call(expression, &call, target, expected_type);
                    }
                    let entered_target = validation::call_target_entry_symbol(
                        self.program,
                        validation::call_target_symbol(self.program, &call),
                    );
                    let (target_machine, target_state) =
                        validation::find_machine_by_entry_state(self.program, entered_target)?;
                    if static_requirement_dispatch.is_some()
                        && target_machine.supply_mode
                            != language_semantics::MachineSupplyMode::CheckedBody
                    {
                        return None;
                    }
                    let has_runtime_receiver = !self.program.call_has_no_runtime_receiver(
                        &call,
                        target_machine,
                        target_state,
                    );
                    if self
                        .program
                        .primitive_type_reference(target_state.return_type)?
                        != expected_type
                    {
                        return None;
                    }
                    let target_parameters = self.program.state_parameters(target_state);
                    if target_parameters.iter().any(|parameter| {
                        parameter.is_const
                            || (parameter.is_mutable
                                && self
                                    .program
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_some()
                                && mutable_scalar_parameter_type(self.program, parameter).is_none())
                    }) {
                        return None;
                    }
                    let (source_call, call_ordinal) =
                        self.call_origin(expression, call.target_symbol)?;
                    if !call.evidence_arguments.is_empty()
                        && !self.has_declared_evidence_inputs(
                            target_machine.symbol,
                            target_state.symbol,
                            call_ordinal,
                            call.evidence_arguments.len(),
                        )
                    {
                        return None;
                    }
                    let arguments = self
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                        .to_vec();
                    if has_runtime_receiver {
                        return None;
                    }
                    if arguments.len() != target_parameters.len() {
                        return None;
                    }
                    let ComputedCallOperands {
                        arguments,
                        structural_arguments,
                        temporary_cleanups,
                        claim_transfers,
                    } = self.call_operands(
                        match source_call {
                            checked_trees::CheckedCallOrigin::Statement(call) => {
                                CallOperandCustody::Statement {
                                    call,
                                    captures: self.flow.control.calls.get(call).argument_operands(),
                                }
                            }
                            checked_trees::CheckedCallOrigin::Declaration { application, call } => {
                                CallOperandCustody::Declaration { application, call }
                            }
                        },
                        &arguments,
                        target_parameters,
                    )?;
                    Some(self.insert(
                        expected_type,
                        ComputationKind::Call {
                            callee: checked_trees::CheckedScalarCallee::Body {
                                machine: target_machine.symbol,
                                state: target_state.symbol,
                            },
                            source_expression: expression,
                            source_call,
                            call_ordinal,
                            arguments,
                            structural_arguments,
                            temporary_cleanups,
                            claim_transfers,
                        },
                    ))
                }
                ExpressionNode::Binary(binary)
                    if expected_type == PrimitiveType::Bool
                        && operator_is_builtin(self.operators, expression)
                        && matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
                {
                    let condition = self.expression(binary.left, PrimitiveType::Bool)?;
                    let evaluate_when = binary.operator == BinaryOperator::And;
                    if let ComputationKind::Value(value) = &self.plans.nodes.get(condition).kind
                        && let Some(facts::ScalarValue::Boolean(value)) =
                            evaluate_checked_scalar(value, &mut |_| None)
                    {
                        return if value == evaluate_when {
                            self.expression(binary.right, PrimitiveType::Bool)
                        } else {
                            Some(condition)
                        };
                    }
                    if normal_return::boolean_result(self.plans, condition) == Some(!evaluate_when)
                        || self.right_operand_never_evaluated(expression)
                    {
                        return Some(condition);
                    }
                    let right = self.expression(binary.right, PrimitiveType::Bool)?;
                    let skipped = self.boolean(!evaluate_when);
                    let (when_true, when_false) = if evaluate_when {
                        (right, skipped)
                    } else {
                        (skipped, right)
                    };
                    Some(self.insert(
                        PrimitiveType::Bool,
                        ComputationKind::Select {
                            source_expression: expression,
                            condition,
                            when_true,
                            when_false,
                        },
                    ))
                }
                ExpressionNode::Unary(unary)
                    if expected_type == PrimitiveType::Bool
                        && unary.operator == UnaryOperator::LogicalNot
                        && operator_is_builtin(self.operators, expression) =>
                {
                    let operand = self.expression(unary.operand, PrimitiveType::Bool)?;
                    Some(self.boolean_negation(operand, expression))
                }
                ExpressionNode::Binary(binary)
                    if expected_type == PrimitiveType::Bool
                        && matches!(
                            binary.operator,
                            BinaryOperator::Equal
                                | BinaryOperator::NotEqual
                                | BinaryOperator::Less
                                | BinaryOperator::LessOrEqual
                                | BinaryOperator::Greater
                                | BinaryOperator::GreaterOrEqual
                        )
                        && operator_is_builtin(self.operators, expression) =>
                {
                    if let Some(comparison) = self.integer_comparison(expression, &binary) {
                        return Some(comparison);
                    }
                    if !matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    ) {
                        return None;
                    }
                    let left = self.expression(binary.left, PrimitiveType::Bool)?;
                    let right = self.expression(binary.right, PrimitiveType::Bool)?;
                    let operands = self.plans.operands.insert_many([left, right]);
                    let mut template = CheckedBooleanExpression::Equal {
                        left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
                        right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
                    };
                    if binary.operator == BinaryOperator::NotEqual {
                        template = CheckedBooleanExpression::Not(Box::new(template));
                    }
                    Some(self.insert(
                        PrimitiveType::Bool,
                        ComputationKind::Apply {
                            source_expression: expression,
                            expression: CheckedScalarExpression::Boolean(Box::new(template)),
                            operands,
                        },
                    ))
                }
                _ => None,
            }
        }

        /*
            Retain an executable occurrence only with one complete declared input
            lane. This is plan formation, not evidence inhabitation: the ordinary
            contract checker still authenticates every source term before emission.
        */
        fn has_declared_evidence_inputs(
            &self,
            target_machine: SymbolHandle,
            target_state: SymbolHandle,
            call_ordinal: u32,
            count: usize,
        ) -> bool {
            let mut calls = self
                .proof
                .contract_calls
                .iter()
                .map(|(_, call)| call)
                .filter(|call| {
                    call.caller_machine_symbol == self.machine
                        && call.caller_state_symbol == self.state
                        && call.statement_index == self.statement_index
                        && call.call_ordinal == call_ordinal as usize
                });
            let Some(call) = calls.next() else {
                return false;
            };
            if calls.next().is_some()
                || call.target_machine_symbol != target_machine
                || call.target_state_symbol != target_state
                || call.signature.is_valid()
            {
                return false;
            }
            let mut parameters = Vec::new();
            for reference in self.proof.contract_fact_refs.span_or_empty(call.requires) {
                let contract = self.proof.contract_facts.get(reference.fact);
                let Some(parameter) = contract.evidence_term else {
                    continue;
                };
                let term = self.proof.evidence_terms.get(parameter);
                if contract.kind != checked_trees::ContractProofFactKind::Requires
                    || contract.owner
                        != (checked_trees::ContractProofFactOwner::Machine {
                            machine_symbol: target_machine,
                        })
                    || term.owner != contract.owner
                    || term.kind != contract.kind
                {
                    return false;
                }
                parameters.push(term.lane_position);
            }
            parameters.sort_unstable();
            parameters.len() == count && parameters.iter().copied().eq(0..count)
        }

        fn call_origin(
            &self,
            expression: ExpressionHandle,
            target: SymbolHandle,
        ) -> Option<(checked_trees::CheckedCallOrigin, u32)> {
            let Some(scope) = &self.declaration_scope else {
                return self
                    .call_ordinal(expression, target)
                    .map(|(call, ordinal)| (call.into(), ordinal));
            };
            let application = scope.plan.application_formations.get(scope.application);
            let mut matching =
                application
                    .application
                    .calls
                    .handles()
                    .enumerate()
                    .filter(|(_, handle)| {
                        let call = scope.plan.application_calls.get(*handle);
                        call.expression == expression && call.authored_target == target
                    });
            let (ordinal, call) = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            let origin = checked_trees::CheckedCallOrigin::Declaration {
                application: scope.application,
                call,
            };
            let (_, call) = origin
                .declaration(scope.plan, self.machine, self.state)
                .ok()?;
            (call.target_state == validation::call_target_entry_symbol(self.program, target))
                .then_some((origin, u32::try_from(ordinal).ok()?))
        }

        pub(super) fn call_ordinal(
            &self,
            expression: ExpressionHandle,
            target: SymbolHandle,
        ) -> Option<(arena::Handle<checked_trees::FlowCallFact>, u32)> {
            let target = validation::call_target_entry_symbol(self.program, target);
            let state = self.flow.control.state(self.machine, self.state)?;
            let mut matching = call_flows(&self.flow.control, state.calls)
                .iter()
                .filter(|call| {
                    call.statement_index == self.statement_index
                        && call.target_symbol == target
                        && call.authored_expression == expression
                });
            let call = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            let handle = self.flow.control.calls.handle_of(call)?;
            Some((handle, u32::try_from(call.call_ordinal).ok()?))
        }
    }
}

pub(crate) fn build_checked_value_computation_plans(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    semantic: &FactPlan,
    flow: &FlowFacts,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    pure: &mut CheckedScalarExpressionPlans,
    exact_integer_casts: &[ExactIntegerCastFact],
) -> (
    CheckedScalarComputationPlans,
    CheckedStructuralValuePlans,
    checked_trees::CheckedCompletionPlans,
) {
    dispatch_copies::withdraw_captured_pure_roots(program, flow, pure);
    let carriers = crate::execution::terminal_unit::types::carrier_index(program);
    let call_result_types = call_result_types::CallResultTypes::build(program, flow, &carriers);
    let mut plans = CheckedScalarComputationPlans::default();
    let mut structural_values = CheckedStructuralValuePlans::default();
    let mut completions = checked_trees::CheckedCompletionPlans::default();
    for machine in program.machines() {
        let states = program.machine_states(machine);
        for state in states {
            let scalar_parameters = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| occupies_scalar_position(program, parameter))
                .cloned()
                .collect::<Vec<_>>();
            let parameters = scalar_parameters.as_slice();
            let Some(parameter_types) = parameters
                .iter()
                .map(|parameter| program.primitive_type_reference(parameter.type_reference))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let mut locals = Vec::new();
            let captured = unique_state_flow(flow, machine, state);
            let statements = program.statement_table.statements(state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                let Ok(statement_ordinal) = u32::try_from(statement_index) else {
                    continue;
                };
                let statement_nodes_start = plans.nodes.storage_slice().len();
                let mut builder = Builder {
                    program,
                    shapes: ShapeCollector::with_carrier_index(program, carriers.clone()),
                    carriers: carriers.clone(),
                    call_result_types: call_result_types.clone(),
                    operators,
                    semantic,
                    flow,
                    borrow,
                    proof,
                    pure,
                    exact_integer_casts,
                    machine: machine.symbol,
                    state: state.symbol,
                    statement_index,
                    parameters,
                    declaration_parameters: &[],
                    declaration_scope: None,
                    declaration_values: Vec::new(),
                    declaration_bindings: &[],
                    authored_parameters: program.state_parameters(state),
                    parameter_types: &parameter_types,
                    locals: &locals,
                    plans: &mut plans,
                    structural_values: &mut structural_values,
                    statement_nodes_start,
                    root_scope: builder::RootScope::Statement,
                    dispatch_receivers: Default::default(),
                    scalar_occurrences: Vec::new(),
                    integer_occurrences: Vec::new(),
                };
                builder.retain_dispatch_copy_roots();
                if crate::execution::terminal_unit::types::is_unit(program, state.return_type) {
                    match statement {
                        StatementNode::Expression(expression)
                            if statement_index + 1 == statements.len() =>
                        {
                            let _ = builder.record_completion(*expression, &mut completions);
                        }
                        StatementNode::Transition(transition) => {
                            for target in transition.targets() {
                                if let TransitionTargetNode::Value(expression) =
                                    program.statement_table.transition_target(target)
                                {
                                    let _ =
                                        builder.record_completion(*expression, &mut completions);
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // Endpoints are ordinary operands. Capture every occurrence
                // before constructing consumers, so a call result and a pure
                // bound supply the same range operation at its authored site.
                for (expression, site) in sites(program, machine, state, statement_index, statement)
                    .into_iter()
                    .rev()
                {
                    for (endpoint, role, ()) in
                        subslice_endpoints(program, operators, expression, site, |_| Some(()))
                    {
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            role,
                            endpoint,
                            expression_result_type_reference(program, machine, state, endpoint)
                                .and_then(|reference| program.primitive_type_reference(reference))
                                .filter(|primitive| primitive.integer_sign_bits().is_some())
                                .unwrap_or(PrimitiveType::U64),
                        );
                    }
                }
                let construction_destination = match statement {
                    StatementNode::LocalData(local) => {
                        Some((local.initial_value, local.type_reference))
                    }
                    StatementNode::Assignment(assignment) => {
                        assignment_stored_value_type(program, machine, state, assignment)
                            .map(|expected| (assignment.value, expected))
                    }
                    StatementNode::Expression(expression) => {
                        let expected = if expressions(program)
                            .call_node(*expression)
                            .and_then(|call| call.native_code_layout.as_ref())
                            .is_some_and(|query| query.discards_result)
                        {
                            expressions(program).constructed_value_type_reference(*expression)
                        } else {
                            state.return_type
                        };
                        Some((*expression, expected))
                    }
                    _ => None,
                };
                let scalar_elements = construction_destination
                    .filter(|(expression, expected)| {
                        !structural_values::needs_copied_place_root(
                            program,
                            state.symbol,
                            statement_index,
                            *expression,
                            *expected,
                            |_| false,
                        )
                    })
                    .and_then(|(expression, expected)| {
                        scalar_array_elements(program, machine.symbol, expression, expected)
                    });
                if let Some((expression, expected)) = construction_destination
                    && scalar_elements.is_none()
                    && !crate::execution::terminal_unit::types::is_unit(program, expected)
                    && (is_scalar_case_value(program, expression, expected)
                        || program.primitive_type_reference(expected).is_none())
                    && let Some(root) = builder.structural_value(expression, expected)
                {
                    builder.retain_structural_root(statement_ordinal, expression, expected, root);
                }
                if let Some(local) = statement.as_local_data()
                    && program
                        .primitive_type_reference(local.type_reference)
                        .is_none()
                    && !expressions(program).expression_is_valid(local.initial_value)
                    && zeroed_structural_parts(program, local.type_reference)
                    && let Ok(projected_qualifications) =
                        zeroed_byte_projected_qualifications(program, local.type_reference)
                {
                    let root = builder
                        .structural_values
                        .nodes
                        .append(CheckedStructuralValue {
                            machine: machine.symbol,
                            state: state.symbol,
                            statement_index: statement_ordinal,
                            expression: local.initial_value,
                            source_span: Default::default(),
                            type_reference: local.type_reference,
                            qualification_decisions: Default::default(),
                            content_transformation: Default::default(),
                            content_inputs: Default::default(),
                            kind: CheckedStructuralValueKind::ZeroedStructural {
                                projected_qualifications,
                            },
                        });
                    builder.retain_structural_root(
                        statement_ordinal,
                        local.initial_value,
                        local.type_reference,
                        root,
                    );
                }
                if let Some(elements) = scalar_elements {
                    for (element_index, element) in elements.elements.into_iter().enumerate() {
                        let Ok(element_ordinal) = u32::try_from(element_index) else {
                            break;
                        };
                        let role = CheckedScalarExpressionRole::ArrayElement {
                            source: CheckedArrayConstructionSource::Statement,
                            element_ordinal,
                        };
                        match element {
                            ScalarArrayElement::Authored {
                                expression,
                                primitive_type,
                            } => {
                                builder.record_root(
                                    pure,
                                    statement_ordinal,
                                    role,
                                    expression,
                                    primitive_type,
                                );
                            }
                            ScalarArrayElement::Projected {
                                row,
                                indices,
                                leaf_type,
                                primitive_type,
                            } => {
                                builder.record_projected_element_root(
                                    statement_ordinal,
                                    role,
                                    row,
                                    &indices,
                                    leaf_type,
                                    primitive_type,
                                );
                            }
                        }
                    }
                }
                for construction in captured
                    .map(|captured| {
                        call_array_constructions_in(
                            program,
                            flow,
                            captured,
                            machine,
                            state,
                            statement_index,
                        )
                    })
                    .unwrap_or_default()
                {
                    builder.retain_structural_argument(
                        construction.expression,
                        construction.type_reference,
                    );
                }
                for (call_ordinal, site) in captured
                    .map(|captured| {
                        nested_call_sites(program, flow, captured, statement, statement_index)
                    })
                    .unwrap_or_default()
                {
                    let CallSite::Expression { call, .. } = site else {
                        continue;
                    };
                    let Ok(call_ordinal) = u32::try_from(call_ordinal) else {
                        continue;
                    };
                    builder.record_call_arguments(
                        statement_ordinal,
                        call_ordinal,
                        call_target_symbol(program, call),
                        program.expression_table.expression_handles(call.arguments),
                    );
                }
                if let Some(local) = statement.as_local_data() {
                    if let Some(carrier) = atomic_once_carrier(program, local.initial_value)
                        && let Some(primitive_type) =
                            declared_place_type_raw(program, machine, Some(state), carrier.place)
                                .and_then(|cell| program.primitive_type_reference(cell))
                    {
                        for (ordinal, operand) in carrier.operands.into_iter().enumerate() {
                            let Ok(operand_ordinal) = u32::try_from(ordinal) else {
                                break;
                            };
                            builder.record_root(
                                pure,
                                statement_ordinal,
                                CheckedScalarExpressionRole::AtomicOperand { operand_ordinal },
                                operand,
                                primitive_type,
                            );
                        }
                    }
                    let structural_destination = program
                        .primitive_type_reference(local.type_reference)
                        .is_none();
                    let argument_roots = (!local.is_mutable || structural_destination)
                        && (result_initializer_call_is_admitted(
                            program,
                            machine,
                            local.initial_value,
                        ) || nested_boundary_applications_compose(
                            program,
                            local.initial_value,
                        ));
                    if let Some(call) = program.expression_table.call_node(local.initial_value) {
                        builder.record_call_arguments(
                            statement_ordinal,
                            0,
                            call_target_symbol(program, call),
                            program.expression_table.expression_handles(call.arguments),
                        );
                    }
                    if let Some(primitive_type) =
                        program.primitive_type_reference(local.type_reference)
                    {
                        let binding_ordinal = u32::try_from(
                            locals
                                .iter()
                                .filter(|local: &&ScalarLocal| local.is_value_binding())
                                .count(),
                        );
                        if let Ok(binding_ordinal) = binding_ordinal {
                            let role = if local.is_mutable {
                                CheckedScalarExpressionRole::StorageInitializer
                            } else {
                                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal }
                            };
                            if !argument_roots {
                                builder.record_root(
                                    pure,
                                    statement_ordinal,
                                    role,
                                    local.initial_value,
                                    primitive_type,
                                );
                            }
                        }
                        locals.push(ScalarLocal::from_local_data(program, local, primitive_type));
                    } else if let Some(binding) =
                        crate::values::scalar::primitive_reference_read::local(program, local)
                    {
                        locals.push(binding);
                    }
                    continue;
                }
                if let Some(call) = statement.as_call() {
                    builder.record_call_arguments(
                        statement_ordinal,
                        0,
                        call.target_symbol,
                        program.statement_table.expression_handles(call.arguments),
                    );
                    continue;
                }
                if let Some(expression) = statement.as_expression()
                    && let ExpressionNode::Call(call) =
                        program.expression_table.expression(*expression)
                {
                    builder.record_call_arguments(
                        statement_ordinal,
                        0,
                        call_target_symbol(program, call),
                        program.expression_table.expression_handles(call.arguments),
                    );
                    if unit_statement_call_is_admitted(program, machine, state, *expression) {
                        continue;
                    }
                }
                if let Some(assignment) = statement.as_assignment() {
                    for (depth, index) in assignment_target_selectors(program, assignment.target)
                        .into_iter()
                        .enumerate()
                    {
                        let Ok(depth) = u32::try_from(depth) else {
                            break;
                        };
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            CheckedScalarExpressionRole::AssignmentIndex { depth },
                            index,
                            PrimitiveType::U64,
                        );
                    }
                    if let Some(primitive_type) =
                        assignment_stored_value_type(program, machine, state, assignment)
                            .filter(|installed| reference_shell(program, *installed).is_none())
                            .and_then(|installed| {
                                assignment_target_primitive_type(program, installed)
                            })
                    {
                        if let Some(carrier) = atomic_assignment_carrier(program, assignment) {
                            for (ordinal, operand) in carrier.operands.into_iter().enumerate() {
                                let Ok(operand_ordinal) = u32::try_from(ordinal) else {
                                    break;
                                };
                                builder.record_root(
                                    pure,
                                    statement_ordinal,
                                    CheckedScalarExpressionRole::AtomicOperand { operand_ordinal },
                                    operand,
                                    primitive_type,
                                );
                            }
                        } else {
                            builder.record_root(
                                pure,
                                statement_ordinal,
                                CheckedScalarExpressionRole::AssignmentValue,
                                assignment.value,
                                primitive_type,
                            );
                        }
                    }
                    if let ExpressionNode::Call(call) =
                        program.expression_table.expression(assignment.value)
                        && builder
                            .plans
                            .root_at(
                                state.symbol,
                                statement_ordinal,
                                CheckedScalarExpressionRole::AssignmentValue,
                            )
                            .is_none()
                    {
                        builder.record_call_arguments(
                            statement_ordinal,
                            0,
                            call_target_symbol(program, call),
                            program.expression_table.expression_handles(call.arguments),
                        );
                    }
                    continue;
                }
                if let Some(expression) = statement.as_expression()
                    && statement_index + 1 == statements.len()
                    && let Some(result_type) = program.primitive_type_reference(state.return_type)
                {
                    builder.record_root(
                        pure,
                        statement_ordinal,
                        CheckedScalarExpressionRole::Return,
                        *expression,
                        result_type,
                    );
                }
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                if let Some(captured) = captured {
                    for call in call_flows(&flow.control, captured.calls)
                        .iter()
                        .filter(|call| call.statement_index == statement_index)
                    {
                        let Some(CallSite::TransitionNamed { arguments, .. }) = find_call_site(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            call.call_ordinal,
                        ) else {
                            continue;
                        };
                        if terminal_callable_target(program, machine, call.target_symbol).is_none()
                        {
                            continue;
                        }
                        let Ok(call_ordinal) = u32::try_from(call.call_ordinal) else {
                            continue;
                        };
                        builder.record_call_arguments(
                            statement_ordinal,
                            call_ordinal,
                            call.target_symbol,
                            program.statement_table.expression_handles(arguments),
                        );
                    }
                }
                if let TransitionGuardNode::When(expression) = transition.guard {
                    builder.record_root(
                        pure,
                        statement_ordinal,
                        CheckedScalarExpressionRole::Guard,
                        expression,
                        PrimitiveType::Bool,
                    );
                }
                for (target, continuation) in
                    [(transition.target, false), (transition.continuation, true)]
                {
                    if !target.is_valid() {
                        continue;
                    }
                    if transition.exit == TransitionExit::Ordinary
                        && let TransitionTargetNode::Value(expression) =
                            program.statement_table.transition_target(target)
                        && program
                            .primitive_type_reference(state.return_type)
                            .is_none()
                        && let Some(root) = builder.structural_value(*expression, state.return_type)
                    {
                        builder.retain_structural_root(
                            statement_ordinal,
                            *expression,
                            state.return_type,
                            root,
                        );
                    }
                    if transition.exit == TransitionExit::Ordinary
                        && let TransitionTargetNode::Value(expression) =
                            program.statement_table.transition_target(target)
                        && let Some(result_type) =
                            program.primitive_type_reference(state.return_type)
                    {
                        let role = if continuation {
                            CheckedScalarExpressionRole::ContinuationReturn
                        } else {
                            CheckedScalarExpressionRole::Return
                        };
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            role,
                            *expression,
                            result_type,
                        );
                    }
                    let Some(transfer) = completing_transition_transfer(program, target) else {
                        continue;
                    };
                    let Some(target_state) =
                        activation_target_state_index(program, machine, transfer.target)
                            .and_then(|target_index| states.get(target_index))
                            .or_else(|| {
                                find_machine_head(program, transfer.target).map(|(_, entry)| entry)
                            })
                    else {
                        continue;
                    };
                    let target_parameters =
                        program.state_parameters(target_state).iter().enumerate();
                    for (argument, (target_position, parameter)) in
                        transfer.arguments.iter().zip(target_parameters)
                    {
                        let Ok(argument_ordinal) = u32::try_from(target_position) else {
                            continue;
                        };
                        let role = if continuation {
                            CheckedScalarExpressionRole::TransitionContinuationArgument {
                                argument_ordinal,
                            }
                        } else {
                            CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                        };
                        let Some(expected_type) =
                            program.primitive_type_reference(parameter.type_reference)
                        else {
                            if !continuation
                                && transition.exit == TransitionExit::Ordinary
                                && !parameter.is_const
                                && !parameter.relevance.is_erased()
                            {
                                builder.retain_structural_argument(
                                    *argument,
                                    parameter.type_reference,
                                );
                                builder.record_transition_selectors(
                                    statement_ordinal,
                                    argument_ordinal,
                                    *argument,
                                );
                            }
                            continue;
                        };
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            role,
                            *argument,
                            expected_type,
                        );
                    }
                }
            }
        }
    }
    plans.seal();
    structural_values.seal();
    completions.seal();
    (plans, structural_values, completions)
}

/*
    Source-admitted observations append only after completed effects and final
    source requirements. Ordinary expression computation stays in its early
    production window; pruning and execution see this completed rank roster.
*/
pub(crate) fn record_rank_observations(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
) {
    rank_observations::record(
        program,
        &facts.operators,
        &facts.semantic,
        &facts.flow,
        &facts.borrow,
        &facts.proof,
        &facts.values.scalar_expressions,
        &facts.operators.exact_integer_casts,
        &facts.termination.rank_arrivals,
        &mut facts.values.scalar_computations,
        &mut facts.values.structural_values,
    );
    facts.values.scalar_computations.seal();
    facts.values.structural_values.seal();
}
