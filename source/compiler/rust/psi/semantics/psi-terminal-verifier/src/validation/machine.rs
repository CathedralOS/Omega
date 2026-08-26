//! Registers and validates one terminal machine in canonical source order.

use super::affine_cleanup::{
    nominal_cleanup_contract_receiver, nominal_cleanups, validate_nominal_affine_cleanup_shape,
    validate_partial_affine_cleanup_shape,
};
use super::crash::{substitute_crash_routes, validate_crash_frontiers};
use super::structural_operations::{validate_service_reach, validate_unit_operation_static};
use super::*;

pub(super) fn validate_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    registry: &mut IdRegistry,
    _policy: ValidationPolicy,
) -> Result<(), ModuleError> {
    if machine.blocks.is_empty() {
        return Err(ModuleError::MachineHasNoBlocks(machine.id));
    }

    let contract_receiver = nominal_cleanup_contract_receiver(module, machine.id);
    let mut blocks = BTreeMap::new();
    let mut value_types = BTreeMap::new();
    let mut structural_roots = BTreeSet::new();
    let mut structural_place_kinds = BTreeMap::new();
    for place in &machine.structural_places {
        insert_unique(&mut registry.places, place.id, ModuleError::DuplicatePlace)?;
        if matches!(machine.result, TerminalMachineResult::Unit)
            && place.kind == psi_core::StructuralPlaceKind::Result
        {
            return Err(ModuleError::UnitMachineHasResultStructuralPlace {
                machine: machine.id,
                place: place.id,
            });
        }
        let root = match place.kind {
            psi_core::StructuralPlaceKind::Parameter { position, .. } => {
                StructuralRootKey::Parameter(position)
            }
            psi_core::StructuralPlaceKind::Result => StructuralRootKey::Result,
            psi_core::StructuralPlaceKind::OperationResult { producer, .. } => {
                StructuralRootKey::OperationResult(producer)
            }
            psi_core::StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                ..
            } => StructuralRootKey::ByteSequenceLiteral(declaration_ordinal),
            psi_core::StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => StructuralRootKey::ProviderAttachment(attachment, field, boundary),
            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } => StructuralRootKey::TrivialAffineLocal(declaration_ordinal),
        };
        if !structural_roots.insert(root) {
            return Err(ModuleError::DuplicateStructuralPlaceRoot {
                machine: machine.id,
                kind: place.kind,
            });
        }
        structural_place_kinds.insert(place.id, place.kind);
    }
    for declaration in machine.parameters.iter().chain(machine.result.scalar_ref()) {
        insert_value(
            &mut value_types,
            &mut registry.values,
            declaration.id,
            declaration.scalar_type,
        )?;
    }
    for block in &machine.blocks {
        insert_unique(&mut registry.blocks, block.id, ModuleError::DuplicateBlock)?;
        if blocks.insert(block.id, block).is_some() {
            return Err(ModuleError::DuplicateBlock(block.id));
        }
        for parameter in &block.parameters {
            insert_value(
                &mut value_types,
                &mut registry.values,
                parameter.id,
                parameter.scalar_type,
            )?;
        }
        for operation in &block.operations {
            insert_unique(
                &mut registry.operations,
                operation.id,
                ModuleError::DuplicateOperation,
            )?;
            if let OperationKind::CallStructuralScalar {
                requirement_obligations,
                ..
            } = &operation.kind
            {
                let Some(result) = operation.result.scalar() else {
                    return Err(ModuleError::ScalarOperationHasUnitResult(operation.id));
                };
                insert_value(
                    &mut value_types,
                    &mut registry.values,
                    result.id,
                    result.scalar_type,
                )?;
                validate_unit_operation_static(module, machine, machines, operation)?;
                for obligation in requirement_obligations {
                    insert_unique(
                        &mut registry.obligations,
                        *obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                continue;
            }
            if let OperationKind::CallStructural {
                requirement_obligations,
                ..
            } = &operation.kind
            {
                validate_unit_operation_static(module, machine, machines, operation)?;
                for obligation in requirement_obligations {
                    insert_unique(
                        &mut registry.obligations,
                        *obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                continue;
            }
            if matches!(
                operation.kind,
                OperationKind::CallUnit { .. }
                    | OperationKind::PortWrite { .. }
                    | OperationKind::EstablishByteSequenceLiteral { .. }
                    | OperationKind::EstablishTrivialAffineLocal { .. }
            ) {
                if !matches!(operation.result, psi_terminal::OperationResult::Unit) {
                    return Err(ModuleError::UnitOperationHasScalarResult(operation.id));
                }
                validate_unit_operation_static(module, machine, machines, operation)?;
                if let OperationKind::CallUnit {
                    requirement_obligations,
                    ..
                } = &operation.kind
                {
                    for obligation in requirement_obligations {
                        insert_unique(
                            &mut registry.obligations,
                            *obligation,
                            ModuleError::DuplicateObligation,
                        )?;
                    }
                }
                continue;
            }
            if let OperationKind::BoundaryCall { boundary, .. } = &operation.kind {
                let boundary = module
                    .boundary_machines
                    .iter()
                    .find(|candidate| candidate.id == *boundary)
                    .ok_or(ModuleError::UnknownBoundaryCallTarget {
                        operation: operation.id,
                        boundary: *boundary,
                    })?;
                let actual = operation.result.scalar().map(|result| result.scalar_type);
                if actual != boundary.result {
                    return Err(ModuleError::BoundaryCallResultMismatch {
                        operation: operation.id,
                        expected: boundary.result,
                        actual,
                    });
                }
                if let Some(result) = operation.result.scalar() {
                    insert_value(
                        &mut value_types,
                        &mut registry.values,
                        result.id,
                        result.scalar_type,
                    )?;
                }
                validate_unit_operation_static(module, machine, machines, operation)?;
                continue;
            }
            let Some(result) = operation.result.scalar() else {
                return Err(ModuleError::ScalarOperationHasUnitResult(operation.id));
            };
            insert_value(
                &mut value_types,
                &mut registry.values,
                result.id,
                result.scalar_type,
            )?;
            match operation.kind.clone() {
                OperationKind::CallUnit { .. }
                | OperationKind::CallStructuralScalar { .. }
                | OperationKind::CallStructural { .. }
                | OperationKind::PortWrite { .. }
                | OperationKind::EstablishByteSequenceLiteral { .. }
                | OperationKind::EstablishTrivialAffineLocal { .. } => {
                    unreachable!("structural/effect operations were validated above")
                }
                OperationKind::BoundaryCall { .. } => {
                    unreachable!("boundary calls were validated above")
                }
                OperationKind::Call {
                    callee,
                    arguments,
                    requirement_obligations,
                    crash_continuations,
                } => {
                    let callee =
                        machines
                            .get(&callee)
                            .copied()
                            .ok_or(ModuleError::UnknownCallTarget {
                                operation: operation.id,
                                callee,
                            })?;
                    validate_service_reach(
                        operation.id,
                        &machine.published_service_ceiling,
                        &callee.published_service_ceiling,
                    )?;
                    if crash_continuations
                        .windows(2)
                        .any(|pair| pair[0].cause >= pair[1].cause)
                    {
                        return Err(ModuleError::NonCanonicalCallCrashContinuations(
                            operation.id,
                        ));
                    }
                    let substitutions = callee
                        .parameters
                        .iter()
                        .zip(&arguments)
                        .map(|(parameter, argument)| {
                            (
                                parameter.id,
                                ScalarTerm::value(*argument, parameter.scalar_type),
                            )
                        })
                        .collect::<BTreeMap<_, _>>();
                    let expected_crash_continuations =
                        substitute_crash_routes(&callee.contract.crash_routes, &substitutions);
                    if crash_continuations != expected_crash_continuations {
                        return Err(ModuleError::CallCrashContinuationsMismatch {
                            operation: operation.id,
                            callee: callee.id,
                        });
                    }
                    for continuation in &crash_continuations {
                        let covered = machine.contract.crash_routes.iter().any(|published| {
                            published.cause == continuation.cause
                                && (published.alternatives == [CrashRouteGuard::Truth]
                                    || continuation
                                        .alternatives
                                        .iter()
                                        .all(|route| published.alternatives.contains(route)))
                        });
                        if !covered {
                            return Err(ModuleError::CallCrashContinuationUncovered {
                                operation: operation.id,
                                cause: continuation.cause,
                            });
                        }
                    }
                    if !callee.structural_places.is_empty()
                        || !callee.content_entry_claims.is_empty()
                        || !callee.content_identity_reshuffles.is_empty()
                        || !callee.content_partition_compositions.is_empty()
                        || callee
                            .contract
                            .requires
                            .iter()
                            .chain(
                                callee
                                    .contract
                                    .ensures
                                    .iter()
                                    .map(|clause| &clause.proposition),
                            )
                            .any(propositions::proposition_contains_content)
                    {
                        return Err(ModuleError::CallTargetHasStructuralContract {
                            operation: operation.id,
                            callee: callee.id,
                        });
                    }
                    let Some(callee_result) = callee.result.scalar() else {
                        return Err(ModuleError::CallTargetReturnsUnit {
                            operation: operation.id,
                            callee: callee.id,
                        });
                    };
                    if operation.result.expect_scalar().scalar_type != callee_result.scalar_type {
                        return Err(ModuleError::CallResultTypeMismatch {
                            operation: operation.id,
                            expected: callee_result.scalar_type,
                            actual: operation.result.expect_scalar().scalar_type,
                        });
                    }
                    if requirement_obligations.len() != callee.contract.requires.len() {
                        return Err(ModuleError::CallRequirementArityMismatch {
                            operation: operation.id,
                            expected: callee.contract.requires.len(),
                            actual: requirement_obligations.len(),
                        });
                    }
                    for obligation in requirement_obligations {
                        insert_unique(
                            &mut registry.obligations,
                            obligation,
                            ModuleError::DuplicateObligation,
                        )?;
                    }
                }
                OperationKind::IntegerConstant { value } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(ModuleError::IntegerConstantRequiresIntegerResult(
                            operation.id,
                        ));
                    };
                    if !integer_type.admits(value) {
                        return Err(ModuleError::IntegerConstantOutsideResultType(operation.id));
                    }
                }
                OperationKind::BooleanConstant { .. } => {
                    if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanConstantRequiresBooleanResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::BooleanStructuralField { source, field } => {
                    if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanStructuralFieldRequiresBooleanResult(
                            operation.id,
                        ));
                    }
                    validate_boolean_structural_field(
                        module,
                        machine,
                        operation.id,
                        source,
                        field,
                    )?;
                }
                OperationKind::BooleanNot { .. } => {
                    if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanNotRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::BooleanEqual { .. } => {
                    if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanEqualRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::IntegerEqual { .. } => {
                    if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::IntegerEqualRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::IntegerLessThan { .. }
                | OperationKind::IntegerLessOrEqual { .. } => {
                    if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::IntegerOrderingRequiresBooleanResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerBitwiseAnd { .. }
                | OperationKind::IntegerBitwiseOr { .. }
                | OperationKind::IntegerBitwiseXor { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::IntegerBitwiseRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerBitwiseNot { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::IntegerBitwiseRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerWiden { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::IntegerWidenRequiresIntegerResult(operation.id));
                    }
                }
                OperationKind::IntegerExactCast { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::IntegerExactCastRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::WrappingIntegerShiftLeft { .. }
                | OperationKind::WrappingIntegerShiftRight { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerShiftRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::ExactIntegerShiftRight { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerShiftRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerShiftLeft { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerShiftRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerAdd { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerSubtract { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerMultiply { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerMultiplyRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerDivide { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerDivideRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerRemainder { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerRemainderRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::WrappingIntegerDivide { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerDivideRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::WrappingIntegerRemainder { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerRemainderRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::SaturatingIntegerDivide { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::SaturatingIntegerDivideRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::SaturatingIntegerRemainder { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(
                            ModuleError::SaturatingIntegerRemainderRequiresIntegerResult(
                                operation.id,
                            ),
                        );
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::WrappingIntegerAdd { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerAdd { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::SaturatingIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerSubtract { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerSubtract { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::SaturatingIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerMultiply { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerMultiplyRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerMultiply { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::SaturatingIntegerMultiplyRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
            }
        }
        for edge in block.terminator.edges() {
            insert_unique(&mut registry.edges, edge, ModuleError::DuplicateEdge)?;
        }
        for cleanup in nominal_cleanups(&block.terminator) {
            for obligation in &cleanup.requirement_obligations {
                insert_unique(
                    &mut registry.obligations,
                    *obligation,
                    ModuleError::DuplicateObligation,
                )?;
            }
        }
    }

    let Some(entry) = blocks.get(&machine.entry) else {
        return Err(ModuleError::UnknownEntryBlock {
            machine: machine.id,
            block: machine.entry,
        });
    };
    if !entry.parameters.is_empty() {
        return Err(ModuleError::EntryBlockCannotHaveParameters(machine.entry));
    }

    let context = PropositionContext::from_value_types_and_places(
        value_types.iter().map(|(id, ty)| (*id, *ty)),
        machine
            .structural_places
            .iter()
            .map(|place| (place.id, place.kind))
            .chain(contract_receiver.map(|receiver| {
                (
                    receiver,
                    StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: true,
                    },
                )
            })),
    )
    .map_err(ModuleError::MalformedProposition)?;
    content::validate_content_entry_claims(machine, registry, &structural_place_kinds, &context)?;
    content::validate_content_identity_reshuffles(
        machine,
        registry,
        &structural_place_kinds,
        &context,
    )?;
    content::validate_content_partition_compositions(
        module,
        machine,
        machines,
        registry,
        &structural_place_kinds,
        &context,
    )?;
    let requires_values = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    validate_crash_frontiers(module, machine, &context, &requires_values)?;
    validate_partial_affine_cleanup_shape(module, machine, machines)?;
    validate_nominal_affine_cleanup_shape(module, machine, machines)?;
    let mut ensures_values = requires_values.clone();
    if let Some(result) = machine.result.scalar() {
        ensures_values.insert(result.id);
    }
    for proposition in &machine.contract.requires {
        contracts::validate_contract_clause_kind(
            proposition,
            machine.contract.id,
            ContractClauseKind::Requires,
        )?;
        context
            .validate(proposition)
            .map_err(ModuleError::MalformedProposition)?;
        contracts::validate_contract_scope(
            proposition,
            &requires_values,
            machine.contract.id,
            ContractClauseKind::Requires,
        )?;
        crash::validate_structural_case_memberships(module, machine, proposition)?;
    }
    for clause in &machine.contract.ensures {
        insert_unique(
            &mut registry.obligations,
            clause.obligation,
            ModuleError::DuplicateObligation,
        )?;
        contracts::validate_contract_clause_kind(
            &clause.proposition,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
        context
            .validate(&clause.proposition)
            .map_err(ModuleError::MalformedProposition)?;
        contracts::validate_contract_scope(
            &clause.proposition,
            &ensures_values,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
        crash::validate_structural_case_memberships(module, machine, &clause.proposition)?;
    }
    if machine
        .contract
        .ensures
        .windows(2)
        .any(|pair| pair[0].obligation >= pair[1].obligation)
    {
        return Err(ModuleError::NonCanonicalContractEnsures(
            machine.contract.id,
        ));
    }

    control_flow::validate_control_flow(
        machine,
        machines,
        &module.boundary_machines,
        &blocks,
        &value_types,
    )?;
    frontier::validate_structural_frontier(module, machine, machines, &blocks)
}
