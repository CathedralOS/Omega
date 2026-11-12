/*
Value custody, linear claim frontiers, nominal cleanup, activation and atomic
storage, constants, locals, partial affine residuals, and structural identity
(`value_custody`).
*/
pub mod activation_storage {
    use crate::CheckingInput;
    use language_core::ReferenceAccess;
    use language_core::activation_storage::ACTIVATION_CORE_SOURCE;
    pub use language_core::activation_storage::ActivationStorageOperation;
    use language_semantics::{DataSupplyMode, MachineSupplyMode, Multiplicity};
    use symbols::SymbolHandle;
    use typed_trees::data::{DataDefinition, DataMember, TypeParameterKind};
    use typed_trees::machine::Machine;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, Copy)]
    pub struct ActivationStorageContract {
        pub operation: ActivationStorageOperation,
        pub declaration: SymbolHandle,
        pub claim: SymbolHandle,
        pub attempt: SymbolHandle,
        pub established_case: SymbolHandle,
        pub rejected_case: SymbolHandle,
        pub byte_type: TypeReferenceHandle,
    }

    fn sealed(program: &dyn CheckingInput, symbol: SymbolHandle, identity: &str) -> bool {
        program
            .symbol_toolchain_relative_path(symbol)
            .is_some_and(|path| path == std::path::Path::new(ACTIVATION_CORE_SOURCE))
            && program
                .normalized_hermetic_symbol_identity(symbol)
                .ok()
                .as_deref()
                == Some(identity)
    }

    fn plain_data<'a>(
        program: &'a dyn CheckingInput,
        reference: TypeReferenceHandle,
        identity: &str,
        supply: DataSupplyMode,
    ) -> Option<&'a DataDefinition> {
        let symbol = program
            .type_reference_table
            .type_reference(reference)
            .as_named_symbol()?;
        let data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == symbol)?;
        (sealed(program, symbol, identity)
            && data.is_public
            && data.supply_mode == supply
            && data.lifetime_parameters.is_empty()
            && program.data_type_parameters(data).is_empty()
            && data.generic_instance.is_none()
            && data.quotient.is_none()
            && data.requires_facts.is_empty()
            && !data.zero_gated
            && data.properties.carry.is_none()
            && data.retired_identities.is_empty())
        .then_some(data)
    }

    fn claim(program: &dyn CheckingInput, reference: TypeReferenceHandle) -> Option<SymbolHandle> {
        let data = plain_data(
            program,
            reference,
            "toolchain::ActivationClaim",
            DataSupplyMode::BoundaryOpaque,
        )?;
        (data.properties.multiplicity == Multiplicity::Linear
            && program.data_members(data).is_empty())
        .then_some(data.symbol)
    }

    fn mutable_referee(
        program: &dyn CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference {
                referee,
                access: ReferenceAccess::Mutable,
                lifetime: None,
            } => Some(*referee),
            _ => None,
        }
    }

    pub fn exact_toolchain_activation_storage_contract(
        program: &dyn CheckingInput,
        machine: &Machine,
    ) -> Option<ActivationStorageContract> {
        if machine.body_is_present || machine.supply_mode != MachineSupplyMode::Boundary {
            return None;
        }
        let operation = if sealed_machine(program, machine, "toolchain::Activation::try_claim") {
            ActivationStorageOperation::Establish
        } else if sealed_machine(program, machine, "toolchain::ActivationClaim::bytes") {
            ActivationStorageOperation::BorrowBytes
        } else if sealed_machine(program, machine, "toolchain::ActivationClaim::release") {
            ActivationStorageOperation::Release
        } else if sealed_machine(program, machine, "toolchain::ActivationClaim::split") {
            ActivationStorageOperation::Split
        } else if sealed_machine(program, machine, "toolchain::ActivationClaim::merge") {
            ActivationStorageOperation::Merge
        } else {
            return None;
        };
        if matches!(
            operation,
            ActivationStorageOperation::Split | ActivationStorageOperation::Merge
        ) {
            return split_merge_contract(program, machine, operation);
        }
        valid_header(program, machine, operation)?;

        let roster_member = |selected_operation, identity| {
            if operation == selected_operation {
                Some(machine)
            } else {
                program
                    .machines()
                    .iter()
                    .find(|candidate| sealed_machine(program, candidate, identity))
            }
        };
        let establish = roster_member(
            ActivationStorageOperation::Establish,
            "toolchain::Activation::try_claim",
        )?;
        let bytes = roster_member(
            ActivationStorageOperation::BorrowBytes,
            "toolchain::ActivationClaim::bytes",
        )?;
        let release = roster_member(
            ActivationStorageOperation::Release,
            "toolchain::ActivationClaim::release",
        )?;
        let establish_state =
            valid_header(program, establish, ActivationStorageOperation::Establish)?;
        let bytes_state = valid_header(program, bytes, ActivationStorageOperation::BorrowBytes)?;
        let release_state = valid_header(program, release, ActivationStorageOperation::Release)?;
        let [bytes_parameter] = program.state_parameters(bytes_state) else {
            return None;
        };
        let [release_parameter] = program.state_parameters(release_state) else {
            return None;
        };
        let claim_symbol = claim(program, release_parameter.type_reference)?;
        if claim(
            program,
            mutable_referee(program, bytes_parameter.type_reference)?,
        )? != claim_symbol
            || !matches!(
                program
                    .type_reference_table
                    .type_reference(release_state.return_type),
                TypeReferenceNode::Unit
            )
        {
            return None;
        }
        let byte_slice = mutable_referee(program, bytes_state.return_type)?;
        let TypeReferenceNode::Slice {
            element_type: byte_type,
        } = program.type_reference_table.type_reference(byte_slice)
        else {
            return None;
        };
        if !plain_primitive(program, *byte_type, PrimitiveType::U8) {
            return None;
        }
        let attempt = plain_data(
            program,
            establish_state.return_type,
            "toolchain::ActivationClaimAttempt",
            DataSupplyMode::CheckedShape,
        )?;
        if attempt.properties.multiplicity != Multiplicity::Affine {
            return None;
        }
        let [
            DataMember::Variant(established),
            DataMember::Variant(rejected),
        ] = program.data_members(attempt)
        else {
            return None;
        };
        let [payload] = program.data_payload_fields(established) else {
            return None;
        };
        if established.name.as_str() != "Established"
            || rejected.name.as_str() != "Rejected"
            || claim(program, payload.type_reference)? != claim_symbol
            || payload.relevance.is_erased()
            || payload.identity.is_some()
            || established.identity.is_some()
            || rejected.identity.is_some()
            || !established.retired_payload_identities.is_empty()
            || !rejected.retired_payload_identities.is_empty()
            || !established.requires_facts.is_empty()
            || !rejected.requires_facts.is_empty()
            || !rejected.payload.is_empty()
        {
            return None;
        }
        Some(ActivationStorageContract {
            operation,
            declaration: machine.symbol,
            claim: claim_symbol,
            attempt: attempt.symbol,
            established_case: established.symbol,
            rejected_case: rejected.symbol,
            byte_type: *byte_type,
        })
    }

    fn sealed_machine(program: &dyn CheckingInput, machine: &Machine, identity: &str) -> bool {
        let declaration = program
            .machine_specialization(machine.symbol)
            .map_or(machine.symbol, |application| application.template);
        sealed(program, declaration, identity)
    }

    fn valid_header<'a>(
        program: &'a dyn CheckingInput,
        machine: &'a Machine,
        operation: ActivationStorageOperation,
    ) -> Option<&'a typed_trees::state::State> {
        let state = common_boundary_state(program, machine)?;
        let [parameter] = program.state_parameters(state) else {
            return None;
        };
        if parameter.is_const
            || (parameter.is_mutable && operation != ActivationStorageOperation::BorrowBytes)
            || parameter.relevance.is_erased()
        {
            return None;
        }
        let parameters = program
            .machine_specialization(machine.symbol)
            .map(|application| {
                program
                    .data_type_parameters
                    .span_or_empty(application.template_parameters)
            })
            .unwrap_or_else(|| program.machine_type_parameters(machine));
        if operation == ActivationStorageOperation::Establish {
            let [bound] = parameters else {
                return None;
            };
            let TypeParameterKind::Const { type_reference } = bound.kind else {
                return None;
            };
            if !plain_primitive(program, type_reference, PrimitiveType::U64)
                || !plain_primitive(program, parameter.type_reference, PrimitiveType::U64)
                || !sealed(
                    program,
                    machine.attached_data_symbol,
                    "toolchain::Activation",
                )
            {
                return None;
            }
        } else if !parameters.is_empty()
            || !sealed(
                program,
                machine.attached_data_symbol,
                "toolchain::ActivationClaim",
            )
        {
            return None;
        }
        Some(state)
    }

    fn split_merge_contract(
        program: &dyn CheckingInput,
        machine: &Machine,
        operation: ActivationStorageOperation,
    ) -> Option<ActivationStorageContract> {
        let establish = program.machines().iter().find(|candidate| {
            sealed_machine(program, candidate, "toolchain::Activation::try_claim")
        })?;
        let base = exact_toolchain_activation_storage_contract(program, establish)?;
        let state = common_boundary_state(program, machine)?;
        if !sealed(
            program,
            machine.attached_data_symbol,
            "toolchain::ActivationClaim",
        ) {
            return None;
        }
        let parameters = program.state_parameters(state);
        let (identity, covered_fields, rejected_fields) = match operation {
            ActivationStorageOperation::Split => {
                let [claim_parameter, committed] = parameters else {
                    return None;
                };
                if claim_parameter.is_mutable
                    || committed.is_mutable
                    || claim(program, claim_parameter.type_reference)? != base.claim
                    || !plain_primitive(program, committed.type_reference, PrimitiveType::U64)
                {
                    return None;
                }
                let type_parameters = program
                    .machine_specialization(machine.symbol)
                    .map(|application| {
                        program
                            .data_type_parameters
                            .span_or_empty(application.template_parameters)
                    })
                    .unwrap_or_else(|| program.machine_type_parameters(machine));
                let [bound] = type_parameters else {
                    return None;
                };
                let TypeParameterKind::Const { type_reference } = bound.kind else {
                    return None;
                };
                if !plain_primitive(program, type_reference, PrimitiveType::U64) {
                    return None;
                }
                ("toolchain::ActivationClaimSplit", 2, 1)
            }
            ActivationStorageOperation::Merge => {
                let [lower, upper] = parameters else {
                    return None;
                };
                if lower.is_mutable
                    || upper.is_mutable
                    || claim(program, lower.type_reference)? != base.claim
                    || claim(program, upper.type_reference)? != base.claim
                    || !program.machine_type_parameters(machine).is_empty()
                {
                    return None;
                }
                ("toolchain::ActivationClaimMerge", 1, 2)
            }
            _ => return None,
        };
        let outcome = plain_data(
            program,
            state.return_type,
            identity,
            DataSupplyMode::CheckedShape,
        )?;
        if outcome.properties.multiplicity != Multiplicity::Affine {
            return None;
        }
        let [covered, rejected] = program.data_members(outcome) else {
            return None;
        };
        let DataMember::Variant(covered) = covered else {
            return None;
        };
        let DataMember::Variant(rejected) = rejected else {
            return None;
        };
        if covered.name.as_str() != "Covered" || rejected.name.as_str() != "Rejected" {
            return None;
        }
        let covered_payload = program.data_payload_fields(covered);
        let rejected_payload = program.data_payload_fields(rejected);
        if covered_payload.len() != covered_fields || rejected_payload.len() != rejected_fields {
            return None;
        }
        if covered_payload
            .iter()
            .chain(rejected_payload.iter())
            .any(|field| {
                claim(program, field.type_reference) != Some(base.claim)
                    || field.relevance.is_erased()
                    || field.identity.is_some()
            })
        {
            return None;
        }
        Some(ActivationStorageContract {
            operation,
            declaration: machine.symbol,
            claim: base.claim,
            attempt: outcome.symbol,
            established_case: covered.symbol,
            rejected_case: rejected.symbol,
            byte_type: base.byte_type,
        })
    }

    fn common_boundary_state<'a>(
        program: &'a dyn CheckingInput,
        machine: &'a Machine,
    ) -> Option<&'a typed_trees::state::State> {
        let declaration = program
            .machine_specialization(machine.symbol)
            .map_or(machine.symbol, |application| application.template);
        let public_declaration = program
            .machines()
            .iter()
            .find(|candidate| candidate.symbol == declaration)
            .is_some_and(|candidate| candidate.is_public);
        if !public_declaration
            || machine.body_is_present
            || machine.supply_mode != MachineSupplyMode::Boundary
            || !machine.lifetime_parameters.is_empty()
            || machine.spelling.is_some()
            || !program.machine_trait_conformances(machine).is_empty()
            || !machine.contracts.is_empty()
            || machine.suspends
            || machine.blocks
            || !machine.conformance_bounds.is_empty()
            || !machine.invokes.is_empty()
            || machine.target.is_some()
            || machine.authored_target.is_some()
            || machine.target_family_interface
            || machine.purpose != typed_trees::machine::MachinePurpose::Runtime
            || !machine.owned_data.is_empty()
            || machine.service_reach_is_installation_bound
            || machine.admitted_same_stack_bytes != 0
        {
            return None;
        }
        let [state] = program.machine_states(machine) else {
            return None;
        };
        if !state.contracts.is_empty() {
            return None;
        }
        Some(state)
    }

    pub fn activation_storage_bound(
        program: &dyn CheckingInput,
        machine: &Machine,
    ) -> Result<u64, &'static str> {
        let contract = exact_toolchain_activation_storage_contract(program, machine)
            .ok_or("activation establishment has no exact sealed declaration contract")?;
        if !matches!(
            contract.operation,
            ActivationStorageOperation::Establish | ActivationStorageOperation::Split
        ) {
            return Err("only activation establishment has a static backing bound");
        }
        let application = program
            .machine_specialization(machine.symbol)
            .ok_or("activation establishment requires a closed static bound application")?;
        let [parameter] = program
            .data_type_parameters
            .span_or_empty(application.template_parameters)
        else {
            return Err("activation bound lost its original const binder");
        };
        let Some(typed_trees::static_applications::StaticMachineBindingValue::Const(actual)) =
            program.specialization_binding(application, parameter.symbol)
        else {
            return Err("activation bound has no unique const substitution");
        };
        match program.type_reference_table.type_reference(*actual) {
            TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() => name
                .as_str()
                .parse()
                .map_err(|_| "activation bound is not a closed u64 byte extent"),
            TypeReferenceNode::ConstExpression(expression) => program
                .closed_integer_expression_value(*expression)
                .and_then(|value| value.to_u64())
                .ok_or("activation bound is not a closed u64 byte extent"),
            _ => Err("activation bound is not a closed u64 byte extent"),
        }
    }

    fn plain_primitive(
        program: &dyn CheckingInput,
        reference: TypeReferenceHandle,
        expected: PrimitiveType,
    ) -> bool {
        matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { .. }
        ) && program.primitive_type_reference(reference) == Some(expected)
    }

    /// The sealed `toolchain::ActivationClaim` declaration itself: opaque,
    /// linear and fieldless, exactly as the activation storage contract
    /// requires of its claim.
    pub fn is_toolchain_activation_claim(
        program: &dyn CheckingInput,
        data: &DataDefinition,
    ) -> bool {
        data.supply_mode == DataSupplyMode::BoundaryOpaque
            && data.properties.multiplicity == Multiplicity::Linear
            && program.data_members(data).is_empty()
            && sealed(program, data.symbol, "toolchain::ActivationClaim")
    }

    pub fn activation_claim_byte_carrier(
        program: &dyn CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        if !sealed(program, symbol, "toolchain::ActivationClaim") {
            return None;
        }
        program.machines().iter().find_map(|machine| {
            let contract = exact_toolchain_activation_storage_contract(program, machine)?;
            (contract.claim == symbol).then_some(contract.byte_type)
        })
    }

    pub fn activation_accessor_has_empty_write_frame(
        program: &dyn CheckingInput,
        target: SymbolHandle,
    ) -> bool {
        let Some((machine, state)) = crate::find_machine_by_entry_state(program, target) else {
            return false;
        };
        program.machine_states(machine).len() == 1
            && state.symbol == target
            && exact_toolchain_activation_storage_contract(program, machine).is_some_and(
                |contract| contract.operation == ActivationStorageOperation::BorrowBytes,
            )
    }

    pub fn type_carries_activation_claim(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        let mut visiting = Vec::new();
        carries_activation_claim(program, type_reference, &mut visiting)
    }

    fn carries_activation_claim(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
        visiting: &mut Vec<SymbolHandle>,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                if activation_claim_byte_carrier(program, *symbol).is_some() {
                    return true;
                }
                if visiting.contains(symbol) {
                    return false;
                }
                let Some(record) =
                    crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
                else {
                    return false;
                };
                visiting.push(*symbol);
                let carries = program
                    .data_member_fields(program.data_members(record))
                    .any(|field| carries_activation_claim(program, field.type_reference, visiting));
                visiting.pop();
                carries
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                carries_activation_claim(program, *element_type, visiting)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                carries_activation_claim(program, *base_type, visiting)
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                if program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .any(|argument| carries_activation_claim(program, *argument, visiting))
                {
                    return true;
                }
                if visiting.contains(base_symbol) {
                    return false;
                }
                let Some(record) = crate::declarations::data_queries::data_definition_by_symbol(
                    program,
                    *base_symbol,
                ) else {
                    return false;
                };
                visiting.push(*base_symbol);
                let carries = program
                    .data_member_fields(program.data_members(record))
                    .any(|field| carries_activation_claim(program, field.type_reference, visiting));
                visiting.pop();
                carries
            }
            _ => false,
        }
    }
}

pub mod atomic_fences {
    use crate::CheckingInput;
    use language_core::atomic::{ATOMIC_CORE_SOURCE, FenceOrdering};
    use symbols::SymbolHandle;
    use typed_trees::operator::OperatorDefinition;

    pub struct AtomicFenceContract {
        pub declaration: SymbolHandle,
        pub ordering_type: typed_trees::types::TypeReferenceHandle,
        pub cases: [(SymbolHandle, FenceOrdering); 3],
    }

    fn sealed_identity(program: &dyn CheckingInput, symbol: SymbolHandle, identity: &str) -> bool {
        program
            .normalized_hermetic_symbol_identity(symbol)
            .ok()
            .as_deref()
            == Some(identity)
            && program
                .symbol_toolchain_relative_path(symbol)
                .is_some_and(|path| path == std::path::Path::new(ATOMIC_CORE_SOURCE))
    }

    pub fn exact_toolchain_atomic_fence_contract(
        program: &dyn CheckingInput,
        operator: &OperatorDefinition,
    ) -> Option<AtomicFenceContract> {
        if !sealed_identity(program, operator.symbol, "toolchain::Atomic::fence")
            || !operator.is_public
            || operator.is_boundary
            || operator.spelling.is_some()
            || !operator.lifetime_parameters.is_empty()
            || !program.operator_type_parameters(operator).is_empty()
            || !program.operator_contracts(operator).is_empty()
            || operator.return_type.is_valid()
        {
            return None;
        }
        let [parameter] = program.operator_parameters(operator) else {
            return None;
        };
        if parameter.is_const || parameter.is_mutable || parameter.relevance.is_erased() {
            return None;
        }
        let (data, symbol) =
            crate::value_custody::intrinsic_boundaries::checked_shape_data_declaration(
                program,
                parameter.type_reference,
                "FenceOrdering",
            )?;
        if !sealed_identity(program, symbol, "toolchain::FenceOrdering")
            || data.properties.multiplicity != language_semantics::Multiplicity::Unrestricted
        {
            return None;
        }
        let members = program.data_members(data);
        if members.len() != 3 {
            return None;
        }
        let mut cases = [(SymbolHandle::invalid(), FenceOrdering::Receive); 3];
        for (index, (member, (name, ordering))) in members
            .iter()
            .zip([
                ("Receive", FenceOrdering::Receive),
                ("Publish", FenceOrdering::Publish),
                ("ReceivePublish", FenceOrdering::ReceivePublish),
            ])
            .enumerate()
        {
            let typed_trees::data::DataMember::Variant(case) = member else {
                return None;
            };
            if case.name.as_str() != name
                || !case.payload.is_empty()
                || !case.requires_facts.is_empty()
            {
                return None;
            }
            cases[index] = (case.symbol, ordering);
        }
        Some(AtomicFenceContract {
            declaration: operator.symbol,
            ordering_type: parameter.type_reference,
            cases,
        })
    }

    pub(crate) fn validate_atomic_fence_declarations(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<diagnostics::Diagnostic>,
    ) {
        for operator in program.operators() {
            if sealed_identity(program, operator.symbol, "toolchain::Atomic::fence")
                && exact_toolchain_atomic_fence_contract(program, operator).is_none()
            {
                diagnostics.push(diagnostics::Diagnostic::error(
                "sealed Atomic::fence must take exactly the sealed copy FenceOrdering with Receive, Publish and ReceivePublish cases and return Unit"));
            }
        }
    }
}

pub mod atomic_operations {
    use access_plans::{AccessOperation, AtomicAccessOperation, BorrowPolarity};
    use language_core::atomic::AtomicOrderingPlan;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableAtomicExpression,
    };
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::TableAssignment;
    use typed_trees::types::TypeReferenceNode;

    pub(crate) fn assignment_receiver_polarity(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        assignment: &TableAssignment,
    ) -> BorrowPolarity {
        target_atomic_operation(program, machine, state, assignment.target, assignment.value)
            .map_or(BorrowPolarity::Exclusive, |operation| {
                AccessOperation::Atomic(operation).receiver_polarity()
            })
    }

    pub(crate) fn target_atomic_operation(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        target: ExpressionHandle,
        value: ExpressionHandle,
    ) -> Option<AtomicAccessOperation> {
        if !place_is_atomic_storage(program, machine, state, target) {
            return None;
        }
        let ExpressionNode::Atomic(atomic) = program.expression_table.expression(value) else {
            return None;
        };
        atomic_write_operands(program, atomic).map(|(operation, _)| operation)
    }

    pub fn is_atomic_type(
        program: &dyn crate::CheckingInput,
        handle: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Named { symbol, .. } => program
                .symbols
                .builtin_type_atom(*symbol)
                .is_some_and(|atom| atom.is_atomic()),
            TypeReferenceNode::Reference { referee, .. } => is_atomic_type(program, *referee),
            TypeReferenceNode::Constrained { base_type, .. } => is_atomic_type(program, *base_type),
            _ => false,
        }
    }

    pub fn place_is_atomic_storage(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        place: ExpressionHandle,
    ) -> bool {
        let Some(declared) =
            crate::value_custody::places::declared_place_type(program, machine, state, place)
        else {
            return false;
        };
        match program.type_reference_table.type_reference(declared) {
            TypeReferenceNode::Named { symbol, .. } => program
                .symbols
                .builtin_type_atom(*symbol)
                .is_some_and(|atom| atom.is_atomic()),
            _ => false,
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AtomicCarrierAccess {
        pub operation: AtomicAccessOperation,
        pub place: ExpressionHandle,
        pub operands: Vec<ExpressionHandle>,
        pub result: ExpressionHandle,
    }

    pub fn atomic_once_carrier(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<AtomicCarrierAccess> {
        let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) else {
            return None;
        };
        atomic_once_operands(program, atomic)
    }

    pub fn atomic_once_operands(
        program: &dyn crate::CheckingInput,
        atomic: &TableAtomicExpression,
    ) -> Option<AtomicCarrierAccess> {
        let AtomicOrderingPlan::CompareExchangeOnce { success, failure } = atomic.ordering else {
            return None;
        };
        if !atomic.result_custody.is_valid_for(atomic.ordering)
            || atomic.result.is_valid()
            || !failure.valid_compare_exchange_failure(success)
        {
            return None;
        }
        let language_core::atomic::AtomicExpressionResultCustody::CompareExchangeOnce(custody) =
            atomic.result_custody
        else {
            return None;
        };
        let ExpressionNode::Call(call) = program.expression_table.expression(atomic.value) else {
            return None;
        };
        let [expected, replacement] = program.expression_table.expression_handles(call.arguments)
        else {
            return None;
        };
        if !call.receiver.is_valid()
            || call.target.as_str() != custody.observation.single_attempt_member_name()
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
        {
            return None;
        }
        Some(AtomicCarrierAccess {
            operation: AtomicAccessOperation::single_attempt(custody.observation, success, failure),
            place: call.receiver,
            operands: vec![*expected, *replacement],
            result: ExpressionHandle::invalid(),
        })
    }

    pub fn atomic_execution_children(
        program: &dyn crate::CheckingInput,
        atomic: &TableAtomicExpression,
    ) -> Vec<ExpressionHandle> {
        atomic_once_operands(program, atomic)
            .map(|carrier| {
                std::iter::once(carrier.place)
                    .chain(carrier.operands)
                    .collect()
            })
            .unwrap_or_else(|| vec![atomic.value])
    }

    pub fn atomic_once_write_place(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let carrier = atomic_once_carrier(program, expression)?;
        place_is_atomic_storage(program, machine, Some(state), carrier.place)
            .then_some(carrier.place)
    }

    pub fn atomic_once_result_type(
        program: &dyn crate::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
        primitive: typed_trees::types::PrimitiveType,
        custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
    ) -> bool {
        fn check(
            program: &dyn crate::CheckingInput,
            reference: typed_trees::types::TypeReferenceHandle,
            primitive: typed_trees::types::PrimitiveType,
            custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
        ) -> Option<()> {
            if !custody.is_canonical() {
                return None;
            }
            let data = program.closed_data_definition(reference)?;
            let origin = data.generic_instance.unwrap_or(reference);
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } = program.type_reference_table.type_reference(origin)
            else {
                return None;
            };
            if !lifetime_arguments.is_empty()
                || program
                    .normalized_hermetic_symbol_identity(*base_symbol)
                    .ok()
                    .as_deref()
                    .and_then(|name| name.strip_prefix("toolchain::"))
                    != Some(custody.outcome_identity.name())
                || program.symbol_toolchain_relative_path(*base_symbol)
                    != Some(std::path::Path::new("layout.omg"))
                || data.properties.multiplicity != match custody.observation {
                    language_core::atomic::AtomicCompareExchangeObservationAxis::Observing => {
                        language_semantics::Multiplicity::Unrestricted
                    }
                    language_core::atomic::AtomicCompareExchangeObservationAxis::NonObserving => {
                        language_semantics::Multiplicity::Affine
                    }
                }
                || !data.requires_facts.is_empty()
                || data.zero_gated
                || data.quotient.is_some()
            {
                return None;
            }
            let [resident] = program
                .type_reference_table
                .type_reference_handles(*arguments)
            else {
                return None;
            };
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(*resident)
            else {
                return None;
            };
            if program.symbols.builtin_type_atom(*symbol)?.is_atomic()
                || program.primitive_type_reference(*resident)? != primitive
            {
                return None;
            }
            let members = program.data_members(data);
            if members.len() != 3 {
                return None;
            }
            for (index, member) in members.iter().enumerate() {
                let typed_trees::data::DataMember::Variant(case) = member else {
                    return None;
                };
                let schema = custody.outcome_identity.cases().get(index)?;
                let name = schema.case.name();
                let payload = schema.payload;
                if case.name.as_str() != name || !case.requires_facts.is_empty() {
                    return None;
                }
                let fields = program.data_payload_fields(case);
                if payload.is_none() {
                    if !fields.is_empty() {
                        return None;
                    }
                } else {
                    let [field] = fields else {
                        return None;
                    };
                    if field.name.as_str() != payload?.field_name()
                        || field.relevance.is_erased()
                        || program.normalized_type_identity(field.type_reference)
                            != program.normalized_type_identity(*resident)
                    {
                        return None;
                    }
                }
            }
            Some(())
        }
        check(program, reference, primitive, custody).is_some()
    }

    pub fn atomic_load_carrier(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<AtomicCarrierAccess> {
        let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) else {
            return None;
        };
        let AtomicOrderingPlan::Load(ordering) = atomic.ordering else {
            return None;
        };
        (ordering.valid_for_load()
            && atomic.result_custody.is_valid_for(atomic.ordering)
            && !atomic.result_custody.has_structural_result()
            && !atomic.result.is_valid())
        .then_some(AtomicCarrierAccess {
            operation: AtomicAccessOperation::Load(ordering),
            place: atomic.value,
            operands: Vec::new(),
            result: ExpressionHandle::invalid(),
        })
    }

    pub fn atomic_result_placeholder(
        program: &dyn crate::CheckingInput,
        state: &State,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<AtomicCarrierAccess> {
        if local.role != language_core::LocalBindingRole::AtomicObservedResult {
            return None;
        }
        let (ordinal, _) = program.state_local_declaration(state, local.symbol)?;
        let typed_trees::statement::StatementNode::Assignment(assignment) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(ordinal + 1)?
        else {
            return None;
        };
        let carrier = atomic_assignment_carrier(program, assignment)?;
        let path = program.expression_table.name_path_node(carrier.result)?;
        let matches = if path.symbol.is_valid() {
            path.symbol == local.symbol
        } else {
            matches!(program.expression_table.name_path_members(path.members), [name] if *name == local.name)
        };
        matches.then_some(carrier)
    }

    pub fn atomic_assignment_carrier(
        program: &dyn crate::CheckingInput,
        assignment: &TableAssignment,
    ) -> Option<AtomicCarrierAccess> {
        let ExpressionNode::Atomic(atomic) = program.expression_table.expression(assignment.value)
        else {
            return None;
        };
        let (operation, operands) = atomic_write_operands(program, atomic)?;
        Some(AtomicCarrierAccess {
            operation,
            place: assignment.target,
            operands,
            result: atomic.result,
        })
    }

    pub fn atomic_value_operands(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<Vec<ExpressionHandle>> {
        if let Some(once) = atomic_once_carrier(program, expression) {
            return Some(std::iter::once(once.place).chain(once.operands).collect());
        }
        if let Some(load) = atomic_load_carrier(program, expression) {
            return Some(vec![load.place]);
        }
        let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) else {
            return None;
        };
        atomic_write_operands(program, atomic).map(|(_, operands)| operands)
    }

    pub fn atomic_resident_type_reference(
        program: &dyn crate::CheckingInput,
        cell: typed_trees::types::TypeReferenceHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let cell = program.type_reference_table.peeled_type_reference(cell);
        if let Some((_, field)) = program.access_view_field_plan_for_type_reference(cell)
            && matches!(field.access, access_plans::FieldAccess::Atomic { .. })
        {
            return Some(field.value_type);
        }
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(cell)
        else {
            return None;
        };
        if !program.symbols.builtin_type_atom(*symbol)?.is_atomic() {
            return None;
        }
        let primitive = program.primitive_type_reference(cell)?;
        let mut residents = program
            .symbols
            .child_handles(program.symbols.root())?
            .filter_map(|symbol| {
                let atom = program.symbols.builtin_type_atom(symbol)?;
                if atom.is_atomic() {
                    return None;
                }
                let reference = program
                    .type_reference_table
                    .find_named_type_reference(symbol)?;
                (program.primitive_type_reference(reference) == Some(primitive))
                    .then_some(reference)
            });
        let resident = residents.next()?;
        residents.next().is_none().then_some(resident)
    }

    fn atomic_write_operands(
        program: &dyn crate::CheckingInput,
        atomic: &TableAtomicExpression,
    ) -> Option<(AtomicAccessOperation, Vec<ExpressionHandle>)> {
        if !atomic.result_custody.is_valid_for(atomic.ordering)
            || atomic.result_custody.has_structural_result()
        {
            return None;
        }
        let binary = |expression| match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => Some(*binary),
            _ => None,
        };
        let prior = |expression| same_result_name(program, expression, atomic.result);
        Some(match atomic.ordering {
            AtomicOrderingPlan::Load(_) | AtomicOrderingPlan::CompareExchangeOnce { .. } => {
                return None;
            }
            AtomicOrderingPlan::Store(ordering) => {
                if !ordering.valid_for_store() || atomic.result.is_valid() {
                    return None;
                }
                (AtomicAccessOperation::Store(ordering), vec![atomic.value])
            }
            AtomicOrderingPlan::Swap(ordering) => {
                if !atomic.result.is_valid() {
                    return None;
                }
                (AtomicAccessOperation::Swap(ordering), vec![atomic.value])
            }
            AtomicOrderingPlan::ReadModifyWrite(ordering) => {
                let model = binary(atomic.value)?;
                if !prior(model.left) {
                    return None;
                }
                let operation = match model.operator {
                    BinaryOperator::Add => AtomicAccessOperation::FetchAdd(ordering),
                    BinaryOperator::Subtract => AtomicAccessOperation::FetchSub(ordering),
                    BinaryOperator::BitwiseXor => AtomicAccessOperation::FetchXor(ordering),
                    BinaryOperator::BitwiseOr => AtomicAccessOperation::FetchOr(ordering),
                    BinaryOperator::BitwiseAnd => AtomicAccessOperation::FetchAnd(ordering),
                    _ => return None,
                };
                (operation, vec![model.right])
            }
            AtomicOrderingPlan::CompareExchange { success, failure } => {
                if !failure.valid_compare_exchange_failure(success) {
                    return None;
                }
                let sum = binary(atomic.value)?;
                let product = binary(sum.right)?;
                let equal = binary(product.left)?;
                let difference = binary(product.right)?;
                if sum.operator != BinaryOperator::Add
                    || product.operator != BinaryOperator::Multiply
                    || equal.operator != BinaryOperator::Equal
                    || difference.operator != BinaryOperator::Subtract
                    || !prior(sum.left)
                    || !prior(equal.left)
                    || !prior(difference.right)
                {
                    return None;
                }
                (
                    AtomicAccessOperation::CompareExchange { success, failure },
                    vec![equal.right, difference.left],
                )
            }
        })
    }

    fn same_result_name(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        result: ExpressionHandle,
    ) -> bool {
        let member = |expression: ExpressionHandle| {
            if !program.expression_table.expression_is_valid(expression) {
                return None;
            }
            let path = program.expression_table.name_path_node(expression)?;
            match program.expression_table.name_path_members(path.members) {
                [member] => Some(member.clone()),
                _ => None,
            }
        };
        matches!((member(expression), member(result)), (Some(left), Some(right)) if left == right)
    }
}

pub mod byte_observations {
    use typed_trees::types::PrimitiveType;

    #[derive(Clone)]
    pub enum ByteObservationCarrier {
        FixedArray { length: u64 },
        Sequence(checked_trees::CheckedByteSequenceCarrier),
    }

    pub fn comparable_byte_carrier_type(
        program: &dyn crate::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<ByteObservationCarrier> {
        let observed_type = match crate::reference_shell(program, type_reference) {
            Some(shell) if shell.access == language_semantics::ReferenceAccess::WriteOnly => {
                return None;
            }
            Some(shell) => shell.referee,
            None => type_reference,
        };
        let storage = program
            .type_reference_table
            .unconstrained_type_reference(observed_type);
        if let typed_trees::types::TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } = program.type_reference_table.type_reference(storage)
            && program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
        {
            return Some(ByteObservationCarrier::FixedArray {
                length: u64::try_from(*length).ok()?,
            });
        }
        crate::byte_sequence_carrier(program, type_reference, &[])
            .filter(|carrier| {
                matches!(
                    carrier,
                    checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                        | checked_trees::CheckedByteSequenceCarrier::BoundedView {
                            access: Some(
                                checked_trees::CheckedStructuralAccess::SharedBorrow
                                    | checked_trees::CheckedStructuralAccess::MutableBorrow
                            ),
                            ..
                        }
                        | checked_trees::CheckedByteSequenceCarrier::BorrowedView {
                            access: Some(
                                checked_trees::CheckedStructuralAccess::SharedBorrow
                                    | checked_trees::CheckedStructuralAccess::MutableBorrow
                            ),
                        }
                )
            })
            .map(ByteObservationCarrier::Sequence)
    }
}

pub mod claim_frontier {
    use language_semantics::Multiplicity;
    use symbols::SymbolHandle;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ClaimFrontierClaim {
        pub path: Vec<facts::PlaceSegment>,
        pub type_reference: TypeReferenceHandle,
        pub conditional: bool,
    }

    /// Share a result’s declared paths so field lookups do not copy the whole list.
    /// Keep their original order, types and conditional flags together.
    pub struct DeclaredClaimFrontier {
        claims: Vec<ClaimFrontierClaim>,
        paths: std::sync::OnceLock<facts::places::PlaceBucketIndex>,
    }

    impl DeclaredClaimFrontier {
        pub(crate) fn new(claims: Vec<ClaimFrontierClaim>) -> Self {
            Self {
                claims,
                paths: std::sync::OnceLock::new(),
            }
        }

        pub fn claims(&self) -> &[ClaimFrontierClaim] {
            &self.claims
        }

        pub fn positions<'query>(
            &'query self,
            path: &'query [facts::PlaceSegment],
        ) -> impl Iterator<Item = usize> + 'query {
            let rows = self.paths.get_or_init(|| {
                facts::places::PlaceBucketIndex::new(self.claims.iter().map(|claim| {
                    facts::places::place_bucket_hash(facts::PlaceRoot::Unknown, &claim.path)
                }))
            });
            rows.candidates(facts::places::place_bucket_hash(
                facts::PlaceRoot::Unknown,
                path,
            ))
            .filter(move |position| self.claims[*position].path == path)
        }
    }

    pub fn declared_claim_frontier(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> std::sync::Arc<DeclaredClaimFrontier> {
        crate::value_custody::type_properties::memoized_declared_claim_frontier(
            program,
            type_reference,
        )
    }

    pub fn owned_receiver_claims(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
    ) -> (Multiplicity, Vec<ClaimFrontierClaim>) {
        let Some(machine) = program.machine(machine_symbol) else {
            return (Multiplicity::Affine, Vec::new());
        };
        if machine.attached_data_application.is_valid() {
            return (
                program.type_multiplicity(machine.attached_data_application),
                linear_claim_frontier(program, machine.attached_data_application),
            );
        }
        let Some(definition) = program.data_definition_by_symbol(machine.attached_data_symbol)
        else {
            return (Multiplicity::Affine, Vec::new());
        };
        let multiplicity = definition.properties.multiplicity;
        if multiplicity == Multiplicity::Linear {
            return (
                multiplicity,
                vec![ClaimFrontierClaim {
                    path: Vec::new(),
                    type_reference: TypeReferenceHandle::invalid(),
                    conditional: false,
                }],
            );
        }
        let mut claims = Vec::new();
        append_data_linear_claim_frontier(
            program,
            definition,
            &[],
            &mut Vec::new(),
            &mut Vec::new(),
            &mut claims,
            false,
        );
        (multiplicity, claims)
    }

    pub fn linear_claim_frontier(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<ClaimFrontierClaim> {
        crate::value_custody::type_properties::memoized_claim_frontier(program, type_reference)
    }

    pub fn type_carries_linear_obligation(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        let mut claims = Vec::new();
        append_linear_claim_frontier(
            program,
            type_reference,
            &[],
            &mut Vec::new(),
            &mut Vec::new(),
            &mut claims,
            true,
        );
        !claims.is_empty()
    }

    pub(crate) fn linear_claim_frontier_uncached(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<ClaimFrontierClaim> {
        let mut claims = Vec::new();
        append_linear_claim_frontier(
            program,
            type_reference,
            &[],
            &mut Vec::new(),
            &mut Vec::new(),
            &mut claims,
            false,
        );
        claims
    }

    fn append_linear_claim_frontier(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        path: &mut Vec<facts::PlaceSegment>,
        visiting: &mut Vec<SymbolHandle>,
        claims: &mut Vec<ClaimFrontierClaim>,
        first_only: bool,
    ) {
        if !type_reference.is_valid() {
            return;
        }
        let multiplicity = type_multiplicity(program, type_reference, substitutions);
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                if multiplicity == Multiplicity::Linear {
                    push_claim(claims, path, type_reference);
                    return;
                }
                append_linear_claim_frontier(
                    program,
                    *base_type,
                    substitutions,
                    path,
                    visiting,
                    claims,
                    first_only,
                );
                return;
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } => {
                if *length == 0 {
                    return;
                }
                if first_only {
                    path.push(facts::PlaceSegment::FixedIndex { index: 0 });
                    append_linear_claim_frontier(
                        program,
                        *element_type,
                        substitutions,
                        path,
                        visiting,
                        claims,
                        true,
                    );
                    path.pop();
                    return;
                }
                let mut element_claims = Vec::new();
                append_linear_claim_frontier(
                    program,
                    *element_type,
                    substitutions,
                    &mut Vec::new(),
                    visiting,
                    &mut element_claims,
                    first_only,
                );
                let prefix_is_conditional = path
                    .iter()
                    .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }));
                for index in 0..*length {
                    for element_claim in &element_claims {
                        let mut claim_path =
                            Vec::with_capacity(path.len() + 1 + element_claim.path.len());
                        claim_path.extend_from_slice(path);
                        claim_path.push(facts::PlaceSegment::FixedIndex { index });
                        claim_path.extend_from_slice(&element_claim.path);
                        claims.push(ClaimFrontierClaim {
                            path: claim_path,
                            type_reference: element_claim.type_reference,
                            conditional: prefix_is_conditional || element_claim.conditional,
                        });
                    }
                }
                return;
            }
            TypeReferenceNode::Named { symbol, .. } => {
                if let Some((replacement, enclosing)) =
                    enclosing_substitution(substitutions, *symbol, type_reference)
                {
                    append_linear_claim_frontier(
                        program,
                        replacement,
                        enclosing,
                        path,
                        visiting,
                        claims,
                        first_only,
                    );
                    return;
                }
            }
            _ => {}
        }

        if multiplicity == Multiplicity::Linear {
            push_claim(claims, path, type_reference);
            return;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { .. } => unreachable!("handled before multiplicity"),
            TypeReferenceNode::Named { symbol, name } => {
                let Some(definition) = find_data_definition(program, *symbol, name.as_str()) else {
                    return;
                };
                append_data_linear_claim_frontier(
                    program,
                    definition,
                    substitutions,
                    path,
                    visiting,
                    claims,
                    first_only,
                );
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } => {
                if let Some(definition) = program.closed_data_definition(type_reference) {
                    append_data_linear_claim_frontier(
                        program,
                        definition,
                        substitutions,
                        path,
                        visiting,
                        claims,
                        first_only,
                    );
                    return;
                }
                let Some(definition) =
                    find_data_definition(program, *base_symbol, base_name.as_str())
                else {
                    return;
                };
                let mut instantiated = substitutions.to_vec();
                instantiated.extend(
                    program
                        .data_type_parameters(definition)
                        .iter()
                        .zip(
                            program
                                .type_reference_table
                                .type_reference_handles(*arguments),
                        )
                        .filter_map(|(parameter, argument)| {
                            matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                                .then_some((parameter.symbol, *argument))
                        }),
                );
                append_data_linear_claim_frontier(
                    program,
                    definition,
                    &instantiated,
                    path,
                    visiting,
                    claims,
                    first_only,
                );
            }
            TypeReferenceNode::FixedArray { .. } => {}
            TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Unit => {}
        }
    }

    fn append_data_linear_claim_frontier(
        program: &dyn crate::CheckingInput,
        definition: &typed_trees::data::DataDefinition,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        path: &mut Vec<facts::PlaceSegment>,
        visiting: &mut Vec<SymbolHandle>,
        claims: &mut Vec<ClaimFrontierClaim>,
        first_only: bool,
    ) {
        if visiting.contains(&definition.symbol) {
            return;
        }
        visiting.push(definition.symbol);
        for member in program.data_members(definition) {
            if first_only && !claims.is_empty() {
                break;
            }
            match member {
                typed_trees::data::DataMember::Field(field) => {
                    append_field_claim_frontier(
                        program,
                        field,
                        substitutions,
                        path,
                        visiting,
                        claims,
                        first_only,
                    );
                }
                typed_trees::data::DataMember::Variant(variant) => {
                    path.push(facts::PlaceSegment::Case {
                        variant: variant.symbol,
                    });
                    for field in program.data_payload_fields(variant) {
                        if first_only && !claims.is_empty() {
                            break;
                        }
                        append_field_claim_frontier(
                            program,
                            field,
                            substitutions,
                            path,
                            visiting,
                            claims,
                            first_only,
                        );
                    }
                    path.pop();
                }
            }
        }
        visiting.pop();
    }

    #[allow(clippy::too_many_arguments)]
    fn append_field_claim_frontier(
        program: &dyn crate::CheckingInput,
        field: &typed_trees::data::DataField,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        path: &mut Vec<facts::PlaceSegment>,
        visiting: &mut Vec<SymbolHandle>,
        claims: &mut Vec<ClaimFrontierClaim>,
        first_only: bool,
    ) {
        path.push(facts::PlaceSegment::Field {
            symbol: field.symbol,
        });
        append_linear_claim_frontier(
            program,
            field.type_reference,
            substitutions,
            path,
            visiting,
            claims,
            first_only,
        );
        path.pop();
    }

    fn push_claim(
        claims: &mut Vec<ClaimFrontierClaim>,
        path: &[facts::PlaceSegment],
        type_reference: TypeReferenceHandle,
    ) {
        claims.push(ClaimFrontierClaim {
            path: path.to_vec(),
            type_reference,
            conditional: path
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. })),
        });
    }

    fn type_multiplicity(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Multiplicity {
        if !type_reference.is_valid() {
            return Multiplicity::Affine;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                type_multiplicity(program, *base_type, substitutions)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                type_multiplicity(program, *element_type, substitutions)
            }
            TypeReferenceNode::Named { symbol, .. } => {
                match enclosing_substitution(substitutions, *symbol, type_reference) {
                    Some((replacement, enclosing)) => {
                        type_multiplicity(program, replacement, enclosing)
                    }
                    None => frontier_type_multiplicity(program, type_reference),
                }
            }
            _ => frontier_type_multiplicity(program, type_reference),
        }
    }

    fn enclosing_substitution(
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        symbol: SymbolHandle,
        occurrence: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, &[(SymbolHandle, TypeReferenceHandle)])> {
        substitutions
            .iter()
            .enumerate()
            .rev()
            .find_map(|(position, (parameter, replacement))| {
                (*parameter == symbol && *replacement != occurrence)
                    .then_some((*replacement, &substitutions[..position]))
            })
    }

    fn frontier_type_multiplicity(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Multiplicity {
        if !type_reference.is_valid() {
            return Multiplicity::Affine;
        }
        if let Some(storage) = program.layout_storage_arguments(type_reference) {
            return frontier_type_multiplicity(program, storage.semantic_type);
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => Multiplicity::Unrestricted,
            TypeReferenceNode::Constrained { base_type, .. } => {
                frontier_type_multiplicity(program, *base_type)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                frontier_type_multiplicity(program, *element_type)
            }
            TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Slice { .. } => {
                Multiplicity::Affine
            }
            TypeReferenceNode::Named { symbol, name } => {
                if symbol.is_valid()
                    && let Some(definition) = last_data_definition_by_symbol(program, *symbol)
                {
                    return definition.properties.multiplicity;
                }
                if symbol.is_valid()
                    && let Some(parameter) = program.data_type_parameter_by_symbol(*symbol)
                {
                    return parameter.bounds.multiplicity;
                }
                if typed_trees::types::PrimitiveType::from_name(name.as_str()).is_some() {
                    return Multiplicity::Unrestricted;
                }
                program
                    .data_definition_by_name_str(name.as_str())
                    .map(|definition| definition.properties.multiplicity)
                    .unwrap_or(Multiplicity::Affine)
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                ..
            } => {
                if base_symbol.is_valid()
                    && let Some(definition) = last_data_definition_by_symbol(program, *base_symbol)
                {
                    return definition.properties.multiplicity;
                }
                program
                    .data_definition_by_name_str(base_name.as_str())
                    .map(|definition| definition.properties.multiplicity)
                    .unwrap_or(Multiplicity::Affine)
            }
        }
    }

    fn find_data_definition<'program>(
        program: &'program dyn crate::CheckingInput,
        symbol: SymbolHandle,
        name: &str,
    ) -> Option<&'program typed_trees::data::DataDefinition> {
        if symbol.is_valid() {
            return last_data_definition_by_symbol(program, symbol);
        }
        program.data_definition_by_name_str(name)
    }

    fn last_data_definition_by_symbol(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        let position = program.with_data_definition_index(|index| {
            index.positions_for_symbol(symbol).last().copied()
        })?;
        program.data_definitions().get(position as usize)
    }

    #[cfg(test)]
    mod declared_path_tests {
        use super::{ClaimFrontierClaim, DeclaredClaimFrontier};
        use facts::{PlaceRoot, PlaceSegment, places::PlaceBucketIndex};
        use typed_trees::types::TypeReferenceHandle;

        #[test]
        fn declared_path_rows_preserve_duplicates_metadata_and_original_positions() {
            let rows = (0..1024)
                .map(|position| ClaimFrontierClaim {
                    path: vec![
                        PlaceSegment::Field {
                            symbol: symbols::SymbolHandle::from_parts(
                                (position % 31 + 1) as u32,
                                (position % 2 + 1) as u32,
                            ),
                        },
                        PlaceSegment::FixedIndex {
                            index: position % 7,
                        },
                    ],
                    type_reference: TypeReferenceHandle::from_parts(position as u32 + 1, 1),
                    conditional: position % 3 == 0,
                })
                .collect::<Vec<_>>();
            let start = std::time::Instant::now();
            let declared = DeclaredClaimFrontier::new(rows.clone());
            declared.positions(&rows[0].path).for_each(drop);
            let preparation_ns = start.elapsed().as_nanos();
            let mut direct_rows = 0;
            let mut prepared_rows = 0;
            for row in &rows {
                let direct = rows
                    .iter()
                    .enumerate()
                    .filter_map(|(position, other)| {
                        direct_rows += 1;
                        (other.path == row.path).then_some(position)
                    })
                    .collect::<Vec<_>>();
                let prepared = declared.positions(&row.path).collect::<Vec<_>>();
                prepared_rows += declared
                    .paths
                    .get()
                    .unwrap()
                    .candidates(facts::places::place_bucket_hash(
                        PlaceRoot::Unknown,
                        &row.path,
                    ))
                    .count();
                assert_eq!(prepared, direct);
            }
            assert_eq!(declared.claims(), rows);
            assert!(
                declared
                    .positions(&[PlaceSegment::FixedIndex { index: 9000 }])
                    .next()
                    .is_none()
            );

            // Deliberately put every row in this query's bucket. Full path values,
            // not hashes, still decide membership, cardinality and first position.
            let path = rows[3].path.clone();
            let collision = DeclaredClaimFrontier::new(rows.clone());
            let query_hash = facts::places::place_bucket_hash(PlaceRoot::Unknown, &path);
            assert!(
                collision
                    .paths
                    .set(PlaceBucketIndex::new(rows.iter().map(|_| query_hash)))
                    .is_ok()
            );
            assert_eq!(
                collision.positions(&path).collect::<Vec<_>>(),
                declared.positions(&path).collect::<Vec<_>>()
            );
            let mut changed = rows.clone();
            changed.remove(0);
            changed[0].conditional = !changed[0].conditional;
            let rebuilt = DeclaredClaimFrontier::new(changed.clone());
            assert_eq!(rebuilt.claims(), changed);
            assert_eq!(
                rebuilt.positions(&path).collect::<Vec<_>>(),
                changed
                    .iter()
                    .enumerate()
                    .filter_map(|(position, row)| (row.path == path).then_some(position))
                    .collect::<Vec<_>>()
            );
            eprintln!(
                "declared-path rows={} direct_rows={direct_rows} prepared_candidates={prepared_rows} preparation_ns={preparation_ns}",
                rows.len()
            );
        }
    }
}

pub mod cleanup {
    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId;
    use language_semantics::{
        MachineSupplyMode, Multiplicity, TerminationGuarantee, TerminationInterface,
    };
    use std::collections::BTreeSet;
    use std::sync::OnceLock;
    use symbols::SymbolHandle;
    use symbols::SymbolKind;
    use typed_trees::data::DataMember;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::machine::Machine;
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::statement::TransitionTargetNode;
    use typed_trees::type_identity::TypeIdentityRequest;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) struct CleanupTypeQueries {
        rows: Vec<(TypeReferenceHandle, CleanupTypeAnswers)>,
    }

    #[derive(Default)]
    pub(crate) struct CleanupTypeAnswers {
        plain_without_nominal_cleanup: OnceLock<bool>,
        no_code_cleanup_contents: OnceLock<bool>,
        no_owned_cleanup_obligations: OnceLock<bool>,
    }

    impl CleanupTypeQueries {
        fn of(program: &dyn crate::CheckingInput) -> Self {
            let mut rows: Vec<_> = program
                .type_reference_table
                .type_reference_rows()
                .map(|(handle, _)| (handle, CleanupTypeAnswers::default()))
                .collect();
            rows.sort_by_key(|(handle, _)| (handle.arena_index(), handle.generation()));
            Self { rows }
        }

        fn answers(&self, reference: TypeReferenceHandle) -> Option<&CleanupTypeAnswers> {
            self.rows
                .binary_search_by_key(
                    &(reference.arena_index(), reference.generation()),
                    |(handle, _)| (handle.arena_index(), handle.generation()),
                )
                .ok()
                .map(|position| &self.rows[position].1)
        }
    }

    fn cleanup_type_answers(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<&CleanupTypeAnswers> {
        program
            .checking_memos()?
            .cleanup_types
            .get_or_init(|| CleanupTypeQueries::of(program))
            .answers(reference)
    }

    pub fn has_plain_contents_without_nominal_cleanup(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        let compute = || {
            crate::value_custody::storage_contents::has_contents(
                program,
                reference,
                &[],
                crate::value_custody::storage_contents::ContentsRequirement::PlainOwned,
            ) && matches!(
                crate::value_custody::cleanup_selection::selected_application(program, reference),
                Ok(None)
            )
        };
        match cleanup_type_answers(program, reference) {
            Some(answers) => *answers.plain_without_nominal_cleanup.get_or_init(compute),
            None => compute(),
        }
    }

    pub fn has_no_code_cleanup_contents(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        let compute = || {
            crate::value_custody::storage_contents::has_contents(
                program,
                reference,
                &[],
                crate::value_custody::storage_contents::ContentsRequirement::DescriptorCleanup,
            )
        };
        match cleanup_type_answers(program, reference) {
            Some(answers) => *answers.no_code_cleanup_contents.get_or_init(compute),
            None => compute(),
        }
    }

    pub fn has_no_owned_cleanup_obligations(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        let compute = || {
            crate::reference_result_custody::result_multiplicity(program, reference)
                == Multiplicity::Unrestricted
                && has_plain_contents_without_nominal_cleanup(program, reference)
        };
        match cleanup_type_answers(program, reference) {
            Some(answers) => *answers.no_owned_cleanup_obligations.get_or_init(compute),
            None => compute(),
        }
    }

    pub fn type_graph_requires_nominal_drop(
        program: &dyn crate::CheckingInput,
        mut type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if crate::value_custody::cleanup_selection::selected_application(program, type_reference)
            .ok()
            .flatten()
            .is_some()
        {
            return true;
        }
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => {
                    return false;
                }
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Named { symbol, .. } => {
                    let Some(replacement) =
                        crate::queries::substitutions::actual_for(substitutions, *symbol)
                    else {
                        break;
                    };
                    if replacement == type_reference {
                        return false;
                    }
                    type_reference = replacement;
                }
                _ => break,
            }
        }

        let identity = program.type_identity_request_text(TypeIdentityRequest {
            substitutions,
            ..TypeIdentityRequest::ordinary(type_reference)
        });
        if !visited.insert(identity) {
            return false;
        }

        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => false,
            TypeReferenceNode::Constrained { base_type, .. } => {
                type_graph_requires_nominal_drop(program, *base_type, substitutions, visited)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                type_graph_requires_nominal_drop(program, *element_type, substitutions, visited)
            }
            TypeReferenceNode::Named { symbol, name } => program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == *symbol || data.name.as_str() == name.as_str())
                .is_some_and(|data| {
                    data_graph_requires_nominal_drop(program, data, substitutions, visited)
                }),
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let Some(data) = crate::declarations::data_queries::data_definition_by_symbol(
                    program,
                    *base_symbol,
                ) else {
                    return false;
                };
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                let parameters = program.data_type_parameters(data);
                if arguments.len() != parameters.len() {
                    return false;
                }
                let mut nested_substitutions = substitutions.to_vec();
                nested_substitutions.extend(
                    parameters
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| (parameter.symbol, *argument)),
                );
                data_graph_requires_nominal_drop(program, data, &nested_substitutions, visited)
            }
            TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => false,
        }
    }

    pub fn data_requires_nominal_drop(
        program: &dyn crate::CheckingInput,
        data: &typed_trees::data::DataDefinition,
    ) -> bool {
        data_graph_requires_nominal_drop(program, data, &[], &mut BTreeSet::new())
    }

    fn data_graph_requires_nominal_drop(
        program: &dyn crate::CheckingInput,
        data: &typed_trees::data::DataDefinition,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if data.cleanup_selection.is_some() {
            return true;
        }
        program
            .data_members(data)
            .iter()
            .any(|member| match member {
                DataMember::Field(field) => {
                    !field.relevance.is_erased()
                        && type_graph_requires_nominal_drop(
                            program,
                            field.type_reference,
                            substitutions,
                            visited,
                        )
                }
                DataMember::Variant(variant) => {
                    program.data_payload_fields(variant).iter().any(|field| {
                        !field.relevance.is_erased()
                            && type_graph_requires_nominal_drop(
                                program,
                                field.type_reference,
                                substitutions,
                                visited,
                            )
                    })
                }
            })
    }

    pub(crate) fn validate_cleanup_machine_declarations(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        crate::value_custody::cleanup_selection::validate_declaration_owners(program, diagnostics);
        let references =
            typed_trees::references::collect_program_references(program.typed_program());
        let mut references = references.type_references.into_iter().collect::<Vec<_>>();
        references
            .sort_unstable_by_key(|reference| (reference.arena_index(), reference.generation()));
        for reference in references {
            if let Err(diagnostic) =
                crate::value_custody::cleanup_selection::selected_application(program, reference)
            {
                diagnostics.push(diagnostic);
            }
        }
        for machine in program.machines() {
            if retired_attached_cleanup(program, machine) {
                let owner = machine.attached_data.as_deref().unwrap_or("Data");
                diagnostics.push(Diagnostic::error(format!(
                "`machine {}` attaches cleanup to data `{owner}`, which is retired: data owns no cleanup machines; \
                 declare an independent static machine taking `&mut {owner}` and select it with \
                 `data {owner} dropped by <machine>`",
                machine.name
            )));
            } else if is_reserved_cleanup(program, machine) {
                validate_cleanup_machine(program, machine, diagnostics);
            }
        }
    }

    pub(crate) fn collect_reserved_cleanup_selection_diagnostics(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let cleanups = program
            .machines()
            .iter()
            .filter(|machine| is_reserved_cleanup(program, machine))
            .collect::<Vec<_>>();
        if cleanups.is_empty() {
            return;
        }
        let internal_transitions = internal_cleanup_transitions(program, &cleanups);
        for selection in program.authored_declaration_selections() {
            if selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::DomainCleanupSelection {
            continue;
        }
            let selection_target = selection.target();
            let Some(target) = selection_target.as_resolved() else {
                continue;
            };
            let Some(cleanup) =
                reserved_cleanup_selected_by(program, &cleanups, target.selected_symbol())
            else {
                continue;
            };
            if internal_transitions.contains(&selection.occurrence_id()) {
                continue;
            }
            diagnostics.push(
            Diagnostic::error(format!(
                "reserved cleanup machine `{}` is compiler-selected and cannot be selected by source; consume the value with `omega::core::drop(value)` or call an ordinary owner-published protocol operation",
                cleanup.name,
            ))
            .with_source_span(selection.source_span()),
        );
        }
    }

    fn internal_cleanup_transitions(
        program: &dyn crate::CheckingInput,
        cleanups: &[&Machine],
    ) -> BTreeSet<AuthoredDeclarationSelectionOccurrenceId> {
        let mut transitions = BTreeSet::new();
        for machine in cleanups {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    for target in transition.targets() {
                        let TransitionTargetNode::Named {
                            path,
                            authored_call_selection: Some(occurrence),
                            ..
                        } = program.statement_table.transition_target(target)
                        else {
                            continue;
                        };
                        if !path.symbol.is_valid() {
                            continue;
                        }
                        let selected = program.symbols.get(path.symbol);
                        if selected.kind == SymbolKind::State && selected.parent == machine.symbol {
                            transitions.insert(*occurrence);
                        }
                    }
                }
            }
        }
        transitions
    }

    pub fn validate_reserved_cleanup_selections(
        program: &dyn crate::CheckingInput,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        collect_reserved_cleanup_selection_diagnostics(program, &mut diagnostics);
        crate::program_validation::finish_strict_diagnostics(diagnostics)
    }

    pub(crate) fn is_reserved_cleanup(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
    ) -> bool {
        crate::value_custody::cleanup_selection::is_selected_machine(program, machine.symbol)
    }

    fn retired_attached_cleanup(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
        machine.attached_data.is_some()
            && machine.attached_data_symbol.is_valid()
            && machine.name.as_str().rsplit("::").next() == Some("drop")
            && crate::declarations::data_queries::data_definition_by_symbol(
                program,
                machine.attached_data_symbol,
            )
            .is_some()
    }

    pub fn nominal_cleanup_owner(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
            if let Ok(owners) = memos.nominal_cleanup_owners.lock()
                && let Some(owner) = owners.get(&symbol).copied()
            {
                return owner;
            }
            let owner = nominal_cleanup_owner_walk(program, symbol);
            if let Ok(mut owners) = memos.nominal_cleanup_owners.lock() {
                owners.insert(symbol, owner);
            }
            return owner;
        }
        nominal_cleanup_owner_walk(program, symbol)
    }

    pub fn nominal_cleanup_target(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<(SymbolHandle, Option<TypeReferenceHandle>)> {
        let owner = nominal_cleanup_owner(program, symbol)?;
        Some((
            owner,
            crate::value_custody::cleanup_selection::subject_for_executable(program, symbol),
        ))
    }

    fn nominal_cleanup_owner_walk(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        let machine = program.machine(symbol)?;
        let mut declaration = machine;
        let mut visited = Vec::new();
        while !is_reserved_cleanup(program, declaration) {
            if visited.contains(&declaration.symbol) {
                return None;
            }
            visited.push(declaration.symbol);
            let template = program.machine_specialization(declaration.symbol)?.template;
            declaration = program.machine(template)?;
        }
        crate::value_custody::cleanup_selection::owner_for_machine(program, declaration.symbol)
    }

    pub fn nominal_cleanup_machine_for_type<'program>(
        program: &'program dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<&'program Machine> {
        let selection =
            crate::value_custody::cleanup_selection::selected_application(program, reference)
                .ok()
                .flatten()?;
        if !selection.executable.is_valid()
            && let Some(instance) = crate::value_custody::cleanup_selection::closed_instance(
                program,
                selection,
                crate::value_custody::type_references::substituted_identity(
                    program,
                    crate::value_custody::places::peel_declared_constraints(program, reference),
                    substitutions,
                ),
            )
        {
            return Some(instance);
        }
        crate::value_custody::cleanup_selection::machine_for_selection(program, selection)
    }

    pub fn data_cleanup_machine(
        program: &dyn crate::CheckingInput,
        data: SymbolHandle,
    ) -> Option<&Machine> {
        let selection = program
            .unique_data_definition_by_symbol(data)?
            .cleanup_selection
            .as_ref()?;
        crate::value_custody::cleanup_selection::machine_for_selection(program, selection)
    }

    fn reserved_cleanup_selected_by<'program>(
        program: &dyn crate::CheckingInput,
        cleanups: &[&'program Machine],
        selected_symbol: SymbolHandle,
    ) -> Option<&'program Machine> {
        let machine_symbol = match program.symbols.get(selected_symbol).kind {
            SymbolKind::Machine => Some(selected_symbol),
            SymbolKind::State => Some(program.symbols.get(selected_symbol).parent),
            _ => None,
        };
        cleanups.iter().copied().find(|machine| {
            if let Some(machine_symbol) = machine_symbol {
                machine.symbol == machine_symbol
            } else {
                program.machine_or_state_symbol(machine, selected_symbol)
            }
        })
    }

    fn validate_cleanup_machine(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(entry) = program.machine_states(machine).first() else {
            diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` has no entry state; reserved cleanup takes one exclusive loan of its own data and returns Unit",
            machine.name
        )));
            return;
        };
        if entry.return_type.is_valid()
            && !matches!(
                program
                    .type_reference_table
                    .type_reference(entry.return_type),
                TypeReferenceNode::Unit
            )
        {
            diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` must return Unit; fallible cleanup is an explicit consuming machine",
            machine.name
        )));
        }
        if machine.suspends || machine.blocks {
            diagnostics.push(Diagnostic::error(format!(
                "cleanup machine `{}` must be non-suspending and nonblocking",
                machine.name
            )));
        }
        if program
            .machine_contracts(machine)
            .iter()
            .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
        {
            diagnostics.push(Diagnostic::error(format!(
                "cleanup machine `{}` may not declare a crash outcome",
                machine.name
            )));
        }
        if machine.supply_mode != MachineSupplyMode::CheckedBody
            && !matches!(
                machine.termination_plan.interface,
                TerminationInterface::Published(TerminationGuarantee::Terminates { .. })
            )
        {
            diagnostics.push(Diagnostic::error(format!(
                "bodyless cleanup machine `{}` must publish `terminates;`",
                machine.name
            )));
        }
    }

    pub(crate) fn validate_cleanup_delivery(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &typed_trees::state::State,
        mut value: typed_trees::expression::ExpressionHandle,
        target: TypeReferenceHandle,
        owner: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        while let ExpressionNode::Cast(cast) = expression_table(program).expression(value) {
            value = cast.value;
        }
        let Some(source_type) =
            crate::value_custody::expression_types::expression_result_type_reference(
                program, machine, state, value,
            )
        else {
            return;
        };
        let Ok(Some(source)) =
            crate::value_custody::cleanup_selection::selected_application(program, source_type)
        else {
            return;
        };
        let target = crate::reference_shell(program, target).map_or(target, |shell| shell.referee);
        if program.normalized_type_identity(source_type) == program.normalized_type_identity(target)
        {
            return;
        }
        match crate::value_custody::cleanup_selection::selected_application(program, target) {
        Ok(Some(target)) if crate::value_custody::cleanup_selection::same_application(program, source, target) => {},
        Err(diagnostic) => diagnostics.push(diagnostic),
        _ => diagnostics.push(Diagnostic::error(format!(
            "owned delivery in {owner} cannot erase or replace its retained dropped by cleanup application",
        ))),
    }
    }
}

pub mod cleanup_selection {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use crate::value_custody::type_references::substituted_identity;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::domain::CleanupSelection;
    use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
    use typed_trees::type_identity::{
        IndexIdentityArgument, TypeIdentityId, normalized_index_identity,
    };
    use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

    fn declarations(program: &dyn CheckingInput) -> impl Iterator<Item = &CleanupSelection> {
        program.cleanup_selections()
    }

    fn target(program: &dyn CheckingInput, selection: &CleanupSelection) -> SymbolHandle {
        expression_table(program)
            .static_machine_application(selection.application)
            .registration_operation
    }

    pub(crate) fn is_selected_machine(program: &dyn CheckingInput, machine: SymbolHandle) -> bool {
        declarations(program).any(|selection| {
            target(program, selection) == machine || selection.executable == machine
        })
    }

    pub(crate) fn machine_for_selection<'program>(
        program: &'program dyn CheckingInput,
        selection: &CleanupSelection,
    ) -> Option<&'program typed_trees::machine::Machine> {
        program.machine(if selection.executable.is_valid() {
            selection.executable
        } else {
            target(program, selection)
        })
    }

    pub(crate) fn closed_instance<'program>(
        program: &'program dyn CheckingInput,
        open: &CleanupSelection,
        subject: typed_trees::type_identity::NormalizedTypeIdentity,
    ) -> Option<&'program typed_trees::machine::Machine> {
        let template = target(program, open);
        let mut found: Option<&typed_trees::machine::Machine> = None;
        for selection in declarations(program).filter(|selection| {
            selection.executable.is_valid() && target(program, selection) == template
        }) {
            let Some(machine) = program.machine(selection.executable) else {
                continue;
            };
            let Some(receiver) = program.machine_states(machine).first().and_then(|entry| {
                match program.state_parameters(entry) {
                    [receiver] => crate::reference_shell(program, receiver.type_reference),
                    _ => None,
                }
            }) else {
                continue;
            };
            let receiver =
                crate::value_custody::places::peel_declared_constraints(program, receiver.referee);
            if substituted_identity(program, receiver, &[]) != subject {
                continue;
            }
            if found.is_some_and(|previous| previous.symbol != machine.symbol) {
                return None;
            }
            found = Some(machine);
        }
        found
    }

    pub(crate) fn selected_application(
        program: &dyn CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Result<Option<&CleanupSelection>, Diagnostic> {
        let mut selected = None;
        visit_type(program, reference, &mut Vec::new(), &mut selected)?;
        Ok(selected)
    }

    fn merge<'program>(
        program: &dyn CheckingInput,
        selection: &'program CleanupSelection,
        selected: &mut Option<&'program CleanupSelection>,
    ) -> Result<(), Diagnostic> {
        if machine_for_selection(program, selection)
            .is_none_or(|machine| machine.attached_data_symbol.is_valid())
        {
            return Err(Diagnostic::error(
                "dropped by must resolve one independent static machine",
            ));
        }
        if let Some(previous) = *selected {
            if !same_application(program, previous, selection) {
                return Err(Diagnostic::error(
                    "conflicting dropped by machine applications on one owned subject",
                ));
            }
        } else {
            *selected = Some(selection);
        }
        Ok(())
    }

    fn binding_identity(
        program: &dyn CheckingInput,
        binding: &StaticMachineBinding,
    ) -> Option<String> {
        let argument = match &binding.value {
            StaticMachineBindingValue::Type(reference)
            | StaticMachineBindingValue::Const(reference) => {
                IndexIdentityArgument::TypeArgument(*reference)
            }
            StaticMachineBindingValue::Machine(argument)
            | StaticMachineBindingValue::Conformance(argument) => {
                IndexIdentityArgument::StaticArgument(argument)
            }
            StaticMachineBindingValue::RuntimeValue { .. } => return None,
        };
        normalized_index_identity(program, argument, &[])
    }

    fn same_binding_value(
        program: &dyn CheckingInput,
        left: &StaticMachineBinding,
        right: &StaticMachineBinding,
    ) -> bool {
        if let (StaticMachineBindingValue::Type(left), StaticMachineBindingValue::Type(right)) =
            (&left.value, &right.value)
        {
            return program.normalized_type_identity(*left)
                == program.normalized_type_identity(*right);
        }
        binding_identity(program, left)
            .zip(binding_identity(program, right))
            .is_some_and(|(left, right)| left == right)
    }

    pub(crate) fn same_application(
        program: &dyn CheckingInput,
        left: &CleanupSelection,
        right: &CleanupSelection,
    ) -> bool {
        if left.application == right.application {
            return true;
        }
        let left_application =
            expression_table(program).static_machine_application(left.application);
        let right_application =
            expression_table(program).static_machine_application(right.application);
        if left_application.registration_operation != right_application.registration_operation {
            return false;
        }
        let left_bindings =
            expression_table(program).static_machine_application_bindings(left.application);
        let right_bindings =
            expression_table(program).static_machine_application_bindings(right.application);
        if left_bindings.is_empty()
            && right_bindings.is_empty()
            && (!expression_table(program)
                .static_machine_application_arguments(left.application)
                .is_empty()
                || !expression_table(program)
                    .static_machine_application_arguments(right.application)
                    .is_empty())
        {
            return left.application == right.application;
        }
        if left_bindings.len() != right_bindings.len()
            || !left_bindings
                .iter()
                .zip(right_bindings)
                .all(|(left, right)| {
                    left.parameter == right.parameter
                        && (left.value == right.value || same_binding_value(program, left, right))
                })
        {
            return false;
        }
        true
    }

    fn visit_type<'program>(
        program: &'program dyn CheckingInput,
        reference: TypeReferenceHandle,
        domains: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
        selected: &mut Option<&'program CleanupSelection>,
    ) -> Result<(), Diagnostic> {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
                ..
            } => {
                visit_type(program, *base_type, domains, selected)?;
                for constraint in program.type_reference_table.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        visit_domain(
                            program,
                            domain.symbol,
                            Some(domain.semantic_id),
                            domains,
                            selected,
                        )?;
                    }
                }
            }
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                ..
            } => {
                if let Some(data) = program
                    .closed_data_definition(reference)
                    .or_else(|| program.unique_data_definition_by_symbol(*symbol))
                    && let Some(selection) = &data.cleanup_selection
                {
                    merge(program, selection, selected)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn visit_domain<'program>(
        program: &'program dyn CheckingInput,
        symbol: SymbolHandle,
        identity: Option<language_semantics::SemanticDomainId>,
        visited: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
        selected: &mut Option<&'program CleanupSelection>,
    ) -> Result<(), Diagnostic> {
        let declared = program.domain_definition(symbol);
        let supplied = match identity {
            Some(identity) => program.domain_definition_for_instance(symbol, identity),
            None => declared.filter(|domain| {
                typed_trees::domain::index_parameters(program, domain).is_empty()
                    && !typed_trees::domain::has_generic_carrier(program, domain)
            }),
        };
        let Some(domain) = supplied.or(declared) else {
            return Ok(());
        };
        let instance = (symbol, identity.unwrap_or(domain.semantic_id));
        if visited.contains(&instance) {
            return Ok(());
        }
        visited.push(instance);
        if let Some(selection) = &domain.cleanup_selection {
            if supplied.is_none() {
                return Err(Diagnostic::error(format!(
                    "owned cleanup requires the exact retained domain application for `{}` (requested {:?}, declaration {:?})",
                    domain.name, identity, domain.semantic_id,
                )));
            }
            merge(program, selection, selected)?;
        }
        if let Some(alias) = &domain.alias {
            for constituent in &alias.constituents {
                visit_domain(program, constituent.domain_symbol, None, visited, selected)?;
            }
        }
        for membership in typed_trees::domain::subject_membership_facts(program, domain) {
            visit_domain(
                program,
                membership.domain_symbol,
                Some(membership.semantic_domain),
                visited,
                selected,
            )?;
        }
        Ok(())
    }

    pub(crate) fn owner_for_machine(
        program: &dyn CheckingInput,
        machine: SymbolHandle,
    ) -> Option<SymbolHandle> {
        let mut owner: Option<(SymbolHandle, TypeIdentityId, bool)> = None;
        for selection in declarations(program).filter(|selection| {
            selection.executable == machine || target(program, selection) == machine
        }) {
            let identity = program.normalized_type_identity(selection.subject);
            let (data, exact) = match program.closed_data_definition(selection.subject) {
                Some(data) => (data.symbol, true),
                None => (
                    program
                        .unique_data_definition_by_symbol(
                            expression_table(program)
                                .static_machine_application(selection.application)
                                .lexical_owner,
                        )?
                        .symbol,
                    false,
                ),
            };
            owner = match owner {
                None => Some((data, identity, exact)),
                Some((previous, previous_identity, previous_exact))
                    if previous == data && previous_identity == identity =>
                {
                    Some((data, identity, previous_exact || exact))
                }
                Some((previous, previous_identity, previous_exact))
                    if previous_identity == identity && previous_exact != exact =>
                {
                    Some(if exact {
                        (data, identity, true)
                    } else {
                        (previous, identity, true)
                    })
                }
                Some(_) => return None,
            };
        }
        owner.map(|(symbol, ..)| symbol)
    }

    pub(crate) fn subject_for_executable(
        program: &dyn CheckingInput,
        machine: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        let mut subject = None;
        for selection in declarations(program).filter(|selection| selection.executable == machine) {
            match subject {
                None => subject = Some(selection.subject),
                Some(previous)
                    if program.normalized_type_identity(previous)
                        == program.normalized_type_identity(selection.subject) => {}
                Some(_) => return None,
            }
        }
        subject
    }

    pub(crate) fn validate_declaration_owners(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for domain in program.domain_definitions() {
            if domain.cleanup_selection.is_none() {
                continue;
            }
            let reference = crate::peel_declared_constraints(program, domain.target_type);
            let owner = match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => *symbol,
                TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                _ => SymbolHandle::invalid(),
            };
            if !owner.is_valid()
                || !matches!(
                    program.symbols.get(owner).kind,
                    symbols::SymbolKind::Data | symbols::SymbolKind::BuiltinType
                )
                || program.symbols.symbol_package_identity(owner)
                    != program.symbols.symbol_package_identity(domain.symbol)
            {
                diagnostics.push(Diagnostic::error(format!("domain `{}` cannot select dropped by cleanup: only the carrier's owning package may authorize the selection", domain.name)));
            }
        }
        for selection in declarations(program) {
            let application =
                expression_table(program).static_machine_application(selection.application);
            let Some(machine) = program.machine(application.registration_operation) else {
                continue;
            };
            let shape = program.machine_states(machine).first().and_then(|entry| {
                let [parameter] = program.state_parameters(entry) else {
                    return None;
                };
                let shell = crate::reference_shell(program, parameter.type_reference)?;
                if parameter.is_const
                    || shell.access != language_semantics::ReferenceAccess::Mutable
                {
                    return None;
                }
                let substitutions = expression_table(program)
                    .static_machine_application_bindings(selection.application)
                    .iter()
                    .filter_map(|binding| match binding.value {
                        StaticMachineBindingValue::Type(actual)
                        | StaticMachineBindingValue::Const(actual) => {
                            Some((binding.parameter, actual))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let actual = substituted_identity(program, shell.referee, &substitutions);
                Some(
                    actual == program.normalized_type_identity(selection.subject)
                        && receiver_regions_match(program, selection, shell.referee),
                )
            });
            if shape != Some(true) {
                let mut diagnostic = Diagnostic::error(format!(
                    "cleanup application `{}` receiver does not match its selected owned subject",
                    machine.name
                ));
                if let Some(occurrence) = selection.authorization
                    && let Some(authored) =
                        program.authored_declaration_selections().get(occurrence)
                {
                    diagnostic = diagnostic.with_source_span(authored.source_span());
                }
                diagnostics.push(diagnostic);
            }
        }
    }

    fn receiver_regions_match(
        program: &dyn CheckingInput,
        selection: &CleanupSelection,
        receiver: TypeReferenceHandle,
    ) -> bool {
        let bindings =
            expression_table(program).static_machine_application_lifetimes(selection.application);
        if bindings.is_empty() {
            return true;
        }
        let subject = region_arguments(program, selection.subject)
            .filter(|regions| !regions.is_empty())
            .or_else(|| {
                program
                    .closed_data_definition(selection.subject)
                    .map(|data| data.lifetime_parameters.as_slice())
            });
        let (Some(receiver), Some(subject)) = (region_arguments(program, receiver), subject) else {
            return true;
        };
        receiver.len() == subject.len()
            && receiver.iter().zip(subject).all(|(authored, owned)| {
                bindings
                    .iter()
                    .find(|binding| binding.parameter.as_str() == authored.as_str())
                    .is_some_and(|binding| binding.argument.as_str() == owned.as_str())
            })
    }

    fn region_arguments(
        program: &dyn CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<&[typed_trees::name::Identifier]> {
        loop {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Generic {
                    lifetime_arguments, ..
                } => return Some(lifetime_arguments),
                _ => return None,
            }
        }
    }
}

pub mod const_expressions {
    use crate::frozen_program::expression_table;
    use symbols::SymbolKind;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    pub(crate) fn validate_formation(
        program: &dyn crate::CheckingInput,
        caller: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        expected: typed_trees::types::TypeReferenceHandle,
        owner: &str,
        diagnostics: &mut Vec<diagnostics::Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        if let Some((elements, element_type)) =
            array_literal_elements(program, expression, expected)
        {
            for element in elements {
                validate_formation(
                    program,
                    caller,
                    state,
                    element,
                    element_type,
                    owner,
                    diagnostics,
                    formation_obligations,
                );
            }
            return;
        }
        let environment = crate::proof_contracts::arithmetic_domains::requires_value_environment(
            program, caller, state,
        );
        crate::proof_contracts::arithmetic_domains::validate_value_range_retaining_obligations(
            program,
            caller,
            Some(state),
            expression,
            &environment,
            program.primitive_type_reference(expected),
            program.arithmetic_domain_for_type_reference(expected),
            owner,
            diagnostics,
            formation_obligations,
        );
        let anonymous_integer = program
            .primitive_type_reference(expected)
            .is_some_and(|primitive| primitive.accepts_integer_literal())
            && typed_trees::const_integer::is_integer_expression(
                expression_table(program),
                expression,
                &|symbol, _| {
                    program.data_type_parameter_by_symbol(symbol).is_some_and(|binder| {
                    matches!(binder.kind, TypeParameterKind::Const { type_reference } | TypeParameterKind::Value { type_reference }
                        if crate::value_custody::type_references::type_references_match(program, type_reference, expected))
                })
                },
            );
        if !anonymous_integer
            && !crate::expression_result_type_reference(program, caller, state, expression)
                .is_some_and(|actual| {
                    crate::value_custody::type_references::type_references_match(
                        program, actual, expected,
                    )
                })
        {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "{owner} has no selected result with its declared carrier"
            )));
        }
        if let Err(mut failures) =
            crate::value_custody::struct_literals::validate_value_construction(
                program,
                caller,
                state,
                expression,
                formation_obligations,
            )
        {
            diagnostics.append(&mut failures);
        }
    }

    pub(crate) fn validate_argument_recipes(
        program: &dyn crate::CheckingInput,
        caller: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[typed_trees::data::TypeParameter],
        arguments: &[typed_trees::expression::StaticMachineArgument],
        diagnostics: &mut Vec<diagnostics::Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        if parameters.len() != arguments.len() {
            return;
        }
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if let TypeParameterKind::Const {
                type_reference: expected,
            }
            | TypeParameterKind::Value {
                type_reference: expected,
            } = parameter.kind
                && argument.type_reference.is_valid()
                && let typed_trees::types::TypeReferenceNode::ConstExpression(expression) = program
                    .type_reference_table
                    .type_reference(argument.type_reference)
            {
                if !has_carrier(program, *expression, &parameter.kind, caller.symbol) {
                    if matches!(parameter.kind, TypeParameterKind::Const { .. })
                        && crate::value_custody::type_references::exact_const_carrier_is_eligible(
                            program, expected,
                        )
                    {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "const argument `{}` has no static value with its exact declared carrier",
                        parameter.name,
                    )));
                    }
                    continue;
                }
                validate_formation(
                    program,
                    caller,
                    state,
                    *expression,
                    expected,
                    &format!(
                        "machine `{}` const argument `{}`",
                        caller.name, parameter.name
                    ),
                    diagnostics,
                    formation_obligations,
                );
            }
            if let Some(application) = &argument.application
                && let Some(declaration) = program.conformance_by_symbol(argument.symbol)
            {
                validate_argument_recipes(
                    program,
                    caller,
                    state,
                    program.conformance_type_parameters(declaration),
                    &application.arguments,
                    diagnostics,
                    formation_obligations,
                );
            }
        }
    }

    pub(crate) fn has_carrier(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        binding_kind: &TypeParameterKind,
        lexical_owner: symbols::SymbolHandle,
    ) -> bool {
        let (TypeParameterKind::Const {
            type_reference: expected,
        }
        | TypeParameterKind::Value {
            type_reference: expected,
        }) = *binding_kind
        else {
            return false;
        };
        let runtime_value = matches!(binding_kind, TypeParameterKind::Value { .. });
        if program
            .primitive_type_reference(expected)
            .is_some_and(|primitive| primitive.accepts_integer_literal())
            && typed_trees::const_integer::is_integer_expression(
                expression_table(program),
                expression,
                &|symbol, _| {
                    program.data_type_parameter_by_symbol(symbol).is_some_and(|binder| {
                    matches!(binder.kind, TypeParameterKind::Const { type_reference }
                        if crate::value_custody::type_references::type_references_match(program, type_reference, expected))
                        || (runtime_value
                            || program.typed_program().finite_value_binder_in_scope(lexical_owner, symbol))
                            && matches!(binder.kind, TypeParameterKind::Value { type_reference }
                        if crate::value_custody::type_references::type_references_match(program, type_reference, expected))
                })
                },
            )
        {
            return true;
        }
        if !crate::value_custody::type_references::exact_const_carrier_is_eligible(
            program, expected,
        ) || !has_value_category(program, expression, runtime_value, lexical_owner)
        {
            return false;
        }
        if matches!(
            expression_table(program).expression(expression),
            ExpressionNode::ArrayLiteral(_)
        ) {
            let Some((elements, element_type)) =
                array_literal_elements(program, expression, expected)
            else {
                return false;
            };
            let element_kind = match binding_kind {
                TypeParameterKind::Value { .. } => TypeParameterKind::Value {
                    type_reference: element_type,
                },
                _ => TypeParameterKind::Const {
                    type_reference: element_type,
                },
            };
            return elements
                .iter()
                .all(|element| has_carrier(program, *element, &element_kind, lexical_owner));
        }
        let actual = match expression_table(program).expression(expression) {
            ExpressionNode::StructLiteral(literal) => {
                let actual = expression_table(program).constructed_value_type_reference(expression);
                actual.is_valid().then_some(actual).or_else(|| {
                    program
                        .type_reference_table
                        .find_named_type_reference(literal.type_symbol)
                })
            }
            _ => crate::parameter_expression_result_type_reference(
                program,
                lexical_owner,
                &[],
                expression,
            ),
        };
        actual.is_some_and(|actual| {
            crate::value_custody::type_references::type_references_match(program, actual, expected)
        })
    }

    fn array_literal_elements(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        expected: typed_trees::types::TypeReferenceHandle,
    ) -> Option<(
        Vec<ExpressionHandle>,
        typed_trees::types::TypeReferenceHandle,
    )> {
        let ExpressionNode::ArrayLiteral(elements) =
            expression_table(program).expression(expression)
        else {
            return None;
        };
        let (element_type, length) = fixed_array_geometry(program, expected)?;
        let elements = expression_table(program).expression_handles(*elements);
        (elements.len() == length).then(|| (elements.to_vec(), element_type))
    }

    fn has_value_category(
        program: &dyn crate::CheckingInput,
        root: ExpressionHandle,
        runtime_value: bool,
        lexical_owner: symbols::SymbolHandle,
    ) -> bool {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(expression) = pending.pop() {
            if !expression_table(program).expression_is_valid(expression) {
                return false;
            }
            if visited.contains(&expression) {
                continue;
            }
            visited.push(expression);
            let node = expression_table(program).expression(expression);
            match node {
                ExpressionNode::Name(path) => {
                    let immutable = if path.symbol.is_valid() {
                        match program.symbols.get(path.symbol).kind {
                            SymbolKind::TypeParameter => program
                                .data_type_parameter_by_symbol(path.symbol)
                                .is_some_and(|parameter| {
                                    matches!(parameter.kind, TypeParameterKind::Const { .. })
                                        || (runtime_value
                                            || program
                                                .typed_program()
                                                .finite_value_binder_in_scope(
                                                    lexical_owner,
                                                    path.symbol,
                                                ))
                                            && matches!(
                                                parameter.kind,
                                                TypeParameterKind::Value { .. }
                                            )
                                }),
                            SymbolKind::Const => program
                                .const_declarations()
                                .iter()
                                .find(|declaration| declaration.symbol == path.symbol)
                                .is_some_and(|declaration| {
                                    declaration
                                        .evaluated_value
                                        .as_ref()
                                        .and_then(|value| value.encoding())
                                        .is_some_and(|encoding| {
                                            crate::validate_exact_const_value_encoding(
                                                program,
                                                declaration.declared_type,
                                                encoding,
                                            )
                                            .is_ok()
                                        })
                                }),
                            _ => false,
                        }
                    } else {
                        let members = expression_table(program).name_path_members(path.members);
                        matches!(members, [name] if name.as_str().parse::<i128>().is_ok())
                    };
                    if !immutable {
                        return false;
                    }
                }
                ExpressionNode::Integer(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::Binary(_)
                | ExpressionNode::Unary(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Match(_) => {}
                _ => return false,
            }
            crate::queries::expression_traversal::children(program, node, |child| {
                pending.push(child)
            });
        }
        true
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ConstProjectionStep {
        Field(symbols::SymbolHandle),
        Element(usize),
    }

    #[derive(Debug, Clone)]
    pub struct ImmutableConstProjection {
        pub root: symbols::SymbolHandle,
        pub path: Vec<ConstProjectionStep>,
        pub carrier: typed_trees::types::TypeReferenceHandle,
    }

    pub fn immutable_const_projection(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<ImmutableConstProjection> {
        enum Step<'node> {
            Member(&'node typed_trees::expression::TableMemberExpression),
            Element(ExpressionHandle),
        }
        let mut current = expression;
        let mut steps = Vec::new();
        let mut visited = Vec::new();
        loop {
            if !program.expression_table.expression_is_valid(current) || visited.contains(&current)
            {
                return None;
            }
            visited.push(current);
            match expression_table(program).expression(current) {
                ExpressionNode::Member(member) => {
                    if !member.member_symbol.is_valid() || member.case_variant.is_some() {
                        return None;
                    }
                    steps.push(Step::Member(member));
                    current = member.receiver;
                }
                ExpressionNode::Indexed(indexed) => {
                    steps.push(Step::Element(indexed.index));
                    current = indexed.collection;
                }
                _ => break,
            }
        }
        if steps.is_empty() {
            return None;
        }
        let ExpressionNode::Name(path) = expression_table(program).expression(current) else {
            return None;
        };
        if !path.symbol.is_valid()
            || path.head_symbol != path.symbol
            || expression_table(program)
                .name_path_members(path.members)
                .len()
                != 1
        {
            return None;
        }
        let parameter = program.data_type_parameter_by_symbol(path.symbol)?;
        let TypeParameterKind::Const {
            type_reference: mut carrier,
        } = parameter.kind
        else {
            return None;
        };
        if !crate::value_custody::type_references::exact_const_carrier_is_eligible(program, carrier)
        {
            return None;
        }
        let mut projection = Vec::with_capacity(steps.len());
        for step in steps.into_iter().rev() {
            match step {
                Step::Member(member) => {
                    let field = crate::value_custody::places::applied_data_member_field(
                        program, carrier, member,
                    )?;
                    if field.symbol != member.member_symbol
                        || program
                            .symbols
                            .get(program.symbols.get(field.symbol).parent)
                            .kind
                            == SymbolKind::Variant
                    {
                        return None;
                    }
                    projection.push(ConstProjectionStep::Field(field.symbol));
                    carrier = field.type_reference;
                }
                Step::Element(index) => {
                    let ExpressionNode::Integer(index) =
                        expression_table(program).expression(index)
                    else {
                        return None;
                    };
                    let index = usize::try_from(index.value_i64()?).ok()?;
                    let (element, length) = fixed_array_geometry(program, carrier)?;
                    if index >= length {
                        return None;
                    }
                    projection.push(ConstProjectionStep::Element(index));
                    carrier = element;
                }
            }
        }
        Some(ImmutableConstProjection {
            root: parameter.symbol,
            path: projection,
            carrier,
        })
    }

    pub fn fixed_array_geometry(
        program: &dyn crate::CheckingInput,
        carrier: typed_trees::types::TypeReferenceHandle,
    ) -> Option<(typed_trees::types::TypeReferenceHandle, usize)> {
        match program
            .type_reference_table
            .type_reference(crate::peel_declared_constraints(program, carrier))
        {
            typed_trees::types::TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } => Some((*element_type, *length)),
            _ => None,
        }
    }
}

pub mod constants {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use language_semantics::Multiplicity;
    use symbols::{BuiltinTypeAtom, SymbolKind};
    use typed_trees::data::DataMember;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub fn completed_integer_constant(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<typed_trees::closed_numeric::ClosedIntegerValue> {
        use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
        if !expression_table(program).expression_is_valid(expression) {
            return None;
        }
        let ExpressionNode::Name(path) = expression_table(program).expression(expression) else {
            return None;
        };
        if !path.symbol.is_valid() || program.symbols.get(path.symbol).kind != SymbolKind::Const {
            return None;
        }
        let mut declarations = program
            .const_declarations()
            .iter()
            .filter(|declaration| declaration.symbol == path.symbol);
        let declaration = declarations.next()?;
        if declarations.next().is_some() || declaration.pending_target_observation {
            return None;
        }
        let primitive = program.primitive_type_reference(declaration.declared_type)?;
        primitive.landed_landing_fixed()?;
        let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
        crate::validate_exact_const_value_encoding(program, declaration.declared_type, encoding)
            .ok()?;
        let DecodedCanonicalConstValue::Integer { value, .. } =
            CanonicalConstValue::new("", encoding, "").decode_encoding()?
        else {
            return None;
        };
        let value = numerics::bignum::BigInt::from_i128(value);
        crate::value_custody::literals::land_integer_value(&value, primitive)?;
        Some(typed_trees::closed_numeric::ClosedIntegerValue {
            value,
            primitive: Some(primitive),
            type_reference: Some(declaration.declared_type),
        })
    }

    pub(crate) fn validate_constants(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for declaration in program.const_declarations() {
            if !eligible(program, declaration.declared_type, &mut Vec::new()) {
                diagnostics.push(Diagnostic::error(
                "constant type must permit copying and contain no cleanup, shared ownership, or interior mutability",
            ).with_source_span(declaration.initializer_source_span));
            }
        }
    }

    pub fn validate_constant_literal_fields(
        program: &dyn CheckingInput,
        destination: TypeReferenceHandle,
        initializer: ExpressionHandle,
    ) -> Result<(), &'static str> {
        let mut pending = vec![(destination, initializer)];
        while let Some((destination, expression)) = pending.pop() {
            let carrier = program
                .type_reference_table
                .unconstrained_type_reference(destination);
            match expression_table(program).expression(expression) {
                ExpressionNode::ArrayLiteral(elements) => {
                    if let TypeReferenceNode::FixedArray { element_type, .. } =
                        program.type_reference_table.type_reference(carrier)
                    {
                        pending.extend(
                            expression_table(program)
                                .expression_handles(*elements)
                                .iter()
                                .map(|element| (*element_type, *element)),
                        );
                    }
                }
                ExpressionNode::StructLiteral(literal) => {
                    for field in expression_table(program).struct_fields(literal.fields) {
                        let field_type =
                        crate::value_custody::struct_literals::selected_construction_field_type(
                            program,
                            expression,
                            literal,
                            field.field_symbol,
                        )
                        .ok_or("constant initializer lost its selected construction field")?;
                        pending.push((field_type, field.value));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn eligible(
        program: &dyn CheckingInput,
        reference: TypeReferenceHandle,
        active: &mut Vec<TypeReferenceHandle>,
    ) -> bool {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
            || active.contains(&reference)
        {
            return false;
        }
        active.push(reference);
        let result = eligible_type(program, reference, active);
        active.pop();
        result
    }

    fn eligible_type(
        program: &dyn CheckingInput,
        reference: TypeReferenceHandle,
        active: &mut Vec<TypeReferenceHandle>,
    ) -> bool {
        if !matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Constrained { .. }
        ) && let Some(storage) = program.layout_storage_arguments(reference)
        {
            return eligible(program, storage.semantic_type, active);
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Unit => true,
            TypeReferenceNode::DynamicTrait { symbol, .. } => {
                program.trait_definition_by_symbol(*symbol).is_some()
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                eligible(program, *base_type, active)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                eligible(program, *element_type, active)
            }
            TypeReferenceNode::Named { symbol, .. } => {
                if let Some(atom) = program.symbols.builtin_type_atom(*symbol) {
                    return matches!(
                        atom,
                        BuiltinTypeAtom::Bool
                            | BuiltinTypeAtom::I8
                            | BuiltinTypeAtom::I16
                            | BuiltinTypeAtom::I32
                            | BuiltinTypeAtom::I64
                            | BuiltinTypeAtom::U8
                            | BuiltinTypeAtom::U16
                            | BuiltinTypeAtom::U32
                            | BuiltinTypeAtom::U64
                            | BuiltinTypeAtom::Address
                            | BuiltinTypeAtom::F32
                            | BuiltinTypeAtom::F64
                            | BuiltinTypeAtom::UInt
                            | BuiltinTypeAtom::Int
                    );
                }
                let Some(data) = program.data_definition_by_symbol(*symbol) else {
                    return false;
                };
                if !symbol.is_valid()
                    || data.properties.multiplicity != Multiplicity::Unrestricted
                    || !program.data_type_parameters(data).is_empty()
                    || active[..active.len() - 1].iter().any(|ancestor| {
                        match program.type_reference_table.type_reference(*ancestor) {
                            TypeReferenceNode::Named {
                                symbol: previous, ..
                            } => previous == symbol,
                            TypeReferenceNode::Generic {
                                base_symbol: previous,
                                ..
                            } => previous == symbol,
                            _ => false,
                        }
                    })
                    || program.data_selects_cleanup(*symbol)
                {
                    return false;
                }
                program
                    .data_members(data)
                    .iter()
                    .all(|member| match member {
                        DataMember::Field(field) => eligible(program, field.type_reference, active),
                        DataMember::Variant(variant) => program
                            .data_payload_fields(variant)
                            .iter()
                            .all(|field| eligible(program, field.type_reference, active)),
                    })
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let Some(data) = program.data_definition_by_symbol(*base_symbol) else {
                    return false;
                };
                if !base_symbol.is_valid()
                    || data.properties.multiplicity != Multiplicity::Unrestricted
                    || active[..active.len() - 1].iter().any(|ancestor| {
                        match program.type_reference_table.type_reference(*ancestor) {
                            TypeReferenceNode::Named {
                                symbol: previous, ..
                            } => previous == base_symbol,
                            TypeReferenceNode::Generic {
                                base_symbol: previous,
                                ..
                            } => previous == base_symbol,
                            _ => false,
                        }
                    })
                    || program.data_selects_cleanup(*base_symbol)
                {
                    return false;
                }
                for argument in program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    if !matches!(
                        program.type_reference_table.type_reference(*argument),
                        TypeReferenceNode::ConstExpression(_)
                    ) && !eligible(program, *argument, active)
                    {
                        return false;
                    }
                }
                program
                    .data_members(data)
                    .iter()
                    .all(|member| match member {
                        DataMember::Field(field) => eligible(program, field.type_reference, active),
                        DataMember::Variant(variant) => program
                            .data_payload_fields(variant)
                            .iter()
                            .all(|field| eligible(program, field.type_reference, active)),
                    })
            }
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::ConstExpression(_) => false,
        }
    }

    #[cfg(test)]
    mod completed_integer_tests {
        use super::*;
        use language_semantics::const_value::CanonicalConstValue;
        use symbols::{SymbolNameRef, SymbolTableBuilder};
        use typed_trees::{
            TypedTrees,
            constant::ConstDeclaration,
            expression::{Expression, NamePath},
            name::Identifier,
            types::{PrimitiveType, TypeReferenceNode},
        };

        fn fixture(
            encoding: Option<String>,
            pending: bool,
            copies: usize,
        ) -> (TypedTrees, ExpressionHandle) {
            let mut builder = SymbolTableBuilder::default();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let children = builder.insert_children(
                root,
                symbols::builtin_type_symbols()
                    .into_iter()
                    .chain([(SymbolKind::Module, SymbolNameRef::Static("ns"))]),
            );
            let u64_symbol = children.handle_at(BuiltinTypeAtom::U64.ordinal() as u32);
            let namespace = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
            let symbol = builder
                .insert_children(namespace, [(SymbolKind::Const, SymbolNameRef::Static("C"))])
                .start();
            let mut program = TypedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            let carrier = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: u64_symbol,
                    name: Identifier::generated("u64"),
                });
            for _ in 0..copies {
                program.push_const_declaration(ConstDeclaration {
                    symbol,
                    declared_type: carrier,
                    pending_target_observation: pending,
                    evaluated_value: encoding
                        .clone()
                        .map(language_core::constant_value::EvaluatedConstant::Encoded),
                    ..Default::default()
                });
            }
            let expression =
                program
                    .expression_table
                    .insert_tree(&Expression::Name(NamePath::resolved(
                        vec![Identifier::generated("ns"), Identifier::generated("C")],
                        namespace,
                        symbol,
                    )));
            (program, expression)
        }

        #[test]
        fn completed_integer_leaves_require_exact_ready_canonical_carriers() {
            let encoded = |kind: &str, value| {
                CanonicalConstValue::from_decoded(
                    &language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                        type_name: kind.to_owned(),
                        value,
                    },
                    value.to_string(),
                )
                .expect("test integer has a canonical structural encoding")
                .encoding
            };
            for (encoding, pending, copies, expected) in [
                (Some(encoded("u64", 0)), false, 1, Some(0)),
                (
                    Some(encoded("u64", i128::from(u64::MAX))),
                    false,
                    1,
                    Some(u64::MAX),
                ),
                (Some(encoded("u8", 0)), false, 1, None),
                (Some(encoded("u64", -1)), false, 1, None),
                (Some("malformed".to_owned()), false, 1, None),
                (None, false, 1, None),
                (Some(encoded("u64", 0)), true, 1, None),
                (Some(encoded("u64", 0)), false, 0, None),
                (Some(encoded("u64", 0)), false, 2, None),
            ] {
                let (program, expression) = fixture(encoding, pending, copies);
                let value = completed_integer_constant(&program, expression);
                assert_eq!(
                    value.as_ref().and_then(|value| value.value.to_u64()),
                    expected
                );
                if let Some(value) = value {
                    assert_eq!(value.primitive, Some(PrimitiveType::U64));
                    assert!(value.type_reference.is_some());
                }
            }
        }
    }
}

pub mod content_classes {
    use crate::CheckingInput;
    use checked_trees::{ContentClassFacts, ContentClassRow};
    use typed_trees::types::TypeReferenceHandle;

    use crate::value_custody::storage_contents::ContentsRequirement;

    pub fn classify_program_content_classes(program: &dyn CheckingInput) -> ContentClassFacts {
        crate::classify_type_references(program, content_classes, ContentClassFacts::with_rows)
    }

    fn content_classes(
        program: &dyn CheckingInput,
        reference: TypeReferenceHandle,
    ) -> ContentClassRow {
        macro_rules! contents {
            ($requirement:expr $(,)?) => {
                crate::has_contents(program, reference, &[], $requirement)
            };
        }
        ContentClassRow {
            plain_owned_with_numeric_constraints: contents!(ContentsRequirement::NumericOwned),
            plain_owned_with_represented_constraints: contents!(
                ContentsRequirement::RepresentedOwned
            ),
            stored_value_contents: contents!(ContentsRequirement::StoredValue),
            linear_owned_contents: contents!(ContentsRequirement::LinearOwned),
            borrow_free_contents: contents!(ContentsRequirement::BorrowFree),
            owned_or_shared_view_fields: crate::has_owned_or_shared_view_fields(program, reference),
            owned_or_shared_view_contents: contents!(ContentsRequirement::OwnedOrSharedView),
            closed_primitive_array: crate::is_closed_primitive_array_type(program, reference),
            bounded_byte_capacity: crate::bounded_byte_buffer_capacity(program, reference),
        }
    }
}

pub mod data {
    use crate::declarations::symbols::TopLevelSymbols;
    use crate::frozen_program::expression_table;
    use crate::proof_contracts::proof_facts::{ProofFactOwner, validate_proof_facts};
    use crate::value_custody::type_references::{
        TypeParameterScope, TypeReferenceOwner, type_reference_label, type_references_match,
        validate_generic_instance_argument_bounds, validate_type_reference_handle,
    };
    use diagnostics::Diagnostic;
    use numerics::bignum::BigInt;
    use typed_trees::data::DataMember;
    use typed_trees::types::{
        DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };

    pub(crate) fn validate_data_field_types(
        program: &dyn crate::CheckingInput,
        symbols: &TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        for data_definition in program.data_definitions() {
            if symbols::builtin_type_symbols()
                .iter()
                .any(|(_, name)| name.as_str() == data_definition.name.as_str())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{}` shadows the builtin type of the same name -- references \
                 would silently bind to the builtin and this definition would never \
                 be used (its contracts would go unvalidated). Pick a different name.",
                    data_definition.name
                )));
            }
            let data_members = program.data_members(data_definition);
            let type_parameters = program.data_type_parameters(data_definition);
            crate::value_custody::type_references::validate_const_binder_carriers(
                program,
                symbols,
                &format!("data `{}`", data_definition.name),
                type_parameters,
                TypeParameterScope::authored(
                    data_definition.symbol,
                    type_parameters,
                    &data_definition.lifetime_parameters,
                ),
                diagnostics,
                formation_obligations,
            );
            validate_generic_instance_argument_bounds(program, data_definition, diagnostics);
            validate_data_member_names(data_definition, data_members, diagnostics);
            validate_data_default_domain(program, data_definition, data_members, diagnostics);

            for member in data_members {
                let payload_fields = match member {
                    DataMember::Field(field) => std::slice::from_ref(field),
                    DataMember::Variant(variant) => {
                        validate_payload_field_names(
                            data_definition,
                            variant,
                            program,
                            diagnostics,
                        );
                        program.data_payload_fields(variant)
                    }
                };

                for field in payload_fields {
                    validate_type_reference_handle(
                        program,
                        field.type_reference,
                        symbols,
                        diagnostics,
                        TypeReferenceOwner::DataField {
                            data: data_definition.name.as_str(),
                            field: field.name.as_str(),
                            generic_depth: 0,
                        },
                        TypeParameterScope::authored(
                            data_definition.symbol,
                            type_parameters,
                            &data_definition.lifetime_parameters,
                        ),
                        formation_obligations,
                    );
                    validate_stored_field_has_a_representation(
                        program,
                        data_definition,
                        field,
                        diagnostics,
                    );
                }
            }
        }
    }

    fn validate_stored_field_has_a_representation(
        program: &dyn crate::CheckingInput,
        data_definition: &typed_trees::data::DataDefinition,
        field: &typed_trees::data::DataField,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if field.relevance.is_erased() {
            return;
        }
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = program
            .type_reference_table
            .type_reference(field.type_reference)
        else {
            return;
        };
        let Some(builtin) = [symbols::BuiltinType::UInt, symbols::BuiltinType::Int]
            .into_iter()
            .find(|candidate| program.symbols.builtin_type_symbol(*candidate) == Some(*symbol))
        else {
            return;
        };
        diagnostics.push(Diagnostic::error(format!(
 "data `{}` field `{}` stores unbounded integer type `{}`, which has no runtime representation; use a sized integer such as `u64` or `i64`, or declare the field `[erased]` to keep it proof-only",
        data_definition.name.as_str(),
        field.name.as_str(),
        builtin.name(),
    )));
    }

    pub fn type_requires_establishment(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        crate::value_custody::type_properties::memoized_type_requires_establishment(
            program,
            type_reference,
            || {
                type_requires_establishment_inner(
                    program,
                    type_reference,
                    &mut EstablishmentIndex::new(),
                )
            },
        )
    }

    pub(crate) struct EstablishmentIndex {
        seen: Vec<EstablishmentIdentity>,
        computed: Vec<(EstablishmentIdentity, bool)>,
    }

    impl EstablishmentIndex {
        fn new() -> Self {
            Self {
                seen: Vec::new(),
                computed: Vec::new(),
            }
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) struct EstablishmentIdentity {
        declaration: symbols::SymbolHandle,
        application: TypeReferenceHandle,
    }

    impl EstablishmentIdentity {
        pub(crate) fn of(definition: &typed_trees::data::DataDefinition) -> Self {
            Self {
                declaration: definition.symbol,
                application: definition.generic_instance.unwrap_or_default(),
            }
        }

        pub(crate) fn order(self) -> (u32, u32, u32, u32) {
            (
                self.declaration.arena_index(),
                self.declaration.generation(),
                self.application.arena_index(),
                self.application.generation(),
            )
        }
    }

    fn validate_data_default_domain(
        program: &dyn crate::CheckingInput,
        definition: &typed_trees::data::DataDefinition,
        members: &[DataMember],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let facts = program.proof_facts.span_or_empty(definition.requires_facts);
        validate_proof_facts(
            program,
            facts,
            diagnostics,
            ProofFactOwner::DataDefaultDomain(definition.name.as_str()),
        );

        for fact in facts {
            let Some(membership) = fact.as_membership() else {
                continue;
            };
            let Some(domain) = program.domain_definition(membership.domain_symbol) else {
                continue;
            };
            let typed_trees::expression::ExpressionNode::Name(path) =
                expression_table(program).expression(membership.value)
            else {
                continue;
            };
            let Some(field_name) = expression_table(program)
                .name_path_members(path.members)
                .last()
            else {
                continue;
            };
            let Some(field_type) = members.iter().find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == field_name.as_str() => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            }) else {
                continue;
            };
            let field_carrier =
                crate::value_custody::places::unwrapped_type_reference(program, field_type)
                    .unwrap_or(field_type);
            let domain_carrier =
                crate::value_custody::places::unwrapped_type_reference(program, domain.target_type)
                    .unwrap_or(domain.target_type);
            if type_references_match(program, field_carrier, domain_carrier) {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
            "data `{}` default-domain fact puts field `{}` in domain `{}`, but the field carrier is `{}` while the domain classifies `{}`",
            definition.name.as_str(),
            field_name.as_str(),
            domain.name.as_str(),
            type_reference_label(program, field_carrier),
            type_reference_label(program, domain_carrier),
        )));
        }
    }

    fn type_requires_establishment_inner(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        index: &mut EstablishmentIndex,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        if !matches!(
            program.type_reference_table.type_reference(type_reference),
            TypeReferenceNode::Constrained { .. }
        ) && let Some(storage) = program.layout_storage_arguments(type_reference)
        {
            return type_requires_establishment_inner(program, storage.semantic_type, index);
        }
        if let Some(interval) =
            crate::proof_contracts::arithmetic_domains::range_constraint_interval(
                program,
                type_reference,
            )
            && (interval.low().is_some_and(|low| low > 0)
                || interval.high().is_some_and(|high| high < 0))
        {
            return true;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| {
                        constraint_requires_establishment(program, *base_type, constraint)
                    })
                    || type_requires_establishment_inner(program, *base_type, index)
            }
            TypeReferenceNode::DynamicTrait { .. } => true,
            TypeReferenceNode::Reference { .. } => !matches!(
                crate::byte_sequence_carrier(program, type_reference, &[]),
                Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView {
                    access: Some(checked_trees::CheckedStructuralAccess::SharedBorrow),
                })
            ),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                !matches!(length, typed_trees::types::FixedArrayLength::Literal(0))
                    && type_requires_establishment_inner(program, *element_type, index)
            }
            TypeReferenceNode::Named { symbol, name } => {
                if symbol.is_valid()
                    && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
                {
                    return true;
                }
                let definition = program.closed_data_definition(type_reference).or_else(|| {
                    if symbol.is_valid() {
                        crate::declarations::data_queries::data_definition_by_symbol(
                            program, *symbol,
                        )
                    } else {
                        program.data_definition_by_name_str(name.as_str())
                    }
                });
                definition.is_some_and(|definition| {
                    if index.seen.is_empty() {
                        data_requires_establishment(program, definition)
                    } else {
                        data_requires_establishment_inner(program, definition, index)
                    }
                })
            }
            TypeReferenceNode::Generic { base_symbol, .. } => program
                .closed_data_definition(type_reference)
                .or_else(|| {
                    crate::declarations::data_queries::data_definition_by_symbol(
                        program,
                        *base_symbol,
                    )
                })
                .is_none_or(|definition| {
                    if index.seen.is_empty() {
                        data_requires_establishment(program, definition)
                    } else {
                        data_requires_establishment_inner(program, definition, index)
                    }
                }),
            _ => false,
        }
    }

    fn constraint_requires_establishment(
        program: &dyn crate::CheckingInput,
        carrier: TypeReferenceHandle,
        constraint: &TypeConstraintNode,
    ) -> bool {
        match constraint {
            TypeConstraintNode::Domain(domain) => match domain.subject {
                DomainConstraintSubject::Declared => {
                    if crate::value_custody::field_domains::domain_is_vacuous(
                        program,
                        domain.symbol,
                        &mut Vec::new(),
                    ) {
                        return false;
                    }
                    let zero_integer = crate::proof_contracts::domains::integer_domain_membership(
                        program,
                        domain.symbol,
                        &domain.arguments,
                        domain.semantic_id,
                        &crate::ScopedArithmeticValue::Integer(BigInt::zero()),
                    ) == crate::StrictArithmeticImplicationJudgment::Proven;
                    let zero_bytes = domain.establishment_routes.is_empty()
                        && crate::value_custody::field_domains::domain_admits_zero_byte_carrier(
                            program,
                            domain.symbol,
                            carrier,
                        );
                    !zero_integer && !zero_bytes
                    && !crate::proof_contracts::contract_entailment::domain_predicates_admit_zero(program, carrier, domain)
                }
                DomainConstraintSubject::Carry(_)
                | DomainConstraintSubject::OmegaLayout { .. }
                | DomainConstraintSubject::Value(
                    language_semantics::value_domain::ValueDomain::Finite,
                ) => false,
            },
            TypeConstraintNode::Named(_) => true,

            TypeConstraintNode::ArithmeticDomain(_) => false,
        }
    }

    pub fn data_requires_establishment(
        program: &dyn crate::CheckingInput,
        definition: &typed_trees::data::DataDefinition,
    ) -> bool {
        if definition.zero_gated && !crate::data_predicates_admit_zero(program, definition) {
            return true;
        }
        crate::value_custody::type_properties::memoized_data_requires_establishment(
            program,
            definition,
            || {
                data_requires_establishment_inner(
                    program,
                    definition,
                    &mut EstablishmentIndex::new(),
                )
            },
        )
    }

    fn data_requires_establishment_inner(
        program: &dyn crate::CheckingInput,
        definition: &typed_trees::data::DataDefinition,
        index: &mut EstablishmentIndex,
    ) -> bool {
        if definition.zero_gated && !crate::data_predicates_admit_zero(program, definition) {
            return true;
        }
        let key = EstablishmentIdentity::of(definition);
        if index.seen.contains(&key) {
            return false;
        }
        if let Ok(position) = index
            .computed
            .binary_search_by_key(&key.order(), |(identity, _)| identity.order())
        {
            return index.computed[position].1;
        }
        index.seen.push(key);

        let members = program.data_members(definition);
        let common_gated = members.iter().any(|member| match member {
            DataMember::Field(field) => {
                type_requires_establishment_inner(program, field.type_reference, index)
            }
            DataMember::Variant(_) => false,
        });
        let zero_case_gated = members
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) => Some(variant),
                DataMember::Field(_) => None,
            })
            .is_some_and(|variant| {
                !crate::value_custody::struct_literals::case_predicates_admit_zero(
                    program, definition, variant,
                ) || program.data_payload_fields(variant).iter().any(|field| {
                    type_requires_establishment_inner(program, field.type_reference, index)
                })
            });

        index.seen.pop();
        let result = common_gated || zero_case_gated;
        match index
            .computed
            .binary_search_by_key(&key.order(), |(identity, _)| identity.order())
        {
            Ok(position) => index.computed[position].1 = result,
            Err(position) => index.computed.insert(position, (key, result)),
        }
        result
    }

    fn validate_payload_field_names(
        data_definition: &typed_trees::data::DataDefinition,
        variant: &typed_trees::data::DataVariant,
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let data_members = program.data_members(data_definition);
        let payload_fields = program.data_payload_fields(variant);
        for (field_index, field) in payload_fields.iter().enumerate() {
            if payload_fields[..field_index]
                .iter()
                .any(|previous| previous.name.as_str() == field.name.as_str())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{}` case `{}` has duplicate payload field `{}`",
                    data_definition.name, variant.name, field.name
                )));
            }

            if data_members.iter().any(|member| {
            matches!(member, DataMember::Field(common) if common.name.as_str() == field.name.as_str())
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` case `{}` payload field `{}` collides with the common field of the same name",
                data_definition.name,
                variant.name,
                field.name
            )));
        }
        }
    }

    fn validate_data_member_names(
        data_definition: &typed_trees::data::DataDefinition,
        data_members: &[DataMember],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (member_index, member) in data_members.iter().enumerate() {
            let member_name = match member {
                DataMember::Field(field) => field.name.as_str(),
                DataMember::Variant(variant) => variant.name.as_str(),
            };

            if data_members[..member_index]
                .iter()
                .any(|previous| data_member_name(previous) == member_name)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{}` has duplicate member `{member_name}`",
                    data_definition.name
                )));
            }
        }
    }

    fn data_member_name(member: &DataMember) -> &str {
        match member {
            DataMember::Field(field) => field.name.as_str(),
            DataMember::Variant(variant) => variant.name.as_str(),
        }
    }
}

pub mod destructure {
    use diagnostics::Diagnostic;
    use typed_trees::data::DataMember;

    const ARM_MARKER_PREFIX: &str = "__arm_destructure#V=";

    pub(crate) fn validate_destructure_exhaustiveness(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let Some(local) = statement.as_local_data() else {
                        continue;
                    };
                    if let Some(encoded) = local.name.as_str().strip_prefix(ARM_MARKER_PREFIX) {
                        validate_arm_pattern_marker(
                            program,
                            machine,
                            state,
                            local,
                            encoded,
                            diagnostics,
                        );
                        continue;
                    }
                    if local.role != language_core::LocalBindingRole::RecordPattern {
                        continue;
                    }
                    let data = match record_pattern_subject(program, machine, state, local) {
                        Ok(data) => data,
                        Err(refusal) => {
                            diagnostics.push(refusal.diagnostic(machine, state));
                            continue;
                        }
                    };

                    let roster = crate::frozen_program::expression_table(program)
                        .match_fields(local.pattern_fields);
                    let declared = program
                        .data_members(data)
                        .iter()
                        .filter_map(DataMember::as_field)
                        .collect::<Vec<_>>();
                    let resolved = local.pattern_subject == data.symbol;
                    let (unmentioned, unknown) =
                        record_pattern_coverage(resolved, &declared, roster);
                    for field in unmentioned {
                        diagnostics.push(Diagnostic::error(format!(
                            "record pattern in machine `{}` state `{}` does not mention field `{}` of `{}` -- record patterns are exhaustive by law: bind it, rename it with `as`, or waive it with `as _`",
                            machine.name, state.name, field.name, data.name,
                        )));
                    }
                    for row in unknown {
                        diagnostics.push(Diagnostic::error(format!(
                            "record pattern in machine `{}` state `{}` names `{}`, which is not a field of `{}`",
                            machine.name, state.name, row.member, data.name,
                        )));
                    }
                }
            }
        }
    }

    /*
        A pattern's coverage of its record, decided on identity alone: the
        declared fields no roster row names, in declaration order, then the
        rows that name no declared field, in authored order. Typed preparation
        gave each row the symbol of the field it names; a row it could not
        place has none. `resolved` says the roster was resolved against this
        very record. One resolved against any other declaration names no field
        here, however its rows are spelled.
    */
    fn record_pattern_coverage<'row>(
        resolved: bool,
        declared: &[&'row typed_trees::data::DataField],
        roster: &'row [typed_trees::expression::TableMatchField],
    ) -> (
        Vec<&'row typed_trees::data::DataField>,
        Vec<&'row typed_trees::expression::TableMatchField>,
    ) {
        let names = |row: &typed_trees::expression::TableMatchField,
                     field: &typed_trees::data::DataField| {
            resolved && field.symbol.is_valid() && row.field_symbol == field.symbol
        };
        let unmentioned = declared
            .iter()
            .copied()
            .filter(|field| !roster.iter().any(|row| names(row, field)))
            .collect();
        let unknown = roster
            .iter()
            .filter(|row| !declared.iter().any(|field| names(row, field)))
            .collect();
        (unmentioned, unknown)
    }

    /// Why a record pattern's subject offers no field roster to cover.
    pub enum RecordPatternRefusal<'program> {
        UnresolvedType,
        NotData,
        Quotient(&'program typed_trees::data::DataDefinition),
        Sum(&'program typed_trees::data::DataDefinition),
    }

    impl RecordPatternRefusal<'_> {
        fn diagnostic(
            &self,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
        ) -> Diagnostic {
            Diagnostic::error(match self {
                Self::UnresolvedType => format!(
                    "record pattern in machine `{}` state `{}` destructures a place whose declared type cannot be resolved -- the exhaustiveness law needs the data definition",
                    machine.name, state.name,
                ),
                Self::NotData => format!(
                    "record pattern in machine `{}` state `{}` destructures a value that is not a data record -- only plain data definitions destructure",
                    machine.name, state.name,
                ),
                Self::Quotient(data) => format!(
                    "record pattern in machine `{}` state `{}` cannot destructure quotient `{}`: retained representatives are opaque and pattern matching requires a named lifted operation",
                    machine.name, state.name, data.name,
                ),
                Self::Sum(data) => format!(
                    "record pattern in machine `{}` state `{}` destructures sum type `{}` -- case variants need a `case` arm, not a record pattern",
                    machine.name, state.name, data.name,
                ),
            })
        }
    }

    /*
        The plain record a `let { .. } = subject;` marker destructures, by the
        subject's declared place type. Typed preparation resolves the pattern's
        roster against this declaration and checking covers the same one, so
        both ask here.
    */
    pub fn record_pattern_subject<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        local: &typed_trees::statement::TableLocalData,
    ) -> Result<&'program typed_trees::data::DataDefinition, RecordPatternRefusal<'program>> {
        let declared = crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            Some(state),
            local.initial_value,
        );
        let resolved = declared.or_else(|| {
            local
                .type_reference
                .is_valid()
                .then_some(local.type_reference)
        });
        let Some(resolved) = resolved else {
            return Err(RecordPatternRefusal::UnresolvedType);
        };
        let Some(data) = crate::value_custody::places::data_definition_for_type(program, resolved)
        else {
            return Err(RecordPatternRefusal::NotData);
        };
        if data.quotient.is_some() {
            return Err(RecordPatternRefusal::Quotient(data));
        }
        if program
            .data_members(data)
            .iter()
            .any(|member| matches!(member, DataMember::Variant(_)))
        {
            return Err(RecordPatternRefusal::Sum(data));
        }
        Ok(data)
    }

    fn validate_arm_pattern_marker(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        local: &typed_trees::statement::TableLocalData,
        encoded: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut parts = encoded.split('#');
        let variant = parts.next().unwrap_or("");
        let mut spelled: Vec<&str> = parts.collect();
        let has_rest = spelled.last() == Some(&"~rest");
        if has_rest {
            spelled.pop();
        }
        if spelled
            .last()
            .is_some_and(|part| part.starts_with("~subject="))
        {
            spelled.pop();
        }

        let declared = crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            Some(state),
            local.initial_value,
        )
        .or_else(|| {
            local
                .type_reference
                .is_valid()
                .then_some(local.type_reference)
        });
        let Some(declared) = declared else {
            return;
        };
        let Some(data) = crate::value_custody::places::data_definition_for_type(program, declared)
        else {
            return;
        };

        let declared_fields: Vec<&str> = if variant.is_empty() {
            let mut fields = Vec::new();
            for member in program.data_members(data) {
                match member {
                    DataMember::Field(field) => {
                        fields.push(field.name.as_str());
                    }
                    DataMember::Variant(_) => {
                        diagnostics.push(Diagnostic::error(format!(
                        "record pattern arm in machine `{}` state `{}` destructures sum type `{}` without naming a case -- spell the variant (`{}::Case {{ .. }}`)",
                        machine.name, state.name, data.name, data.name,
                    )));
                        return;
                    }
                }
            }
            fields
        } else {
            let qualified_pattern = format!("{}::{variant}", data.name);
            if program.domain_definitions().iter().any(|domain| {
                domain.name.as_str() == qualified_pattern
                    && crate::value_custody::type_references::type_references_match(
                        program,
                        declared,
                        domain.target_type,
                    )
            }) {
                diagnostics.push(Diagnostic::error(format!(
                "domain pattern `{qualified_pattern}` cannot bind payload fields; payload destructuring is only valid for a data case"
            )));
                return;
            }
            let Some(case) = program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(case) if case.name.as_str() == variant => Some(case),
                    _ => None,
                })
            else {
                return;
            };
            program
                .data_members(data)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field),
                    DataMember::Variant(_) => None,
                })
                .chain(program.data_payload_fields(case))
                .map(|field| field.name.as_str())
                .collect()
        };

        let subject_name = if variant.is_empty() {
            data.name.to_string()
        } else {
            format!("{}::{}", data.name, variant)
        };
        if !has_rest {
            for field in &declared_fields {
                if !spelled.contains(field) {
                    diagnostics.push(Diagnostic::error(format!(
                    "destructure arm in machine `{}` state `{}` does not mention field `{}` of `{}` -- record patterns are exhaustive by law: bind it, rename it with `as`, waive it with `as _`, or opt out with `..`",
                    machine.name, state.name, field, subject_name,
                )));
                }
            }
        }
        for field in &spelled {
            if !declared_fields.contains(field) {
                diagnostics.push(Diagnostic::error(format!(
                "destructure arm in machine `{}` state `{}` names `{}`, which is not a field of `{}`",
                machine.name, state.name, field, subject_name,
            )));
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::record_pattern_coverage;
        use symbols::SymbolHandle;
        use typed_trees::data::DataField;
        use typed_trees::expression::{MatchFieldAction, TableMatchField};
        use typed_trees::name::Identifier;

        fn field(symbol: u32, name: &str) -> DataField {
            DataField {
                identity: None,
                symbol: SymbolHandle::from_arena_index(symbol),
                name: Identifier::generated(name),
                relevance: language_core::BindingRelevance::Relevant,
                type_reference: Default::default(),
            }
        }

        fn row(symbol: Option<u32>, member: &str, binding: Option<&str>) -> TableMatchField {
            TableMatchField {
                component: language_core::PatternComponent::Fields,
                member: Identifier::generated(member),
                field_symbol: symbol.map_or_else(SymbolHandle::invalid, |symbol| {
                    SymbolHandle::from_arena_index(symbol)
                }),
                action: binding.map_or(MatchFieldAction::Waive, |binding| {
                    MatchFieldAction::Bind(Identifier::generated(binding))
                }),
            }
        }

        fn verdict(
            resolved: bool,
            declared: &[DataField],
            roster: &[TableMatchField],
        ) -> (Vec<String>, Vec<String>) {
            let declared = declared.iter().collect::<Vec<_>>();
            let (unmentioned, unknown) = record_pattern_coverage(resolved, &declared, roster);
            (
                unmentioned
                    .iter()
                    .map(|field| field.name.to_string())
                    .collect(),
                unknown.iter().map(|row| row.member.to_string()).collect(),
            )
        }

        /// `data Point { x; y; z; }`: a field counts as named only through a
        /// row carrying its symbol, bound or waived alike.
        #[test]
        fn coverage_follows_field_symbols_in_declaration_and_authored_order() {
            let point = [field(11, "x"), field(12, "y"), field(13, "z")];
            let none: (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
            let full = [
                row(Some(13), "z", None),
                row(Some(11), "x", Some("x")),
                row(Some(12), "y", Some("vertical")),
            ];
            assert_eq!(verdict(true, &point, &full), none);
            assert_eq!(
                verdict(true, &point, &full[1..2]),
                (vec!["y".to_owned(), "z".to_owned()], Vec::new())
            );
            let stray = [
                row(None, "depth", None),
                row(Some(11), "x", Some("x")),
                row(Some(12), "y", None),
                row(Some(13), "z", None),
                row(Some(40), "other", None),
            ];
            assert_eq!(
                verdict(true, &point, &stray),
                (Vec::new(), vec!["depth".to_owned(), "other".to_owned()])
            );
        }

        /// A spelling decides nothing: rows spelled like the fields but
        /// carrying no symbol, another record's symbols, or resolved against
        /// another record cover no field and name none.
        #[test]
        fn a_row_spelled_like_a_field_does_not_name_it() {
            let point = [field(11, "x"), field(12, "y")];
            let everything = (
                vec!["x".to_owned(), "y".to_owned()],
                vec!["x".to_owned(), "y".to_owned()],
            );
            let unplaced = [row(None, "x", Some("x")), row(None, "y", None)];
            assert_eq!(verdict(true, &point, &unplaced), everything);
            let foreign = [row(Some(21), "x", Some("x")), row(Some(22), "y", None)];
            assert_eq!(verdict(true, &point, &foreign), everything);
            let placed = [row(Some(11), "x", Some("x")), row(Some(12), "y", None)];
            assert_eq!(verdict(false, &point, &placed), everything);
            // A declared field without a symbol is named by no row.
            let unsymboled = [DataField {
                symbol: SymbolHandle::invalid(),
                ..field(0, "x")
            }];
            assert_eq!(
                verdict(true, &unsymboled, &unplaced[..1]),
                (vec!["x".to_owned()], vec!["x".to_owned()])
            );
        }
    }
}

pub mod dispatch_extraction {
    use crate::frozen_program::expression_table;
    use checked_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use checked_trees::statement::{StatementNode, TransitionGuardNode};
    use symbols::SymbolHandle;
    use typed_trees::data::DataMember;
    use typed_trees::machine::Machine;

    #[derive(Default, Clone)]
    pub struct DispatchExtraction {
        selected_cases: Vec<(ExpressionHandle, SymbolHandle)>,
    }

    impl DispatchExtraction {
        pub fn reads_payload_binding(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            payload: ExpressionHandle,
        ) -> bool {
            Self::payload_binding_receiver(program, expression, payload).is_some()
        }

        pub fn payload_binding_receiver(
            program: &dyn crate::CheckingInput,
            mut expression: ExpressionHandle,
            payload: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            if !expression_table(program).expression_is_valid(expression)
                || !expression_table(program).expression_is_valid(payload)
            {
                return None;
            }
            let source = expression_table(program).member_node(payload)?;
            source.case_variant.as_ref()?;
            loop {
                match expression_table(program).expression(expression) {
                    ExpressionNode::Member(member) => {
                        if member.case_variant.is_some() {
                            return expression_table(program)
                                .expressions_structurally_equal(expression, payload)
                                .then_some(expression);
                        }
                        expression = member.receiver;
                    }
                    ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                    _ => return None,
                }
            }
        }

        pub fn rejoin_completed_subject(
            &mut self,
            observed: ExpressionHandle,
            captured: ExpressionHandle,
        ) {
            for (subject, _) in &mut self.selected_cases {
                if *subject == observed {
                    *subject = captured;
                }
            }
        }

        pub fn of_transition_arm(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
        ) -> Self {
            let mut extraction = Self::default();
            let Some(StatementNode::Transition(transition)) =
                program.statement_table.statement_at(state, statement_index)
            else {
                return extraction;
            };
            let TransitionGuardNode::When(guard) = transition.guard else {
                return extraction;
            };
            let mut conjuncts = vec![guard];
            while let Some(conjunct) = conjuncts.pop() {
                let Some(binary) = expression_table(program).binary_node(conjunct) else {
                    continue;
                };
                match binary.operator {
                    BinaryOperator::And => conjuncts.extend([binary.left, binary.right]),
                    _ => {
                        if let Some(case) = crate::applied_case_membership_variant(
                            program,
                            machine,
                            Some(state),
                            conjunct,
                            binary,
                        ) {
                            extraction.selected_cases.push((binary.left, case));
                        }
                    }
                }
            }
            extraction
        }

        pub fn reads_extracted_copy(
            &self,
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            is_plain_copy: impl Fn(ExpressionHandle) -> bool,
        ) -> bool {
            self.extracted_copy_payload(program, expression, is_plain_copy)
                .is_some()
        }

        pub fn extracted_copy_payload(
            &self,
            program: &dyn crate::CheckingInput,
            mut expression: ExpressionHandle,
            is_plain_copy: impl Fn(ExpressionHandle) -> bool,
        ) -> Option<ExpressionHandle> {
            if self.selected_cases.is_empty() {
                return None;
            }
            let (payload, member) = loop {
                match expression_table(program).expression(expression) {
                    ExpressionNode::Member(member) if member.case_variant.is_some() => {
                        break (expression, member);
                    }
                    ExpressionNode::Member(member) => expression = member.receiver,
                    ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                    _ => return None,
                }
            };
            self.selected_cases
                .iter()
                .any(|(selected, case)| {
                    if !expression_table(program)
                        .expressions_structurally_equal(*selected, member.receiver)
                    {
                        return false;
                    }
                    let Some(owner) =
                        program.data_definition_by_symbol(program.symbols.get(*case).parent)
                    else {
                        return false;
                    };
                    let Some(variant) =
                        program
                            .data_members(owner)
                            .iter()
                            .find_map(|row| match row {
                                DataMember::Variant(variant) if variant.symbol == *case => {
                                    Some(variant)
                                }
                                _ => None,
                            })
                    else {
                        return false;
                    };
                    member.case_variant.as_ref() == Some(&variant.name)
                        && program.data_payload_fields(variant).iter().any(|field| {
                            field.name == member.member
                                && (!member.member_symbol.is_valid()
                                    || member.member_symbol == field.symbol)
                        })
                })
                .then_some(payload)
                .filter(|payload| is_plain_copy(*payload))
        }
    }
}

pub mod executable_value_occurrences {
    pub(crate) mod expression {
        use super::ExpressionHandle;
        use super::ValueOccurrenceVisitor;

        impl ValueOccurrenceVisitor<'_, '_> {
            pub(crate) fn collect_expression_children(&mut self, expression: ExpressionHandle) {
                let program = self.program;
                crate::queries::expression_traversal::executable_children(
                    program,
                    program.expression_table.expression(expression),
                    |child| self.collect_nested_expression(expression, child),
                );
            }
        }
    }
    pub(crate) mod statement {
        use super::SymbolHandle;
        use super::ValueOccurrenceVisitor;
        use checked_trees::CheckedValueStatementRole;
        use language_core::inline_assembly::AsmRegisterOperation;
        use typed_trees::statement::{StatementNode, TransitionGuardNode};
        use typed_trees::types::PrimitiveType;

        impl ValueOccurrenceVisitor<'_, '_> {
            pub(crate) fn collect_statement(
                &mut self,
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                statement_index: usize,
                statement: &StatementNode,
            ) {
                match statement {
                    StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
                    StatementNode::Assignment(assignment) => {
                        self.collect_statement_expression(
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            assignment.target,
                            CheckedValueStatementRole::AssignmentTargetSubexpression,
                        );
                        self.collect_statement_expression(
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            assignment.value,
                            CheckedValueStatementRole::AssignmentValue,
                        );
                    }
                    StatementNode::Call(call) => {
                        let register_operation = self
                            .program
                            .symbols
                            .builtin_function_for_symbol(call.target_symbol)
                            .and_then(|builtin| {
                                AsmRegisterOperation::from_intrinsic_name(builtin.name())
                            });
                        for (argument_index, argument) in self
                            .program
                            .statement_table
                            .expression_handles(call.arguments)
                            .iter()
                            .copied()
                            .enumerate()
                        {
                            self.collect_expression(
                                argument,
                                checked_trees::CheckedValueOrigin::StateStatement {
                                    machine_symbol,
                                    state_symbol,
                                    statement_index,
                                    role: CheckedValueStatementRole::CallArgument,
                                },
                                register_operation
                                    .and_then(|operation| {
                                        operation.input_carriers().get(argument_index)
                                    })
                                    .and_then(|carrier| PrimitiveType::from_name(carrier.name())),
                            );
                        }
                    }
                    StatementNode::Expression(expression) => {
                        self.collect_statement_expression(
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            *expression,
                            CheckedValueStatementRole::Expression,
                        );
                    }
                    StatementNode::LocalData(local_data) => {
                        if local_data.initial_value.is_valid() {
                            self.collect_statement_expression(
                                machine_symbol,
                                state_symbol,
                                statement_index,
                                local_data.initial_value,
                                CheckedValueStatementRole::LocalInitializer,
                            );
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(expression) = transition.guard {
                            self.collect_statement_expression(
                                machine_symbol,
                                state_symbol,
                                statement_index,
                                expression,
                                CheckedValueStatementRole::TransitionGuard,
                            );
                        }
                        for target in transition.targets() {
                            self.collect_transition_target(
                                machine_symbol,
                                state_symbol,
                                statement_index,
                                target,
                            );
                        }
                    }
                }
            }
        }
    }
    pub(crate) mod transition {
        use super::SymbolHandle;
        use super::ValueOccurrenceVisitor;
        use checked_trees::CheckedValueStatementRole;
        use typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

        impl ValueOccurrenceVisitor<'_, '_> {
            pub(crate) fn collect_transition_target(
                &mut self,
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                statement_index: usize,
                target: TransitionTargetHandle,
            ) {
                if !target.is_valid() {
                    return;
                }

                match self.program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in self
                            .program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .copied()
                        {
                            self.collect_statement_expression(
                                machine_symbol,
                                state_symbol,
                                statement_index,
                                argument,
                                CheckedValueStatementRole::TransitionTargetArgument,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        self.collect_statement_expression(
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            *expression,
                            CheckedValueStatementRole::TransitionTargetValue,
                        );
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }

    use checked_trees::{
        CheckedValueFact, CheckedValueFacts, CheckedValueOrigin, CheckedValueStatementRole,
    };
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::references::DeclarationSelection;
    use typed_trees::types::PrimitiveType;

    pub fn visit_executable_value_occurrences(
        program: &dyn crate::CheckingInput,
        roots: &DeclarationSelection,
        visit: &mut dyn FnMut(ExpressionHandle, CheckedValueOrigin, Option<PrimitiveType>),
    ) {
        let mut visitor = ValueOccurrenceVisitor { program, visit };
        for machine in program.machines() {
            if !roots.contains(machine.symbol) {
                continue;
            }
            if let Some(subjects) =
                typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
            {
                for (ordinal, expression) in subjects.into_iter().enumerate() {
                    visitor.collect_expression(
                        expression,
                        CheckedValueOrigin::MachineDecrease {
                            machine_symbol: machine.symbol,
                            ordinal,
                        },
                        None,
                    );
                }
            }

            for owned_data in program.machine_owned_data(machine) {
                if owned_data.initial_value.is_valid() {
                    visitor.collect_expression(
                        owned_data.initial_value,
                        CheckedValueOrigin::MachineOwnedDataInitializer {
                            machine_symbol: machine.symbol,
                            data_symbol: owned_data.symbol,
                        },
                        None,
                    );
                }
            }
            for state in program.machine_states(machine) {
                for (statement_index, statement) in program
                    .statement_table
                    .indexed_statements(state.statement_nodes)
                {
                    visitor.collect_statement(
                        machine.symbol,
                        state.symbol,
                        statement_index,
                        statement,
                    );
                }
            }
        }
    }

    pub fn collect_executable_value_origins(
        program: &dyn crate::CheckingInput,
        roots: &DeclarationSelection,
    ) -> CheckedValueFacts {
        let mut facts = CheckedValueFacts::default();
        visit_executable_value_occurrences(program, roots, &mut |expression, origin, _| {
            facts.values.insert(CheckedValueFact {
                expression,
                origin,
                type_reference: typed_trees::types::TypeReferenceHandle::invalid(),
                primitive_type: None,
                integer_range: None,
            });
        });
        facts
    }

    pub fn visit_statement_value_occurrences(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        statement: &typed_trees::statement::StatementNode,
        visit: &mut dyn FnMut(ExpressionHandle, CheckedValueOrigin, Option<PrimitiveType>),
    ) {
        ValueOccurrenceVisitor { program, visit }.collect_statement(
            machine,
            state,
            statement_index,
            statement,
        );
    }

    pub(crate) struct ValueOccurrenceVisitor<'program, 'visitor> {
        program: &'program dyn crate::CheckingInput,
        visit: &'visitor mut dyn FnMut(ExpressionHandle, CheckedValueOrigin, Option<PrimitiveType>),
    }

    impl ValueOccurrenceVisitor<'_, '_> {
        fn collect_statement_expression(
            &mut self,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
            expression: ExpressionHandle,
            role: CheckedValueStatementRole,
        ) {
            self.collect_expression(
                expression,
                CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    role,
                },
                None,
            );
        }

        fn collect_expression(
            &mut self,
            expression: ExpressionHandle,
            origin: CheckedValueOrigin,
            expected_primitive: Option<PrimitiveType>,
        ) {
            if !expression.is_valid() {
                return;
            }
            (self.visit)(expression, origin, expected_primitive);
            self.collect_expression_children(expression);
        }

        fn collect_nested_expression(
            &mut self,
            parent: ExpressionHandle,
            expression: ExpressionHandle,
        ) {
            self.collect_expression(
                expression,
                CheckedValueOrigin::NestedExpression { parent },
                None,
            );
        }
    }
}

pub mod intrinsic_boundaries {
    use language_semantics::MachineSupplyMode;
    use symbols::SymbolHandle;
    use typed_trees::boundary_completions::exact_direct_intrinsic_signature;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::types::{PrimitiveType, TypeReferenceNode};

    pub fn exact_external_leaf_boundary_requirement(
        program: &dyn crate::CheckingInput,
        target_state_symbol: SymbolHandle,
    ) -> Option<(SymbolHandle, SymbolHandle)> {
        exact_compiler_intrinsic_boundary_requirement(program, target_state_symbol)
            .or_else(|| exact_evaluated_binding_boundary_requirement(program, target_state_symbol))
    }

    fn exact_evaluated_binding_boundary_requirement(
        program: &dyn crate::CheckingInput,
        target_state_symbol: SymbolHandle,
    ) -> Option<(SymbolHandle, SymbolHandle)> {
        let machine = program.machine_holding_state(target_state_symbol)?;
        if machine.body_is_present
            || !machine.lifetime_parameters.is_empty()
            || !program.machine_type_parameters(machine).is_empty()
            || !matches!(
                machine.supply_mode,
                MachineSupplyMode::ExternalRealization {
                    binding: None,
                    mechanism: None,
                }
            )
        {
            return None;
        }
        let [state] = program.machine_states(machine) else {
            return None;
        };
        let [conformance] = program.machine_trait_conformances(machine) else {
            return None;
        };
        if state.symbol != target_state_symbol
            || conformance.external_binding.is_some()
            || !conformance.via_expression.is_valid()
            || !conformance.trait_lifetime_arguments.is_empty()
            || !program
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .is_empty()
        {
            return None;
        }
        let (definition, requirement) = satisfied_trait_parts(program, machine, conformance)?;
        (definition.symbol == conformance.symbol
            && definition.is_boundary
            && definition.lifetime_parameters.is_empty()
            && program.trait_type_parameters(definition).is_empty()
            && requirement.symbol == conformance.requirement_symbol
            && requirement.lifetime_parameters.is_empty()
            && program
                .state_signature_type_parameters(requirement)
                .is_empty()
            && requirement.native_callback_parameters.is_empty()
            && !requirement.suspends
            && same_runtime_signature(
                program,
                (program.state_parameters(state), state.return_type),
                (
                    program.state_signature_parameters(requirement),
                    requirement.return_type,
                ),
            ))
        .then_some((requirement.symbol, machine.attached_data_symbol))
    }

    fn satisfied_trait_parts<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        conformance: &'program typed_trees::machine::TraitConformance,
    ) -> Option<(
        &'program typed_trees::trait_definition::TraitDefinition,
        &'program typed_trees::signature::StateSignature,
    )> {
        let typed_trees::machine::SatisfiedDeclaration::Trait {
            definition,
            requirement,
        } = typed_trees::machine::resolve_satisfied_declaration(program, machine, conformance)?
        else {
            return None;
        };
        Some((definition, requirement))
    }

    fn same_runtime_signature(
        program: &dyn crate::CheckingInput,
        (leaf_parameters, leaf_result): (&[StateParameter], TypeReferenceHandle),
        (requirement_parameters, requirement_result): (&[StateParameter], TypeReferenceHandle),
    ) -> bool {
        leaf_parameters.len() == requirement_parameters.len()
            && leaf_parameters
                .iter()
                .zip(requirement_parameters)
                .all(|(leaf, requirement)| {
                    leaf.is_const == requirement.is_const
                        && leaf.is_mutable == requirement.is_mutable
                        && leaf.relevance == requirement.relevance
                        && program.normalized_type_identity(leaf.type_reference)
                            == program.normalized_type_identity(requirement.type_reference)
                })
            && program.normalized_type_identity(leaf_result)
                == program.normalized_type_identity(requirement_result)
    }

    pub fn exact_compiler_intrinsic_boundary_requirement(
        program: &dyn crate::CheckingInput,
        target_state_symbol: SymbolHandle,
    ) -> Option<(SymbolHandle, SymbolHandle)> {
        let machine = program.machine_holding_state(target_state_symbol)?;
        if !program.machine_type_parameters(machine).is_empty() {
            return None;
        }
        let (authored_binding, inferred_hosted_intrinsic) =
            intrinsic_realization_candidate(program, machine)?;
        let inferred_trait_name = inferred_hosted_intrinsic.map(|(_, trait_name)| trait_name);
        let [state] = program.machine_states(machine) else {
            return None;
        };
        let byte_result = (machine.name.as_str() == "ConsoleNativeProvider::read_byte"
            && program.state_parameters(state).is_empty())
        .then(|| exact_byte_read_result_type(program, state.return_type))
        .flatten();
        if state.symbol != target_state_symbol
            || !(exact_direct_intrinsic_signature(
                program,
                program.state_parameters(state),
                state.return_type,
            ) || byte_result.is_some())
        {
            return None;
        }

        let mut matches = program
            .machine_trait_conformances(machine)
            .iter()
            .filter_map(|conformance| {
                if ((inferred_trait_name.is_some()
                    && (conformance.external_binding.is_some()
                        || conformance.via_expression.is_valid()
                        || conformance.external_binding_source_span.is_some()))
                    || (inferred_trait_name.is_none()
                        && conformance.external_binding != authored_binding))
                    || conformance.requirement.is_none()
                    || !program
                        .type_reference_table
                        .type_reference_handles(conformance.arguments)
                        .is_empty()
                {
                    return None;
                }
                let (definition, requirement) =
                    satisfied_trait_parts(program, machine, conformance)?;
                let provider_prefix = inferred_hosted_intrinsic
                    .map(|(provider_name, _)| provider_name)
                    .unwrap_or_default();
                (definition.symbol == conformance.symbol
                    && definition.is_boundary
                    && inferred_trait_name
                        .is_none_or(|trait_name| definition.name.as_str() == trait_name)
                    && definition.lifetime_parameters.is_empty()
                    && program.trait_type_parameters(definition).is_empty()
                    && requirement.symbol == conformance.requirement_symbol
                    && (inferred_trait_name.is_none()
                        || machine
                            .name
                            .as_str()
                            .strip_prefix(provider_prefix)
                            .and_then(|suffix| suffix.strip_prefix("::"))
                            == Some(requirement.name.as_str()))
                    && requirement.lifetime_parameters.is_empty()
                    && program
                        .state_signature_type_parameters(requirement)
                        .is_empty()
                    && requirement.native_callback_parameters.is_empty()
                    && !requirement.suspends
                    && byte_result.is_some() == requirement.blocks
                    && ((byte_result.is_none()
                        && exact_direct_intrinsic_signature(
                            program,
                            program.state_signature_parameters(requirement),
                            requirement.return_type,
                        ))
                        || (byte_result.is_some()
                            && definition.name.as_str() == "Console"
                            && requirement.name.as_str() == "read_byte"
                            && program.state_signature_parameters(requirement).is_empty()
                            && exact_byte_read_result_type(program, requirement.return_type)
                                == byte_result)))
                    .then_some(requirement.symbol)
            });
        let requirement = matches.next()?;
        matches
            .next()
            .is_none()
            .then_some((requirement, machine.attached_data_symbol))
    }

    fn intrinsic_realization_candidate(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Option<(
        Option<language_semantics::ExternalBindingId>,
        Option<(&'static str, &'static str)>,
    )> {
        if machine.body_is_present || !machine.lifetime_parameters.is_empty() {
            return None;
        }
        let authored_binding = match machine.supply_mode {
            MachineSupplyMode::ExternalRealization {
                binding: Some(binding),
                mechanism: Some(language_semantics::ExternalBindingMechanism::CompilerIntrinsic),
            } if program.external_bindings.identity(binding)
                == Some(&language_semantics::ExternalBindingIdentity::CompilerIntrinsic) =>
            {
                Some(binding)
            }
            _ => None,
        };
        let inferred_hosted_intrinsic = match machine.name.as_str() {
            "ConsoleNativeProvider::exit_process" | "ConsoleNativeProvider::write_byte" => {
                Some(("ConsoleNativeProvider", "Console"))
            }
            "ProcessExitNativeProvider::exit_process" => {
                Some(("ProcessExitNativeProvider", "ProcessExit"))
            }
            _ => None,
        }
        .filter(|(provider_name, _)| {
            machine.supply_mode == MachineSupplyMode::Boundary
                && machine.attached_data.as_ref().map(|name| name.as_str()) == Some(*provider_name)
        });
        (authored_binding.is_some() || inferred_hosted_intrinsic.is_some())
            .then_some((authored_binding, inferred_hosted_intrinsic))
    }

    pub fn checked_shape_data_declaration<'program>(
        program: &'program dyn crate::CheckingInput,
        result_type: TypeReferenceHandle,
        name: &str,
    ) -> Option<(&'program typed_trees::data::DataDefinition, SymbolHandle)> {
        let symbol = &program
            .type_reference_table
            .type_reference(result_type)
            .as_named_symbol()?;
        let data = crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)?;
        if !symbol.is_valid()
            || data.name.as_str() != name
            || data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || !data.lifetime_parameters.is_empty()
            || !program.data_type_parameters(data).is_empty()
            || data.generic_instance.is_some()
            || data.quotient.is_some()
            || !data.requires_facts.is_empty()
            || data.zero_gated
            || data.properties.carry.is_some()
            || data.properties.multiplicity == language_semantics::Multiplicity::Linear
        {
            return None;
        }
        Some((data, *symbol))
    }

    pub fn exact_byte_read_result_type(
        program: &dyn crate::CheckingInput,
        result_type: TypeReferenceHandle,
    ) -> Option<SymbolHandle> {
        use typed_trees::data::DataMember;

        let (data, symbol) = checked_shape_data_declaration(program, result_type, "ByteRead")?;
        let [DataMember::Variant(eof), DataMember::Variant(byte)] = program.data_members(data)
        else {
            return None;
        };
        if eof.name.as_str() != "Eof"
            || !eof.payload.is_empty()
            || !eof.requires_facts.is_empty()
            || byte.name.as_str() != "Byte"
        {
            return None;
        }
        let [field] = program.data_payload_fields(byte) else {
            return None;
        };
        if field.name.as_str() != "value"
            || field.relevance.is_erased()
            || !matches!(
                program
                    .type_reference_table
                    .type_reference(field.type_reference),
                TypeReferenceNode::Named { .. }
            )
            || program.primitive_type_reference(field.type_reference) != Some(PrimitiveType::I32)
        {
            return None;
        }
        crate::case_requires_field_intervals(program, byte)?
            .iter()
            .all(|(_, minimum, maximum)| {
                minimum.is_none_or(|minimum| minimum <= 0)
                    && maximum.is_none_or(|maximum| maximum >= 255)
            })
            .then_some(symbol)
    }
    pub fn boundary_requirement_completion(
        program: &dyn crate::CheckingInput,
        requirement: SymbolHandle,
    ) -> semantic_vocabulary::BoundaryCompletion {
        typed_trees::boundary_completions::boundary_requirement_completion(
            program.typed_program(),
            requirement,
        )
    }

    pub fn is_canonical_process_exit_requirement(
        program: &dyn crate::CheckingInput,
        trait_symbol: SymbolHandle,
        requirement: SymbolHandle,
    ) -> bool {
        typed_trees::boundary_completions::is_canonical_process_exit_requirement(
            program.typed_program(),
            trait_symbol,
            requirement,
        )
    }

    pub fn valid_boundary_completion_declaration(
        program: &dyn crate::CheckingInput,
        declaration: &typed_trees::BoundaryCompletionDeclaration,
    ) -> bool {
        typed_trees::boundary_completions::valid_boundary_completion_declaration(
            program.typed_program(),
            declaration,
        )
    }

    pub fn validate_boundary_completion_declarations(
        program: &dyn crate::CheckingInput,
    ) -> Result<(), &'static str> {
        typed_trees::boundary_completions::validate_boundary_completion_declarations(
            program.typed_program(),
        )
    }
}

pub mod locals {
    use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
    use diagnostics::Diagnostic;
    use language_core::is_self_receiver;
    use symbols::SymbolKind;
    use typed_trees::signature::StateParameter;
    use typed_trees::statement::StatementNode;
    use typed_trees::{machine::Machine, state::State};
    pub(crate) mod type_value_scope {
        use diagnostics::Diagnostic;
        use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

        use super::StateValueScope;

        impl StateValueScope<'_, '_> {
            pub(crate) fn type_reference(
                &self,
                reference: TypeReferenceHandle,
                diagnostics: &mut Vec<Diagnostic>,
            ) {
                let table = &self.program.type_reference_table;
                match table.type_reference(reference) {
                    TypeReferenceNode::Reference { referee, .. } => {
                        self.type_reference(*referee, diagnostics)
                    }
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => {
                        self.type_reference(*element_type, diagnostics)
                    }
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        self.type_reference(*base_type, diagnostics);
                        for constraint in table.constraints(*constraints) {
                            match constraint {
                                TypeConstraintNode::Domain(domain) => {
                                    for argument in &domain.arguments {
                                        self.type_reference(*argument, diagnostics);
                                    }
                                }
                                TypeConstraintNode::Named(_)
                                | TypeConstraintNode::ArithmeticDomain(_) => {}
                            }
                        }
                    }
                    TypeReferenceNode::Generic { arguments, .. } => {
                        for argument in table.type_reference_handles(*arguments) {
                            self.type_reference(*argument, diagnostics);
                        }
                    }
                    TypeReferenceNode::ConstExpression(expression) => {
                        self.expression(*expression, diagnostics)
                    }
                    _ => {}
                }
            }
        }
    }
    pub(crate) mod value_scope {
        use crate::declarations::symbols::TopLevelSymbols;
        use diagnostics::Diagnostic;
        use typed_trees::{
            expression::{ExpressionHandle, ExpressionNode},
            machine::Machine,
            state::State,
        };

        pub(crate) struct StateValueScope<'program, 'symbols> {
            pub(crate) program: &'program dyn crate::CheckingInput,
            pub(crate) machine: &'program Machine,
            pub(crate) state: &'program State,
            pub(crate) symbols: &'symbols TopLevelSymbols<'program>,
            pub(crate) prior_statements: &'symbols [typed_trees::statement::StatementNode],
            pub(crate) context: &'static str,
        }

        impl StateValueScope<'_, '_> {
            pub(crate) fn expression(
                &self,
                expression: ExpressionHandle,
                diagnostics: &mut Vec<Diagnostic>,
            ) {
                if !expression.is_valid() {
                    return;
                }
                let table = &self.program.expression_table;
                match table.expression(expression) {
                    ExpressionNode::Match(dispatch) => {
                        for child in crate::queries::expression_traversal::match_children(
                            self.program,
                            *dispatch,
                        ) {
                            self.expression(child, diagnostics);
                        }
                    }
                    ExpressionNode::Name(path) => {
                        let members = table.name_path_members(path.members);
                        let Some(name) = members.first() else {
                            return;
                        };
                        let root = if path.head_symbol.is_valid() {
                            path.head_symbol
                        } else {
                            table
                                .name_path_member_symbols(path.member_symbols)
                                .first()
                                .copied()
                                .filter(|symbol| symbol.is_valid())
                                .unwrap_or(path.symbol)
                        };
                        let bare_identity_matches = members.len() != 1
                            || !path.head_symbol.is_valid()
                            || path.head_symbol == path.symbol;
                        if !bare_identity_matches
                            || !crate::value_custody::locals::state_value_root_is_known(
                                self.program,
                                self.machine,
                                self.state,
                                self.prior_statements,
                                self.symbols,
                                root,
                                name.as_str(),
                            )
                        {
                            diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{}` {} value `{}` is not in the state's explicit parameter scope or prior local declarations",
                        self.machine.name, self.state.name, self.context, name,
                    )));
                        }
                    }
                    ExpressionNode::Binary(binary) => {
                        self.expression(binary.left, diagnostics);
                        self.expression(binary.right, diagnostics);
                    }
                    ExpressionNode::Unary(unary) => self.expression(unary.operand, diagnostics),
                    ExpressionNode::Borrow(borrow) => self.expression(borrow.target, diagnostics),
                    ExpressionNode::Cast(cast) => {
                        self.expression(cast.value, diagnostics);
                        self.type_reference(cast.target_type, diagnostics);
                    }
                    ExpressionNode::Member(member) => self.expression(member.receiver, diagnostics),
                    ExpressionNode::Indexed(indexed) => {
                        self.expression(indexed.collection, diagnostics);
                        self.expression(indexed.index, diagnostics);
                    }
                    ExpressionNode::Range(range) => {
                        self.expression(range.start, diagnostics);
                        self.expression(range.end, diagnostics);
                    }
                    ExpressionNode::Atomic(atomic) => {
                        self.expression(atomic.value, diagnostics);
                        self.expression(atomic.result, diagnostics);
                    }
                    ExpressionNode::Call(call) => {
                        if !crate::declarations::operators::namespaces::expression_call_has_operator_namespace(
                    self.program,
                    call,
                ) {
                    self.expression(call.receiver, diagnostics);
                }
                        for argument in table.expression_handles(call.arguments) {
                            self.expression(*argument, diagnostics);
                        }
                    }
                    ExpressionNode::ArrayLiteral(elements) => {
                        for element in table.expression_handles(*elements) {
                            self.expression(*element, diagnostics);
                        }
                    }
                    ExpressionNode::StructLiteral(literal) => {
                        for field in table.struct_fields(literal.fields) {
                            self.expression(field.value, diagnostics);
                        }
                    }
                    ExpressionNode::SavedSubject(_)
                    | ExpressionNode::Boolean(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::UndeterminedNaN(_)
                    | ExpressionNode::Integer(_)
                    | ExpressionNode::String(_) => {}
                    ExpressionNode::TypeExpression(reference)
                    | ExpressionNode::ZeroValue(reference) => {
                        self.type_reference(*reference, diagnostics)
                    }
                }
            }
        }
    }
    pub(crate) use value_scope::StateValueScope;

    pub(crate) fn state_binding_type(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        prior_statements: &[StatementNode],
        symbol: symbols::SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        if !symbol.is_valid() {
            return None;
        }
        let declaration = program.symbols.get(symbol);
        if declaration.parent != state.symbol {
            return None;
        }
        match declaration.kind {
            SymbolKind::Parameter => program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
                .map(|parameter| parameter.type_reference),
            SymbolKind::Local => prior_statements
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.symbol == symbol => {
                        Some(local.type_reference)
                    }
                    _ => None,
                }),
            _ => None,
        }
    }

    pub(crate) fn state_value_root_is_known(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        prior_statements: &[StatementNode],
        symbols: &TopLevelSymbols<'_>,
        root: symbols::SymbolHandle,
        name: &str,
    ) -> bool {
        let kind = program.symbols.get(root).kind;
        if is_self_receiver(name) || matches!(kind, SymbolKind::Local | SymbolKind::Parameter) {
            return crate::value_custody::locals::state_binding_type(
                program,
                state,
                prior_statements,
                root,
            )
            .is_some();
        }
        if root.is_valid() && kind == SymbolKind::Unknown {
            return false;
        }
        if root.is_valid() && kind == SymbolKind::Const {
            return program
                .const_declarations()
                .iter()
                .filter(|declaration| declaration.symbol == root)
                .count()
                == 1;
        }
        if root.is_valid()
            && matches!(
                kind,
                SymbolKind::Conformance | SymbolKind::ConformanceParameter
            )
        {
            return true;
        }
        if symbols.has_type_symbol(root)
            || symbols.has_type(name)
            || symbols.machine(name).is_some()
            || symbols.trait_definition(name).is_some()
        {
            return true;
        }
        let (scope, _) =
            program.machine_type_reference_scope(machine, |left, right| left.name == right.name);
        if scope
            .iter()
            .filter(|parameter| {
                parameter.kind.is_const_or_value()
                    && if root.is_valid() {
                        parameter.symbol == root
                    } else {
                        parameter.name.as_str() == name
                    }
            })
            .count()
            == 1
        {
            return true;
        }
        if let Some(attached_data) = &machine.attached_data
            && program.data_definitions().iter().any(|definition| {
                definition.name == *attached_data
                    && program
                        .data_type_parameters(definition)
                        .iter()
                        .any(|parameter| {
                            parameter.name.as_str() == name && parameter.kind.is_const_or_value()
                        })
            })
        {
            return true;
        }
        for definition in program.data_definitions() {
            for member in program.data_members(definition) {
                if let Some(variant) = member.as_variant()
                    && variant.name.as_str() == name
                {
                    return true;
                }
            }
        }
        crate::value_custody::locals::state_binding_type(program, state, prior_statements, root)
            .is_some()
    }

    pub(crate) struct WritableRoots<'program, 'state> {
        pub(crate) program: &'program dyn crate::CheckingInput,
        pub(crate) machine: &'program Machine,
        pub(crate) machine_symbols: &'state MachineSymbols<'program>,
        pub(crate) statements: &'state [StatementNode],
        pub(crate) parameters: &'state [StateParameter],
    }

    impl WritableRoots<'_, '_> {
        pub(crate) fn contains_for_write(&self, root_name: &str, bare_reassignment: bool) -> bool {
            (self.machine_symbols.has_owned_data(root_name)
                && self
                    .program
                    .machine_owned_data(self.machine)
                    .iter()
                    .any(|owned| owned.name.as_str() == root_name))
                || self.statements.iter().any(|statement| {
                    let Some(local_data) = statement.as_local_data() else {
                        return false;
                    };
                    if local_data.name.as_str() != root_name {
                        return false;
                    }
                    if !bare_reassignment {
                        return true;
                    }
                    local_data.is_mutable || local_is_mutable_reference(self.program, local_data)
                })
                || self
                    .parameters
                    .iter()
                    .any(|parameter| parameter.is_mutable && parameter.name.as_str() == root_name)
        }

        pub(crate) fn contains_for_shared_access(&self, root_name: &str) -> bool {
            (self.machine_symbols.has_owned_data(root_name)
            && (self
                .program
                .machine_owned_data(self.machine)
                .iter()
                .any(|owned| owned.name.as_str() == root_name)))
            || self.statements.iter().any(|statement| {
                matches!(
                    statement,
                    StatementNode::LocalData(local_data) if local_data.name.as_str() == root_name
                )
            })
            || self
                .parameters
                .iter()
                .any(|parameter| parameter.name.as_str() == root_name)
        }
    }

    pub(crate) fn local_is_mutable_reference(
        program: &dyn crate::CheckingInput,
        local_data: &typed_trees::statement::TableLocalData,
    ) -> bool {
        use typed_trees::types::TypeReferenceNode;
        if !local_data.type_reference.is_valid() {
            return false;
        }
        matches!(
            program
                .type_reference_table
                .type_reference(local_data.type_reference),
            TypeReferenceNode::Reference { access, .. } if access.is_exclusive()
        )
    }

    pub(crate) fn validate_local_data_names(
        statements: &[StatementNode],
        machine_symbols: &MachineSymbols<'_>,
        parameters: &[StateParameter],
        machine_name: &str,
        state_name: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (statement_index, statement) in statements.iter().enumerate() {
            let Some(local_data) = statement.as_local_data() else {
                continue;
            };
            if !local_data.role.is_runtime_binding() {
                continue;
            }

            if machine_symbols.has_member(local_data.name.as_str())
                || parameters
                    .iter()
                    .any(|parameter| parameter.name == local_data.name)
            {
                diagnostics.push(Diagnostic::error(format!(
                "machine `{machine_name}` state `{state_name}` local data `{}` conflicts with an existing name",
                local_data.name
            )));
                continue;
            }

            if statements[..statement_index].iter().any(|previous| {
                matches!(
                    previous,
                    StatementNode::LocalData(previous)
                        if previous.role.is_runtime_binding()
                            && previous.name == local_data.name
                )
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{machine_name}` state `{state_name}` has duplicate local data `{}`",
                    local_data.name
                )));
            }
        }
    }

    #[cfg(test)]
    pub(crate) mod value_scope_tests {
        use super::*;
        use symbols::{SymbolNameRef, SymbolTableBuilder};
        use typed_trees::{TypedTrees, constant::ConstDeclaration, name::Identifier};

        #[test]
        fn declaration_constants_keep_exact_scope_without_admitting_foreign_storage() {
            for copies in [0, 1, 2] {
                let mut builder = SymbolTableBuilder::default();
                let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
                let roots = builder.insert_children(
                    root,
                    symbols::builtin_type_symbols().into_iter().chain([
                        (SymbolKind::Const, SymbolNameRef::Static("ZERO")),
                        (SymbolKind::Machine, SymbolNameRef::Static("main")),
                        (SymbolKind::Machine, SymbolNameRef::Static("other")),
                    ]),
                );
                let constant = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
                let machine_symbol = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
                let other = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 2);
                let state_symbol = builder
                    .insert_children(
                        machine_symbol,
                        [(SymbolKind::State, SymbolNameRef::Static("entry"))],
                    )
                    .start();
                let foreign_state = builder
                    .insert_children(other, [(SymbolKind::State, SymbolNameRef::Static("entry"))])
                    .start();
                let foreign = builder.insert_children(
                    foreign_state,
                    [
                        (SymbolKind::Local, SymbolNameRef::Static("ZERO")),
                        (SymbolKind::Parameter, SymbolNameRef::Static("ZERO")),
                    ],
                );
                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..Default::default()
                };
                for _ in 0..copies {
                    program.push_const_declaration(ConstDeclaration {
                        symbol: constant,
                        pending_target_observation: true,
                        ..Default::default()
                    });
                }
                let machine = Machine {
                    symbol: machine_symbol,
                    name: Identifier::generated("main"),
                    ..Default::default()
                };
                let state = State {
                    symbol: state_symbol,
                    name: Identifier::generated("entry"),
                    ..Default::default()
                };
                let symbols = TopLevelSymbols::build(&program, &mut Vec::new());
                let known = |root| {
                    state_value_root_is_known(
                        &program,
                        &machine,
                        &state,
                        &[],
                        &symbols,
                        root,
                        "ZERO",
                    )
                };
                assert_eq!(known(constant), copies == 1);
                assert!(!known(foreign.handle_at(0)));
                assert!(!known(foreign.handle_at(1)));
            }
        }
    }
}

pub mod logical_matches {
    use crate::frozen_program::expression_table;
    use arena::Handle;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{
        CompletedValueCapture, ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern,
        TableSavedSubject, ValueCaptureOwner,
    };
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, Copy)]
    pub struct LogicalRecordCapture {
        definition: ExpressionHandle,
        capture: Handle<CompletedValueCapture>,
        subject: ExpressionHandle,
        anchor: ExpressionHandle,
        head: ExpressionHandle,
        arm_value: ExpressionHandle,
    }

    impl LogicalRecordCapture {
        pub fn anchor(self) -> ExpressionHandle {
            self.anchor
        }

        pub fn arm_value(self) -> ExpressionHandle {
            self.arm_value
        }

        pub fn permits_alias(self, program: &TypedTrees, saved: &TableSavedSubject) -> bool {
            saved.capture == self.capture
                && saved.pattern_head == self.head
                && program.saved_value_source(saved) == Some(self.subject)
                && expression_table(program)
                    .value_capture(saved.capture)
                    .definition
                    == self.definition
                && expression_table(program)
                    .value_capture(saved.capture)
                    .value_anchor
                    == self.anchor
        }
    }

    pub fn logical_record_match_capture(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        subject_type: TypeReferenceHandle,
    ) -> Option<LogicalRecordCapture> {
        if !expression_table(program).expression_is_valid(expression) {
            return None;
        }
        let ExpressionNode::Match(dispatch) = expression_table(program).expression(expression)
        else {
            return None;
        };
        let [arm] = expression_table(program).match_arms(dispatch.arms) else {
            return None;
        };
        let MatchPattern::Destructure(pattern) = arm.pattern else {
            return None;
        };
        if pattern.guard.is_valid() || !pattern.proof_selectors.is_empty() {
            return None;
        }
        if pattern.condition.is_valid()
            && !matches!(
                expression_table(program).expression(pattern.condition),
                ExpressionNode::Boolean(true)
            )
        {
            return None;
        }
        let data = crate::value_custody::places::data_definition_for_type(program, subject_type)?;
        if data.quotient.is_some()
            || program
                .data_members(data)
                .iter()
                .any(|member| member.as_variant().is_some())
            || expression_table(program)
                .name_path_node(pattern.head)?
                .symbol
                != data.symbol
        {
            return None;
        }
        let fields = expression_table(program).match_fields(pattern.fields);
        let declared = program
            .data_members(data)
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => Some(field),
                _ => None,
            })
            .collect::<Vec<_>>();
        if (!pattern.has_rest && fields.len() != declared.len())
            || fields.iter().enumerate().any(|(position, field)| {
                matches!(field.action, MatchFieldAction::Equal(_))
                    || fields[..position]
                        .iter()
                        .any(|prior| prior.field_symbol == field.field_symbol)
                    || !declared.iter().any(|declared| {
                        declared.symbol == field.field_symbol
                            && declared.name.as_str() == field.member.as_str()
                    })
            })
        {
            return None;
        }
        if !expression_table(program).contains_value_capture(dispatch.capture) {
            return None;
        }
        let capture = expression_table(program).value_capture(dispatch.capture);
        if capture.owner != ValueCaptureOwner::Match
            || capture.definition != expression
            || capture.subject != dispatch.subject
            || !expression_table(program).expression_is_valid(capture.value_anchor)
        {
            return None;
        }
        let binding = LogicalRecordCapture {
            definition: expression,
            capture: dispatch.capture,
            subject: dispatch.subject,
            anchor: capture.value_anchor,
            head: pattern.head,
            arm_value: arm.value,
        };
        if capture.value_anchor != capture.subject {
            let ExpressionNode::SavedSubject(saved) =
                expression_table(program).expression(capture.value_anchor)
            else {
                return None;
            };
            if !binding.permits_alias(program, saved) {
                return None;
            }
        }
        Some(binding)
    }

    pub fn expression_has_closed_value_captures(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> bool {
        if !expression_table(program).expression_is_valid(expression) {
            return false;
        }
        let mut expressions = Vec::new();
        crate::queries::expression_traversal::collect_expression_tree(
            program,
            expression,
            &mut expressions,
        );
        let mut position = 0;
        while position < expressions.len() {
            let current = expressions[position];
            position += 1;
            if let ExpressionNode::Match(dispatch) = expression_table(program).expression(current) {
                let dependencies = expression_table(program)
                    .match_arms(dispatch.arms)
                    .iter()
                    .flat_map(|arm| {
                        expression_table(program).match_pattern_dependencies(arm.pattern)
                    })
                    .collect::<Vec<_>>();
                for dependency in dependencies {
                    crate::queries::expression_traversal::collect_expression_tree(
                        program,
                        dependency,
                        &mut expressions,
                    );
                }
            }
        }
        expressions.iter().all(|expression| {
            let ExpressionNode::SavedSubject(saved) =
                expression_table(program).expression(*expression)
            else {
                return true;
            };
            if program.saved_value_source(saved).is_none() {
                return false;
            }
            let capture = expression_table(program).value_capture(saved.capture);
            let owner = match capture.owner {
                ValueCaptureOwner::Match => capture.definition,
                ValueCaptureOwner::ExpressionArgument { call, .. } => call,
                ValueCaptureOwner::StatementArgument { .. } => return false,
            };
            expressions.contains(&owner)
                && expressions.contains(&capture.subject)
                && expressions.contains(&capture.value_anchor)
                && (!saved.pattern_head.is_valid() || expressions.contains(&saved.pattern_head))
        })
    }

    #[derive(Clone)]
    pub struct LogicalMatchBinding<Subject> {
        pub capture: LogicalRecordCapture,
        pub observed: Subject,
    }

    pub fn logical_saved_subject_projection<Subject: Clone>(
        program: &TypedTrees,
        expression: ExpressionHandle,
        bindings: &[LogicalMatchBinding<Subject>],
    ) -> Option<(Subject, Vec<facts::PlaceSegment>)> {
        let mut leaf = expression;
        for _ in 0..64 {
            match expression_table(program).expression(leaf) {
                ExpressionNode::Member(member) => leaf = member.receiver,
                ExpressionNode::Indexed(indexed) => leaf = indexed.collection,
                ExpressionNode::Borrow(borrow) => leaf = borrow.target,
                ExpressionNode::SavedSubject(saved) => {
                    let place = crate::canonical_place_from_expression(program, expression)?;
                    let facts::PlaceRoot::Expression(anchor) = place.root else {
                        return None;
                    };
                    let binding = bindings.iter().rev().find(|binding| {
                        binding.capture.anchor() == anchor
                            && binding.capture.permits_alias(program, saved)
                    })?;
                    return Some((binding.observed.clone(), place.segments));
                }
                _ => return None,
            }
        }
        None
    }
}

pub mod membership_evidence {
    use crate::CanonicalPlace;
    use crate::frozen_program::expression_table;
    use facts::{FactContextHandle, FactPlan, PlaceSegment};
    use language_semantics::SemanticDomainId;
    use symbols::SymbolHandle;

    pub(crate) struct Membership {
        place: CanonicalPlace,
        domain: SymbolHandle,
        instance: SemanticDomainId,
    }

    pub struct MembershipEvidence<'checking> {
        program: &'checking dyn crate::CheckingInput,
        _semantic: &'checking FactPlan,
        rows: Vec<Membership>,
        flexible: Vec<usize>,
    }

    impl<'checking> MembershipEvidence<'checking> {
        pub fn prepare(
            program: &'checking dyn crate::CheckingInput,
            semantic: &'checking FactPlan,
            contexts: &[FactContextHandle],
        ) -> Self {
            let mut rows = Vec::new();
            for handle in semantic.unique_context_facts(contexts.iter().copied()) {
                let fact = semantic.facts.get(handle);
                let Some((_, _, domain, instance)) = fact.payload.domain_membership() else {
                    continue;
                };
                let Some(place) = fact.place.as_place().and_then(|place| {
                    canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(place),
                    )
                }) else {
                    continue;
                };
                rows.push(Membership {
                    place,
                    domain,
                    instance,
                });
            }
            rows.sort_unstable_by(|left, right| {
                left.place.compare_identity(&right.place).then_with(|| {
                    domain_key(left.domain, left.instance)
                        .cmp(&domain_key(right.domain, right.instance))
                })
            });
            let flexible = rows
                .iter()
                .enumerate()
                .filter_map(|(position, row)| {
                    row.place
                        .segments
                        .iter()
                        .any(|segment| {
                            matches!(
                                segment,
                                PlaceSegment::Index { .. } | PlaceSegment::FixedRange { .. }
                            )
                        })
                        .then_some(position)
                })
                .collect();
            Self {
                program,
                _semantic: semantic,
                rows,
                flexible,
            }
        }

        pub fn contains(
            &self,
            subject: &CanonicalPlace,
            domain: SymbolHandle,
            instance: SemanticDomainId,
        ) -> bool {
            if !instance.is_valid() {
                return false;
            }
            let matches = |row: &Membership| {
                row.domain == domain
                    && row.instance == instance
                    && row.place.root == subject.root
                    && segments_cover_subject(self.program, &row.place.segments, &subject.segments)
            };
            if self
                .rows
                .binary_search_by(|row| {
                    row.place.compare_identity(subject).then_with(|| {
                        domain_key(row.domain, row.instance).cmp(&domain_key(domain, instance))
                    })
                })
                .ok()
                .is_some_and(|position| matches(&self.rows[position]))
            {
                return true;
            }
            if subject
                .segments
                .iter()
                .any(|segment| matches!(segment, PlaceSegment::Index { .. }))
            {
                self.rows.iter().any(matches)
            } else {
                self.flexible
                    .iter()
                    .any(|position| matches(&self.rows[*position]))
            }
        }
    }

    fn domain_key(domain: SymbolHandle, instance: SemanticDomainId) -> (u32, u32, u32) {
        (domain.arena_index(), domain.generation(), instance.0)
    }

    pub fn segments_cover_subject(
        program: &dyn crate::CheckingInput,
        candidate: &[facts::PlaceSegment],
        subject: &[facts::PlaceSegment],
    ) -> bool {
        candidate.len() == subject.len()
            && candidate
                .iter()
                .zip(subject.iter())
                .all(|(candidate, subject)| {
                    crate::storage_path_relations::canonical_place_segments_equal(
                        program, *candidate, *subject,
                    ) || segment_covers_subject(program, *candidate, *subject)
                })
    }

    fn segment_covers_subject(
        program: &dyn crate::CheckingInput,
        candidate: facts::PlaceSegment,
        subject: facts::PlaceSegment,
    ) -> bool {
        let facts::PlaceSegment::FixedRange { start, end } = candidate else {
            return false;
        };
        if start >= end {
            return false;
        }
        match subject {
            facts::PlaceSegment::FixedIndex { index } => start <= index && index < end,
            facts::PlaceSegment::FixedRange {
                start: subject_start,
                end: subject_end,
            } => subject_start < subject_end && start <= subject_start && subject_end <= end,
            facts::PlaceSegment::Index { expression } => {
                if start == 0 && end == usize::MAX {
                    true
                } else {
                    expression_table(program)
                        .constant_integer_value(expression)
                        .and_then(|value| usize::try_from(value).ok())
                        .is_some_and(|index| start <= index && index < end)
                }
            }
            _ => false,
        }
    }

    pub fn canonical_place_from_semantic_place(
        program: &dyn crate::CheckingInput,
        semantic: &FactPlan,
        place: &facts::Place,
    ) -> Option<CanonicalPlace> {
        let mut canonical = match place.root {
            facts::PlaceRoot::Unknown => return None,
            facts::PlaceRoot::Symbol(symbol) => symbol.is_valid().then_some(CanonicalPlace {
                root: facts::PlaceRoot::Symbol(symbol),
                segments: Vec::new(),
            })?,
            root @ (facts::PlaceRoot::InvocationReferent { .. }
            | facts::PlaceRoot::InvocationOutput(_)) => CanonicalPlace {
                root,
                segments: Vec::new(),
            },
            facts::PlaceRoot::Expression(expression) => {
                if !expression_table(program).expression_is_valid(expression) {
                    return None;
                }
                CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                }
            }
            facts::PlaceRoot::TypeReference(type_reference) => CanonicalPlace {
                root: facts::PlaceRoot::TypeReference(type_reference),
                segments: Vec::new(),
            },
        };
        canonical.extend_segments(semantic.place_segments.span_or_empty(place.segments));
        Some(canonical)
    }
}

pub mod owned_value_source {
    use crate::frozen_program::expression_table;
    use crate::value_custody::storage_contents::ContentsRequirement;
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::TypeReferenceHandle;

    pub fn value_preserving_cast_operand(
        program: &dyn crate::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let cast = expression_table(program).cast_node(expression)?;
        if cast.form.is_recast()
            || cast.domain != numerics::arithmetic::ArithmeticDomain::Exact
            || program.primitive_type_reference(cast.target_type).is_some()
        {
            return None;
        }
        let actual =
            crate::expression_type_reference_in_state(program, state, statement, cast.value)?;
        let actual_identity = crate::stored_structural_type_identity(program, actual, &[], &[]);
        let target_identity =
            crate::stored_structural_type_identity(program, cast.target_type, &[], &[]);
        (actual_identity.is_valid() && actual_identity == target_identity).then_some(cast.value)
    }

    pub fn ownership_source_expression(
        program: &dyn crate::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        mut expression: ExpressionHandle,
    ) -> ExpressionHandle {
        while let Some(operand) =
            value_preserving_cast_operand(program, state, statement, expression)
        {
            expression = operand;
        }
        expression
    }

    pub fn plain_owned_value_source(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
    ) -> Option<SymbolHandle> {
        whole_owned_value_source(program, expression, reference).filter(|_| {
            crate::has_contents(program, reference, &[], ContentsRequirement::PlainOwned)
        })
    }

    pub fn whole_owned_value_source(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
    ) -> Option<SymbolHandle> {
        let path = expression_table(program).name_path_node(expression)?;
        if path.symbol != path.head_symbol
            || expression_table(program)
                .name_path_members(path.members)
                .len()
                != 1
            || !matches!(
                program.symbols.get(path.symbol).kind,
                SymbolKind::Local | SymbolKind::Parameter
            )
        {
            return None;
        }
        let actual =
            crate::value_custody::expression_types::named_value_type_reference(program, path)?;
        (program.normalized_type_identity(actual) == program.normalized_type_identity(reference))
            .then_some(path.symbol)
    }
}

pub mod partial_affine_residuals {
    use checked_trees::CheckedMemberIdentity;
    use checked_trees::{
        CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralFieldType, CheckedUnitStructuralPathSegment,
        CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape,
        is_partial_affine_field_type,
    };
    use std::collections::BTreeMap;
    use typed_trees::type_identity::TypeIdentityId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CleanupCaseReachability {
        Inactive,
        Selected,
        Runtime,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CleanupCaseCondition {
        pub path: Vec<CheckedUnitStructuralPathSegment>,
        pub type_identity: TypeIdentityId,
        pub case_identity: CheckedMemberIdentity,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CaseSelectedCleanupRegion {
        pub disposition: CheckedUnitPartialAffineDiscardPlan,
        pub conditions: Vec<CleanupCaseCondition>,
    }

    /// Keeps the checked cleanup walk beside its ordered action leaves. A scope
    /// closes only after its own children, reachable cases and common tail.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CaseSelectedCleanupPartition {
        pub regions: Vec<CaseSelectedCleanupRegion>,
        pub nodes: Vec<CleanupPartitionNode>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum CleanupPartitionNode {
        Scope {
            path: Vec<CheckedUnitStructuralPathSegment>,
            type_identity: TypeIdentityId,
            conditions: Vec<CleanupCaseCondition>,
            steps: Vec<usize>,
        },
        Leaf {
            region_ordinal: usize,
        },
        Cases {
            path: Vec<CheckedUnitStructuralPathSegment>,
            type_identity: TypeIdentityId,
            arms: Vec<CleanupPartitionArm>,
            common: Vec<usize>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CleanupPartitionArm {
        pub case_identity: CheckedMemberIdentity,
        pub reachability: CleanupCaseReachability,
        pub steps: Vec<usize>,
    }

    #[derive(Clone, Copy)]
    enum PartitionDestination {
        Scope(usize),
        Arm { node: usize, arm: usize },
        Common(usize),
    }

    pub(crate) struct ResidualTraversal<'selection> {
        select_case: &'selection dyn Fn(
            &[CheckedUnitStructuralPathSegment],
            TypeIdentityId,
            &CheckedMemberIdentity,
        ) -> Option<CleanupCaseReachability>,
        partition_paths: &'selection [Vec<CheckedUnitStructuralPathSegment>],
        conditions: Vec<CleanupCaseCondition>,
        regions: Vec<CaseSelectedCleanupRegion>,
        nodes: Vec<CleanupPartitionNode>,
        destinations: Vec<PartitionDestination>,
        retain_partition: bool,
        max_partition_nodes: usize,
        active_types: Vec<TypeIdentityId>,
    }

    impl ResidualTraversal<'_> {
        fn append_node(&mut self, node: CleanupPartitionNode) -> Option<usize> {
            if self.nodes.len() == self.max_partition_nodes {
                return None;
            }
            let position = self.nodes.len();
            self.nodes.push(node);
            if let Some(destination) = self.destinations.last().copied() {
                match destination {
                    PartitionDestination::Scope(owner) => {
                        let CleanupPartitionNode::Scope { steps, .. } =
                            self.nodes.get_mut(owner)?
                        else {
                            return None;
                        };
                        steps.push(position);
                    }
                    PartitionDestination::Arm { node, arm } => {
                        let CleanupPartitionNode::Cases { arms, .. } = self.nodes.get_mut(node)?
                        else {
                            return None;
                        };
                        arms.get_mut(arm)?.steps.push(position);
                    }
                    PartitionDestination::Common(owner) => {
                        let CleanupPartitionNode::Cases { common, .. } =
                            self.nodes.get_mut(owner)?
                        else {
                            return None;
                        };
                        common.push(position);
                    }
                }
            }
            Some(position)
        }

        fn emit(
            &mut self,
            source: &CheckedUnitStructuralArgumentSourcePlan,
            path: &[CheckedUnitStructuralPathSegment],
            type_identity: TypeIdentityId,
        ) -> Option<()> {
            if self.retain_partition {
                self.append_node(CleanupPartitionNode::Leaf {
                    region_ordinal: self.regions.len(),
                })?;
            }
            self.regions.push(CaseSelectedCleanupRegion {
                disposition: CheckedUnitPartialAffineDiscardPlan {
                    partition_leaf: Default::default(),
                    source: source.clone(),
                    path: path.to_vec(),
                    type_identity,
                    case_conditions: Default::default(),
                },
                conditions: self.conditions.clone(),
            });
            Some(())
        }
    }

    fn has_runtime_storage(field: &checked_trees::CheckedUnitStructuralFieldPlan) -> Option<bool> {
        match (&field.field_type, field.relevance.is_erased()) {
            (CheckedUnitStructuralFieldType::Erased { type_identity }, true) => {
                (type_identity.is_valid() && *type_identity == field.receiving_type_identity)
                    .then_some(false)
            }
            (CheckedUnitStructuralFieldType::Erased { .. }, false) | (_, true) => None,
            (_, false) => Some(true),
        }
    }

    fn validate_type(
        types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
        current_type: TypeIdentityId,
        active: &mut Vec<TypeIdentityId>,
        complete: &mut Vec<TypeIdentityId>,
    ) -> Option<()> {
        if complete.contains(&current_type) {
            return Some(());
        }
        if active.contains(&current_type) {
            return None;
        }
        let declaration = *types.get(&current_type)?;
        active.push(current_type);
        match &declaration.shape {
            CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive)
                if declaration.scalar_restriction.as_ref().map_or_else(
                    || {
                        is_partial_affine_field_type(&CheckedUnitStructuralFieldType::Scalar(
                            *primitive,
                        ))
                    },
                    is_partial_affine_field_type,
                ) => {}
            CheckedUnitStructuralTypeShape::Record { fields, .. } => {
                for (index, field) in fields.iter().enumerate() {
                    if !field.identity.is_valid()
                        || fields[..index]
                            .iter()
                            .any(|earlier| earlier.identity == field.identity)
                    {
                        return None;
                    }
                    if !has_runtime_storage(field)? {
                        continue;
                    }
                    if !is_partial_affine_field_type(&field.field_type)
                        && !field.cleanup_type_identity().is_valid()
                    {
                        return None;
                    }
                    let type_identity = field.cleanup_type_identity();
                    if type_identity.is_valid() {
                        validate_type(types, type_identity, active, complete)?;
                    }
                }
            }
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                length,
            } => {
                if length.literal() != Some(0) {
                    validate_type(types, *element_type_identity, active, complete)?;
                }
            }
            CheckedUnitStructuralTypeShape::Reference {
                referent_identity,
                access,
                ..
            } if *access != checked_trees::CheckedStructuralAccess::Owned
                && referent_identity.is_valid() => {}
            CheckedUnitStructuralTypeShape::ByteSequence(carrier)
                if carrier.is_borrowed()
                    && declaration.descriptor_cleanup
                    && !declaration.owns_linear_claim => {}
            CheckedUnitStructuralTypeShape::BorrowedSliceView { .. }
                if declaration.descriptor_cleanup && !declaration.owns_linear_claim => {}
            CheckedUnitStructuralTypeShape::Sum { cases } => {
                validate_inline_alternatives(&[], cases)?;
            }
            CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                validate_inline_alternatives(fields, cases)?;
            }
            _ => return None,
        }
        active.pop();
        complete.push(current_type);
        Some(())
    }

    fn validate_inline_alternatives(
        common: &[checked_trees::CheckedUnitStructuralFieldPlan],
        cases: &[checked_trees::CheckedUnitStructuralCasePlan],
    ) -> Option<()> {
        let fields_are_inline = |fields: &[checked_trees::CheckedUnitStructuralFieldPlan]| {
            fields.iter().enumerate().all(|(position, field)| {
                field.identity.is_valid()
                    && match has_runtime_storage(field) {
                        Some(false) => true,
                        Some(true) => {
                            is_partial_affine_field_type(&field.field_type)
                                && !matches!(
                                    field.field_type,
                                    CheckedUnitStructuralFieldType::Structural { .. }
                                )
                        }
                        None => false,
                    }
                    && !fields[..position]
                        .iter()
                        .any(|earlier| earlier.identity == field.identity)
            })
        };
        (!cases.is_empty()
            && fields_are_inline(common)
            && cases.iter().enumerate().all(|(position, case)| {
                case.identity.is_valid()
                    && !cases[..position]
                        .iter()
                        .any(|earlier| earlier.identity == case.identity)
                    && fields_are_inline(&case.fields)
            }))
        .then_some(())
    }

    fn visit(
        types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        current_type: TypeIdentityId,
        moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
        traversal: &mut ResidualTraversal<'_>,
        emit: bool,
    ) -> Option<usize> {
        if moved_paths.iter().any(|(path, _)| path.is_empty()) {
            return (moved_paths.len() == 1 && moved_paths[0].1 == current_type).then_some(0);
        }
        if traversal.active_types.contains(&current_type) {
            return None;
        }
        traversal.active_types.push(current_type);
        if emit && traversal.retain_partition {
            let scope = traversal.append_node(CleanupPartitionNode::Scope {
                path: prefix.clone(),
                type_identity: current_type,
                conditions: traversal.conditions.clone(),
                steps: Vec::new(),
            })?;
            traversal
                .destinations
                .push(PartitionDestination::Scope(scope));
        }
        let result = visit_region(
            types,
            source,
            current_type,
            moved_paths,
            boundaries,
            prefix,
            traversal,
            emit,
        );
        traversal.active_types.pop();
        if emit && traversal.retain_partition {
            traversal.destinations.pop();
        }
        result
    }

    fn visit_region(
        types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        current_type: TypeIdentityId,
        moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
        traversal: &mut ResidualTraversal<'_>,
        emit: bool,
    ) -> Option<usize> {
        if moved_paths.is_empty()
            && boundaries.is_empty()
            && !traversal
                .partition_paths
                .iter()
                .any(|path| path.starts_with(prefix) && path.len() > prefix.len())
            && validate_type(types, current_type, &mut Vec::new(), &mut Vec::new()).is_some()
        {
            if emit {
                traversal.emit(source, prefix, current_type)?;
            }
            return Some(1);
        }
        let nominal_boundary = boundaries.iter().any(|(path, _)| path.is_empty());
        if nominal_boundary
            && (!moved_paths.is_empty()
                || boundaries
                    .iter()
                    .any(|(path, identity)| path.is_empty() && identity != &current_type))
        {
            return None;
        }
        if nominal_boundary && emit {
            traversal.emit(source, prefix, current_type)?;
        }
        let declaration = *types.get(&current_type)?;
        let mut count = usize::from(nominal_boundary);
        match &declaration.shape {
            CheckedUnitStructuralTypeShape::Record { fields, .. } => {
                let child_boundaries = boundaries
                    .iter()
                    .copied()
                    .filter(|(path, _)| !path.is_empty())
                    .collect::<Vec<_>>();
                count = count.checked_add(visit_fields(
                    types,
                    source,
                    fields,
                    moved_paths,
                    &child_boundaries,
                    prefix,
                    traversal,
                    emit,
                )?)?;
            }
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                length,
            } => {
                let mut touched = Vec::new();
                for (path, _) in moved_paths {
                    let CheckedUnitStructuralPathSegment::FixedIndex(index) = path[0] else {
                        return None;
                    };
                    if !length.admits_fixed_index(index) {
                        return None;
                    }
                    touched.push(index);
                }
                for (path, _) in boundaries {
                    let [CheckedUnitStructuralPathSegment::FixedIndex(index), ..] = *path else {
                        return None;
                    };
                    if !length.admits_fixed_index(*index) {
                        return None;
                    }
                    touched.push(*index);
                }
                for partition in traversal.partition_paths {
                    if let Some([CheckedUnitStructuralPathSegment::FixedIndex(index), ..]) =
                        partition.strip_prefix(prefix.as_slice())
                    {
                        if !length.admits_fixed_index(*index) {
                            return None;
                        }
                        touched.push(*index);
                    }
                }
                touched.sort_unstable();
                touched.dedup();
                let length = length.literal()?;
                let untouched = usize::try_from(length).ok()?.checked_sub(touched.len())?;
                if untouched > 0 {
                    validate_type(
                        types,
                        *element_type_identity,
                        &mut Vec::new(),
                        &mut Vec::new(),
                    )?;
                }
                if !emit {
                    count = count.checked_add(untouched)?;
                }
                let indexes = (0..if emit { length } else { 0 })
                    .rev()
                    .chain(touched.into_iter().rev().filter(|_| !emit));
                for index in indexes {
                    let matching = moved_paths.iter().filter_map(|(path, moved_type)| {
                    matches!(path[0], CheckedUnitStructuralPathSegment::FixedIndex(candidate) if candidate == index)
                        .then_some((&path[1..], *moved_type))
                }).collect::<Vec<_>>();
                    let child_boundaries = boundaries.iter().filter_map(|(path, boundary_type)| {
                    let (segment, remaining) = path.split_first()?;
                    matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(candidate) if *candidate == index)
                        .then_some((remaining, *boundary_type))
                }).collect::<Vec<_>>();
                    prefix.push(CheckedUnitStructuralPathSegment::FixedIndex(index));
                    count = count.checked_add(visit(
                        types,
                        source,
                        *element_type_identity,
                        &matching,
                        &child_boundaries,
                        prefix,
                        traversal,
                        emit,
                    )?)?;
                    prefix.pop();
                }
            }
            CheckedUnitStructuralTypeShape::Sum { cases }
            | CheckedUnitStructuralTypeShape::Mixed { cases, .. } => {
                let common = match &declaration.shape {
                    CheckedUnitStructuralTypeShape::Mixed { fields, .. } => fields.as_slice(),
                    _ => &[],
                };
                if cases.is_empty()
                    || cases.iter().enumerate().any(|(case_index, case)| {
                        !case.identity.is_valid()
                            || cases[..case_index]
                                .iter()
                                .any(|earlier| earlier.identity == case.identity)
                    })
                    || moved_paths
                        .iter()
                        .chain(boundaries)
                        .any(|(path, _)| match path.first() {
                            Some(CheckedUnitStructuralPathSegment::Case(identity)) => {
                                !cases.iter().any(|case| &case.identity == identity)
                            }
                            Some(CheckedUnitStructuralPathSegment::Field(_)) => !matches!(
                                declaration.shape,
                                CheckedUnitStructuralTypeShape::Mixed { .. }
                            ),
                            None => !nominal_boundary,
                            _ => true,
                        })
                {
                    return None;
                }
                let common_moves = moved_paths
                    .iter()
                    .copied()
                    .filter(|(path, _)| {
                        matches!(
                            path.first(),
                            Some(CheckedUnitStructuralPathSegment::Field(_))
                        )
                    })
                    .collect::<Vec<_>>();
                let common_boundaries = boundaries
                    .iter()
                    .copied()
                    .filter(|(path, _)| {
                        matches!(
                            path.first(),
                            Some(CheckedUnitStructuralPathSegment::Field(_))
                        )
                    })
                    .collect::<Vec<_>>();
                let mut reachable_cases = 0_usize;
                let mut established_cases = 0_usize;
                let case_node = if emit && traversal.retain_partition {
                    Some(traversal.append_node(CleanupPartitionNode::Cases {
                        path: prefix.clone(),
                        type_identity: current_type,
                        arms: Vec::new(),
                        common: Vec::new(),
                    })?)
                } else {
                    None
                };
                for case in cases {
                    let reachability =
                        (traversal.select_case)(prefix, current_type, &case.identity)?;
                    let arm = if let Some(node) = case_node {
                        let CleanupPartitionNode::Cases { arms, .. } = &mut traversal.nodes[node]
                        else {
                            return None;
                        };
                        let ordinal = arms.len();
                        arms.push(CleanupPartitionArm {
                            case_identity: case.identity.clone(),
                            reachability,
                            steps: Vec::new(),
                        });
                        Some(PartitionDestination::Arm { node, arm: ordinal })
                    } else {
                        None
                    };
                    if reachability == CleanupCaseReachability::Inactive {
                        continue;
                    }
                    if let Some(arm) = arm {
                        traversal.destinations.push(arm);
                    }
                    reachable_cases = reachable_cases.checked_add(1)?;
                    if reachability == CleanupCaseReachability::Selected {
                        established_cases = established_cases.checked_add(1)?;
                    }
                    let condition_depth = traversal.conditions.len();
                    if reachability == CleanupCaseReachability::Runtime {
                        traversal.conditions.push(CleanupCaseCondition {
                            path: prefix.clone(),
                            type_identity: current_type,
                            case_identity: case.identity.clone(),
                        });
                    }
                    let payload_moves = moved_paths.iter().filter(|&(path, _identity)| matches!(path.first(), Some(CheckedUnitStructuralPathSegment::Case(candidate)) if candidate == &case.identity)).map(|(path, identity)| (&path[1..], *identity)).collect::<Vec<_>>();
                    let payload_boundaries = boundaries.iter().filter(|&(path, _identity)| matches!(path.first(), Some(CheckedUnitStructuralPathSegment::Case(candidate)) if candidate == &case.identity)).map(|(path, identity)| (&path[1..], *identity)).collect::<Vec<_>>();
                    if !nominal_boundary
                        && payload_moves.is_empty()
                        && payload_boundaries.is_empty()
                        && common_moves.is_empty()
                        && common_boundaries.is_empty()
                        && !common.iter().any(|field| {
                            matches!(
                                field.field_type,
                                CheckedUnitStructuralFieldType::Structural { .. }
                            )
                        })
                        && validate_inline_alternatives(common, std::slice::from_ref(case))
                            .is_some()
                    {
                        if emit {
                            traversal.emit(source, prefix, current_type)?;
                        }
                        count = count.checked_add(1)?;
                    } else {
                        prefix.push(CheckedUnitStructuralPathSegment::Case(
                            case.identity.clone(),
                        ));
                        count = count.checked_add(visit_fields(
                            types,
                            source,
                            &case.fields,
                            &payload_moves,
                            &payload_boundaries,
                            prefix,
                            traversal,
                            emit,
                        )?)?;
                        prefix.pop();
                    }
                    traversal.conditions.truncate(condition_depth);
                    if arm.is_some() {
                        traversal.destinations.pop();
                    }
                }
                if established_cases > 0 && reachable_cases != 1 {
                    return None;
                }
                if let Some(node) = case_node {
                    traversal
                        .destinations
                        .push(PartitionDestination::Common(node));
                }
                count = count.checked_add(visit_fields(
                    types,
                    source,
                    common,
                    &common_moves,
                    &common_boundaries,
                    prefix,
                    traversal,
                    emit,
                )?)?;
                if case_node.is_some() {
                    traversal.destinations.pop();
                }
            }
            _ => return None,
        }
        Some(count)
    }

    fn visit_fields(
        types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        fields: &[checked_trees::CheckedUnitStructuralFieldPlan],
        moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
        traversal: &mut ResidualTraversal<'_>,
        emit: bool,
    ) -> Option<usize> {
        if fields.iter().enumerate().any(|(position, field)| {
            !field.identity.is_valid()
                || match has_runtime_storage(field) {
                    Some(false) => false,
                    Some(true) => {
                        !is_partial_affine_field_type(&field.field_type)
                            && !field.cleanup_type_identity().is_valid()
                    }
                    None => true,
                }
                || fields[..position]
                    .iter()
                    .any(|earlier| earlier.identity == field.identity)
        }) {
            return None;
        }
        let mut count = 0_usize;
        let mut matched = 0_usize;
        let mut matched_boundaries = 0_usize;
        for field in fields.iter().rev() {
            if !has_runtime_storage(field)? {
                if traversal.partition_paths.iter().any(|path| {
                matches!(path.strip_prefix(prefix.as_slice()).and_then(|relative| relative.first()),
                    Some(CheckedUnitStructuralPathSegment::Field(identity)) if identity == &field.identity)
            }) {
                return None;
            }
                continue;
            }
            let matching = moved_paths.iter().filter_map(|(path, identity)| {
            let (segment, remaining) = path.split_first()?;
            matches!(segment, CheckedUnitStructuralPathSegment::Field(candidate) if candidate == &field.identity)
                .then_some((remaining, *identity))
        }).collect::<Vec<_>>();
            let child_boundaries = boundaries.iter().filter_map(|(path, identity)| {
            let (segment, remaining) = path.split_first()?;
            matches!(segment, CheckedUnitStructuralPathSegment::Field(candidate) if candidate == &field.identity)
                .then_some((remaining, *identity))
        }).collect::<Vec<_>>();
            matched = matched.checked_add(matching.len())?;
            matched_boundaries = matched_boundaries.checked_add(child_boundaries.len())?;
            let type_identity = field.cleanup_type_identity();
            if !type_identity.is_valid() {
                if !matching.is_empty() || !child_boundaries.is_empty() {
                    return None;
                }
                continue;
            }
            prefix.push(CheckedUnitStructuralPathSegment::Field(
                field.identity.clone(),
            ));
            count = count.checked_add(visit(
                types,
                source,
                type_identity,
                &matching,
                &child_boundaries,
                prefix,
                traversal,
                emit,
            )?)?;
            prefix.pop();
        }
        (matched == moved_paths.len() && matched_boundaries == boundaries.len()).then_some(count)
    }

    pub fn reconstruct(
        types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        root_type: TypeIdentityId,
        moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        max_residuals: usize,
    ) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
        let select_case = |prefix: &[CheckedUnitStructuralPathSegment],
                           _: TypeIdentityId,
                           case: &CheckedMemberIdentity| {
            let mut selected = None;
            for (path, _) in moved_paths.iter().chain(boundaries) {
                let Some([CheckedUnitStructuralPathSegment::Case(identity), ..]) =
                    path.strip_prefix(prefix)
                else {
                    continue;
                };
                if selected.is_some_and(|earlier: &CheckedMemberIdentity| earlier != identity) {
                    return None;
                }
                selected = Some(identity);
            }
            Some(if selected? == case {
                CleanupCaseReachability::Selected
            } else {
                CleanupCaseReachability::Inactive
            })
        };
        reconstruct_case_selected_regions(
            types,
            source,
            root_type,
            moved_paths,
            boundaries,
            &[],
            max_residuals,
            &select_case,
        )?
        .into_iter()
        .map(|region| region.conditions.is_empty().then_some(region.disposition))
        .collect()
    }

    pub fn reconstruct_case_selected_regions(
        types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        root_type: TypeIdentityId,
        moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        partition_paths: &[Vec<CheckedUnitStructuralPathSegment>],
        max_residuals: usize,
        select_case: &dyn Fn(
            &[CheckedUnitStructuralPathSegment],
            TypeIdentityId,
            &CheckedMemberIdentity,
        ) -> Option<CleanupCaseReachability>,
    ) -> Option<Vec<CaseSelectedCleanupRegion>> {
        reconstruct_selected_cleanup(
            types,
            source,
            root_type,
            moved_paths,
            boundaries,
            partition_paths,
            max_residuals,
            select_case,
            false,
        )
        .map(|partition| partition.regions)
    }

    pub fn reconstruct_case_selected_cleanup_partition(
        types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        root_type: TypeIdentityId,
        moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        partition_paths: &[Vec<CheckedUnitStructuralPathSegment>],
        max_residuals: usize,
        select_case: &dyn Fn(
            &[CheckedUnitStructuralPathSegment],
            TypeIdentityId,
            &CheckedMemberIdentity,
        ) -> Option<CleanupCaseReachability>,
    ) -> Option<CaseSelectedCleanupPartition> {
        reconstruct_selected_cleanup(
            types,
            source,
            root_type,
            moved_paths,
            boundaries,
            partition_paths,
            max_residuals,
            select_case,
            true,
        )
    }

    fn reconstruct_selected_cleanup(
        types: &BTreeMap<TypeIdentityId, &CheckedUnitStructuralTypePlan>,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        root_type: TypeIdentityId,
        moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        boundaries: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        partition_paths: &[Vec<CheckedUnitStructuralPathSegment>],
        max_residuals: usize,
        select_case: &dyn Fn(
            &[CheckedUnitStructuralPathSegment],
            TypeIdentityId,
            &CheckedMemberIdentity,
        ) -> Option<CleanupCaseReachability>,
        retain_partition: bool,
    ) -> Option<CaseSelectedCleanupPartition> {
        if boundaries.iter().enumerate().any(|(position, (path, _))| {
            boundaries[..position]
                .iter()
                .any(|(earlier, _)| path == earlier)
                || moved_paths
                    .iter()
                    .any(|(moved, _)| path.starts_with(moved) || moved.starts_with(path))
        }) {
            return None;
        }
        if moved_paths.iter().enumerate().any(|(index, (path, _))| {
            path.is_empty()
                || moved_paths[..index]
                    .iter()
                    .any(|(earlier, _)| path.starts_with(earlier) || earlier.starts_with(path))
        }) {
            return None;
        }
        let mut traversal = ResidualTraversal {
            select_case,
            partition_paths,
            conditions: Vec::new(),
            regions: Vec::new(),
            nodes: Vec::new(),
            destinations: Vec::new(),
            retain_partition,
            max_partition_nodes: max_residuals,
            active_types: Vec::new(),
        };
        let count = visit(
            types,
            source,
            root_type,
            moved_paths,
            boundaries,
            &mut Vec::new(),
            &mut traversal,
            false,
        )?;
        if count > max_residuals {
            return None;
        }
        traversal.regions.try_reserve_exact(count).ok()?;
        let emitted = visit(
            types,
            source,
            root_type,
            moved_paths,
            boundaries,
            &mut Vec::new(),
            &mut traversal,
            true,
        )?;
        (emitted == count && traversal.regions.len() == count && traversal.destinations.is_empty())
            .then_some(CaseSelectedCleanupPartition {
                regions: traversal.regions,
                nodes: traversal.nodes,
            })
    }
}

pub mod permission_provenance {
    use language_semantics::PermissionProvenance;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    pub fn expression_permission_provenance(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        resolve_place: &mut impl FnMut(
            ExpressionHandle,
        ) -> Result<Option<PermissionProvenance>, &'static str>,
    ) -> Result<Option<PermissionProvenance>, &'static str> {
        expression_origin(program, expression, resolve_place, &mut Vec::new())
    }

    fn expression_origin(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        resolve_place: &mut impl FnMut(
            ExpressionHandle,
        ) -> Result<Option<PermissionProvenance>, &'static str>,
        active: &mut Vec<ExpressionHandle>,
    ) -> Result<Option<PermissionProvenance>, &'static str> {
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return Err("permission provenance has a stale or cyclic expression");
        }
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Name(path) = node {
            let mut constants = program
                .const_declarations()
                .iter()
                .filter(|declaration| path.symbol.is_valid() && declaration.symbol == path.symbol);
            if let Some(declaration) = constants.next() {
                if constants.next().is_some()
                    || program.symbols.get(path.symbol).kind != symbols::SymbolKind::Const
                    || !program
                        .type_reference_table
                        .contains_type_reference(declaration.declared_type)
                    || program.type_multiplicity(declaration.declared_type)
                        != language_semantics::Multiplicity::Unrestricted
                {
                    return Err("permission provenance constant has invalid declaration custody");
                }
                return Ok(None);
            }
        }
        if !matches!(
            node,
            ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
        ) {
            return resolve_place(expression);
        }
        active.push(expression);
        let mut common = None;
        let mut different = false;
        let mut retain = |operand| -> Result<(), &'static str> {
            if let Some(origin) = expression_origin(program, operand, resolve_place, active)? {
                match common {
                    Some(previous) => different |= previous != origin,
                    None => common = Some(origin),
                }
            }
            Ok(())
        };
        match node {
            ExpressionNode::StructLiteral(literal) => {
                let fields = program.expression_table.struct_fields(literal.fields);
                if fields.len() != literal.fields.len() {
                    return Err("permission provenance record fields are stale");
                }
                for field in fields {
                    retain(field.value)?;
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let values = program.expression_table.expression_handles(*elements);
                if values.len() != elements.len() {
                    return Err("permission provenance array elements are stale");
                }
                for value in values {
                    retain(*value)?;
                }
            }
            _ => return Err("permission provenance lost its aggregate expression"),
        }
        active.pop();
        Ok(if different { None } else { common })
    }
}

pub mod plan_laid {
    use crate::CheckingInput;
    use diagnostics::Diagnostic;
    use layout_plans::LayoutPlacementReport;
    use typed_trees::data::DataField;

    pub(crate) fn validate_plans(program: &dyn CheckingInput, diagnostics: &mut Vec<Diagnostic>) {
        let mut data_symbols = Vec::with_capacity(program.plan_laid_layouts.len());
        for plan in &program.plan_laid_layouts {
            macro_rules! laid {
                ($detail:literal) => {
                    diagnostics.push(Diagnostic::error(format!(
                        concat!("plan-laid value type `{}` ", $detail),
                        plan.data_name
                    )))
                };
            }
            if data_symbols.contains(&program.normalized_type_identity(plan.application)) {
                laid!("repeats its exact synthesized data identity");
                continue;
            }
            data_symbols.push(program.normalized_type_identity(plan.application));
            if !crate::queries::layout_checking::layout_family_is_complete(program, plan) {
                laid!("lost its complete exact target-family application roster");
                continue;
            }

            let Some(data) = program.data_field_definition(plan.application) else {
                laid!("no longer names its exact synthesized data identity");
                continue;
            };
            let data_fields = runtime_fields(program, data);
            let data_field_symbols = data_fields
                .iter()
                .map(|field| field.symbol)
                .collect::<Vec<_>>();
            if data_field_symbols != plan.field_symbols
                || plan
                    .geometries(program.typed_program())
                    .any(|geometry| geometry.offsets.len() != plan.field_symbols.len())
            {
                laid!("changed its exact synthesized field identity inventory");
                continue;
            }

            let Some(schema) = program.data_field_definition(plan.semantic_type) else {
                laid!("no longer names its exact source schema identity");
                continue;
            };
            let schema_fields = runtime_fields(program, schema);
            if schema_fields
                .iter()
                .map(|field| field.symbol)
                .ne(plan.schema_field_symbols.iter().copied())
                || plan.schema_field_symbols.len() != plan.field_symbols.len()
            {
                laid!("changed its exact source schema field identity inventory");
                continue;
            }
            if !schema_argument_matches(program, plan, &schema_fields) {
                laid!("lost its exact complete captured Schema argument");
                continue;
            }
            if data_fields
                .iter()
                .zip(&schema_fields)
                .any(|(data_field, schema_field)| {
                    data_field.symbol != schema_field.symbol
                        || data_field.identity != schema_field.identity
                        || program.normalized_type_identity(data_field.type_reference)
                            != program.normalized_type_identity(schema_field.type_reference)
                })
            {
                laid!("changed its exact schema-to-synthesized field correspondence");
                continue;
            }
            if !plan.geometries(program.typed_program()).all(|geometry| {
                target_neutral_report_matches(program, geometry, schema, &schema_fields)
            }) {
                laid!("changed its exact target-neutral layout report identity");
                continue;
            }
            if !plan
                .geometries(program.typed_program())
                .all(|geometry| geometry_matches(geometry, &schema_fields))
            {
                laid!("changed its exact validated geometry projection");
                continue;
            }
            if !plan.geometries(program.typed_program()).all(|geometry| {
                stored_integer_capabilities_match(program, geometry, &schema_fields)
            }) {
                laid!("changed its exact stored-integer type capability");
                continue;
            }

            let Some(storage) = program.layout_storage_arguments(plan.application) else {
                diagnostics.push(Diagnostic::error(
                    "fixed representation lost its core constructor application",
                ));
                continue;
            };
            let reclosed = program.static_operand_from_type_reference(storage.policy)
            .map_err(|reason| reason.to_owned())
            .and_then(|operand| crate::declarations::traits::conformance_applications::close_conformance_application(program, &operand)
                .map_err(|error| error.to_string()));
            match reclosed {
                Ok(application)
                    if application == plan.policy_application
                        && application.declaration == plan.policy_symbol
                        && application.trait_definition == storage.contract
                        && application.rows.iter().any(|row| {
                            row.realization_machine == plan.policy_plan_machine_symbol
                                || program.captured_machine_applications.iter().any(
                                    |(_, captured)| {
                                        captured.instance == plan.policy_plan_machine_symbol
                                            && captured.template == row.realization_machine
                                            && captured.conformance_application == application
                                    },
                                )
                        }) => {}
                Ok(_) => diagnostics.push(Diagnostic::error(
                    "fixed representation changed its exact selected implementation application",
                )),
                Err(reason) => diagnostics.push(Diagnostic::error(format!(
                    "fixed representation policy reclosure: {reason}"
                ))),
            }
        }
    }

    fn schema_argument_matches(
        program: &dyn CheckingInput,
        plan: &typed_trees::PlanLaidLayout,
        schema_fields: &[&DataField],
    ) -> bool {
        let Some(schema) = program.data_field_definition(plan.semantic_type) else {
            return false;
        };
        let mut retired_identities = schema.retired_identities.clone();
        retired_identities.sort_unstable();
        let matches = |argument: typed_trees::PlanLaidSchemaArgument| {
            program
                .plan_laid_retired_field_identities
                .span(argument.retired_field_identities)
                .is_some_and(|rows| {
                    rows.iter()
                        .map(|row| row.identity)
                        .eq(retired_identities.iter().copied())
                })
                && schema_argument_rows_match(program, argument, schema_fields)
        };
        let arguments = match plan.realizations {
            typed_trees::PlanLaidRealizations::Invariant(_) => {
                return plan.schema_argument.is_none_or(&matches);
            }
            typed_trees::PlanLaidRealizations::TargetInputs(span) => {
                let Some(targets) = program.plan_laid_target_layouts.span(span) else {
                    return false;
                };
                if plan.schema_argument.is_some() {
                    return false;
                }
                targets
                    .iter()
                    .map(|target| target.schema_argument)
                    .collect::<Vec<_>>()
            }
        };
        arguments
            .iter()
            .all(|argument| argument.is_some_and(matches))
    }

    fn schema_argument_rows_match(
        program: &dyn CheckingInput,
        argument: typed_trees::PlanLaidSchemaArgument,
        schema_fields: &[&DataField],
    ) -> bool {
        if argument.abi_version != 4 {
            return false;
        }
        let Some(rows) = program.plan_laid_schema_fields.span(argument.fields) else {
            return false;
        };
        if rows.len() != schema_fields.len() || rows.len() > 32 {
            return false;
        }
        let Some(retired) = program
            .plan_laid_retired_field_identities
            .span(argument.retired_field_identities)
        else {
            return false;
        };
        if retired.len() > 32
            || retired.iter().enumerate().any(|(index, row)| {
                retired[..index]
                    .iter()
                    .any(|previous| previous.identity == row.identity)
                    || schema_fields
                        .iter()
                        .any(|field| field.identity == Some(row.identity))
            })
        {
            return false;
        }
        let mut canonical_fields = schema_fields.to_vec();
        if canonical_fields
            .iter()
            .any(|field| field.identity.is_some())
        {
            canonical_fields.sort_unstable_by_key(|field| field.identity);
        }
        rows.iter()
            .zip(canonical_fields)
            .enumerate()
            .all(|(index, (row, field))| {
                row.schema_field == field.symbol
                    && row.identity == field.identity
                    && row.key != 0
                    && !rows[..index].iter().any(|previous| {
                        previous.key == row.key || previous.schema_field == row.schema_field
                    })
                    && row.layout_source
                        == program
                            .typed_program()
                            .field_layout_source(field.type_reference)
                    && row.byte_size != 0
                    && row.byte_alignment.is_power_of_two()
                    && row.kind
                        == if program
                            .primitive_type_reference(field.type_reference)
                            .is_some()
                        {
                            typed_trees::PlanLaidSchemaFieldKind::Scalar
                        } else if matches!(
                            program.type_reference_table.type_reference(
                                program
                                    .type_reference_table
                                    .unconstrained_type_reference(field.type_reference)
                            ),
                            typed_trees::types::TypeReferenceNode::FixedArray { .. }
                        ) {
                            typed_trees::PlanLaidSchemaFieldKind::Repeated
                        } else {
                            typed_trees::PlanLaidSchemaFieldKind::Nested
                        }
            })
    }

    fn target_neutral_report_matches(
        program: &dyn CheckingInput,
        plan: &typed_trees::PlanLaidGeometry,
        schema: &typed_trees::data::DataDefinition,
        schema_fields: &[&DataField],
    ) -> bool {
        let report = &plan.validated_layout;
        if report.schema_report_fingerprint
            != typed_trees::identity::normalized_schema_report_fingerprint(
                program.typed_program(),
                schema,
            )
        {
            return false;
        }

        let mut matched_entries = 0usize;
        let mut offsets = Vec::with_capacity(schema_fields.len());
        let mut has_only_single_at_entries = true;
        for field in schema_fields {
            let entries = report
                .entries
                .iter()
                .filter(|entry| {
                    entry.member_identity == field.identity
                        && (field.identity.is_some() || entry.field == field.name.as_str())
                })
                .collect::<Vec<_>>();
            matched_entries += entries.len();
            match entries.as_slice() {
                [entry] => match entry.placement {
                    LayoutPlacementReport::At { offset } => offsets.push(offset),
                    LayoutPlacementReport::IntegerAt { .. }
                    | LayoutPlacementReport::Bits { .. } => {
                        has_only_single_at_entries = false;
                    }
                },
                _ => has_only_single_at_entries = false,
            }
        }
        let expected_offsets = has_only_single_at_entries.then_some(offsets);
        matched_entries == report.entries.len() && report.offsets == expected_offsets
    }

    fn stored_integer_capabilities_match(
        program: &dyn CheckingInput,
        plan: &typed_trees::PlanLaidGeometry,
        schema_fields: &[&DataField],
    ) -> bool {
        plan.integer_fields.iter().all(|integer| {
            let Some(field) = schema_fields.get(integer.field_index) else {
                return false;
            };
            stored_integer_write_is_total(
                program,
                field.type_reference,
                u64::from(integer.stored_width_bits),
                integer.interpretation,
            ) == Some(integer.write_is_total)
        })
    }

    fn stored_integer_write_is_total(
        program: &dyn CheckingInput,
        field_type: typed_trees::types::TypeReferenceHandle,
        stored_width: u64,
        interpretation: layout_plans::IntegerInterpretation,
    ) -> Option<bool> {
        if !(1..=64).contains(&stored_width) {
            return None;
        }
        let primitive = program.primitive_type_reference(field_type)?;
        let (admitted_minimum, admitted_maximum) =
            if let Some(range) = crate::scalar_representation_range(program, field_type) {
                (i128::from(range.minimum), i128::from(range.maximum))
            } else {
                let width = primitive.scalar_byte_size()? * 8;
                if primitive.is_signed_integer() {
                    (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
                } else {
                    (0, (1i128 << width) - 1)
                }
            };
        let (stored_minimum, stored_maximum) = stored_integer_bounds(stored_width, interpretation)?;
        Some(admitted_minimum >= stored_minimum && admitted_maximum <= stored_maximum)
    }

    fn stored_integer_bounds(
        stored_width: u64,
        interpretation: layout_plans::IntegerInterpretation,
    ) -> Option<(i128, i128)> {
        if !(1..=64).contains(&stored_width) {
            return None;
        }
        Some(interpretation.stored_bounds(stored_width))
    }

    fn geometry_matches(
        plan: &typed_trees::PlanLaidGeometry,
        schema_fields: &[&DataField],
    ) -> bool {
        let report = &plan.validated_layout;
        if report.size.and_then(|size| usize::try_from(size).ok()) != Some(plan.size)
            || usize::try_from(report.align).ok() != Some(plan.align)
        {
            return false;
        }

        let mut offsets = Vec::with_capacity(schema_fields.len());
        let mut bit_fields = Vec::new();
        let mut integer_fields = Vec::new();
        let mut repeated_fields = Vec::new();
        for (field_index, field) in schema_fields.iter().enumerate() {
            let entries = report
                .entries
                .iter()
                .filter(|entry| {
                    entry.member_identity == field.identity
                        && (field.identity.is_some() || entry.field == field.name.as_str())
                })
                .collect::<Vec<_>>();
            match entries.as_slice() {
                [entry] if matches!(entry.placement, LayoutPlacementReport::At { .. }) => {
                    let LayoutPlacementReport::At { offset } = entry.placement else {
                        unreachable!()
                    };
                    let Ok(offset) = usize::try_from(offset) else {
                        return false;
                    };
                    offsets.push(offset);
                }
                [entry] if matches!(entry.placement, LayoutPlacementReport::IntegerAt { .. }) => {
                    let LayoutPlacementReport::IntegerAt {
                        offset,
                        stored_width,
                        interpretation,
                    } = entry.placement
                    else {
                        unreachable!()
                    };
                    let (Ok(offset), Ok(stored_width_bits)) =
                        (usize::try_from(offset), u16::try_from(stored_width))
                    else {
                        return false;
                    };
                    offsets.push(offset);
                    let Some(retained) = plan
                        .integer_fields
                        .iter()
                        .find(|integer| integer.field_index == field_index)
                    else {
                        return false;
                    };
                    integer_fields.push(typed_trees::PlanLaidIntegerField {
                        field_index,
                        stored_width_bits,
                        interpretation,
                        write_is_total: retained.write_is_total,
                    });
                }
                entries
                    if entries.len() > 1
                        && entries.iter().all(|entry| {
                            matches!(entry.placement, LayoutPlacementReport::At { .. })
                        }) =>
                {
                    let mut element_offsets = entries
                        .iter()
                        .map(|entry| match entry.placement {
                            LayoutPlacementReport::At { offset } => offset,
                            _ => unreachable!(),
                        })
                        .collect::<Vec<_>>();
                    element_offsets.sort_unstable();
                    let Some(stride) = element_offsets[1].checked_sub(element_offsets[0]) else {
                        return false;
                    };
                    if stride == 0
                        || element_offsets
                            .windows(2)
                            .any(|window| window[1].checked_sub(window[0]) != Some(stride))
                    {
                        return false;
                    }
                    let (Ok(offset), Ok(element_stride)) =
                        (usize::try_from(element_offsets[0]), usize::try_from(stride))
                    else {
                        return false;
                    };
                    offsets.push(offset);
                    repeated_fields.push(typed_trees::PlanLaidRepeatedField {
                        field_index,
                        element_stride,
                    });
                }
                entries
                    if !entries.is_empty()
                        && entries.iter().all(|entry| {
                            matches!(entry.placement, LayoutPlacementReport::Bits { .. })
                        }) =>
                {
                    let mut fragments = Vec::with_capacity(entries.len());
                    for entry in entries {
                        let LayoutPlacementReport::Bits {
                            container,
                            container_width,
                            destination_lsb,
                            source_lsb,
                            width,
                        } = entry.placement
                        else {
                            unreachable!()
                        };
                        let (
                            Ok(container_byte_offset),
                            Ok(container_width_bits),
                            Ok(destination_lsb),
                            Ok(source_lsb),
                            Ok(width),
                        ) = (
                            usize::try_from(container),
                            u16::try_from(container_width),
                            u16::try_from(destination_lsb),
                            u16::try_from(source_lsb),
                            u16::try_from(width),
                        )
                        else {
                            return false;
                        };
                        if fragments.is_empty() {
                            offsets.push(container_byte_offset);
                        }
                        fragments.push(typed_trees::PlanLaidBitFragment {
                            container_byte_offset,
                            container_width_bits,
                            destination_lsb,
                            source_lsb,
                            width,
                        });
                    }
                    bit_fields.push(typed_trees::PlanLaidBitField {
                        field_index,
                        fragments,
                    });
                }
                _ => return false,
            }
        }

        offsets == plan.offsets
            && bit_fields == plan.bit_fields
            && integer_fields == plan.integer_fields
            && repeated_fields == plan.repeated_fields
    }

    fn runtime_fields<'program>(
        program: &'program dyn CheckingInput,
        data: &typed_trees::data::DataDefinition,
    ) -> Vec<&'program DataField> {
        program
            .live_data_fields(program.data_members(data))
            .collect()
    }
}

pub mod qualification_casts {
    use crate::frozen_program::expression_table;
    use crate::proof_contracts::arithmetic_domains::BigIntInterval;
    use crate::proof_contracts::arithmetic_domains::{ExpressionInterval, expression_interval};
    use crate::{ScopedArithmeticValue, StrictArithmeticImplicationJudgment};
    use diagnostics::Diagnostic;
    use numerics::bignum::BigInt;
    use symbols::JoinedPath;
    use typed_trees::expression::{ExpressionNode, TableCastExpression};

    pub(crate) fn validate(
        program: &dyn crate::CheckingInput,
        intervals: &[ExpressionInterval],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let references =
            typed_trees::references::collect_program_references(program.typed_program());
        for (expression, node) in expression_table(program).expression_entries() {
            if let ExpressionNode::Cast(cast) = node
                && !cast.semantic_domain.is_empty()
                && references.expressions.contains(&expression)
            {
                judge_qualification_cast(
                    program,
                    expression,
                    expression_interval(intervals, cast.value),
                    cast,
                    diagnostics,
                );
            }
        }
    }

    fn judge_qualification_cast(
        program: &dyn crate::CheckingInput,
        occurrence: typed_trees::expression::ExpressionHandle,
        source: Option<&ExpressionInterval>,
        cast: &TableCastExpression,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let members = expression_table(program).name_path_members(cast.semantic_domain);
        let base_name = members.joined_path();
        let mut name = base_name.clone();
        if !cast.semantic_domain_arguments.is_empty() {
            let arguments = program
                .type_reference_table
                .type_reference_handles(cast.semantic_domain_arguments)
                .iter()
                .map(|argument| program.display_type_reference(*argument))
                .collect::<Vec<_>>()
                .join(", ");
            name = format!("{name}<{arguments}>");
        }
        let declared = program.domain_definition(cast.semantic_domain_symbol);
        match declared {
            Some(domain) => {
                if crate::field_domains::domain_requires_provenance(program, domain.symbol) {
                    return;
                }
                if !domain.predicate_presence.is_present()
                    && domain.alias.is_none()
                    && !crate::field_domains::domain_is_vacuous(
                        program,
                        domain.symbol,
                        &mut Vec::new(),
                    )
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "`as ... in {name}` cannot establish a non-vacuous domain; every \
                     predicate must be proved and routed provenance must come from an exact \
                     requirement authorized by the domain"
                    )));
                    return;
                }
                if crate::field_domains::domain_is_vacuous(program, domain.symbol, &mut Vec::new())
                {
                    return;
                }
                if program
                    .primitive_type_reference(domain.target_type)
                    .is_none()
                {
                    return;
                }
                let mut judgment = operand_mint_judgment(program, cast, source);
                if judgment == MintJudgment::NotProved
                    && predicates_hold_on_private_arrivals(program, occurrence, cast, domain.symbol)
                {
                    judgment = MintJudgment::Discharged;
                }
                match judgment {
                    MintJudgment::Discharged => {}
                    MintJudgment::FactFalse => {
                        diagnostics.push(Diagnostic::error(format!(
                            "`as ... in {name}` cannot mint: a `{name}` domain fact is \
                         FALSE at this value -- the predicate obligation is owed \
                         (decision 19's \"predicate obligation not discharged\" \
                         class)",
                        )));
                    }
                    MintJudgment::NotProved => {
                        diagnostics.push(Diagnostic::error(format!(
                            "`as ... in {name}` cannot prove every domain predicate from the \
                         checked operand value; constrain that value with a guarantee or guard",
                        )));
                    }
                }
            }
            None => {
                let matching_names = program
                    .domain_definitions()
                    .iter()
                    .filter(|domain| {
                        domain.name.as_str() == base_name
                            || domain.name.as_str().ends_with(&format!("::{base_name}"))
                    })
                    .count();
                diagnostics.push(Diagnostic::error(format!(
                    "{} cast domain `{name}` for target `{}`",
                    if matching_names > 1 {
                        "ambiguous"
                    } else {
                        "unknown"
                    },
                    program.display_type_reference(cast.target_type),
                )));
            }
        }
    }

    #[derive(PartialEq, Eq)]
    pub(crate) enum MintJudgment {
        Discharged,
        FactFalse,
        NotProved,
    }

    fn operand_mint_judgment(
        program: &dyn crate::CheckingInput,
        cast: &TableCastExpression,
        source: Option<&ExpressionInterval>,
    ) -> MintJudgment {
        let value = if let Some(literal) = expression_table(program).integer_node(cast.value)
            && let Some(value) = literal.decimal_value_bignum()
        {
            ScopedArithmeticValue::Integer(value)
        } else if let Some(source) = source {
            ScopedArithmeticValue::BoundedInteger {
                value: Box::new(ScopedArithmeticValue::Atom {
                    identity: format!("qualification operand {:?}", cast.value),
                    unsigned: false,
                }),
                interval: BigIntInterval {
                    low: source.low.map(BigInt::from_i64),
                    high: source.high.map(BigInt::from_i64),
                },
            }
        } else {
            return MintJudgment::NotProved;
        };
        match crate::proof_contracts::domains::integer_domain_membership(
            program,
            cast.semantic_domain_symbol,
            program
                .type_reference_table
                .type_reference_handles(cast.semantic_domain_arguments),
            cast.semantic_domain_id,
            &value,
        ) {
            StrictArithmeticImplicationJudgment::Proven => MintJudgment::Discharged,
            StrictArithmeticImplicationJudgment::Refuted => MintJudgment::FactFalse,
            StrictArithmeticImplicationJudgment::Unknown => MintJudgment::NotProved,
        }
    }

    fn predicates_hold_on_private_arrivals(
        program: &dyn crate::CheckingInput,
        occurrence: typed_trees::expression::ExpressionHandle,
        cast: &TableCastExpression,
        domain: symbols::SymbolHandle,
    ) -> bool {
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let owns = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .any(|statement| {
                        let mut handles = Vec::new();
                        crate::expression_traversal::collect_statement_expression_trees(
                            program,
                            statement,
                            &mut handles,
                        );
                        handles.contains(&occurrence)
                    });
                if owns {
                    return crate::expression_result_type_reference(program, machine, state, cast.value)
                    .is_some_and(|operand| {
                        crate::proof_contracts::contract_entailment::domain_predicates_hold_at_occurrence(
                            program, machine, state, occurrence, operand, domain,
                        )
                    });
                }
            }
        }
        false
    }
}

pub mod receiving_values {
    use crate::frozen_program::expression_table;
    use checked_trees::FlowPermissionEventFact;
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use language_semantics::PermissionEventSource;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::statement::StatementNode;

    pub fn receiving_value<'a>(
        program: &dyn crate::CheckingInput,
        segments: &'a arena::PagedArenaSet<PlaceSegment>,
        event: &FlowPermissionEventFact,
    ) -> Option<(usize, ExpressionHandle, &'a [PlaceSegment])> {
        let statement_index = match event.source {
            PermissionEventSource::Statement { statement_index }
            | PermissionEventSource::Call {
                statement_index, ..
            } => statement_index,
            _ => return None,
        };
        let event_path = segments.span_or_empty(event.segments);
        let (mut expression, mut path) = if let Some(expression) = event.root.as_expression() {
            (expression, event_path)
        } else {
            let state = program.state_by_symbol(event.state_symbol)?;
            match program
                .statement_table
                .statement_at(state, statement_index)?
            {
                StatementNode::LocalData(local)
                    if event.root == PlaceRoot::Symbol(local.symbol) =>
                {
                    (local.initial_value, event_path)
                }
                StatementNode::Assignment(assignment) => {
                    let destination = crate::canonical_place_from_expression_in_state(
                        program,
                        state.symbol,
                        statement_index,
                        assignment.target,
                    )?;
                    if destination.root != event.root {
                        return None;
                    }
                    (
                        assignment.value,
                        event_path.strip_prefix(destination.segments.as_slice())?,
                    )
                }
                _ => return None,
            }
        };
        if !expression.is_valid() {
            return None;
        }
        loop {
            expression = crate::ownership_source_expression(
                program,
                event.state_symbol,
                statement_index,
                expression,
            );
            let Some(segment) = path.first() else {
                break;
            };
            match (expression_table(program).expression(expression), segment) {
                (ExpressionNode::ArrayLiteral(values), PlaceSegment::FixedIndex { index }) => {
                    expression = *expression_table(program)
                        .expression_handles(*values)
                        .get(*index)?;
                }
                (ExpressionNode::StructLiteral(literal), PlaceSegment::Case { variant }) => {
                    let definition = program.data_definition_by_symbol(literal.type_symbol)?;
                    let declared = program.data_variant(definition, *variant)?;
                    if literal.case_name.as_ref() != Some(&declared.name) {
                        return None;
                    }
                }
                (ExpressionNode::StructLiteral(literal), PlaceSegment::Field { symbol }) => {
                    let (definition, declared) = program.any_field_owner(*symbol)?;
                    if definition.symbol != literal.type_symbol {
                        return None;
                    }
                    expression = expression_table(program)
                        .struct_fields(literal.fields)
                        .iter()
                        .find(|field| field.name == declared.name)?
                        .value;
                }
                _ => break,
            }
            path = &path[1..];
        }
        Some((statement_index, expression, path))
    }
}

pub mod reference_access {
    use crate::frozen_program::expression_table;
    use crate::value_custody::expression_types::declared_reference_access;
    use facts::PlaceRoot;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) enum ReferenceAccessRefusal {
        ExplicitWriteOnlyRequired,
        WriteOnlyMayNotRead,
        MutableSourceRequired,
        ExclusiveBorrowSourceRequired {
            root: PlaceRoot,
            access: language_semantics::ReferenceAccess,
        },
        NamedPlaceRequired {
            access: language_semantics::ReferenceAccess,
        },
        BareRangeFormsSharedView,
    }

    pub(crate) fn receiving_reference_access_refusal(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        prior_statements: &[typed_trees::statement::StatementNode],
        value: ExpressionHandle,
        required: TypeReferenceHandle,
        required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    ) -> Option<ReferenceAccessRefusal> {
        use language_semantics::ReferenceAccess;
        if let ExpressionNode::Match(dispatch) = expression_table(program).expression(value) {
            return expression_table(program)
                .match_arms(dispatch.arms)
                .iter()
                .find_map(|arm| {
                    receiving_reference_access_refusal(
                        program,
                        machine,
                        state,
                        prior_statements,
                        arm.value,
                        required,
                        required_substitutions,
                    )
                });
        }
        let (applied_required, _) =
            crate::value_custody::expression_types::substituted_required_reference(
                program,
                required,
                required_substitutions,
            );
        let expected_access = declared_reference_access(program, applied_required);
        let completed_borrow = completed_reference_borrow_source(program, machine, state, value);
        let supplied_access = completed_borrow
            .as_ref()
            .map(|borrow| borrow.access)
            .or_else(|| declared_call_reference_access(program, value))
            .or_else(|| {
                value_forwarded_exclusive_reference_access(
                    program,
                    machine,
                    state,
                    prior_statements,
                    value,
                )
            })
            .or_else(|| {
                crate::value_custody::expression_types::owned_rvalue_exclusive_reference_access(
                    program, value,
                )
            });
        if let Some(borrow) = &completed_borrow
            && let Some(refusal) = borrow_source_access_refusal(
                program,
                machine,
                state,
                prior_statements,
                borrow.source,
                borrow.access,
                borrow.interpretation
                    == Some(
                        crate::value_custody::recasts::ReferenceFormationInterpretation::Declared,
                    )
                    && crate::value_custody::expression_types::borrow_selects_reference_storage(
                        program,
                        borrow.source,
                        required,
                        required_substitutions,
                    ),
            )
        {
            return Some(refusal);
        }
        if expected_access == Some(ReferenceAccess::WriteOnly)
            && supplied_access != Some(ReferenceAccess::WriteOnly)
            && !matches!(expression_table(program).expression(value),
            ExpressionNode::Borrow(borrow) if borrow.access == ReferenceAccess::WriteOnly)
        {
            return Some(ReferenceAccessRefusal::ExplicitWriteOnlyRequired);
        }
        if supplied_access == Some(ReferenceAccess::WriteOnly)
            && expected_access != Some(ReferenceAccess::WriteOnly)
        {
            return Some(ReferenceAccessRefusal::WriteOnlyMayNotRead);
        }
        if expected_access == Some(ReferenceAccess::Mutable)
            && completed_borrow
                .as_ref()
                .is_some_and(|borrow| borrow.interpretation.is_none())
        {
            return Some(ReferenceAccessRefusal::BareRangeFormsSharedView);
        }
        if expected_access == Some(ReferenceAccess::Mutable)
            && !matches!(supplied_access, Some(ReferenceAccess::Mutable))
        {
            return Some(ReferenceAccessRefusal::MutableSourceRequired);
        }
        None
    }

    pub(crate) fn is_reference_destination(
        program: &dyn crate::CheckingInput,
        required: TypeReferenceHandle,
    ) -> bool {
        declared_reference_access(program, required).is_some()
    }

    pub(crate) fn report_reference_access_conflict(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        prior_statements: &[typed_trees::statement::StatementNode],
        value: ExpressionHandle,
        required: TypeReferenceHandle,
        owner: &str,
        diagnostics: &mut Vec<diagnostics::Diagnostic>,
    ) -> bool {
        if !is_reference_destination(program, required) {
            return false;
        }
        let Some(refusal) = receiving_reference_access_refusal(
            program,
            machine,
            state,
            prior_statements,
            value,
            required,
            &[],
        ) else {
            return false;
        };
        let reason = match refusal {
            ReferenceAccessRefusal::ExplicitWriteOnlyRequired => {
                "requires explicit write-only attenuation; pass `&write ...`"
            }
            ReferenceAccessRefusal::WriteOnlyMayNotRead => {
                "may read, but the source supplies only write-only authority"
            }
            ReferenceAccessRefusal::ExclusiveBorrowSourceRequired {
                access: language_semantics::ReferenceAccess::WriteOnly,
                ..
            } => "requires exclusive source access; shared or immutable storage cannot supply it",
            ReferenceAccessRefusal::ExclusiveBorrowSourceRequired { .. }
            | ReferenceAccessRefusal::MutableSourceRequired => {
                "requires mutable source access; shared or immutable storage cannot supply it"
            }
            ReferenceAccessRefusal::NamedPlaceRequired { .. } => {
                "requires a named source place; a temporary cannot lend exclusive access"
            }
            ReferenceAccessRefusal::BareRangeFormsSharedView => {
                "receives a bare range that forms a shared view; `bare-range-reference-access` spells the mutable subrange `&mut collection[start..end]`"
            }
        };
        diagnostics.push(diagnostics::Diagnostic::error(format!("{owner} {reason}")));
        true
    }

    pub(crate) struct CompletedBorrow {
        source: ExpressionHandle,
        access: language_semantics::ReferenceAccess,
        interpretation: Option<crate::value_custody::recasts::ReferenceFormationInterpretation>,
    }

    fn completed_reference_borrow_source(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        value: ExpressionHandle,
    ) -> Option<CompletedBorrow> {
        use language_semantics::ReferenceAccess;

        if let Some(formation) =
            crate::value_custody::recasts::reference_formation_source(program, value)
        {
            return Some(CompletedBorrow {
                source: formation.source,
                access: formation.access,
                interpretation: Some(formation.interpretation),
            });
        }
        match expression_table(program).expression(value) {
            ExpressionNode::Indexed(indexed)
                if program.expression_table.range_node(indexed.index).is_some()
                    && crate::value_custody::places::place_has_builtin_coordinates(
                        program, machine, state, value,
                    ) =>
            {
                let reference = crate::value_custody::places::declared_place_type_raw(
                    program,
                    machine,
                    state,
                    indexed.collection,
                )?;
                if !crate::collection_operations::type_reference_is_collection(program, reference) {
                    return None;
                }
                Some(CompletedBorrow {
                    source: indexed.collection,
                    access: ReferenceAccess::Shared,
                    interpretation: None,
                })
            }
            _ => None,
        }
    }

    fn declared_call_reference_access(
        program: &dyn crate::CheckingInput,
        argument: ExpressionHandle,
    ) -> Option<language_semantics::ReferenceAccess> {
        crate::machine_calls::calls::call_result_type(program, argument)
            .and_then(|reference| declared_reference_access(program, reference))
    }

    fn value_forwarded_exclusive_reference_access(
        program: &dyn crate::CheckingInput,
        current_machine: &Machine,
        current_state: Option<&State>,
        prior_statements: &[typed_trees::statement::StatementNode],
        argument: ExpressionHandle,
    ) -> Option<language_semantics::ReferenceAccess> {
        let (root, binding_type, _) = source_place_root(
            program,
            current_machine,
            current_state,
            prior_statements,
            argument,
        )?;
        crate::value_custody::expression_types::place_forwarded_exclusive_reference_access(
            program,
            argument,
            root,
            binding_type,
        )
    }

    fn borrow_source_access_refusal(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        prior_statements: &[typed_trees::statement::StatementNode],
        source: ExpressionHandle,
        access: language_semantics::ReferenceAccess,
        lends_storage: bool,
    ) -> Option<ReferenceAccessRefusal> {
        let Some((root, binding_type, owned_root_is_writable)) =
            source_place_root(program, machine, state, prior_statements, source)
        else {
            return access
                .is_exclusive()
                .then_some(ReferenceAccessRefusal::NamedPlaceRequired { access });
        };
        let permitted = if lends_storage {
            crate::value_custody::expression_types::place_storage_borrow_access(
            program,
            source,
            root,
            binding_type,
            access,
        )
        .is_some_and(|storage| {
            !access.is_exclusive()
                || storage == crate::value_custody::expression_types::StorageWriteAccess::BorrowedStorage
                || reference_slot_is_writable(
                    program,
                    machine,
                    state,
                    prior_statements,
                    source,
                    root,
                )
        })
        } else {
            crate::value_custody::expression_types::place_permits_reference_borrow(
                program,
                source,
                root,
                binding_type,
                owned_root_is_writable,
                access,
            )
        };
        (!permitted).then_some(if access == language_semantics::ReferenceAccess::Shared {
            ReferenceAccessRefusal::WriteOnlyMayNotRead
        } else {
            ReferenceAccessRefusal::ExclusiveBorrowSourceRequired { root, access }
        })
    }

    fn reference_slot_is_writable(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        prior_statements: &[typed_trees::statement::StatementNode],
        source: ExpressionHandle,
        root: PlaceRoot,
    ) -> bool {
        let Some(place) = crate::canonical_place_from_expression(program, source) else {
            return false;
        };
        if !place.segments.is_empty() {
            return true;
        }
        let Some(symbol) = root.as_symbol() else {
            return false;
        };
        prior_statements.iter().any(|statement| {
            statement
                .as_local_data()
                .is_some_and(|local| local.symbol == symbol && local.is_mutable)
        }) || state.is_some_and(|state| {
            program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.symbol == symbol && parameter.is_mutable)
        }) || program
            .machine_owned_data_by_symbol(machine, symbol)
            .is_some()
    }

    pub(crate) fn assignment_storage_write_access(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        prior_statements: &[typed_trees::statement::StatementNode],
        target: ExpressionHandle,
    ) -> Option<crate::value_custody::expression_types::StorageWriteAccess> {
        let (root, root_type, writable) =
            source_place_root(program, machine, state, prior_statements, target)?;
        let writes_parameter_referent = state.is_some_and(|state| {
            crate::value_custody::places::assignment_targets_parameter_referent(
                program, state, target,
            )
        });
        let writes_range_backing = expression_table(program)
            .indexed_node(target)
            .is_some_and(|indexed| expression_table(program).indexed_on_range(indexed));
        if writes_parameter_referent || writes_range_backing {
            return crate::value_custody::expression_types::place_permits_reference_borrow(
                program,
                target,
                root,
                root_type,
                writable,
                language_semantics::ReferenceAccess::WriteOnly,
            )
            .then_some(
                crate::value_custody::expression_types::StorageWriteAccess::BorrowedStorage,
            );
        }
        crate::value_custody::expression_types::place_storage_write_access(
            program, target, root, root_type,
        )
    }

    fn source_place_root(
        program: &dyn crate::CheckingInput,
        current_machine: &Machine,
        current_state: Option<&State>,
        prior_statements: &[typed_trees::statement::StatementNode],
        argument: ExpressionHandle,
    ) -> Option<(PlaceRoot, TypeReferenceHandle, bool)> {
        if !crate::value_custody::places::place_has_builtin_coordinates(
            program,
            current_machine,
            current_state,
            argument,
        ) {
            return None;
        }
        let mut root = argument;
        loop {
            let receiver = match expression_table(program).expression(root) {
                ExpressionNode::Member(member) => member.receiver,
                ExpressionNode::Indexed(indexed) => indexed.collection,
                ExpressionNode::Name(_) => break,
                ExpressionNode::SavedSubject(saved) => {
                    let source = program.saved_value_source(saved)?;
                    let reference = if let Some(state) = current_state {
                        crate::value_custody::expression_types::expression_result_type_reference(
                            program,
                            current_machine,
                            state,
                            source,
                        )?
                    } else {
                        crate::value_custody::places::declared_place_type_raw(
                            program,
                            current_machine,
                            current_state,
                            source,
                        )?
                    };
                    let anchor = expression_table(program)
                        .value_capture(saved.capture)
                        .value_anchor;
                    if !expression_table(program).expression_is_valid(anchor) {
                        return None;
                    }
                    return Some((PlaceRoot::Expression(anchor), reference, true));
                }
                _ => return None,
            };
            root = receiver;
        }
        let path = expression_table(program).name_path_node(root)?;
        let [_] = expression_table(program).name_path_members(path.members) else {
            return None;
        };
        if path.head_symbol != path.symbol {
            return None;
        }
        if let Some(state) = current_state
            && let Some(binding_type) = crate::value_custody::locals::state_binding_type(
                program,
                state,
                prior_statements,
                path.symbol,
            )
        {
            let owned_root_is_writable = match program.symbols.get(path.symbol).kind {
                symbols::SymbolKind::Local => true,
                symbols::SymbolKind::Parameter => program
                    .state_parameters(state)
                    .iter()
                    .any(|parameter| parameter.symbol == path.symbol && parameter.is_mutable),
                _ => false,
            };
            return Some((
                PlaceRoot::Symbol(path.symbol),
                binding_type,
                owned_root_is_writable,
            ));
        }
        program
            .machine_owned_data_by_symbol(current_machine, path.symbol)
            .map(|owned| (PlaceRoot::Symbol(path.symbol), owned.type_reference, true))
    }
}

pub mod reference_local_establishment {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use crate::value_custody::source_liveness::{expression_uses_symbol, statement_uses_symbol};
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::statement::StatementNode;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn validate(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut unestablished: Vec<(SymbolHandle, &str)> = Vec::new();
        for statement in program.statement_table.statements(state.statement_nodes) {
            let stored = match statement {
                StatementNode::Assignment(assignment) => {
                    whole_local_target(program, assignment.target)
                }
                _ => None,
            };
            unestablished.retain(|(symbol, name)| {
                let used = match statement {
                    StatementNode::Assignment(assignment) if stored == Some(*symbol) => {
                        expression_uses_symbol(program, assignment.value, *symbol)
                    }
                    _ => statement_uses_symbol(program, statement, *symbol),
                };
                if used {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{}` uses reference local `{name}` before a store \
                     establishes it; a reference declared without an initializer holds no \
                     descriptor until it is assigned",
                        machine.name,
                        state.name.as_str(),
                    )));
                    return false;
                }
                stored != Some(*symbol)
            });
            if let Some(local) = statement.as_local_data()
                && !local.initial_value.is_valid()
                && is_reference(program, local.type_reference)
            {
                unestablished.push((local.symbol, local.name.as_str()));
            }
        }
    }

    fn whole_local_target(
        program: &dyn CheckingInput,
        target: ExpressionHandle,
    ) -> Option<SymbolHandle> {
        let path = expression_table(program).name_path_node(target)?;
        (expression_table(program)
            .name_path_members(path.members)
            .len()
            == 1
            && path.symbol.is_valid())
        .then_some(path.symbol)
    }

    fn is_reference(program: &dyn CheckingInput, mut type_reference: TypeReferenceHandle) -> bool {
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Reference { .. } => return true,
                _ => return false,
            }
        }
    }
}

pub mod reference_shells {
    use language_semantics::ReferenceAccess;
    use symbols::SymbolHandle;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ReferenceShell {
        pub access: ReferenceAccess,
        pub referee: TypeReferenceHandle,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SliceViewShell {
        pub access: ReferenceAccess,
        pub slice: TypeReferenceHandle,
        pub element: TypeReferenceHandle,
    }

    pub fn reference_shell(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<ReferenceShell> {
        reference_shell_with_substitutions(program, reference, &[])
    }

    pub(crate) fn reference_shell_with_substitutions(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<ReferenceShell> {
        let shell = peel_carrier_shells(program, reference, substitutions)?;
        let TypeReferenceNode::Reference {
            referee, access, ..
        } = program.type_reference_table.type_reference(shell)
        else {
            return None;
        };
        Some(ReferenceShell {
            access: *access,
            referee: *referee,
        })
    }

    pub fn slice_view_shell(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<SliceViewShell> {
        let shell = peel_carrier_shells(program, reference, substitutions)?;
        let TypeReferenceNode::Reference {
            referee, access, ..
        } = program.type_reference_table.type_reference(shell)
        else {
            return None;
        };
        let slice = peel_carrier_shells(program, *referee, substitutions)?;
        let TypeReferenceNode::Slice { element_type } =
            program.type_reference_table.type_reference(slice)
        else {
            return None;
        };
        Some(SliceViewShell {
            access: *access,
            slice,
            element: *element_type,
        })
    }

    pub(crate) fn peel_carrier_shells(
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<TypeReferenceHandle> {
        for _ in 0..=program.type_reference_table.type_reference_count() {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Named { symbol, .. } => {
                    let Some(replacement) =
                        crate::queries::substitutions::actual_for(substitutions, *symbol)
                    else {
                        return Some(reference);
                    };
                    if replacement == reference {
                        return None;
                    }
                    reference = replacement;
                }
                _ => return Some(reference),
            }
        }
        None
    }

    pub fn carrier_beneath_borrows(
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        if !reference.is_valid() {
            return None;
        }
        for _ in 0..=program.type_reference_table.type_reference_count() {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Reference { referee, .. } => reference = *referee,
                _ => return Some(reference),
            }
        }
        None
    }

    pub fn stored_view_shell(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<ReferenceShell> {
        slice_view_shell(program, reference, substitutions)?;
        reference_shell(program, reference)
    }

    pub fn byte_sequence_carrier(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<checked_trees::CheckedByteSequenceCarrier> {
        let view = slice_view_shell(program, type_reference, substitutions)?;
        if program.primitive_type_reference(view.element) != Some(PrimitiveType::U8) {
            return None;
        }
        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView {
            access: Some(view.access.into()),
        })
    }
}

pub mod resident_content {
    use symbols::SymbolHandle;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub struct ResidentOwnerQualifications {
        pub extent: SymbolHandle,
        pub granted_symbol: SymbolHandle,
        pub resident_symbol: SymbolHandle,
        pub granted: language_semantics::SemanticDomainId,
        pub resident: language_semantics::SemanticDomainId,
        pub policy: TypeReferenceHandle,
        pub schema: TypeReferenceHandle,
    }

    pub fn resident_owner_qualifications(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<ResidentOwnerQualifications> {
        let carrier = program
            .type_reference_table
            .peeled_type_reference(reference);
        let TypeReferenceNode::Named { symbol: extent, .. } =
            program.type_reference_table.type_reference(carrier)
        else {
            return None;
        };
        if !is_extent_declaration(program, *extent) || program.symbols.name(*extent) != "Extent" {
            return None;
        }
        let domains = crate::value_custody::field_domains::declared_domain_constraint_records(
            program, reference,
        );
        let canonical_domain = |name| {
            let mut matching = domains.iter().copied().filter(|domain| {
            is_extent_declaration(program, domain.symbol)
                && program.domain_definition(domain.symbol).is_some_and(|definition| {
                    definition.name.as_str().rsplit("::").next() == Some(name)
                        && matches!(
                            program.type_reference_table.type_reference(
                                program.type_reference_table.peeled_type_reference(definition.target_type)
                            ),
                            TypeReferenceNode::Named { symbol, .. } if symbol == extent
                        )
                })
        });
            let domain = matching.next()?;
            (matching.next().is_none() && domain.semantic_id.is_valid()).then_some(domain)
        };
        let granted = canonical_domain("Granted")?;
        let resident = canonical_domain("Resident")?;
        if !granted.arguments.is_empty() {
            return None;
        }
        let [policy, schema] = resident.arguments.as_slice() else {
            return None;
        };
        Some(ResidentOwnerQualifications {
            extent: *extent,
            granted_symbol: granted.symbol,
            resident_symbol: resident.symbol,
            granted: granted.semantic_id,
            resident: resident.semantic_id,
            policy: *policy,
            schema: *schema,
        })
    }

    pub fn is_extent_declaration(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> bool {
        program
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| program.symbols.source_file(span))
            .is_some_and(|file| {
                file.origin == source::SourceOrigin::Toolchain
                    && file.path.strip_prefix(&file.package_root).ok()
                        == Some(std::path::Path::new("extent.omg"))
            })
    }
}

pub mod scalar_case_constructor {
    use crate::value_custody::storage_contents::ContentsRequirement;
    use symbols::SymbolHandle;
    use typed_trees::data::DataMember;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    pub struct ScalarCaseConstructor {
        pub type_reference: TypeReferenceHandle,
        pub case: SymbolHandle,
        pub fields: Vec<(SymbolHandle, ExpressionHandle, PrimitiveType)>,
    }

    pub struct StructuralCaseConstructor {
        pub type_reference: TypeReferenceHandle,
        pub case: SymbolHandle,
        pub fields: Vec<(SymbolHandle, ExpressionHandle, TypeReferenceHandle)>,
        pub omitted_fields: Vec<(SymbolHandle, TypeReferenceHandle)>,
    }

    pub fn is_scalar_case_value(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
    ) -> bool {
        is_case_value(
            program,
            expression,
            reference,
            matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Match(_)
            ),
        )
    }

    fn is_case_value(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        allow_existing: bool,
    ) -> bool {
        if !crate::has_contents(program, reference, &[], ContentsRequirement::PlainOwned) {
            return false;
        }
        let expected = program.normalized_type_identity(reference);
        let mut pending = vec![(expression, 0_usize)];
        while let Some((expression, depth)) = pending.pop() {
            if depth >= program.expression_table.expression_count() {
                return false;
            }
            if let Some(constructor) = scalar_case_constructor(program, expression) {
                if program.normalized_type_identity(constructor.type_reference) != expected {
                    return false;
                }
                continue;
            }
            if allow_existing && scalar_case_value_source(program, expression, reference).is_some()
            {
                continue;
            }
            let Some(dispatch) = program.expression_table.match_node(expression) else {
                return false;
            };
            let arms = program.expression_table.match_arms(dispatch.arms);
            if arms.is_empty() {
                return false;
            }
            pending.extend(arms.iter().map(|arm| (arm.value, depth + 1)));
        }
        true
    }

    pub fn scalar_case_value_source(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
    ) -> Option<SymbolHandle> {
        let source = crate::plain_owned_value_source(program, expression, reference)?;
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let owner = crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)?;
        let members = program.data_members(owner);
        if members.is_empty()
            || members.iter().any(|member| match member {
                DataMember::Field(_) => true,
                DataMember::Variant(case) => {
                    program.data_payload_fields(case).iter().any(|field| {
                        program
                            .primitive_type_reference(field.type_reference)
                            .is_none()
                    })
                }
            })
        {
            return None;
        }
        Some(source)
    }

    fn case_constructor_parts(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        expected: Option<TypeReferenceHandle>,
        erased_member_owner: bool,
    ) -> Option<(
        TypeReferenceHandle,
        SymbolHandle,
        Vec<(SymbolHandle, ExpressionHandle, TypeReferenceHandle, bool)>,
    )> {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let (authored_owner, selected, fields) =
            match program.expression_table.expression(expression) {
                ExpressionNode::StructLiteral(literal) => (
                    crate::declarations::data_queries::data_definition_by_symbol(
                        program,
                        literal.type_symbol,
                    )?,
                    literal.case_symbol?,
                    program.expression_table.struct_fields(literal.fields),
                ),
                ExpressionNode::Name(name) => (
                    crate::exact_case_reference_owner(program, expression)?,
                    name.symbol,
                    &[][..],
                ),
                _ => return None,
            };
        let reference = match expected {
            Some(expected) => crate::unwrapped_type_reference(program, expected)?,
            None => {
                if !authored_owner.type_parameters.is_empty() {
                    return None;
                }
                program
                    .type_reference_table
                    .find_named_type_reference(authored_owner.symbol)?
            }
        };
        let owner = program.closed_data_constructor_definition(reference, authored_owner.symbol)?;
        let (case_ordinal, authored_case) = program
            .data_members(authored_owner)
            .iter()
            .enumerate()
            .find_map(|(ordinal, member)| match member {
                DataMember::Variant(case) if case.symbol == selected => Some((ordinal, case)),
                _ => None,
            })?;
        let DataMember::Variant(case) = program.data_members(owner).get(case_ordinal)? else {
            return None;
        };
        if case.name != authored_case.name || case.identity != authored_case.identity {
            return None;
        }
        if (!erased_member_owner
            && (!crate::has_contents(program, reference, &[], ContentsRequirement::NumericOwned)
                || program
                    .data_members(owner)
                    .iter()
                    .any(|member| matches!(member, DataMember::Field(_)))))
            || program.symbols.get(selected).parent != authored_owner.symbol
        {
            return None;
        }
        let declarations = program.data_payload_fields(case);
        let mut operands: Vec<(SymbolHandle, ExpressionHandle, TypeReferenceHandle, bool)> =
            Vec::with_capacity(fields.len());
        for field in fields {
            let declaration = program
                .data_members(authored_owner)
                .iter()
                .enumerate()
                .find_map(|(ordinal, member)| match member {
                    DataMember::Field(original) if original.symbol == field.field_symbol => {
                        let DataMember::Field(closed) = program.data_members(owner).get(ordinal)?
                        else {
                            return None;
                        };
                        Some((original, closed))
                    }
                    _ => None,
                })
                .or_else(|| {
                    program
                        .data_payload_fields(authored_case)
                        .iter()
                        .enumerate()
                        .find_map(|(ordinal, original)| {
                            (original.symbol == field.field_symbol)
                                .then(|| declarations.get(ordinal).map(|closed| (original, closed)))
                                .flatten()
                        })
                })?;
            let (original, declared) = declaration;
            if original.name != declared.name
                || original.identity != declared.identity
                || original.relevance != declared.relevance
            {
                return None;
            }
            if erased_member_owner && declared.relevance.is_erased() {
                if !program.expression_table.expression_is_valid(field.value) {
                    return None;
                }
                continue;
            }
            if !field.field_symbol.is_valid()
                || operands
                    .iter()
                    .any(|(symbol, ..)| *symbol == declared.symbol)
                || !program.expression_table.expression_is_valid(field.value)
            {
                return None;
            }
            operands.push((
                declared.symbol,
                field.value,
                declared.type_reference,
                declared.relevance.is_erased(),
            ));
        }
        let payload_operands = operands
            .iter()
            .filter(|(symbol, ..)| {
                declarations
                    .iter()
                    .any(|declaration| declaration.symbol == *symbol)
            })
            .count();
        let runtime_payload = declarations
            .iter()
            .filter(|declaration| !(erased_member_owner && declaration.relevance.is_erased()))
            .count();
        (erased_member_owner || payload_operands == runtime_payload).then_some((
            reference,
            case.symbol,
            operands,
        ))
    }

    pub fn scalar_case_constructor(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<ScalarCaseConstructor> {
        let (reference, selected, operands) =
            case_constructor_parts(program, expression, None, false)?;
        let mut fields = Vec::with_capacity(operands.len());
        for (symbol, value, declared, ..) in operands {
            fields.push((symbol, value, program.primitive_type_reference(declared)?));
        }
        Some(ScalarCaseConstructor {
            type_reference: reference,
            case: selected,
            fields,
        })
    }

    pub fn structural_case_constructor(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<StructuralCaseConstructor> {
        let (reference, selected, operands) =
            case_constructor_parts(program, expression, Some(expected), true)?;
        let mut fields = Vec::with_capacity(operands.len());
        for (symbol, value, declared, erased) in operands {
            if erased {
                return None;
            }
            fields.push((symbol, value, declared));
        }
        let owner = program.closed_data_definition(reference)?;
        let omitted_fields = program
            .data_members(owner)
            .iter()
            .flat_map(|member| match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(case) if case.symbol == selected => {
                    program.data_payload_fields(case)
                }
                DataMember::Variant(_) => &[],
            })
            .filter(|field| !field.relevance.is_erased())
            .filter(|field| fields.iter().all(|(symbol, ..)| *symbol != field.symbol))
            .map(|field| (field.symbol, field.type_reference))
            .collect();
        Some(StructuralCaseConstructor {
            type_reference: reference,
            case: selected,
            fields,
            omitted_fields,
        })
    }
}

pub mod scalar_representation_range {
    use numerics::bignum::BigInt;
    use typed_trees::types::{
        PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };

    pub fn scalar_representation_range(
        program: &dyn crate::CheckingInput,
        handle: TypeReferenceHandle,
    ) -> Option<language_semantics::wire::WireScalarRange> {
        fn collect(
            program: &dyn crate::CheckingInput,
            primitive: PrimitiveType,
            handle: TypeReferenceHandle,
            minimum: &mut BigInt,
            maximum: &mut BigInt,
            found: &mut bool,
        ) -> Option<()> {
            match program.type_reference_table.type_reference(handle) {
                TypeReferenceNode::Reference { referee, .. } => {
                    collect(program, primitive, *referee, minimum, maximum, found)
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        if let TypeConstraintNode::Domain(domain) = constraint
                            && let Some((lower, upper)) =
                                crate::exact_declared_domain_carrier_interval(
                                    program, primitive, domain,
                                )
                        {
                            if lower > *minimum {
                                *minimum = lower;
                            }
                            if upper < *maximum {
                                *maximum = upper;
                            }
                            *found = true;
                        }
                    }
                    collect(program, primitive, *base_type, minimum, maximum, found)
                }
                _ => Some(()),
            }
        }

        let primitive = program.primitive_type_reference(handle)?;
        if primitive == PrimitiveType::Bool {
            return Some(language_semantics::wire::WireScalarRange {
                minimum: 0,
                maximum: 1,
                signed: false,
            });
        }
        let (carrier_minimum, carrier_maximum) = match primitive {
            PrimitiveType::I8 => (i64::from(i8::MIN), i128::from(i8::MAX)),
            PrimitiveType::I16 => (i64::from(i16::MIN), i128::from(i16::MAX)),
            PrimitiveType::I32 => (i64::from(i32::MIN), i128::from(i32::MAX)),
            PrimitiveType::I64 => (i64::MIN, i128::from(i64::MAX)),
            PrimitiveType::U8 => (0, i128::from(u8::MAX)),
            PrimitiveType::U16 => (0, i128::from(u16::MAX)),
            PrimitiveType::U32 => (0, i128::from(u32::MAX)),
            PrimitiveType::U64 => (0, i128::from(u64::MAX)),
            _ => return None,
        };
        let mut minimum = BigInt::from_i64(carrier_minimum);
        let mut maximum = BigInt::from_u128(u128::try_from(carrier_maximum).ok()?);
        let mut found = matches!(primitive, PrimitiveType::I32 | PrimitiveType::U32);
        collect(
            program,
            primitive,
            handle,
            &mut minimum,
            &mut maximum,
            &mut found,
        )?;
        if !found {
            return None;
        }
        let (minimum, maximum) = if minimum > maximum {
            (1, 0)
        } else {
            (minimum.to_i64()?, maximum.to_i64()?)
        };
        Some(language_semantics::wire::WireScalarRange {
            minimum,
            maximum,
            signed: primitive.is_signed_integer(),
        })
    }
}

pub mod source_liveness {
    use crate::frozen_program::expression_table;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::StatementNode;

    pub fn statement_uses_symbol(
        program: &dyn crate::CheckingInput,
        statement: &StatementNode,
        symbol: SymbolHandle,
    ) -> bool {
        match statement {
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
            StatementNode::Assignment(assignment) => {
                expression_uses_symbol(program, assignment.target, symbol)
                    || expression_uses_symbol(program, assignment.value, symbol)
            }
            StatementNode::Call(call) => {
                call.receiver_symbol == symbol
                    || program
                        .statement_table
                        .expression_handles(call.arguments)
                        .iter()
                        .any(|argument| expression_uses_symbol(program, *argument, symbol))
            }
            StatementNode::Expression(expression) => {
                expression_uses_symbol(program, *expression, symbol)
            }
            StatementNode::LocalData(local_data) => {
                expression_uses_symbol(program, local_data.initial_value, symbol)
            }
            StatementNode::Transition(transition) => {
                transition_guard_uses_symbol(program, transition.guard, symbol)
                    || transition_target_uses_symbol(
                        program,
                        program.statement_table.transition_target(transition.target),
                        symbol,
                    )
                    || transition_target_uses_symbol(
                        program,
                        program
                            .statement_table
                            .transition_target(transition.continuation),
                        symbol,
                    )
            }
        }
    }

    pub(crate) fn expression_uses_symbol(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        symbol: SymbolHandle,
    ) -> bool {
        expression_uses_symbol_except(program, expression, symbol, &|_| false)
    }

    fn expression_uses_symbol_except(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        symbol: SymbolHandle,
        completed_value: &dyn Fn(ExpressionHandle) -> bool,
    ) -> bool {
        if completed_value(expression) {
            return false;
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::SavedSubject(saved) => {
                program.saved_value_source(saved).is_some_and(|subject| {
                    expression_uses_symbol_except(program, subject, symbol, completed_value)
                })
            }
            ExpressionNode::Match(dispatch) => {
                crate::queries::expression_traversal::match_children(program, *dispatch).any(
                    |child| expression_uses_symbol_except(program, child, symbol, completed_value),
                )
            }
            ExpressionNode::Atomic(atomic) => {
                expression_uses_symbol_except(program, atomic.value, symbol, completed_value)
            }
            ExpressionNode::ArrayLiteral(values) => expression_table(program)
                .expression_handles(*values)
                .iter()
                .any(|value| {
                    expression_uses_symbol_except(program, *value, symbol, completed_value)
                }),
            ExpressionNode::Binary(binary) => {
                expression_uses_symbol_except(program, binary.left, symbol, completed_value)
                    || expression_uses_symbol_except(program, binary.right, symbol, completed_value)
            }
            ExpressionNode::Call(call) => {
                (call.receiver.is_valid()
                    && expression_uses_symbol_except(
                        program,
                        call.receiver,
                        symbol,
                        completed_value,
                    ))
                    || expression_table(program)
                        .expression_handles(call.arguments)
                        .iter()
                        .any(|argument| {
                            expression_uses_symbol_except(
                                program,
                                *argument,
                                symbol,
                                completed_value,
                            )
                        })
            }
            ExpressionNode::Cast(cast) => {
                expression_uses_symbol_except(program, cast.value, symbol, completed_value)
            }
            ExpressionNode::Indexed(indexed) => {
                expression_uses_symbol_except(program, indexed.collection, symbol, completed_value)
                    || expression_uses_symbol_except(
                        program,
                        indexed.index,
                        symbol,
                        completed_value,
                    )
            }
            ExpressionNode::Range(range) => {
                (range.start.is_valid()
                    && expression_uses_symbol_except(program, range.start, symbol, completed_value))
                    || (range.end.is_valid()
                        && expression_uses_symbol_except(
                            program,
                            range.end,
                            symbol,
                            completed_value,
                        ))
            }
            ExpressionNode::Member(member) => {
                member.member_symbol == symbol
                    || expression_uses_symbol_except(
                        program,
                        member.receiver,
                        symbol,
                        completed_value,
                    )
            }
            ExpressionNode::Borrow(inner_expression) => expression_uses_symbol_except(
                program,
                inner_expression.target,
                symbol,
                completed_value,
            ),
            ExpressionNode::Unary(unary) => {
                expression_uses_symbol_except(program, unary.operand, symbol, completed_value)
            }
            ExpressionNode::Name(path) => {
                expression_table(program)
                    .name_path_member_symbols(path.member_symbols)
                    .first()
                    .copied()
                    .filter(|member| member.is_valid())
                    .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
                    .or_else(|| path.symbol.is_valid().then_some(path.symbol))
                    .is_some_and(|path_symbol| path_symbol == symbol)
                    || expression_table(program)
                        .name_path_member_symbols(path.member_symbols)
                        .contains(&symbol)
                    || path.symbol == symbol
            }
            ExpressionNode::StructLiteral(struct_literal) => expression_table(program)
                .struct_fields(struct_literal.fields)
                .iter()
                .any(|field| {
                    expression_uses_symbol_except(program, field.value, symbol, completed_value)
                }),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => false,
        }
    }

    fn transition_guard_uses_symbol(
        program: &dyn crate::CheckingInput,
        guard: typed_trees::statement::TransitionGuardNode,
        symbol: SymbolHandle,
    ) -> bool {
        match guard {
            typed_trees::statement::TransitionGuardNode::Always => false,
            typed_trees::statement::TransitionGuardNode::When(expression) => {
                expression_uses_symbol(program, expression, symbol)
            }
        }
    }

    pub fn transition_target_uses_symbol(
        program: &dyn crate::CheckingInput,
        target: &typed_trees::statement::TransitionTargetNode,
        symbol: SymbolHandle,
    ) -> bool {
        transition_target_uses_symbol_except(program, target, symbol, &|_| false)
    }

    pub fn transition_target_uses_symbol_except(
        program: &dyn crate::CheckingInput,
        target: &typed_trees::statement::TransitionTargetNode,
        symbol: SymbolHandle,
        completed_value: &dyn Fn(ExpressionHandle) -> bool,
    ) -> bool {
        match target {
            typed_trees::statement::TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                path.head_symbol == symbol
                    || path.symbol == symbol
                    || program
                        .statement_table
                        .expression_handles(*arguments)
                        .iter()
                        .any(|argument| {
                            expression_uses_symbol_except(
                                program,
                                *argument,
                                symbol,
                                completed_value,
                            )
                        })
            }
            typed_trees::statement::TransitionTargetNode::Value(expression) => {
                expression_uses_symbol_except(program, *expression, symbol, completed_value)
            }
            typed_trees::statement::TransitionTargetNode::Terminal => false,
        }
    }
}

pub mod static_inline_storage {
    use crate::frozen_program::expression_table;
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::data::DataDefinition;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Clone, PartialEq, Eq)]
    pub(crate) struct Binding {
        parameter: SymbolHandle,
        argument: TypeReferenceHandle,
        outer: usize,
    }

    pub fn runtime_inline_array_extent(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<SymbolHandle> {
        Search {
            program,
            remaining: 4096,
            visited: Vec::new(),
        }
        .ty(reference, &[])
    }

    pub(crate) struct Search<'a> {
        program: &'a dyn crate::CheckingInput,
        remaining: usize,
        visited: Vec<(TypeReferenceHandle, Vec<Binding>)>,
    }
    impl Search<'_> {
        fn ty(
            &mut self,
            reference: TypeReferenceHandle,
            bindings: &[Binding],
        ) -> Option<SymbolHandle> {
            self.remaining = self.remaining.checked_sub(1)?;
            if !reference.is_valid() || self.visited.contains(&(reference, bindings.to_vec())) {
                return None;
            }
            self.visited.push((reference, bindings.to_vec()));
            let program = self.program;
            if let Some(storage) = program.layout_storage_arguments(reference) {
                return self.ty(storage.semantic_type, bindings);
            }
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => None,
                TypeReferenceNode::Constrained { base_type, .. } => self.ty(*base_type, bindings),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    let extent = if program.closed_fixed_array_length(reference).is_some() {
                        None
                    } else {
                        match length {
                            FixedArrayLength::ConstParameter { symbol, .. } => {
                                self.value_symbol(*symbol, bindings)
                            }
                            FixedArrayLength::Expression(value) => self.value(*value, bindings),
                            FixedArrayLength::Literal(_) | FixedArrayLength::ConstCall { .. } => {
                                None
                            }
                        }
                    };
                    extent.or_else(|| self.ty(*element_type, bindings))
                }
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some(binding) = bindings
                        .iter()
                        .rev()
                        .find(|binding| binding.parameter == *symbol)
                    {
                        return self.ty(binding.argument, &bindings[..binding.outer]);
                    }
                    let data = program.data_definition_by_symbol(*symbol)?;
                    if self.independent_storage(data.symbol) {
                        return None;
                    }
                    if let Some(origin) =
                        data.generic_instance.filter(|origin| *origin != reference)
                    {
                        return self.ty(origin, bindings);
                    }
                    self.fields(data, bindings)
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } => {
                    let data = program.data_definition_by_symbol(*base_symbol)?;
                    if self.independent_storage(data.symbol) {
                        return None;
                    }
                    let parameters = program.data_type_parameters(data);
                    let arguments = program
                        .type_reference_table
                        .type_reference_handles(*arguments);
                    if parameters.len() != arguments.len() {
                        return None;
                    }
                    let mut applied = bindings.to_vec();
                    let outer = applied.len();
                    applied.extend(parameters.iter().zip(arguments).map(
                        |(parameter, argument)| Binding {
                            parameter: parameter.symbol,
                            argument: *argument,
                            outer,
                        },
                    ));
                    self.fields(data, &applied)
                }
                _ => None,
            }
        }

        fn independent_storage(&self, data: SymbolHandle) -> bool {
            self.program
                .access_view_plans
                .iter()
                .any(|view| view.data_symbol == data)
        }

        fn fields(&mut self, data: &DataDefinition, bindings: &[Binding]) -> Option<SymbolHandle> {
            let fields = self
                .program
                .data_member_fields(self.program.data_members(data))
                .filter(|field| !field.relevance.is_erased())
                .map(|field| field.type_reference)
                .collect::<Vec<_>>();
            fields
                .into_iter()
                .find_map(|field| self.ty(field, bindings))
        }

        fn value(
            &mut self,
            reference: TypeReferenceHandle,
            bindings: &[Binding],
        ) -> Option<SymbolHandle> {
            self.remaining = self.remaining.checked_sub(1)?;
            if self
                .program
                .static_const_type_reference_identity(reference)
                .is_some()
            {
                return None;
            }
            match self.program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => self.value_symbol(*symbol, bindings),
                TypeReferenceNode::ConstExpression(expression) => {
                    self.expression(*expression, bindings)
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    self.value(*base_type, bindings)
                }
                _ => None,
            }
        }

        fn value_symbol(
            &mut self,
            symbol: SymbolHandle,
            bindings: &[Binding],
        ) -> Option<SymbolHandle> {
            if let Some(binding) = bindings
                .iter()
                .rev()
                .find(|binding| binding.parameter == symbol)
            {
                return self.value(binding.argument, &bindings[..binding.outer]);
            }
            (symbol.is_valid()
                && matches!(
                    self.program.symbols.get(symbol).kind,
                    SymbolKind::Parameter | SymbolKind::Local | SymbolKind::Field
                ))
            .then_some(symbol)
        }

        fn expression(
            &mut self,
            expression: ExpressionHandle,
            bindings: &[Binding],
        ) -> Option<SymbolHandle> {
            self.remaining = self.remaining.checked_sub(1)?;
            if !expression.is_valid()
                || expression_table(self.program)
                    .constant_integer_value(expression)
                    .is_some()
            {
                return None;
            }
            let node = expression_table(self.program).expression(expression);
            if let ExpressionNode::Name(path) = node {
                return self.value_symbol(path.symbol, bindings);
            }
            let mut children = Vec::new();
            crate::queries::expression_traversal::children(self.program, node, |child| {
                children.push(child)
            });
            children
                .into_iter()
                .find_map(|child| self.expression(child, bindings))
        }
    }

    pub fn validate_prepared_inline_storage(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<diagnostics::Diagnostic>,
    ) {
        let proof = crate::proof_only_classification(program);
        for machine in program.machines() {
            if !program.machine_type_parameters(machine).is_empty()
                || proof.is_proof_machine(program, machine)
                || crate::mathematical_applications::is_indexed_logical_machine(program, machine)
                || crate::value_custody::content_projections::is_content_projection_machine(
                    program, machine,
                )
            {
                continue;
            }
            for state in program.machine_states(machine) {
                let mut roots = vec![(state.return_type, "result".to_owned())];
                roots.extend(
                    program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| !parameter.is_const && !parameter.relevance.is_erased())
                        .map(|parameter| {
                            (
                                parameter.type_reference,
                                format!("parameter `{}`", parameter.name),
                            )
                        }),
                );
                roots.extend(
                    program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .filter_map(|statement| statement.as_local_data())
                        .filter(|local| !local.relevance.is_erased())
                        .map(|local| (local.type_reference, format!("local `{}`", local.name))),
                );
                for (reference, role) in roots {
                    if let Some(extent) = runtime_inline_array_extent(program, reference) {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "machine `{}` state `{}` {role} uses runtime value `{}` for an inline fixed-array extent; a value index cannot determine a static inline layout; use a const extent, an authored sum or explicit storage/descriptor",
                        machine.name, state.name, program.symbols.name(extent),
                    )));
                    }
                }
            }
        }
    }
}

pub mod storage_contents {
    use crate::declarations::data_queries::DataDefinitionLookup;
    use access_plans::FieldAccess;
    use language_semantics::Multiplicity;
    use symbols::SymbolHandle;
    use typed_trees::data::DataDefinition;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::data::{DataField, DataMember};
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    #[cfg(test)]
    mod substitution_tests {
        use super::*;
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        #[test]
        fn unrelated_static_operand_does_not_change_scalar_contents() {
            let mut symbols = SymbolTableBuilder::default();
            let parameter =
                symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("Phantom"));
            let holder = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Holder"));
            let mut program = typed_trees::TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let scalar = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: typed_trees::name::Identifier::from("u8"),
                });
            let formal = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: parameter,
                    name: typed_trees::name::Identifier::from("Phantom"),
                });
            let substitutions = [(parameter, TypeReferenceHandle::invalid())];
            let parameters =
                program
                    .data_type_parameters
                    .insert_many([typed_trees::data::TypeParameter {
                        symbol: parameter,
                        name: typed_trees::name::Identifier::from("Phantom"),
                        ..Default::default()
                    }]);
            let members = program
                .data_members
                .insert_many([DataMember::Field(DataField {
                    type_reference: formal,
                    relevance: language_core::BindingRelevance::Erased,
                    ..Default::default()
                })]);
            let data = DataDefinition {
                symbol: holder,
                name: typed_trees::name::Identifier::from("Holder"),
                type_parameters: parameters,
                members,
                ..Default::default()
            };
            program.push_data_definition(data.clone());
            for requirement in [
                ContentsRequirement::RepresentedOwned,
                ContentsRequirement::DescriptorCleanup,
            ] {
                assert!(has_contents(&program, scalar, &[], requirement));
                assert!(has_contents(&program, scalar, &substitutions, requirement));
                assert!(!has_contents(&program, formal, &substitutions, requirement));
            }
            assert!(!data_has_contents(
                &program,
                &data,
                &substitutions,
                ContentsRequirement::DescriptorCleanup
            ));
            assert!(data_has_contents(
                &program,
                &data,
                &[(parameter, scalar)],
                ContentsRequirement::DescriptorCleanup
            ));
            let extent = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: typed_trees::name::Identifier::from("6"),
                });
            let generic_array =
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::FixedArray {
                        element_type: scalar,
                        length: typed_trees::types::FixedArrayLength::ConstParameter {
                            symbol: parameter,
                            name: typed_trees::name::Identifier::from("Phantom"),
                        },
                    });
            for requirement in [
                ContentsRequirement::RepresentedOwned,
                ContentsRequirement::DescriptorCleanup,
            ] {
                assert!(
                    has_contents(&program, generic_array, &[(parameter, extent)], requirement),
                    "a closed array extent cannot change its contents capability"
                );
                assert!(!has_contents(
                    &program,
                    generic_array,
                    &substitutions,
                    requirement
                ));
            }
        }
    }

    #[derive(Clone, PartialEq, Eq)]
    pub(crate) enum ContentType {
        Scalar(PrimitiveType),
        Atomic(PrimitiveType),
        Array(Box<ContentType>, Option<usize>),
        Slice(Box<ContentType>),
        Shared(Box<ContentType>),
        Data {
            symbol: SymbolHandle,
            application: TypeReferenceHandle,
            arguments: Vec<ContentType>,
        },
        View,
        ConstValue(Option<usize>),
        RuntimeIndex,
        ImplementationSelection,
        UnclassifiedOperand,
        Provider,
    }

    impl ContentType {
        fn size(&self) -> usize {
            match self {
                Self::Scalar(_)
                | Self::Atomic(_)
                | Self::ConstValue(_)
                | Self::RuntimeIndex
                | Self::ImplementationSelection
                | Self::UnclassifiedOperand
                | Self::View
                | Self::Provider => 1,
                Self::Array(element, _) => 1 + element.size(),
                Self::Slice(element) | Self::Shared(element) => 1 + element.size(),
                Self::Data { arguments, .. } => 1 + arguments.iter().map(Self::size).sum::<usize>(),
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ContentsRequirement {
        PlainOwned,
        NumericOwned,
        RepresentedOwned,
        BorrowFree,
        LinearOwned,
        StoredValue,
        CleanupOwned,
        StructuralConstruction,
        OwnedOrSharedView,
        DescriptorCleanup,
        BorrowedStorage,
        StableObservation,
        SeamMarshal,
    }

    pub fn has_contents(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        requirement: ContentsRequirement,
    ) -> bool {
        check_contents_requirement(program, reference, substitutions, requirement)
    }

    pub fn data_has_contents(
        program: &dyn crate::CheckingInput,
        data: &DataDefinition,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        requirement: ContentsRequirement,
    ) -> bool {
        if let Some(application) = data.generic_instance {
            return has_contents(program, application, &[], requirement);
        }
        let parameters = program.data_type_parameters(data);
        let arguments = if parameters.is_empty() {
            Vec::new()
        } else {
            let Some(arguments) =
                contents_arguments(program, substitutions, parameters, requirement)
            else {
                return false;
            };
            let Some(arguments) = parameters
                .iter()
                .map(|parameter| {
                    arguments
                        .iter()
                        .rev()
                        .find(|(symbol, _)| *symbol == parameter.symbol)
                        .map(|(_, argument)| (parameter.symbol, argument.clone()))
                })
                .collect::<Option<Vec<_>>>()
            else {
                return false;
            };
            arguments
        };
        let resolved = ContentType::Data {
            symbol: data.symbol,
            application: TypeReferenceHandle::invalid(),
            arguments: arguments
                .into_iter()
                .map(|(_, argument)| argument)
                .collect(),
        };
        check_contents(
            program,
            &resolved,
            &mut Vec::new(),
            &mut Vec::new(),
            requirement,
        )
    }

    pub fn has_owned_or_shared_view_fields(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        owned_or_shared_view_fields(program, reference)
    }

    fn owned_or_shared_view_fields(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        let symbol = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => *symbol,
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if program
                .type_reference_table
                .type_reference_handles(*arguments)
                .is_empty() =>
            {
                *base_symbol
            }
            _ => return false,
        };
        if !symbol.is_valid() {
            return false;
        }
        let Some(data) = definition(program, symbol) else {
            return false;
        };
        if data.properties.multiplicity == Multiplicity::Linear
            || program.data_selects_cleanup(symbol)
        {
            return false;
        }
        let shared_view = |field: &DataField| {
            let TypeReferenceNode::Reference {
                access: language_semantics::ReferenceAccess::Shared,
                referee,
                ..
            } = program
                .type_reference_table
                .type_reference(field.type_reference)
            else {
                return false;
            };
            matches!(
                program
                    .type_reference_table
                    .type_reference(crate::peel_declared_constraints(program, *referee)),
                TypeReferenceNode::Slice { .. }
                    | TypeReferenceNode::Named { .. }
                    | TypeReferenceNode::Generic { .. }
            )
        };
        let mut has_view = false;
        program.data_members(data).iter().all(|member| {
            let field = match member {
                DataMember::Field(field) => field,
                _ => return false,
            };
            has_view |= shared_view(field);
            !field.relevance.is_erased()
                && (has_contents(
                    program,
                    field.type_reference,
                    &[],
                    ContentsRequirement::NumericOwned,
                ) || shared_view(field))
        }) && has_view
    }

    fn check_contents_requirement(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        requirement: ContentsRequirement,
    ) -> bool {
        let Some(arguments) = contents_arguments(program, substitutions, &[], requirement) else {
            return false;
        };
        resolve(program, reference, &arguments, requirement).is_some_and(|resolved| {
            check_contents(
                program,
                &resolved,
                &mut Vec::new(),
                &mut Vec::new(),
                requirement,
            )
        })
    }

    fn contents_arguments(
        program: &dyn crate::CheckingInput,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        parameters: &[typed_trees::data::TypeParameter],
        requirement: ContentsRequirement,
    ) -> Option<Vec<(SymbolHandle, ContentType)>> {
        let mut arguments = Vec::new();
        for (symbol, argument) in substitutions {
            let argument = match parameters
                .iter()
                .find(|parameter| parameter.symbol == *symbol)
            {
                Some(parameter) => resolve_parameter_argument(
                    program,
                    parameter,
                    *argument,
                    &arguments,
                    requirement,
                ),
                None => resolve(program, *argument, &arguments, requirement).or_else(|| {
                    program.declared_constant_extent(*argument).map(|extent| {
                        ContentType::ConstValue(
                            extent
                                .literal()
                                .and_then(|value| usize::try_from(value).ok()),
                        )
                    })
                }),
            }
            .unwrap_or(ContentType::UnclassifiedOperand);
            arguments.push((*symbol, argument));
        }
        Some(arguments)
    }

    fn resolve_parameter_argument(
        program: &dyn crate::CheckingInput,
        parameter: &typed_trees::data::TypeParameter,
        reference: TypeReferenceHandle,
        arguments: &[(SymbolHandle, ContentType)],
        requirement: ContentsRequirement,
    ) -> Option<ContentType> {
        match parameter.kind {
            TypeParameterKind::Value { .. } => Some(ContentType::RuntimeIndex),
            TypeParameterKind::Implementation => Some(ContentType::ImplementationSelection),
            TypeParameterKind::Type => resolve(program, reference, arguments, requirement),
            TypeParameterKind::Const { .. }
                if program
                    .type_reference_table
                    .contains_type_reference(reference)
                    && (requirement == ContentsRequirement::BorrowFree
                        || matches!(program.type_reference_table.type_reference(reference),
                        TypeReferenceNode::Named { symbol, .. } if !symbol.is_valid())) =>
            {
                Some(ContentType::ConstValue(
                    program
                        .declared_constant_extent(reference)
                        .and_then(|extent| extent.literal())
                        .and_then(|value| usize::try_from(value).ok()),
                ))
            }
            _ => None,
        }
    }

    fn resolve(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        arguments: &[(SymbolHandle, ContentType)],
        requirement: ContentsRequirement,
    ) -> Option<ContentType> {
        if !reference.is_valid() {
            return None;
        }
        if let Some(storage) = program.layout_storage_arguments(reference) {
            return resolve(program, storage.semantic_type, arguments, requirement);
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                if let Some(argument) =
                    crate::queries::substitutions::actual_for(arguments, *symbol)
                {
                    return (!matches!(
                        argument,
                        ContentType::ImplementationSelection | ContentType::UnclassifiedOperand
                    ))
                    .then_some(argument);
                }
                if let Some(primitive) = program.primitive_type_reference(reference) {
                    let atomic = match program.symbols.builtin_type_atom(*symbol) {
                        Some(atom) => atom.is_atomic(),
                        None => program
                            .access_field_plan_for_type_reference(reference)
                            .is_some_and(|field| {
                                matches!(field.access, FieldAccess::Atomic { .. })
                            }),
                    };
                    if atomic {
                        return Some(ContentType::Atomic(primitive));
                    }
                    return Some(ContentType::Scalar(primitive));
                }
                if requirement == ContentsRequirement::BorrowedStorage
                    && is_boundary_provider(program, *symbol)
                {
                    return Some(ContentType::Provider);
                }
                let data = definition(program, *symbol)?;
                program
                    .data_type_parameters(data)
                    .is_empty()
                    .then_some(ContentType::Data {
                        symbol: *symbol,
                        application: reference,
                        arguments: Vec::new(),
                    })
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments: source_arguments,
                ..
            } => {
                let data = definition(program, *base_symbol)?;
                let parameters = program.data_type_parameters(data);
                let source_arguments = program
                    .type_reference_table
                    .type_reference_handles(*source_arguments);
                if parameters.len() != source_arguments.len() {
                    return None;
                }
                let resolved = parameters
                    .iter()
                    .zip(source_arguments)
                    .map(|(parameter, reference)| {
                        resolve_parameter_argument(
                            program,
                            parameter,
                            *reference,
                            arguments,
                            requirement,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?;
                if let Some(instance) = program.closed_data_definition(reference) {
                    if instance.supply_mode != language_semantics::DataSupplyMode::CheckedShape
                        || instance.properties != data.properties
                        || (!matches!(
                            requirement,
                            ContentsRequirement::LinearOwned
                                | ContentsRequirement::StoredValue
                                | ContentsRequirement::BorrowFree
                                | ContentsRequirement::CleanupOwned
                                | ContentsRequirement::StructuralConstruction
                                | ContentsRequirement::BorrowedStorage
                        ) && program.data_selects_cleanup(*base_symbol))
                    {
                        return None;
                    }
                    return Some(ContentType::Data {
                        symbol: instance.symbol,
                        application: reference,
                        arguments: resolved,
                    });
                }
                Some(ContentType::Data {
                    symbol: *base_symbol,
                    application: reference,
                    arguments: resolved,
                })
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let length = match length {
                    typed_trees::types::FixedArrayLength::Literal(length) => Some(*length),
                    typed_trees::types::FixedArrayLength::ConstParameter { symbol, .. } => {
                        match crate::queries::substitutions::actual_for(arguments, *symbol) {
                            Some(ContentType::ConstValue(length)) => length,
                            _ => None,
                        }
                    }
                    typed_trees::types::FixedArrayLength::Expression(reference) => program
                        .declared_constant_extent(*reference)
                        .and_then(|extent| extent.literal())
                        .and_then(|value| usize::try_from(value).ok()),
                    typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
                };
                if length.is_none() && requirement != ContentsRequirement::BorrowFree {
                    return None;
                }
                Some(ContentType::Array(
                    Box::new(resolve(program, *element_type, arguments, requirement)?),
                    length,
                ))
            }
            TypeReferenceNode::Reference { .. }
                if matches!(
                    requirement,
                    ContentsRequirement::DescriptorCleanup
                        | ContentsRequirement::BorrowedStorage
                        | ContentsRequirement::StoredValue
                ) =>
            {
                Some(ContentType::View)
            }
            TypeReferenceNode::Reference {
                access: language_semantics::ReferenceAccess::Shared,
                referee,
                ..
            } if matches!(
                requirement,
                ContentsRequirement::StableObservation | ContentsRequirement::SeamMarshal
            ) =>
            {
                Some(ContentType::Shared(Box::new(resolve(
                    program,
                    *referee,
                    arguments,
                    requirement,
                )?)))
            }
            TypeReferenceNode::Reference {
                access: language_semantics::ReferenceAccess::Shared,
                referee,
                ..
            } if matches!(
                requirement,
                ContentsRequirement::OwnedOrSharedView
                    | ContentsRequirement::StructuralConstruction
            ) && matches!(
                program
                    .type_reference_table
                    .type_reference(crate::peel_declared_constraints(program, *referee)),
                TypeReferenceNode::Slice { .. }
                    | TypeReferenceNode::Named { .. }
                    | TypeReferenceNode::Generic { .. }
            ) =>
            {
                Some(ContentType::View)
            }
            TypeReferenceNode::DynamicTrait { symbol, .. }
                if requirement == ContentsRequirement::BorrowedStorage
                    && is_boundary_provider(program, *symbol) =>
            {
                Some(ContentType::Provider)
            }
            TypeReferenceNode::DynamicTrait { .. }
                if requirement != ContentsRequirement::SeamMarshal =>
            {
                Some(ContentType::Scalar(PrimitiveType::Selector))
            }
            TypeReferenceNode::Slice { element_type }
                if matches!(
                    requirement,
                    ContentsRequirement::StableObservation | ContentsRequirement::SeamMarshal
                ) =>
            {
                Some(ContentType::Slice(Box::new(resolve(
                    program,
                    *element_type,
                    arguments,
                    requirement,
                )?)))
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                if requirement == ContentsRequirement::DescriptorCleanup
                    && !matches!(
                        crate::value_custody::cleanup_selection::selected_application(
                            program, reference
                        ),
                        Ok(None)
                    )
                {
                    return None;
                }
                let rows = program.type_reference_table.constraints(*constraints);
                if requirement == ContentsRequirement::PlainOwned
                    || rows.len() != constraints.count() as usize
                    || rows.is_empty()
                {
                    return None;
                }
                let contents = resolve(program, *base_type, arguments, requirement)?;
                let understood = rows.iter().all(|constraint| match constraint {

                typed_trees::types::TypeConstraintNode::ArithmeticDomain(_) => true,
                typed_trees::types::TypeConstraintNode::Domain(domain) => {
                    if matches!(requirement,
                        ContentsRequirement::StructuralConstruction
                            | ContentsRequirement::StoredValue
                            | ContentsRequirement::RepresentedOwned
                            | ContentsRequirement::BorrowedStorage
                            | ContentsRequirement::BorrowFree
                            | ContentsRequirement::DescriptorCleanup)
                        && domain.subject == typed_trees::types::DomainConstraintSubject::Declared
                        && domain.semantic_id.is_valid()
                        && typed_trees::domain::domain_by_symbol(program, domain.symbol).is_some()
                    {
                        return true;
                    }
                    if matches!(domain.subject,
                        typed_trees::types::DomainConstraintSubject::Value(_))
                    {
                        return matches!(contents, ContentType::Scalar(primitive)
                            if crate::scalar_field_domain_predicate(program, domain, primitive).is_some());
                    }
                    (matches!(
                        requirement,
                        ContentsRequirement::StableObservation
                            | ContentsRequirement::SeamMarshal
                            | ContentsRequirement::LinearOwned
                            | ContentsRequirement::StoredValue
                            | ContentsRequirement::OwnedOrSharedView
                    ) && is_plain_value_domain(program, domain))
                        || (matches!(
                            requirement,
                            ContentsRequirement::NumericOwned
                                | ContentsRequirement::RepresentedOwned
                                    | ContentsRequirement::StoredValue
                                | ContentsRequirement::BorrowFree
                                | ContentsRequirement::StructuralConstruction
                                | ContentsRequirement::BorrowedStorage
                        ) && matches!(contents, ContentType::Scalar(_) | ContentType::Atomic(_))
                            && is_plain_value_domain(program, domain)
                            && (crate::exact_declared_domain_interval(program, domain).is_some()
                                || matches!(contents, ContentType::Scalar(primitive)
                                    if crate::scalar_field_domain_predicate(program, domain, primitive).is_some())))
                        || (matches!(requirement,
                                ContentsRequirement::RepresentedOwned
                                    | ContentsRequirement::StoredValue
                                    | ContentsRequirement::BorrowFree
                                    | ContentsRequirement::StructuralConstruction
                                    | ContentsRequirement::BorrowedStorage)
                            && matches!(&contents, ContentType::Array(element, _)
                                if matches!(element.as_ref(), ContentType::Scalar(PrimitiveType::U8)))
                            && is_plain_value_domain(program, domain)
                            && domain.arguments.is_empty()
                            && typed_trees::byte_predicates::domain_byte_predicate(
                                program, domain.symbol,
                            ).is_some())
                }
                typed_trees::types::TypeConstraintNode::Named(_) => false,
            });
                understood.then_some(contents)
            }
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => None,
        }
    }

    pub(crate) fn is_plain_value_domain(
        program: &dyn crate::CheckingInput,
        domain: &typed_trees::types::DomainConstraint,
    ) -> bool {
        if domain.subject != typed_trees::types::DomainConstraintSubject::Declared
            || !domain.symbol.is_valid()
            || domain.classification.is_some()
            || !domain.semantic_roles.is_empty()
            || !domain.establishment_routes.is_empty()
        {
            return false;
        }
        program
            .domain_definition(domain.symbol)
            .is_some_and(|definition| {
                definition.alias.is_none()
                    && definition.classification.is_none()
                    && definition.semantic_roles.is_empty()
                    && definition.establishment_routes.is_empty()
            })
    }

    fn is_boundary_provider(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> bool {
        program
            .traits()
            .iter()
            .any(|definition| definition.symbol == symbol && definition.is_boundary)
    }

    fn definition(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&DataDefinition> {
        if !symbol.is_valid() {
            return None;
        }
        match crate::declarations::data_queries::memoized_data_definition_lookup(program, symbol) {
            DataDefinitionLookup::Unique(position) => {
                program.data_definitions().get(position as usize)
            }
            _ => None,
        }
    }

    fn check_contents(
        program: &dyn crate::CheckingInput,
        resolved: &ContentType,
        active: &mut Vec<(SymbolHandle, usize)>,
        complete: &mut Vec<ContentType>,
        requirement: ContentsRequirement,
    ) -> bool {
        if complete.contains(resolved) {
            return true;
        }
        let ContentType::Data {
            symbol,
            application,
            arguments,
        } = resolved
        else {
            return match resolved {
                ContentType::Scalar(_) => true,
                ContentType::Atomic(_) => !matches!(
                    requirement,
                    ContentsRequirement::StableObservation
                        | ContentsRequirement::SeamMarshal
                        | ContentsRequirement::DescriptorCleanup
                ),
                ContentType::Array(element, length) => {
                    (length.is_some() || requirement == ContentsRequirement::BorrowFree)
                        && check_contents(program, element, active, complete, requirement)
                }
                ContentType::Slice(element) | ContentType::Shared(element) => {
                    check_contents(program, element, active, complete, requirement)
                }
                ContentType::View | ContentType::Provider => true,
                ContentType::ConstValue(_)
                | ContentType::RuntimeIndex
                | ContentType::ImplementationSelection
                | ContentType::UnclassifiedOperand => false,
                ContentType::Data { .. } => unreachable!(),
            };
        };
        let (data, substitutions) = if let Some(data) = program.closed_data_definition(*application)
        {
            (data, Vec::new())
        } else {
            let Some(data) = definition(program, *symbol) else {
                return false;
            };
            let parameters = program.data_type_parameters(data);
            if parameters.len() != arguments.len() {
                return false;
            }
            let substitutions = parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.symbol, argument.clone()))
                .collect::<Vec<_>>();
            (data, substitutions)
        };
        // A boundary-opaque declaration authors no fields. Once the build
        // selects its representation, its contents are those of the selected
        // carrier, so every contents question about the opaque value is
        // answered from that carrier here, in the one place that walks
        // contents. The declaration's own multiplicity and cleanup selection
        // still apply below: the carrier supplies the shape, the opaque
        // declaration keeps deciding whether a value may be copied.
        let selected_carrier = (data.supply_mode
            == language_semantics::DataSupplyMode::BoundaryOpaque)
            .then(|| {
                representation_selections::selection_for_opaque(
                    program.opaque_representation_selections(),
                    data.symbol,
                )
            })
            .flatten()
            .map(|selection| selection.carrier_type());
        // An opaque declaration's empty field roster says nothing about hidden
        // references. The sealed toolchain activation claim is the exception:
        // its backing is activation-local, and the activation storage contract
        // owns every loan of it, so no caller place hides behind the claim.
        if requirement == ContentsRequirement::BorrowFree
            && selected_carrier.is_none()
            && data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            && !crate::value_custody::activation_storage::is_toolchain_activation_claim(
                program, data,
            )
        {
            return false;
        }
        if (requirement == ContentsRequirement::DescriptorCleanup
            && data.cleanup_selection.is_some())
            || (data.properties.multiplicity == Multiplicity::Linear
                && !matches!(
                    requirement,
                    ContentsRequirement::LinearOwned
                        | ContentsRequirement::StoredValue
                        | ContentsRequirement::BorrowFree
                        | ContentsRequirement::StructuralConstruction
                        | ContentsRequirement::BorrowedStorage
                ))
            || active
                .iter()
                .any(|&(owner, size)| owner == *symbol && resolved.size() >= size)
            || (!matches!(
                requirement,
                ContentsRequirement::LinearOwned
                    | ContentsRequirement::StoredValue
                    | ContentsRequirement::BorrowFree
                    | ContentsRequirement::CleanupOwned
                    | ContentsRequirement::StructuralConstruction
                    | ContentsRequirement::BorrowedStorage
            ) && program.data_selects_cleanup(*symbol))
        {
            return false;
        }
        active.push((*symbol, resolved.size()));
        let mut check_field = |field: &typed_trees::data::DataField| {
            if field.relevance.is_erased() {
                if requirement == ContentsRequirement::BorrowFree {
                    return resolve(program, field.type_reference, &substitutions, requirement)
                        .is_some_and(|field| {
                            check_contents(program, &field, active, complete, requirement)
                        });
                }
                return matches!(
                    requirement,
                    ContentsRequirement::NumericOwned
                        | ContentsRequirement::CleanupOwned
                        | ContentsRequirement::StructuralConstruction
                        | ContentsRequirement::StoredValue
                        | ContentsRequirement::BorrowedStorage
                        | ContentsRequirement::DescriptorCleanup
                        | ContentsRequirement::SeamMarshal
                ) && resolve(program, field.type_reference, &substitutions, requirement)
                    .is_some();
            }
            resolve(program, field.type_reference, &substitutions, requirement)
                .is_some_and(|field| check_contents(program, &field, active, complete, requirement))
        };
        let supported = match selected_carrier {
            Some(carrier) => resolve(program, carrier, &[], requirement).is_some_and(|carrier| {
                check_contents(program, &carrier, active, complete, requirement)
            }),
            None => program
                .data_members(data)
                .iter()
                .all(|member| match member {
                    DataMember::Field(field) => check_field(field),
                    DataMember::Variant(variant) => program
                        .data_payload_fields(variant)
                        .iter()
                        .all(&mut check_field),
                }),
        };
        active.pop();
        if supported {
            complete.push(resolved.clone());
        }
        supported
    }
}

pub mod structural_identity {
    use crate::value_custody::reference_shells::{reference_shell, slice_view_shell};
    use symbols::SymbolHandle;
    use typed_trees::type_identity::TypeIdentityRequest;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub fn structural_identity_type(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> TypeReferenceHandle {
        let mut reference = type_reference;
        for _ in 0..=program.type_reference_table.type_reference_count() {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { referee, .. } => reference = *referee,
                TypeReferenceNode::Constrained { constraints, .. }
                    if program
                        .primitive_type_reference(reference)
                        .is_some_and(|primitive| {
                            program
                                .type_reference_table
                                .constraints(*constraints)
                                .iter()
                                .any(|constraint| {
                                    use typed_trees::types::TypeConstraintNode;
                                    match constraint {
                                    TypeConstraintNode::Named(_) => true,
                                    TypeConstraintNode::Domain(domain) => domain.subject
                                        != typed_trees::types::DomainConstraintSubject::Value(
                                            language_semantics::value_domain::ValueDomain::Finite,
                                        )
                                        && !crate::proof_contracts::domains::scalar_tag_constraint(
                                            program, domain, primitive,
                                        ),
                                    _ => false,
                                }
                                })
                        }) =>
                {
                    return reference;
                }
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Named { symbol, .. } => {
                    match crate::queries::substitutions::actual_for(substitutions, *symbol) {
                        Some(actual) if actual != reference => reference = actual,
                        _ => return reference,
                    }
                }
                _ => return reference,
            }
        }
        reference
    }

    pub fn structural_storage_type(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> TypeReferenceHandle {
        let Some(outer) = crate::value_custody::reference_shells::peel_carrier_shells(
            program,
            type_reference,
            substitutions,
        ) else {
            return TypeReferenceHandle::invalid();
        };
        if let TypeReferenceNode::Reference { referee, .. } =
            program.type_reference_table.type_reference(outer)
        {
            let Some(stored) = crate::value_custody::reference_shells::peel_carrier_shells(
                program,
                *referee,
                substitutions,
            ) else {
                return TypeReferenceHandle::invalid();
            };
            if matches!(
                program.type_reference_table.type_reference(stored),
                TypeReferenceNode::Reference { .. }
            ) {
                let mut selected = stored;
                let mut remaining = program.type_reference_table.type_reference_count();
                while let TypeReferenceNode::Reference { referee, .. } =
                    program.type_reference_table.type_reference(selected)
                {
                    let Some(next) = remaining.checked_sub(1) else {
                        return TypeReferenceHandle::invalid();
                    };
                    remaining = next;
                    let Some(next) = crate::value_custody::reference_shells::peel_carrier_shells(
                        program,
                        *referee,
                        substitutions,
                    ) else {
                        return TypeReferenceHandle::invalid();
                    };
                    selected = next;
                }
                return stored;
            }
        }
        structural_identity_type(program, type_reference, substitutions)
    }

    pub fn logical_structural_type_identity(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> typed_trees::type_identity::TypeIdentityId {
        program.storage_type_identity(TypeIdentityRequest {
            binders,
            substitutions,
            ..TypeIdentityRequest::ordinary(structural_identity_type(
                program,
                type_reference,
                substitutions,
            ))
        })
    }

    pub fn structural_type_identity(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> typed_trees::type_identity::TypeIdentityId {
        let storage = structural_storage_type(program, type_reference, substitutions);
        if !storage.is_valid() {
            return Default::default();
        }
        program.storage_type_identity(TypeIdentityRequest {
            binders,
            substitutions,
            ..TypeIdentityRequest::ordinary(storage)
        })
    }

    pub fn stores_reference_shape(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        reference_shell(program, type_reference).is_some()
            && crate::stored_view_shell(program, type_reference, substitutions).is_none()
    }

    pub fn stored_structural_type_identity(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> typed_trees::type_identity::TypeIdentityId {
        if !structural_storage_type(program, type_reference, substitutions).is_valid() {
            return Default::default();
        }
        if stores_reference_shape(program, type_reference, substitutions)
            || slice_view_shell(program, type_reference, substitutions).is_some()
        {
            return program.storage_type_identity(TypeIdentityRequest {
                binders,
                substitutions,
                ..TypeIdentityRequest::ordinary(type_reference)
            });
        }
        structural_type_identity(program, type_reference, binders, substitutions)
    }
}

pub mod transition_operand_moves {
    use crate::frozen_program::expression_table;
    use std::collections::HashSet;

    use checked_trees::CheckFacts;
    use language_semantics::PermissionEventSource;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::TransitionTargetNode;

    pub fn transition_operand_moves(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
        statement_index: usize,
        local: SymbolHandle,
    ) -> Option<Vec<PermissionEventSource>> {
        let statements = program.statement_table.statements(state.statement_nodes);
        let transition = statements.get(statement_index)?.as_transition()?;
        let operands = match program.statement_table.transition_target(transition.target) {
            TransitionTargetNode::Named { arguments, .. } => {
                program.statement_table.expression_handles(*arguments)
            }
            TransitionTargetNode::Value(value) => std::slice::from_ref(value),
            _ => return Some(Vec::new()),
        };
        let mut nodes = Vec::new();
        let mut seen = HashSet::new();
        for argument in operands {
            crate::queries::expression_traversal::collect_expression_nodes(
                program, *argument, &mut nodes, &mut seen,
            );
        }
        let flow = facts.flow.control.state(machine, state.symbol)?;
        let calls = facts.flow.control.calls.span_or_empty(flow.calls);
        let mut moves = Vec::new();
        for node in nodes {
            let Some(call) = expression_table(program).call_node(node) else {
                continue;
            };
            let names_local = expression_table(program)
                .expression_handles(call.arguments)
                .iter()
                .any(|argument| {
                    matches!(expression_table(program).expression(*argument),
                    ExpressionNode::Name(name) if name.symbol == local
                        && name.head_symbol == local
                        && program.expression_table.name_path_members(name.members).len() == 1)
                });
            if !names_local {
                continue;
            }
            let mut occurrences = calls.iter().filter(|fact| {
                fact.statement_index == statement_index && fact.authored_expression == node
            });
            let occurrence = occurrences.next()?;
            if occurrences.next().is_some() {
                return None;
            }
            moves.push(PermissionEventSource::Call {
                statement_index,
                call_ordinal: occurrence.call_ordinal,
                target_symbol: occurrence.target_symbol,
            });
        }
        Some(moves)
    }
}

pub mod type_properties {
    use crate::CheckingInput;
    use crate::value_custody::claim_frontier::{ClaimFrontierClaim, DeclaredClaimFrontier};
    use std::sync::{Arc, OnceLock};
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) struct ClaimFrontierQueries {
        rows: Vec<(TypeReferenceHandle, OnceLock<Arc<DeclaredClaimFrontier>>)>,
    }

    impl ClaimFrontierQueries {
        fn of(program: &dyn CheckingInput) -> Self {
            let mut rows: Vec<_> = program
                .type_reference_table
                .type_reference_rows()
                .map(|(handle, _)| (handle, OnceLock::new()))
                .collect();
            rows.sort_by_key(|(handle, _)| (handle.arena_index(), handle.generation()));
            Self { rows }
        }
    }

    pub(crate) fn memoized_claim_frontier(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<ClaimFrontierClaim> {
        memoized_declared_claim_frontier(program, type_reference)
            .claims()
            .to_vec()
    }

    pub(crate) fn memoized_declared_claim_frontier(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Arc<DeclaredClaimFrontier> {
        let compute = || {
            Arc::new(DeclaredClaimFrontier::new(
                crate::value_custody::claim_frontier::linear_claim_frontier_uncached(
                    program,
                    type_reference,
                ),
            ))
        };
        let Some(memos) = program.checking_memos() else {
            return compute();
        };
        let queries = memos
            .claim_frontiers
            .get_or_init(|| ClaimFrontierQueries::of(program));
        let Ok(position) = queries.rows.binary_search_by_key(
            &(type_reference.arena_index(), type_reference.generation()),
            |(handle, _)| (handle.arena_index(), handle.generation()),
        ) else {
            return compute();
        };
        queries.rows[position].1.get_or_init(compute).clone()
    }

    pub(crate) struct EstablishmentQueries {
        rows: Vec<(
            crate::value_custody::data::EstablishmentIdentity,
            OnceLock<bool>,
        )>,
        type_rows: Vec<(TypeReferenceHandle, OnceLock<bool>)>,
    }

    impl EstablishmentQueries {
        fn of(program: &dyn CheckingInput) -> Self {
            let mut identities = program
                .data_definitions()
                .iter()
                .map(crate::value_custody::data::EstablishmentIdentity::of)
                .collect::<Vec<_>>();
            identities.sort_by_key(|identity| identity.order());
            identities.dedup();
            let mut type_rows: Vec<_> = program
                .type_reference_table
                .type_reference_rows()
                .map(|(handle, _)| (handle, OnceLock::new()))
                .collect();
            type_rows.sort_by_key(|(handle, _)| (handle.arena_index(), handle.generation()));
            Self {
                rows: identities
                    .into_iter()
                    .map(|identity| (identity, OnceLock::new()))
                    .collect(),
                type_rows,
            }
        }
    }

    pub(crate) fn memoized_type_requires_establishment(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
        compute: impl FnOnce() -> bool,
    ) -> bool {
        let Some(memos) = program.checking_memos() else {
            return compute();
        };
        let queries = memos
            .establishment
            .get_or_init(|| EstablishmentQueries::of(program));
        let Ok(position) = queries.type_rows.binary_search_by_key(
            &(type_reference.arena_index(), type_reference.generation()),
            |(handle, _)| (handle.arena_index(), handle.generation()),
        ) else {
            return compute();
        };
        *queries.type_rows[position].1.get_or_init(compute)
    }

    pub(crate) fn memoized_data_requires_establishment(
        program: &dyn CheckingInput,
        definition: &typed_trees::data::DataDefinition,
        compute: impl FnOnce() -> bool,
    ) -> bool {
        let Some(memos) = program.checking_memos() else {
            return compute();
        };
        let queries = memos
            .establishment
            .get_or_init(|| EstablishmentQueries::of(program));
        let Ok(position) = queries.rows.binary_search_by_key(
            &crate::value_custody::data::EstablishmentIdentity::of(definition).order(),
            |(identity, _)| identity.order(),
        ) else {
            return compute();
        };
        *queries.rows[position].1.get_or_init(compute)
    }
}

pub mod type_shapes {
    use typed_trees::data::TypeParameterKind;

    use typed_trees::types::{
        PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };

    pub(crate) fn exact_integer_primitive(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        allow_addr: bool,
    ) -> Option<PrimitiveType> {
        let primitive = program
            .type_reference_table
            .primitive_type(type_reference)?;
        let fixed = matches!(
            primitive,
            PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
        ) || (allow_addr && primitive == PrimitiveType::Addr);
        if !fixed
            || program.arithmetic_domain_for_type_reference(type_reference)
                != numerics::arithmetic::ArithmeticDomain::Exact
        {
            return None;
        }
        Some(primitive)
    }

    pub(crate) fn collection_element_and_length(
        program: &dyn crate::CheckingInput,
        collection_type: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Option<usize>)> {
        match program.type_reference_table.type_reference(collection_type) {
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => Some((
                *element_type,
                match length {
                    typed_trees::types::FixedArrayLength::Literal(length) => Some(*length),
                    _ => None,
                },
            )),
            TypeReferenceNode::Slice { element_type } => Some((*element_type, None)),
            _ => None,
        }
    }

    pub(crate) fn has_slice_base(
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> bool {
        let mut visited = Vec::new();
        while reference.is_valid() && !visited.contains(&reference) {
            visited.push(reference);
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Slice { .. } => return true,
                TypeReferenceNode::Reference { referee, .. } => reference = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                _ => return false,
            }
        }
        false
    }

    pub(crate) fn scalar_parameter_type_pair(
        required: &TypeParameterKind,
        actual: &TypeParameterKind,
    ) -> Option<(TypeReferenceHandle, TypeReferenceHandle)> {
        match (required, actual) {
            (
                TypeParameterKind::Const {
                    type_reference: required_type,
                },
                TypeParameterKind::Const {
                    type_reference: actual_type,
                },
            )
            | (
                TypeParameterKind::Value {
                    type_reference: required_type,
                },
                TypeParameterKind::Value {
                    type_reference: actual_type,
                },
            ) => Some((*required_type, *actual_type)),
            _ => None,
        }
    }

    pub(crate) fn attenuated_reference_pair(
        program: &dyn crate::CheckingInput,
        actual: TypeReferenceHandle,
        required: TypeReferenceHandle,
    ) -> (TypeReferenceHandle, TypeReferenceHandle) {
        match (
            program.type_reference_table.type_reference(actual),
            program.type_reference_table.type_reference(required),
        ) {
            (
                TypeReferenceNode::Reference {
                    referee: actual,
                    access: language_semantics::ReferenceAccess::Mutable,
                    ..
                },
                TypeReferenceNode::Reference {
                    referee: required,
                    access: language_semantics::ReferenceAccess::Shared,
                    ..
                },
            ) => (*actual, *required),
            _ => (actual, required),
        }
    }

    pub(crate) fn head_data_symbol(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<symbols::SymbolHandle> {
        program.type_reference_table.type_head_symbol(reference)
    }

    pub fn exact_constrained_shells(
        program: &dyn crate::CheckingInput,
        mut handle: TypeReferenceHandle,
    ) -> Option<Vec<&[TypeConstraintNode]>> {
        let mut shells = Vec::new();
        loop {
            handle = program
                .type_reference_table
                .dereferenced_type_reference(handle);
            match program.type_reference_table.type_reference(handle) {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    let constraints = program.type_reference_table.constraints(*constraints);
                    if constraints.iter().any(|constraint| {
                        matches!(
                            constraint,
                            TypeConstraintNode::ArithmeticDomain(domain)
                                if *domain != numerics::arithmetic::ArithmeticDomain::Exact
                        )
                    }) {
                        return None;
                    }
                    shells.push(constraints);
                    handle = *base_type;
                }
                _ => return Some(shells),
            }
        }
    }
}

pub use executable_value_occurrences::{
    collect_executable_value_origins, visit_executable_value_occurrences,
    visit_statement_value_occurrences,
};
