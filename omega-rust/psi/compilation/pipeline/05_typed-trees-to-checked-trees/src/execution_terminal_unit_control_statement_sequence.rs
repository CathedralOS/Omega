/*
    The single-state body producer: execution-ordered scalar bindings, primitive
    and structural field stores, borrowed-window moves and repairs, selected
    operator and FMA applications, structural bindings, view-subslice locals,
    calls and completion.
    Constructors and calls share structural binding ordinals. Completion selects
    one existing result or constructs its final expression in this same sequence.
    Each statement is planned by the emitter its kind and evidence select; no
    whole-body shape chooses a different producer, so any statement the
    sequence admits composes with any other.

    Source ordinals identify facts; execution positions schedule operations.
    Inclusive/exclusive source anchors bound the sequence without treating a
    skipped following statement as an executable endpoint.

    `build` sets up the sequence and walks the body's admitted execution order:
    `local_data` plans a `let`, `assignments` plans an assignment, and
    `statement_call` then plans the call either statement leaves (or a call
    or expression statement performs). `completion` selects the structural or
    scalar result and checks call-count agreement. The exit owner receives the
    sequence's exact window debt and distinguishes normal completion from
    abandonment instead of requiring repair before it knows the terminator.
*/

use super::super::{
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, ExpressionNode, Multiplicity, StatementNode,
    SymbolHandle,
};
use crate::execution::terminal_unit::ScalarCalleeClosure;
use crate::execution::terminal_unit::calls::{ExpectedCallValueResult, build_call_operation};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::call_occurrences;
use crate::execution::terminal_unit::control::call_results::bind_scalar_call_result;
use crate::execution::terminal_unit::control::call_results::bind_structural_call_result;
use crate::execution::terminal_unit::control::call_results::checked_structural_result_type;
use crate::execution::terminal_unit::control::structural_operands;
use crate::execution::terminal_unit::scalar_locals::scalar_expression_local_at;
use crate::execution::terminal_unit::scalar_return_expressions::checked_boolean_contains_short_circuit;
use crate::execution::terminal_unit::types::{ShapeCollector, is_unit, machine_binders};
use crate::flow::expressions;

use checked_trees::CheckedUnitStructuralReturnPlan;
use std::ops::Bound;

mod assignments {
    /*
        Planning one assignment of a single-state body.

        An assignment's stores come, in order, from: the store sequence's own
        planned stores at this statement (primitive, field, element, byte and
        record-member stores); or an owned structural value stored into a field,
        displacing old content only when that field is occupied. What remains is
        an assignment whose right-hand side is the
        call this statement performs: its result binding and the store that
        consumes it go back to the sequence, which plans the call first.
    */
    use super::super::super::borrowed_windows::{OpenWindows, ValueReplacement};
    use super::super::call_results::checked_copied_result_type;
    use super::{
        CheckFacts, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan as ParameterPlan,
        CheckedUnitEffectOperationPlan as OperationPlan, CheckedUnitScalarResultBindingPlan,
        CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralParameterPlan,
        CheckedUnitStructuralResultBindingPlan as BindingPlan, Construction,
        LocalConstructionTrace, Multiplicity, ShapeCollector, SymbolHandle,
        checked_structural_result_type, is_unit, statement_call_target, structural_operands,
    };

    /* The sequence state an assignment reads and extends. */
    pub(super) struct Planner<'a, 'program, 'shapes> {
        pub(super) construction: Construction<'a, 'program, 'shapes>,
        pub(super) binders: &'a [(SymbolHandle, String)],
        pub(super) scalar_parameters: &'a [ParameterPlan],
        pub(super) parameter_storage: &'a [checked_trees::CheckedUnitParameterStoragePlan],
        pub(super) stores: &'a mut std::iter::Peekable<std::vec::IntoIter<OperationPlan>>,
        pub(super) operations: &'a mut Vec<OperationPlan>,
        pub(super) scalar_count: &'a mut usize,
        pub(super) structural_count: &'a mut usize,
        pub(super) windows: &'a mut OpenWindows,
        pub(super) structural_results: &'a mut Vec<(BindingPlan, facts::PlaceRoot)>,
    }

    /* What an assignment left for its own call. */
    pub(super) enum AssignmentPlan {
        /* Every store the assignment needs is planned. */
        Planned,
        /* The right-hand side is a scalar call; `store` consumes its result. */
        ScalarCall {
            result: CheckedUnitScalarResultBindingPlan,
            store: OperationPlan,
        },
        /*
            The right-hand side produces an owned structural value. Its store is
            planned after argument evaluation establishes the actual window frontier.
        */
        StructuralCall {
            result: BindingPlan,
        },
    }

    pub(super) fn plan(
        planner: Planner<'_, '_, '_>,
        statement_index: u32,
        assignment: &typed_trees::statement::TableAssignment,
    ) -> Option<AssignmentPlan> {
        let Planner {
            construction:
                Construction {
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    machine,
                    state,
                    structural_parameters,
                    entry_claims,
                    trivial_affine_locals,
                    trace,
                },
            binders,
            scalar_parameters,
            parameter_storage,
            stores,
            operations,
            scalar_count,
            structural_count,
            windows,
            structural_results,
        } = planner;
        if let Some(capture) = super::super::super::borrowed_windows::capture_assignment_destination(
            program,
            facts,
            shapes,
            machine,
            state,
            structural_parameters,
            scalar_parameters,
            parameter_storage,
            binders,
            statement_index,
            assignment,
            trace,
        ) {
            operations.push(capture);
        }
        let mut consumed = false;
        while let Some(store) =
            stores.next_if(|store| store.store_statement_index() == Some(statement_index))
        {
            operations.push(store);
            consumed = true;
        }
        if consumed {
            return Some(AssignmentPlan::Planned);
        }
        if let Some(expected) =
            super::super::scalar_arrays::assignment_type(program, machine, state, assignment)
        {
            trace.phase("statement sequence: assignment: completed array operands");
            trace.statement(Some(statement_index));
            let elements = super::super::scalar_arrays::elements(
                program,
                facts,
                machine.symbol,
                state.symbol,
                statement_index,
                checked_trees::CheckedArrayConstructionSource::Statement,
                assignment.value,
                expected,
            )?;
            trace.phase("statement sequence: assignment: completed array type");
            trace.statement(Some(statement_index));
            let produced = BindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(*structural_count).ok()?,
                type_identity: shapes.add_type(expected, binders, &[])?,
                multiplicity: Multiplicity::Unrestricted,
                local: Default::default(),
            };
            trace.phase("statement sequence: assignment: completed array destination");
            trace.statement(Some(statement_index));
            let replacement = windows.replace(
                program,
                facts,
                shapes,
                machine,
                state,
                structural_parameters,
                statement_index,
                assignment,
                &produced,
                produced.binding_ordinal.checked_add(1)?,
                CheckedUnitStructuralArgumentPlan::structural_result(
                    produced.binding_ordinal,
                    produced.type_identity,
                    CheckedStructuralAccess::Owned,
                ),
                trace,
            )?;
            *structural_count =
                structural_count.checked_add(1 + replacement.displaced_bindings())?;
            operations.push(OperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                    qualifications: facts.qualifications.formation_domains_at(
                        machine.symbol,
                        state.symbol,
                        statement_index,
                        assignment.value,
                    ),
                    source: checked_trees::CheckedArrayConstructionSource::Statement,
                    result: produced,
                    elements,
                },
            ));
            replacement.consume_source(&mut *operations)?;
            operations.extend(replacement.into_operations());
            return Some(AssignmentPlan::Planned);
        }
        let assignment_phase = |phase: &'static str| {
            trace.phase(phase);
            trace.statement(Some(statement_index));
        };
        assignment_phase("statement sequence: assignment: structural value field store");
        if let Some(root) = facts.values.structural_values.root_for_expression(
            state.symbol,
            statement_index,
            assignment.value,
        ) {
            if root.machine != machine.symbol {
                return None;
            }
            assignment_phase("statement sequence: assignment: structural operand custody");
            let (calls, completion) = super::prepare_value_operands(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                state,
                structural_parameters,
                trivial_affine_locals,
                entry_claims,
                statement_index,
                structural_results,
                operations,
                structural_count,
                root.root,
                trace,
                windows,
            )?;
            if completion == checked_trees::CheckedValueCompletion::NoNormal {
                operations.push(OperationPlan::EvaluateStructuralValue {
                    statement_index,
                    value: root.root,
                    calls,
                    operand_source: None,
                });
                return Some(AssignmentPlan::Planned);
            }
            assignment_phase("statement sequence: assignment: structural result type");
            let installed =
                installed_carrier(program, machine, state, assignment, root.type_reference);
            let mut produced = if matches!(
                facts.values.structural_values.nodes.get(root.root).kind,
                checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
            ) {
                checked_copied_result_type(program, shapes, installed, binders)?
            } else {
                checked_structural_result_type(program, shapes, installed, binders)?
            };
            produced.statement_index = statement_index;
            produced.binding_ordinal = u32::try_from(*structural_count).ok()?;
            assignment_phase("statement sequence: assignment: structural destination window");
            let replacement = completed_value_replacement(
                program,
                facts,
                shapes,
                machine,
                state,
                structural_parameters,
                scalar_parameters,
                parameter_storage,
                windows,
                assignment,
                &produced,
                root.type_reference,
                trace,
            )?;
            *structural_count =
                structural_count.checked_add(1 + replacement.displaced_bindings())?;
            let discard_result_on_return = produced.multiplicity == Multiplicity::Affine;
            structural_results.push((
                produced.clone(),
                facts::PlaceRoot::Expression(assignment.value),
            ));
            let establishment = structural_operands::establish_value(
                facts,
                produced,
                root.root,
                calls,
                None,
                discard_result_on_return,
                operations,
            )?;
            operations.push(establishment);
            assignment_phase("statement sequence: assignment: structural result consumption");
            replacement.consume_source(&mut *operations)?;
            operations.extend(replacement.into_operations());
            return Some(AssignmentPlan::Planned);
        }
        assignment_phase("statement sequence: assignment: pending store order");
        if match stores.peek() {
            None => false,
            Some(pending) => match pending.store_statement_index() {
                None => true,
                Some(ordinal) => ordinal <= statement_index,
            },
        } {
            return None;
        }
        assignment_phase("statement sequence: assignment: call source result type");
        let result_type = validation::call_target_return_type(
            program,
            statement_call_target(program, assignment)?,
        )?;
        if let Some(primitive_type) = program.primitive_type_reference(result_type) {
            let binding_ordinal = u32::try_from(*scalar_count).ok()?;
            let position = u32::try_from(scalar_parameters.len())
                .ok()?
                .checked_add(binding_ordinal)?;
            assignment_phase("statement sequence: assignment: call result field store");
            let store =
                super::super::super::structural_scalar_store::build_structural_call_result_field_store(
                    program,
                    facts,
                    machine,
                    state,
                    structural_parameters,
                    scalar_parameters,
                    parameter_storage,
                    statement_index,
                    assignment,
                    (position, primitive_type),
                    trace,
                )?;
            *scalar_count = scalar_count.checked_add(1)?;
            Some(AssignmentPlan::ScalarCall {
                result: CheckedUnitScalarResultBindingPlan {
                    statement_index,
                    binding_ordinal,
                    primitive_type,
                },
                store,
            })
        } else if is_unit(program, result_type) {
            None
        } else {
            assignment_phase("statement sequence: assignment: structural call result field store");
            let installed = installed_carrier(program, machine, state, assignment, result_type);
            let mut result = checked_structural_result_type(program, shapes, installed, binders)?;
            result.statement_index = statement_index;
            result.binding_ordinal = u32::try_from(*structural_count).ok()?;
            Some(AssignmentPlan::StructuralCall { result })
        }
    }

    /*
        Plan the completed call's replacement against the post-operand frontier.
        Destination selectors were captured before the RHS. Operand transfers may
        have vacated that destination; selecting its old-value death before those
        transfers would dispose an already-consumed value. Binding ordinals are
        final here, so neither the store nor its cleanup needs later renumbering.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn call_result_replacement(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        scalar_parameters: &[ParameterPlan],
        parameter_storage: &[checked_trees::CheckedUnitParameterStoragePlan],
        windows: &mut OpenWindows,
        assignment: &typed_trees::statement::TableAssignment,
        result: &BindingPlan,
        trace: &LocalConstructionTrace,
    ) -> Option<ValueReplacement> {
        let declared = validation::call_target_return_type(
            program,
            statement_call_target(program, assignment)?,
        )?;
        let result_type = installed_carrier(program, machine, state, assignment, declared);
        completed_value_replacement(
            program,
            facts,
            shapes,
            machine,
            state,
            structural_parameters,
            scalar_parameters,
            parameter_storage,
            windows,
            assignment,
            result,
            result_type,
            trace,
        )
    }

    /*
        The carrier an assignment installs. Rebinding a reference stores that
        descriptor; writing through it stores the referent. Callers that only
        know the producer's declared type use it when the destination has no
        stored-value classification.
    */
    fn installed_carrier(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        assignment: &typed_trees::statement::TableAssignment,
        produced: typed_trees::types::TypeReferenceHandle,
    ) -> typed_trees::types::TypeReferenceHandle {
        validation::assignment_stored_value_type(program, machine, state, assignment)
            .unwrap_or(produced)
    }

    /*
        Inline byte fields and byte owner slots use their byte-storage layout;
        a reference-shaped receiving place instead replaces its whole referent.
        This choice depends on the destination, independently of whether a call,
        literal, or composed expression produced the completed value.
    */
    #[allow(clippy::too_many_arguments)]
    fn completed_value_replacement(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        scalar_parameters: &[ParameterPlan],
        parameter_storage: &[checked_trees::CheckedUnitParameterStoragePlan],
        windows: &mut OpenWindows,
        assignment: &typed_trees::statement::TableAssignment,
        result: &BindingPlan,
        result_type: typed_trees::types::TypeReferenceHandle,
        trace: &LocalConstructionTrace,
    ) -> Option<ValueReplacement> {
        if super::super::super::types::byte_sequence_carrier(program, result_type, &[]).is_some() {
            let declared = validation::declared_place_type_raw(
                program,
                machine,
                Some(state),
                assignment.target,
            )?;
            if validation::reference_shell(program, declared).is_none() {
                let store =
                    super::super::super::structural_scalar_store::build_completed_byte_value_store(
                        program,
                        facts,
                        machine,
                        state,
                        structural_parameters,
                        scalar_parameters,
                        parameter_storage,
                        result.statement_index,
                        assignment,
                        result.binding_ordinal,
                        result_type,
                        trace,
                    )?;
                return Some(ValueReplacement::from_store(store));
            }
        }
        windows.replace(
            program,
            facts,
            shapes,
            machine,
            state,
            structural_parameters,
            result.statement_index,
            assignment,
            result,
            result.binding_ordinal.checked_add(1)?,
            CheckedUnitStructuralArgumentPlan::structural_result(
                result.binding_ordinal,
                result.type_identity,
                CheckedStructuralAccess::Owned,
            ),
            trace,
        )
    }
}
pub(in super::super) mod bounded_byte_values {
    /*
        Bounded byte locals own a snapshot of their initializer, not a borrow of
        its operands. Literals, readable places and built-in concatenation share
        the byte-value plan used by stores. Construction proves a capacity bound
        and the declared predicates; it neither truncates nor pads live contents.
        Raw fixed arrays retain their exact-width construction rule.

        The declaration's mutability affects subsequent writes, not construction.
        Each write must preserve its predicates; initializer contents are not
        immutable facts about the resulting storage.
    */
    use super::{CheckedStructuralAccess, Multiplicity};
    use crate::execution::terminal_unit::types::{
        byte_sequence_carrier, structural_access_for_type_reference,
    };
    use crate::flow::expressions;
    use checked_trees::CheckedByteSequenceCarrier;

    /*
        Plan the initializer under the declaration's capacity and byte predicates.
        The caller owns the result binding and publishes ordinary establishment.
    */
    pub(in crate::execution::terminal_unit) fn value_plan(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        state: &typed_trees::state::State,
        statement_index: u32,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<checked_trees::CheckedByteSequenceValuePlan> {
        if !expressions(program).expression_is_valid(local.initial_value)
            || program.type_multiplicity(local.type_reference) != Multiplicity::Unrestricted
            || structural_access_for_type_reference(program, local.type_reference)
                != Some(CheckedStructuralAccess::Owned)
        {
            return None;
        }
        let CheckedByteSequenceCarrier::BoundedOwned { capacity } =
            byte_sequence_carrier(program, local.type_reference, &[])?
        else {
            return None;
        };
        let domains =
            validation::field_domains::domain_constraint_symbols(program, local.type_reference);
        let plain_byte_domain = |symbol| {
            program.domain_definition(symbol).is_some_and(|domain| {
                domain.establishment_routes.is_empty()
                    && domain.semantic_roles == Default::default()
            }) && typed_trees::byte_predicates::domain_byte_predicate(program, symbol).is_some()
        };
        if !domains.iter().copied().all(plain_byte_domain) {
            return None;
        }
        let mut value = checked_trees::CheckedByteSequenceValuePlan::default();
        let (root, maximum_length) =
            crate::execution::terminal_unit::structural_scalar_store::byte_value_plan(
                program,
                operators,
                state,
                statement_index,
                local.initial_value,
                &domains,
                &mut value,
                0,
            )?;
        if maximum_length > capacity {
            return None;
        }
        value.root = root;
        Some(value)
    }

    /*
        Literal construction is shared by local establishment and a selected
        normal result. Neither site gains a view or infers an encoding domain.
    */
    pub(in crate::execution::terminal_unit) fn expression_octets(
        program: &dyn ::validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<Vec<u8>> {
        if !program.expression_table.expression_is_valid(expression)
            || program.type_multiplicity(reference) != Multiplicity::Unrestricted
            || structural_access_for_type_reference(program, reference)
                != Some(CheckedStructuralAccess::Owned)
        {
            return None;
        }
        let CheckedByteSequenceCarrier::BoundedOwned { capacity } =
            byte_sequence_carrier(program, reference, &[])?
        else {
            return None;
        };
        let typed_trees::expression::ExpressionNode::String(bytes) =
            program.expression_table.expression(expression)
        else {
            return None;
        };
        if u64::try_from(bytes.len()).ok()? > capacity {
            return None;
        }
        let domains = validation::field_domains::domain_constraint_symbols(program, reference);
        let plain_literal_domain = |symbol| {
            program.domain_definition(symbol).is_some_and(|domain| {
                domain.establishment_routes.is_empty()
                    && domain.semantic_roles == Default::default()
            }) && validation::field_domains::string_literal_expression_grants_domain(
                program, expression, symbol,
            )
        };
        domains
            .into_iter()
            .all(plain_literal_domain)
            .then(|| bytes.to_vec())
    }
}
mod completion {
    /*
        Completing a single-state body once every statement is planned.

        The exit owner receives exact borrowed-window debt and requires closure on
        normal paths; this prefix cannot presume every terminator returns.
        The body's structural result is chosen from its final statement's checked
        value, a returned call's result, or a returned array. The checked
        value plans the completion the same way whatever its spelling: an existing
        whole formal is forwarded and any other value is established by its value
        operation, references and views included. A transition tail completes at
        the state's terminator instead. Returned carriers are released at their
        checked weakening boundary. A scalar body completes with its final
        expression's checked value. Structural values retain their ordinary transfer
        and cleanup roster across scalar completion; every flow call must be planned.
        Exit weakening follows final-value evaluation. Statement-boundary weakening
        still precedes that statement, so it restores parent access at exactly the
        checked boundary without ending a loan needed by the final expression.
    */
    use super::super::super::borrowed_windows::OpenWindows;
    use super::super::super::reference_results::stored_loan_parts;
    use super::super::super::state_graph::returns::literal_result_operation;
    use super::super::call_results::checked_structural_result_type;
    use super::reference_retirement::OwnerCandidates;
    use super::{
        CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
        CheckedUnitEffectOperationPlan as OperationPlan, CheckedUnitEntryClaimPlan,
        CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan as BindingPlan,
        Construction, Multiplicity, StatementNode, StatementSequence, SymbolHandle,
        append_reference_releases, structural_operands,
    };
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use checked_trees::{CheckedUnitCallPlan, CheckedUnitStructuralReturnPlan as ReturnPlan};

    /* A planned body's final state, with the inputs completion reads. */
    pub(super) struct Completion<'a, 'program, 'shapes> {
        pub(super) construction: Construction<'a, 'program, 'shapes>,
        pub(super) owner_candidates: OwnerCandidates,
        pub(super) prepared_calls:
            &'a mut crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
        pub(super) calls: &'a [&'a checked_trees::FlowCallFact],
        pub(super) statement_end: std::ops::Bound<usize>,
        pub(super) binders: &'a [(SymbolHandle, String)],
        pub(super) scalar_control: Option<checked_trees::CheckedUnitScalarControlPlan>,
        pub(super) operations: Vec<OperationPlan>,
        pub(super) place_loans: Vec<checked_trees::CheckedPlaceLoanPlan>,
        pub(super) scalar_count: usize,
        pub(super) structural_count: usize,
        pub(super) structural_local_symbols: Vec<SymbolHandle>,
        pub(super) windows: OpenWindows,
        pub(super) returned_call: Option<BindingPlan>,
        pub(super) returned_scalar_call: Option<CheckedUnitScalarResultBindingPlan>,
        pub(super) structural_results: Vec<(BindingPlan, facts::PlaceRoot)>,
        pub(super) call_count: usize,
    }

    pub(super) fn complete(completion: Completion<'_, '_, '_>) -> Option<StatementSequence> {
        let Completion {
            construction:
                Construction {
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    machine,
                    state,
                    structural_parameters,
                    entry_claims,
                    trivial_affine_locals,
                    trace,
                },
            owner_candidates,
            prepared_calls,
            calls,
            statement_end,
            binders,
            scalar_control,
            mut operations,
            place_loans,
            mut scalar_count,
            mut structural_count,
            structural_local_symbols,
            mut windows,
            returned_call,
            mut returned_scalar_call,
            mut structural_results,
            call_count,
        } = completion;
        if operations
            .last()
            .is_some_and(|operation| !operation.has_normal_continuation())
        {
            return Some(StatementSequence {
                scalar_result: None,
                scalar_control: None,
                structural_result: None,
                operations,
                windows,
                place_loans,
                structural_local_symbols,
                structural_results,
                next_scalar_binding: scalar_count,
                owner_candidates,
            });
        }
        let statements = typed_trees::statement::reachable_state_statements(
            program.statement_table.statements(state.statement_nodes),
        );
        let tail = statements.last().and_then(|statement| {
            let expression = statement.as_expression()?;
            if expressions(program)
                .call_node(*expression)
                .and_then(|call| call.native_code_layout.as_ref())
                .is_some_and(|query| query.discards_result)
            {
                return None;
            }
            Some((
                u32::try_from(statements.len().checked_sub(1)?).ok()?,
                *expression,
            ))
        });
        let returned_value = tail
            .filter(|_| {
                program
                    .primitive_type_reference(state.return_type)
                    .is_none()
            })
            .and_then(|(ordinal, expression)| {
                facts.values.structural_values.root_for_expression(
                    state.symbol,
                    ordinal,
                    expression,
                )
            });
        let exit = tail
            .map(|(statement_index, expression)| {
                checked_trees::CheckedStructuralReturnCompletion::Value {
                    statement_index,
                    expression,
                    evaluation: checked_trees::FlowPermissionEvaluation::Statement,
                }
            })
            .or_else(|| {
                let binding = returned_call.as_ref()?;
                let mut matching =
                    operations
                        .iter()
                        .filter_map(OperationPlan::call)
                        .filter(|call| {
                            call.structural_result().is_some_and(|result| {
                                result.binding_ordinal == binding.binding_ordinal
                            })
                        });
                let call = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                Some(checked_trees::CheckedStructuralReturnCompletion::Call {
                    coordinate: call.coordinate,
                })
            });
        let return_binding = |binding| Some(ReturnPlan::from_binding(binding, exit?));
        let mut structural_result = if let Some(root) = returned_value {
            trace.phase("statement sequence: structural result: returned value");
            if root.machine != machine.symbol || root.type_reference != state.return_type {
                return None;
            }
            if let Some(result) = forwarded_parameter(
                program,
                facts,
                machine,
                state,
                structural_parameters,
                entry_claims,
                root,
            ) {
                Some(result)
            } else {
                let (calls, completion) = super::prepare_value_operands(
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    machine,
                    state,
                    structural_parameters,
                    trivial_affine_locals,
                    entry_claims,
                    root.statement_ordinal,
                    &mut structural_results,
                    &mut operations,
                    &mut structural_count,
                    root.root,
                    trace,
                    &mut windows,
                )?;
                if completion == checked_trees::CheckedValueCompletion::NoNormal {
                    operations.push(OperationPlan::EvaluateStructuralValue {
                        statement_index: root.statement_ordinal,
                        value: root.root,
                        calls,
                        operand_source: None,
                    });
                    return Some(StatementSequence {
                        scalar_result: None,
                        scalar_control: None,
                        structural_result: None,
                        operations,
                        windows,
                        place_loans,
                        structural_local_symbols,
                        structural_results,
                        next_scalar_binding: scalar_count,
                        owner_candidates,
                    });
                }
                trace.phase("statement sequence: returned value type");
                trace.statement(Some(root.statement_ordinal));
                let mut result =
                    checked_structural_result_type(program, shapes, state.return_type, binders)?;
                result.statement_index = root.statement_ordinal;
                result.binding_ordinal = u32::try_from(structural_count).ok()?;
                trace.phase("statement sequence: returned value producer");
                trace.statement(Some(root.statement_ordinal));
                let establishment = structural_operands::establish_value(
                    facts,
                    result.clone(),
                    root.root,
                    calls,
                    None,
                    false,
                    &mut operations,
                )?;
                operations.push(establishment);
                trace.phase("statement sequence: returned value exit");
                trace.statement(Some(root.statement_ordinal));
                return_binding(result)
            }
        } else if let Some((ordinal, expression)) = tail
            && let Some(operation) = literal_result_operation(
                program,
                shapes,
                state,
                binders,
                &mut structural_count,
                ordinal,
                expression,
            )
        {
            trace.phase("statement sequence: structural result: literal value");
            let result = operation.structural_result_binding()?.clone();
            operations.push(operation);
            return_binding(result)
        } else if let Some(binding) = returned_call {
            for operation in &mut operations {
                if let Some((result, discard_result_on_return)) = operation
                    .call_mut()
                    .and_then(CheckedUnitCallPlan::structural_result_mut)
                    && result.binding_ordinal == binding.binding_ordinal
                {
                    if binding.multiplicity == Multiplicity::Affine
                        && !*discard_result_on_return
                        && stored_loan_parts(program, state.return_type).is_none()
                    {
                        return None;
                    }
                    *discard_result_on_return = false;
                }
            }
            return_binding(binding)
        } else if tail.is_some()
            && validation::is_closed_primitive_array_type(program, state.return_type)
        {
            trace.phase("statement sequence: structural result: returned scalar array");
            let statements = typed_trees::statement::reachable_state_statements(
                program.statement_table.statements(state.statement_nodes),
            );
            if let Some(expression) = statements
                .last()
                .and_then(|statement| statement.as_expression().copied())
            {
                let statement_index = u32::try_from(statements.len().checked_sub(1)?).ok()?;
                let elements = super::super::scalar_arrays::elements(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    checked_trees::CheckedArrayConstructionSource::Statement,
                    expression,
                    state.return_type,
                )?;
                let result = BindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(structural_count).ok()?,
                    type_identity: shapes.add_type(state.return_type, binders, &[])?,
                    multiplicity: program.type_multiplicity(state.return_type),
                    local: Default::default(),
                };
                let qualifications = facts.qualifications.formation_domains_at(
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    expression,
                );
                for domain in &qualifications {
                    shapes.add_domain(*domain, state.return_type, binders)?;
                }
                operations.push(OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                        qualifications,
                        source: checked_trees::CheckedArrayConstructionSource::Statement,
                        result: result.clone(),
                        elements,
                    },
                ));
                return_binding(result)
            } else {
                None
            }
        } else {
            None
        };
        trace.phase("statement sequence: reference result");
        if let Some(result) = &mut structural_result
            && (validation::reference_shell(program, state.return_type).is_some()
                || validation::reference_result_custody::is_reference_record(
                    program,
                    state.return_type,
                ))
        {
            result.reference_sources =
                validation::reference_result_custody::captured_return_sources(
                    program, facts, state,
                )?;
        }
        trace.phase("statement sequence: returned carrier releases");
        let statements = typed_trees::statement::reachable_state_statements(
            program.statement_table.statements(state.statement_nodes),
        );
        let completion_entry = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)))
            .or_else(|| statements.len().checked_sub(1));
        if let Some(statement_index) = completion_entry {
            trace.phase("statement sequence: statement-entry descriptor retirement");
            prepared_calls.prepare_before(
                program,
                facts,
                shapes,
                machine,
                state,
                structural_parameters,
                trivial_affine_locals,
                entry_claims,
                &operations,
                statement_index,
            );
            append_reference_releases(
                &owner_candidates,
                program,
                facts,
                prepared_calls,
                shapes,
                structural_parameters,
                machine.symbol,
                state.symbol,
                u32::try_from(statement_index).ok()?,
                &structural_results,
                &mut operations,
                trace,
            )?;
        }
        if scalar_control.is_none()
            && returned_scalar_call.is_none()
            && let Some(primitive_type) = program.primitive_type_reference(state.return_type)
        {
            trace.phase("statement sequence: scalar completion");
            let statements = typed_trees::statement::reachable_state_statements(
                program.statement_table.statements(state.statement_nodes),
            );
            let completion_index = statements.len().checked_sub(1)?;
            let order = program
                .statement_table
                .admitted_execution_order(state.statement_nodes);
            let end_position = order.prefix_end(statement_end)?;
            let owns_completion =
                super::super::terminal_invocation(program, machine, state, completion_index)
                    .is_none()
                    || order
                        .position(completion_index)
                        .is_some_and(|position| position < end_position);
            if owns_completion
                && let Some(expression) =
                    super::super::completion_expression(program, machine, state, completion_index)
            {
                let statement_index = u32::try_from(completion_index).ok()?;
                let role = CheckedScalarExpressionRole::Return;
                let computations = &facts.values.scalar_computations;
                let mut roots =
                    computations.roots_for_statement_role(state.symbol, statement_index, role);
                let value = if let Some(root) = roots.next() {
                    if roots.next().is_some()
                        || !computations.is_authored_root(
                            root,
                            machine.symbol,
                            expression,
                            primitive_type,
                        )
                        || facts
                            .values
                            .scalar_expressions
                            .expression_at(state.symbol, statement_index, role)
                            .is_some()
                    {
                        return None;
                    }
                    checked_trees::CheckedCallScalarArgument::Computation(root.root)
                } else {
                    let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
                        state.symbol,
                        statement_index,
                        role,
                    )?;
                    if binding.expression != expression
                        || crate::values::scalar_expression_type(value) != Some(primitive_type)
                    {
                        return None;
                    }
                    checked_trees::CheckedCallScalarArgument::Pure(value.clone())
                };
                let result = CheckedUnitScalarResultBindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(scalar_count).ok()?,
                    primitive_type,
                };
                operations.push(OperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Scalar { result, value },
                ));
                scalar_count = scalar_count.checked_add(1)?;
                returned_scalar_call = Some(result);
            }
        }
        trace.phase("statement sequence: call count agreement");
        (call_count == calls.len()).then_some(StatementSequence {
            scalar_result: returned_scalar_call,
            scalar_control,
            structural_result,
            operations,
            windows,
            place_loans,
            structural_local_symbols,
            structural_results,
            next_scalar_binding: scalar_count,
            owner_candidates,
        })
    }

    /*
        A completion value that is an existing whole formal forwards that formal:
        its own custody crosses the return and no second binding is materialized.
        The value's canonical place names the formal; its planned entry must carry
        exactly the result's contract. A shared view formal forwards its loan
        unchanged. An owned formal needs the result's multiplicity and
        qualifications, and a linear one the same checked input-origin claims it
        returns. Otherwise the value is established like any other.
    */
    fn forwarded_parameter(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[CheckedUnitStructuralParameterPlan],
        entry_claims: &[CheckedUnitEntryClaimPlan],
        root: &checked_trees::CheckedStructuralValueRoot,
    ) -> Option<ReturnPlan> {
        let checked_trees::CheckedStructuralValueKind::Place(value) =
            &facts.values.structural_values.nodes.get(root.root).kind
        else {
            return None;
        };
        if !matches!(
            value.source,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
        ) || !value.path.is_empty()
        {
            return None;
        }
        let place = place_at(
            program,
            state.symbol,
            usize::try_from(root.statement_ordinal).ok()?,
            root.expression,
        )?;
        let symbol = place.root.as_symbol()?;
        if !place.segments.is_empty() {
            return None;
        }
        let authored = program.state_parameters(state);
        let (parameter_index, parameter) =
            parameters.iter().enumerate().find(|(_, parameter)| {
                authored
                    .get(parameter.position as usize)
                    .is_some_and(|source| source.symbol == symbol)
            })?;
        let source = authored.get(parameter.position as usize)?;
        if program.normalized_type_identity(source.type_reference)
            != program.normalized_type_identity(state.return_type)
        {
            return None;
        }
        let source_plan = CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(parameter_index).ok()?,
        };
        if crate::execution::terminal_unit::types::borrowed_slice_result(program, state.return_type)
        {
            if Some(parameter.access)
                != crate::execution::terminal_unit::types::structural_access_for_type_reference(
                    program,
                    state.return_type,
                )
                || !parameter.qualifications.is_empty()
            {
                return None;
            }
            return Some(ReturnPlan {
                source: source_plan,
                completion: checked_trees::CheckedStructuralReturnCompletion::Value {
                    statement_index: root.statement_ordinal,
                    expression: root.expression,
                    evaluation: checked_trees::FlowPermissionEvaluation::Statement,
                },
                type_identity: parameter.type_identity,
                multiplicity: validation::reference_result_custody::binding_multiplicity(
                    program,
                    state.return_type,
                ),
                reference_sources: Vec::new(),
            });
        }
        if parameter.access != CheckedStructuralAccess::Owned
            || parameter.multiplicity != program.type_multiplicity(state.return_type)
            || parameter.qualifications
                != validation::structural_result_qualifications(program, state.return_type).ok()?
        {
            return None;
        }
        if parameter.multiplicity == Multiplicity::Linear {
            let returned = validation::reconstruct_structural_parameter_return_claims(
                program,
                facts,
                machine.symbol,
                state.symbol,
                source.symbol,
            )
            .ok()?;
            let mut input = entry_claims
                .iter()
                .filter(|claim| claim.parameter_index as usize == parameter_index)
                .map(|claim| (claim.claim_identity, claim.path.clone()))
                .collect::<Vec<_>>();
            input.sort_by(|left, right| left.1.cmp(&right.1));
            if input != returned {
                return None;
            }
        }
        Some(ReturnPlan {
            source: source_plan,
            completion: checked_trees::CheckedStructuralReturnCompletion::Value {
                statement_index: root.statement_ordinal,
                expression: root.expression,
                evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            },
            type_identity: parameter.type_identity,
            multiplicity: parameter.multiplicity,
            reference_sources: if validation::reference_result_custody::is_reference_record(
                program,
                state.return_type,
            ) {
                validation::reference_result_custody::captured_return_sources(
                    program, facts, state,
                )?
            } else {
                Vec::new()
            },
        })
    }
}
pub(in crate::execution::terminal_unit) use local_data::dispatch_observation;

mod local_data {
    /*
        Planning one `let` statement of a single-state body.

        Each initializer kind has its own producer, tried in order: an erased or
        record-pattern marker plans nothing; a mutable view local establishes its
        descriptor home and stores its initializer there, a call's completed view
        included; a byte literal establishes its immutable storage; an atomic placeholder or load binds
        the atomic event; a selected boundary operator or IEEE FMA binds its
        realization; a move out of borrowed storage opens a window; a view
        subslice narrows an established view; a constructed structural value, a
        closed scalar array, a mutable primitive local and a scalar computation
        establish their binding directly. What remains is a call initializer:
        its result binding goes back to the sequence, which plans the call.

        Reference captures precede structural construction and establish their
        descriptor bindings. A silent place loan plans no operation; its row
        records the place every read and store through it rejoins. Runtime scalar
        parameters are backing roots alongside structural parameters and earlier
        locals; their view reads retain the parameter's declared carrier for
        ordinary representation-view replay.
        Passing such an alias onward still requires its backing storage to be
        established by lowering; admitting the read does not fabricate that home.
    */
    use super::super::super::atomic_operations::once_event;
    use super::super::super::atomic_operations::result_placeholder;
    use super::super::super::cleanup::anonymous::insert_continuation;
    use super::super::super::reference_results::local_owes_record_loans;
    use super::super::super::reference_results::local_record_loans;
    use super::bounded_byte_values::value_plan;
    use super::{
        CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
        CheckedUnitEffectOperationPlan, CheckedUnitScalarResultBindingPlan,
        CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, Construction,
        ExpressionNode, Multiplicity, ShapeCollector, SymbolHandle, checked_structural_result_type,
        scalar_computation_local_at, structural_operands,
    };
    use crate::execution::terminal_unit::borrowed_windows::OpenWindows;
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::calls::byte_sequence_literal_argument;
    use crate::execution::terminal_unit::calls::projected_evaluated_path;
    use crate::execution::terminal_unit::calls::value_index_segment;
    use crate::execution::terminal_unit::calls::view_subslice::admit;
    use crate::execution::terminal_unit::calls::view_subslice::view_kind;
    use crate::execution::terminal_unit::calls::view_subslice::whole_extent;
    use crate::execution::terminal_unit::initialized_locals::initialized_local;
    use crate::execution::terminal_unit::structural_scalar_store::is_shared_view_field;
    use crate::execution::terminal_unit::types::borrowed_slice_view_element;
    use crate::execution::terminal_unit::types::{
        parameter_qualifications, supports_primitive_storage,
    };
    use crate::flow::expressions;
    use crate::published::places::place_at;

    /* The sequence state a `let` statement reads and extends. */
    pub(super) struct Planner<'a, 'program, 'shapes> {
        pub(super) construction: Construction<'a, 'program, 'shapes>,
        pub(super) binders: &'a [(SymbolHandle, String)],
        pub(super) operations: &'a mut Vec<CheckedUnitEffectOperationPlan>,
        pub(super) place_loans: &'a mut Vec<checked_trees::CheckedPlaceLoanPlan>,
        pub(super) scalar_count: &'a mut usize,
        pub(super) structural_count: &'a mut usize,
        pub(super) structural_local_symbols: &'a mut Vec<SymbolHandle>,
        pub(super) windows: &'a mut OpenWindows,
        pub(super) array_bindings:
            &'a mut Vec<(SymbolHandle, CheckedUnitStructuralResultBindingPlan)>,
        pub(super) atomic_result: &'a mut Option<CheckedUnitScalarResultBindingPlan>,
        pub(super) structural_results:
            &'a mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
    }

    /* What a `let` statement left for its own call. */
    pub(super) enum LocalPlan {
        /* Every operation the statement needs is planned. */
        Planned,
        /* The initializer is a scalar call whose result binds here. */
        ScalarCall(CheckedUnitScalarResultBindingPlan),
        /* The initializer is a structural call whose whole result binds the local. */
        StructuralCall(CheckedUnitStructuralResultBindingPlan),
    }

    /*
        Restore the primary construction site when this local is declined. Its last
        attempted step is secondary diagnostic context, not proof of the cause.
        The note is tied to this state/statement and cleared by any later builder.
    */
    pub(super) fn plan(
        planner: Planner<'_, '_, '_>,
        index: usize,
        statement_index: u32,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<LocalPlan> {
        let trace = planner.construction.trace;
        let entry = trace.decline_mark();
        let planned = plan_inner(planner, index, statement_index, local);
        if planned.is_none() {
            trace.restore_with_declined_step(&entry);
        }
        planned
    }

    fn plan_inner(
        planner: Planner<'_, '_, '_>,
        index: usize,
        statement_index: u32,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<LocalPlan> {
        if local.relevance.is_erased() {
            return Some(LocalPlan::Planned);
        }
        let Planner {
            construction:
                Construction {
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    machine,
                    state,
                    structural_parameters,
                    entry_claims,
                    trivial_affine_locals,
                    trace,
                },
            binders,
            operations,
            place_loans,
            scalar_count,
            structural_count,
            structural_local_symbols,
            windows,
            array_bindings,
            atomic_result,
            structural_results,
        } = planner;
        let local_phase = |phase: &'static str| {
            trace.phase(phase);
            trace.statement(Some(statement_index));
        };
        if validation::dispatch_subject_of_erased_binding(program, state, local) {
            return Some(LocalPlan::Planned);
        }
        local_phase("statement sequence: local data: initializer expression");
        if !expressions(program).expression_is_valid(local.initial_value)
            && validation::is_atomic_type(program, local.type_reference)
        {
            local_phase("statement sequence: local data: atomic storage home");
            establish_declared_home(
                program,
                facts,
                machine,
                state,
                shapes,
                binders,
                operations,
                structural_count,
                structural_local_symbols,
                structural_results,
                index,
                statement_index,
                local,
                None,
            )?;
            return Some(LocalPlan::Planned);
        }
        let uninit_zeroed = !expressions(program).expression_is_valid(local.initial_value)
            && validation::zeroed_structural_parts(program, local.type_reference)
            && !validation::type_requires_establishment(program, local.type_reference);
        if !expressions(program).expression_is_valid(local.initial_value) && !uninit_zeroed {
            if program
                .primitive_type_reference(local.type_reference)
                .is_none()
            {
                local_phase("statement sequence: local data: deferred structural binding");
                establish_declared_home(
                    program,
                    facts,
                    machine,
                    state,
                    shapes,
                    binders,
                    operations,
                    structural_count,
                    structural_local_symbols,
                    structural_results,
                    index,
                    statement_index,
                    local,
                    None,
                )?;
                return Some(LocalPlan::Planned);
            }
            return None;
        }
        if let Some(loan) = dispatch_observation(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            statement_index,
            local,
        ) {
            place_loans.push(loan);
            return Some(LocalPlan::Planned);
        }
        let reference_capture =
            super::super::reference_captures::plan(program, facts, machine, state, index, local);
        let structural_root = facts.values.structural_values.root_for_expression(
            state.symbol,
            statement_index,
            local.initial_value,
        );
        if local.role == language_core::LocalBindingRole::RecordPattern {
            return Some(LocalPlan::Planned);
        }
        if !local.is_mutable
            && let Some(argument) =
                byte_sequence_literal_argument(program, local.type_reference, local.initial_value)
        {
            local_phase("statement sequence: local data: byte literal");
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
                bytes,
            } = argument.source
            else {
                return None;
            };
            let type_identity = shapes.add_type(local.type_reference, binders, &[])?;
            if type_identity != argument.type_identity
                || program.type_multiplicity(local.type_reference) != Multiplicity::Unrestricted
            {
                return None;
            }
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(*structural_count).ok()?,
                type_identity,
                multiplicity: Multiplicity::Unrestricted,
                local: initialized_local(program, machine, state, local),
            };
            *structural_count = structural_count.checked_add(1)?;
            structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
            structural_local_symbols.push(local.symbol);
            let qualifications =
                parameter_qualifications(program, shapes, local.type_reference, binders)?;
            operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
                    result,
                    value: checked_trees::CheckedByteSequenceValuePlan::literal(bytes),
                    qualifications,
                },
            ));
            return Some(LocalPlan::Planned);
        }
        if let Some(value) = value_plan(program, &facts.operators, state, statement_index, local) {
            local_phase("statement sequence: local data: bounded byte literal");
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(*structural_count).ok()?,
                type_identity: shapes.add_type(local.type_reference, binders, &[])?,
                multiplicity: Multiplicity::Unrestricted,
                local: initialized_local(program, machine, state, local),
            };
            *structural_count = structural_count.checked_add(1)?;
            structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
            structural_local_symbols.push(local.symbol);
            let qualifications =
                parameter_qualifications(program, shapes, local.type_reference, binders)?;
            operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
                    result,
                    value,
                    qualifications,
                },
            ));
            return Some(LocalPlan::Planned);
        }
        if validation::atomic_once_carrier(program, local.initial_value).is_some() {
            local_phase("statement sequence: local data: single-attempt atomic result");
            let mut result =
                checked_structural_result_type(program, shapes, local.type_reference, binders)?;
            result.statement_index = statement_index;
            result.binding_ordinal = u32::try_from(*structural_count).ok()?;
            result.local = initialized_local(program, machine, state, local);
            let access = once_event(
                program,
                facts,
                machine,
                state,
                statement_index,
                local,
                result.clone(),
                trace,
                shapes,
            )?;
            operations.push(CheckedUnitEffectOperationPlan::AtomicAccess(access));
            *structural_count = structural_count.checked_add(1)?;
            structural_results.push((result, facts::PlaceRoot::Symbol(local.symbol)));
            structural_local_symbols.push(local.symbol);
            return Some(LocalPlan::Planned);
        }
        if let Some(primitive_type) = program.primitive_type_reference(local.type_reference)
            && result_placeholder(program, state, index, local)
        {
            local_phase("statement sequence: local data: atomic result placeholder");
            *atomic_result = Some(CheckedUnitScalarResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(*scalar_count).ok()?,
                primitive_type,
            });
            *scalar_count = scalar_count.checked_add(1)?;
            return Some(LocalPlan::Planned);
        }
        local_phase("statement sequence: local data: borrowed window move");
        if let Some((operation, result)) = windows.move_out(
            program,
            facts,
            shapes,
            machine,
            state,
            structural_parameters,
            binders,
            statement_index,
            local,
            u32::try_from(*structural_count).ok()?,
        ) {
            *structural_count = structural_count.checked_add(1)?;
            structural_results.push((result, facts::PlaceRoot::Symbol(local.symbol)));
            structural_local_symbols.push(local.symbol);
            operations.push(operation);
            return Some(LocalPlan::Planned);
        }
        local_phase("statement sequence: local data: view subslice");
        let declared_is_view = view_kind(program, local.type_reference).is_some()
            || borrowed_slice_view_element(program, local.type_reference, &[]).is_some();
        let subslice_target = if declared_is_view {
            local.type_reference
        } else {
            validation::expression_result_type_reference(
                program,
                machine,
                state,
                local.initial_value,
            )
            .filter(|result| validation::slice_view_shell(program, *result, &[]).is_some())
            .unwrap_or(local.type_reference)
        };
        let subslice = structural_root
            .is_none()
            .then(|| {
                admit(
                    program,
                    facts,
                    machine,
                    state,
                    structural_parameters,
                    subslice_target,
                    local.initial_value,
                    index,
                    checked_trees::CheckedSubsliceSite::LocalBinding,
                )
                .map(|subslice| (subslice, None))
                .or_else(|| {
                    declared_is_view.then(|| {
                        whole_extent(
                            program,
                            machine,
                            state,
                            structural_parameters,
                            subslice_target,
                            local.initial_value,
                            index,
                        )
                        .map(|(subslice, storage)| (subslice, Some(storage)))
                    })?
                })
            })
            .flatten();
        if let Some((subslice, storage)) = subslice {
            if let checked_trees::CheckedStorageRoot::ViewLocal { symbol } = subslice.range.root
                && !structural_local_symbols.contains(&symbol)
            {
                return None;
            }
            let type_identity = shapes.add_type(subslice_target, binders, &[])?;
            if type_identity != subslice.range.type_identity {
                return None;
            }
            if !declared_is_view
                && program.normalized_type_identity(local.type_reference)
                    != validation::slice_view_shell(program, subslice_target, &[])
                        .map(|view| program.normalized_type_identity(view.element))?
            {
                return None;
            }
            if let Some(storage) = storage {
                shapes.add_type(storage, binders, &[])?;
            }
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(*structural_count).ok()?,
                type_identity,
                multiplicity: match subslice.range.access {
                    CheckedStructuralAccess::MutableBorrow => Multiplicity::Affine,
                    _ => Multiplicity::Unrestricted,
                },
                local: initialized_local(program, machine, state, local),
            };
            *structural_count = structural_count.checked_add(1)?;
            structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
            structural_local_symbols.push(local.symbol);
            operations.push(CheckedUnitEffectOperationPlan::EstablishViewSubslice {
                result,
                source: subslice.argument(),
            });
            return Some(LocalPlan::Planned);
        }
        if structural_root.is_none()
            && let Some(capture) = reference_capture
        {
            local_phase("statement sequence: local data: reference capture");
            shapes.add_stored_type(capture.backing_type, binders, &[])?;
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(*structural_count).ok()?,
                type_identity: shapes.add_reference_type(local.type_reference, binders)?,
                multiplicity: Multiplicity::Affine,
                local: initialized_local(program, machine, state, local),
            };
            *structural_count = structural_count.checked_add(1)?;
            structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
            structural_local_symbols.push(local.symbol);
            operations.push(CheckedUnitEffectOperationPlan::EstablishReference {
                formation_expression: local.initial_value,
                backing_type: capture.backing_type,
                source_contract: crate::values::reference_sources::qualification_contract(
                    program,
                    local.type_reference,
                    capture.backing_type,
                    capture.storage,
                ),
                result,
                source: capture.source,
                storage: capture.storage,
            });
            return Some(LocalPlan::Planned);
        }
        local_phase("statement sequence: local data: place loan");
        if structural_root.is_none()
            && let Some(loan) = place_loan(
                program,
                facts,
                machine,
                state,
                structural_parameters,
                statement_index,
                local,
            )
        {
            if let PlaceLoan::Captured(loan) = loan {
                place_loans.push(loan);
            }
            return Some(LocalPlan::Planned);
        }
        if let Some(root) = structural_root {
            local_phase("statement sequence: local data: structural value: reference record loans");
            if local_owes_record_loans(program, local.type_reference) {
                local_record_loans(program, facts, machine.symbol, state, statement_index)?;
            }
            local_phase("statement sequence: local data: structural value: root identity");
            if root.machine != machine.symbol
                || root.expression != local.initial_value
                || root.type_reference != local.type_reference
            {
                return None;
            }
            local_phase("statement sequence: local data: structural value: operand custody");
            let (calls, completion) = super::prepare_value_operands(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                state,
                structural_parameters,
                trivial_affine_locals,
                entry_claims,
                statement_index,
                structural_results,
                operations,
                structural_count,
                root.root,
                trace,
                windows,
            )?;
            if completion == checked_trees::CheckedValueCompletion::NoNormal {
                operations.push(CheckedUnitEffectOperationPlan::EvaluateStructuralValue {
                    statement_index,
                    value: root.root,
                    calls,
                    operand_source: None,
                });
                return Some(LocalPlan::Planned);
            }
            local_phase("statement sequence: local data: structural value: type shape");
            let type_identity = shapes.add_stored_type(local.type_reference, binders, &[])?;
            parameter_qualifications(program, shapes, local.type_reference, binders)?;
            let mut result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(*structural_count).ok()?,
                type_identity,
                multiplicity: validation::reference_result_custody::result_multiplicity(
                    program,
                    local.type_reference,
                ),
                local: initialized_local(program, machine, state, local),
            };
            *structural_count = structural_count.checked_add(1)?;
            let stable_home = local.is_mutable;
            let owner = if stable_home {
                result.local = Default::default();
                facts::PlaceRoot::Expression(local.initial_value)
            } else {
                structural_local_symbols.push(local.symbol);
                facts::PlaceRoot::Symbol(local.symbol)
            };
            structural_results.push((result.clone(), owner));
            let discard_result_on_return =
                !stable_home && result.multiplicity == Multiplicity::Affine;
            let establishment = structural_operands::establish_value(
                facts,
                result.clone(),
                root.root,
                calls,
                stable_home.then_some(checked_trees::CheckedArrayConstructionSource::Statement),
                discard_result_on_return,
                operations,
            )?;
            let call_coordinate = establishment.call().map(|call| call.coordinate);
            operations.push(establishment);
            if let Some(coordinate) = call_coordinate {
                insert_continuation(
                    program,
                    facts,
                    shapes,
                    machine,
                    state,
                    coordinate,
                    operations.len(),
                    structural_results,
                    operations,
                    trace,
                )?;
            }
            if stable_home {
                establish_declared_home(
                    program,
                    facts,
                    machine,
                    state,
                    shapes,
                    binders,
                    operations,
                    structural_count,
                    structural_local_symbols,
                    structural_results,
                    index,
                    statement_index,
                    local,
                    Some(
                        checked_trees::CheckedStructuralLocalInitialization::completed_result(
                            &result, root.root,
                        ),
                    ),
                )?;
            }
            return Some(LocalPlan::Planned);
        }
        if validation::scalar_array_preserves_memberships(program, local.type_reference)
            && !matches!(
                program.expression_table.expression(local.initial_value),
                ExpressionNode::Call(_)
            )
        {
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(*structural_count).ok()?,
                type_identity: shapes.add_type(local.type_reference, binders, &[])?,
                multiplicity: program.type_multiplicity(local.type_reference),
                local: initialized_local(program, machine, state, local),
            };
            local_phase("statement sequence: local data: scalar array: elements");
            let elements = super::super::scalar_arrays::elements(
                program,
                facts,
                machine.symbol,
                state.symbol,
                statement_index,
                checked_trees::CheckedArrayConstructionSource::Statement,
                local.initial_value,
                local.type_reference,
            )?;
            *structural_count = structural_count.checked_add(1)?;
            array_bindings.push((local.symbol, result.clone()));
            structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
            structural_local_symbols.push(local.symbol);
            let qualifications = facts.qualifications.formation_domains_at(
                machine.symbol,
                state.symbol,
                statement_index,
                local.initial_value,
            );
            for domain in &qualifications {
                shapes.add_domain(*domain, local.type_reference, binders)?;
            }
            operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                    qualifications,
                    source: checked_trees::CheckedArrayConstructionSource::Statement,
                    result,
                    elements,
                },
            ));
            return Some(LocalPlan::Planned);
        }
        if let Some(primitive_type) = program.primitive_type_reference(local.type_reference) {
            if local.is_mutable {
                local_phase(
                    "statement sequence: local data: mutable primitive local: storage type",
                );
                if !supports_primitive_storage(program, local.type_reference) {
                    return None;
                }
                let value = super::storage_initializer::value(
                    program,
                    facts,
                    machine.symbol,
                    state,
                    statement_index,
                    local,
                    primitive_type,
                    trace,
                )?;
                operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Primitive {
                        statement_index,
                        symbol: local.symbol,
                        type_identity: shapes.add_type(local.type_reference, binders, &[])?,
                        primitive_type,
                        value,
                    },
                ));
                return Some(LocalPlan::Planned);
            }
            let binding_ordinal = u32::try_from(*scalar_count).ok()?;
            *scalar_count = scalar_count.checked_add(1)?;
            let source_ordinal = u32::try_from(
                program
                    .immutable_scalar_locals_before(state, usize::try_from(statement_index).ok()?),
            )
            .ok()?;
            if !matches!(
                program.expression_table.expression(local.initial_value),
                ExpressionNode::Call(_)
            ) || facts
                .values
                .scalar_computations
                .root_at(
                    state.symbol,
                    statement_index,
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: source_ordinal,
                    },
                )
                .is_some()
            {
                let (mut result, value) = scalar_computation_local_at(
                    program,
                    facts,
                    machine.symbol,
                    state,
                    statement_index,
                    source_ordinal,
                    local,
                    trace,
                )?;
                result.binding_ordinal = binding_ordinal;
                operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Scalar { result, value },
                ));
                return Some(LocalPlan::Planned);
            }
            local_phase("statement sequence: local data: scalar call binding");
            Some(LocalPlan::ScalarCall(CheckedUnitScalarResultBindingPlan {
                statement_index,
                binding_ordinal,
                primitive_type,
            }))
        } else {
            local_phase("statement sequence: local data: structural call binding");
            if !matches!(
                program.expression_table.expression(local.initial_value),
                ExpressionNode::Call(_)
            ) {
                return None;
            }
            local_phase("statement sequence: local data: structural result shape");
            let mut result =
                checked_structural_result_type(program, shapes, local.type_reference, binders)?;
            result.statement_index = statement_index;
            result.binding_ordinal = u32::try_from(*structural_count).ok()?;
            result.local = initialized_local(program, machine, state, local);
            Some(LocalPlan::StructuralCall(result))
        }
    }

    // A private case observation reads the original inline storage. It does not
    // need a descriptor that would keep a separate child loan alive in the arm.
    // Keep ordinary reference production whenever the captured value can escape.
    pub(in crate::execution::terminal_unit) fn dispatch_observation(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        statement_index: u32,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<checked_trees::CheckedPlaceLoanPlan> {
        use typed_trees::expression::BinaryOperator;
        use typed_trees::statement::StatementRootRole;

        if !local.symbol.is_valid()
            || local.role != language_core::LocalBindingRole::DispatchObservation
        {
            return None;
        }
        let PlaceLoan::Captured(loan) = place_loan(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            statement_index,
            local,
        )?
        else {
            return None;
        };
        if loan.access != CheckedStructuralAccess::SharedBorrow
            || validation::reference_shell(program, loan.referee).is_some()
            || loan.recast_source.is_valid()
            || loan.path.iter().any(|step| {
                !matches!(
                    step,
                    checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                        | checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                )
            })
        {
            return None;
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        let following = statements.get(usize::try_from(statement_index).ok()?.checked_add(1)?..)?;
        if following.is_empty() {
            return None;
        }
        let mut visits = std::collections::HashMap::<_, _, symbols::BuildSymbolHasher>::default();
        let mut epoch = 0u32;
        for statement in following {
            statement.as_transition()?;
            for (role, root) in program.tables.expression_roots(statement) {
                epoch = epoch.checked_add(1)?;
                let mut pending = vec![(root, false)];
                while let Some((expression, leaving)) = pending.pop() {
                    if !expression.is_valid() {
                        continue;
                    }
                    if !expressions(program).expression_is_valid(expression) {
                        return None;
                    }
                    if leaving {
                        visits.insert(expression, (epoch, true));
                        continue;
                    }
                    if let Some((seen, complete)) = visits.get(&expression)
                        && *seen == epoch
                    {
                        if *complete {
                            continue;
                        }
                        return None;
                    }
                    visits.insert(expression, (epoch, false));
                    let node = expressions(program).expression(expression);
                    if matches!(node, ExpressionNode::Name(path)
                        if path.resolved_symbol() == Some(local.symbol))
                    {
                        return None;
                    }
                    pending.push((expression, true));
                    if role == StatementRootRole::TransitionGuard
                        && let ExpressionNode::Binary(binary) = node
                        && binary.operator == BinaryOperator::CaseMembership
                        && let ExpressionNode::Name(path) =
                            expressions(program).expression(binary.left)
                        && path.resolved_symbol() == Some(local.symbol)
                        && expressions(program).name_path_members(path.members).len() == 1
                    {
                        pending.push((binary.right, false));
                    } else if let ExpressionNode::SavedSubject(saved) = node {
                        // Inspect the dependency without evaluating its producer.
                        let source = program.saved_value_source(saved)?;
                        if !expressions(program).expression_is_valid(source) {
                            return None;
                        }
                        pending.push((source, false));
                    } else {
                        validation::expression_traversal::children(program, node, |child| {
                            pending.push((child, false));
                        });
                    }
                }
            }
        }
        Some(loan)
    }

    /*
        A silent loan's admission: the place it names, or, for a recast whose
        place the checked walk cannot name, admission alone.
    */
    enum PlaceLoan {
        Captured(checked_trees::CheckedPlaceLoanPlan),
        /*
            The §5b recast judgment admitted the loan, but its place carries a
            selector no checked coordinate names (`self.bytes[k * 2]`). No use of
            it can rejoin a place, so lowering refuses each one.
        */
        Unnamed,
    }

    /*
        A `let` binding a loan of a place: `let seen: &mut u8 = &mut self.byte;`
        or `let borrowed: &u32 = &self.size;`. Write-only loans retain the same
        place without granting read access. The binding is immutable and a
        reference cannot be reseated, so the name denotes one place for its whole
        scope and carries no runtime value of its own: every read through it is a
        read of that place, every write through an exclusive loan is the ordinary
        store of that place, and the borrow layer owns the loan's lifetime. The
        statement therefore plans no operation, like a record pattern's marker;
        its row records the place lowering rejoins each use to.

        The conditions are what make that rejoin exact, not a convenience. The
        place must root at a structural parameter of this state or at an earlier
        local of this state — the borrow ledger names the loaned storage in
        either spelling — and its projection must bottom out in exactly the
        declared referent: a loan of a wider or narrower leaf is not this local's
        place. A shared loan may also alias a runtime scalar parameter; it has no
        write authority, and the read keeps the parameter's declared carrier. A
        runtime index stays a `RuntimeIndex` segment for the terminal's bound
        re-proof. A `&place as &T` recast (either access) skips the referent
        equality: the §5b judgment already proved the two shapes compatible. The
        referent may be a record as well as a primitive.
    */
    fn place_loan(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        statement_index: u32,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<PlaceLoan> {
        if local.is_mutable {
            return None;
        }
        let formation = validation::reference_formation_source(program, local.initial_value)?;
        let shell = validation::reference_shell(program, local.type_reference)?;
        if shell.access != formation.access {
            return None;
        }
        let access = CheckedStructuralAccess::from(formation.access);
        let referee = shell.referee;
        let statement = usize::try_from(statement_index).ok()?;
        let place = place_at(program, state.symbol, statement, formation.source)?;
        let symbol = place.root.as_symbol()?;
        let parameters = program.state_parameters(state);
        let (root, root_type) =
            match typed_trees::signature::state_parameter_position(parameters, symbol) {
                Some(position) => {
                    let parameter = &parameters[position];
                    let root = match structural_parameters
                        .iter()
                        .position(|candidate| candidate.position as usize == position)
                    {
                        Some(dense) => checked_trees::CheckedPlaceLoanRoot::StructuralParameter {
                            parameter_index: u32::try_from(dense).ok()?,
                        },
                        None if access == CheckedStructuralAccess::SharedBorrow
                            && !parameter.is_const
                            && !parameter.relevance.is_erased()
                            && program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some() =>
                        {
                            checked_trees::CheckedPlaceLoanRoot::ScalarParameter {
                                position: u32::try_from(position).ok()?,
                            }
                        }
                        None => return None,
                    };
                    (root, parameter.type_reference)
                }
                None => {
                    let (ordinal, root_local) = program.state_local_declaration(state, symbol)?;
                    if ordinal >= statement {
                        return None;
                    }
                    (
                        checked_trees::CheckedPlaceLoanRoot::Local { symbol },
                        root_local.type_reference,
                    )
                }
            };
        let mut root_type = root_type;
        while let typed_trees::types::TypeReferenceNode::Reference { referee, .. } =
            program.type_reference_table.type_reference(root_type)
        {
            root_type = *referee;
        }
        let recast_source = if formation.view_type.is_valid() {
            validation::declared_place_type_raw(program, machine, Some(state), formation.source)
                .filter(|source| source.is_valid())?
        } else {
            typed_trees::types::TypeReferenceHandle::default()
        };
        let projected = projected_evaluated_path(
            program,
            machine.symbol,
            state.symbol,
            statement,
            &place,
            Some(&|expression| {
                value_index_segment(program, &facts.operators, state, statement, expression)
            }),
        );
        let path = match projected {
            Some(ProjectedArgumentPath {
                storage_type: storage,
                path,
                ..
            }) => {
                if !recast_source.is_valid()
                    && program.normalized_type_identity(storage)
                        != program.normalized_type_identity(referee)
                {
                    return None;
                }
                path
            }
            None if recast_source.is_valid() => return Some(PlaceLoan::Unnamed),
            None => return None,
        };
        let decided_byte_view_store = matches!(
            access,
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
        ) && (validation::validate_literal_indexed_recast_footprint(
            program,
            machine,
            state,
            statement,
            local.initial_value,
            referee,
            formation.access,
        )
        .is_some()
            || validation::validate_runtime_indexed_recast_source(
                program,
                machine,
                state,
                statement,
                local.initial_value,
                referee,
                formation.access,
            )
            .is_some());
        Some(PlaceLoan::Captured(checked_trees::CheckedPlaceLoanPlan {
            statement_index,
            symbol: local.symbol,
            access,
            referee,
            root,
            root_type,
            path,
            recast_source,
            decided_byte_view_store,
        }))
    }

    /*
        Publish a declared local's home before any member or whole-slot store
        establishes its contents. Borrows and writers rejoin the binding by
        ordinal the same way established values do.
    */
    #[allow(clippy::too_many_arguments)]
    fn establish_declared_home(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        shapes: &mut ShapeCollector<'_>,
        binders: &[(SymbolHandle, String)],
        operations: &mut Vec<CheckedUnitEffectOperationPlan>,
        structural_count: &mut usize,
        structural_local_symbols: &mut Vec<SymbolHandle>,
        structural_results: &mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
        declaration_ordinal: usize,
        statement_index: u32,
        local: &typed_trees::statement::TableLocalData,
        initial_value: Option<checked_trees::CheckedStructuralLocalInitialization>,
    ) -> Option<()> {
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: shapes.add_stored_type(local.type_reference, binders, &[])?,
            multiplicity: validation::reference_result_custody::result_multiplicity(
                program,
                local.type_reference,
            ),
            local: initialized_local(program, machine, state, local),
        };
        if let Some(initial) = &initial_value {
            let mut sources = operations.iter().filter(|operation| {
                operation
                    .structural_result_binding()
                    .is_some_and(|binding| {
                        binding.binding_ordinal == initial.source_binding_ordinal()
                    })
            });
            let source = sources.next()?;
            if sources.next().is_some() || !result.local.is_named() {
                return None;
            }
            let binding = source.structural_result_binding()?;
            let (value, calls) = source.structural_value_evaluation()?;
            if value != initial.value
                || binding.statement_index != statement_index
                || binding.type_identity != result.type_identity
                || binding.multiplicity != result.multiplicity
                || facts.values.structural_values.completion(value, calls)?
                    == checked_trees::CheckedValueCompletion::NoNormal
            {
                return None;
            }
        }
        *structural_count = structural_count.checked_add(1)?;
        let qualifications = if is_shared_view_field(program, local.type_reference) {
            parameter_qualifications(program, shapes, local.type_reference, binders)?
        } else {
            Vec::new()
        };
        operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::Structural {
                statement_index,
                declaration_ordinal: u32::try_from(declaration_ordinal).ok()?,
                symbol: local.symbol,
                result: result.clone(),
                initial_value,
                qualifications,
                discard_result_on_return: result.multiplicity == Multiplicity::Affine,
            },
        ));
        structural_results.push((result, facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        Some(())
    }
}
mod reference_retirement {

    /*
        Exact holder retirement and owned operands used by statement scheduling.
        The checked value graph supplies constructor transfers and copied-carrier
        correspondence; loan formation supplies immediate parent dependencies.
        Transport preserves those parents without creating a new reborrow, and
        retirement never infers custody from the spelling of an initializer.
    */
    use super::{
        CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan,
        SymbolHandle,
    };
    use arena::Handle;
    use arena::HandleSpan;
    use checked_trees::{
        BorrowLoanFact, BorrowLoanLineage, BorrowLoanOwnerSegment, CheckedCleanupCaseCondition,
        CheckedStructuralReturnCompletion as ReturnCompletion,
        CheckedUnitCleanupBoundary as CleanupBoundary, CheckedUnitLocalBindingPlan,
        CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
        CheckedUnitStructuralReturnPlan, FlowBorrowWeakeningReason, FlowInvalidationSource,
        FlowPermissionEvaluation, StateBorrowFact,
    };
    use facts::FactContextHandle;
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use language_semantics::PermissionEventSource;
    use typed_trees::state::State;
    use typed_trees::statement::StatementExecutionOrder;
    use validation::CanonicalPlace;
    use validation::ContentsRequirement;
    use validation::canonical_place_type_reference;
    use validation::has_contents;
    use validation::reference_result_custody::StoredCarrierSource;
    use validation::reference_result_custody::loan_place_segments;
    use validation::reference_result_custody::loan_weakening;
    use validation::reference_result_custody::release_boundary;
    use validation::reference_result_custody::storage_owner_type_reference;
    use validation::reference_result_custody::storage_place_loans;
    use validation::reference_result_custody::stored_carrier_sources;
    use validation::reference_shell;
    use validation::transition_call_target;

    type Results = [(CheckedUnitStructuralResultBindingPlan, PlaceRoot)];

    mod completion {
        /*
            A final call must finish using local data before that data is released.
            Follow each returning branch separately: a call can consume a holder on
            one branch while another branch leaves it for checked exit cleanup.
        */
        use super::{BorrowLoanFact, Handle, Retirement, RetirementNode};
        use checked_trees::{
            CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
            FlowClaimJoinExit,
        };
        use language_semantics::PermissionEventSource;
        use typed_trees::types::TypeReferenceHandle;

        struct CompletedDescriptorSources {
            formations: Vec<Handle<BorrowLoanFact>>,
            call_descriptors: Vec<validation::completed_storage::CompletedCallDescriptor>,
        }

        impl Retirement<'_> {
            /*
                The final slot can contain a descriptor from any completed branch,
                including the old value on a skipped-call path. Follow the exact
                predecessors recorded by checking; equal backing is not identity.
            */
            fn completed_descriptor_sources(
                &self,
                handle: Handle<BorrowLoanFact>,
            ) -> Result<CompletedDescriptorSources, &'static str> {
                let borrow = &self.facts.borrow;
                let loan = borrow.loans.get(handle);
                let owner_path = borrow
                    .owner_segments
                    .span(loan.owner_path)
                    .ok_or("completion cleanup: the descriptor has no retained owner path")?;
                let mut pending = self
                    .resource_loans()
                    .ok_or("completion cleanup: the body has no unique resource roster")?
                    .filter_map(|(handle, candidate)| {
                        (candidate.storage_owner == loan.storage_owner
                            && candidate.statement_index == loan.statement_index
                            && borrow.owner_segments.span(candidate.owner_path) == Some(owner_path))
                        .then_some((handle, false))
                    })
                    .collect::<Vec<_>>();
                if pending.is_empty() {
                    return Err(
                        "completion cleanup: the descriptor has no exact slot alternatives",
                    );
                }
                let Some(Some(state)) =
                    super::unique_borrow_state(self.facts, self.machine, self.state.symbol)
                else {
                    return Err("completion cleanup: the body has no unique borrow state");
                };
                let mut active = Vec::new();
                let mut completed = Vec::new();
                let mut output = CompletedDescriptorSources {
                    formations: Vec::new(),
                    call_descriptors: Vec::new(),
                };
                while let Some((source, leaving)) = pending.pop() {
                    if leaving {
                        active.pop();
                        completed.push(source);
                        continue;
                    }
                    if completed.contains(&source) {
                        continue;
                    }
                    if active.contains(&source) || !borrow.state_owns_loan(state, source) {
                        return Err(
                            "completion cleanup: descriptor predecessors cycle or leave this state",
                        );
                    }
                    active.push(source);
                    pending.push((source, true));
                    let occurrence = borrow.loans.get(source);
                    if occurrence.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency {
                        continue;
                    }
                    if occurrence.established_by_call.is_valid() {
                        output
                            .call_descriptors
                            .extend(self.call_descriptor_sources(source)?);
                    } else if occurrence.formation_expression.is_valid()
                        && occurrence.formation_owner_path.is_some()
                    {
                        output.formations.push(source);
                    } else {
                        let sources = borrow
                            .loan_suppliers
                            .span(occurrence.supplying_loans)
                            .ok_or("completion cleanup: descriptor predecessors are unavailable")?;
                        if sources.is_empty() {
                            return Err(
                                "completion cleanup: the descriptor has no call or formation predecessor",
                            );
                        }
                        for source in sources {
                            if !borrow.state_owns_loan(state, *source) {
                                return Err(
                                    "completion cleanup: a descriptor predecessor belongs to another state",
                                );
                            }
                            let previous = borrow.loans.get(*source);
                            if previous.storage_owner != occurrence.storage_owner
                                || borrow.owner_segments.span(previous.owner_path)
                                    != borrow.owner_segments.span(occurrence.owner_path)
                                || previous.kind != occurrence.kind
                            {
                                return Err(
                                    "completion cleanup: a predecessor names different storage or access",
                                );
                            }
                            pending.push((*source, false));
                        }
                    }
                }
                Ok(output)
            }

            /*
                A call-written descriptor is the value in that exact invocation's
                completed slot, not an expression with the same statement number.
            */
            fn call_descriptor_sources(
                &self,
                handle: Handle<BorrowLoanFact>,
            ) -> Result<Vec<validation::completed_storage::CompletedCallDescriptor>, &'static str>
            {
                let borrow = &self.facts.borrow;
                let loan = borrow.loans.get(handle);
                if !borrow.calls.is_valid(loan.established_by_call) {
                    return Err(
                        "completion cleanup: the descriptor has no checked establishing call",
                    );
                }
                let call = borrow.calls.get(loan.established_by_call);
                if borrow.call_at(
                    self.machine,
                    self.state.symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) != Some(loan.established_by_call)
                    || !borrow.loans_contain(call.completed_reference_loans, handle)
                {
                    return Err(
                        "completion cleanup: the descriptor does not belong to its exact checked call",
                    );
                }
                let slots = borrow
                    .reference_slot_effects
                    .span(call.reference_slot_effects)
                    .ok_or("completion cleanup: the checked call has no retained slot effects")?;
                if !slots.iter().any(|slot| {
                    slot.root == loan.storage_owner
                        && borrow
                            .owner_segments
                            .span(slot.owner_path)
                            .is_some_and(|prefix| borrow.loan_owner_path(loan).starts_with(prefix))
                }) {
                    return Err(
                        "completion cleanup: the checked call does not establish this descriptor slot",
                    );
                }
                validation::completed_storage::loan_call_descriptors(
                    self.program,
                    self.facts,
                    self.machine,
                    self.state.symbol,
                    handle,
                )?
                .ok_or("completion cleanup: this call has no authenticated descriptor sources")
            }

            /*
                A skipped call leaves an earlier formation in the same slot. Check
                every stored alternative against its own ordinary formation or exact
                completed call; one matching alternative cannot cover another one.
            */
            fn completed_descriptor_matches(
                &self,
                stored: validation::completed_storage::StoredOperand,
                sources: &CompletedDescriptorSources,
                statements: arena::HandleSpan<checked_trees::FlowStatementFact>,
            ) -> Result<bool, &'static str> {
                use validation::completed_storage::StoredOperand;
                match stored {
                    StoredOperand::Unavailable => Ok(false),
                    StoredOperand::Alternatives(alternatives) => {
                        if alternatives.is_empty() {
                            return Ok(false);
                        }
                        for alternative in alternatives {
                            if !self.completed_descriptor_matches(
                                alternative,
                                sources,
                                statements,
                            )? {
                                return Ok(false);
                            }
                        }
                        Ok(true)
                    }
                    stored => {
                        if !sources.call_descriptors.is_empty()
                            && validation::completed_storage::matches_call_descriptor_frontier(
                                self.program,
                                stored.clone(),
                                &sources.call_descriptors,
                            )?
                        {
                            return Ok(true);
                        }
                        let Some(captures) = stored.captures() else {
                            return Ok(false);
                        };
                        Ok(!captures.is_empty()
                            && captures.iter().all(|stored| {
                                sources.formations.iter().any(|source| {
                                validation::completed_storage::descriptor_formation_matches_loan(
                                    self.program,
                                    &self.facts.flow.control,
                                    &self.facts.borrow,
                                    self.state.symbol,
                                    statements,
                                    self.facts.borrow.loans.get(*source),
                                    stored.capture,
                                    &stored.projection,
                                ) == Some(true)
                            })
                            }))
                    }
                }
            }

            fn completion_refusal(&self, reason: &'static str, loan: Handle<BorrowLoanFact>) {
                self.trace.phase(reason);
                if !::validation::trace_flags::check_sites() {
                    return;
                }
                eprintln!(
                    "CHECKSITE completion-cleanup-refusal machine={:?} state={:?} loan={loan:?} reason={reason}",
                    self.machine, self.state.symbol
                );
                let borrow = &self.facts.borrow;
                let mut pending = vec![loan];
                let mut visited = Vec::new();
                while let Some(handle) = pending.pop() {
                    if visited.contains(&handle) || !borrow.loans.is_valid(handle) {
                        continue;
                    }
                    visited.push(handle);
                    let row = borrow.loans.get(handle);
                    let suppliers = borrow.loan_suppliers.span(row.supplying_loans);
                    eprintln!(
                        "CHECKSITE completion-cleanup-predecessor loan={handle:?} occurrence={row:?} owner_path={:?} suppliers={suppliers:?}",
                        borrow.owner_segments.span(row.owner_path)
                    );
                    if let Some(suppliers) = suppliers {
                        pending.extend_from_slice(suppliers);
                    }
                }
            }

            pub(super) fn completion_retires_holder(
                &self,
                loan: Handle<BorrowLoanFact>,
                source: CheckedUnitStructuralArgumentSourcePlan,
                formation: u32,
                selection: &[CheckedUnitStructuralPathSegment],
                reference: TypeReferenceHandle,
                active: &mut Vec<RetirementNode>,
            ) -> bool {
                let Some(completion) = self.completion.filter(|_| self.at_exit) else {
                    if self.at_exit {
                        self.completion_refusal(
                            "completion cleanup: the body has no prepared completion plan",
                            loan,
                        );
                    }
                    return false;
                };
                let transfers =
                    match crate::execution::terminal_unit::storage_transfers::at_completion(
                        self.program,
                        self.facts,
                        self.prepared_calls,
                        self.results,
                        source,
                        formation,
                        selection,
                        self.operations,
                        completion,
                    ) {
                        Ok(transfers) => transfers,
                        Err(reason) => {
                            self.trace.phase(reason);
                            return false;
                        }
                    };
                if transfers.exits.is_empty() {
                    self.trace
                        .phase("completion cleanup: the completion plan has no normal exits");
                    return false;
                }
                let Ok(exits) = crate::execution::terminal_unit::cleanup::normal_edges::state_exits(
                    self.program,
                    self.facts,
                    self.machine,
                    self.state,
                ) else {
                    self.trace
                        .phase("completion cleanup: the body has no checked normal exits");
                    return false;
                };
                transfers.exits.iter().all(|(exit, selected)| {
                    if !exits.contains(exit) {
                        self.trace.phase(
                            "completion cleanup: the selected completion has no exact checked exit",
                        );
                        return false;
                    }
                    self.transferred_holder_retired(
                        loan,
                        reference,
                        &[&transfers.prefix, selected],
                        Some(*exit),
                        active,
                    )
                })
            }

            /*
                A value can remain in storage after its last read. Use the completed
                storage snapshot to identify it at cleanup; the active-loan inventory
                has already expired loans that no later expression needs to read.
            */
            pub(super) fn exit_cleanup_retires_holder(
                &self,
                handle: Handle<BorrowLoanFact>,
                reference: TypeReferenceHandle,
                exit: FlowClaimJoinExit,
            ) -> bool {
                if !self.at_exit {
                    return false;
                }
                let sources = match self.completed_descriptor_sources(handle) {
                    Ok(sources) => sources,
                    Err(reason) => {
                        self.completion_refusal(reason, handle);
                        return false;
                    }
                };
                let loan = self.facts.borrow.loans.get(handle);
                if loan.storage_owner == self.uncommitted_owner {
                    self.trace
                        .phase("completion cleanup: this owner still needs its saved access");
                    return false;
                }
                let Some(path) = super::transport::carrier_path(
                    self.program,
                    reference,
                    self.facts.borrow.loan_owner_path(loan),
                ) else {
                    return false;
                };
                let Some(path) = super::loan_place_segments(self.program, reference, &path) else {
                    return false;
                };
                let Some(state) = self
                    .facts
                    .flow
                    .control
                    .state(self.machine, self.state.symbol)
                else {
                    self.trace
                        .phase("completion cleanup: the body has no retained flow state");
                    return false;
                };
                let completed =
                    match crate::execution::terminal_unit::cleanup::normal_edges::state_exit_storage(
                        self.program,
                        self.facts,
                        self.machine,
                        self.state,
                        exit,
                    ) {
                        Ok(snapshots) => snapshots,
                        Err(reason) => {
                            self.completion_refusal(reason, handle);
                            return false;
                        }
                    };
                let control = &self.facts.flow.control;
                let storage = validation::CanonicalPlace {
                    root: loan.storage_owner,
                    segments: path.clone(),
                };
                if !completed.iter().all(|snapshot| {
                    let Some(stored) = validation::completed_storage::sources_at_snapshot(
                        self.program,
                        control,
                        *snapshot,
                        &storage,
                        validation::completed_storage::StoredObservation::Binding,
                    ) else {
                        if ::validation::trace_flags::check_sites() {
                            eprintln!(
                                "CHECKSITE completion-cleanup-storage-unavailable loan={handle:?} exit={exit:?} snapshot={snapshot:?} storage={storage:?}"
                            );
                        }
                        return false;
                    };
                    if ::validation::trace_flags::check_sites() {
                        eprintln!(
                            "CHECKSITE completion-cleanup-storage loan={handle:?} exit={exit:?} snapshot={snapshot:?} captures={:?} call_sources={} formations={:?}",
                            stored.clone().captures().map(|values| values.into_iter().map(|value| (value.capture, value.projection)).collect::<Vec<_>>()),
                            sources.call_descriptors.len(), sources.formations,
                        );
                    }
                    match self.completed_descriptor_matches(stored, &sources, state.statements) {
                        Ok(matches) => matches,
                        Err(reason) => {
                            self.completion_refusal(reason, handle);
                            false
                        }
                    }
                }) {
                    self.completion_refusal("completion cleanup: the selected return has no matching stored occurrence of this saved access", handle);
                    self.trace
                        .statement(u32::try_from(exit.statement_index).ok());
                    return false;
                }
                let covered = self.facts.contextual_cleanup.any_state_disposition(
                    self.machine,
                    self.state.symbol,
                    |_, disposition| {
                        if disposition.source != PermissionEventSource::StateExit
                            || disposition.exit != exit
                            || disposition.root != loan.storage_owner
                            || disposition.zero_image
                        {
                            return false;
                        }
                        let Some(selected) = self.facts.flow.ownership.segments.span(disposition.path) else {
                            return false;
                        };
                        if selected.len() > path.len()
                            || !selected.iter().zip(&path).all(|(left, right)| {
                                validation::storage_path_relations::canonical_place_segments_equal(self.program, *left, *right)
                            })
                        {
                            return false;
                        }
                        let Some(excluded) = self.facts.contextual_cleanup.exclusions.span(disposition.exclusions) else {
                            return false;
                        };
                        excluded.iter().all(|excluded| {
                            excluded.root != loan.storage_owner
                                || self.facts.contextual_cleanup.segments.span(excluded.segments)
                                    .is_some_and(|segments| {
                                        !validation::storage_path_relations::canonical_place_segments_may_overlap(self.program, segments, &path)
                                    })
                        })
                    },
                );
                if !covered {
                    if ::validation::trace_flags::check_sites() {
                        eprintln!(
                            "CHECKSITE saved-exit-cleanup-uncovered machine={:?} state={:?} loan={handle:?} row={loan:?} path={path:?} exit={exit:?}",
                            self.machine, self.state.symbol
                        );
                        let mut count = 0;
                        let mut printed = 0;
                        self.facts.contextual_cleanup.any_state_disposition(self.machine, self.state.symbol, |disposition_handle, disposition| {
                            if count < 12 || disposition.root == loan.storage_owner {
                                eprintln!("CHECKSITE saved-exit-cleanup-candidate loan={handle:?} disposition={disposition_handle:?} row={disposition:?} path={:?} exclusions={:?}", self.facts.flow.ownership.segments.span(disposition.path), self.facts.contextual_cleanup.exclusions.span(disposition.exclusions));
                                printed += 1;
                            }
                            count += 1;
                            false
                        });
                        eprintln!(
                            "CHECKSITE saved-exit-cleanup-candidate-count loan={handle:?} total={count} printed={printed} all_matching_owners=true"
                        );
                    }
                    self.completion_refusal(
                        "completion cleanup: no checked cleanup covers this saved access",
                        handle,
                    );
                    self.trace
                        .statement(u32::try_from(exit.statement_index).ok());
                }
                covered
            }
        }
    }

    /*
        Death timing comes from the loan schedule; payload availability comes from
        facts at that exact boundary. Do not reread a selected, partially consumed
        sum merely because a later descriptor in its payload now expires.
    */
    pub(super) fn prepare_case_conditions(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        parameters: &[CheckedUnitStructuralParameterPlan],
        results: &Results,
        operations: &mut [CheckedUnitEffectOperationPlan],
    ) -> Option<()> {
        use crate::execution::terminal_unit::{
            cleanup::checked_path_segments, types::machine_binders,
        };
        let authored = program.state_by_symbol(state)?;
        let machine_def = program.machine_by_symbol(machine)?;
        if operations.is_empty() {
            return Some(());
        }
        let binders = machine_binders(program, machine_def);
        for operation in operations {
            let CheckedUnitEffectOperationPlan::ReleaseReference {
                source,
                path,
                statement_index,
                defer_to_exit,
                case_conditions,
                ..
            } = operation
            else {
                continue;
            };
            if !path
                .iter()
                .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Case(_)))
            {
                continue;
            }
            let root = match source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                    results
                        .iter()
                        .find(|(result, _)| result.binding_ordinal == *binding_ordinal)?
                        .1
                }
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                    PlaceRoot::Symbol(*symbol)
                }
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    expression,
                    ..
                } => PlaceRoot::Expression(*expression),
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                    let parameter = parameters.get(*parameter_index as usize)?;
                    PlaceRoot::Symbol(
                        program
                            .state_parameters(authored)
                            .get(parameter.position as usize)?
                            .symbol,
                    )
                }
                _ => return None,
            };
            let boundary = *statement_index as usize;
            let reference = canonical_place_type_reference(
                program,
                state,
                boundary,
                &CanonicalPlace {
                    root,
                    segments: Vec::new(),
                },
            )?;
            let flow = facts.flow.control.state(machine, state)?;
            let points: Vec<(usize, Vec<FactContextHandle>)> = if *defer_to_exit {
                facts
                    .flow
                    .control
                    .outgoing
                    .span_or_empty(flow.outgoing)
                    .iter()
                    .map(|exit| {
                        (
                            exit.statement_index,
                            facts
                                .flow
                                .semantic_constraint_contexts(exit.constraints)
                                .collect(),
                        )
                    })
                    .collect()
            } else {
                facts
                    .flow
                    .state_statement(flow, boundary)
                    .map(|statement| {
                        vec![(
                            boundary,
                            facts
                                .flow
                                .semantic_constraint_contexts(statement.entry_constraints)
                                .collect(),
                        )]
                    })
                    .unwrap_or_default()
            };
            let mut conditions = Vec::new();
            for (position, segment) in path.iter().enumerate() {
                let CheckedUnitStructuralPathSegment::Case(case_identity) = segment else {
                    continue;
                };
                let segments =
                    checked_path_segments(program, state, boundary, reference, &path[..=position])?;
                let subject = CanonicalPlace { root, segments };
                if !points.is_empty()
                    && points.iter().all(|(statement, contexts)| {
                        crate::flow::place_cases_are_selected(
                            program,
                            &facts.semantic,
                            contexts,
                            machine,
                            state,
                            *statement,
                            &subject,
                        )
                    })
                {
                    continue;
                }
                let mut carrier = subject;
                carrier.segments.pop();
                let carrier_reference =
                    canonical_place_type_reference(program, state, boundary, &carrier)?;
                let type_identity = shapes.add_type(carrier_reference, &binders, &[])?;
                conditions.push(CheckedCleanupCaseCondition {
                    path: shapes
                        .cleanup_control
                        .paths
                        .insert_many(path[..position].iter().cloned()),
                    type_identity,
                    case_identity: case_identity.clone(),
                });
            }
            *case_conditions = shapes.cleanup_control.conditions.insert_many(conditions);
        }
        Some(())
    }

    /*
        Missing borrow rows mean no recorded holders; duplicate rows cannot name
        one retirement schedule. Keep these outcomes distinct while consulting
        the ledger's existing revision-checked index instead of scanning all states
        once for every statement and every descendant holder.
    */
    pub(super) fn unique_borrow_state(
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<Option<&StateBorrowFact>> {
        facts
            .borrow
            .with_state_borrows(machine, state, |rows| match rows {
                [] => Some(None),
                [handle] => Some(Some(facts.borrow.states.get(*handle))),
                _ => None,
            })
    }

    /*
        Completion transfers one actual result occurrence, named or anonymous.
        Its captured origin roster is produced by checking; this join schedules
        local disposition without inferring a return from a type or source root.
    */
    pub(super) fn returned_reference_binding(
        order: &StatementExecutionOrder,
        returned: &CheckedUnitStructuralReturnPlan,
        result: &CheckedUnitStructuralResultBindingPlan,
    ) -> bool {
        !returned.reference_sources.is_empty()
            && returned.source
                == (CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                })
            && returned.type_identity == result.type_identity
            && returned.multiplicity == result.multiplicity
            && match returned.completion {
                ReturnCompletion::Value {
                    statement_index, ..
                } => order.at_or_before(result.statement_index as usize, statement_index as usize),
                ReturnCompletion::Call { coordinate } => {
                    result.statement_index == coordinate.statement_index
                }
            }
    }

    use crate::execution::terminal_unit::storage_transfers as transfers;
    use transfers::{Destination, transfers};
    mod backing {
        /*
            Owned children of a borrowed construction live in its retained backing home.
            A descriptor observes that home; it never becomes the children's owner.
        */
        use super::*;

        impl Retirement<'_> {
            pub(super) fn backing_retired(
                &self,
                value: checked_trees::CheckedStructuralValueHandle,
                path: &[checked_trees::BorrowLoanOwnerSegment],
                active: &mut Vec<RetirementNode>,
            ) -> bool {
                let values = &self.facts.values.structural_values;
                if !values.nodes.is_valid(value) {
                    return false;
                }
                let node = values.nodes.get(value);
                if node.machine != self.machine
                    || node.state != self.state.symbol
                    || !self
                        .order
                        .at_or_before(node.statement_index as usize, self.statement_index as usize)
                    || values
                        .root_for_expression(node.state, node.statement_index, node.expression)
                        .is_none_or(|root| root.root != value)
                {
                    return false;
                }
                let owner = facts::PlaceRoot::Expression(node.expression);
                if self.call_cleanup_settles_owner(&owner, node.statement_index as usize) {
                    return true;
                }
                let Some(resources) = self.resource_loans() else {
                    return false;
                };
                let mut found = false;
                let retired = resources
                    .filter(|(_, loan)| {
                        loan.storage_owner == owner
                            && loan.statement_index == node.statement_index as usize
                            && self.facts.borrow.loan_owner_path(loan).starts_with(path)
                    })
                    .all(|(handle, _)| {
                        found = true;
                        self.holder_retired_following(handle, active)
                    });
                found && retired
            }
        }
    }
    mod carriers {
        /*
            Materialized root descriptors have occurrence identities even when no
            source local holds them. The checked call's input-origin roster joins those
            occurrences into the same disposal order as named loans. Empty source-loan
            spans must not make an anonymous carrier disappear from that order.
        */
        use super::*;
        use checked_trees::CheckedNormalCleanupAction;
        use checked_trees::CheckedStructuralAccess;
        use checked_trees::CheckedUnitCallPlan;
        use checked_trees::CheckedUnitCleanupBoundary;
        use checked_trees::CheckedUnitLocalBindingPlan;
        use facts::PlaceRoot;
        use validation::CanonicalPlace;
        use validation::borrow_contracts::substituted_result_is_view_free;
        use validation::canonical_place_type_reference;
        use validation::reference_result_custody::StoredCarrierSource;
        use validation::reference_result_custody::named_place_loans;
        use validation::reference_result_custody::record_carrier_sources;
        use validation::reference_result_custody::stored_carrier_sources;

        impl Retirement<'_> {
            /*
                A temporary has no authored use after its enclosing statement, but its
                returned children can retain it. Reverse production schedules each child
                before the exact input descriptor that supplied it.
            */
            pub(in crate::execution::terminal_unit::control::statement_sequence) fn carrier_children_retired(
                &self,
                parent: &CheckedUnitStructuralResultBindingPlan,
            ) -> bool {
                let mut operations = self
                    .operations
                    .iter()
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls);
                operations.all(|operation| {
                    if let CheckedUnitEffectOperationPlan::EstablishLocal(
                        CheckedUnitLocalBindingPlan::StructuralValue {
                            result,
                            value,
                            calls,
                            ..
                        },
                    ) = operation
                    {
                        if result.binding_ordinal == parent.binding_ordinal {
                            return true;
                        }
                        if self.carrier_retired(result, &mut Vec::new()) {
                            if ::validation::trace_flags::check_sites() {
                                eprintln!(
                                    "CHECKSITE carrier-child-settled state={:?} parent={} child={result:?} value={value:?} at_exit={} pending_owner={:?}",
                                    self.state.symbol, parent.binding_ordinal, self.at_exit, self.uncommitted_owner,
                                );
                            }
                            return true;
                        }
                        let values = &self.facts.values.structural_values;
                        let reference = values.nodes.get(*value).type_reference;
                        let Some(leaves) = record_carrier_sources(
                            self.program,
                            reference,
                            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal: result.binding_ordinal,
                            },
                        ) else {
                            return false;
                        };
                        return leaves.iter().all(|leaf| {
                            let Some(sources) =
                                stored_carrier_sources(self.program, self.facts, *value, &leaf.path)
                            else {
                                return false;
                            };
                            sources.iter().all(|source| match source {
                                StoredCarrierSource::Borrow { source, .. } => {
                                    !self.source_retains_carrier(source, parent)
                                }
                                StoredCarrierSource::Call { value, .. } => calls
                                    .iter()
                                    .find(|call| call.value == *value)
                                    .and_then(|call| call.operation())
                                    .and_then(CheckedUnitEffectOperationPlan::call)
                                    .is_some_and(|call| !self.call_retains_carrier(call, parent)),
                                StoredCarrierSource::Place(_) | StoredCarrierSource::Image { .. } => true,
                            })
                        });
                    }
                    if let CheckedUnitEffectOperationPlan::EstablishReference { result, source, .. } =
                        operation
                    {
                        return !self.source_retains_carrier(source, parent)
                            || self.carrier_retired(result, &mut Vec::new());
                    }
                    let Some(call) = operation.call() else {
                        return true;
                    };
                    let Some(result) = call.structural_result() else {
                        return true;
                    };
                    if result.binding_ordinal == parent.binding_ordinal
                        || !self.order.precedes(
                            result.statement_index as usize,
                            self.statement_index as usize,
                        )
                    {
                        return true;
                    }
                    let retains = self.call_retains_carrier(call, parent);
                    !retains || self.carrier_retired(result, &mut Vec::new())
                })
            }

            fn call_retains_carrier(
                &self,
                call: &CheckedUnitCallPlan,
                parent: &CheckedUnitStructuralResultBindingPlan,
            ) -> bool {
                call.structural_custody().is_some_and(|custody| {
                    custody.retained_reference_inputs.iter().any(|index| {
                        call.structural_arguments
                            .get(*index as usize)
                            .is_some_and(|source| self.source_retains_carrier(source, parent))
                    })
                })
            }

            fn source_retains_carrier(
                &self,
                source: &CheckedUnitStructuralArgumentPlan,
                parent: &CheckedUnitStructuralResultBindingPlan,
            ) -> bool {
                if let Some(binding) = source.source_structural_result_binding_ordinal() {
                    binding == parent.binding_ordinal
                } else {
                    source.structural_storage_root().is_some_and(|root| {
                        self.results
                            .iter()
                            .any(|(binding, candidate)| binding == parent && *candidate == root)
                    })
                }
            }

            pub(super) fn carrier_retired(
                &self,
                result: &CheckedUnitStructuralResultBindingPlan,
                active: &mut Vec<RetirementNode>,
            ) -> bool {
                let node = RetirementNode::Carrier(result.binding_ordinal);
                if active.contains(&node) {
                    return false;
                }
                active.push(node);
                let retired = self.carrier_retired_inner(result, active);
                active.pop();
                retired
            }

            fn carrier_retired_inner(
                &self,
                result: &CheckedUnitStructuralResultBindingPlan,
                active: &mut Vec<RetirementNode>,
            ) -> bool {
                if self.at_exit
                    && self.returned.is_some_and(|returned| {
                        super::returned_reference_binding(self.order, returned, result)
                    })
                {
                    return self.uncommitted_owner == PlaceRoot::Symbol(Default::default());
                }
                if self.at_exit
                    && matches!(
                        result.multiplicity,
                        language_semantics::Multiplicity::Unrestricted
                            | language_semantics::Multiplicity::Affine
                    )
                    && (result.local.is_named()
                        || result.multiplicity == language_semantics::Multiplicity::Affine)
                    && super::carrier_loans(
                        self.facts,
                        self.machine,
                        self.state.symbol,
                        result,
                        self.results,
                    )
                    .is_some_and(|loans| loans.is_empty())
                {
                    if ::validation::trace_flags::check_sites() {
                        eprintln!(
                            "CHECKSITE carrier-frame-completion state={:?} result={result:?} pending_owner={:?} reason=no-local-loans",
                            self.state.symbol, self.uncommitted_owner,
                        );
                    }
                    return true;
                }
                let source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                };
                if self
                    .operations
                    .iter()
                    .chain(self.pending)
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .any(|operation| match operation {
                        CheckedUnitEffectOperationPlan::ReleaseReference {
                            source: released,
                            path,
                            statement_index,
                            defer_to_exit,
                            ..
                        } => {
                            *released == source
                                && path.is_empty()
                                && self.order.at_or_before(
                                    *statement_index as usize,
                                    self.statement_index as usize,
                                )
                                && (!*defer_to_exit || self.at_exit)
                        }
                        CheckedUnitEffectOperationPlan::ContinuationCleanup {
                            cleanup_actions,
                            boundary,
                            ..
                        } => {
                            let statement = match boundary {
                                CheckedUnitCleanupBoundary::StatementEntry { statement_index }
                                | CheckedUnitCleanupBoundary::Replacement {
                                    statement_index, ..
                                } => *statement_index,
                                _ => return false,
                            };
                            statement <= self.statement_index
                                && statement > result.statement_index
                                && cleanup_actions.iter().any(|action| {
                                    matches!(action,
                                    CheckedNormalCleanupAction::Discard(discard)
                                        if discard.source == source && discard.path.is_empty())
                                })
                        }
                        _ => false,
                    })
                {
                    return true;
                }
                if let Some(loans) = super::carrier_loans(
                    self.facts,
                    self.machine,
                    self.state.symbol,
                    result,
                    self.results,
                ) && !loans.is_empty()
                    && self
                        .facts
                        .borrow
                        .loan_group(loans)
                        .is_some_and(|mut group| {
                            group.all(|(handle, _)| self.holder_retired_following(handle, active))
                        })
                {
                    return true;
                }
                let Some(transfers) = transfers(
                    self.program,
                    self.facts,
                    self.prepared_calls,
                    self.results,
                    result.binding_ordinal,
                    self.operations,
                ) else {
                    return false;
                };
                if !self.transfers_cover_references(result, transfers.occurrence_transfers()) {
                    return false;
                }
                !transfers.possible.is_empty()
                    && transfers
                        .possible
                        .iter()
                        .all(|transfer| match &transfer.destination {
                            Destination::Holder(receiver, _) => {
                                self.carrier_retired(receiver, active)
                            }
                            Destination::Backing(value, path) => {
                                self.backing_retired(*value, path, active)
                            }
                            Destination::Storage {
                                statement_index,
                                captured_selection,
                            } => {
                                let statement_index = *statement_index;
                                if !self.order.precedes(
                                    statement_index as usize,
                                    self.statement_index as usize,
                                ) {
                                    return false;
                                }
                                let Some((owner, path)) = stores::destination(
                                    self.program,
                                    self.facts,
                                    self.machine,
                                    self.state,
                                    statement_index,
                                    captured_selection,
                                ) else {
                                    return false;
                                };
                                let Some(groups) = named_place_loans(
                                    self.program,
                                    self.facts,
                                    self.machine,
                                    self.state,
                                    owner,
                                    statement_index,
                                    &path,
                                ) else {
                                    return false;
                                };
                                !groups.is_empty()
                                    && groups.iter().all(|(_, group)| {
                                        self.facts.borrow.loan_group(*group).is_some_and(
                                            |mut loans| {
                                                loans.all(|(handle, _)| {
                                                    self.holder_retired_following(handle, active)
                                                })
                                            },
                                        )
                                    })
                            }
                            Destination::Call {
                                statement_index,
                                structural_result,
                            } => {
                                if !self.order.precedes(
                                    *statement_index as usize,
                                    self.statement_index as usize,
                                ) {
                                    return false;
                                }
                                if !structural_result {
                                    return super::carrier_loans(
                                        self.facts,
                                        self.machine,
                                        self.state.symbol,
                                        result,
                                        self.results,
                                    )
                                    .is_some_and(|loans| {
                                        self.facts.borrow.loan_group(loans).is_some_and(
                                            |mut group| {
                                                group.all(|(supplier, _)| {
                                                    self.call_receivers_retired(
                                                        supplier,
                                                        *statement_index,
                                                        false,
                                                        active,
                                                    )
                                                })
                                            },
                                        )
                                    });
                                }
                                let mut found = false;
                                let retired = self
                                    .operations
                                    .iter()
                                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                                    .filter_map(|operation| operation.call())
                                    .filter(|call| {
                                        call.coordinate.statement_index == *statement_index
                                    })
                                    .filter(|call| {
                                        call.structural_arguments.iter().any(|argument| {
                                            argument.access == CheckedStructuralAccess::Owned
                                                && (argument
                                                    .source_structural_result_binding_ordinal()
                                                    == Some(result.binding_ordinal)
                                                    || argument
                                                        .structural_storage_root()
                                                        .is_some_and(|root| {
                                                            self.results.iter().any(
                                                                |(binding, candidate)| {
                                                                    binding == result
                                                                        && *candidate == root
                                                                },
                                                            )
                                                        }))
                                        })
                                    })
                                    .all(|call| {
                                        found = true;
                                        let Some(receiver) = call.structural_result() else {
                                            return true;
                                        };
                                        let Some(target) =
                                            self.program.state_by_symbol(call.target.state)
                                        else {
                                            return false;
                                        };
                                        if substituted_result_is_view_free(
                                            self.program,
                                            target.return_type,
                                            &[],
                                        ) {
                                            return true;
                                        }
                                        self.carrier_retired(receiver, active)
                                    });
                                found && retired
                            }
                            _ => false,
                        })
            }

            /*
                Descriptor death need not move an aggregate in one operation. A region
                store can consume its leaves separately. Every declared reference leaf
                must be definitely transferred, and `carrier_retired` still follows all
                receivers until their occurrences retire. This grants no disposal of
                the aggregate's other contents or of an unselected sibling descriptor.
            */
            fn transfers_cover_references(
                &self,
                result: &CheckedUnitStructuralResultBindingPlan,
                transferred: &[Vec<CheckedUnitStructuralPathSegment>],
            ) -> bool {
                if transferred.iter().any(|path| path.is_empty()) {
                    return true;
                }
                let covered = (|| {
                    let mut owners = self.results.iter().filter(|(binding, _)| binding == result);
                    let (_, root) = owners.next()?;
                    if owners.next().is_some() {
                        return None;
                    }
                    let reference = canonical_place_type_reference(
                        self.program,
                        self.state.symbol,
                        result.statement_index as usize,
                        &CanonicalPlace {
                            root: *root,
                            segments: Vec::new(),
                        },
                    )?;
                    let leaves = record_carrier_sources(
                        self.program,
                        reference,
                        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                    )?;
                    Some(
                        !leaves.is_empty()
                            && leaves.iter().all(|leaf| {
                                transferred.iter().any(|path| leaf.path.starts_with(path))
                            }),
                    )
                })();
                covered == Some(true)
            }
        }
    }
    mod owned_backing {

        /*
            Join source-selected anonymous owner deaths to completed holder lifetimes.
            A materialized descriptor and its owned backing are separate occurrences.
            The retained value graph identifies backing; the existing retirement query
            closes every actual holder and transport before attaching that disposition.
        */
        use super::{Results, Retirement};
        use crate::execution::terminal_unit::calls::resolve_call_target;
        use crate::execution::terminal_unit::cleanup::retained_owners::dependency_dispositions;
        use crate::execution::terminal_unit::structural_value_calls::PreparedCalls;
        use crate::execution::terminal_unit::types::ShapeCollector;
        use crate::shared::collections::push_unique;
        use checked_trees::{
            CheckFacts, CheckedReferenceOwnerCleanup,
            CheckedUnitEffectOperationPlan as OperationPlan, CheckedUnitLocalBindingPlan,
            CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
            CheckedUnitStructuralResultBindingPlan,
        };
        use symbols::SymbolHandle;

        struct OwnerCandidate {
            disposition: arena::Handle<checked_trees::ContextualCleanupDisposition>,
            source_call: arena::Handle<checked_trees::FlowCallFact>,
            target_identity: (SymbolHandle, SymbolHandle),
            values: Vec<checked_trees::CheckedStructuralValueHandle>,
        }

        /*
            State-local construction scratch, prepared once and borrowed at each expiry.
            It names issued source/value relationships, never a second liveness ledger.
        */
        pub(in crate::execution::terminal_unit) struct OwnerCandidates {
            owners: Vec<OwnerCandidate>,
            pub(in crate::execution::terminal_unit::control::statement_sequence) storage_borrowers:
                super::StorageBorrowers,
        }

        impl OwnerCandidates {
            pub(in crate::execution::terminal_unit) fn new(
                program: &dyn validation::CheckingInput,
                facts: &CheckFacts,
                machine: SymbolHandle,
                state: SymbolHandle,
            ) -> Option<Self> {
                let values = &facts.values.structural_values;
                let flow = facts.flow.control.state(machine, state)?;
                let calls = facts.flow.control.calls.span(flow.calls)?;
                let roots = values
                    .roots
                    .iter()
                    .map(|(_, root)| root)
                    .filter(|root| {
                        root.machine == machine
                            && root.state == state
                            && validation::reference_result_custody::stored_loan_parts(
                                program,
                                root.type_reference,
                            )
                            .is_some()
                    })
                    .collect::<Vec<_>>();
                let mut owners: Vec<OwnerCandidate> = Vec::new();
                for value_root in roots.into_iter().rev() {
                    for handle in dependency_dispositions(program, facts, state, value_root.root)? {
                        let row = facts.contextual_cleanup.dispositions.get(handle);
                        let language_semantics::PermissionEventSource::Call {
                            statement_index,
                            call_ordinal,
                            target_symbol,
                        } = row.source
                        else {
                            return None;
                        };
                        let mut origins = calls.iter().enumerate().filter(|(_, call)| {
                            call.statement_index == statement_index
                                && call.call_ordinal == call_ordinal
                                && call.target_symbol == target_symbol
                        });
                        let (ordinal, source) = origins.next()?;
                        if origins.next().is_some() {
                            return None;
                        }
                        if !facts
                            .flow
                            .ownership
                            .with_state_permissions(state, |handles| {
                                handles.iter().copied().any(|handle| {
                                    let event = facts.flow.ownership.permissions.get(handle);
                                    event.machine_symbol == machine
                                        && event.state_symbol == state
                                        && event.source == row.source
                                        && event.root == row.root
                                        && event.kind
                                            == language_semantics::PermissionEventKind::Establish
                                        && event.access
                                            != language_semantics::PermissionAccess::Owned
                                })
                            })
                        {
                            continue;
                        }
                        if !source.argument_operands().handles().any(|operand| {
                            let operand = facts.flow.control.operands.get(operand);
                            operand.state_symbol == state
                                && operand.statement_index == statement_index
                                && operand.expression == value_root.expression
                                && program.normalized_type_identity(operand.type_reference)
                                    == program.normalized_type_identity(value_root.type_reference)
                        }) {
                            continue;
                        }
                        let source_call = flow.calls.handle_at(u32::try_from(ordinal).ok()?);
                        if let Some(owner) = owners.iter_mut().find(|owner| {
                            let existing =
                                facts.contextual_cleanup.dispositions.get(owner.disposition);
                            existing.root == row.root && existing.source == row.source
                        }) {
                            push_unique(&mut owner.values, value_root.root);
                        } else {
                            owners.push(OwnerCandidate {
                                disposition: handle,
                                source_call,
                                target_identity: resolve_call_target(
                                    program,
                                    source.target_symbol,
                                )?
                                .identity(),
                                values: vec![value_root.root],
                            });
                        }
                    }
                }
                Some(Self {
                    owners,
                    storage_borrowers: super::StorageBorrowers::new(facts, machine, state),
                })
            }
        }

        struct Owner<'a> {
            disposition: arena::Handle<checked_trees::ContextualCleanupDisposition>,
            row: &'a checked_trees::ContextualCleanupDisposition,
            carriers: Vec<&'a CheckedUnitStructuralResultBindingPlan>,
        }

        #[allow(clippy::too_many_arguments)]
        pub(in crate::execution::terminal_unit::control::statement_sequence) fn prepare_owner_cleanups(
            candidates: &OwnerCandidates,
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            prepared_calls: &PreparedCalls,
            shapes: &mut ShapeCollector<'_>,
            machine: SymbolHandle,
            state: &typed_trees::state::State,
            statement_index: u32,
            returned: Option<&checked_trees::CheckedUnitStructuralReturnPlan>,
            completion: Option<
                &crate::execution::terminal_unit::storage_transfers::PreparedCompletion<'_>,
            >,
            at_exit: bool,
            trace: &crate::execution::terminal_unit::control::LocalConstructionTrace,
            results: &Results,
            operations: &[OperationPlan],
            completed_operations: usize,
            pending: &mut [OperationPlan],
        ) -> Option<()> {
            let mut owners = Vec::new();
            for candidate in &candidates.owners {
                let row = facts
                    .contextual_cleanup
                    .dispositions
                    .get(candidate.disposition);
                let source_call = facts.flow.control.calls.get(candidate.source_call);
                let calls = operations
                    .iter()
                    .flat_map(OperationPlan::with_value_calls)
                    .filter_map(OperationPlan::call)
                    .filter(|call| {
                        call.coordinate.statement_index as usize == source_call.statement_index
                            && call.coordinate.call_ordinal as usize == source_call.call_ordinal
                            && (call.target.machine, call.target.state) == candidate.target_identity
                            && call.completion()
                                == semantic_vocabulary::BoundaryCompletion::NormalReturn
                    })
                    .collect::<Vec<_>>();
                let [completed_call] = calls.as_slice() else {
                    continue;
                };
                let mut carriers = operations
                    .iter()
                    .flat_map(OperationPlan::with_value_calls)
                    .filter_map(|operation| match operation {
                        OperationPlan::EstablishLocal(
                            CheckedUnitLocalBindingPlan::StructuralValue { result, value, .. },
                        ) if candidate.values.contains(value) => Some(result),
                        OperationPlan::EstablishReference {
                            formation_expression,
                            result,
                            ..
                        } if candidate.values.iter().any(|value| {
                            let value = facts.values.structural_values.nodes.get(*value);
                            value.expression == *formation_expression
                                && value.statement_index == result.statement_index
                        }) =>
                        {
                            Some(result)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                carriers.retain(|carrier| {
                    completed_call.structural_arguments.iter().any(|argument| {
                        argument.access.borrow_access_kind().is_some()
                            && argument.source_structural_result_binding_ordinal()
                                == Some(carrier.binding_ordinal)
                            && argument.path.first()
                                == Some(&checked_trees::CheckedUnitStructuralPathSegment::Referent)
                    })
                });
                if carriers.is_empty() {
                    continue;
                }
                owners.push(Owner {
                    disposition: candidate.disposition,
                    row,
                    carriers,
                });
            }
            if owners.is_empty() {
                return Some(());
            }
            let order = program
                .statement_table
                .admitted_execution_order(state.statement_nodes);
            let transported_loans =
                super::transport::collect(program, facts, machine, state, results, operations)?;
            for index in 0..pending.len() {
                if !matches!(pending[index], OperationPlan::ReleaseReference { .. }) {
                    continue;
                }
                let mut completed = Vec::new();
                for owner in &owners {
                    if operations.iter().chain(&pending[..=index]).any(|operation| {
                        matches!(operation, OperationPlan::ReleaseReference { owner_cleanups, .. }
                            if shapes.cleanup_control.reference_owner_cleanups.span_or_empty(*owner_cleanups)
                                .iter().any(|selected| selected.disposition == owner.disposition))
                    }) {
                        continue;
                    }
                    let retirement = Retirement {
                        program,
                        trace,
                        facts,
                        prepared_calls,
                        machine,
                        state,
                        order: &order,
                        statement_index,
                        results,
                        operations,
                        completed_operations,
                        pending: &pending[..=index],
                        at_exit,
                        uncommitted_owner: owner.row.root,
                        returned,
                        completion,
                        transported_loans: &transported_loans,
                        storage_borrowers: &candidates.storage_borrowers,
                    };
                    let loans = validation::reference_result_custody::storage_loans(
                        facts,
                        machine,
                        state.symbol,
                        owner.row.root,
                    )?;
                    let closed = |retirement: &Retirement<'_>| {
                        owner.carriers.iter().all(|carrier| {
                            retirement.carrier_retired(carrier, &mut Vec::new())
                                && retirement.carrier_children_retired(carrier)
                        }) && loans.iter().all(|loan| {
                            !retirement.return_transfers_holder(facts.borrow.loans.get(*loan))
                                && retirement.holder_retired(*loan)
                                && retirement.children_retired(*loan)
                        })
                    };
                    if !closed(&retirement) {
                        continue;
                    }
                    let before = Retirement {
                        pending: &pending[..index],
                        ..retirement
                    };
                    if closed(&before) {
                        continue;
                    }
                    let facts::PlaceRoot::Expression(expression) = owner.row.root else {
                        return None;
                    };
                    completed.push(CheckedReferenceOwnerCleanup {
                        partitions: Default::default(),
                        disposition: owner.disposition,
                        owner: CheckedUnitStructuralArgumentPlan {
                            projection_evaluations: Default::default(),
                            source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                value_dependency: None,
                                expression,
                                logical_root_type_identity: None,
                            },
                            path: Vec::new(),
                            type_identity: shapes.add_type(owner.row.disposed_type, &[], &[])?,
                            access: checked_trees::CheckedStructuralAccess::Owned,
                        },
                        actions: arena::HandleSpan::empty(),
                    });
                }
                if let OperationPlan::ReleaseReference { owner_cleanups, .. } = &mut pending[index]
                {
                    *owner_cleanups = shapes
                        .cleanup_control
                        .reference_owner_cleanups
                        .insert_many(completed);
                }
            }
            Some(())
        }
    }
    pub(super) use owned_backing::{OwnerCandidates, prepare_owner_cleanups};
    pub(super) mod selections {
        /*
            Selected descriptors have a receiving carrier and anonymous complement
            slots. Closing the complete receiving leaf cohort also closes that leaf
            in its unobservable complement; scalar/root custody keeps its normal death.
            Exact historical transport, not shared spelling or backing, joins the
            original suppliers. This correspondence precedes parent retirement and is
            retained for emission rather than rediscovered from runtime source shapes.
        */
        use super::{BorrowLoanFact, CheckedUnitEffectOperationPlan as OperationPlan, Retirement};
        use crate::borrow::owner_place_segments;
        use arena::{Handle, HandleSpan};
        use checked_trees::CheckedUnitSelectedReferenceRetirementPlan;
        use validation::reference_result_custody::local_record_loans;
        use validation::reference_result_custody::path_has_nominal_receiver;
        use validation::reference_result_custody::storage_owner_type_reference;

        /*
            Transport records include copies, so ancestry alone cannot dispose a
            supplier. Only the exact current candidate occurrence owned by this join
            dies here; an older supplier still needs its own definite move or disposal.
        */
        pub(super) fn disposes_candidate(
            retirement: &Retirement<'_>,
            handle: Handle<BorrowLoanFact>,
            complement: &CheckedUnitSelectedReferenceRetirementPlan,
        ) -> bool {
            let receipt = retirement
                .facts
                .flow
                .ownership
                .owned_selections
                .get(complement.selection);
            let loan = retirement.facts.borrow.loans.get(handle);
            if !retirement
                .facts
                .flow
                .ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .iter()
                .any(|source| source.symbol == loan.owner_symbol)
                || !retirement
                    .order
                    .precedes(loan.statement_index, receipt.statement_ordinal as usize)
            {
                return false;
            }
            let Some(reference) = storage_owner_type_reference(
                retirement.program,
                retirement.state,
                loan.owner_symbol,
                receipt.statement_ordinal,
            ) else {
                return false;
            };
            let path = retirement.facts.borrow.loan_owner_path(loan);
            if super::transport::named_carrier_path(
                retirement.program,
                retirement.state,
                loan.owner_symbol,
                reference,
                path,
            )
            .is_none_or(|selected| selected != complement.path)
            {
                return false;
            }
            retirement.resource_loans().is_some_and(|resources| {
                resources
                    .filter(|(_, candidate)| {
                        candidate.owner_symbol == loan.owner_symbol
                            && retirement.order.precedes(
                                candidate.statement_index,
                                receipt.statement_ordinal as usize,
                            )
                            && retirement.facts.borrow.loan_owner_path(candidate) == path
                    })
                    .map(|(_, candidate)| candidate.statement_index)
                    .max_by_key(|statement| retirement.order.position(*statement))
                    == Some(loan.statement_index)
            })
        }

        pub(super) fn supplies_group(
            retirement: &Retirement<'_>,
            supplier: Handle<BorrowLoanFact>,
            receivers: HandleSpan<BorrowLoanFact>,
        ) -> bool {
            let Some(mut group) = retirement.facts.borrow.loan_group(receivers) else {
                return false;
            };
            group.any(|(receiver, _)| {
                super::transport::supplies(retirement.transported_loans, supplier, receiver)
            })
        }

        pub(in crate::execution::terminal_unit::control::statement_sequence) fn closed_complements(
            retirement: &Retirement<'_>,
        ) -> Option<Vec<CheckedUnitSelectedReferenceRetirementPlan>> {
            if !retirement
                .pending
                .iter()
                .any(|operation| matches!(operation, OperationPlan::ReleaseReference { .. }))
            {
                return Some(Vec::new());
            }
            let mut output = Vec::new();
            let state_selections = retirement
                .facts
                .flow
                .ownership
                .with_state_owned_selections(retirement.state.symbol, |handles| handles.to_vec());
            for selection in state_selections {
                let receipt = retirement
                    .facts
                    .flow
                    .ownership
                    .owned_selections
                    .get(selection);
                if receipt.machine != retirement.machine
                    || receipt.state != retirement.state.symbol
                    || receipt.statement_ordinal >= retirement.statement_index
                    || !receipt.destination.is_valid()
                    || !retirement
                        .facts
                        .flow
                        .ownership
                        .selection_sources
                        .span_or_empty(receipt.sources)
                        .iter()
                        .any(|source| source.retains_storage)
                {
                    continue;
                }
                let Some(groups) = local_record_loans(
                    retirement.program,
                    retirement.facts,
                    retirement.machine,
                    retirement.state,
                    receipt.statement_ordinal,
                ) else {
                    continue;
                };
                for (source, loans) in groups {
                    let retains_receiver =
                        retirement.facts.borrow.loan_group(loans)?.any(|(_, loan)| {
                            let path =
                                owner_place_segments(retirement.facts.borrow.loan_owner_path(loan));
                            retirement
                                .facts
                                .flow
                                .ownership
                                .selection_sources
                                .span_or_empty(receipt.sources)
                                .iter()
                                .any(|supplier| {
                                    storage_owner_type_reference(
                                        retirement.program,
                                        retirement.state,
                                        supplier.symbol,
                                        receipt.statement_ordinal,
                                    )
                                    .is_none_or(|reference| {
                                        path_has_nominal_receiver(
                                            retirement.program,
                                            reference,
                                            &path,
                                        )
                                    })
                                })
                        });
                    if retains_receiver {
                        continue;
                    }
                    if retirement
                        .operations
                        .iter()
                        .chain(retirement.pending)
                        .flat_map(OperationPlan::with_value_calls)
                        .any(|operation| {
                            matches!(operation, OperationPlan::ReleaseReference {
                                selection_complements, ..
                            } if selection_complements.iter().any(|complement| {
                                complement.selection == selection && complement.path == source.path
                            }))
                        })
                    {
                        continue;
                    }
                    if !retirement.facts.borrow.loan_group(loans)?.any(|(loan, _)| {
                        retirement.pending.iter().any(|operation| {
                            matches!(operation, OperationPlan::ReleaseReference { loans: released, .. }
                                if supplies_group(retirement, loan, *released))
                        })
                    }) {
                        continue;
                    }
                    if retirement.facts.borrow.loan_group(loans)?.all(|(loan, _)| {
                        retirement.holder_retired(loan) && retirement.children_retired(loan)
                    }) {
                        output.push(CheckedUnitSelectedReferenceRetirementPlan {
                            selection,
                            path: source.path,
                            loans,
                        });
                    }
                }
            }
            Some(output)
        }

        /*
            A receiving cohort closes at its last contributing descriptor release.
            An unrelated later release cannot stand in for this custody dependency.
        */
        pub(in crate::execution::terminal_unit::control::statement_sequence) fn complement_completions(
            retirement: &Retirement<'_>,
        ) -> Option<Vec<(usize, CheckedUnitSelectedReferenceRetirementPlan)>> {
            closed_complements(retirement)?
                .into_iter()
                .map(|complement| {
                    let position = retirement.pending.iter().enumerate().rev().find_map(
                        |(index, operation)| {
                            let OperationPlan::ReleaseReference { loans, .. } = operation else {
                                return None;
                            };
                            retirement
                                .facts
                                .borrow
                                .loan_group(complement.loans)?
                                .any(|(receiver, _)| supplies_group(retirement, receiver, *loans))
                                .then_some(index)
                        },
                    )?;
                    Some((position, complement))
                })
                .collect()
        }
    }
    pub(super) mod stores {
        /*
            Stored descriptors retain the receiving occurrence, not the RHS producer's
            original slot. The checked store supplies exact storage geometry; the loan
            ledger supplies the complete formation group and its weakening boundary.
        */
        use super::{
            BorrowLoanFact, CheckedUnitEffectOperationPlan as OperationPlan,
            CheckedUnitStructuralArgumentSourcePlan as SourcePlan,
            CheckedUnitStructuralPathSegment as PathSegment, Retirement, SymbolHandle, transfers,
        };
        use crate::borrow::owner_path_from_place_segments;
        use crate::borrow::place_path_covers_owner;
        use crate::execution::terminal_unit::types::dense_structural_parameter_index;

        fn storage_source(
            program: &dyn validation::CheckingInput,
            state: &typed_trees::state::State,
            owner: SymbolHandle,
        ) -> Option<SourcePlan> {
            let parameters = program.state_parameters(state);
            if let Some(position) = parameters
                .iter()
                .position(|parameter| parameter.symbol == owner)
            {
                return Some(SourcePlan::Parameter {
                    parameter_index: dense_structural_parameter_index(
                        program, parameters, position,
                    )?,
                });
            }
            Some(SourcePlan::StructuralLocal { symbol: owner })
        }

        /*
            Resolve a current named binding, keeping authored formal positions separate
            from the dense structural argument lane. This names storage, not proof that
            a descriptor was installed there; `named_place_loans` requires its activation.
        */
        pub(super) fn source_owner(
            program: &dyn validation::CheckingInput,
            state: &typed_trees::state::State,
            source: &SourcePlan,
        ) -> Option<SymbolHandle> {
            match source {
                SourcePlan::StructuralLocal { symbol }
                | SourcePlan::StructuralLocalBorrow { symbol, .. } => Some(*symbol),
                SourcePlan::Parameter { parameter_index } => {
                    validation::reference_result_custody::origin_formal(
                        program,
                        program.state_parameters(state),
                        *parameter_index,
                    )
                    .map(|(_, parameter)| parameter.symbol)
                }
                _ => None,
            }
        }

        fn receiving_place(
            program: &dyn validation::CheckingInput,
            facts: &checked_trees::CheckFacts,
            machine: SymbolHandle,
            state: &typed_trees::state::State,
            statement_index: u32,
            selection: &[PathSegment],
        ) -> Option<validation::CanonicalPlace> {
            let flow = facts.flow.control.state(machine, state.symbol)?;
            let statement = facts.flow.state_statement(flow, statement_index as usize)?;
            let control = &facts.flow.control;
            if !control.operands.is_valid(statement.stored_value) {
                return None;
            }
            let value = control.operands.get(statement.stored_value);
            if !value.type_reference.is_valid() {
                return None;
            }
            let destination = if control.operands.is_valid(statement.store_target) {
                let captured = control.operands.get(statement.store_target);
                let [destination] = control.operand_referents.span(captured.referents)? else {
                    return None;
                };
                validation::CanonicalPlace {
                    root: destination.root,
                    segments: control
                        .operand_referent_segments
                        .span(destination.segments)?
                        .to_vec(),
                }
            } else {
                if !facts.semantic.places.is_valid(statement.stored_destination) {
                    return None;
                }
                validation::membership_evidence::canonical_place_from_semantic_place(
                    program,
                    &facts.semantic,
                    facts.semantic.places.get(statement.stored_destination),
                )?
            };
            let selection = validation::reference_result_custody::loan_place_segments(
                program,
                value.type_reference,
                selection,
            )?;
            let segments =
                facts::project_captured_view_segments(&destination.segments, &selection)?;
            Some(validation::CanonicalPlace {
                root: destination.root,
                segments,
            })
        }

        pub(super) fn destination(
            program: &dyn validation::CheckingInput,
            facts: &checked_trees::CheckFacts,
            machine: SymbolHandle,
            state: &typed_trees::state::State,
            statement_index: u32,
            selection: &[PathSegment],
        ) -> Option<(SymbolHandle, Vec<PathSegment>)> {
            let destination =
                receiving_place(program, facts, machine, state, statement_index, selection)?;
            let owner = destination.root.as_symbol()?;
            let path = owner_path_from_place_segments(program, &destination.segments);
            let reference = validation::reference_result_custody::storage_owner_type_reference(
                program,
                state,
                owner,
                statement_index,
            )?;
            Some((
                owner,
                super::transport::named_carrier_path(program, state, owner, reference, &path)?,
            ))
        }

        pub(super) fn receiver(
            program: &dyn validation::CheckingInput,
            facts: &checked_trees::CheckFacts,
            machine: SymbolHandle,
            state: &typed_trees::state::State,
            operation: &OperationPlan,
        ) -> Option<(SymbolHandle, u32, Vec<PathSegment>)> {
            operation.installed_value()?;
            let statement = operation.source_statement_index()?;
            let selection = operation
                .store()
                .map_or(&[][..], |store| store.captured_selection.as_slice());
            let (symbol, path) = destination(program, facts, machine, state, statement, selection)?;
            if operation
                .structural_result_binding()
                .is_some_and(|result| result.local.symbol != symbol)
            {
                return None;
            }
            Some((symbol, statement, path))
        }

        pub(super) fn loans(
            retirement: &Retirement<'_>,
            operation: &OperationPlan,
        ) -> Option<Vec<validation::reference_result_custody::RecordLeafLoans>> {
            let (owner, formation, path) = receiver(
                retirement.program,
                retirement.facts,
                retirement.machine,
                retirement.state,
                operation,
            )?;
            validation::reference_result_custody::named_place_loans(
                retirement.program,
                retirement.facts,
                retirement.machine,
                retirement.state,
                owner,
                formation,
                &path,
            )
        }

        /*
            A no-displacement replacement ends its resident descriptors in the store
            itself. There need not be a cleanup operation for an intrinsically
            discardable reference. Join the exact weakening to the captured destination;
            the executable root may be a pointer into a different storage owner.
        */
        pub(super) fn replaces(
            retirement: &Retirement<'_>,
            store: &checked_trees::CheckedStorePlan,
            handle: arena::Handle<BorrowLoanFact>,
        ) -> bool {
            let replaced = (|| {
                match store.destination.leaf {
                    checked_trees::CheckedStoreLeaf::StructuralStore {
                        opening_binding: None,
                        ..
                    }
                    | checked_trees::CheckedStoreLeaf::OwnedRebind { .. } => {}
                    _ => return None,
                }
                store.installed_value()?;
                let loan = retirement.facts.borrow.loans.get(handle);
                let statement = store.statement_index as usize;
                if !retirement.order.precedes(loan.statement_index, statement)
                    || !retirement
                        .order
                        .precedes(statement, retirement.statement_index as usize)
                {
                    return None;
                }
                let weakening = validation::reference_result_custody::loan_weakening(
                    retirement.facts,
                    retirement.machine,
                    retirement.state.symbol,
                    handle,
                )?;
                if weakening.reason != checked_trees::FlowBorrowWeakeningReason::LocalReassigned
                    || weakening.source
                        != (checked_trees::FlowInvalidationSource::Statement {
                            statement_index: statement,
                        })
                {
                    return None;
                }
                let destination = receiving_place(
                    retirement.program,
                    retirement.facts,
                    retirement.machine,
                    retirement.state,
                    store.statement_index,
                    &store.captured_selection,
                )?;
                Some(
                    destination.root == loan.storage_owner
                        && place_path_covers_owner(
                            retirement.program,
                            &destination.segments,
                            retirement.facts.borrow.loan_owner_path(loan),
                        ),
                )
            })();
            replaced == Some(true)
        }

        /*
            A store's installed result retains its actual producer graph. Anonymous
            construction is not another named holder, but fresh borrows in that graph
            still pin their immediate supplying descriptor until the stored leaf ends.
        */
        pub(super) fn construction<'a>(
            retirement: &Retirement<'a>,
            child: arena::Handle<BorrowLoanFact>,
            operation: &OperationPlan,
        ) -> Option<(&'a OperationPlan, Vec<PathSegment>)> {
            let (owner, formation, destination) = receiver(
                retirement.program,
                retirement.facts,
                retirement.machine,
                retirement.state,
                operation,
            )?;
            let loan = retirement.facts.borrow.loans.get(child);
            if loan.owner_symbol != owner
                || loan.statement_index != formation as usize
                || !loans(retirement, operation)?
                    .iter()
                    .any(|(_, group)| retirement.facts.borrow.loans_contain(*group, child))
            {
                return None;
            }
            let reference = validation::reference_result_custody::storage_owner_type_reference(
                retirement.program,
                retirement.state,
                owner,
                formation,
            )?;
            let path = super::transport::named_carrier_path(
                retirement.program,
                retirement.state,
                owner,
                reference,
                retirement.facts.borrow.loan_owner_path(loan),
            )?;
            let installed = operation.installed_value()?;
            let binding = installed.source_structural_result_binding_ordinal()?;
            let mut producers = retirement.operations.iter().filter(|producer| {
                producer
                    .structural_result_binding()
                    .is_some_and(|result| result.binding_ordinal == binding)
            });
            let producer = producers.next()?;
            if producers.next().is_some() {
                return None;
            }
            let mut selected = installed.path.clone();
            selected.extend_from_slice(path.strip_prefix(destination.as_slice())?);
            Some((producer, selected))
        }

        /*
            Retire exact leaf groups without destroying scalar siblings. Replacement
            disposal and later transfers already end or relocate an earlier occurrence;
            neither authorizes releasing a new descriptor now occupying that same slot.
        */
        pub(in crate::execution::terminal_unit::control::statement_sequence) fn releases(
            retirement: &Retirement<'_>,
            operation: &OperationPlan,
        ) -> Option<Vec<OperationPlan>> {
            if super::unique_borrow_state(
                retirement.facts,
                retirement.machine,
                retirement.state.symbol,
            )?
            .is_none_or(|borrowing| borrowing.loans.is_empty())
            {
                return Some(Vec::new());
            }
            let Some((owner, formation, _)) = receiver(
                retirement.program,
                retirement.facts,
                retirement.machine,
                retirement.state,
                operation,
            ) else {
                return Some(Vec::new());
            };
            if !retirement
                .order
                .precedes(formation as usize, retirement.statement_index as usize)
            {
                return Some(Vec::new());
            }
            let reference = validation::reference_result_custody::storage_owner_type_reference(
                retirement.program,
                retirement.state,
                owner,
                formation,
            )?;
            if !validation::has_contents(
                retirement.program,
                reference,
                &[],
                validation::ContentsRequirement::DescriptorCleanup,
            ) {
                return Some(Vec::new());
            }
            let Some(groups) = loans(retirement, operation) else {
                return Some(Vec::new());
            };
            let owner_source = storage_source(retirement.program, retirement.state, owner)?;
            let mut output = Vec::new();
            for (source, group) in groups.into_iter().rev() {
                let selected = validation::reference_result_custody::loan_place_segments(
                    retirement.program,
                    reference,
                    &source.path,
                )?;
                if !retirement
                    .storage_borrowers_retired(facts::PlaceRoot::Symbol(owner), &selected)?
                {
                    continue;
                }
                let moved = transfers::from_storage(
                    retirement.program,
                    retirement.facts,
                    retirement.prepared_calls,
                    retirement.results,
                    owner_source.clone(),
                    formation,
                    &source.path,
                    retirement.operations,
                )?;
                if moved.possible.iter().any(|transfer| {
                    transfer.source.starts_with(&source.path)
                        || source.path.starts_with(&transfer.source)
                }) {
                    continue;
                }
                let mut captured = retirement.facts.borrow.loan_group(group)?;
                if !captured.all(|(loan, _)| {
                    !retirement.disposed(loan)
                        && retirement.children_retired(loan)
                        && validation::reference_result_custody::release_statement(
                            retirement.program,
                            retirement.facts,
                            retirement.machine,
                            retirement.state.symbol,
                            loan,
                        )
                        .is_some_and(|boundary| {
                            retirement.order.at_or_before(
                                boundary as usize,
                                retirement.statement_index as usize,
                            )
                        })
                }) {
                    continue;
                }
                if ::validation::trace_flags::check_sites() {
                    eprintln!(
                        "CHECKSITE stored-reference-release state={:?} owner={owner:?} formation={formation} statement={} source={owner_source:?} path={:?} loans={group:?} at_exit={} pending_owner={:?}",
                        retirement.state.symbol,
                        retirement.statement_index,
                        source.path,
                        retirement.at_exit,
                        retirement.uncommitted_owner,
                    );
                }
                output.push(OperationPlan::ReleaseReference {
                    statement_index: retirement.statement_index,
                    source: owner_source.clone(),
                    path: source.path,
                    select_evaluated_storage: false,
                    defer_to_exit: retirement.at_exit,
                    loans: group,
                    selection_complements: Vec::new(),
                    owner_cleanups: arena::HandleSpan::empty(),
                    case_conditions: Default::default(),
                });
            }
            Some(output)
        }
    }
    pub(super) mod transport {
        /*
            Join receiving carrier occurrences to their historical supplying loans.
            Selection belongs to the checked value graph; declared paths identify the
            exact loan leaves. This module owns collection, formation choice and the
            complete receiver-to-supplier correspondence used by retirement.
        */
        use super::{
            CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
            Results, SymbolHandle,
        };
        use arena::Handle;
        use checked_trees::{BorrowLoanFact, CheckedUnitLocalBindingPlan};
        use facts::PlaceRoot;
        use typed_trees::state::State;
        use validation::reference_result_custody::StoredCarrierSource;
        use validation::reference_result_custody::storage_owner_type_reference;
        use validation::reference_result_custody::stored_carrier_sources;

        pub(super) use crate::borrow::{carrier_path, named_carrier_path};

        /*
            Historical carrier transport, not a new loan descent. A copy and an owned
            move retain the supplying occurrence's immediate parents; neither makes
            the supplying descriptor itself a parent of the receiving descriptor.
        */
        pub(in crate::execution::terminal_unit::control::statement_sequence) type TransportedLoans =
            Vec<(Handle<BorrowLoanFact>, Handle<BorrowLoanFact>)>;

        /*
            Rejoin each receiving loan leaf through its retained construction graph.
            The relation is prepared once for statement scheduling and remains valid
            after the supplying holder retires: retirement cannot erase provenance.
        */
        pub(in crate::execution::terminal_unit::control::statement_sequence) fn collect(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: &State,
            results: &Results,
            operations: &[CheckedUnitEffectOperationPlan],
        ) -> Option<TransportedLoans> {
            let Some(borrowing) = super::unique_borrow_state(facts, machine, state.symbol)? else {
                return Some(Vec::new());
            };
            if borrowing.loans.is_empty() {
                return Some(Vec::new());
            }
            let order = program
                .statement_table
                .admitted_execution_order(state.statement_nodes);
            let mut output = Vec::new();
            for (receiver, loan) in facts.borrow.loan_group(borrowing.loans)? {
                for supplier in facts.borrow.loan_suppliers.span(loan.supplying_loans)? {
                    if !facts.borrow.state_owns_loan(borrowing, *supplier) {
                        return None;
                    }
                    crate::shared::collections::push_unique(&mut output, (receiver, *supplier));
                }
            }
            for operation in operations {
                let (owner, formation) = if let Some((result, _)) =
                    operation.discardable_structural_result()
                {
                    let mut registered = results.iter().filter(|(binding, _)| binding == result);
                    let Some((_, PlaceRoot::Symbol(owner))) = registered.next() else {
                        continue;
                    };
                    if registered.next().is_some() {
                        return None;
                    }
                    (*owner, result.statement_index)
                } else if let Some((owner, formation, _)) =
                    super::stores::receiver(program, facts, machine, state, operation)
                {
                    (owner, formation)
                } else {
                    continue;
                };
                let reference = storage_owner_type_reference(program, state, owner, formation)?;
                for (receiver, loan) in
                    facts
                        .borrow
                        .statement_resource_loans(borrowing)?
                        .filter(|(_, loan)| {
                            loan.storage_owner == PlaceRoot::Symbol(owner)
                                && loan.statement_index == formation as usize
                        })
                {
                    let CheckedUnitEffectOperationPlan::EstablishLocal(
                        CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
                    ) = operation
                    else {
                        continue;
                    };
                    let path = named_carrier_path(
                        program,
                        state,
                        owner,
                        reference,
                        facts.borrow.loan_owner_path(loan),
                    )?;
                    let sources = stored_carrier_sources(program, facts, *value, &path)?;
                    for source in sources {
                        let StoredCarrierSource::Place(source) = source else {
                            continue;
                        };
                        let (source_owner, source_formation) = match source.source {
                            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal,
                            } => {
                                let mut holders = results.iter().filter(|(binding, _)| {
                                    binding.binding_ordinal == binding_ordinal
                                });
                                let (binding, PlaceRoot::Symbol(owner)) = holders.next()? else {
                                    return None;
                                };
                                if holders.next().is_some() {
                                    return None;
                                }
                                (*owner, Some(binding.statement_index as usize))
                            }
                            _ => {
                                match super::stores::source_owner(program, state, &source.source) {
                                    Some(owner) => (owner, None),
                                    _ => continue,
                                }
                            }
                        };
                        let Some(supplying_type) =
                            storage_owner_type_reference(program, state, source_owner, formation)
                        else {
                            continue;
                        };
                        let mut supplying = Vec::new();
                        for (handle, candidate) in facts
                            .borrow
                            .statement_resource_loans(borrowing)?
                            .filter(|(_, candidate)| {
                                candidate.storage_owner == PlaceRoot::Symbol(source_owner)
                                    && order
                                        .precedes(candidate.statement_index, loan.statement_index)
                                    && source_formation.is_none_or(|formation| {
                                        candidate.statement_index == formation
                                    })
                            })
                        {
                            if named_carrier_path(
                                program,
                                state,
                                source_owner,
                                supplying_type,
                                facts.borrow.loan_owner_path(candidate),
                            )? == source.path
                            {
                                supplying.push((handle, candidate.statement_index));
                            }
                        }
                        let Some(formation) = supplying
                            .iter()
                            .map(|(_, statement)| *statement)
                            .max_by_key(|statement| order.position(*statement))
                        else {
                            continue;
                        };
                        for (supplier, _) in supplying
                            .into_iter()
                            .filter(|(_, statement)| *statement == formation)
                        {
                            crate::shared::collections::push_unique(
                                &mut output,
                                (receiver, supplier),
                            );
                        }
                    }
                }
            }
            Some(output)
        }

        /*
            Follow complete historical suppliers, including intermediate invocation
            associations. Only callers select the receiving resource whose death is
            owed; reachability itself neither disposes a supplier nor grants access.
        */
        pub(super) fn supplies(
            transported: &TransportedLoans,
            supplier: Handle<BorrowLoanFact>,
            receiver: Handle<BorrowLoanFact>,
        ) -> bool {
            let mut pending = vec![receiver];
            let mut visited = Vec::new();
            while let Some(receiver) = pending.pop() {
                if receiver == supplier {
                    return true;
                }
                if visited.contains(&receiver) {
                    continue;
                }
                visited.push(receiver);
                pending.extend(transported.iter().filter_map(|(received, supplying)| {
                    (*received == receiver).then_some(*supplying)
                }));
            }
            false
        }
    }

    pub(super) fn transferred_paths(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
        results: &Results,
        binding_ordinal: u32,
        operations: &[CheckedUnitEffectOperationPlan],
    ) -> Option<Vec<Vec<CheckedUnitStructuralPathSegment>>> {
        Some(
            transfers(
                program,
                facts,
                prepared_calls,
                results,
                binding_ordinal,
                operations,
            )?
            .possible
            .into_iter()
            .map(|transfer| transfer.source)
            .collect(),
        )
    }

    /*
        State-local membership only. Holder readiness is evaluated against each
        statement's current operations and pending releases, never cached here.
    */
    pub(super) struct StorageBorrowers {
        rows: Option<Vec<(PlaceRoot, Handle<BorrowLoanFact>)>>,
    }

    impl StorageBorrowers {
        fn new(facts: &CheckFacts, machine: SymbolHandle, state: SymbolHandle) -> Self {
            let rows = (|| {
                let borrowing = unique_borrow_state(facts, machine, state)?;
                let mut rows = if let Some(borrowing) = borrowing {
                    facts
                        .borrow
                        .statement_resource_loans(borrowing)?
                        .map(|(handle, loan)| (loan.root, handle))
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                rows.sort_by_key(|(root, _)| root.group_key());
                Some(rows)
            })();
            Self { rows }
        }

        fn for_owner(&self, owner: PlaceRoot) -> Option<&[(PlaceRoot, Handle<BorrowLoanFact>)]> {
            let rows = self.rows.as_ref()?;
            let key = owner.group_key();
            let first = rows.partition_point(|(root, _)| root.group_key() < key);
            let count = rows[first..].partition_point(|(root, _)| root.group_key() == key);
            Some(&rows[first..first + count])
        }
    }

    /*
        One statement's retirement view. Every query over the same statement
        shares these fields; a `Copy` value lets a caller rebind only the prefix,
        pending set, exit flag and returned holder it distinguishes.
    */
    #[derive(Clone, Copy)]
    pub(super) struct Retirement<'a> {
        pub(super) program: &'a dyn ::validation::CheckingInput,
        pub(super) trace: &'a crate::execution::terminal_unit::control::LocalConstructionTrace,
        pub(super) facts: &'a CheckFacts,
        pub(super) prepared_calls:
            &'a crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
        pub(super) machine: SymbolHandle,
        pub(super) state: &'a State,
        pub(super) order: &'a StatementExecutionOrder,
        pub(super) statement_index: u32,
        pub(super) results: &'a Results,
        pub(super) operations: &'a [CheckedUnitEffectOperationPlan],
        /*
            Only this operation prefix has completed at the insertion point.
            A selected future owner death cannot settle a holder needed before it.
        */
        pub(super) completed_operations: usize,
        pub(super) pending: &'a [CheckedUnitEffectOperationPlan],
        /*
            Deferred edge releases count only once completion schedules them.
            Ordinary statement expiry cannot consume a future edge's custody.
        */
        pub(super) at_exit: bool,
        /*
            A pending owner disposition cannot prove its own holders have ended.
            The zero symbol leaves ordinary source-disposition reasoning unchanged.
        */
        pub(super) uncommitted_owner: PlaceRoot,
        /*
            The selected completion transfers this exact holder before its deferred
            ancestor dispositions. It never settles a loan at statement entry.
        */
        pub(super) returned: Option<&'a CheckedUnitStructuralReturnPlan>,
        /*
            A function's final expression can still use its local values. Exit
            cleanup must wait for those uses before releasing the data they need.
        */
        pub(super) completion:
            Option<&'a crate::execution::terminal_unit::storage_transfers::PreparedCompletion<'a>>,
        pub(super) transported_loans: &'a transport::TransportedLoans,
        pub(super) storage_borrowers: &'a StorageBorrowers,
    }

    /*
        One retirement query follows both logical loan occurrences and physical
        result carriers. Neither graph can erase the other's active recursion.
    */
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum RetirementNode {
        Loan(Handle<BorrowLoanFact>),
        Carrier(u32),
    }

    impl Retirement<'_> {
        /*
            Current statement resources own physical holder death. Call-only
            associations authenticate transport and remain in the supplier graph.
        */
        fn resource_loans(
            &self,
        ) -> Option<impl Iterator<Item = (Handle<BorrowLoanFact>, &BorrowLoanFact)>> {
            let state = unique_borrow_state(self.facts, self.machine, self.state.symbol)??;
            self.facts.borrow.statement_resource_loans(state)
        }

        /*
            Releasing a stored descriptor changes the bytes of its home. A view of
            that home must end first, even when it is not a child of the reference
            stored there. Disjoint fields keep their independent release schedules.
        */
        fn storage_borrowers_retired(
            &self,
            owner: PlaceRoot,
            selection: &[PlaceSegment],
        ) -> Option<bool> {
            for (root, handle) in self.storage_borrowers.for_owner(owner)? {
                let loan = self.facts.borrow.loans.get(*handle);
                if *root == owner
                    && self
                        .order
                        .precedes(loan.statement_index, self.statement_index as usize)
                    && crate::flow::canonical_place_segments_may_overlap(
                        self.program,
                        self.facts.borrow.access_segments.span(loan.segments)?,
                        selection,
                    )
                    && !self.holder_retired(*handle)
                {
                    return Some(false);
                }
            }
            Some(true)
        }

        /*
            Computed homes have no independent statement-result binding. Their
            stored descriptors nevertheless own the same captured formation and
            weakening as named storage. End those leaves only after every holder
            borrowing the home has actually retired. This does not dispose the
            backing's owned contents or authorize a nominal cleanup.
            The state-wide loan roster includes unevaluated alternatives. These are
            candidate storage deaths: the existing ownership continuation graph
            selects actual births before emitting exact descriptor releases.
        */
        pub(super) fn backing_releases(&self) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
            let state = unique_borrow_state(self.facts, self.machine, self.state.symbol)?;
            let Some(state) = state else {
                return Some(Vec::new());
            };
            if state.loans.is_empty() {
                return Some(Vec::new());
            }
            let mut owners = Vec::new();
            for (_, loan) in self.facts.borrow.statement_resource_loans(state)? {
                if matches!(loan.storage_owner, PlaceRoot::Expression(_))
                    && !crate::borrow::is_descriptor_load_observation(&self.facts.borrow, loan)
                    && self
                        .order
                        .precedes(loan.statement_index, self.statement_index as usize)
                    && !owners.contains(&(loan.storage_owner, loan.statement_index))
                {
                    owners.push((loan.storage_owner, loan.statement_index));
                }
            }
            let mut releases = Vec::new();
            for (owner, formation) in owners.into_iter().rev() {
                if !self.storage_borrowers_retired(owner, &[])? {
                    continue;
                }
                self.trace
                    .phase("statement sequence: completion backing source type");
                let reference = canonical_place_type_reference(
                    self.program,
                    self.state.symbol,
                    formation,
                    &CanonicalPlace {
                        root: owner,
                        segments: Vec::new(),
                    },
                )?;
                if !has_contents(
                    self.program,
                    reference,
                    &[],
                    ContentsRequirement::DescriptorCleanup,
                ) {
                    continue;
                }
                self.trace
                    .phase("statement sequence: completion backing descriptor loans");
                let groups = match storage_place_loans(
                    self.program,
                    self.facts,
                    self.machine,
                    self.state,
                    owner,
                    reference,
                    u32::try_from(formation).ok()?,
                    &[],
                ) {
                    Ok(groups) => groups,
                    Err(reason) => {
                        self.trace.phase(reason);
                        if ::validation::trace_flags::check_sites() {
                            eprintln!(
                                "CHECKSITE backing-storage-retirement state={:?} current_statement={} formation={formation} owner={owner:?} reference={reference:?} refusal={reason}",
                                self.state.symbol, self.statement_index,
                            );
                        }
                        return None;
                    }
                };
                for (source, loans) in groups {
                    self.trace
                        .phase("statement sequence: completion backing descriptor transfers");
                    let transfers = transfers::from_storage(
                        self.program,
                        self.facts,
                        self.prepared_calls,
                        self.results,
                        source.source.source.clone(),
                        u32::try_from(formation).ok()?,
                        &source.path,
                        self.operations,
                    )?;
                    if transfers.possible.iter().any(|transfer| {
                        source.path.starts_with(&transfer.source)
                            || transfer.source.starts_with(&source.path)
                    }) {
                        continue;
                    }
                    let mut ready = true;
                    self.trace
                        .phase("statement sequence: completion backing descriptor group");
                    for (handle, _) in self.facts.borrow.loan_group(loans)? {
                        if self.disposed(handle)
                            || !self.children_retired(handle)
                            || !release_boundary(
                                self.program,
                                self.facts,
                                self.machine,
                                self.state.symbol,
                                handle,
                            )
                            .is_some_and(|boundary| {
                                self.order
                                    .at_or_before(boundary as usize, self.statement_index as usize)
                            })
                        {
                            ready = false;
                            break;
                        }
                    }
                    if ready {
                        releases.push(CheckedUnitEffectOperationPlan::ReleaseReference {
                            statement_index: self.statement_index,
                            source: source.source.source,
                            path: source.path,
                            select_evaluated_storage: true,
                            defer_to_exit: self.at_exit,
                            loans,
                            selection_complements: Vec::new(),
                            owner_cleanups: HandleSpan::empty(),
                            case_conditions: Default::default(),
                        });
                    }
                }
            }
            Some(releases)
        }

        /*
            An immediate child must have ended its actual carrier, not merely
            reached its parent's lexical last use. Each child's own schedule obeys
            this same rule, so reverse production closes a chain deepest first.
        */
        pub(super) fn children_retired(&self, parent: arena::Handle<BorrowLoanFact>) -> bool {
            let Some(Some(state)) =
                unique_borrow_state(self.facts, self.machine, self.state.symbol)
            else {
                return false;
            };
            let capturing_operations = self.capturing_operations(state);
            let mut pending = vec![(parent, false)];
            let mut active = Vec::new();
            let mut completed = Vec::new();
            while let Some((parent, leaving)) = pending.pop() {
                if leaving {
                    active.pop();
                    completed.push(parent);
                    continue;
                }
                if completed.contains(&parent) {
                    continue;
                }
                if active.contains(&parent) {
                    return false;
                }
                active.push(parent);
                pending.push((parent, true));
                let Some(state_loans) = self.facts.borrow.statement_resource_loans(state) else {
                    return false;
                };
                for (handle, loan) in state_loans {
                    if self
                        .order
                        .precedes(loan.statement_index, self.statement_index as usize)
                        && self.retains_parent(
                            handle,
                            parent,
                            &mut Vec::new(),
                            &capturing_operations,
                        )
                    {
                        if !self.holder_retired(handle) {
                            return false;
                        }
                        pending.push((handle, false));
                    }
                }
            }
            true
        }

        fn retains_parent(
            &self,
            child: arena::Handle<BorrowLoanFact>,
            parent: arena::Handle<BorrowLoanFact>,
            active: &mut Vec<arena::Handle<BorrowLoanFact>>,
            capturing_operations: &std::collections::BTreeMap<
                (u32, u32),
                Vec<&CheckedUnitEffectOperationPlan>,
            >,
        ) -> bool {
            if child == parent || active.contains(&child) {
                return false;
            }
            let loan = self.facts.borrow.loans.get(child);
            if loan.lineage
                == (BorrowLoanLineage::Reborrow {
                    parent_loan: parent,
                })
                || self.captures_holder(child, parent, capturing_operations)
            {
                return true;
            }
            active.push(child);
            let retained = self.transported_loans.iter().any(|(receiver, supplier)| {
                *receiver == child
                    && self.retains_parent(*supplier, parent, active, capturing_operations)
            });
            active.pop();
            retained
        }

        /*
            Rejoin each descriptor to its exact producer once for this immutable
            operation prefix. Parent queries still perform the original source and
            path checks; this roster carries no expiry or liveness judgment. Without
            the producer association, every (child, parent) query scans every earlier
            operation and reconstructs the same captured loan groups again.
        */
        fn capturing_operations(
            &self,
            state: &checked_trees::StateBorrowFact,
        ) -> std::collections::BTreeMap<(u32, u32), Vec<&CheckedUnitEffectOperationPlan>> {
            let mut capturing = std::collections::BTreeMap::<_, Vec<_>>::new();
            for operation in self
                .operations
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            {
                let loans = if let Some(call) = operation.call() {
                    call.structural_result().and_then(|result| {
                        carrier_loans(
                            self.facts,
                            self.machine,
                            self.state.symbol,
                            result,
                            self.results,
                        )
                    })
                } else if let CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { result, .. },
                ) = operation
                {
                    carrier_loans(
                        self.facts,
                        self.machine,
                        self.state.symbol,
                        result,
                        self.results,
                    )
                } else if let CheckedUnitEffectOperationPlan::EstablishReference {
                    result, ..
                } = operation
                {
                    local_reference_loans(
                        self.facts,
                        self.machine,
                        self.state.symbol,
                        result,
                        self.results,
                    )
                } else {
                    if operation.installed_value().is_some()
                        && let Some(loans) = self.facts.borrow.loan_group(state.loans)
                    {
                        for (child, _) in loans {
                            capturing
                                .entry((child.arena_index(), child.generation()))
                                .or_default()
                                .push(operation);
                        }
                    }
                    continue;
                };
                if let Some(loans) = loans.and_then(|loans| self.facts.borrow.loan_group(loans)) {
                    for (child, _) in loans {
                        capturing
                            .entry((child.arena_index(), child.generation()))
                            .or_default()
                            .push(operation);
                    }
                }
            }
            capturing
        }

        /*
            Materialized descriptors retain their supplying carrier even when the
            source resource model deliberately leaves aggregate lineage unretained.
            This is a disposal dependency from the checked operation's actual input,
            not a new reborrow permission or an initializer reconstruction.
        */
        fn captures_holder(
            &self,
            child: arena::Handle<BorrowLoanFact>,
            parent: arena::Handle<BorrowLoanFact>,
            capturing_operations: &std::collections::BTreeMap<
                (u32, u32),
                Vec<&CheckedUnitEffectOperationPlan>,
            >,
        ) -> bool {
            let Some(operations) =
                capturing_operations.get(&(child.arena_index(), child.generation()))
            else {
                return false;
            };
            let parent = self.facts.borrow.loans.get(parent);
            operations.iter().copied().any(|operation| {
                if let Some(call) = operation.call() {
                    return call.structural_result().is_some_and(|_| {
                        self.facts.borrow.loans.get(child).source_owner_symbol
                            == parent.owner_symbol
                            && call.structural_arguments.iter().any(|source| {
                                source.access.borrow_access_kind().is_some()
                                    && self.source_captures_holder(source, parent)
                            })
                    });
                }
                let construction = if let CheckedUnitEffectOperationPlan::EstablishLocal(
                    CheckedUnitLocalBindingPlan::StructuralValue { result, .. },
                ) = operation
                {
                    let loan = self.facts.borrow.loans.get(child);
                    let Some(reference) = storage_owner_type_reference(
                        self.program,
                        self.state,
                        loan.owner_symbol,
                        result.statement_index,
                    ) else {
                        return false;
                    };
                    let Some(path) = transport::named_carrier_path(
                        self.program,
                        self.state,
                        loan.owner_symbol,
                        reference,
                        self.facts.borrow.loan_owner_path(loan),
                    ) else {
                        return false;
                    };
                    Some((operation, path))
                } else {
                    stores::construction(self, child, operation)
                };
                if let Some((producer, path)) = construction {
                    let loan = self.facts.borrow.loans.get(child);
                    if let Some(call) = producer.call() {
                        return loan.source_owner_symbol == parent.owner_symbol
                            && call.structural_arguments.iter().any(|source| {
                                source.access.borrow_access_kind().is_some()
                                    && self.source_captures_holder(source, parent)
                            });
                    }
                    let CheckedUnitEffectOperationPlan::EstablishLocal(
                        CheckedUnitLocalBindingPlan::StructuralValue { value, calls, .. },
                    ) = producer
                    else {
                        return false;
                    };
                    let Some(sources) =
                        stored_carrier_sources(self.program, self.facts, *value, &path)
                    else {
                        return false;
                    };
                    return sources.into_iter().any(|source| {
                        if let StoredCarrierSource::Borrow { value, source } = &source {
                            return loan.formation_expression
                                == self
                                    .facts
                                    .values
                                    .structural_values
                                    .nodes
                                    .get(*value)
                                    .expression
                                && source.access.borrow_access_kind().is_some()
                                && self.source_captures_holder(source, parent);
                        }
                        let StoredCarrierSource::Call { value, .. } = source else {
                            return false;
                        };
                        if loan.formation_expression
                            != self
                                .facts
                                .values
                                .structural_values
                                .nodes
                                .get(value)
                                .expression
                            || loan.source_owner_symbol != parent.owner_symbol
                        {
                            return false;
                        }
                        calls
                            .iter()
                            .find(|call| call.value == value)
                            .and_then(|call| call.operation())
                            .and_then(CheckedUnitEffectOperationPlan::call)
                            .is_some_and(|call| {
                                call.structural_arguments.iter().any(|source| {
                                    source.access.borrow_access_kind().is_some()
                                        && self.source_captures_holder(source, parent)
                                })
                            })
                    });
                }
                let CheckedUnitEffectOperationPlan::EstablishReference { source, .. } = operation
                else {
                    return false;
                };
                self.source_captures_holder(source, parent)
            })
        }

        fn source_captures_holder(
            &self,
            source: &CheckedUnitStructuralArgumentPlan,
            parent: &BorrowLoanFact,
        ) -> bool {
            let source_holder = match source.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                    self.results.iter().find(|(result, root)| {
                        result.binding_ordinal == binding_ordinal
                            && result.statement_index as usize == parent.statement_index
                            && *root == PlaceRoot::Symbol(parent.owner_symbol)
                    })
                }
                _ => source.structural_storage_root().and_then(|source_root| {
                    self.results.iter().find(|(result, root)| {
                        *root == source_root
                            && result.statement_index as usize == parent.statement_index
                            && *root == PlaceRoot::Symbol(parent.owner_symbol)
                    })
                }),
            };
            let stored_holder = stores::source_owner(self.program, self.state, &source.source)
                .is_some_and(|owner| {
                    owner == parent.owner_symbol
                        && self.operations.iter().any(|operation| {
                            stores::receiver(
                                self.program,
                                self.facts,
                                self.machine,
                                self.state,
                                operation,
                            )
                            .is_some_and(|(stored, formation, _)| {
                                stored == owner && formation as usize == parent.statement_index
                            }) && stores::loans(self, operation)
                                .is_some_and(|groups| !groups.is_empty())
                        })
                });
            if source_holder.is_none() && !stored_holder {
                return false;
            }
            let owner_path = self.facts.borrow.loan_owner_path(parent);
            let prefix = source
                .path
                .iter()
                .position(|segment| *segment == CheckedUnitStructuralPathSegment::Referent)
                .unwrap_or(source.path.len());
            let compared = owner_path.len().min(prefix);
            let Some(reference) = storage_owner_type_reference(
                self.program,
                self.state,
                parent.owner_symbol,
                self.statement_index,
            ) else {
                return true;
            };
            let Some(segments) =
                loan_place_segments(self.program, reference, &source.path[..compared])
            else {
                return true;
            };
            owner_path
                .iter()
                .zip(segments)
                .all(|(owner, selected)| match (owner, selected) {
                    (BorrowLoanOwnerSegment::Field(owner), PlaceSegment::Field { symbol }) => {
                        *owner == symbol
                    }
                    (
                        BorrowLoanOwnerSegment::FixedIndex(owner),
                        PlaceSegment::FixedIndex { index },
                    ) => *owner == index,
                    (BorrowLoanOwnerSegment::Case(owner), PlaceSegment::Case { variant }) => {
                        *owner == variant
                    }
                    (BorrowLoanOwnerSegment::DynamicIndex, _) => true,
                    _ => false,
                })
        }

        /*
            The symbol owns a home this plan already established: a structural
            local binding, or a named result rooted at that symbol. An expression
            carrier with the symbol erased is not that home.
        */
        fn owner_home_established(&self, symbol: SymbolHandle) -> bool {
            if !symbol.is_valid() {
                return false;
            }
            self.operations.iter().any(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishLocal(
                        CheckedUnitLocalBindingPlan::Structural { symbol: local, .. }
                    ) if *local == symbol
                )
            }) || self.results.iter().any(|(result, root)| {
                *root == PlaceRoot::Symbol(symbol) && result.local.symbol == symbol
            })
        }

        pub(super) fn holder_retired(&self, handle: Handle<BorrowLoanFact>) -> bool {
            self.holder_retired_following(handle, &mut Vec::new())
        }

        /*
            Calls can publish receiving loans through results or stored effects.
            The supplying occurrence, not the syntactic placement of the call,
            identifies which receivers must finish before its backing may retire.
        */
        fn call_receivers_retired(
            &self,
            supplier: Handle<BorrowLoanFact>,
            statement_index: u32,
            requires_receiver: bool,
            active: &mut Vec<RetirementNode>,
        ) -> bool {
            if !self
                .order
                .precedes(statement_index as usize, self.statement_index as usize)
            {
                return false;
            }
            let Some(resources) = self.resource_loans() else {
                return false;
            };
            let mut found_receiver = false;
            let retired = resources
                .filter(|(receiver, loan)| {
                    loan.statement_index == statement_index as usize
                        && transport::supplies(self.transported_loans, supplier, *receiver)
                })
                .all(|(receiver, _)| {
                    found_receiver = true;
                    self.holder_retired_following(receiver, active)
                });
            retired && (!requires_receiver || found_receiver)
        }

        fn holder_retired_following(
            &self,
            handle: Handle<BorrowLoanFact>,
            active: &mut Vec<RetirementNode>,
        ) -> bool {
            let Some(Some(state)) =
                unique_borrow_state(self.facts, self.machine, self.state.symbol)
            else {
                return false;
            };
            if !self
                .facts
                .borrow
                .state_owns_statement_resource(state, handle)
            {
                return false;
            }
            if active.contains(&RetirementNode::Loan(handle)) {
                return false;
            }
            active.push(RetirementNode::Loan(handle));
            let retired = self.holder_retired_inner(handle, active);
            active.pop();
            retired
        }

        fn holder_retired_inner(
            &self,
            handle: Handle<BorrowLoanFact>,
            active: &mut Vec<RetirementNode>,
        ) -> bool {
            let loan = self.facts.borrow.loans.get(handle);
            if !self
                .order
                .precedes(loan.statement_index, self.statement_index as usize)
            {
                return true;
            }
            if self.disposed(handle) {
                return true;
            }
            if self.at_exit && self.return_transfers_holder(loan) {
                return self.uncommitted_owner == PlaceRoot::Symbol(Default::default());
            }
            if crate::borrow::is_descriptor_load_observation(&self.facts.borrow, loan) {
                return release_boundary(
                    self.program,
                    self.facts,
                    self.machine,
                    self.state.symbol,
                    handle,
                )
                .is_some_and(|boundary| {
                    self.order
                        .at_or_before(boundary as usize, self.statement_index as usize)
                });
            }
            if self.at_exit
                && let Some(weakening) =
                    loan_weakening(self.facts, self.machine, self.state.symbol, handle)
                && matches!(
                    weakening.reason,
                    FlowBorrowWeakeningReason::LastUseExpired
                        | FlowBorrowWeakeningReason::StateExit
                )
                && let FlowInvalidationSource::Statement { statement_index } = weakening.source
                && (self.order.position(statement_index).is_none()
                    || self
                        .order
                        .at_or_before(statement_index, self.statement_index as usize))
                && self.owner_home_established(loan.owner_symbol)
            {
                return true;
            }
            if let Some(weakening) =
                loan_weakening(self.facts, self.machine, self.state.symbol, handle)
                && weakening.reason == FlowBorrowWeakeningReason::LocalReassigned
                && let FlowInvalidationSource::Statement { statement_index } = weakening.source
                && let Ok(statement_index) = u32::try_from(statement_index)
                && unique_borrow_state(self.facts, self.machine, self.state.symbol)
                    .flatten()
                    .and_then(|state| self.facts.borrow.calls.span(state.calls))
                    .is_some_and(|calls| {
                        calls.iter().any(|call| {
                            call.statement_index == statement_index as usize
                                && crate::borrow::calls::replaces_reference_loan(
                                    self.program,
                                    &self.facts.borrow,
                                    call,
                                    handle,
                                )
                        })
                    })
                && self.call_receivers_retired(handle, statement_index, false, active)
            {
                return true;
            }
            let mut holders = self.results.iter().filter(|(holder, root)| {
                *root == PlaceRoot::Symbol(loan.owner_symbol)
                    && holder.statement_index as usize == loan.statement_index
            });
            let holder = holders.next();
            let stored = self.operations.iter().any(|operation| {
                stores::receiver(
                    self.program,
                    self.facts,
                    self.machine,
                    self.state,
                    operation,
                )
                .is_some_and(|(owner, formation, _)| {
                    owner == loan.owner_symbol && formation as usize == loan.statement_index
                }) && stores::loans(self, operation).is_some_and(|groups| {
                    groups
                        .iter()
                        .any(|(_, group)| self.facts.borrow.loans_contain(*group, handle))
                })
            });
            let mut earlier_homes = self.results.iter().filter(|(binding, root)| {
                *root == loan.storage_owner
                    && *root == PlaceRoot::Symbol(loan.owner_symbol)
                    && self
                        .order
                        .precedes(binding.statement_index as usize, loan.statement_index)
            });
            let materialized_home =
                earlier_homes.next().is_some() && earlier_homes.next().is_none();
            if holder.is_none() && !stored && !materialized_home {
                let Some((ordinal, alias)) = self
                    .program
                    .state_local_declaration(self.state, loan.owner_symbol)
                else {
                    return false;
                };
                let unmaterialized_alias = ordinal == loan.statement_index
                    && self.facts.borrow.loan_owner_path(loan).is_empty()
                    && reference_shell(self.program, alias.type_reference).is_some()
                    && !self
                        .results
                        .iter()
                        .any(|(_, root)| *root == PlaceRoot::Symbol(loan.owner_symbol))
                    && u32::try_from(ordinal).ok().is_some_and(|ordinal| {
                        self.facts
                            .values
                            .structural_values
                            .root_for_expression(self.state.symbol, ordinal, alias.initial_value)
                            .is_none()
                    });
                return unmaterialized_alias
                    && release_boundary(
                        self.program,
                        self.facts,
                        self.machine,
                        self.state.symbol,
                        handle,
                    )
                    .is_some_and(|boundary| {
                        self.order
                            .at_or_before(boundary as usize, self.statement_index as usize)
                    });
            }
            if holders.next().is_some() {
                return false;
            }
            let source = holder.map_or(
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: loan.owner_symbol,
                },
                |(holder, _)| CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: holder.binding_ordinal,
                },
            );
            let Ok(formation) = u32::try_from(loan.statement_index) else {
                return false;
            };
            let Some(reference) = storage_owner_type_reference(
                self.program,
                self.state,
                loan.owner_symbol,
                formation,
            ) else {
                return false;
            };
            let Some(selection) = transport::carrier_path(
                self.program,
                reference,
                self.facts.borrow.loan_owner_path(loan),
            ) else {
                return false;
            };
            let Some(transfers) = transfers::from_storage(
                self.program,
                self.facts,
                self.prepared_calls,
                self.results,
                source.clone(),
                formation,
                &selection,
                self.operations,
            ) else {
                return false;
            };
            if self.transferred_holder_retired(handle, reference, &[&transfers], None, active) {
                return true;
            }
            self.completion_retires_holder(handle, source, formation, &selection, reference, active)
        }

        fn transferred_holder_retired(
            &self,
            handle: Handle<BorrowLoanFact>,
            reference: typed_trees::types::TypeReferenceHandle,
            transfers: &[&crate::execution::terminal_unit::storage_transfers::Transfers],
            exit: Option<checked_trees::FlowClaimJoinExit>,
            active: &mut Vec<RetirementNode>,
        ) -> bool {
            let loan = self.facts.borrow.loans.get(handle);
            let owner_path = self.facts.borrow.loan_owner_path(loan);
            let selected_path = |path: &[CheckedUnitStructuralPathSegment]| {
                loan_place_segments(self.program, reference, path).and_then(|segments| {
                    segments
                        .into_iter()
                        .map(|segment| match segment {
                            PlaceSegment::Field { symbol } => {
                                Some(BorrowLoanOwnerSegment::Field(symbol))
                            }
                            PlaceSegment::FixedIndex { index } => {
                                Some(BorrowLoanOwnerSegment::FixedIndex(index))
                            }
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()
                })
            };
            let definitely_transferred = transfers
                .iter()
                .flat_map(|row| &row.definite)
                .any(|path| selected_path(path).is_some_and(|path| owner_path.starts_with(&path)));
            let cleaned = !definitely_transferred
                && exit
                    .is_some_and(|exit| self.exit_cleanup_retires_holder(handle, reference, exit));
            if !cleaned && !definitely_transferred {
                return false;
            }
            let mut found = false;
            for transfer in transfers.iter().flat_map(|row| &row.possible) {
                let Some(selected) = selected_path(&transfer.source) else {
                    return false;
                };
                if !owner_path.starts_with(&selected) {
                    continue;
                }
                found = true;
                let (destination, mut path) = match &transfer.destination {
                    Destination::Unknown | Destination::RetainedBySuccessor => return false,
                    Destination::Outgoing => {
                        if exit.is_none()
                            || self.uncommitted_owner != PlaceRoot::Symbol(Default::default())
                        {
                            return false;
                        }
                        continue;
                    }
                    Destination::Backing(value, path) => {
                        let mut path = path.clone();
                        path.extend_from_slice(&owner_path[selected.len()..]);
                        if !self.backing_retired(*value, &path, active) {
                            return false;
                        }
                        continue;
                    }
                    Destination::Call {
                        statement_index,
                        structural_result,
                    } => {
                        if !self.call_receivers_retired(
                            handle,
                            *statement_index,
                            *structural_result,
                            active,
                        ) {
                            return false;
                        }
                        continue;
                    }
                    Destination::Storage {
                        statement_index,
                        captured_selection,
                    } => {
                        if !self
                            .order
                            .precedes(*statement_index as usize, self.statement_index as usize)
                        {
                            return false;
                        }
                        let Some((symbol, path)) = stores::destination(
                            self.program,
                            self.facts,
                            self.machine,
                            self.state,
                            *statement_index,
                            captured_selection,
                        ) else {
                            return false;
                        };
                        let Some(reference) = storage_owner_type_reference(
                            self.program,
                            self.state,
                            symbol,
                            *statement_index,
                        ) else {
                            return false;
                        };
                        let Some(mut receiving_path) =
                            validation::reference_result_custody::loan_owner_path(
                                self.program,
                                reference,
                                &path,
                            )
                        else {
                            return false;
                        };
                        receiving_path.extend_from_slice(&owner_path[selected.len()..]);
                        let Some(resources) = self.resource_loans() else {
                            return false;
                        };
                        let mut found_receiver = false;
                        let retired = resources
                            .filter(|(_, receiver)| {
                                receiver.owner_symbol == symbol
                                    && receiver.statement_index == *statement_index as usize
                                    && self.facts.borrow.loan_owner_path(receiver) == receiving_path
                            })
                            .all(|(receiver, _)| {
                                found_receiver = true;
                                self.holder_retired_following(receiver, active)
                            });
                        if !retired || !found_receiver {
                            return false;
                        }
                        continue;
                    }
                    Destination::Holder(destination, path) => (destination, path.clone()),
                };
                if !self.order.precedes(
                    destination.statement_index as usize,
                    self.statement_index as usize,
                ) {
                    return false;
                }
                path.extend_from_slice(&owner_path[selected.len()..]);
                if self.return_receives_carrier(destination, &path, loan) {
                    if self.uncommitted_owner != PlaceRoot::Symbol(Default::default()) {
                        return false;
                    }
                    continue;
                }
                if self.carrier_retired(destination, active) {
                    continue;
                }
                let mut bindings = self
                    .results
                    .iter()
                    .filter(|(binding, _)| binding == destination);
                let Some((_, PlaceRoot::Symbol(owner))) = bindings.next() else {
                    return false;
                };
                if bindings.next().is_some() {
                    return false;
                }
                let Some(resources) = self.resource_loans() else {
                    return false;
                };
                let mut receiving = resources.filter(|(_, candidate)| {
                    candidate.owner_symbol == *owner
                        && candidate.statement_index == destination.statement_index as usize
                        && self.facts.borrow.loan_owner_path(candidate) == path
                });
                let mut found_receiver = false;
                if !receiving.all(|(received, _)| {
                    found_receiver = true;
                    self.holder_retired_following(received, active)
                }) || !found_receiver
                {
                    return false;
                }
            }
            found || cleaned
        }

        fn return_transfers_holder(&self, loan: &BorrowLoanFact) -> bool {
            let path = self.facts.borrow.loan_owner_path(loan);
            self.results.iter().any(|(holder, root)| {
                *root == loan.storage_owner
                    && *root == PlaceRoot::Symbol(loan.owner_symbol)
                    && holder.statement_index as usize == loan.statement_index
                    && self.return_receives_carrier(holder, path, loan)
            })
        }

        /*
            A definite owned transfer can end in the anonymous completion value,
            which has no source-local loan holder. Join that receiving occurrence to
            the actual captured return, rather than inventing a named local for it.
        */
        fn return_receives_carrier(
            &self,
            receiver: &CheckedUnitStructuralResultBindingPlan,
            path: &[BorrowLoanOwnerSegment],
            loan: &BorrowLoanFact,
        ) -> bool {
            let Some(returned) = self.returned.filter(|_| self.at_exit) else {
                return false;
            };
            if !returned_reference_binding(self.order, returned, receiver) {
                return false;
            }
            let Some(path) = transport::carrier_path(self.program, self.state.return_type, path)
            else {
                return false;
            };
            let (statement_index, expression, transition_target) = match returned.completion {
                ReturnCompletion::Value {
                    statement_index,
                    expression,
                    evaluation,
                } => {
                    let target = match evaluation {
                        FlowPermissionEvaluation::Statement => Default::default(),
                        FlowPermissionEvaluation::TransitionTarget(target) => target,
                        _ => return false,
                    };
                    (statement_index as usize, Some(expression), target)
                }
                ReturnCompletion::Call { coordinate } => {
                    let Some(machine) = self.program.machine_by_symbol(self.machine) else {
                        return false;
                    };
                    let target = transition_call_target(
                        self.program,
                        machine,
                        self.state,
                        coordinate.statement_index as usize,
                        coordinate.call_ordinal as usize,
                    )
                    .unwrap_or_default();
                    (coordinate.statement_index as usize, None, target)
                }
            };
            if loan.last_use_statement_index != statement_index {
                return false;
            }
            let Some(segments) = self.facts.borrow.access_segments.span(loan.segments) else {
                return false;
            };
            let Some(Some(state)) =
                unique_borrow_state(self.facts, self.machine, self.state.symbol)
            else {
                return false;
            };
            self.facts
                .borrow
                .returned_loans
                .span(state.returned_loans)
                .is_some_and(|captured| {
                    captured.iter().any(|captured| {
                        captured.statement_index == statement_index
                            && captured.transition_target == transition_target
                            && expression.is_none_or(|expression| {
                                captured.formation_expression == expression
                            })
                            && captured.root == loan.root
                            && captured.incoming_reference == loan.incoming_reference
                            && self
                                .facts
                                .borrow
                                .access_segments
                                .span(captured.incoming_owner_path)
                                == self
                                    .facts
                                    .borrow
                                    .access_segments
                                    .span(loan.incoming_owner_path)
                            && captured.kind == loan.kind
                            && self.facts.borrow.access_segments.span(captured.segments)
                                == Some(segments)
                            && self
                                .facts
                                .borrow
                                .owner_segments
                                .span(captured.owner_path)
                                .and_then(|owner| {
                                    transport::carrier_path(
                                        self.program,
                                        self.state.return_type,
                                        owner,
                                    )
                                })
                                .is_some_and(|captured_path| captured_path == path)
                    })
                })
        }

        /*
            A source owner in the result roster is provenance, not a statement
            that an anonymous result is the owner's carrier. Translate an owned
            extraction's selected subtree; borrowed extractions retain their carrier.
        */
        fn disposed_result_path(
            &self,
            binding_ordinal: u32,
            path: &[CheckedUnitStructuralPathSegment],
            loan: &BorrowLoanFact,
        ) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
            let mut holders = self
                .results
                .iter()
                .filter(|(holder, _)| holder.binding_ordinal == binding_ordinal);
            let (holder, _) = holders.next()?;
            if holders.next().is_some() {
                return None;
            }
            if holder.local.symbol == loan.owner_symbol
                && holder.statement_index as usize == loan.statement_index
            {
                return Some(path.to_vec());
            }
            let mut producers = self
                .operations
                .iter()
                .chain(self.pending)
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::MoveStructuralField {
                        source,
                        result,
                        expression,
                    } if result.binding_ordinal == binding_ordinal => {
                        Some((source, result, expression))
                    }
                    _ => None,
                });
            let (source, result, expression) = producers.next()?;
            if !self
                .order
                .precedes(loan.statement_index, result.statement_index as usize)
                || producers.next().is_some()
                || source.source
                    != (CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol: loan.owner_symbol,
                    })
                || !transfers::extraction_owns_source(self.program, source, result, *expression)?
            {
                return None;
            }
            let mut selected = source.path.clone();
            selected.extend_from_slice(path);
            Some(selected)
        }

        /*
            Whether the contextual owner's death is already settled by a completed
            exact call cleanup: one disposition names the call that owns it and,
            before exit, that call's continuation cleanup has been planned. A
            future hook or a dependency edge cannot settle it. Both the retained
            loan owner and an anonymous backing owner ask this same question.
        */
        fn call_cleanup_settles_owner(&self, owner: &PlaceRoot, statement_index: usize) -> bool {
            if *owner == self.uncommitted_owner {
                return false;
            }
            self.facts
                .contextual_cleanup
                .any_state_disposition(self.machine, self.state.symbol, |_, disposition| {
                    disposition.root == *owner
                        && disposition.path.is_empty()
                        && disposition.exclusions.is_empty()
                        && matches!(disposition.source,
                        PermissionEventSource::Call { statement_index: call_statement, call_ordinal, .. }
                            if call_statement == statement_index
                                && (self.at_exit || self.operations[..self.completed_operations].iter().any(|operation| {
                                    matches!(operation, CheckedUnitEffectOperationPlan::ContinuationCleanup {
                                        boundary: CleanupBoundary::Call(coordinate), ..
                                    } if coordinate.statement_index as usize == call_statement
                                        && coordinate.call_ordinal as usize == call_ordinal)
                                })))
                })
        }

        /*
            Disposal ends an occurrence, not all future values in the same local.
            Replacement's checked boundary captures the old destination after RHS
            evaluation. A release instead names the exact retained loan group.
        */
        pub(super) fn disposed(&self, handle: Handle<BorrowLoanFact>) -> bool {
            let loan = self.facts.borrow.loans.get(handle);
            if self
                .order
                .precedes(loan.statement_index, self.statement_index as usize)
                && self.call_cleanup_settles_owner(&loan.storage_owner, loan.statement_index)
            {
                return true;
            }
            self.operations
                .iter()
                .chain(self.pending)
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .any(|operation| match operation {
                    CheckedUnitEffectOperationPlan::Store(store) => {
                        stores::replaces(self, store, handle)
                    }
                    CheckedUnitEffectOperationPlan::ReleaseReference {
                        statement_index,
                        loans,
                        selection_complements,
                        defer_to_exit,
                        ..
                    } => {
                        (!*defer_to_exit || self.at_exit)
                            && self.order.at_or_before(
                                *statement_index as usize,
                                self.statement_index as usize,
                            )
                            && (self.facts.borrow.loans_contain(*loans, handle)
                                || selection_complements.iter().any(|complement| {
                                    selections::disposes_candidate(self, handle, complement)
                                }))
                    }
                    CheckedUnitEffectOperationPlan::ContinuationCleanup {
                        boundary,
                        cleanup_actions,
                        ..
                    } => {
                        let statement = match boundary {
                            CleanupBoundary::Replacement {
                                statement_index, ..
                            }
                            | CleanupBoundary::StatementEntry { statement_index } => {
                                *statement_index
                            }
                            _ => return false,
                        };
                        if self
                            .order
                            .precedes(self.statement_index as usize, statement as usize)
                            || !self
                                .order
                                .precedes(loan.statement_index, statement as usize)
                        {
                            return false;
                        }
                        let Some(reference) = storage_owner_type_reference(
                            self.program,
                            self.state,
                            loan.owner_symbol,
                            statement,
                        ) else {
                            return false;
                        };
                        if self.operations.iter().any(|operation| {
                            let Some((owner, formation, selection)) = stores::receiver(
                                self.program,
                                self.facts,
                                self.machine,
                                self.state,
                                operation,
                            ) else {
                                return false;
                            };
                            self.order
                                .precedes(loan.statement_index, formation as usize)
                                && self.order.precedes(formation as usize, statement as usize)
                                && owner == loan.owner_symbol
                                && validation::reference_result_custody::loan_owner_path(
                                    self.program,
                                    reference,
                                    &selection,
                                )
                                .is_some_and(|path| {
                                    self.facts.borrow.loan_owner_path(loan).starts_with(&path)
                                })
                        }) {
                            return false;
                        }
                        cleanup_actions.iter().any(|discard| {
                            let path = match discard.source() {
                                CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                    binding_ordinal,
                                } => {
                                    let Some(path) = self.disposed_result_path(
                                        *binding_ordinal,
                                        discard.path(),
                                        loan,
                                    ) else {
                                        return false;
                                    };
                                    path
                                }
                                source
                                    if stores::source_owner(self.program, self.state, source)
                                        == Some(loan.owner_symbol) =>
                                {
                                    discard.path().to_vec()
                                }
                                _ => return false,
                            };
                            validation::reference_result_custody::loan_owner_path(
                                self.program,
                                reference,
                                &path,
                            )
                            .is_some_and(|path| {
                                self.facts.borrow.loan_owner_path(loan).starts_with(&path)
                            })
                        })
                    }
                    _ => false,
                })
        }
    }

    /*
        An established descriptor retains every whole-owner loan occurrence.
        Match its registered local binding, rather than re-reading the initializer.
    */
    pub(super) fn local_reference_loans(
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        result: &CheckedUnitStructuralResultBindingPlan,
        results: &Results,
    ) -> Option<HandleSpan<BorrowLoanFact>> {
        let loans = carrier_loans(facts, machine, state, result, results)?;
        facts
            .borrow
            .loan_group(loans)?
            .all(|(_, loan)| facts.borrow.loan_owner_path(loan).is_empty())
            .then_some(loans)
    }

    /*
        Retained occurrences of this exact storage establishment, whether stored
        at the root or beneath record/array paths. The path-specific retirement
        owner still decides how each descriptor ends. An empty proven roster is
        distinct from a missing or ambiguous producer relationship.
    */
    pub(super) fn carrier_loans(
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        result: &CheckedUnitStructuralResultBindingPlan,
        results: &Results,
    ) -> Option<HandleSpan<BorrowLoanFact>> {
        let mut roots = results.iter().filter(|(binding, _)| binding == result);
        let (_, owner) = roots.next()?;
        if !matches!(owner, PlaceRoot::Symbol(_) | PlaceRoot::Expression(_)) {
            return None;
        }
        if roots.next().is_some() {
            return None;
        }
        let borrowing = unique_borrow_state(facts, machine, state)??;
        facts
            .borrow
            .storage_loans(borrowing, *owner, result.statement_index as usize)
    }
}
mod result_consumption {

    /*
        Result and source consumption: structural result bindings are appended
        per operation, retained results feed a call argument, and value-call
        sources place the operation their operand evaluates into.
        Producers and prior retirement receipts share the retained value-call
        inventory: nesting cannot hide a completed release or authorize a second one.
        One structural value's operand custody is one sequence: retain its call
        plans, settle the sources those calls evaluate under the statement's
        owned-selection policy, and adopt the anonymous producers they consume.
    */
    use super::super::super::borrowed_windows::OpenWindows;
    use super::super::super::local_storage::occupant;
    use super::super::super::reference_results::release_boundary;
    use super::super::super::{
        CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEntryClaimPlan,
        CheckedUnitStructuralArgumentSourcePlan as SourcePlan,
    };
    use super::super::structural_operands::value_calls;
    use super::super::{
        CheckFacts, CheckedStructuralAccess as StructuralAccess,
        CheckedUnitEffectOperationPlan as OperationPlan,
        CheckedUnitStructuralParameterPlan as ParameterPlan,
        CheckedUnitStructuralResultBindingPlan as BindingPlan, Multiplicity, PermissionEventSource,
        StatementNode, SymbolHandle,
    };
    use super::reference_retirement::OwnerCandidates;
    use super::reference_retirement::Retirement;
    use super::reference_retirement::carrier_loans;
    use super::reference_retirement::local_reference_loans;
    use super::reference_retirement::prepare_case_conditions;
    use super::reference_retirement::prepare_owner_cleanups;
    use super::reference_retirement::returned_reference_binding;
    use super::reference_retirement::selections::complement_completions;
    use super::reference_retirement::unique_borrow_state;
    use crate::execution::terminal_unit::ScalarCalleeClosure;
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::types::ShapeCollector;
    use arena::Handle;
    use arena::HandleSpan;
    use checked_trees::{
        BorrowLoanFact, CheckedNormalCleanupAction, CheckedStructuralRecordFieldValue,
        CheckedStructuralValueCall, CheckedStructuralValueHandle, CheckedStructuralValueKind,
        CheckedUnitCallPlan, CheckedUnitCleanupBoundary, CheckedUnitLocalBindingPlan,
        CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralReturnPlan,
        CheckedValueCompletion, FlowBorrowWeakeningReason, FlowInvalidationSource,
        NominalMachineUseSite,
    };
    use facts::PlaceRoot;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::StatementExecutionOrder;
    use validation::ContentsRequirement;
    use validation::call_target_return_type;
    use validation::has_contents;
    use validation::reference_result_custody::loan_weakening;
    use validation::reference_result_custody::local_record_loans;
    use validation::reference_result_custody::local_storage_loans;
    use validation::reference_result_custody::stored_loan_parts;
    use validation::reference_result_custody::whole_local_release_boundary;

    /*
        The latest release boundary any loan in the group reaches: a descriptor
        retires after every alternative source and every descendant can end.
    */
    fn latest_loan_release_boundary(
        order: &StatementExecutionOrder,
        facts: &CheckFacts,
        loans: HandleSpan<BorrowLoanFact>,
        release: impl Fn(Handle<BorrowLoanFact>) -> Option<u32>,
    ) -> Option<u32> {
        facts
            .borrow
            .loan_group(loans)?
            .try_fold(0u32, |boundary, (loan, _)| {
                u32::try_from(order.latest(boundary as usize, release(loan)? as usize)?).ok()
            })
    }

    /*
        A carrier replaced in its declared home belongs to the replacement's
        checked cleanup, not statement-entry expiry. Every alternative must name
        that same disposition; an incomplete or mixed group cannot suppress release.
        This remains true while scanning statements before the replacement exists
        in the operation list. The assignment planner must still produce its normal
        cleanup after capturing the RHS, and completion checks that disposition.
        A descendant can keep the backing loan live beyond this carrier's death;
        its lexical last use does not move the recorded replacement boundary.
    */
    fn replacement_owns_release(
        order: &StatementExecutionOrder,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        loans: HandleSpan<BorrowLoanFact>,
    ) -> bool {
        let Some(group) = facts.borrow.loan_group(loans) else {
            return false;
        };
        let mut replacement = None;
        for (handle, loan) in group {
            let Some(weakening) = loan_weakening(facts, machine, state, handle) else {
                return false;
            };
            let FlowInvalidationSource::Statement { statement_index } = weakening.source else {
                return false;
            };
            if weakening.reason != FlowBorrowWeakeningReason::LocalReassigned
                || !order.precedes(loan.statement_index, statement_index)
                || replacement.is_some_and(|previous| previous != statement_index)
            {
                return false;
            }
            replacement = Some(statement_index);
        }
        replacement.is_some()
    }

    /*
        A selected call death closes this descriptor's anonymous backing before
        the frame return. The retained graph authenticates membership; the exact
        contextual disposition, not that membership, authorizes the owner's death.
    */
    fn normal_call_disposes_backing(
        facts: &CheckFacts,
        state: SymbolHandle,
        results: &[(BindingPlan, PlaceRoot)],
        operations: &[OperationPlan],
        binding: u32,
    ) -> bool {
        let values = &facts.values.structural_values;
        let Some((holder, PlaceRoot::Expression(expression))) = results
            .iter()
            .find(|(result, _)| result.binding_ordinal == binding)
        else {
            return false;
        };
        let Some(root) = values.root_for_expression(state, holder.statement_index, *expression)
        else {
            return false;
        };
        let Some(dependencies) = values.value_dependencies(root.root) else {
            return false;
        };
        operations
            .iter()
            .rev()
            .take_while(|operation| {
                matches!(
                    operation,
                    OperationPlan::ContinuationCleanup {
                        boundary: CheckedUnitCleanupBoundary::Call(_),
                        ..
                    }
                )
            })
            .any(|operation| {
                let OperationPlan::ContinuationCleanup {
                    boundary: CheckedUnitCleanupBoundary::Call(coordinate),
                    cleanup_actions,
                    ..
                } = operation
                else {
                    return false;
                };
                cleanup_actions.iter().any(|action| {
                    let expression = match *action.source() {
                        SourcePlan::StructuralExpression { expression, .. } => expression,
                        SourcePlan::StructuralResult { binding_ordinal } => {
                            let Some((_, owner)) = results
                                .iter()
                                .find(|(result, _)| result.binding_ordinal == binding_ordinal)
                            else {
                                return false;
                            };
                            let Some(expression) = owner.as_expression() else {
                                return false;
                            };
                            expression
                        }
                        _ => return false,
                    };
                    let Some(owner) =
                        values.root_for_expression(state, coordinate.statement_index, expression)
                    else {
                        return false;
                    };
                    owner.root != root.root
                        && dependencies.contains(&owner.root)
                        && facts.contextual_cleanup.any_state_disposition(
                            root.machine,
                            state,
                            |_, disposition| {
                                disposition.root == PlaceRoot::Expression(expression)
                                    && matches!(
                                        disposition.source,
                                        PermissionEventSource::Call {
                                            statement_index,
                                            call_ordinal,
                                            ..
                                        } if statement_index == coordinate.statement_index as usize
                                            && call_ordinal == coordinate.call_ordinal as usize
                                    )
                            },
                        )
                })
            })
    }

    /*
        Source groups identify named loan resources. A computed descriptor has no
        such group, so its exact result occurrence identifies its one release.
    */
    pub(in crate::execution::terminal_unit) fn release_matches(
        operation: &OperationPlan,
        result: &BindingPlan,
        loans: HandleSpan<BorrowLoanFact>,
    ) -> bool {
        operation.with_value_calls().any(|operation| {
            let OperationPlan::ReleaseReference {
                loans: released,
                source,
                path,
                ..
            } = operation
            else {
                return false;
            };
            if !path.is_empty() {
                return false;
            }
            if loans.is_empty() {
                *source
                    == (SourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    })
            } else {
                *released == loans
            }
        })
    }

    /*
        A whole discard closes both the carrier home and its stored descriptors.
        Statement and exit expiry share this physical receipt: scheduling another
        reference release after it would retire the same result twice. A projected
        or case-conditional action does not close the whole occurrence.
        Only statement-entry actions are final during sequencing. Call and
        replacement actions are still provisional until temporary-action selection;
        their transfer and replacement judgments remain separate below.
    */
    fn whole_result_discarded(operation: &OperationPlan, result: &BindingPlan) -> bool {
        let OperationPlan::ContinuationCleanup {
            boundary: CheckedUnitCleanupBoundary::StatementEntry { .. },
            cleanup_actions,
            ..
        } = operation
        else {
            return false;
        };
        cleanup_actions.iter().any(|action| {
            matches!(action, CheckedNormalCleanupAction::Discard(discard)
                if discard.source == SourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                }
                    && discard.path.is_empty()
                    && discard.type_identity == result.type_identity
                    && discard.case_conditions.is_empty())
        })
    }

    /*
        A prior constructor or call transfer already moved this descriptor's
        custody to another holder.
    */
    fn custody_transferred(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
        results: &[(BindingPlan, PlaceRoot)],
        binding_ordinal: u32,
        operations: &[OperationPlan],
    ) -> Option<bool> {
        Some(
            super::reference_retirement::transferred_paths(
                program,
                facts,
                prepared_calls,
                results,
                binding_ordinal,
                operations,
            )?
            .iter()
            .any(|path| path.is_empty()),
        )
    }

    /*
        A materialized carrier owns the same captured loan group whether formed
        locally or returned by a call. Scheduling and custody accounting must join
        that exact holder rather than select retirement by producer family.
    */
    pub(in crate::execution::terminal_unit) struct CapturedReferenceResult<'operation> {
        pub result: &'operation BindingPlan,
        pub loans: HandleSpan<BorrowLoanFact>,
        /*
            Scalar siblings and borrows of the carrier home can outlive its last
            descriptor observation. Whole disposal must retain both inventories.
        */
        whole_storage_boundary: u32,
        storage_loans: Vec<Handle<BorrowLoanFact>>,
    }

    /*
        Every named carrier retains its own lexical uses and borrowers, regardless
        of whether a call, constructor, or reference formation produced it. An
        anonymous carrier has no later authored uses; the result dependency graph
        still retains it for descriptors formed from it.
    */
    fn captured_storage_lifetime(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        results: &[(BindingPlan, PlaceRoot)],
        result: &BindingPlan,
    ) -> Option<(u32, Vec<Handle<BorrowLoanFact>>)> {
        let mut owners = results.iter().filter(|(binding, _)| binding == result);
        let (_, owner) = owners.next()?;
        if owners.next().is_some() {
            return None;
        }
        let PlaceRoot::Symbol(owner) = owner else {
            return Some((0, Vec::new()));
        };
        let authored = program.state_by_symbol(state)?;
        let (formation, _) = program.state_local_declaration(authored, *owner)?;
        if formation != result.statement_index as usize {
            return None;
        }
        let boundary =
            whole_local_release_boundary(program, facts, machine, authored, result.statement_index);
        if boundary.is_none() && ::validation::trace_flags::check_sites() {
            let groups =
                local_record_loans(program, facts, machine, authored, result.statement_index);
            eprintln!(
                "CHECKSITE captured-holder-boundary-unavailable state={state:?} result={result:?} owner={owner:?} groups={groups:?}"
            );
            if let Some(groups) = groups {
                for (_, group) in groups {
                    for (handle, loan) in facts.borrow.loan_group(group).into_iter().flatten() {
                        eprintln!(
                            "CHECKSITE captured-holder-boundary-loan state={state:?} result={} loan={handle:?} row={loan:?} weakening={:?} release={:?}",
                            result.binding_ordinal,
                            loan_weakening(facts, machine, state, handle),
                            validation::reference_result_custody::release_statement(
                                program, facts, machine, state, handle
                            )
                        );
                    }
                }
            }
        }
        Some((
            boundary?,
            local_storage_loans(facts, machine, state, *owner)?,
        ))
    }

    pub(in crate::execution::terminal_unit) fn captured_reference_result<'operation>(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        results: &[(BindingPlan, PlaceRoot)],
        operation: &'operation OperationPlan,
        operations: &[OperationPlan],
    ) -> Option<CapturedReferenceResult<'operation>> {
        let direct = match operation {
            OperationPlan::EstablishReference { result, .. } => {
                local_reference_loans(facts, machine, state, result, results)
                    .map(|loans| (result, loans))
            }
            OperationPlan::EstablishLocal(CheckedUnitLocalBindingPlan::StructuralValue {
                result,
                value,
                ..
            }) if facts.values.structural_values.nodes.is_valid(*value)
                && stored_loan_parts(
                    program,
                    facts
                        .values
                        .structural_values
                        .nodes
                        .get(*value)
                        .type_reference,
                )
                .is_some() =>
            {
                local_reference_loans(facts, machine, state, result, results)
                    .map(|loans| (result, loans))
            }
            _ => operation.call().and_then(|call| {
                let custody = call.structural_custody()?;
                let result = call.structural_result()?;
                call_target_return_type(program, call.target.state)
                    .is_some_and(|returned| {
                        has_contents(
                            program,
                            returned,
                            &[],
                            ContentsRequirement::DescriptorCleanup,
                        ) && (!custody.reference_loans.is_empty()
                            || stored_loan_parts(program, returned).is_some())
                    })
                    .then_some((result, custody.reference_loans))
            }),
        };
        if ::validation::trace_flags::check_sites() {
            let result = match operation {
                OperationPlan::EstablishReference { result, .. }
                | OperationPlan::EstablishLocal(CheckedUnitLocalBindingPlan::StructuralValue {
                    result,
                    ..
                }) => Some(result),
                _ => None,
            };
            if let Some(result) = result {
                eprintln!(
                    "CHECKSITE captured-retirement-inventory state={state:?} result={result:?} direct={direct:?} owners={:?}",
                    results
                        .iter()
                        .filter(|(binding, _)| binding == result)
                        .collect::<Vec<_>>()
                );
            }
        }
        if let Some((result, loans)) = direct {
            let (whole_storage_boundary, storage_loans) =
                captured_storage_lifetime(program, facts, machine, state, results, result)?;
            return Some(CapturedReferenceResult {
                result,
                loans,
                whole_storage_boundary,
                storage_loans,
            });
        }
        let (result, _) = operation.discardable_structural_result()?;
        let loans = carrier_loans(facts, machine, state, result, results)?;
        if loans.is_empty() {
            return None;
        }
        let authored_state = program.state_by_symbol(state)?;
        let mut owners = results.iter().filter(|(binding, _)| binding == result);
        let (_, PlaceRoot::Symbol(owner)) = owners.next()? else {
            return None;
        };
        if owners.next().is_some() {
            return None;
        }
        let (formation, local) = program.state_local_declaration(authored_state, *owner)?;
        if formation != result.statement_index as usize
            || !has_contents(
                program,
                local.type_reference,
                &[],
                ContentsRequirement::DescriptorCleanup,
            )
            || !super::reference_retirement::transferred_paths(
                program,
                facts,
                &crate::execution::terminal_unit::structural_value_calls::PreparedCalls::EMPTY,
                results,
                result.binding_ordinal,
                operations,
            )?
            .is_empty()
            || unique_borrow_state(facts, machine, state)?.is_some_and(|borrowing| {
                facts
                    .borrow
                    .loan_group(borrowing.loans)
                    .is_some_and(|mut loans| {
                        loans.any(|(_, loan)| {
                            loan.storage_owner == PlaceRoot::Symbol(*owner)
                                && loan.statement_index != formation
                        })
                    })
            })
        {
            return None;
        }
        if result.multiplicity != Multiplicity::Unrestricted
            && crate::execution::terminal_unit::storage_transfers::has_computation_transfer(
                program, facts, state, results, result, operations,
            )?
        {
            return None;
        }
        let (whole_storage_boundary, storage_loans) =
            captured_storage_lifetime(program, facts, machine, state, results, result)?;
        Some(CapturedReferenceResult {
            result,
            loans,
            whole_storage_boundary,
            storage_loans,
        })
    }

    /*
        End captured reference custody at entry to the next authored statement.
        Whole carriers use ordinary affine edge disposal, which retires every leaf;
        reference results retain one release of their complete group. Reverse production order
        closes descendants before earlier parent carriers at a shared boundary.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::execution::terminal_unit) fn append_reference_releases(
        owner_candidates: &OwnerCandidates,
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
        shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
        parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: u32,
        results: &[(BindingPlan, PlaceRoot)],
        operations: &mut Vec<OperationPlan>,
        trace: &super::super::super::LocalConstructionTrace,
    ) -> Option<()> {
        trace.phase("reference retirement: transport");
        let authored_state = program.state_by_symbol(state)?;
        let borrowing = unique_borrow_state(facts, machine, state)?;
        if borrowing.is_none_or(|borrowing| borrowing.loans.is_empty())
            && !operations
                .iter()
                .chain(prepared_calls.operations_for(machine, state))
                .flat_map(OperationPlan::with_value_calls)
                .any(|operation| {
                    captured_reference_result(
                        program, facts, machine, state, results, operation, operations,
                    )
                    .is_some()
                })
        {
            return Some(());
        }
        let order = program
            .statement_table
            .admitted_execution_order(authored_state.statement_nodes);
        let transported_loans = super::reference_retirement::transport::collect(
            program,
            facts,
            machine,
            authored_state,
            results,
            operations,
        )?;
        let mut pending = Vec::new();
        let mut retired = Vec::new();
        let statement_retirement = Retirement {
            program,
            trace,
            facts,
            prepared_calls,
            machine,
            state: authored_state,
            order: &order,
            statement_index,
            results,
            operations,
            completed_operations: operations.len(),
            pending: &[],
            at_exit: false,
            uncommitted_owner: PlaceRoot::Symbol(Default::default()),
            returned: None,
            completion: None,
            transported_loans: &transported_loans,
            storage_borrowers: &owner_candidates.storage_borrowers,
        };
        for operation in operations
            .iter()
            .rev()
            .flat_map(OperationPlan::with_value_calls)
        {
            trace.phase("reference retirement: selected complements");
            let complements = complement_completions(&Retirement {
                pending: &pending,
                ..statement_retirement
            })?;
            for (position, complement) in complements {
                trace.phase("reference retirement: complement receiver");
                let OperationPlan::ReleaseReference {
                    selection_complements,
                    ..
                } = pending.get_mut(position)?
                else {
                    return None;
                };
                selection_complements.push(complement);
            }
            trace.phase("reference retirement: stored occurrences");
            let stored_releases = super::reference_retirement::stores::releases(
                &Retirement {
                    pending: &pending,
                    ..statement_retirement
                },
                operation,
            )?;
            pending.extend(stored_releases);
            if operation
                .discardable_structural_result()
                .is_some_and(|(result, _)| {
                    operations
                        .iter()
                        .chain(&pending)
                        .flat_map(OperationPlan::with_value_calls)
                        .any(|operation| whole_result_discarded(operation, result))
                })
            {
                continue;
            }
            let reference_result = captured_reference_result(
                program, facts, machine, state, results, operation, operations,
            );
            if let Some(captured) = reference_result {
                let result = captured.result;
                let loans = captured.loans;
                if replacement_owns_release(&order, facts, machine, state, loans) {
                    continue;
                }
                trace.phase("reference retirement: captured custody transfer");
                if custody_transferred(
                    program,
                    facts,
                    prepared_calls,
                    results,
                    result.binding_ordinal,
                    operations,
                )? {
                    continue;
                }

                trace.phase("reference retirement: captured loan boundary");
                let boundary = latest_loan_release_boundary(&order, facts, loans, |loan| {
                    release_boundary(program, facts, machine, state, loan)
                })?;
                trace.phase("reference retirement: captured storage boundary");
                let boundary = order.latest(
                    boundary as usize,
                    order.successor(result.statement_index as usize)?,
                )?;
                let boundary = u32::try_from(
                    order.latest(boundary, captured.whole_storage_boundary as usize)?,
                )
                .ok()?;
                let retirement = Retirement {
                    pending: &pending,
                    ..statement_retirement
                };
                trace.phase("reference retirement: captured loan group");
                if order.at_or_before(boundary as usize, statement_index as usize)
                    && retirement.carrier_children_retired(result)
                    && captured
                        .storage_loans
                        .iter()
                        .all(|loan| retirement.holder_retired(*loan))
                    && facts
                        .borrow
                        .loan_group(loans)?
                        .all(|(loan, _)| retirement.children_retired(loan))
                    && !operations
                        .iter()
                        .chain(&pending)
                        .flat_map(OperationPlan::with_value_calls)
                        .any(|operation| release_matches(operation, result, loans))
                {
                    pending.push(OperationPlan::ReleaseReference {
                        statement_index,
                        source: SourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                        path: Vec::new(),
                        select_evaluated_storage: false,
                        defer_to_exit: false,
                        loans,
                        selection_complements: Vec::new(),
                        owner_cleanups: HandleSpan::empty(),
                        case_conditions: Default::default(),
                    });
                    if operation.discardable_structural_result().is_some() {
                        retired.push(result.binding_ordinal);
                    }
                }
                if captured.whole_storage_boundary == 0
                    || operations
                        .iter()
                        .chain(&pending)
                        .flat_map(OperationPlan::with_value_calls)
                        .any(|operation| release_matches(operation, result, loans))
                {
                    continue;
                }
            }
            let Some((result, _)) = operation.discardable_structural_result() else {
                continue;
            };
            if result.multiplicity != Multiplicity::Affine
                || !order.precedes(result.statement_index as usize, statement_index as usize)
            {
                continue;
            }
            let Some(StatementNode::LocalData(local)) =
                crate::flow::state_statements(program, authored_state.statement_nodes)
                    .get(result.statement_index as usize)
            else {
                continue;
            };
            if !results.iter().any(|(binding, root)| {
                binding == result && *root == PlaceRoot::Symbol(local.symbol)
            }) {
                continue;
            }
            if occupant(program, authored_state, operations, local.symbol)
                != Some(result.binding_ordinal)
            {
                continue;
            }
            if borrowing.is_some_and(|borrowing| {
                facts
                    .borrow
                    .loan_group(borrowing.loans)
                    .is_some_and(|mut loans| {
                        loans.any(|(_, loan)| {
                            loan.owner_symbol == local.symbol
                                && loan.statement_index != result.statement_index as usize
                                && order.precedes(loan.statement_index, statement_index as usize)
                        })
                    })
            }) {
                continue;
            }
            if borrowing
                .and_then(|borrowing| {
                    facts.borrow.storage_loans(
                        borrowing,
                        PlaceRoot::Symbol(local.symbol),
                        result.statement_index as usize,
                    )
                })
                .is_some_and(|loans| loans.is_empty())
            {
                continue;
            }
            if !has_contents(
                program,
                local.type_reference,
                &[],
                ContentsRequirement::DescriptorCleanup,
            ) {
                continue;
            }
            trace.phase("reference retirement: local custody transfer");
            let transferred_paths = super::reference_retirement::transferred_paths(
                program,
                facts,
                prepared_calls,
                results,
                result.binding_ordinal,
                operations,
            )?;
            if transferred_paths.iter().any(|path| path.is_empty()) {
                continue;
            }
            let Some(boundary) = whole_local_release_boundary(
                program,
                facts,
                machine,
                authored_state,
                result.statement_index,
            ) else {
                continue;
            };
            let retains_siblings = order.precedes(statement_index as usize, boundary as usize)
                || !transferred_paths.is_empty();
            let retirement = Retirement {
                pending: &pending,
                ..statement_retirement
            };
            trace.phase("reference retirement: local loan leaves");
            let local_loans = local_record_loans(
                program,
                facts,
                machine,
                authored_state,
                result.statement_index,
            )?;
            if local_loans.is_empty() {
                continue;
            }
            if !retains_siblings
                && local_loans.iter().any(|(_, group)| {
                    !facts.borrow.loan_group(*group).is_some_and(|mut loans| {
                        loans.all(|(loan, _)| retirement.children_retired(loan))
                    })
                })
            {
                continue;
            }
            let leaf_releases = if retains_siblings {
                local_loans
                    .into_iter()
                    .rev()
                    .filter(|(source, group)| {
                        facts.borrow.loan_group(*group).is_some_and(|mut loans| {
                            loans.all(|(loan, _)| retirement.children_retired(loan))
                        }) && !transferred_paths.iter().any(|path| {
                            path.starts_with(&source.path) || source.path.starts_with(path)
                        }) && latest_loan_release_boundary(&order, facts, *group, |loan| {
                            validation::reference_result_custody::release_statement(
                                program, facts, machine, state, loan,
                            )
                        })
                        .is_some_and(|boundary| {
                            order.at_or_before(boundary as usize, statement_index as usize)
                        }) && !operations
                            .iter()
                            .chain(&pending)
                            .flat_map(OperationPlan::with_value_calls)
                            .any(|operation| {
                                matches!(operation, OperationPlan::ReleaseReference {
                                loans: released, ..
                            } if *released == *group)
                            })
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            if retains_siblings && leaf_releases.is_empty() {
                continue;
            }
            trace.phase("reference retirement: local storage loans");
            let storage_loans = local_storage_loans(facts, machine, state, local.symbol)?;
            if storage_loans
                .into_iter()
                .any(|handle| !retirement.holder_retired(handle))
            {
                continue;
            }
            if retains_siblings {
                for (source, group) in leaf_releases {
                    pending.push(OperationPlan::ReleaseReference {
                        statement_index,
                        source: SourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                        path: source.path,
                        select_evaluated_storage: false,
                        defer_to_exit: false,
                        loans: group,
                        selection_complements: Vec::new(),
                        owner_cleanups: HandleSpan::empty(),
                        case_conditions: Default::default(),
                    });
                }
                continue;
            }
            retired.push(result.binding_ordinal);
            pending.push(OperationPlan::ContinuationCleanup {
                partitions: Default::default(),
                selected_regions: Vec::new(),
                boundary: CheckedUnitCleanupBoundary::StatementEntry { statement_index },
                cleanup_actions: vec![CheckedNormalCleanupAction::Discard(
                    CheckedUnitPartialAffineDiscardPlan {
partition_leaf: Default::default(),
                        source:
                            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal: result.binding_ordinal,
                            },
                        path: Vec::new(),
                        type_identity: result.type_identity,
                        case_conditions: Default::default(),
                    },
                )],
            });
        }
        loop {
            trace.phase("reference retirement: backing occurrences");
            let releases = Retirement {
                pending: &pending,
                ..statement_retirement
            }
            .backing_releases()?;
            if releases.is_empty() {
                break;
            }
            pending.extend(releases);
        }
        for binding in retired {
            trace.phase("reference retirement: producer disposal");
            let (_, discard_on_return) = operations.iter_mut().find_map(|operation| {
                operation.discardable_structural_result_by_ordinal_mut(binding)
            })?;
            *discard_on_return = false;
        }
        trace.phase("reference retirement: case conditions");
        prepare_owner_cleanups(
            owner_candidates,
            program,
            facts,
            prepared_calls,
            shapes,
            machine,
            authored_state,
            statement_index,
            None,
            None,
            false,
            trace,
            results,
            operations,
            operations.len(),
            &mut pending,
        )?;
        prepare_case_conditions(
            program,
            facts,
            shapes,
            machine,
            state,
            parameters,
            results,
            &mut pending,
        )?;
        let insertion = operations
            .iter()
            .rposition(|operation| {
                !matches!(operation,
            OperationPlan::ContinuationCleanup {
                boundary: CheckedUnitCleanupBoundary::Call(coordinate), ..
            } if order.precedes(coordinate.statement_index as usize, statement_index as usize))
            })
            .map_or(0, |position| position + 1);
        operations.splice(insertion..insertion, pending);
        Some(())
    }

    /*
        Loans released by the state's natural exit carry their weakening at a
        boundary the per-statement loop never visits (the transition statement, or
        one past the last ordinary statement). The completion still needs the
        `ReleaseReference` op so the result-binding cleanup sees the disposal.
    */
    pub(in crate::execution::terminal_unit) fn append_exit_reference_releases(
        owner_candidates: &OwnerCandidates,
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
        shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
        parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
        machine: SymbolHandle,
        state: SymbolHandle,
        returned: Option<&CheckedUnitStructuralReturnPlan>,
        completion: crate::execution::terminal_unit::storage_transfers::CompletionPlan<'_>,
        results: &[(BindingPlan, PlaceRoot)],
        operations: &mut Vec<OperationPlan>,
        trace: &super::super::LocalConstructionTrace,
    ) -> Option<()> {
        let authored_state = program.state_by_symbol(state)?;
        let borrowing = unique_borrow_state(facts, machine, state)?;
        if borrowing.is_none_or(|borrowing| borrowing.loans.is_empty())
            && !operations
                .iter()
                .chain(prepared_calls.operations_for(machine, state))
                .flat_map(OperationPlan::with_value_calls)
                .any(|operation| {
                    captured_reference_result(
                        program, facts, machine, state, results, operation, operations,
                    )
                    .is_some()
                })
        {
            return Some(());
        }
        let prepared_completion = match completion.prepare(program, facts, machine, authored_state)
        {
            Ok(prepared) => prepared,
            Err(reason) => {
                trace.phase(reason);
                return None;
            }
        };
        if !prepared_completion.has_normal_completion() {
            return Some(());
        }
        let completion = Some(&prepared_completion);
        let order = program
            .statement_table
            .admitted_execution_order(authored_state.statement_nodes);
        let mut pending = Vec::new();
        trace.phase("statement sequence: completion carrier inventory");
        for operation in operations.iter().flat_map(OperationPlan::with_value_calls) {
            if let Some(captured) = captured_reference_result(
                program, facts, machine, state, results, operation, operations,
            ) {
                let result = captured.result;
                let loans = captured.loans;
                if ::validation::trace_flags::check_sites() {
                    eprintln!(
                        "CHECKSITE exit-reference-inventory state={state:?} result={result:?} loans={loans:?} whole_boundary={} storage_loans={:?}",
                        captured.whole_storage_boundary, captured.storage_loans
                    );
                }
                trace.statement(Some(result.statement_index));
                if operations
                    .iter()
                    .flat_map(OperationPlan::with_value_calls)
                    .any(|operation| whole_result_discarded(operation, result))
                {
                    if ::validation::trace_flags::check_sites() {
                        eprintln!(
                            "CHECKSITE exit-reference-skipped state={state:?} result={} reason=discarded",
                            result.binding_ordinal
                        );
                    }
                    continue;
                }
                if replacement_owns_release(&order, facts, machine, state, loans) {
                    if ::validation::trace_flags::check_sites() {
                        eprintln!(
                            "CHECKSITE exit-reference-skipped state={state:?} result={} reason=replacement",
                            result.binding_ordinal
                        );
                    }
                    continue;
                }
                if custody_transferred(
                    program,
                    facts,
                    prepared_calls,
                    results,
                    result.binding_ordinal,
                    operations,
                )? {
                    if ::validation::trace_flags::check_sites() {
                        eprintln!(
                            "CHECKSITE exit-reference-skipped state={state:?} result={} reason=transferred",
                            result.binding_ordinal
                        );
                    }
                    continue;
                }
                if returned
                    .is_some_and(|returned| returned_reference_binding(&order, returned, result))
                {
                    trace.phase("statement sequence: returned carrier loan boundary");
                    trace.statement(Some(result.statement_index));
                    let authored = program.state_by_symbol(state)?;
                    let statement_count =
                        crate::flow::state_statements(program, authored.statement_nodes).len();
                    if !facts.borrow.loan_group(loans)?.all(|(loan, _)| {
                        release_boundary(program, facts, machine, state, loan)
                            .is_some_and(|boundary| boundary as usize == statement_count)
                    }) {
                        return None;
                    }
                    continue;
                }
                trace.phase("statement sequence: completion carrier release boundary");
                trace.statement(Some(result.statement_index));
                let boundary = latest_loan_release_boundary(&order, facts, loans, |loan| {
                    super::super::super::reference_results::release_statement(
                        program, facts, machine, state, loan,
                    )
                })?;
                let boundary = order.latest(
                    boundary as usize,
                    order.successor(result.statement_index as usize)?,
                )?;
                let boundary = u32::try_from(
                    order.latest(boundary, captured.whole_storage_boundary as usize)?,
                )
                .ok()?;
                if !operations
                    .iter()
                    .flat_map(OperationPlan::with_value_calls)
                    .any(|operation| release_matches(operation, result, loans))
                    && !pending
                        .iter()
                        .any(|(_, operation, _)| release_matches(operation, result, loans))
                {
                    pending.push((
                        boundary,
                        OperationPlan::ReleaseReference {
                            statement_index: boundary,
                            source: SourcePlan::StructuralResult {
                                binding_ordinal: result.binding_ordinal,
                            },
                            path: Vec::new(),
                            select_evaluated_storage: false,
                            defer_to_exit: true,
                            loans,
                            selection_complements: Vec::new(),
                            owner_cleanups: HandleSpan::empty(),
                            case_conditions: Default::default(),
                        },
                        captured.storage_loans,
                    ));
                }
            }
        }
        pending.sort_by_key(|(boundary, _, _)| order.position(*boundary as usize));
        let statement_index = authored_state.statement_nodes.count();
        trace.phase("statement sequence: completion carrier transport");
        let transported_loans = super::reference_retirement::transport::collect(
            program,
            facts,
            machine,
            authored_state,
            results,
            operations,
        )?;
        let statement_retirement = Retirement {
            program,
            trace,
            facts,
            prepared_calls,
            machine,
            state: authored_state,
            order: &order,
            statement_index,
            results,
            operations,
            completed_operations: operations.len(),
            pending: &[],
            at_exit: false,
            uncommitted_owner: PlaceRoot::Symbol(Default::default()),
            returned: None,
            completion,
            transported_loans: &transported_loans,
            storage_borrowers: &owner_candidates.storage_borrowers,
        };
        trace.phase("statement sequence: completion carrier retirement order");
        let mut scheduled = Vec::new();
        loop {
            let retirement = Retirement {
                pending: &scheduled,
                at_exit: true,
                returned,
                ..statement_retirement
            };
            trace.phase("statement sequence: completion reference complements");
            let complements = complement_completions(&retirement)?;
            if !complements.is_empty() {
                for (position, complement) in complements {
                    let OperationPlan::ReleaseReference {
                        selection_complements,
                        ..
                    } = scheduled.get_mut(position)?
                    else {
                        return None;
                    };
                    selection_complements.push(complement);
                }
                continue;
            }
            trace.phase("statement sequence: completion reference carrier dependencies");
            let mut next = None;
            for (index, (_, operation, storage_loans)) in pending.iter().enumerate() {
                let OperationPlan::ReleaseReference { loans, source, .. } = operation else {
                    return None;
                };
                let SourcePlan::StructuralResult { binding_ordinal } = source else {
                    return None;
                };
                let (result, _) = results
                    .iter()
                    .find(|(result, _)| result.binding_ordinal == *binding_ordinal)?;
                if !retirement.carrier_children_retired(result) {
                    if ::validation::trace_flags::check_sites() {
                        eprintln!(
                            "CHECKSITE exit-reference-pending state={state:?} result={} reason=carrier-children scheduled={} loans={loans:?}",
                            result.binding_ordinal,
                            scheduled.len()
                        );
                    }
                    continue;
                }
                let mut waiting_storage = None;
                if !storage_loans.iter().all(|loan| {
                    let retired = retirement.holder_retired(*loan);
                    if !retired {
                        waiting_storage = Some(*loan);
                    }
                    retired
                }) {
                    if ::validation::trace_flags::check_sites() {
                        eprintln!(
                            "CHECKSITE exit-reference-pending state={state:?} result={} reason=storage-holder scheduled={} storage_loans={storage_loans:?} waiting={waiting_storage:?} row={:?}",
                            result.binding_ordinal,
                            scheduled.len(),
                            waiting_storage.map(|loan| facts.borrow.loans.get(loan)),
                        );
                    }
                    continue;
                }
                if !facts
                    .borrow
                    .loan_group(*loans)?
                    .all(|(loan, _)| retirement.children_retired(loan))
                {
                    if ::validation::trace_flags::check_sites() {
                        eprintln!(
                            "CHECKSITE exit-reference-pending state={state:?} result={} reason=loan-children scheduled={} rows={:?}",
                            result.binding_ordinal,
                            scheduled.len(),
                            facts
                                .borrow
                                .loan_group(*loans)
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                        );
                    }
                    continue;
                }
                next = Some(index);
                break;
            }
            if let Some(index) = next {
                scheduled.push(pending.remove(index).1);
                continue;
            }
            trace.phase("statement sequence: completion reference backing dependencies");
            let mut releases = retirement.backing_releases()?;
            trace.phase("statement sequence: completion stored reference dependencies");
            for operation in operations
                .iter()
                .rev()
                .flat_map(OperationPlan::with_value_calls)
            {
                releases.extend(super::reference_retirement::stores::releases(
                    &retirement,
                    operation,
                )?);
            }
            if releases.is_empty() {
                if !pending.is_empty() {
                    if ::validation::trace_flags::check_sites() {
                        for (_, operation, storage_loans) in &pending {
                            let OperationPlan::ReleaseReference {
                                source: SourcePlan::StructuralResult { binding_ordinal },
                                loans,
                                ..
                            } = operation
                            else {
                                continue;
                            };
                            let Some((result, owner)) = results
                                .iter()
                                .find(|(result, _)| result.binding_ordinal == *binding_ordinal)
                            else {
                                continue;
                            };
                            eprintln!(
                                "CHECKSITE completion-retirement-blocked machine={machine:?} state={state:?} binding={binding_ordinal} owner={owner:?} carrier_children_retired={} storage_loans={storage_loans:?} descriptor_loans={loans:?}",
                                retirement.carrier_children_retired(result),
                            );
                        }
                        if let Some(Some(borrowing)) = unique_borrow_state(facts, machine, state)
                            && let Some(resources) =
                                facts.borrow.statement_resource_loans(borrowing)
                        {
                            for (handle, loan) in resources {
                                eprintln!(
                                    "CHECKSITE completion-retirement-holder loan={handle:?} retired={} occurrence={loan:?} weakening={:?}",
                                    retirement.holder_retired(handle),
                                    loan_weakening(facts, machine, state, handle),
                                );
                            }
                        }
                    }
                    return None;
                }
                break;
            }
            scheduled.extend(releases);
        }
        let insertion = operations
            .iter()
            .rposition(|operation| {
                !matches!(
                    operation,
                    OperationPlan::ContinuationCleanup {
                        boundary: CheckedUnitCleanupBoundary::Call(_),
                        ..
                    }
                )
            })
            .map_or(0, |position| position + 1);
        for index in 0..scheduled.len() {
            if let OperationPlan::ReleaseReference {
                source: SourcePlan::StructuralResult { binding_ordinal },
                path,
                ..
            } = &scheduled[index]
                && path.is_empty()
                && normal_call_disposes_backing(facts, state, results, operations, *binding_ordinal)
            {
                let captured = operations
                    .iter()
                    .flat_map(OperationPlan::with_value_calls)
                    .filter_map(|operation| {
                        captured_reference_result(
                            program, facts, machine, state, results, operation, operations,
                        )
                    })
                    .find(|captured| captured.result.binding_ordinal == *binding_ordinal)?;
                let retirement = Retirement {
                    completed_operations: insertion,
                    pending: &scheduled[..index],
                    ..statement_retirement
                };
                if retirement.carrier_children_retired(captured.result)
                    && captured
                        .storage_loans
                        .iter()
                        .all(|loan| retirement.holder_retired(*loan))
                    && facts
                        .borrow
                        .loan_group(captured.loans)?
                        .all(|(loan, _)| retirement.children_retired(loan))
                    && let OperationPlan::ReleaseReference { defer_to_exit, .. } =
                        &mut scheduled[index]
                {
                    *defer_to_exit = false;
                }
            }
        }
        for release in &scheduled {
            if let OperationPlan::ReleaseReference {
                source: SourcePlan::StructuralResult { binding_ordinal },
                defer_to_exit: false,
                ..
            } = release
                && let Some((_, discard)) = operations.iter_mut().find_map(|operation| {
                    operation.discardable_structural_result_by_ordinal_mut(*binding_ordinal)
                })
            {
                *discard = false;
            }
        }
        prepare_owner_cleanups(
            owner_candidates,
            program,
            facts,
            prepared_calls,
            shapes,
            machine,
            authored_state,
            statement_index,
            returned,
            completion,
            true,
            trace,
            results,
            operations,
            insertion,
            &mut scheduled,
        )?;
        trace.phase("statement sequence: completion carrier case conditions");
        prepare_case_conditions(
            program,
            facts,
            shapes,
            machine,
            state,
            parameters,
            results,
            &mut scheduled,
        )?;
        operations.splice(insertion..insertion, scheduled);
        Some(())
    }

    /*
        A selected continuation owns both the chosen result and the conditional
        complement until the actual death edge. Original producers must not also
        publish unconditional return drops for those same owners.
    */
    pub(in crate::execution::terminal_unit) fn retain_selected_sources(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &State,
        statement: u32,
        operations: &mut [OperationPlan],
    ) -> Option<()> {
        let Some((_, receipt)) = facts
            .flow
            .ownership
            .owned_selection_at(state.symbol, statement)
        else {
            return Some(());
        };
        for source in facts
            .flow
            .ownership
            .selection_sources
            .span(receipt.sources)?
        {
            if program
                .state_parameter_by_symbol(state, source.symbol)
                .is_some()
            {
                continue;
            }
            let storage = super::super::super::local_storage::source(
                program,
                state,
                operations,
                source.symbol,
            )?;
            let SourcePlan::StructuralResult { binding_ordinal } = storage else {
                if matches!(storage, SourcePlan::StructuralLocal { .. }) {
                    continue;
                }
                return None;
            };
            let mut producers = operations
                .iter()
                .enumerate()
                .filter_map(|(index, operation)| {
                    operation
                        .statement_structural_result_binding()
                        .filter(|result| result.binding_ordinal == binding_ordinal)
                        .map(|result| (index, result))
                });
            let (producer, result) = producers.next()?;
            if producers.next().is_some()
                || !matches!(
                    result.multiplicity,
                    Multiplicity::Affine | Multiplicity::Linear
                )
                || result.statement_index != source.statement_ordinal
            {
                return None;
            }
            let linear = result.multiplicity == Multiplicity::Linear;
            if linear && source.retains_storage {
                return None;
            }
            if let Some((_, discard)) =
                operations[producer].discardable_structural_result_by_ordinal_mut(binding_ordinal)
            {
                if linear && *discard {
                    return None;
                }
                *discard = false;
            }
        }
        Some(())
    }

    /*
        Discharge only producers owned by this sequence. Call-owned operand
        preparation has its own result namespace segment; its enclosing sequence
        owns earlier locals. Each owner updates its own disposal debt, while source
        custody independently rejoins every operand across that boundary.
    */
    pub(in crate::execution::terminal_unit) fn consume_retained_results(
        operations: &mut [OperationPlan],
        consumer: &OperationPlan,
    ) -> Option<()> {
        let Some(call) = consumer.call() else {
            return Some(());
        };
        for argument in &call.structural_arguments {
            let Some(binding) = argument.source_structural_result_binding_ordinal() else {
                continue;
            };
            if operations.iter().any(|operation| {
                operation
                    .structural_result_binding()
                    .is_some_and(|result| result.binding_ordinal == binding)
            }) {
                consume_result(
                    operations,
                    binding,
                    argument.access,
                    !argument.path.is_empty(),
                )?;
            }
        }
        Some(())
    }

    pub(in crate::execution::terminal_unit) fn consume_value_call_sources(
        facts: &CheckFacts,
        results: &mut Vec<(BindingPlan, PlaceRoot)>,
        operations: &mut [OperationPlan],
        calls: &[CheckedStructuralValueCall],
        trace: &LocalConstructionTrace,
    ) -> Option<()> {
        for call in calls {
            for operand in call
                .operands()
                .iter()
                .chain(call.operation())
                .flat_map(OperationPlan::with_value_calls)
            {
                let Some(result) = operand.structural_result_binding() else {
                    continue;
                };
                trace.phase("call sources: producer occurrence");
                trace.statement(Some(result.statement_index));
                let expression = if let Some(producer) = operand.call() {
                    let Some(NominalMachineUseSite::Expression(expression)) =
                        producer.source_site()
                    else {
                        if ::validation::trace_flags::trace_completion() {
                            eprintln!(
                                "CHECKSITE completion-call-source-kind trace={trace:?} producer={producer:?}"
                            );
                        }
                        return None;
                    };
                    expression
                } else {
                    trace.phase("call sources: non-call producer");
                    trace.statement(Some(result.statement_index));
                    match operand {
                        OperationPlan::EstablishLocal(
                            CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
                        ) => facts.values.structural_values.nodes.get(*value).expression,
                        OperationPlan::EstablishReference {
                            formation_expression,
                            ..
                        }
                        | OperationPlan::MoveStructuralField {
                            expression: formation_expression,
                            ..
                        } => *formation_expression,
                        _ => {
                            if ::validation::trace_flags::trace_completion() {
                                eprintln!(
                                    "CHECKSITE completion-producer-kind trace={trace:?} producer={operand:?}"
                                );
                            }
                            return None;
                        }
                    }
                };
                let root = PlaceRoot::Expression(expression);
                trace.phase("call sources: binding agreement");
                trace.statement(Some(result.statement_index));
                if let Some((existing, existing_root)) = results
                    .iter()
                    .find(|(existing, _)| existing.binding_ordinal == result.binding_ordinal)
                {
                    if existing != result || *existing_root != root {
                        if ::validation::trace_flags::trace_completion() {
                            eprintln!(
                                "CHECKSITE completion-binding-conflict trace={trace:?} existing={existing:?} existing_root={existing_root:?} result={result:?} root={root:?} producer={operand:?}"
                            );
                        }
                        return None;
                    }
                    continue;
                }
                results.push((result.clone(), root));
            }
            for operation in call
                .operands()
                .iter()
                .chain(call.operation())
                .flat_map(OperationPlan::with_value_calls)
            {
                if let OperationPlan::EstablishLocal(
                    CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
                ) = operation
                {
                    trace.phase("call sources: stored value transfer");
                    consume_value_places(facts, *value, results, operations)?;
                }
                trace.phase("call sources: retained result transfer");
                consume_retained_results(operations, operation)?;
            }
        }
        trace.phase("call sources: finished");
        Some(())
    }

    pub(in crate::execution::terminal_unit) fn consume_results(
        operations: &mut [OperationPlan],
        consumer: &OperationPlan,
    ) -> Option<()> {
        {
            for (binding_ordinal, access, projected) in consumer
                .structural_operands()
                .iter()
                .filter_map(|argument| {
                    argument
                        .source_structural_result_binding_ordinal()
                        .map(|ordinal| (ordinal, argument.access, !argument.path.is_empty()))
                })
            {
                consume_result(operations, binding_ordinal, access, projected)?;
            }
        }
        if let Some(value) = consumer.installed_value()
            && let Some(binding_ordinal) = value.source_structural_result_binding_ordinal()
        {
            consume_result(
                operations,
                binding_ordinal,
                value.access,
                !value.path.is_empty(),
            )?;
        }
        Some(())
    }

    pub(in crate::execution::terminal_unit) fn consume_result(
        operations: &mut [OperationPlan],
        binding_ordinal: u32,
        access: StructuralAccess,
        projected: bool,
    ) -> Option<()> {
        let mut producers = operations.iter_mut().filter(|operation| {
            operation
                .structural_result_binding()
                .is_some_and(|result| result.binding_ordinal == binding_ordinal)
        });
        let producer = producers.next()?;
        if producers.next().is_some() {
            return None;
        }
        if matches!(producer, OperationPlan::MoveStructuralField { .. }) {
            return Some(());
        }
        if producer
            .structural_result_binding()
            .is_some_and(|result| result.multiplicity == Multiplicity::Unrestricted)
        {
            return Some(());
        }
        let reference_carrier = matches!(producer, OperationPlan::EstablishReference { .. })
            || producer
                .call()
                .and_then(CheckedUnitCallPlan::structural_custody)
                .is_some_and(|custody| !custody.reference_loans.is_empty());
        if reference_carrier {
            return match access {
                StructuralAccess::Owned => !projected,
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow => true,
            }
            .then_some(());
        }
        if matches!(
            access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ) {
            return Some(());
        }
        let (result, discard_result_on_return) = producer.discardable_structural_result_mut()?;
        if result.multiplicity != Multiplicity::Affine {
            if result.multiplicity == Multiplicity::Linear
                && !*discard_result_on_return
                && access == StructuralAccess::Owned
                && !projected
            {
                return Some(());
            }
            return None;
        }
        if !projected {
            *discard_result_on_return = false;
        }
        Some(())
    }

    /*
        Sequence one structural value's operand custody into the operation stream:
        retain the value's call plans, settle the sources those calls evaluate
        under the statement's owned-selection policy, and adopt the exact anonymous
        producers into the receiving results. Returns the plans the value's
        establishment consumes.

        A selected continuation owns both the chosen result and the conditional
        complement until the actual death edge, so a value rooted in an owned
        selection retains its sources instead of consuming them unconditionally.
        That policy belongs to this sequence, not to each producer site.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::execution::terminal_unit) fn prepare_value_operands(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        scalar_callees: ScalarCalleeClosure,
        shapes: &mut ShapeCollector<'_>,
        machine: &Machine,
        state: &State,
        structural_parameters: &[ParameterPlan],
        trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
        entry_claims: &[CheckedUnitEntryClaimPlan],
        statement_ordinal: u32,
        results: &mut Vec<(BindingPlan, PlaceRoot)>,
        operations: &mut [OperationPlan],
        structural_count: &mut usize,
        root: CheckedStructuralValueHandle,
        trace: &LocalConstructionTrace,
        windows: &mut OpenWindows,
    ) -> Option<(Vec<CheckedStructuralValueCall>, CheckedValueCompletion)> {
        let (calls, completion) = value_calls(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            entry_claims,
            results,
            operations,
            structural_count,
            root,
            trace,
            windows,
        )?;
        if completion == CheckedValueCompletion::NoNormal {
            trace.phase("value operands: nonreturning call sources");
            trace.statement(Some(statement_ordinal));
            consume_value_call_sources(facts, results, operations, &calls, trace)?;
            return Some((calls, completion));
        }
        if facts
            .flow
            .ownership
            .owned_selection_at(state.symbol, statement_ordinal)
            .is_some()
        {
            trace.phase("value operands: selected sources");
            trace.statement(Some(statement_ordinal));
            retain_selected_sources(program, facts, state, statement_ordinal, operations)?;
        } else {
            trace.phase("value operands: stored sources");
            trace.statement(Some(statement_ordinal));
            consume_value_places(facts, root, results, operations)?;
        }
        trace.phase("value operands: completed call sources");
        trace.statement(Some(statement_ordinal));
        consume_value_call_sources(facts, results, operations, &calls, trace)?;
        trace.phase("value operands: finished");
        trace.statement(Some(statement_ordinal));
        Some((calls, completion))
    }

    pub(in crate::execution::terminal_unit) fn consume_value_places(
        facts: &CheckFacts,
        root: CheckedStructuralValueHandle,
        results: &[(BindingPlan, PlaceRoot)],
        operations: &mut [OperationPlan],
    ) -> Option<()> {
        let plans = &facts.values.structural_values;
        let mut pending = vec![(root, false)];
        let mut visited = Vec::new();
        while let Some((value, projected)) = pending.pop() {
            if !plans.nodes.is_valid(value) || visited.contains(&value) {
                return None;
            }
            visited.push(value);
            match &plans.nodes.get(value).kind {
                CheckedStructuralValueKind::NativeCodeLayout { .. } => {}
                CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                    pending.extend(arguments.iter().rev().map(|value| (*value, false)));
                }
                CheckedStructuralValueKind::Place(argument) => {
                    if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } = argument.source
                    {
                        let mut matching = results
                            .iter()
                            .filter(|(_, source)| *source == PlaceRoot::Symbol(symbol));
                        let (result, _) = matching.next()?;
                        if matching.next().is_some() || result.type_identity != argument.type_identity {
                            return None;
                        }
                        consume_result(
                            operations,
                            result.binding_ordinal,
                            argument.access,
                            projected || !argument.path.is_empty(),
                        )?;
                    }
                }
                CheckedStructuralValueKind::Record { fields, .. }
                | CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                    for field in plans.record_fields.span(*fields)? {
                        if let CheckedStructuralRecordFieldValue::Structural(value) = field.value {
                            pending.push((value, false));
                        }
                    }
                }
                CheckedStructuralValueKind::Dispatch { arms, .. } => {
                    pending.extend(
                        plans
                            .dispatch_arms
                            .span(*arms)?
                            .iter()
                            .map(|arm| (arm.value, projected)),
                    );
                }
                CheckedStructuralValueKind::Projection {
                    source,
                    path,
                    copies_selected_value,
                    ..
                } => {
                    pending.push((
                        *source,
                        projected || !path.is_empty() || *copies_selected_value,
                    ));
                }
                CheckedStructuralValueKind::ViewSubslice { source, .. } => {
                    pending.push((*source, true));
                }
                CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                    pending.push((*source, projected));
                }
                CheckedStructuralValueKind::FixedArray { elements } => {
                    pending.extend(elements.iter().map(|value| (*value, false)));
                }
                CheckedStructuralValueKind::Reference { source_value, .. } => {
                    if source_value.is_valid() {
                        pending.push((*source_value, false));
                    }
                }
                CheckedStructuralValueKind::BorrowedSliceView { .. }
                | CheckedStructuralValueKind::MutableSliceView { .. }
                | CheckedStructuralValueKind::Call { .. }
                | CheckedStructuralValueKind::CopiedStructuralPlace { .. }
                | CheckedStructuralValueKind::ZeroedScalarArray { .. }
                | CheckedStructuralValueKind::ScalarArray { .. }
                | CheckedStructuralValueKind::Scalar { .. }
                | CheckedStructuralValueKind::ZeroedStructural { .. }
                | CheckedStructuralValueKind::ByteLiteralView { .. }
                | CheckedStructuralValueKind::Case(_) => {}
            }
        }
        Some(())
    }
}
mod statement_call {
    /*
        Planning the call a single-state body statement performs.

        After a statement's own producer binds (or declines to bind) the call's
        result, this matches the statement to its one flow call, establishes the
        call's structural operands, including extraction from borrowed storage,
        builds the call operation, and binds its result: a
        discarded structural result is disposed on the call's continuation, a
        bound one joins the body's structural results, a scalar one binds in the
        dense namespace. Normal operand cleanup follows result capture before an
        enclosing result store can displace its destination. Select structural
        replacement only after operand preparation updates the open-window ledger;
        selecting it earlier would dispose an actual that the call already consumed.
    */
    use super::{
        CheckedUnitEffectOperationPlan, CheckedUnitScalarResultBindingPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan,
        Construction, ExpectedCallValueResult, Multiplicity, StatementNode, SymbolHandle,
        bind_scalar_call_result, bind_structural_call_result, build_call_operation,
        consume_results, structural_operands,
    };
    use checked_trees::CheckedUnitPartialAffineDiscardPlan;

    /* The sequence state a statement's call reads and extends. */
    pub(super) struct Planner<'a, 'program, 'shapes> {
        pub(super) construction: Construction<'a, 'program, 'shapes>,
        pub(super) scalar_parameters: &'a [super::CheckedStructuralScalarParameterPlan],
        pub(super) parameter_storage: &'a [checked_trees::CheckedUnitParameterStoragePlan],
        pub(super) calls: &'a [&'a checked_trees::FlowCallFact],
        pub(super) operations: &'a mut Vec<CheckedUnitEffectOperationPlan>,
        pub(super) structural_count: &'a mut usize,
        pub(super) call_count: &'a mut usize,
        pub(super) windows: &'a mut super::super::super::borrowed_windows::OpenWindows,
        pub(super) structural_local_symbols: &'a mut Vec<SymbolHandle>,
        pub(super) array_bindings:
            &'a mut Vec<(SymbolHandle, CheckedUnitStructuralResultBindingPlan)>,
        pub(super) returned_call: &'a mut Option<CheckedUnitStructuralResultBindingPlan>,
        pub(super) structural_results:
            &'a mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
    }

    /* What the statement's own producer left for its call. */
    pub(super) struct StatementCall {
        /* The statement is the machine's value-returning completion. */
        pub(super) completes_machine: bool,
        /* The scalar result binding, when the call's result is scalar. */
        pub(super) result: Option<CheckedUnitScalarResultBindingPlan>,
        /* The structural result binding; it names the local it initializes. */
        pub(super) structural_result: Option<CheckedUnitStructuralResultBindingPlan>,
        /* Stores consuming the call's result, run as its continuation. */
        pub(super) result_stores: Vec<CheckedUnitEffectOperationPlan>,
    }

    pub(super) fn plan(
        planner: Planner<'_, '_, '_>,
        statement: &typed_trees::statement::StatementNode,
        index: usize,
        statement_index: u32,
        statement_call: StatementCall,
    ) -> Option<()> {
        let Planner {
            construction:
                Construction {
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    machine,
                    state,
                    structural_parameters,
                    entry_claims,
                    trivial_affine_locals,
                    trace,
                },
            scalar_parameters,
            parameter_storage,
            calls,
            operations,
            structural_count,
            call_count,
            windows,
            structural_local_symbols,
            array_bindings,
            returned_call,
            structural_results,
        } = planner;
        let StatementCall {
            completes_machine,
            result,
            mut structural_result,
            result_stores: call_result_stores,
        } = statement_call;
        let call_phase = |phase: &'static str| {
            trace.phase(phase);
            trace.statement(Some(statement_index));
        };
        call_phase("statement sequence: call: flow call");
        let mut matching = calls
            .iter()
            .copied()
            .filter(|call| call.statement_index == index);
        let Some(call) = matching.next() else {
            call_phase("statement sequence: call: missing exact flow occurrence");
            return None;
        };
        if matching.next().is_some() {
            call_phase("statement sequence: call: competing flow occurrences");
            return None;
        }
        if matches!(statement, StatementNode::Transition(_))
            && !super::super::call_occurrences::is_terminal_call_occurrence(
                program, machine, state, call,
            )
        {
            call_phase("statement sequence: call: transition is not its completing occurrence");
            return None;
        }
        let authored_expression = match statement {
            StatementNode::LocalData(local) => Some(local.initial_value),
            StatementNode::Expression(expression) => Some(*expression),
            StatementNode::Assignment(assignment) => Some(assignment.value),
            _ => None,
        };
        if let Some(expression) = authored_expression {
            let authored = program.expression_table.call_node(expression)?;
            if call.authored_expression != expression
                || call.target_symbol
                    != validation::call_target_entry_symbol(
                        program,
                        validation::call_target_symbol(program, authored),
                    )
            {
                return None;
            }
        }
        *call_count = call_count.checked_add(1)?;
        call_phase("statement sequence: call: structural operands");
        structural_operands::plan_call_operands(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            entry_claims,
            structural_results,
            operations,
            structural_count,
            call,
            trace,
            windows,
        )?;
        if operations
            .last()
            .is_some_and(|operation| !operation.has_normal_continuation())
        {
            return Some(());
        }
        if let Some(contract) = crate::checks::atomic_fences::contract(program, call) {
            call_phase("statement sequence: call: atomic fence ordering");
            if result.is_some() || structural_result.is_some() || !call_result_stores.is_empty() {
                return None;
            }
            let flow_state = facts
                .flow
                .control
                .state_by_symbol(state.symbol)
                .filter(|candidate| candidate.machine_symbol == machine.symbol)?;
            let ordering =
                crate::checks::atomic_fences::ordering(program, facts, flow_state, call, &contract)
                    .ok()?;
            let declaration = program.operator_definition_by_symbol(contract.declaration)?;
            let site = validation::find_call_site(
                program,
                machine.symbol,
                state.symbol,
                index,
                call.call_ordinal,
            )?;
            let arguments = crate::execution::terminal_unit::calls::structural_call_arguments(
                program,
                shapes,
                facts,
                call,
                machine,
                state,
                structural_parameters,
                trivial_affine_locals,
                crate::execution::terminal_unit::calls::CallTarget {
                    parameters: program.operator_parameters(declaration),
                    return_type: declaration.return_type,
                    supply_mode: language_semantics::MachineSupplyMode::CheckedBody,
                    declaration: None,
                },
                &site,
                index,
                structural_results,
                operations,
                trace,
            )?;
            let [argument]: [_; 1] = arguments.try_into().ok()?;
            let operation = CheckedUnitEffectOperationPlan::AtomicFence {
                statement_index,
                declaration: contract.declaration,
                ordering,
                argument,
            };
            consume_results(operations, &operation)?;
            operations.push(operation);
            return Some(());
        }
        let mut structural_store_bindings = 0;
        let mut structural_replacement = None;
        if let Some(result) = &mut structural_result {
            result.binding_ordinal = u32::try_from(*structural_count).ok()?;
            if let StatementNode::Assignment(assignment) = statement {
                call_phase("statement sequence: call: replacement destination");
                let replacement = super::assignments::call_result_replacement(
                    program,
                    facts,
                    shapes,
                    machine,
                    state,
                    structural_parameters,
                    scalar_parameters,
                    parameter_storage,
                    windows,
                    assignment,
                    result,
                    trace,
                )?;
                structural_store_bindings = replacement.displaced_bindings();
                structural_replacement = Some(replacement);
            }
        }
        if let Some(authored) = statement.as_call()
            && validation::compiler_operations::statement_wire_codec_intrinsic(program, authored)
                .is_some()
        {
            call_phase("statement sequence: call: derived wire codec");
            let store = super::wire_codec::written_store(
                program,
                machine,
                state,
                statement_index,
                authored,
            )?;
            operations.push(store);
            return Some(());
        }
        call_phase("statement sequence: call: call operation");
        let mut operation = build_call_operation(
            program,
            shapes,
            facts,
            Some(scalar_callees),
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            call,
            result
                .as_ref()
                .map(|result| ExpectedCallValueResult::Scalar(result.primitive_type))
                .or_else(|| {
                    structural_result
                        .as_ref()
                        .map(ExpectedCallValueResult::Structural)
                }),
            structural_results,
            operations,
            trace,
        )?;
        let coordinate = operation.invocation_coordinate()?;
        if operation
            .call()
            .is_some_and(|call| *call.result() == checked_trees::CheckedUnitCallResult::NoNormal)
        {
            consume_results(operations, &operation)?;
            operations.push(operation);
            return Some(());
        }
        call_phase("statement sequence: call: result binding");
        if let Some(result) = &structural_result
            && !result.local.is_named()
            && !completes_machine
            && call_result_stores.is_empty()
            && structural_replacement.is_none()
        {
            operation = bind_structural_call_result(
                program,
                facts,
                machine.symbol,
                state.symbol,
                operation,
                result.clone(),
            )?;
            let reference_result = super::captured_reference_result(
                program,
                facts,
                machine.symbol,
                state.symbol,
                structural_results,
                &operation,
                operations,
            )
            .is_some();
            let call = operation.call_mut()?;
            let coordinate = call.coordinate;
            let (_, discard_result_on_return) = call.structural_result_mut()?;
            *discard_result_on_return = false;
            consume_results(&mut *operations, &operation)?;
            operations.push(operation);
            if result.multiplicity == Multiplicity::Affine && !reference_result {
                operations.push(CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    partitions: Default::default(),
                    selected_regions: Vec::new(),
                    boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
                    cleanup_actions: vec![checked_trees::CheckedNormalCleanupAction::Discard(
                        CheckedUnitPartialAffineDiscardPlan {
                            partition_leaf: Default::default(),
                            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal: result.binding_ordinal,
                            },
                            path: Vec::new(),
                            type_identity: result.type_identity,
                            case_conditions: Default::default(),
                        },
                    )],
                });
            }
            super::super::super::cleanup::anonymous::insert_continuation(
                program,
                facts,
                shapes,
                machine,
                state,
                coordinate,
                operations.len(),
                structural_results,
                operations,
                trace,
            )?;
            *structural_count = structural_count.checked_add(1)?;
            return Some(());
        }
        if let Some(result) = structural_result {
            call_phase("statement sequence: call: structural result binding");
            operation = bind_structural_call_result(
                program,
                facts,
                machine.symbol,
                state.symbol,
                operation,
                result.clone(),
            )?;
            if completes_machine {
                *returned_call = Some(result.clone());
            }
            if result.local.is_named() {
                let symbol = result.local.symbol;
                structural_local_symbols.push(symbol);
                if matches!(statement, StatementNode::LocalData(local)
                    if validation::is_closed_primitive_array_type(program, local.type_reference))
                {
                    array_bindings.push((symbol, result.clone()));
                }
                if operation
                    .call()
                    .is_some_and(|call| call.structural_result().is_some())
                {
                    structural_results.push((result, facts::PlaceRoot::Symbol(symbol)));
                }
            }
            *structural_count = structural_count.checked_add(1)?;
        }
        call_phase("statement sequence: call: argument consumption");
        consume_results(&mut *operations, &operation)?;
        call_phase("statement sequence: call: scalar result binding");
        operations.push(match result {
            Some(result) => bind_scalar_call_result(facts, operation, result, true)?,
            None => operation,
        });
        let completion_position = operations.len();
        for store in call_result_stores {
            consume_results(&mut *operations, &store)?;
            operations.push(store);
        }
        if let Some(replacement) = structural_replacement {
            replacement.consume_source(&mut *operations)?;
            operations.extend(replacement.into_operations());
        }
        *structural_count = structural_count.checked_add(structural_store_bindings)?;
        call_phase("statement sequence: call: normal completion");
        super::super::super::cleanup::anonymous::insert_continuation(
            program,
            facts,
            shapes,
            machine,
            state,
            coordinate,
            completion_position,
            structural_results,
            operations,
            trace,
        )?;
        Some(())
    }
}
mod storage_initializer {
    /*
        The value that initializes one mutable primitive local's storage.

        Checking retains a local's `StorageInitializer` value in exactly one of two
        forms: a bound pure expression (`let mut count: u32 = 0;`), or a scalar
        computation rooted at that coordinate when the initializer evaluates a call
        or an ordered selection (`let mut stored: i32 = echo(a);`, a policy-erased
        conversion). Both establish the same initialized storage, and later reads,
        assignments and borrows name the local's symbol, never this value. A
        computation that exists but belongs to another machine, authored expression
        or carrier is a custody mismatch, not permission to fall back to the pure
        form.
    */
    use super::{
        CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, LocalConstructionTrace,
        SymbolHandle, checked_boolean_contains_short_circuit,
    };
    use crate::execution::terminal_unit::structural_scalar_store::scalar_custody_is_exact;
    use crate::values::scalar_expression_type;
    use checked_trees::CheckedCallScalarArgument;
    use typed_trees::types::PrimitiveType;

    pub(super) fn value(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
        statement_index: u32,
        local: &typed_trees::statement::TableLocalData,
        primitive_type: PrimitiveType,
        trace: &LocalConstructionTrace,
    ) -> Option<CheckedCallScalarArgument> {
        let role = CheckedScalarExpressionRole::StorageInitializer;
        let computations = &facts.values.scalar_computations;
        let mut roots = computations.roots_for_statement_role(state.symbol, statement_index, role);
        if let Some(root) = roots.next() {
            trace.phase(
                "statement sequence: local data: mutable primitive local: computation initializer",
            );
            trace.statement(Some(statement_index));
            if roots.next().is_some()
                || !computations.is_authored_root(
                    root,
                    machine,
                    local.initial_value,
                    primitive_type,
                )
                || facts
                    .values
                    .scalar_expressions
                    .expression_at(state.symbol, statement_index, role)
                    .is_some()
            {
                return None;
            }
            return Some(CheckedCallScalarArgument::Computation(root.root));
        }
        trace.phase("statement sequence: local data: mutable primitive local: storage initializer");
        trace.statement(Some(statement_index));
        let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_index,
            role,
        )?;
        if binding.expression != local.initial_value
            || binding.destination != local.symbol
            || scalar_expression_type(value) != Some(primitive_type)
            || !scalar_custody_is_exact(program, facts, state, binding, value, primitive_type)
            || matches!(value, CheckedScalarExpression::Boolean(expression)
                if checked_boolean_contains_short_circuit(expression))
        {
            return None;
        }
        Some(CheckedCallScalarArgument::Pure(value.clone()))
    }
}
mod wire_codec {
    /*
        Cursor store for a derived `Schema::encode`.

        Numbered data no longer grows a synthesized machine. The checked product
        still owes the exclusive `written` update the old encoder performed: one
        era varint, then each field's tag varint and, for a byte slice, its length
        varint plus payload. The store lands on the one referent the selector
        proved. A local referent keeps a local root; a borrowed parameter is that
        parameter's own root, because lowering finds the formal by its authored
        position and has no local home for it. An open selector or a field this
        walker cannot size fails the plan.
    */
    use checked_trees::CheckedUnitEffectOperationPlan;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
    use typed_trees::wire::WireMember;

    pub(super) fn written_store(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: u32,
        call: &typed_trees::statement::TableCall,
    ) -> Option<CheckedUnitEffectOperationPlan> {
        if call.target.as_str() != "encode" {
            return None;
        }
        let arguments = program.statement_table.expression_handles(call.arguments);
        let [value, _out, written] = arguments else {
            return None;
        };
        let length = framed_length(program, state, call, *value)?;
        let (root, segments) = validation::compiler_operations::single_exclusive_reference_storage(
            program, machine, *written,
        )?;
        let path = store_path(program, &segments)?;
        let store_root = cursor_store_root(program, state, root)?;
        let literal = facts::u64_integer_literal(length, PrimitiveType::U64)?;
        Some(CheckedUnitEffectOperationPlan::new_store(
            statement_index,
            store_root,
            path,
            checked_trees::CheckedStoreLeaf::Primitive {
                primitive_type: PrimitiveType::U64,
                preserves_byte_memberships: false,
                receiving_range: None,
            },
            checked_trees::CheckedStoreValue::assigned(
                checked_trees::CheckedScalarStoreValue::Pure(literal),
            ),
        ))
    }

    /*
        The checked root of the one storage place the cursor names.

        `choose(chosen, other, true)` returns the caller's `chosen` parameter when
        that input is already a reference, and a local when the caller wrote
        `&mut chosen`. Both are the same exclusive referent. Recording the
        parameter as a local asks lowering for a home the entry never established.
    */
    fn cursor_store_root(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        storage_root: symbols::SymbolHandle,
    ) -> Option<checked_trees::CheckedStoreRoot> {
        let declaration = program.symbols.get(storage_root);
        if declaration.parent != state.symbol {
            return None;
        }
        match declaration.kind {
            symbols::SymbolKind::Local => Some(checked_trees::CheckedStoreRoot::Local {
                symbol: storage_root,
            }),
            symbols::SymbolKind::Parameter => {
                let position = program
                    .state_parameters(state)
                    .iter()
                    .position(|parameter| parameter.symbol == storage_root)?;
                Some(checked_trees::CheckedStoreRoot::Parameter {
                    position: u32::try_from(position).ok()?,
                })
            }
            _ => None,
        }
    }

    fn store_path(
        program: &dyn ::validation::CheckingInput,
        segments: &[facts::PlaceSegment],
    ) -> Option<Vec<checked_trees::CheckedUnitStructuralPathSegment>> {
        let mut path = Vec::with_capacity(segments.len());
        for segment in segments {
            let facts::PlaceSegment::Field { symbol } = segment else {
                return None;
            };
            path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                crate::execution::terminal_unit::types::terminal_field_identity(program, *symbol)?,
            ));
        }
        Some(path)
    }

    fn framed_length(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        call: &typed_trees::statement::TableCall,
        value: ExpressionHandle,
    ) -> Option<u64> {
        let leaf = program
            .statement_table
            .name_path_members(call.receiver)
            .last()?;
        let schema = program
            .wire_schemas()
            .iter()
            .find(|schema| schema.name.as_str() == leaf.as_str())?;
        let literal = struct_literal(program, state, value)?;
        let mut total = varint_len(0)?;
        for member in program.wire_members(schema.members) {
            match member {
                WireMember::Reserved(_) => {}
                WireMember::Field(field) if is_byte_slice(program, field.type_reference) => {
                    let payload = field_slice_len(program, literal, field.name.as_str())?;
                    total = total
                        .checked_add(varint_len(field.number)?)?
                        .checked_add(varint_len(payload)?)?
                        .checked_add(payload)?;
                }
                WireMember::Field(_) | WireMember::Version(_) => return None,
            }
        }
        Some(total)
    }

    fn struct_literal<'a>(
        program: &'a dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        value: ExpressionHandle,
    ) -> Option<&'a typed_trees::expression::TableStructLiteral> {
        let target = match program.expression_table.expression(value) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => value,
        };
        let ExpressionNode::Name(name) = program.expression_table.expression(target) else {
            return None;
        };
        let symbol = name.symbol;
        let initializer = program
            .statement_table
            .iter_statements(state.statement_nodes)
            .find_map(|(_, statement)| match statement {
                typed_trees::statement::StatementNode::LocalData(local)
                    if local.symbol == symbol =>
                {
                    Some(local.initial_value)
                }
                _ => None,
            })?;
        match program.expression_table.expression(initializer) {
            ExpressionNode::StructLiteral(literal) => Some(literal),
            _ => None,
        }
    }

    fn field_slice_len(
        program: &dyn ::validation::CheckingInput,
        literal: &typed_trees::expression::TableStructLiteral,
        field: &str,
    ) -> Option<u64> {
        let supplied = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|supplied| supplied.name.as_str() == field)?;
        slice_len(program, supplied.value)
    }

    fn slice_len(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<u64> {
        let expression = match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => expression,
        };
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
        else {
            return None;
        };
        let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index)
        else {
            return None;
        };
        if range.end_inclusive {
            return None;
        }
        constant_u64(program, range.end)?.checked_sub(constant_u64(program, range.start)?)
    }

    fn constant_u64(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<u64> {
        let ExpressionNode::Integer(literal) = program.expression_table.expression(expression)
        else {
            return None;
        };
        u64::try_from(literal.value_i64()?).ok()
    }

    fn is_byte_slice(
        program: &dyn ::validation::CheckingInput,
        mut type_reference: TypeReferenceHandle,
    ) -> bool {
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Slice { element_type } => {
                    return program.primitive_type_reference(*element_type)
                        == Some(PrimitiveType::U8);
                }
                _ => return false,
            }
        }
    }

    fn varint_len(mut value: u64) -> Option<u64> {
        let mut length = 1u64;
        while value >= 128 {
            value >>= 7;
            length = length.checked_add(1)?;
            if length > 10 {
                return None;
            }
        }
        Some(length)
    }
}
pub(in super::super) use result_consumption::{
    append_exit_reference_releases, append_reference_releases, captured_reference_result,
    consume_result, consume_results, consume_retained_results, prepare_value_operands,
    release_matches,
};

pub(in super::super) struct StatementSequence {
    pub(in super::super) scalar_result: Option<CheckedUnitScalarResultBindingPlan>,
    pub(in super::super) scalar_control: Option<checked_trees::CheckedUnitScalarControlPlan>,
    pub(in super::super) structural_result: Option<CheckedUnitStructuralReturnPlan>,
    pub(in super::super) operations: Vec<CheckedUnitEffectOperationPlan>,
    /*
        Exact restoration debt at the end of the operation prefix. Its exit
        owner requires closure for normal completion, not crash abandonment.
    */
    pub(in super::super) windows: super::super::borrowed_windows::OpenWindows,
    /* Every silent place loan the sequence admitted, in statement order. */
    pub(in super::super) place_loans: Vec<checked_trees::CheckedPlaceLoanPlan>,
    pub(in super::super) structural_local_symbols: Vec<SymbolHandle>,
    /*
        Every structural result binding the sequence established, keyed by
        the authored root it publishes (a `let` local's symbol or an
        anonymous operand's expression). A state's successor edges plan their
        own operand calls against these, as a later statement's call would.
    */
    pub(in super::super) structural_results:
        Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
    /*
        The dense scalar binding ordinal the next retained result takes after
        this sequence: the prefix's immutable bindings, then every scalar
        result the sequence retained. A guarded arm's terminal call result
        numbers here, in the namespace its arm inherits.
    */
    pub(in super::super) next_scalar_binding: usize,
    owner_candidates: reference_retirement::OwnerCandidates,
}

impl StatementSequence {
    /*
        Retire descriptors only after the consumer has produced this state's
        final value. A graph terminator can still move a stored reference after
        the ordinary statement prefix; disposing that old home beforehand would
        schedule a second release after its return move.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::execution::terminal_unit) fn finish_reference_retirement(
        &mut self,
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        prepared_calls: &mut crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
        shapes: &mut ShapeCollector<'_>,
        parameters: &[CheckedUnitStructuralParameterPlan],
        trivial_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
        entry_claims: &[CheckedUnitEntryClaimPlan],
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        returned: Option<&CheckedUnitStructuralReturnPlan>,
        composed: Option<
            crate::execution::terminal_unit::storage_transfers::ComposedCompletion<'_>,
        >,
        trace: &LocalConstructionTrace,
    ) -> Option<()> {
        if self
            .operations
            .iter()
            .any(|operation| !operation.has_normal_continuation())
        {
            return Some(());
        }
        prepared_calls.prepare_before(
            program,
            facts,
            shapes,
            machine,
            state,
            parameters,
            trivial_locals,
            entry_claims,
            &self.operations,
            crate::flow::state_statements(program, state.statement_nodes).len(),
        );
        trace.phase("statement sequence: completion descriptor retirement");
        append_exit_reference_releases(
            &self.owner_candidates,
            program,
            facts,
            prepared_calls,
            shapes,
            parameters,
            machine.symbol,
            state.symbol,
            returned,
            composed.map_or(
                crate::execution::terminal_unit::storage_transfers::CompletionPlan::Body {
                    state: state.symbol,
                    scalar_control: self.scalar_control.as_ref(),
                    returned,
                },
                crate::execution::terminal_unit::storage_transfers::CompletionPlan::Composed,
            ),
            &self.structural_results,
            &mut self.operations,
            trace,
        )
    }
}

/*
    The construction context every statement producer reads and extends: the
    program, the checked facts, the scalar callee closure, the shape collector,
    the machine/state pair under evaluation, the structural parameter plans,
    the entry claims, the trivial affine locals and the construction trace.
    The four producers (`assignments`, `local_data`, `statement_call` and
    `completion`) each restated this same ten-field prefix as the head of their
    own planner and `build` staged it again at every dispatch site; the prefix
    is identical for all of them, so one owner carries it and each producer's
    remaining fields name only the ledger its statement kind extends.
*/
pub(in super::super) struct Construction<'a, 'program, 'shapes> {
    pub(super) program: &'program dyn ::validation::CheckingInput,
    pub(super) facts: &'program CheckFacts,
    pub(super) scalar_callees: ScalarCalleeClosure,
    pub(super) shapes: &'a mut ShapeCollector<'shapes>,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) structural_parameters: &'a mut [CheckedUnitStructuralParameterPlan],
    pub(super) entry_claims: &'a [CheckedUnitEntryClaimPlan],
    pub(super) trivial_affine_locals:
        &'a [(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    pub(super) trace: &'a LocalConstructionTrace,
}

/*
    The trace phase for one statement kind in the shared sequence, so an
    omission names which statement family stopped local construction.
*/
fn statement_phase(statement: &StatementNode) -> &'static str {
    match statement {
        StatementNode::Assignment(_) => "statement sequence: assignment",
        StatementNode::LocalData(_) => "statement sequence: local data",
        StatementNode::Call(_) => "statement sequence: call",
        StatementNode::Expression(_) => "statement sequence: expression",
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {
            "statement sequence: assembly fact"
        }
        StatementNode::Transition(_) => "statement sequence: transition",
    }
}

/*
    A case arm's payload marker, a local whose role is `ArmPattern`,
    precedes that arm's transition; the arm, not the sequence, binds its
    fields.
*/
fn is_arm_destructure_marker(statement: &StatementNode) -> bool {
    matches!(statement, StatementNode::LocalData(local)
        if local.role == language_core::LocalBindingRole::ArmPattern)
}

/* The machine an assignment's right-hand side calls, when it is a direct call. */
fn statement_call_target(
    program: &dyn ::validation::CheckingInput,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<SymbolHandle> {
    let call = program.expression_table.call_node(assignment.value)?;
    Some(validation::call_target_symbol(program, call))
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn build(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
    shapes: &mut ShapeCollector<'_>,
    prepared_calls: &mut crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &mut [CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    parameter_storage: &[checked_trees::CheckedUnitParameterStoragePlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    calls: &[&checked_trees::FlowCallFact],
    trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    construction_statement_count: usize,
    statement_end: Bound<usize>,
    _call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &LocalConstructionTrace,
) -> Option<StatementSequence> {
    trace.phase("statement sequence: scalar control guard");
    let (scalar_control, terminator_start) = match scalar_control(program, facts, machine, state) {
        Some((control, prefix_count)) => (Some(control), prefix_count),
        None => (None, usize::MAX),
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    let sequenced = |index: usize, statement: &StatementNode| {
        let terminal_call = matches!(statement, StatementNode::Transition(_))
            && call_occurrences::terminal_invocation(program, machine, state, index).is_some();
        if (matches!(statement, StatementNode::Transition(_)) && !terminal_call)
            || is_arm_destructure_marker(statement)
        {
            return false;
        }
        let completion_expression =
            call_occurrences::completion_expression(program, machine, state, index);
        !completion_expression.is_some_and(|expression| {
            !is_unit(program, state.return_type)
                && ((!matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::Call(_)
                ) && validation::expression_normal_completion_possible(program, expression)
                    != Some(false))
                    || u32::try_from(index).ok().is_some_and(|ordinal| {
                        facts
                            .values
                            .scalar_computations
                            .root_at(state.symbol, ordinal, CheckedScalarExpressionRole::Return)
                            .is_some()
                            || facts
                                .values
                                .structural_values
                                .root_for_expression(state.symbol, ordinal, expression)
                                .is_some()
                    }))
        })
    };
    let execution_order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    trace.phase("statement sequence: execution window");
    let end_position = execution_order.prefix_end(statement_end)?;
    if !execution_order
        .indices()
        .iter()
        .copied()
        .take(construction_statement_count)
        .eq(0..construction_statement_count)
        || construction_statement_count > end_position
    {
        trace.phase("statement sequence: construction prefix differs from execution");
        return None;
    }
    let construction_position = construction_statement_count;
    let ordered = &execution_order.indices()[..end_position];
    let walk_position = ordered
        .iter()
        .enumerate()
        .skip(construction_position)
        .find(|(_, index)| !sequenced(**index, &statements[**index]))
        .map_or(end_position, |(position, _)| position);
    let rest: Vec<_> = ordered[walk_position..]
        .iter()
        .map(|index| &statements[*index])
        .collect();
    let completion_owns_rest = execution_order
        .position(terminator_start)
        .is_some_and(|position| walk_position >= position)
        || (walk_position + 1 == execution_order.indices().len()
            && matches!(rest.as_slice(), [StatementNode::Expression(_)]))
        || rest
            .iter()
            .skip_while(|statement| is_arm_destructure_marker(statement))
            .all(|statement| matches!(statement, StatementNode::Transition(_)));
    if !completion_owns_rest {
        trace.phase("statement sequence: unsupported statement kind");
        trace.statement(
            ordered
                .get(walk_position)
                .and_then(|index| u32::try_from(*index).ok()),
        );
        return None;
    }
    let owner_candidates =
        reference_retirement::OwnerCandidates::new(program, facts, machine.symbol, state.symbol)?;
    let mut operations = Vec::<CheckedUnitEffectOperationPlan>::new();
    let mut place_loans = Vec::new();
    let mut scalar_count = crate::flow::state_statements(program, state.statement_nodes)
        .iter()
        .take(construction_statement_count)
        .filter(|statement| {
            matches!(statement,
            StatementNode::LocalData(local) if !local.is_mutable
                && program.primitive_type_reference(local.type_reference).is_some())
        })
        .count();
    let mut structural_count = 0_usize;
    let mut structural_local_symbols = Vec::new();
    let mut windows = super::super::borrowed_windows::OpenWindows::default();
    let mut array_bindings = Vec::<(SymbolHandle, CheckedUnitStructuralResultBindingPlan)>::new();
    let mut returned_call = None;
    let mut returned_scalar_call = None;
    let mut atomic_result = None;
    let mut structural_results = Vec::new();
    let mut call_count = 0_usize;
    let binders = machine_binders(program, machine);
    macro_rules! construction {
        () => {
            Construction {
                program,
                facts,
                scalar_callees,
                shapes: &mut *shapes,
                machine,
                state,
                structural_parameters: &mut *structural_parameters,
                entry_claims,
                trivial_affine_locals,
                trace,
            }
        };
    }
    trace.phase("statement sequence: scalar field store sequence");
    let mut stores =
        super::super::structural_scalar_store::build_structural_scalar_field_store_sequence_traced(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            scalar_parameters,
            parameter_storage,
            construction_statement_count,
            trace,
        )?
        .into_iter()
        .peekable();
    for index in ordered[construction_position..walk_position]
        .iter()
        .copied()
    {
        if operations
            .last()
            .is_some_and(|operation| !operation.has_normal_continuation())
        {
            break;
        }
        let statement = &statements[index];
        if let StatementNode::Expression(expression) = statement
            && expressions(program)
                .call_node(*expression)
                .and_then(|call| call.native_code_layout.as_ref())
                .is_some_and(|query| query.discards_result)
        {
            let statement_index = u32::try_from(index).ok()?;
            let root = facts.values.structural_values.root_for_expression(
                state.symbol,
                statement_index,
                *expression,
            )?;
            let mut result =
                checked_structural_result_type(program, shapes, root.type_reference, &binders)?;
            result.statement_index = statement_index;
            result.binding_ordinal = u32::try_from(structural_count).ok()?;
            structural_count = structural_count.checked_add(1)?;
            operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                    result,
                    value: root.root,
                    calls: Vec::new(),
                    operand_source: None,
                    discard_result_on_return: true,
                },
            ));
            continue;
        }

        let statement_index = u32::try_from(index).ok()?;
        prepared_calls.prepare_before(
            program,
            facts,
            shapes,
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            entry_claims,
            &operations,
            index,
        );
        trace.phase("statement sequence: reference retirement");
        trace.statement(Some(statement_index));
        append_reference_releases(
            &owner_candidates,
            program,
            facts,
            prepared_calls,
            shapes,
            structural_parameters,
            machine.symbol,
            state.symbol,
            statement_index,
            &structural_results,
            &mut operations,
            trace,
        )?;
        trace.phase(statement_phase(statement));
        trace.statement(Some(statement_index));
        if matches!(
            statement,
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_)
        ) {
            continue;
        }
        if crate::execution::citation_erasure::is_fact_only_citation(program, facts, statement) {
            continue;
        }
        let completes_machine = matches!(statement, StatementNode::Expression(expression)
            if !is_unit(program, state.return_type)
                && (!matches!(program.expression_table.expression(*expression), ExpressionNode::Call(_))
                    || index.checked_add(1) == Some(statements.len())))
            || (matches!(statement, StatementNode::Transition(_))
                && !is_unit(program, state.return_type)
                && call_occurrences::terminal_invocation(program, machine, state, index).is_some());
        let mut structural_result = None;
        let mut call_result_stores = Vec::new();
        if atomic_result.is_some()
            && !matches!(statement, StatementNode::Assignment(assignment)
                if validation::atomic_assignment_carrier(program, assignment).is_some())
        {
            return None;
        }
        let result = match statement {
            StatementNode::Assignment(assignment)
                if validation::atomic_assignment_carrier(program, assignment).is_some() =>
            {
                trace.phase("statement sequence: assignment: atomic event");
                trace.statement(Some(statement_index));
                operations.push(CheckedUnitEffectOperationPlan::AtomicAccess(
                    super::super::atomic_operations::writing_event(
                        program,
                        facts,
                        machine,
                        state,
                        statement_index,
                        assignment,
                        atomic_result.take(),
                        trace,
                        shapes,
                    )?,
                ));
                continue;
            }
            StatementNode::Assignment(assignment) => match assignments::plan(
                assignments::Planner {
                    construction: construction!(),
                    binders: &binders,
                    scalar_parameters,
                    parameter_storage,
                    stores: &mut stores,
                    operations: &mut operations,
                    scalar_count: &mut scalar_count,
                    structural_count: &mut structural_count,
                    windows: &mut windows,
                    structural_results: &mut structural_results,
                },
                statement_index,
                assignment,
            )? {
                assignments::AssignmentPlan::Planned => continue,
                assignments::AssignmentPlan::ScalarCall { result, store } => {
                    call_result_stores.push(store);
                    Some(result)
                }
                assignments::AssignmentPlan::StructuralCall { result } => {
                    structural_result = Some(result);
                    None
                }
            },
            StatementNode::LocalData(local) => match local_data::plan(
                local_data::Planner {
                    construction: construction!(),
                    binders: &binders,
                    operations: &mut operations,
                    place_loans: &mut place_loans,
                    scalar_count: &mut scalar_count,
                    structural_count: &mut structural_count,
                    structural_local_symbols: &mut structural_local_symbols,
                    windows: &mut windows,
                    array_bindings: &mut array_bindings,
                    atomic_result: &mut atomic_result,
                    structural_results: &mut structural_results,
                },
                index,
                statement_index,
                local,
            )? {
                local_data::LocalPlan::Planned => continue,
                local_data::LocalPlan::ScalarCall(result) => Some(result),
                local_data::LocalPlan::StructuralCall(result) => {
                    structural_result = Some(result);
                    None
                }
            },
            StatementNode::Call(call) if call.discards_result => {
                trace.phase("statement sequence: call: discarded result type");
                trace.statement(Some(statement_index));
                let result_type = validation::call_target_return_type(program, call.target_symbol)?;
                if let Some(primitive_type) = program.primitive_type_reference(result_type) {
                    let result = CheckedUnitScalarResultBindingPlan {
                        statement_index,
                        binding_ordinal: u32::try_from(scalar_count).ok()?,
                        primitive_type,
                    };
                    Some(result)
                } else if !is_unit(program, result_type) {
                    let mut result =
                        checked_structural_result_type(program, shapes, result_type, &binders)?;
                    result.statement_index = statement_index;
                    structural_result = Some(result);
                    None
                } else {
                    None
                }
            }
            StatementNode::Call(_) => None,
            StatementNode::Expression(_)
                if completes_machine
                    && program
                        .primitive_type_reference(state.return_type)
                        .is_some() =>
            {
                let result = CheckedUnitScalarResultBindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(scalar_count).ok()?,
                    primitive_type: program.primitive_type_reference(state.return_type)?,
                };
                scalar_count = scalar_count.checked_add(1)?;
                returned_scalar_call = Some(result);
                Some(result)
            }
            StatementNode::Expression(_) if completes_machine => {
                let mut result =
                    checked_structural_result_type(program, shapes, state.return_type, &binders)?;
                result.statement_index = statement_index;
                structural_result = Some(result);
                None
            }
            StatementNode::Expression(_)
                if call_occurrences::tail_call(program, state, index).is_some() =>
            {
                None
            }
            StatementNode::Expression(_)
                if call_occurrences::unit_statement_call(program, machine, state, index)
                    .is_some() =>
            {
                None
            }
            StatementNode::Transition(_)
                if call_occurrences::terminal_invocation(program, machine, state, index)
                    .is_some() =>
            {
                if let Some(primitive_type) = program.primitive_type_reference(state.return_type) {
                    let result = CheckedUnitScalarResultBindingPlan {
                        statement_index,
                        binding_ordinal: u32::try_from(scalar_count).ok()?,
                        primitive_type,
                    };
                    scalar_count = scalar_count.checked_add(1)?;
                    returned_scalar_call = Some(result);
                    Some(result)
                } else if !is_unit(program, state.return_type) {
                    let mut result = checked_structural_result_type(
                        program,
                        shapes,
                        state.return_type,
                        &binders,
                    )?;
                    result.statement_index = statement_index;
                    structural_result = Some(result);
                    None
                } else {
                    None
                }
            }
            _ => return None,
        };
        statement_call::plan(
            statement_call::Planner {
                construction: construction!(),
                scalar_parameters,
                parameter_storage,
                calls,
                operations: &mut operations,
                structural_count: &mut structural_count,
                call_count: &mut call_count,
                windows: &mut windows,
                structural_local_symbols: &mut structural_local_symbols,
                array_bindings: &mut array_bindings,
                returned_call: &mut returned_call,
                structural_results: &mut structural_results,
            },
            statement,
            index,
            statement_index,
            statement_call::StatementCall {
                completes_machine,
                result,
                structural_result,
                result_stores: call_result_stores,
            },
        )?;
        if operations
            .last()
            .is_some_and(|operation| !operation.has_normal_continuation())
        {
            break;
        }
    }
    trace.statement(None);
    {
        completion::complete(completion::Completion {
            construction: construction!(),
            owner_candidates,
            prepared_calls,
            binders: &binders,
            calls,
            statement_end,
            scalar_control,
            operations,
            place_loans,
            scalar_count,
            structural_count,
            structural_local_symbols,
            windows,
            returned_call,
            returned_scalar_call,
            structural_results,
            call_count,
        })
    }
}

/*
    Find where this state's ordinary operations hand control to its completion.
    Assembly markers and proved assertions remain in the prefix with their
    instruction calls. The first transition, or a final value expression,
    completes the state; earlier Unit expressions remain ordinary operations.
    This selects a coordinate only. The completion still needs its exact
    checked value, result type and exit obligations.
*/
pub(in super::super) fn terminator_statement_index(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
) -> usize {
    let statements = program.statement_table.statements(state.statement_nodes);
    statements
        .iter()
        .enumerate()
        .position(|(statement_index, statement)| {
            matches!(statement, StatementNode::Transition(_))
                || (!is_unit(program, state.return_type)
                    && statement_index + 1 == statements.len()
                    && matches!(statement, StatementNode::Expression(_)))
        })
        .unwrap_or(statements.len())
}

/*
    Share scalar exit discovery while leaving operation storage with its sequence.
    The retained tail, when the state's statements end in control that returns
    the machine result — independent of what the result's carrier is. The
    scalar-control plan wraps the same terminator with its primitive carrier;
    structural result machines ask the predicate directly so a transition-only
    body counts as producing its result at the terminator.
*/
pub(in super::super) fn returning_terminator(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<(checked_trees::CheckedScalarStateTerminator, usize)> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix_count = terminator_statement_index(program, state);
    let mut guarded = facts
        .flow
        .guarded_exits
        .tails
        .iter()
        .filter(|tail| tail.state == state.symbol);
    let binary = crate::execution::terminal_scalar::checked_terminator(
        program,
        machine,
        state,
        &facts.values.scalar_expressions,
        prefix_count,
    );
    let terminator = if let Some(
        terminator @ (checked_trees::CheckedScalarStateTerminator::Conditional { .. }
        | checked_trees::CheckedScalarStateTerminator::Return { .. }
        | checked_trees::CheckedScalarStateTerminator::Crash { .. }),
    ) = binary
    {
        terminator
    } else if let Some(tail) = guarded.next() {
        if guarded.next().is_some() {
            return None;
        }
        let arms = facts.flow.guarded_exits.exits.span(tail.arms)?;
        if arms.first()?.guard_statement_ordinal as usize != prefix_count
            || !arms.iter().all(|arm| {
                matches!(
                    arm.destination,
                    checked_trees::CheckedScalarBranchDestination::Return { .. }
                )
            })
            || tail.fallback.as_ref().is_some_and(|fallback| {
                !matches!(
                    fallback,
                    checked_trees::CheckedScalarBranchDestination::Return { .. }
                )
            })
        {
            return None;
        }
        checked_trees::CheckedScalarStateTerminator::Guarded {
            arms: tail.arms,
            fallback: tail.fallback.clone(),
        }
    } else {
        crate::execution::terminal_scalar::checked_terminator(
            program,
            machine,
            state,
            &facts.values.scalar_expressions,
            prefix_count,
        )?
    };
    let returning_tail = match &terminator {
        checked_trees::CheckedScalarStateTerminator::Return { statement_ordinal } => matches!(
            statements.get(*statement_ordinal as usize),
            Some(StatementNode::Transition(_))
        ),
        checked_trees::CheckedScalarStateTerminator::Guarded { .. }
        | checked_trees::CheckedScalarStateTerminator::Conditional {
            when_true: checked_trees::CheckedScalarBranchDestination::Return { .. },
            when_false: checked_trees::CheckedScalarBranchDestination::Return { .. },
            ..
        } => true,
        checked_trees::CheckedScalarStateTerminator::Crash { .. } => true,
        _ => false,
    };
    returning_tail.then_some((terminator, prefix_count))
}

pub(in super::super) fn scalar_control(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<(checked_trees::CheckedUnitScalarControlPlan, usize)> {
    let primitive_type = program.primitive_type_reference(state.return_type)?;
    let (terminator, prefix_count) = returning_terminator(program, facts, machine, state)?;
    let retained_return = |statement, continuation| {
        let role = if continuation {
            CheckedScalarExpressionRole::ContinuationReturn
        } else {
            CheckedScalarExpressionRole::Return
        };
        facts
            .values
            .scalar_expressions
            .bound_expression_at(state.symbol, statement, role)
            .is_some()
            || facts
                .values
                .scalar_computations
                .root_at(state.symbol, statement, role)
                .is_some()
    };
    let retained_destination =
        |destination: &checked_trees::CheckedScalarBranchDestination| match destination {
            checked_trees::CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } => retained_return(*statement_ordinal, *is_continuation),
            _ => false,
        };
    let complete = match &terminator {
        checked_trees::CheckedScalarStateTerminator::Return { statement_ordinal } => {
            retained_return(*statement_ordinal, false)
        }
        checked_trees::CheckedScalarStateTerminator::Conditional {
            when_true,
            when_false,
            ..
        } => retained_destination(when_true) && retained_destination(when_false),
        checked_trees::CheckedScalarStateTerminator::Guarded { arms, fallback } => {
            facts
                .flow
                .guarded_exits
                .exits
                .span(*arms)?
                .iter()
                .all(|arm| retained_destination(&arm.destination))
                && fallback.as_ref().is_none_or(retained_destination)
        }
        checked_trees::CheckedScalarStateTerminator::Crash { .. } => true,
        _ => false,
    };
    if !complete {
        return None;
    }
    let selected_copies = if matches!(
        terminator,
        checked_trees::CheckedScalarStateTerminator::Crash { .. }
    ) {
        Vec::new()
    } else {
        let copies =
            super::selected_copies::SelectedCopies::new(program, facts, machine.symbol, state)?;
        let paths = terminator.returning_paths(&facts.flow.guarded_exits).ok()?;
        copies.for_destinations(paths.iter().map(|path| &path.destination))?
    };
    Some((
        checked_trees::CheckedUnitScalarControlPlan {
            primitive_type,
            terminator,
            selected_copies,
        },
        prefix_count,
    ))
}

fn scalar_computation_local_at(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    binding_ordinal: u32,
    local: &typed_trees::statement::TableLocalData,
    trace: &LocalConstructionTrace,
) -> Option<(
    CheckedUnitScalarResultBindingPlan,
    checked_trees::CheckedCallScalarArgument,
)> {
    let role = CheckedScalarExpressionRole::LocalInitializer { binding_ordinal };
    let computations = &facts.values.scalar_computations;
    let mut roots = computations.roots_for_statement_role(state.symbol, statement_index, role);
    let Some(root) = roots.next() else {
        trace.phase("statement sequence: local data: scalar local: pure initializer");
        trace.statement(Some(statement_index));
        let (result, value) = scalar_expression_local_at(
            program,
            facts,
            state,
            statement_index,
            binding_ordinal,
            local,
        )?;
        return Some((
            result,
            checked_trees::CheckedCallScalarArgument::Pure(value),
        ));
    };
    trace.phase("statement sequence: local data: scalar local: computation initializer");
    trace.statement(Some(statement_index));
    let primitive_type = program.primitive_type_reference(local.type_reference)?;
    if roots.next().is_some()
        || root.machine != machine
        || local.is_mutable
        || !computations.nodes.is_valid(root.root)
        || computations.nodes.get(root.root).authored_root != local.initial_value
        || computations.nodes.get(root.root).primitive_type != primitive_type
        || facts
            .values
            .scalar_expressions
            .expression_at(state.symbol, statement_index, role)
            .is_some()
    {
        return None;
    }
    Some((
        CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal,
            primitive_type,
        },
        checked_trees::CheckedCallScalarArgument::Computation(root.root),
    ))
}
