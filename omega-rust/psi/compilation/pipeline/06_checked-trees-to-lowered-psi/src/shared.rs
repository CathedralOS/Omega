/*
Shared vocabulary and checked-table readers consumed across all lowering plan families:
`LoweringError`, `TerminalIdentityTableBuilder` and dense ID allocation (`terminal_identities`),
`LoweredSelectedMachine` (`producer_result`), `DirectCall`, structural type lowering, checked fact
readers, and architectural source guards.
*/

pub(crate) mod activation_storage {

    use crate::shared::lowering_error::{LoweringError, unsupported};
    use checked_trees::{CheckedUnitCallRoute, CheckedUnitEffectOperationPlan};
    use semantic_vocabulary::StructuralTypeId;
    use terminal_psi::{
        ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalIdentityTableBuilder,
    };

    fn identity(bound: u64) -> String {
        format!("compiler(activation-byte-backing:{bound})")
    }

    pub(crate) fn backing_type(
        identities: &mut TerminalIdentityTableBuilder,
        types: &[StructuralTypeDeclaration],
        bound: u64,
    ) -> Result<StructuralTypeId, LoweringError> {
        let key = identities.intern(&identity(bound));
        let declaration = terminal_psi::structural_type_by_identity(types, key).ok_or(
            LoweringError::Unsupported("activation backing carrier was not prepared"),
        )?;
        if declaration.shape
            != StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: bound,
            })
            || declaration.owns_linear_claim
            || !declaration.no_code_affine_cleanup
            || declaration.descriptor_cleanup
        {
            return unsupported("activation backing identity has conflicting geometry or custody");
        }
        Ok(declaration.id)
    }

    pub(crate) fn prepare_operation(
        identities: &mut TerminalIdentityTableBuilder,
        types: &mut Vec<StructuralTypeDeclaration>,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        if let Some(call) = operation.call()
            && let CheckedUnitCallRoute::ActivationStorage {
                bound: Some(bound), ..
            } = call.route()
        {
            let key = identities.intern(&identity(*bound));
            if terminal_psi::structural_type_by_identity(types, key).is_none() {
                let next = types
                    .iter()
                    .map(|row| row.id.get())
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "activation backing type identity overflows",
                    ))?;
                types.push(StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id: crate::shared::terminal_identities::structural_type_id(next),
                    identity: key,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    copyable: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                        capacity: *bound,
                    }),
                });
            }
            backing_type(identities, types, *bound)?;
        }
        if let Some((_, calls)) = operation.structural_value_evaluation() {
            for call in calls {
                for dependency in call.operation_dependencies() {
                    prepare_operation(identities, types, dependency)?;
                }
            }
        }
        Ok(())
    }
}
pub(crate) mod call_operands {

    pub(crate) use terminal_psi::call_structural_arguments as structural_arguments;
}
pub(crate) mod call_sites {

    use crate::shared::checked_facts::flow_facts;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use checked_trees::expression::ExpressionHandle;
    use checked_trees::types::PrimitiveType;
    use checked_trees::{CheckedTrees, CheckedUnitCallCoordinate, FlowCallResult};
    use symbols::SymbolHandle;

    pub(crate) struct AuthoredCall {
        pub source_target: SymbolHandle,
        pub scalar_arguments: Vec<(ExpressionHandle, PrimitiveType)>,
        pub structural_arguments: Vec<(u32, ExpressionHandle)>,
        pub evaluation_order: Vec<usize>,
    }

    pub(crate) fn locate(
        checked: &CheckedTrees,
        caller_state: SymbolHandle,
        coordinate: CheckedUnitCallCoordinate,
    ) -> Result<AuthoredCall, LoweringError> {
        let program = &checked.typed;
        let (machine, _) = crate::shared::declarations::state_declaration(checked, caller_state)?;
        let control = &checked.facts.flow.control;
        let call = checked_call(
            checked,
            caller_state,
            coordinate.statement_index as usize,
            coordinate.call_ordinal as usize,
        )?;
        let source_target = call.authored_target;
        let arguments = control
            .call_arguments
            .span_or_empty(call.authored_arguments);
        let target =
            crate::shared::declarations::target_signature(checked, machine.symbol, source_target)?;
        let parameters = target.parameters;
        if arguments.len() != parameters.len() {
            return unsupported("call row disagrees with its callee's argument count");
        }
        let mut scalar_arguments = Vec::new();
        let mut structural_arguments = Vec::new();
        for (position, parameter) in parameters.iter().enumerate() {
            let argument = arguments[position];
            let Some(primitive) = program.primitive_type_reference(parameter.type_reference) else {
                structural_arguments.push((
                    u32::try_from(position).map_err(|_| {
                        LoweringError::Unsupported("call structural position exceeds u32")
                    })?,
                    argument,
                ));
                continue;
            };
            if parameter.is_const
                || (parameter.is_mutable
                    && !(matches!(primitive, PrimitiveType::Bool) || primitive.is_fixed_integer()))
            {
                return unsupported("call passes a primitive the callee cannot receive by value");
            }
            scalar_arguments.push((argument, primitive));
        }
        Ok(AuthoredCall {
            source_target,
            scalar_arguments,
            structural_arguments,
            evaluation_order: control.call_argument_evaluation_order(call),
        })
    }

    pub(crate) struct ModuleCallTarget<'source> {
        pub machine: &'source checked_trees::machine::Machine,
        pub state: &'source checked_trees::state::State,
    }

    pub(crate) fn selected_module_target<'source>(
        checked: &'source CheckedTrees,
        occurrence: &lowered_psi::LoweredSourceCallOccurrence,
    ) -> Result<ModuleCallTarget<'source>, LoweringError> {
        let (target, _, _) = checked_invocation(checked, occurrence.origin)?;
        let (machine, state) = crate::shared::declarations::state_declaration(checked, target)?;
        Ok(ModuleCallTarget { machine, state })
    }

    pub(crate) fn checked_invocation(
        checked: &CheckedTrees,
        origin: lowered_psi::LoweredSourceCallOrigin,
    ) -> Result<
        (
            symbols::SymbolHandle,
            Option<checked_trees::StaticRequirementDispatchFact>,
            Option<checked_trees::NominalMachineUseSite>,
        ),
        LoweringError,
    > {
        let selected = match origin {
            lowered_psi::LoweredSourceCallOrigin::Statement(coordinate) => {
                let call = checked_source_call(checked, coordinate)?;
                (
                    call.target_symbol,
                    call.static_requirement_dispatch,
                    source_site(checked, coordinate, call)?,
                )
            }
            lowered_psi::LoweredSourceCallOrigin::Declaration {
                state,
                application,
                call,
                ..
            } => {
                let owner =
                    checked
                        .machine_holding_state(state)
                        .ok_or(LoweringError::Unsupported(
                            "declaration invocation lost its source owner",
                        ))?;
                let (_, retained) =
                    checked_trees::CheckedCallOrigin::Declaration { application, call }
                        .declaration(
                            &checked.facts.termination.rank_arrivals,
                            owner.symbol,
                            state,
                        )
                        .map_err(LoweringError::Unsupported)?;
                (
                    retained.target_state,
                    None,
                    Some(checked_trees::NominalMachineUseSite::Expression(
                        retained.expression,
                    )),
                )
            }
        };
        Ok(selected)
    }

    pub(crate) fn checked_source_call(
        checked: &CheckedTrees,
        coordinate: crate::emission::operation_emission::buffer::SourceCallCoordinate,
    ) -> Result<&checked_trees::FlowCallFact, LoweringError> {
        checked_call(
            checked,
            coordinate.state,
            coordinate.statement_index,
            coordinate.call_ordinal,
        )
    }

    pub(crate) fn source_site(
        checked: &CheckedTrees,
        coordinate: crate::emission::operation_emission::buffer::SourceCallCoordinate,
        call: &checked_trees::FlowCallFact,
    ) -> Result<Option<checked_trees::NominalMachineUseSite>, LoweringError> {
        use checked_trees::{FlowCallSite, NominalMachineUseSite};
        match call.authored_site {
            FlowCallSite::Expression => Ok(Some(NominalMachineUseSite::Expression(
                call.authored_expression,
            ))),
            FlowCallSite::Statement => {
                let (_, state) =
                    crate::shared::declarations::state_declaration(checked, coordinate.state)?;
                let offset = u32::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported("call statement ordinal exceeds u32")
                })?;
                let start = state.statement_nodes.start();
                let arena_index =
                    start
                        .arena_index()
                        .checked_add(offset)
                        .ok_or(LoweringError::Unsupported(
                            "call statement coordinate overflows",
                        ))?;
                Ok(Some(NominalMachineUseSite::Statement(
                    arena::Handle::from_parts(arena_index, start.generation()),
                )))
            }
            FlowCallSite::NamedTransition => Ok(None),
            FlowCallSite::Unbound => unsupported("call occurrence has no bound authored site"),
        }
    }

    fn checked_call(
        checked: &CheckedTrees,
        caller_state: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Result<&checked_trees::FlowCallFact, LoweringError> {
        let control = &checked.facts.flow.control;
        let state = control
            .state_by_symbol(caller_state)
            .ok_or(LoweringError::Unsupported(
                "call coordinate has no checked flow state",
            ))?;
        let calls = control
            .calls
            .span(state.calls)
            .ok_or(LoweringError::Unsupported(
                "call coordinate has no complete checked flow roster",
            ))?;
        let mut matching = calls.iter().filter(|call| {
            call.statement_index == statement_index && call.call_ordinal == call_ordinal
        });
        let call = matching.next().ok_or(LoweringError::Unsupported(
            "call coordinate has no checked flow occurrence",
        ))?;
        if matching.next().is_some() {
            return unsupported("call coordinate has more than one checked flow occurrence");
        }
        if call.authored_site == checked_trees::FlowCallSite::Unbound {
            return unsupported("call coordinate has no bound authored site");
        }
        Ok(call)
    }

    fn planned_result(
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
    ) -> FlowCallResult {
        flow_facts(checked)
            .control
            .call_at(
                state,
                coordinate.statement_index as usize,
                coordinate.call_ordinal as usize,
            )
            .map_or_else(Default::default, |call| call.authored_result)
    }

    pub(crate) fn discards_result(
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
    ) -> bool {
        planned_result(checked, state, coordinate) == FlowCallResult::Discarded
    }

    pub(crate) fn initializes_immutable_local(
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
    ) -> bool {
        planned_result(checked, state, coordinate) == FlowCallResult::ImmutableLocal
    }
}
pub(crate) mod checked_facts {

    use checked_trees::{CheckedTrees, CheckedValueFacts, FlowFacts};

    pub(crate) fn flow_facts(checked: &CheckedTrees) -> &FlowFacts {
        &checked.facts.flow
    }

    pub(crate) fn value_facts(checked: &CheckedTrees) -> &CheckedValueFacts {
        &checked.facts.values
    }
}
pub(crate) mod content_classes {

    use checked_trees::CheckedTrees;
    use typed_trees::types::TypeReferenceHandle;

    fn classes(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> checked_trees::ContentClassRow {
        checked.facts.content_classes.classes(reference)
    }

    pub(crate) fn stored_value_contents(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> bool {
        classes(checked, reference).stored_value_contents
    }

    pub(crate) fn plain_owned_with_numeric_constraints(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> bool {
        classes(checked, reference).plain_owned_with_numeric_constraints
    }

    pub(crate) fn plain_owned_with_represented_constraints(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> bool {
        classes(checked, reference).plain_owned_with_represented_constraints
    }

    pub(crate) fn bounded_byte_capacity(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> Option<usize> {
        classes(checked, reference).bounded_byte_capacity
    }
}
pub(crate) mod declarations {

    use crate::shared::lowering_error::{LoweringError, unsupported};
    use checked_trees::CheckedTrees;
    use checked_trees::signature::StateParameter;
    use symbols::SymbolHandle;

    pub(crate) fn state_declaration(
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
    ) -> Result<
        (
            &checked_trees::machine::Machine,
            &checked_trees::state::State,
        ),
        LoweringError,
    > {
        let program = &checked.typed;
        let mut states = program
            .machine_states_named(state)
            .filter(|_| state.is_valid());
        let (machine, state) = states.next().ok_or(LoweringError::Unsupported(
            "state symbol names no declared machine state",
        ))?;
        if states.next().is_some() {
            return unsupported("state symbol names more than one declared machine state");
        }
        Ok((machine, state))
    }

    pub(crate) struct TargetSignature<'source> {
        pub parameters: &'source [StateParameter],
        pub return_type: typed_trees::types::TypeReferenceHandle,
        pub boundary: bool,
        parameter_coordinates: ParameterCoordinates,
    }

    enum ParameterCoordinates {
        Authored,
        ServiceOperands,
    }

    impl TargetSignature<'_> {
        pub(crate) fn authored_parameter_position(&self, retained_position: u32) -> Option<u32> {
            let retained_position = usize::try_from(retained_position).ok()?;
            let position = match self.parameter_coordinates {
                ParameterCoordinates::Authored => {
                    self.parameters.get(retained_position)?;
                    retained_position
                }
                ParameterCoordinates::ServiceOperands => {
                    self.parameters.iter().enumerate().nth(retained_position)?.0
                }
            };
            u32::try_from(position).ok()
        }
    }

    pub(crate) fn target_signature(
        checked: &CheckedTrees,
        caller_machine: SymbolHandle,
        source_target: SymbolHandle,
    ) -> Result<TargetSignature<'_>, LoweringError> {
        let program = &checked.typed;
        if !source_target.is_valid() {
            return unsupported("call names no live callee");
        }
        if let Some(slot) = program.dynamic_slot_by_symbol(source_target) {
            return Ok(TargetSignature {
                parameters: &slot.parameters,
                return_type: slot.return_type,
                boundary: false,
                parameter_coordinates: ParameterCoordinates::Authored,
            });
        }
        let target_state = program
            .machine_by_symbol(source_target)
            .and_then(|machine| program.machine_states(machine).first())
            .map_or(source_target, |state| state.symbol);
        let mut states = program.machines().iter().flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .filter_map(move |state| (state.symbol == target_state).then_some((machine, state)))
        });
        if let Some((machine, state)) = states.next() {
            if states.next().is_some() || !machine.symbol.is_valid() {
                return unsupported("call source custody disagrees with its authored callee owner");
            }
            return Ok(TargetSignature {
                parameters: program.state_parameters(state),
                return_type: state.return_type,
                boundary: machine.has_bodyless_signature(),
                parameter_coordinates: ParameterCoordinates::Authored,
            });
        }
        if let Some(operator) = program
            .operators()
            .iter()
            .find(|operator| operator.is_boundary && operator.symbol == source_target)
        {
            return Ok(TargetSignature {
                parameters: program.operator_parameters(operator),
                return_type: operator.return_type,
                boundary: true,
                parameter_coordinates: ParameterCoordinates::Authored,
            });
        }
        let requirement =
            if let Some((owner, signature)) = program.machine_parameter_signature(source_target) {
                if owner.symbol != caller_machine {
                    return unsupported(
                        "call source custody uses another machine's callable parameter",
                    );
                }
                signature.symbol
            } else {
                source_target
            };
        let mut signatures = program
            .traits()
            .iter()
            .flat_map(|definition| program.executable_trait_machine_instances(definition))
            .filter(|signature| signature.symbol == requirement);
        let signature = signatures.next().ok_or(LoweringError::Unsupported(
            "call source custody has no authored trait requirement",
        ))?;
        if signatures.next().is_some() || !signature.symbol.is_valid() {
            return unsupported(
                "call source custody disagrees with its authored trait requirement",
            );
        }
        Ok(TargetSignature {
            parameters: program.state_signature_parameters(signature),
            return_type: signature.return_type,
            boundary: true,
            parameter_coordinates: ParameterCoordinates::ServiceOperands,
        })
    }

    #[cfg(test)]
    mod state_selection_tests {
        use super::*;
        use arena::{Arena, Handle, HandleSpan};
        use checked_trees::{machine::Machine, state::State};
        use typed_trees::TypedTrees;

        fn rows<T: Default>(values: Vec<T>, generation: u32) -> Arena<T> {
            Arena::from_storage(T::default(), values, None, generation, true).unwrap()
        }
        fn span<T>(first: u32, count: u32, generation: u32) -> HandleSpan<T> {
            HandleSpan::from_parts(Handle::from_parts(first, generation), count)
        }
        fn fixture() -> TypedTrees {
            let state = |index, generation| State {
                symbol: SymbolHandle::from_parts(index, generation),
                ..Default::default()
            };
            let mut program = TypedTrees::default();
            program.tables.machine_states = rows(
                vec![
                    state(3, 1),
                    state(4, 1),
                    state(3, 2),
                    State::default(),
                    state(4, 1),
                ],
                3,
            );
            program.tables.machines = rows(
                vec![
                    Machine {
                        symbol: SymbolHandle::from_parts(10, 1),
                        states: span(1, 2, 3),
                        ..Default::default()
                    },
                    Machine {
                        symbol: SymbolHandle::invalid(),
                        states: span(3, 2, 3),
                        ..Default::default()
                    },
                    Machine {
                        symbol: SymbolHandle::from_parts(10, 2),
                        states: span(1, 1, 3),
                        ..Default::default()
                    },
                ],
                7,
            )
            .into();
            program.roots.machines = span(1, 3, 7);
            program
        }
        fn compare(checked: &CheckedTrees) {
            for symbol in [
                SymbolHandle::from_parts(3, 1),
                SymbolHandle::from_parts(3, 2),
                SymbolHandle::from_parts(4, 1),
                SymbolHandle::from_parts(4, 2),
                SymbolHandle::from_parts(30, 1),
                SymbolHandle::invalid(),
            ] {
                match (
                    state_declaration(checked, symbol),
                    original_scan(checked, symbol),
                ) {
                    (Ok((machine, state)), Ok((expected_machine, expected_state))) => {
                        assert!(std::ptr::eq(machine, expected_machine));
                        assert!(std::ptr::eq(state, expected_state));
                    }
                    (Err(actual), Err(expected)) => assert_eq!(actual, expected),
                    other => panic!("different lookup outcome: {other:?}"),
                }
            }
        }

        #[test]
        fn frozen_state_selection_matches_original_scan_after_equal_revision_replacements() {
            let program = fixture();
            let checked = CheckedTrees::with_roots(program.clone(), Default::default());
            compare(&checked);
            assert_eq!(
                state_declaration(&checked, SymbolHandle::from_parts(3, 1)).unwrap_err(),
                LoweringError::Unsupported(
                    "state symbol names more than one declared machine state"
                )
            );
            assert_eq!(
                state_declaration(&checked, SymbolHandle::invalid()).unwrap_err(),
                LoweringError::Unsupported("state symbol names no declared machine state")
            );
            assert_eq!(
                state_declaration(&checked, SymbolHandle::from_parts(4, 1))
                    .unwrap()
                    .0
                    .symbol,
                SymbolHandle::from_parts(10, 1)
            );
            assert!(
                !state_declaration(&checked, SymbolHandle::from_parts(3, 2))
                    .unwrap()
                    .0
                    .symbol
                    .is_valid()
            );

            let mut warmed = program.clone();
            assert_eq!(
                warmed
                    .machine_states_named(SymbolHandle::from_parts(4, 1))
                    .count(),
                1
            );
            let old_revision = warmed.tables.machine_states.revision();
            let mut replacement = fixture().tables.machine_states;
            replacement.for_each_mut(|_, state| {
                state.symbol = SymbolHandle::from_parts(30, 1);
            });
            let replacement = rows(
                replacement.iter().map(|(_, state)| state.clone()).collect(),
                3,
            );
            assert_eq!(replacement.revision(), old_revision);
            warmed.tables.machine_states = replacement;
            let machine_revision = warmed.tables.machines.revision();
            warmed.tables.machines = rows(
                vec![
                    Machine {
                        symbol: SymbolHandle::from_parts(11, 1),
                        states: span(1, 1, 3),
                        ..Default::default()
                    },
                    Machine::default(),
                    Machine::default(),
                ],
                7,
            )
            .into();
            assert_eq!(warmed.tables.machines.revision(), machine_revision);
            let rebuilt = CheckedTrees::with_roots(warmed, Default::default());
            compare(&rebuilt);
            assert!(state_declaration(&rebuilt, SymbolHandle::from_parts(4, 1)).is_err());
            assert_eq!(
                state_declaration(&rebuilt, SymbolHandle::from_parts(30, 1))
                    .unwrap()
                    .0
                    .symbol,
                SymbolHandle::from_parts(11, 1)
            );
            compare(&checked);

            let mut wrong_root = program.clone();
            wrong_root.roots.machines = span(1, 3, 8);
            compare(&CheckedTrees::with_roots(wrong_root, Default::default()));
            let mut vacant_state = program.clone();
            assert!(
                vacant_state
                    .tables
                    .machine_states
                    .free(Handle::from_parts(2, 3))
            );
            compare(&CheckedTrees::with_roots(vacant_state, Default::default()));
            let mut vacant_machine = program.clone();
            assert!(
                vacant_machine
                    .tables
                    .machines
                    .free(Handle::from_parts(2, 7))
            );
            compare(&CheckedTrees::with_roots(
                vacant_machine,
                Default::default(),
            ));
            let mut wrong_states = program;
            wrong_states
                .tables
                .machines
                .get_mut(Handle::from_parts(1, 7))
                .states = span(1, 2, 4);
            compare(&CheckedTrees::with_roots(wrong_states, Default::default()));
        }

        fn original_scan(
            checked: &CheckedTrees,
            state: symbols::SymbolHandle,
        ) -> Result<
            (
                &checked_trees::machine::Machine,
                &checked_trees::state::State,
            ),
            LoweringError,
        > {
            let program = &checked.typed;
            let mut states = program.machines().iter().flat_map(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .filter_map(move |candidate| {
                        (state.is_valid() && candidate.symbol == state)
                            .then_some((machine, candidate))
                    })
            });
            let (machine, state) = states.next().ok_or(LoweringError::Unsupported(
                "state symbol names no declared machine state",
            ))?;
            if states.next().is_some() {
                return unsupported("state symbol names more than one declared machine state");
            }
            Ok((machine, state))
        }
    }
}
pub(crate) mod direct_calls {

    use crate::emission::operation_emission::buffer::SourceQuotientCall;
    use crate::emission::operation_emission::buffer::{OperationBuffer, SourceInvocation};
    use crate::shared::lowering_error::LoweringError;
    use lowered_psi::LoweredSourceFrontierBindings;
    use semantic_vocabulary::{MachineId, ObligationId, ProofTerm, ScalarTerm, ValueId};
    use terminal_psi::{
        ClaimTransfer, CrashRouteBucket, Operation, OperationKind, OperationResult,
        StructuralArgument, StructuralMultiplicity,
    };

    pub(crate) struct DirectCall {
        pub(crate) callee: MachineId,
        pub(crate) inputs: ModuleInvocationArguments,
        pub(crate) result: DirectCallResult,
    }

    pub(crate) struct ModuleInvocationArguments {
        pub(crate) arguments: Vec<ValueId>,
        pub(crate) erased_arguments: Vec<ScalarTerm>,
        pub(crate) erased_proof_arguments: Vec<ProofTerm>,
        pub(crate) structural_arguments: Vec<StructuralArgument>,
        pub(crate) claim_transfers: Vec<ClaimTransfer>,
        pub(crate) requirement_obligations: Vec<ObligationId>,
        pub(crate) crash_continuations: Vec<CrashRouteBucket>,
    }

    impl DirectCall {
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn new(
            callee: MachineId,
            arguments: Vec<ValueId>,
            erased_arguments: Vec<ScalarTerm>,
            erased_proof_arguments: Vec<ProofTerm>,
            structural_arguments: Vec<StructuralArgument>,
            claim_transfers: Vec<ClaimTransfer>,
            requirement_obligations: Vec<ObligationId>,
            crash_continuations: Vec<CrashRouteBucket>,
            result: DirectCallResult,
        ) -> Self {
            DirectCall {
                callee,
                inputs: ModuleInvocationArguments {
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                },
                result,
            }
        }
    }

    pub(crate) enum CallRecord<'a> {
        Quotient(checked_trees::CheckedStructuralValueHandle),
        Frontier(&'a LoweredSourceFrontierBindings),
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_direct_call(
        operations: &mut OperationBuffer,
        coordinate: impl Into<SourceInvocation>,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        selected_state: symbols::SymbolHandle,
        record: CallRecord<'_>,
        result: OperationResult,
        call: DirectCall,
    ) -> Result<semantic_vocabulary::OperationId, LoweringError> {
        let operation = operations.allocate();
        match record {
            CallRecord::Quotient(value) => operations.quotient_calls.push(SourceQuotientCall {
                value,
                call: operation,
            }),
            CallRecord::Frontier(frontier_before_call) => operations.record_source_call(
                coordinate,
                source_site,
                operation,
                selected_state,
                frontier_before_call,
            )?,
        }
        operations.push(Operation::unsuspended(
            operation,
            result,
            call.into_operation_kind(),
        ));
        Ok(operation)
    }

    pub(crate) enum DirectCallResult {
        Unit,
        Scalar,
        Structural {
            multiplicity: StructuralMultiplicity,
        },
    }

    impl DirectCall {
        pub(crate) fn into_operation_kind(self) -> OperationKind {
            let Self {
                callee,
                inputs,
                result,
            } = self;
            let ModuleInvocationArguments {
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } = inputs;
            match result {
                DirectCallResult::Unit => OperationKind::CallUnit {
                    callee,
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                },
                DirectCallResult::Scalar => OperationKind::CallStructuralScalar {
                    callee,
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                },
                DirectCallResult::Structural { multiplicity }
                    if multiplicity == StructuralMultiplicity::Linear
                        && arguments.is_empty()
                        && erased_arguments.is_empty()
                        && erased_proof_arguments.is_empty() =>
                {
                    OperationKind::CallStructural {
                        callee,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        selected_evidence: Vec::new(),
                    }
                }
                DirectCallResult::Structural { .. } => {
                    OperationKind::CallStructuralWithScalarArguments {
                        callee,
                        arguments,
                        erased_arguments,
                        erased_proof_arguments,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        selected_evidence: Vec::new(),
                    }
                }
            }
        }
    }

    pub(crate) use terminal_psi::{DirectCallRole, direct_call_parts};
}
pub(crate) mod lowering_error {

    use crate::shared::checked_facts::flow_facts;
    use checked_trees::CheckedTrees;
    use diagnostics::Diagnostic;
    use language_semantics::PermissionClaimIdentity;
    use semantic_vocabulary::{
        ContentProjectionIdentity, PlaceId, PropositionError, StructuralPlaceKind,
    };

    use crate::proofs::float_meaning_projection::FloatMeaningProjectionLoweringError;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProjectedCustodyObservation {
        pub operation: semantic_vocabulary::OperationId,
        pub kind: &'static str,
        pub root_declarations: Vec<semantic_vocabulary::StructuralTypeId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedCleanupEntrySite {
        pub block: semantic_vocabulary::BlockId,
        pub target: semantic_vocabulary::BlockId,
        pub structural_arguments: Vec<terminal_psi::StructuralArgument>,
        pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedCleanupEntryMismatch {
        pub machine: semantic_vocabulary::MachineId,
        pub edge: semantic_vocabulary::EdgeId,
        pub region: terminal_psi::StructuralArgument,
        pub sites: Vec<SelectedCleanupEntrySite>,
    }

    /// Bounded context from the exact rejected projection step. It retains no
    /// declaration graph or literal bytes and grants no alternate address path.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ArgumentProjectionFailure {
        pub reason: &'static str,
        pub machine: Option<String>,
        pub root: semantic_vocabulary::StructuralTypeId,
        pub selected_type: Option<semantic_vocabulary::StructuralTypeId>,
        pub selected_carrier: &'static str,
        pub failed_step: usize,
        pub segments: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        pub segment_count: usize,
        pub source_root: Option<facts::PlaceRoot>,
        pub source_parameter: Option<u32>,
        pub source_result: Option<u32>,
        pub published_root: Option<semantic_vocabulary::StructuralTypeId>,
        pub argument_type: Option<typed_trees::type_identity::TypeIdentityId>,
        pub logical_root: Option<typed_trees::type_identity::TypeIdentityId>,
        pub root_reference_hops: u32,
        pub access: Option<checked_trees::CheckedStructuralAccess>,
        pub operation_file: &'static str,
        pub operation_line: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum LoweringError {
        TaskActivation(Vec<Diagnostic>),
        MachineNotFound(String),
        AmbiguousMachineName(String),
        MissingUnitStructuralType(String),
        InvalidCleanupCompletionIndex(semantic_vocabulary::IdTableBuildError),
        MissingDependencyRosterEntry {
            identity_kind: &'static str,
            role: &'static str,
        },
        InvalidCleanupPartition {
            reason: &'static str,
        },
        InvalidCleanupCasePartition {
            reason: &'static str,
            place: semantic_vocabulary::PlaceId,
            path: Vec<terminal_psi::StructuralPathSegment>,
            structural_type: semantic_vocabulary::StructuralTypeId,
            leaf_span: std::ops::Range<usize>,
            declared_cases: usize,
            common_scopes: usize,
        },
        InvalidReferenceContract {
            supplying_type: typed_trees::types::TypeReferenceHandle,
            reason: String,
        },
        MissingValueStorageLocal {
            machine: Option<semantic_vocabulary::MachineId>,
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan,
            current_local_count: usize,
            current_locals: Vec<(facts::PlaceRoot, semantic_vocabulary::PlaceId)>,
            completed_cleanup_home: Option<semantic_vocabulary::PlaceId>,
            retired_binding_count: usize,
            retired_bindings: Vec<(semantic_vocabulary::PlaceId, Option<u32>, &'static str)>,
        },
        MissingCapturedArgumentNamespace {
            block: semantic_vocabulary::BlockId,
            argument: terminal_psi::StructuralArgument,
            last_operation: Option<semantic_vocabulary::OperationId>,
        },
        ObservationHomeCountMismatch {
            root: checked_trees::CheckedStorageRoot,
            matching_homes: usize,
            parameter_positions: Vec<u32>,
            local_symbols: Vec<symbols::SymbolHandle>,
            view_symbols: Vec<symbols::SymbolHandle>,
        },
        MissingUnitDomainPlan {
            domain: language_semantics::SemanticDomainId,
            name: String,
        },
        MissingUnitStructuralDomainCarrier {
            domain: language_semantics::SemanticDomainId,
            carrier: semantic_vocabulary::StructuralTypeId,
        },
        InvalidOccurrenceMembership {
            machine: String,
            state: String,
            statement_index: u32,
            domain: String,
            carrier: semantic_vocabulary::StructuralTypeId,
            cause: Box<LoweringError>,
        },
        InvalidUnitDomainPredicate {
            domain: language_semantics::SemanticDomainId,
            name: String,
            carrier: semantic_vocabulary::StructuralTypeId,
            cause: Box<LoweringError>,
        },
        DebugSourceFileCountOverflow,
        DebugSourceLengthOverflow,
        MissingDebugSourceFile(usize),
        DebugSemanticCodec(terminal_codec::CodecError),
        InvalidDebugMap(terminal_codec::DebugMapError),
        InvalidControlGraph(terminal_semantics::control_graph::ControlGraphError),
        InvalidServiceIdentity(String),
        InvalidDeclarationIdentity(String),
        InvalidBoundaryApplication(String),
        InvalidFloatMeaningProjection(FloatMeaningProjectionLoweringError),
        InvalidQuotientCorrespondence(Vec<String>),
        InvalidUnitMachinePlan {
            machine: String,
            reason: &'static str,
            gap: Option<Diagnostic>,
        },
        MissingSelectionOwner {
            machine: String,
            state: String,
            statement_index: u32,
            local: String,
        },
        MissingSelectionSources {
            machine: String,
            state: String,
            statement_index: u32,
            source_places: Vec<PlaceId>,
            live_places: Vec<PlaceId>,
        },
        InvalidFieldObservationStoragePath(validation::StorageObservationTypePathError),
        InvalidQualificationStoragePath {
            reason: &'static str,
            carrier: String,
            state: String,
            statement_index: usize,
            call_ordinal: usize,
            segments: Vec<facts::PlaceSegment>,
            segment_count: usize,
            declaration: crate::emission::call_qualifications::DeclarationRepresentation,
        },
        InvalidCallMembershipSource(&'static str),
        InvalidSuspensionPlan(&'static str),
        Unsupported(&'static str),
        InvalidArgumentProjection(Box<ArgumentProjectionFailure>),
        CompilerEvaluationAdmission(String),
        CapturedLoanDescriptorMismatch {
            source_state: usize,
            target_state: usize,
            target_parameter: u32,
            earlier: terminal_psi::StructuralTypeShape,
            incoming: terminal_psi::StructuralTypeShape,
            source_access: checked_trees::CheckedStructuralAccess,
            target_access: checked_trees::CheckedStructuralAccess,
        },
        UnpublishedScalarCondition {
            owner: symbols::SymbolHandle,
            expression: typed_trees::expression::ExpressionHandle,
            reason: String,
        },
        MissingCrashArgumentSource {
            argument: terminal_psi::StructuralArgument,
        },
        ConflictingStructuralPlaceType {
            place: PlaceId,
            first: semantic_vocabulary::StructuralTypeId,
            second: semantic_vocabulary::StructuralTypeId,
        },
        InvalidSelectedCleanupEntry(Box<SelectedCleanupEntryMismatch>),
        MissingScalarParameterQualification {
            state: String,
            source_position: u32,
            prepared_positions: Vec<u32>,
        },
        MissingSelectionFrontier {
            machine: String,
            state: String,
            statement_index: u32,
            local: String,
            formation_statement: u32,
        },
        NonDenseStructuralResult {
            reason: &'static str,
            completed_bindings: usize,
            requested_binding: u32,
            statement_index: u32,
        },
        ConflictingStructuralValueHome {
            root: facts::PlaceRoot,
            established: terminal_psi::StructuralArgument,
            established_type: semantic_vocabulary::StructuralTypeId,
            requested: terminal_psi::StructuralArgument,
            requested_type: semantic_vocabulary::StructuralTypeId,
            producer_file: &'static str,
            producer_line: u32,
        },
        MissingConstructionClaim {
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            statement_index: u32,
            expression: typed_trees::expression::ExpressionHandle,
            structural_type: semantic_vocabulary::StructuralTypeId,
            root_candidates: usize,
        },
        InvalidUnitStatementPlan {
            machine: String,
            state: String,
            statement_index: u32,
            reason: &'static str,
        },
        CapturedScalarStoreEndpoint {
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            statement_index: u32,
            destination: PlaceId,
            root_type: semantic_vocabulary::StructuralTypeId,
            path: Vec<terminal_psi::StructuralPathSegment>,
            expected: semantic_vocabulary::ScalarType,
            actual: Option<semantic_vocabulary::ScalarType>,
        },
        UnavailableContractResult,
        InvalidSealedAccessorDeclaration(Diagnostic),
        OverlappingProjectedCustody {
            reason: &'static str,
            place: PlaceId,
            earlier: Vec<terminal_psi::StructuralPathSegment>,
            requested: Vec<terminal_psi::StructuralPathSegment>,
            observation: Option<Box<ProjectedCustodyObservation>>,
        },
        UnpinnedBorrowedStorageWindow {
            place: String,
            reason: &'static str,
        },
        UnadmittedQuotientRequest {
            diagnostics: Vec<String>,
        },
        InvalidPsiIntegerType,
        UnlandedIntegerLiteral,
        IntegerLandingMismatch,
        IntegerLiteralOutsideSupportedMagnitude,
        IntegerLiteralOutsidePsiType,
        ContentConservationFingerprintMismatch {
            expected: u64,
            actual: u64,
        },
        ContentIdentityFactOwnerMismatch,
        ContentPartitionFactOwnerMismatch,
        ContentPartitionNotConservation,
        ContentPartitionInputClaimNotLowered,
        ContentPartitionInputClaimBindingMismatch,
        ContentEntryClaimRequiresEntryPlace,
        ContentEntryClaimHasNoProjection,
        ContentEntryClaimMapsMultiplePlaces,
        DuplicateContentEntryClaimInput,
        OverlappingContentEntryClaimInput,
        DuplicateContentPartitionSubstitution,
        DuplicateContentPartitionComposition,
        DuplicateContentPartitionProducerCoordinate {
            state: symbols::SymbolHandle,
            statement_index: usize,
            call_ordinal: usize,
            previous_target: symbols::SymbolHandle,
            requested_target: symbols::SymbolHandle,
            previous_source_site: Option<checked_trees::NominalMachineUseSite>,
            requested_source_site: Option<checked_trees::NominalMachineUseSite>,
            previous_terminal_site: lowered_psi::LoweredSourceCallSite,
            requested_terminal_site: lowered_psi::LoweredSourceCallSite,
            source_names: Option<(String, String, String)>,
        },
        ContentPartitionProducerOperationMissing,
        ContentPartitionProducerTargetMismatch,
        ContentPartitionResultRewriteUnsupported,
        ContentPartitionDerivedSourceUnsupported,
        ContentPartitionSubstitutionCoverageMismatch,
        ContentPartitionReplayMismatch,
        UnknownContentClaimIdentity,
        ContentIdentityInputParameterMismatch,
        ContentIdentityNotDirectEquality,
        ContentIdentityProjectionMismatch,
        ContentIdentityDirectionMismatch,
        ContentIdentityRootMismatch,
        ContentIdentityClaimMapsMultiplePlaces,
        DuplicateContentIdentityProjection,
        DuplicateContentIdentityInput,
        DuplicateContentIdentityOutput,
        OverlappingContentIdentityInput,
        OverlappingContentIdentityOutput,
        ContentProjectionAlgebraMismatch(ContentProjectionIdentity),
        CrashFrontierClaimNotLowered(PermissionClaimIdentity),
        InvalidContentDomainIdentity,
        ZeroContentProjectionFingerprint,
        ContentTermNestingTooDeep,
        ConflictingContentPlaceRoot {
            id: PlaceId,
            first: StructuralPlaceKind,
            second: StructuralPlaceKind,
        },
        InvalidContentProposition(PropositionError),
        InvalidCrashPredicate(PropositionError),
    }

    impl LoweringError {
        pub(crate) fn qualification_storage_path(
            checked: &CheckedTrees,
            coordinate: crate::emission::operation_emission::buffer::SourceCallCoordinate,
            reference: typed_trees::types::TypeReferenceHandle,
            segments: &[facts::PlaceSegment],
            reason: &'static str,
            declaration: crate::emission::call_qualifications::DeclarationRepresentation,
        ) -> Self {
            Self::InvalidQualificationStoragePath {
                reason,
                carrier: checked
                    .normalized_type_identity_text(reference)
                    .chars()
                    .take(256)
                    .collect(),
                state: checked
                    .symbols
                    .display_path(coordinate.state, "::")
                    .chars()
                    .take(256)
                    .collect(),
                statement_index: coordinate.statement_index,
                call_ordinal: coordinate.call_ordinal,
                segments: segments.iter().take(32).copied().collect(),
                segment_count: segments.len(),
                declaration,
            }
        }

        pub(crate) fn with_source_names(mut self, checked: &CheckedTrees) -> Self {
            if let Self::DuplicateContentPartitionProducerCoordinate {
                state,
                previous_target,
                requested_target,
                source_names,
                ..
            } = &mut self
            {
                *source_names = Some((
                    checked.symbols.display_path(*state, "::"),
                    checked.symbols.display_path(*previous_target, "::"),
                    checked.symbols.display_path(*requested_target, "::"),
                ));
            }
            self
        }

        pub(crate) fn at_statement(
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            statement_index: u32,
            reason: &'static str,
        ) -> Self {
            Self::InvalidUnitStatementPlan {
                machine: checked.symbols.display_path(machine, "::"),
                state: checked.symbols.display_path(state, "::"),
                statement_index,
                reason,
            }
        }

        pub fn plan_gap(&self) -> Option<&Diagnostic> {
            match self {
                Self::InvalidUnitMachinePlan { gap: Some(gap), .. } => Some(gap),
                _ => None,
            }
        }
    }

    diagnostics::debug_display!(LoweringError);

    impl std::error::Error for LoweringError {}

    pub(crate) fn unsupported<T>(message: &'static str) -> Result<T, LoweringError> {
        Err(LoweringError::Unsupported(message))
    }

    pub(crate) fn invalid_suspension<T>(reason: &'static str) -> Result<T, LoweringError> {
        Err(LoweringError::InvalidSuspensionPlan(reason))
    }

    pub(crate) fn unit_machine_plan_error<'a>(
        checked: &'a CheckedTrees,
        machine_name: &'a str,
        symbol: symbols::SymbolHandle,
    ) -> impl FnOnce(LoweringError) -> LoweringError + 'a {
        move |error| match error {
            LoweringError::Unsupported(reason) => LoweringError::InvalidUnitMachinePlan {
                machine: machine_name.to_owned(),
                reason,
                gap: flow_facts(checked)
                    .terminal_unit_effects
                    .gap_diagnostic(symbol, &checked.symbols),
            },
            LoweringError::InvalidArgumentProjection(mut failure) => {
                failure
                    .machine
                    .get_or_insert_with(|| machine_name.to_owned());
                LoweringError::InvalidArgumentProjection(failure)
            }
            error => error,
        }
    }
}
pub(crate) mod producer_result {

    use crate::shared::lowering_error::{LoweringError, unsupported};
    use lowered_psi::LoweredPsi;

    #[derive(Default, Debug, PartialEq, Eq)]
    pub(crate) enum DebugPublication {
        #[default]
        FromCheckedPlan,
        Omit,
    }

    #[derive(Default)]
    pub(crate) struct LoweringCompletion {
        pub(crate) debug: DebugPublication,
    }

    pub(crate) struct LoweredSelectedMachine {
        pub(crate) terminal: LoweredPsi,
        pub(crate) source_machine_ids: Vec<(symbols::SymbolHandle, semantic_vocabulary::MachineId)>,
        pub(crate) source_boundary_ids: Vec<(
            symbols::SymbolHandle,
            semantic_vocabulary::BoundaryMachineId,
        )>,
        pub(crate) completion: LoweringCompletion,
    }

    impl LoweredSelectedMachine {
        pub(crate) fn source_mapped(
            lowered: SourceMappedLowered,
            completion: LoweringCompletion,
        ) -> Self {
            Self {
                terminal: lowered.terminal,
                source_machine_ids: lowered.source_machine_ids,
                source_boundary_ids: lowered.source_boundary_ids,
                completion,
            }
        }
    }

    pub(crate) struct SourceMappedLowered {
        pub(crate) terminal: LoweredPsi,
        pub(crate) source_machine_ids: Vec<(symbols::SymbolHandle, semantic_vocabulary::MachineId)>,
        pub(crate) source_boundary_ids: Vec<(
            symbols::SymbolHandle,
            semantic_vocabulary::BoundaryMachineId,
        )>,
    }

    impl SourceMappedLowered {
        pub(crate) fn new(
            terminal: LoweredPsi,
            sources: Vec<(symbols::SymbolHandle, semantic_vocabulary::MachineId)>,
            boundary_sources: Vec<(
                symbols::SymbolHandle,
                semantic_vocabulary::BoundaryMachineId,
            )>,
        ) -> Result<Self, LoweringError> {
            if sources.len() != terminal.semantic_module.machines.len()
                || sources.iter().enumerate().any(|(index, (source, id))| {
                    sources[..index]
                        .iter()
                        .any(|(prior_source, prior_id)| prior_source == source || prior_id == id)
                })
            {
                return unsupported("source owners do not match the exact machine catalog");
            }
            let source_machine_ids = terminal
                .semantic_module
                .machines
                .iter()
                .map(|machine| {
                    sources
                        .iter()
                        .find(|(_, id)| *id == machine.id)
                        .copied()
                        .ok_or(LoweringError::Unsupported(
                            "emitted machine has no exact source owner",
                        ))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if boundary_sources.len() != terminal.semantic_module.boundary_machines.len()
                || boundary_sources
                    .iter()
                    .enumerate()
                    .any(|(position, (source, identity))| {
                        source_machine_ids
                            .iter()
                            .any(|(machine_source, _)| machine_source == source)
                            || boundary_sources[..position].iter().any(
                                |(prior_source, prior_identity)| {
                                    prior_source == source || prior_identity == identity
                                },
                            )
                            || !terminal
                                .semantic_module
                                .boundary_machines
                                .iter()
                                .any(|boundary| boundary.id == *identity)
                    })
            {
                return unsupported("source owners do not match the exact boundary catalog");
            }
            Ok(Self {
                terminal,
                source_machine_ids,
                source_boundary_ids: boundary_sources,
            })
        }
    }
}
pub(crate) mod reference_custody {

    use checked_trees::CheckedTrees;
    use language_semantics::Multiplicity;
    use typed_trees::types::TypeReferenceHandle;

    fn custody(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> checked_trees::ReferenceCustodyRow {
        checked.facts.reference_custody.custody(reference)
    }

    pub(crate) fn result_multiplicity(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> Multiplicity {
        custody(checked, reference).result_multiplicity
    }

    pub(crate) fn binding_multiplicity(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> Multiplicity {
        custody(checked, reference).binding_multiplicity
    }
}
pub(crate) mod structural_types {

    use std::collections::BTreeSet;

    use checked_trees::CheckedUnitStructuralFieldType;
    use checked_trees::types::PrimitiveType;
    use semantic_vocabulary::{IeeeFloatFormat, StructuralCaseId, StructuralTypeId};
    use terminal_psi::{
        ByteSequenceCarrier, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType, StructuralTypeDeclaration,
    };

    use crate::emission::scalar_types::terminal_scalar_type;
    use crate::expression_preparation::bindings::structural_paths::terminal_access;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use crate::shared::terminal_identities::{allocate_dense, lookup_type_id, structural_field_id};

    pub(crate) mod scalar_refinements {

        use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
        use semantic_vocabulary::{ScalarRefinement, ScalarTerm, ScalarType};
        use typed_trees::types::PrimitiveType;

        use crate::emission::scalar_types::{integer_value, terminal_scalar_type};
        use crate::proofs::contract_predicates::{self, PredicateTerms};
        use crate::shared::lowering_error::{LoweringError, unsupported};

        pub(crate) fn lower(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            primitive: PrimitiveType,
            predicate: &CheckedBooleanExpression,
        ) -> Result<ScalarRefinement, LoweringError> {
            let carrier = terminal_scalar_type(primitive)?;
            let predicate =
                contract_predicates::proposition(identities, predicate, &FieldSubject { carrier })?;
            ScalarRefinement::new(carrier, predicate).map_err(|_| {
                LoweringError::Unsupported("scalar field predicate is not closed over its carrier")
            })
        }

        struct FieldSubject {
            carrier: ScalarType,
        }

        impl PredicateTerms for FieldSubject {
            fn structural_identity(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                left: &checked_trees::CheckedStructuralParameterField,
                right: &checked_trees::CheckedStructuralParameterField,
            ) -> Result<semantic_vocabulary::Proposition, LoweringError> {
                crate::proofs::crash_routes::lower_structural_identity(
                    identities,
                    left,
                    right,
                    &[],
                    &[],
                )
            }

            fn scalar(
                &self,
                _identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                expression: &CheckedScalarExpression,
            ) -> Result<ScalarTerm, LoweringError> {
                crate::proofs::integer_predicates::lower(expression, &[], &mut |expression| {
                    match expression {
                        CheckedScalarExpression::Parameter {
                            position: 0,
                            primitive_type,
                        } if terminal_scalar_type(*primitive_type)? == self.carrier => {
                            Ok(ScalarTerm::value(ScalarRefinement::SUBJECT, self.carrier))
                        }
                        CheckedScalarExpression::IntegerLiteral { literal } => {
                            let Some(integer) = self.carrier.as_integer() else {
                                return unsupported(
                                    "scalar field predicate has a noninteger literal carrier",
                                );
                            };
                            ScalarTerm::integer(integer, integer_value(literal, self.carrier)?)
                                .map_err(LoweringError::InvalidCrashPredicate)
                        }
                        CheckedScalarExpression::IeeeFloatLiteral { value }
                            if ScalarType::IeeeFloat(value.format()) == self.carrier =>
                        {
                            Ok(ScalarTerm::IeeeFloatLiteral(*value))
                        }
                        CheckedScalarExpression::Boolean(predicate) => {
                            self.subject_boolean(predicate)
                        }
                        _ => unsupported(
                            "scalar field predicate names a value outside its formal subject",
                        ),
                    }
                })
            }

            fn boolean(
                &self,
                _identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                expression: &CheckedBooleanExpression,
            ) -> Result<ScalarTerm, LoweringError> {
                self.subject_boolean(expression)
            }
        }

        impl FieldSubject {
            fn subject_boolean(
                &self,
                expression: &CheckedBooleanExpression,
            ) -> Result<ScalarTerm, LoweringError> {
                match expression {
                    CheckedBooleanExpression::Constant(value) => Ok(ScalarTerm::boolean(*value)),
                    CheckedBooleanExpression::Parameter { position: 0 }
                        if self.carrier == ScalarType::Boolean =>
                    {
                        Ok(ScalarTerm::value(ScalarRefinement::SUBJECT, self.carrier))
                    }
                    CheckedBooleanExpression::Not(operand) => {
                        ScalarTerm::boolean_not(self.subject_boolean(operand)?)
                            .map_err(LoweringError::InvalidCrashPredicate)
                    }
                    CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                        self.subject_boolean(left)?,
                        self.subject_boolean(right)?,
                    )
                    .map_err(LoweringError::InvalidCrashPredicate),
                    _ => unsupported("scalar field Boolean predicate names an unsupported subject"),
                }
            }
        }
    }

    pub(crate) fn lower_inline_carrier(
        field: &checked_trees::CheckedUnitStructuralFieldPlan,
        types: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    ) -> Result<Option<StructuralTypeId>, LoweringError> {
        let identity = match (
            field.scalar_carrier_identity.is_valid(),
            field.byte_sequence_type_identity.is_valid(),
        ) {
            (true, false) => field.scalar_carrier_identity,
            (false, true) => field.byte_sequence_type_identity,
            (false, false) => return Ok(None),
            (true, true) => {
                return unsupported(
                    "inline field has competing scalar and byte carrier identities",
                );
            }
        };
        lookup_type_id(types, &identity).map(Some)
    }

    pub(crate) fn terminal_structural_field_type(
        primitive: PrimitiveType,
    ) -> Result<StructuralFieldType, LoweringError> {
        Ok(match primitive {
            PrimitiveType::F32 => StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
            PrimitiveType::F64 => StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64),
            primitive => StructuralFieldType::Scalar(terminal_scalar_type(primitive)?),
        })
    }

    pub(crate) fn terminal_byte_sequence_carrier(
        carrier: checked_trees::CheckedByteSequenceCarrier,
    ) -> ByteSequenceCarrier {
        match carrier {
            checked_trees::CheckedByteSequenceCarrier::BorrowedView { access } => {
                ByteSequenceCarrier::BorrowedView {
                    access: access.map(terminal_access),
                }
            }
            checked_trees::CheckedByteSequenceCarrier::BoundedView { access, capacity } => {
                ByteSequenceCarrier::BoundedView {
                    access: access.map(terminal_access),
                    capacity,
                }
            }
            checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity } => {
                ByteSequenceCarrier::BoundedOwned { capacity }
            }
        }
    }

    pub(crate) fn lower_field_type(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        field_type: &CheckedUnitStructuralFieldType,
        type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    ) -> Result<StructuralFieldType, LoweringError> {
        Ok(match field_type {
            CheckedUnitStructuralFieldType::BoundedInteger(integer) => {
                StructuralFieldType::BoundedInteger(*integer)
            }
            CheckedUnitStructuralFieldType::RefinedScalar {
                primitive,
                predicate,
            } => StructuralFieldType::RefinedScalar(scalar_refinements::lower(
                identities, *primitive, predicate,
            )?),
            CheckedUnitStructuralFieldType::AtomicBoundedInteger(integer) => {
                StructuralFieldType::AtomicBoundedInteger(*integer)
            }
            CheckedUnitStructuralFieldType::Atomic(primitive) => {
                StructuralFieldType::Atomic(terminal_scalar_type(*primitive)?)
            }
            CheckedUnitStructuralFieldType::Scalar(primitive) => {
                terminal_structural_field_type(*primitive)?
            }
            CheckedUnitStructuralFieldType::PredicateByteSequence {
                carrier,
                predicates,
            } => StructuralFieldType::PredicateByteSequence {
                carrier: terminal_byte_sequence_carrier(*carrier),
                predicates: *predicates,
            },
            CheckedUnitStructuralFieldType::ByteSequence(carrier) => {
                StructuralFieldType::ByteSequence(terminal_byte_sequence_carrier(*carrier))
            }
            CheckedUnitStructuralFieldType::Structural { type_identity } => {
                StructuralFieldType::Structural(lookup_type_id(type_ids, type_identity)?)
            }
            CheckedUnitStructuralFieldType::Erased { type_identity } => {
                StructuralFieldType::Erased {
                    type_identity: identities.intern(
                        &checked
                            .type_identity_text(*type_identity)
                            .unwrap_or_default(),
                    ),
                }
            }
        })
    }

    pub(crate) fn lower_mixed_fields(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        fields: &[checked_trees::CheckedUnitStructuralFieldPlan],
        type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
        next_field: &mut u64,
    ) -> Result<Vec<StructuralFieldDeclaration>, LoweringError> {
        let mut field_identities = BTreeSet::new();
        fields
            .iter()
            .map(|field| {
                if !field.identity.is_valid() || !field_identities.insert(field.identity.clone()) {
                    return unsupported("mixed structural type has duplicate field identities");
                }
                let field_type =
                    lower_field_type(identities, checked, &field.field_type, type_ids)?;
                Ok(StructuralFieldDeclaration {
                    inline_carrier: lower_inline_carrier(field, type_ids)?,
                    storage: field.storage.clone(),
                    id: structural_field_id(allocate_dense(next_field)?),
                    identity: identities.intern(&field.identity.spelling()),
                    relevance: field.relevance,
                    field_type,
                })
            })
            .collect()
    }

    pub(crate) fn lower_mixed_cases(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        cases: &[checked_trees::CheckedUnitStructuralCasePlan],
        type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
        next_field: &mut u64,
        next_case: &mut u64,
    ) -> Result<Vec<StructuralCaseDeclaration>, LoweringError> {
        let mut case_identities = BTreeSet::new();
        cases
            .iter()
            .map(|case| {
                if !case.identity.is_valid() || !case_identities.insert(case.identity.clone()) {
                    return unsupported("mixed structural type has duplicate case identities");
                }
                Ok(StructuralCaseDeclaration {
                    id: StructuralCaseId::new(allocate_dense(next_case)?)
                        .expect("allocated structural case identity is nonzero"),
                    identity: identities.intern(&case.identity.spelling()),
                    fields: lower_mixed_fields(
                        identities,
                        checked,
                        &case.fields,
                        type_ids,
                        next_field,
                    )?,
                })
            })
            .collect()
    }

    pub(crate) fn unique_declaration<'a>(
        types: &'a [StructuralTypeDeclaration],
        identity: StructuralTypeId,
        absent: &'static str,
        ambiguous: &'static str,
    ) -> Result<&'a StructuralTypeDeclaration, LoweringError> {
        let mut matching = types
            .iter()
            .filter(|declaration| declaration.id == identity);
        let declaration = matching.next().ok_or(LoweringError::Unsupported(absent))?;
        if matching.next().is_some() {
            return unsupported(ambiguous);
        }
        Ok(declaration)
    }
}
pub(crate) mod terminal_identities {

    use language_semantics::{Multiplicity, PermissionClaimIdentity, ServiceReachId};
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, MachineId, ObligationId,
        OperationId, PlaceId, PropositionId, ScalarType, ServiceId, StructuralDomainId,
        StructuralFieldId, StructuralTypeId, ValueId,
    };

    use crate::shared::lowering_error::LoweringError;

    pub(crate) const BYTE_VIEW_ELEMENT_TYPE_IDENTITY: &str = "named(name(u8))";

    pub(crate) const TERMINAL_UNIT_CALL_OBLIGATION_BASE: u64 = 1_u64 << 63;

    pub(crate) const fn structural_multiplicity(
        multiplicity: Multiplicity,
    ) -> terminal_psi::StructuralMultiplicity {
        match multiplicity {
            Multiplicity::Unrestricted => terminal_psi::StructuralMultiplicity::Unrestricted,
            Multiplicity::Affine => terminal_psi::StructuralMultiplicity::Affine,
            Multiplicity::Linear => terminal_psi::StructuralMultiplicity::Linear,
        }
    }

    macro_rules! id_constructor {
        ($function:ident, $type:ty) => {
            pub(crate) fn $function(raw: u64) -> $type {
                <$type>::new(raw).expect("fixed terminal-Psi identities are nonzero")
            }
        };
    }

    id_constructor!(value_id, ValueId);
    id_constructor!(structural_type_id, StructuralTypeId);
    id_constructor!(structural_field_id, StructuralFieldId);
    id_constructor!(structural_domain_id, StructuralDomainId);
    id_constructor!(service_id, ServiceId);
    id_constructor!(boundary_machine_id, BoundaryMachineId);
    id_constructor!(machine_id, MachineId);
    id_constructor!(block_id, BlockId);
    id_constructor!(place_id, PlaceId);
    id_constructor!(claim_id, ClaimId);
    id_constructor!(operation_id, OperationId);
    id_constructor!(edge_id, EdgeId);
    id_constructor!(contract_id, ContractId);
    id_constructor!(obligation_id, ObligationId);
    id_constructor!(proposition_id, PropositionId);

    /*
        A service is named the way every other declaration is. A portable
        artifact needs a hermetic name (package or toolchain owned). Code lowered
        for one build-time invocation may instead reach a service declared in
        the invoking program's own unpackaged source, such as a boundary trait
        in `build.omg`; that code is never published, so the service takes the
        same invocation-local coordinate its machines already take. Portable
        lowering still refuses such a declaration.
    */
    pub(crate) fn service_declaration_identity(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        symbol: symbols::SymbolHandle,
    ) -> Result<terminal_psi::TerminalIdentityIndex, LoweringError> {
        crate::machine_lowering::declarations::semantic_identity(identities, checked, symbol)
            .map_err(LoweringError::InvalidServiceIdentity)
    }

    pub(crate) fn primitive_scalar_identity<'a>(
        mut plans: impl Iterator<Item = &'a checked_trees::CheckedUnitStructuralTypePlan>,
        primitive: typed_trees::types::PrimitiveType,
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        plans
            .find(|plan| {
                matches!(
                    plan.shape,
                    checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(candidate)
                        if candidate == primitive
                )
            })
            .map(|plan| plan.identity)
    }

    pub(crate) fn byte_view_element_identity<'a>(
        plans: impl Iterator<Item = &'a checked_trees::CheckedUnitStructuralTypePlan>,
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        primitive_scalar_identity(plans, typed_trees::types::PrimitiveType::U8)
    }

    pub(crate) fn lookup_type_id(
        ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
        identity: &typed_trees::type_identity::TypeIdentityId,
    ) -> Result<StructuralTypeId, LoweringError> {
        ids.iter()
            .find_map(|(candidate, id)| (candidate == identity).then_some(*id))
            .ok_or(LoweringError::Unsupported(
                "Unit closure references an unlowered structural type",
            ))
    }

    pub(crate) fn structural_declaration_by_identity<'types>(
        types: &'types [terminal_psi::StructuralTypeDeclaration],
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        type_identity: typed_trees::type_identity::TypeIdentityId,
        absent_message: &'static str,
    ) -> Result<&'types terminal_psi::StructuralTypeDeclaration, LoweringError> {
        terminal_psi::structural_type_by_identity(
            types,
            identities.intern(
                &checked
                    .type_identity_text(type_identity)
                    .unwrap_or_default(),
            ),
        )
        .ok_or(LoweringError::Unsupported(absent_message))
    }

    pub(crate) fn stored_structural_identity(
        checked: &checked_trees::CheckedTrees,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> typed_trees::type_identity::TypeIdentityId {
        validation::stored_structural_type_identity(&checked.typed, reference, &[], &[])
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct StructuralDomainBinding {
        pub byte_predicate: Option<terminal_psi::ByteSequencePredicate>,
        pub semantic_domain: language_semantics::SemanticDomainId,
        pub carrier: StructuralTypeId,
        pub domain: StructuralDomainId,
    }

    pub(crate) fn structural_domain_identity(family: &str, carrier_identity: &str) -> String {
        terminal_psi::StructuralDomainDeclaration::carrier_qualified_identity(
            family,
            carrier_identity,
        )
    }

    pub(crate) fn hex_text(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    pub(crate) fn lookup_domain_id(
        ids: &[StructuralDomainBinding],
        source: language_semantics::SemanticDomainId,
    ) -> Result<StructuralDomainId, LoweringError> {
        let mut matches = ids.iter().filter(|row| row.semantic_domain == source);
        let first = matches.next().ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered structural domain",
        ))?;
        if matches.next().is_some() {
            return Err(LoweringError::Unsupported(
                "Unit closure structural domain family requires an exact carrier",
            ));
        }
        Ok(first.domain)
    }

    pub(crate) fn lookup_domain_id_for_carrier(
        ids: &[StructuralDomainBinding],
        source: language_semantics::SemanticDomainId,
        carrier: StructuralTypeId,
    ) -> Result<StructuralDomainId, LoweringError> {
        let mut matches = ids
            .iter()
            .filter(|row| row.semantic_domain == source && row.carrier == carrier);
        let first = matches
            .next()
            .ok_or(LoweringError::MissingUnitStructuralDomainCarrier {
                domain: source,
                carrier,
            })?;
        if matches.next().is_some() {
            return Err(LoweringError::Unsupported(
                "Unit closure repeats a structural domain carrier binding",
            ));
        }
        Ok(first.domain)
    }

    pub(crate) fn lookup_service_id(
        ids: &[(ServiceReachId, ServiceId)],
        source: ServiceReachId,
    ) -> Result<ServiceId, LoweringError> {
        ids.iter()
            .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
            .ok_or(LoweringError::Unsupported(
                "Unit closure references an unlowered checked service",
            ))
    }

    pub(crate) fn lower_service_reach_sorted(
        source: &[ServiceReachId],
        ids: &[(ServiceReachId, ServiceId)],
    ) -> Result<Vec<ServiceId>, LoweringError> {
        let mut lowered = source
            .iter()
            .map(|service| lookup_service_id(ids, *service))
            .collect::<Result<Vec<_>, _>>()?;
        lowered.sort();
        lowered.dedup();
        Ok(lowered)
    }

    pub(crate) fn lookup_machine_id(
        ids: &[(symbols::SymbolHandle, MachineId)],
        source: symbols::SymbolHandle,
    ) -> Result<MachineId, LoweringError> {
        lookup_machine_id_with(
            ids,
            source,
            "Unit call target is absent from the lowered closure",
        )
    }

    pub(crate) fn lookup_machine_id_with(
        ids: &[(symbols::SymbolHandle, MachineId)],
        source: symbols::SymbolHandle,
        absent_message: &'static str,
    ) -> Result<MachineId, LoweringError> {
        ids.iter()
            .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
            .ok_or(LoweringError::Unsupported(absent_message))
    }

    pub(crate) fn lookup_claim_id(
        ids: &[(PermissionClaimIdentity, ClaimId)],
        source: PermissionClaimIdentity,
    ) -> Result<ClaimId, LoweringError> {
        ids.iter()
            .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
            .ok_or(LoweringError::Unsupported(
                "Unit claim transfer references a non-entry caller claim",
            ))
    }

    pub(crate) fn dense_identity(index: usize) -> Result<u64, LoweringError> {
        u64::try_from(index)
            .map_err(|_| LoweringError::Unsupported("terminal Unit identity count exceeds u64"))?
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "terminal Unit identity count exceeds u64",
            ))
    }

    pub(crate) fn allocate_dense(next: &mut u64) -> Result<u64, LoweringError> {
        let current = *next;
        *next = next.checked_add(1).ok_or(LoweringError::Unsupported(
            "terminal Unit identity count exceeds u64",
        ))?;
        Ok(current)
    }

    pub(crate) fn mint<T>(next: &mut u64, construct: impl FnOnce(u64) -> T) -> T {
        let current = *next;
        *next = next
            .checked_add(1)
            .expect("generated terminal-Psi identity advances");
        construct(current)
    }

    pub(crate) fn mint_with<T>(
        next: &mut u64,
        construct: impl FnOnce(u64) -> T,
        exhausted: &'static str,
    ) -> Result<T, LoweringError> {
        let current = *next;
        *next = next
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(exhausted))?;
        Ok(construct(current))
    }

    pub(crate) fn fresh_field_snapshot(
        next_value: &mut u64,
        scalar_type: ScalarType,
        field: semantic_vocabulary::StructuralFieldId,
        qualifications: &[terminal_psi::ScalarFieldQualification],
    ) -> Result<terminal_psi::ValueDeclaration, LoweringError> {
        let qualification = qualifications.iter().find(|row| row.field == field);
        Ok(terminal_psi::ValueDeclaration {
            qualifications: qualification
                .map(|row| row.value_type.qualifications)
                .unwrap_or_default(),
            id: value_id(allocate_dense(next_value)?),
            scalar_type: qualification.map_or(scalar_type, |row| row.value_type.scalar_type),
        })
    }
}
#[cfg(test)]
mod source_guards {

    use std::fs;
    use std::path::Path;

    fn source_lines() -> Vec<(String, usize, String)> {
        lines_under(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"))
    }

    fn lines_under(source_root: &Path) -> Vec<(String, usize, String)> {
        let mut directories = vec![source_root.to_path_buf()];
        let mut lines = Vec::new();
        while let Some(directory) = directories.pop() {
            let entries = fs::read_dir(&directory)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
            for entry in entries {
                let path = entry
                    .unwrap_or_else(|error| panic!("cannot list {}: {error}", directory.display()))
                    .path();
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                if path.is_dir() {
                    if !name.starts_with('.') && !matches!(name, "tests" | "examples" | "target") {
                        directories.push(path);
                    }
                    continue;
                }
                if path.extension().and_then(|extension| extension.to_str()) != Some("rs")
                    || name == "tests.rs"
                    || name.ends_with("_tests.rs")
                {
                    continue;
                }
                let source = fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
                let file = path
                    .strip_prefix(source_root)
                    .expect("scanned source stays within the crate")
                    .display()
                    .to_string();
                let mut test_close: Option<String> = None;
                let mut pending_test = false;
                for (line_number, line) in source.lines().enumerate() {
                    if let Some(close) = &test_close {
                        if line == close {
                            test_close = None;
                        }
                        continue;
                    }
                    let trimmed = line.trim_start();
                    if trimmed == "#[cfg(test)]" {
                        pending_test = true;
                        continue;
                    }
                    if pending_test {
                        pending_test = false;
                        let indent_len = line.len() - trimmed.len();
                        test_close = Some(format!("{}}}", &line[..indent_len]));
                        continue;
                    }
                    if trimmed.starts_with("/*")
                        || trimmed.starts_with('*')
                        || trimmed.ends_with("*/")
                    {
                        continue;
                    }
                    lines.push((file.clone(), line_number + 1, line.to_owned()));
                }
            }
        }
        lines
    }

    fn violations(forbidden: &[&str]) -> Vec<String> {
        let mut violations = Vec::new();
        for (file, line_number, line) in source_lines() {
            for name in forbidden {
                if line.contains(name) {
                    violations.push(format!("{file}:{line_number}: {name}"));
                }
            }
        }
        violations.sort();
        violations
    }

    fn string_literals(line: &str) -> Vec<String> {
        let mut literals = Vec::new();
        let mut characters = line.chars();
        while let Some(character) = characters.next() {
            if character != '"' {
                continue;
            }
            let mut literal = String::new();
            while let Some(character) = characters.next() {
                match character {
                    '\\' => {
                        if let Some(escaped) = characters.next() {
                            literal.push(escaped);
                        }
                    }
                    '"' => break,
                    _ => literal.push(character),
                }
            }
            literals.push(literal);
        }
        literals
    }

    #[test]
    fn lowering_reads_checked_type_facts_without_reconstruction() {
        let violations = violations(&[
            concat!("declared_place_", "type_raw"),
            concat!("expression_result_", "type_reference"),
            concat!("unwrapped_", "type_reference"),
        ]);
        assert!(
            violations.is_empty(),
            "lowering must read checked place/result/carrier facts: {violations:?}"
        );
    }

    #[test]
    fn lowering_reads_no_authored_statements_or_expressions() {
        let violations: Vec<String> = violations(&[
            concat!("statement_", "table"),
            concat!("expression_", "table"),
            concat!("Statement", "Node::"),
            concat!("Expression", "Node::"),
            concat!("reference_result_", "custody"),
        ])
        .into_iter()
        .filter(|entry| !entry.starts_with("machine_lowering.rs:"))
        .collect();
        assert!(
            violations.is_empty(),
            "lowering must read plan rows and checked facts, not authored source: {violations:?}"
        );
    }

    #[test]
    fn lowering_refusals_do_not_revalidate_the_plan() {
        let phrasings = [
            concat!("disagrees with its ", "checked"),
            concat!("omits or ", "duplicates"),
            concat!("declared more ", "than once"),
            concat!("not canonical ", "source order"),
            concat!("substi", "tuted"),
            concat!("re", "join"),
            concat!("drift", "ed"),
            concat!("changed ", "its"),
            concat!("differs ", "from its"),
            concat!("exchanged ", "its"),
        ];
        let mut violations = Vec::new();
        for (file, line_number, line) in source_lines() {
            for literal in string_literals(&line) {
                for phrasing in phrasings {
                    if literal.contains(phrasing) {
                        violations.push(format!("{file}:{line_number}: {literal}"));
                    }
                }
            }
        }
        violations.sort();
        assert!(
            violations.is_empty(),
            "lowering refusals must name an unsupported shape, not re-validate the plan: {violations:?}"
        );
    }

    #[test]
    fn successor_transitions_have_one_plan_and_one_lowering() {
        let compilation_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let mut constructors = Vec::new();
        let mut lowerings = Vec::new();
        for (file, line_number, line) in lines_under(&compilation_root) {
            let trimmed = line.trim_start();
            if trimmed.contains(concat!("CheckedStructuralControl", "SuccessorPlan {"))
                && !trimmed.starts_with(concat!("struct", " "))
                && !trimmed.starts_with(concat!("pub", " struct"))
                && !trimmed.starts_with(concat!("impl", " "))
            {
                constructors.push(format!("{file}:{line_number}: {}", trimmed));
            }
            if trimmed.contains(concat!("fn ", "successor_edge(")) {
                lowerings.push(format!("{file}:{line_number}: {}", trimmed));
            }
        }
        constructors.sort();
        lowerings.sort();
        assert_eq!(
            constructors.len(),
            1,
            "exactly one successor-plan construction: {constructors:?}"
        );
        assert_eq!(
            lowerings.len(),
            1,
            "exactly one lowering of successor transfers: {lowerings:?}"
        );
    }

    fn direct_call_construction(line: &str) -> bool {
        let Some(position) = line.find(concat!("Direct", "Call {")) else {
            return false;
        };
        let prefix = &line[..position];
        let fields = &line[position + concat!("Direct", "Call").len()..];
        if fields.trim_start().starts_with("{ .. }") {
            return false;
        }
        !(prefix.trim_start().starts_with("let ") && !prefix.contains('='))
    }

    #[test]
    fn direct_call_patterns_are_not_mints() {
        assert!(direct_call_construction(concat!(
            "Direct",
            "Call { callee }"
        )));
        assert!(direct_call_construction(concat!(
            "let call = crate::shared::direct_calls::Direct",
            "Call { callee };"
        )));
        assert!(!direct_call_construction(concat!(
            "CheckedProviderRequirementKind::Direct",
            "Call { .. }"
        )));
        assert!(!direct_call_construction(concat!(
            "let CheckedProviderRequirementKind::Direct",
            "Call { diagnostic_name } = &source.kind"
        )));
    }

    #[test]
    fn calls_have_one_emitter_and_one_argument_constructor() {
        let stage_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        const CALL_FILES: &[&str] = &[
            "attached_unit.rs",
            "unit_operation_frame.rs",
            "unit_arguments.rs",
            "operation_emission.rs",
            "scalar_lowering.rs",
            "scalar_computations.rs",
        ];
        let mut emitters = Vec::new();
        let mut constructors = Vec::new();
        let mut free_mints = Vec::new();
        for (file, line_number, line) in lines_under(&stage_root) {
            let trimmed = line.trim_start();
            if direct_call_construction(trimmed)
                && !trimmed.starts_with(concat!("struct", " "))
                && !trimmed.starts_with(concat!("pub", " struct"))
                && !trimmed.starts_with(concat!("pub", "(crate) struct"))
                && !trimmed.starts_with(concat!("impl", " "))
            {
                emitters.push(format!("{file}:{line_number}: {}", trimmed));
            }
            if trimmed.contains(concat!("fn ", "structural_argument(")) {
                constructors.push(format!("{file}:{line_number}: {}", trimmed));
            }
            if CALL_FILES.iter().any(|call_file| file.ends_with(call_file))
                && trimmed.contains(concat!(" StructuralArg", "ument {"))
            {
                free_mints.push(format!("{file}:{line_number}: {}", trimmed));
            }
        }
        emitters.sort();
        constructors.sort();
        free_mints.sort();
        assert_eq!(
            emitters.len(),
            1,
            "exactly one direct-call construction: {emitters:?}"
        );
        assert_eq!(
            constructors.len(),
            1,
            "exactly one call structural-argument constructor: {constructors:?}"
        );
        assert!(
            free_mints.is_empty(),
            "call files mint structural arguments through the shared constructor: {free_mints:?}"
        );
    }
}
