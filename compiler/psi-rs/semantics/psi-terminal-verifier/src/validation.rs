use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    BlockId, ClaimId, ContentAlgebra, ContentConservation, ContentProjectionIdentity,
    ContentStructuralPlace, ContentTerm, ContractId, EdgeId, MachineId, ObligationId, OperationId,
    PlaceId, Proposition, PropositionContext, PropositionError, PropositionId, ScalarTerm,
    ScalarType, StructuralPlaceKind, ValueId,
};
use psi_terminal::{
    ContentPartitionComposition, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
    OperationKind, PropositionBinderArgumentKind, PropositionBinderKind, PropositionEvidence,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
};

use crate::verification::substitute_proposition_values;

#[derive(Debug, Clone, Copy)]
pub struct ValidatedTerminalModule<'module> {
    module: &'module TerminalModule,
}

impl<'module> ValidatedTerminalModule<'module> {
    pub const fn module(self) -> &'module TerminalModule {
        self.module
    }

    pub fn machine(self, id: MachineId) -> Option<&'module TerminalMachine> {
        self.module.machines.iter().find(|machine| machine.id == id)
    }

    pub fn value_context(
        self,
        machine: &TerminalMachine,
    ) -> Result<PropositionContext, ModuleError> {
        PropositionContext::from_value_types_and_places(
            machine_value_types(machine),
            machine
                .structural_places
                .iter()
                .map(|place| (place.id, place.kind)),
        )
        .map_err(ModuleError::MalformedProposition)
    }
}

pub fn validate_module(
    module: &TerminalModule,
) -> Result<ValidatedTerminalModule<'_>, ModuleError> {
    if module.machines.is_empty() {
        return Err(ModuleError::EmptyModule);
    }
    validate_proposition_vocabulary(module)?;

    let mut registry = IdRegistry::default();
    for machine in &module.machines {
        insert_unique(
            &mut registry.machines,
            machine.id,
            ModuleError::DuplicateMachine,
        )?;
        insert_unique(
            &mut registry.contracts,
            machine.contract.id,
            ModuleError::DuplicateContract,
        )?;
    }
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    for machine in &module.machines {
        validate_machine(machine, &machines, &mut registry)?;
    }
    validate_call_graph(module)?;
    if !registry.machines.contains(&module.entry) {
        return Err(ModuleError::UnknownEntryMachine(module.entry));
    }

    Ok(ValidatedTerminalModule { module })
}

fn validate_call_graph(module: &TerminalModule) -> Result<(), ModuleError> {
    let calls = module
        .machines
        .iter()
        .map(|machine| {
            let callees = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| match &operation.kind {
                    OperationKind::Call { callee, .. } => Some(*callee),
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            (machine.id, callees)
        })
        .collect::<BTreeMap<_, _>>();

    let mut indegree = calls
        .keys()
        .copied()
        .map(|machine| (machine, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for callees in calls.values() {
        for callee in callees {
            let count = indegree
                .get_mut(callee)
                .expect("validated call target is registered");
            *count += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(machine, count)| (*count == 0).then_some(*machine))
        .collect::<BTreeSet<_>>();
    let mut completed = 0_usize;
    while let Some(machine) = ready.pop_first() {
        completed += 1;
        for callee in &calls[&machine] {
            let count = indegree
                .get_mut(callee)
                .expect("validated call target has an indegree");
            *count -= 1;
            if *count == 0 {
                ready.insert(*callee);
            }
        }
    }
    if completed != calls.len() {
        let machine = indegree
            .into_iter()
            .find_map(|(machine, count)| (count != 0).then_some(machine))
            .expect("incomplete topological order has a cyclic remainder");
        return Err(ModuleError::RecursiveCallSliceNotYetSupported(machine));
    }
    Ok(())
}

fn validate_proposition_vocabulary(module: &TerminalModule) -> Result<(), ModuleError> {
    let mut declarations = BTreeMap::new();
    let mut declaration_names = BTreeSet::new();
    for (index, declaration) in module.proposition_declarations.iter().enumerate() {
        let expected = PropositionId::new(
            u64::try_from(index)
                .expect("proposition declaration count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        )
        .expect("one-based proposition identity is nonzero");
        if declaration.id != expected {
            return Err(ModuleError::NonDensePropositionDeclaration {
                expected,
                actual: declaration.id,
            });
        }
        if declarations.insert(declaration.id, declaration).is_some() {
            return Err(ModuleError::DuplicatePropositionDeclaration(declaration.id));
        }
        if declaration.name.is_empty() {
            return Err(ModuleError::EmptyPropositionIdentity);
        }
        if !declaration_names.insert(declaration.name.as_str()) {
            return Err(ModuleError::DuplicatePropositionName(
                declaration.name.clone(),
            ));
        }
        let mut binder_names = BTreeSet::new();
        for binder in &declaration.binders {
            if binder.name.is_empty() || !binder_names.insert(binder.name.as_str()) {
                return Err(ModuleError::InvalidPropositionBinder(declaration.id));
            }
            if matches!(
                &binder.kind,
                PropositionBinderKind::Const { type_identity } if type_identity.is_empty()
            ) {
                return Err(ModuleError::InvalidPropositionBinder(declaration.id));
            }
        }
        if declaration.parameter_types.iter().any(String::is_empty)
            || matches!(
                &declaration.evidence,
                PropositionEvidence::Witness { evidence_type } if evidence_type.is_empty()
            )
        {
            return Err(ModuleError::EmptyPropositionIdentity);
        }
    }

    let mut applications = BTreeSet::new();
    for (index, application) in module.proposition_applications.iter().enumerate() {
        let expected = PropositionId::new(
            u64::try_from(index)
                .expect("proposition application count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        )
        .expect("one-based proposition identity is nonzero");
        if application.id != expected {
            return Err(ModuleError::NonDensePropositionApplication {
                expected,
                actual: application.id,
            });
        }
        if !applications.insert(application.id) {
            return Err(ModuleError::DuplicatePropositionApplication(application.id));
        }
        let Some(declaration) = declarations.get(&application.declaration) else {
            return Err(ModuleError::UnknownPropositionDeclaration(
                application.declaration,
            ));
        };
        if application.binder_arguments.len() != declaration.binders.len()
            || application.arguments.len() != declaration.parameter_types.len()
        {
            return Err(ModuleError::PropositionApplicationArityMismatch(
                application.id,
            ));
        }
        for (argument, binder) in application
            .binder_arguments
            .iter()
            .zip(&declaration.binders)
        {
            let kind_matches = matches!(
                (&argument.kind, &binder.kind),
                (
                    PropositionBinderArgumentKind::Type,
                    PropositionBinderKind::Type
                ) | (
                    PropositionBinderArgumentKind::Const,
                    PropositionBinderKind::Const { .. }
                ) | (
                    PropositionBinderArgumentKind::Machine,
                    PropositionBinderKind::Machine
                )
            );
            if !kind_matches || argument.identity.is_empty() {
                return Err(ModuleError::PropositionApplicationBinderMismatch(
                    application.id,
                ));
            }
        }
        if application.arguments.iter().any(String::is_empty) {
            return Err(ModuleError::EmptyPropositionIdentity);
        }
    }
    Ok(())
}

#[derive(Default)]
struct IdRegistry {
    machines: BTreeSet<MachineId>,
    blocks: BTreeSet<BlockId>,
    contracts: BTreeSet<ContractId>,
    operations: BTreeSet<OperationId>,
    edges: BTreeSet<EdgeId>,
    obligations: BTreeSet<ObligationId>,
    values: BTreeSet<ValueId>,
    places: BTreeSet<PlaceId>,
    content_projection_algebras: BTreeMap<ContentProjectionIdentity, ContentAlgebra>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StructuralRootKey {
    Parameter(u32),
    Result,
}

fn validate_machine(
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    registry: &mut IdRegistry,
) -> Result<(), ModuleError> {
    if machine.blocks.is_empty() {
        return Err(ModuleError::MachineHasNoBlocks(machine.id));
    }

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
            insert_value(
                &mut value_types,
                &mut registry.values,
                operation.result.id,
                operation.result.scalar_type,
            )?;
            match operation.kind.clone() {
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
                            .any(proposition_contains_content)
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
                    if operation.result.scalar_type != callee_result.scalar_type {
                        return Err(ModuleError::CallResultTypeMismatch {
                            operation: operation.id,
                            expected: callee_result.scalar_type,
                            actual: operation.result.scalar_type,
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
                    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                        return Err(ModuleError::IntegerConstantRequiresIntegerResult(
                            operation.id,
                        ));
                    };
                    if !integer_type.admits(value) {
                        return Err(ModuleError::IntegerConstantOutsideResultType(operation.id));
                    }
                }
                OperationKind::BooleanConstant { .. } => {
                    if operation.result.scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanConstantRequiresBooleanResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::BooleanNot { .. } => {
                    if operation.result.scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanNotRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::BooleanEqual { .. } => {
                    if operation.result.scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanEqualRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::IntegerEqual { .. } => {
                    if operation.result.scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::IntegerEqualRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::IntegerLessThan { .. }
                | OperationKind::IntegerLessOrEqual { .. } => {
                    if operation.result.scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::IntegerOrderingRequiresBooleanResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerBitwiseAnd { .. }
                | OperationKind::IntegerBitwiseOr { .. }
                | OperationKind::IntegerBitwiseXor { .. } => {
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::IntegerBitwiseRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerBitwiseNot { .. } => {
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::IntegerBitwiseRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerWiden { .. } => {
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::IntegerWidenRequiresIntegerResult(operation.id));
                    }
                }
                OperationKind::IntegerExactCast { obligation, .. } => {
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::WrappingIntegerShiftRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::ExactIntegerShiftRight { obligation, .. } => {
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::WrappingIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerAdd { .. } => {
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::SaturatingIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerSubtract { .. } => {
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::WrappingIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerSubtract { .. } => {
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::SaturatingIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerMultiply { .. } => {
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::WrappingIntegerMultiplyRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerMultiply { .. } => {
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
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
            .map(|place| (place.id, place.kind)),
    )
    .map_err(ModuleError::MalformedProposition)?;
    validate_content_entry_claims(machine, registry, &structural_place_kinds, &context)?;
    validate_content_identity_reshuffles(machine, registry, &structural_place_kinds, &context)?;
    validate_content_partition_compositions(machine, registry, &structural_place_kinds, &context)?;
    let requires_values = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    validate_crash_frontiers(machine, &context, &requires_values)?;
    let mut ensures_values = requires_values.clone();
    if let Some(result) = machine.result.scalar() {
        ensures_values.insert(result.id);
    }
    for proposition in &machine.contract.requires {
        validate_contract_clause_kind(
            proposition,
            machine.contract.id,
            ContractClauseKind::Requires,
        )?;
        context
            .validate(proposition)
            .map_err(ModuleError::MalformedProposition)?;
        validate_contract_scope(
            proposition,
            &requires_values,
            machine.contract.id,
            ContractClauseKind::Requires,
        )?;
    }
    for clause in &machine.contract.ensures {
        insert_unique(
            &mut registry.obligations,
            clause.obligation,
            ModuleError::DuplicateObligation,
        )?;
        validate_contract_clause_kind(
            &clause.proposition,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
        context
            .validate(&clause.proposition)
            .map_err(ModuleError::MalformedProposition)?;
        validate_contract_scope(
            &clause.proposition,
            &ensures_values,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
    }

    validate_control_flow(machine, machines, &blocks, &value_types)
}

fn substitute_crash_routes(
    routes: &[CrashRouteBucket],
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .filter_map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => Some(CrashRouteGuard::Truth),
                    CrashRouteGuard::Predicate(predicate) => {
                        match substitute_proposition_values(predicate.proposition(), substitutions)
                        {
                            Proposition::Truth => Some(CrashRouteGuard::Truth),
                            Proposition::Falsehood => None,
                            proposition => Some(CrashRouteGuard::Predicate(
                                CrashPredicateTerm::new(proposition),
                            )),
                        }
                    }
                })
                .collect::<Vec<_>>();
            alternatives.sort();
            alternatives.dedup();
            if alternatives.contains(&CrashRouteGuard::Truth) {
                alternatives = vec![CrashRouteGuard::Truth];
            }
            (!alternatives.is_empty()).then_some(CrashRouteBucket {
                cause: bucket.cause,
                alternatives,
            })
        })
        .collect()
}

fn validate_crash_frontiers(
    machine: &TerminalMachine,
    context: &PropositionContext,
    contract_values: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if machine
        .contract
        .crash_routes
        .windows(2)
        .any(|pair| pair[0].cause >= pair[1].cause)
    {
        return Err(ModuleError::NonCanonicalCrashRoutes(machine.id));
    }
    for bucket in &machine.contract.crash_routes {
        if bucket.alternatives.is_empty() {
            return Err(ModuleError::EmptyCrashRouteBucket {
                machine: machine.id,
                cause: bucket.cause,
            });
        }
        if bucket
            .alternatives
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || (bucket.alternatives.contains(&CrashRouteGuard::Truth)
                && bucket.alternatives != [CrashRouteGuard::Truth])
        {
            return Err(ModuleError::NonCanonicalCrashRouteAlternatives {
                machine: machine.id,
                cause: bucket.cause,
            });
        }
        for guard in &bucket.alternatives {
            let CrashRouteGuard::Predicate(predicate) = guard else {
                continue;
            };
            if matches!(
                predicate.proposition(),
                Proposition::Truth | Proposition::Falsehood
            ) {
                return Err(ModuleError::NonCanonicalCrashRouteAlternatives {
                    machine: machine.id,
                    cause: bucket.cause,
                });
            }
            context
                .validate(predicate.proposition())
                .map_err(ModuleError::MalformedProposition)?;
            validate_contract_scope(
                predicate.proposition(),
                contract_values,
                machine.contract.id,
                ContractClauseKind::Crash,
            )?;
        }
    }
    let expected = machine
        .content_entry_claims
        .iter()
        .map(|binding| binding.claim)
        .collect::<Vec<_>>();
    for block in &machine.blocks {
        let Terminator::Crash {
            cause,
            site_guard,
            frontier_lower_bound,
            ..
        } = &block.terminator
        else {
            continue;
        };
        if site_guard.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ModuleError::NonCanonicalCrashSiteGuard(block.id));
        }
        for predicate in site_guard {
            if matches!(
                predicate.proposition(),
                Proposition::Truth | Proposition::Falsehood
            ) {
                return Err(ModuleError::NonCanonicalCrashSiteGuard(block.id));
            }
            context
                .validate(predicate.proposition())
                .map_err(ModuleError::MalformedProposition)?;
        }
        let covered = machine
            .contract
            .crash_routes
            .iter()
            .filter(|bucket| bucket.cause == *cause)
            .any(|bucket| {
                bucket.alternatives.iter().any(|route| match route {
                    CrashRouteGuard::Truth => true,
                    CrashRouteGuard::Predicate(predicate) => site_guard.contains(predicate),
                })
            });
        if !covered {
            return Err(ModuleError::CrashRouteUncovered {
                block: block.id,
                cause: *cause,
            });
        }
        if frontier_lower_bound
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalCrashFrontier(block.id));
        }
        // Terminal Psi has no claim-consuming operation yet, so every entry
        // claim is still live at every reachable crash. Requiring exact
        // equality now prevents a producer from laundering an omitted claim
        // as cleanup. Later claim-transfer operations refine the reconstructed
        // live set; the row remains the explicit local lower bound.
        if frontier_lower_bound != &expected {
            return Err(ModuleError::CrashFrontierMismatch { block: block.id });
        }
    }
    Ok(())
}

fn validate_content_entry_claims(
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    let mut inputs = BTreeSet::<ContentStructuralPlace>::new();
    for (index, binding) in machine.content_entry_claims.iter().enumerate() {
        let expected = ClaimId::new(
            u64::try_from(index)
                .expect("an in-memory claim count fits u64")
                .checked_add(1)
                .expect("an in-memory claim count cannot exhaust u64"),
        )
        .expect("dense claim identities begin at one");
        if binding.claim != expected {
            return Err(ModuleError::NonDenseContentEntryClaim {
                expected,
                actual: binding.claim,
            });
        }
        if binding.projections.is_empty() {
            return Err(ModuleError::ContentEntryClaimHasNoProjections(
                binding.claim,
            ));
        }
        if binding
            .projections
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentEntryProjectionOrder(
                binding.claim,
            ));
        }
        if binding.input.version != psi_core::ContentPlaceVersion::Entry
            || !matches!(
                structural_place_kinds.get(&binding.input.root),
                Some(StructuralPlaceKind::Parameter { .. })
            )
        {
            return Err(ModuleError::ContentEntryClaimRequiresEntryParameter(
                binding.claim,
            ));
        }
        if inputs.contains(&binding.input) {
            return Err(ModuleError::DuplicateContentEntryClaimInput(
                binding.input.clone(),
            ));
        }
        if let Some(previous) = inputs
            .iter()
            .find(|previous| content_places_overlap(previous, &binding.input))
        {
            return Err(ModuleError::OverlappingContentEntryClaimInput {
                first: previous.clone(),
                second: binding.input.clone(),
            });
        }
        inputs.insert(binding.input.clone());
        for content in &binding.projections {
            if let Some(previous) = registry
                .content_projection_algebras
                .insert(content.projection, content.algebra.clone())
                && previous != content.algebra
            {
                return Err(ModuleError::ContentProjectionAlgebraMismatch(
                    content.projection,
                ));
            }
            let term = ContentTerm::Projection {
                projection: content.projection,
                subject: binding.input.clone(),
            };
            context
                .validate(&Proposition::ContentConservation(ContentConservation::new(
                    content.algebra.clone(),
                    term.clone(),
                    term,
                )))
                .map_err(ModuleError::MalformedProposition)?;
        }
    }
    Ok(())
}

fn validate_content_identity_reshuffles(
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    let mut claims = BTreeSet::<ClaimId>::new();
    let mut inputs = BTreeSet::<ContentStructuralPlace>::new();
    let mut outputs = BTreeSet::<ContentStructuralPlace>::new();
    for reshuffle in &machine.content_identity_reshuffles {
        insert_unique(&mut claims, reshuffle.claim, ModuleError::DuplicateClaim)?;
        if reshuffle.projections.is_empty() {
            return Err(ModuleError::ContentIdentityReshuffleHasNoProjections(
                reshuffle.claim,
            ));
        }
        let Some(binding) = machine
            .content_entry_claims
            .iter()
            .find(|binding| binding.claim == reshuffle.claim)
        else {
            return Err(ModuleError::ContentIdentityClaimHasNoEntryBinding(
                reshuffle.claim,
            ));
        };
        if binding.input != reshuffle.input || binding.projections != reshuffle.projections {
            return Err(ModuleError::ContentIdentityEntryBindingMismatch(
                reshuffle.claim,
            ));
        }
        if reshuffle
            .projections
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentIdentityProjectionOrder(
                reshuffle.claim,
            ));
        }
        if reshuffle.input.version != psi_core::ContentPlaceVersion::Entry
            || !matches!(
                structural_place_kinds.get(&reshuffle.input.root),
                Some(StructuralPlaceKind::Parameter { .. })
            )
        {
            return Err(ModuleError::ContentIdentityReshuffleRequiresEntryParameter(
                reshuffle.claim,
            ));
        }
        if reshuffle.output.version != psi_core::ContentPlaceVersion::Current
            || !matches!(
                structural_place_kinds.get(&reshuffle.output.root),
                Some(StructuralPlaceKind::Result)
            )
        {
            return Err(ModuleError::ContentIdentityReshuffleRequiresCurrentResult(
                reshuffle.claim,
            ));
        }
        if inputs.contains(&reshuffle.input) {
            return Err(ModuleError::DuplicateContentIdentityInput(
                reshuffle.input.clone(),
            ));
        }
        if let Some(previous) = inputs
            .iter()
            .find(|previous| content_places_overlap(previous, &reshuffle.input))
        {
            return Err(ModuleError::OverlappingContentIdentityInput {
                first: previous.clone(),
                second: reshuffle.input.clone(),
            });
        }
        inputs.insert(reshuffle.input.clone());
        if outputs.contains(&reshuffle.output) {
            return Err(ModuleError::DuplicateContentIdentityOutput(
                reshuffle.output.clone(),
            ));
        }
        if let Some(previous) = outputs
            .iter()
            .find(|previous| content_places_overlap(previous, &reshuffle.output))
        {
            return Err(ModuleError::OverlappingContentIdentityOutput {
                first: previous.clone(),
                second: reshuffle.output.clone(),
            });
        }
        outputs.insert(reshuffle.output.clone());
        for (content, proposition) in reshuffle
            .projections
            .iter()
            .zip(reshuffle.inferred_propositions())
        {
            if let Some(previous) = registry
                .content_projection_algebras
                .insert(content.projection, content.algebra.clone())
                && previous != content.algebra
            {
                return Err(ModuleError::ContentProjectionAlgebraMismatch(
                    content.projection,
                ));
            }
            context
                .validate(&proposition)
                .map_err(ModuleError::MalformedProposition)?;
        }
    }
    Ok(())
}

fn validate_content_partition_compositions(
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    let mut rows = BTreeSet::<&ContentPartitionComposition>::new();
    for composition in &machine.content_partition_compositions {
        if !rows.insert(composition) {
            return Err(ModuleError::DuplicateContentPartitionComposition);
        }
        if composition.input_claims.is_empty() {
            return Err(ModuleError::ContentPartitionCompositionHasNoInputClaims);
        }
        if composition
            .input_claims
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentPartitionInputClaims);
        }
        if composition
            .substitutions
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentPartitionSubstitutions);
        }
        if composition.source.algebra() != composition.derived.algebra() {
            return Err(ModuleError::ContentPartitionAlgebraMismatch);
        }
        if !content_term_contains_partition(composition.source.left())
            && !content_term_contains_partition(composition.source.right())
        {
            return Err(ModuleError::ContentPartitionSourceHasNoSeparation);
        }

        let source_kinds = validate_partition_source_places(composition)?;
        let source_context = PropositionContext::from_value_types_and_places(
            [],
            composition
                .source_structural_places
                .iter()
                .map(|place| (place.id, place.kind)),
        )
        .map_err(ModuleError::MalformedProposition)?;
        source_context
            .validate(&Proposition::ContentConservation(
                composition.source.clone(),
            ))
            .map_err(ModuleError::MalformedProposition)?;
        context
            .validate(&composition.inferred_proposition())
            .map_err(ModuleError::MalformedProposition)?;
        register_partition_projections(registry, &composition.source)?;
        register_partition_projections(registry, &composition.derived)?;

        let substitutions = composition
            .substitutions
            .iter()
            .map(|substitution| (substitution.source.clone(), substitution.target.clone()))
            .collect::<BTreeMap<_, _>>();
        if substitutions.len() != composition.substitutions.len() {
            return Err(ModuleError::NonCanonicalContentPartitionSubstitutions);
        }
        let target_count = composition
            .substitutions
            .iter()
            .map(|substitution| &substitution.target)
            .collect::<BTreeSet<_>>()
            .len();
        if target_count != composition.substitutions.len() {
            return Err(ModuleError::DuplicateContentPartitionSubstitutionTarget);
        }
        let source_subjects = content_conservation_subjects(&composition.source);
        if source_subjects
            != substitutions
                .keys()
                .cloned()
                .collect::<BTreeSet<ContentStructuralPlace>>()
        {
            return Err(ModuleError::ContentPartitionSubstitutionCoverageMismatch);
        }
        for substitution in &composition.substitutions {
            validate_partition_substitution_shape(
                substitution,
                &source_kinds,
                structural_place_kinds,
            )?;
        }
        let replayed = replay_partition_conservation(&composition.source, &substitutions)?;
        if replayed != composition.derived {
            return Err(ModuleError::ContentPartitionReplayMismatch);
        }

        let listed_claims = composition
            .input_claims
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut used_claims = BTreeSet::new();
        for (projection, subject) in content_conservation_projections(&composition.derived) {
            if subject.version != psi_core::ContentPlaceVersion::Entry {
                continue;
            }
            let matching = machine
                .content_entry_claims
                .iter()
                .filter(|binding| {
                    binding.input == subject
                        && binding.projections.iter().any(|content| {
                            content.projection == projection
                                && content.algebra == *composition.derived.algebra()
                        })
                })
                .map(|binding| binding.claim)
                .collect::<Vec<_>>();
            let [claim] = matching.as_slice() else {
                return Err(ModuleError::ContentPartitionInputProjectionNotClaimBound(
                    subject,
                ));
            };
            if !listed_claims.contains(claim) {
                return Err(ModuleError::ContentPartitionInputClaimNotListed(*claim));
            }
            used_claims.insert(*claim);
        }
        if used_claims != listed_claims {
            return Err(ModuleError::ContentPartitionInputClaimUnused);
        }
    }
    Ok(())
}

fn validate_partition_source_places(
    composition: &ContentPartitionComposition,
) -> Result<BTreeMap<PlaceId, StructuralPlaceKind>, ModuleError> {
    let mut ids = BTreeMap::new();
    let mut roots = BTreeSet::new();
    for place in &composition.source_structural_places {
        if ids.insert(place.id, place.kind).is_some() {
            return Err(ModuleError::DuplicateContentPartitionSourcePlace(place.id));
        }
        let root = match place.kind {
            StructuralPlaceKind::Parameter { position, .. } => {
                StructuralRootKey::Parameter(position)
            }
            StructuralPlaceKind::Result => StructuralRootKey::Result,
        };
        if !roots.insert(root) {
            return Err(ModuleError::DuplicateContentPartitionSourceRoot(place.kind));
        }
    }
    Ok(ids)
}

fn validate_partition_substitution_shape(
    substitution: &psi_terminal::ContentPlaceSubstitution,
    source_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    target_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
) -> Result<(), ModuleError> {
    match (
        substitution.source.version,
        source_kinds.get(&substitution.source.root),
        substitution.target.version,
        target_kinds.get(&substitution.target.root),
    ) {
        (
            psi_core::ContentPlaceVersion::Entry,
            Some(StructuralPlaceKind::Parameter { .. }),
            psi_core::ContentPlaceVersion::Entry,
            Some(StructuralPlaceKind::Parameter { .. }),
        )
        | (
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::Result),
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::Result),
        ) => Ok(()),
        _ => Err(ModuleError::InvalidContentPartitionSubstitutionShape),
    }
}

fn replay_partition_conservation(
    source: &ContentConservation,
    substitutions: &BTreeMap<ContentStructuralPlace, ContentStructuralPlace>,
) -> Result<ContentConservation, ModuleError> {
    Ok(ContentConservation::new(
        source.algebra().clone(),
        replay_partition_term(source.left(), substitutions)?,
        replay_partition_term(source.right(), substitutions)?,
    ))
}

fn replay_partition_term(
    term: &ContentTerm,
    substitutions: &BTreeMap<ContentStructuralPlace, ContentStructuralPlace>,
) -> Result<ContentTerm, ModuleError> {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => Ok(ContentTerm::Projection {
            projection: *projection,
            subject: substitutions
                .get(subject)
                .cloned()
                .ok_or(ModuleError::ContentPartitionSubstitutionCoverageMismatch)?,
        }),
        ContentTerm::Separate(terms) => ContentTerm::separate(
            terms
                .iter()
                .map(|term| replay_partition_term(term, substitutions))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(ModuleError::MalformedProposition),
    }
}

fn content_term_contains_partition(term: &ContentTerm) -> bool {
    match term {
        ContentTerm::Projection { .. } => false,
        ContentTerm::Separate(_) => true,
    }
}

fn content_conservation_subjects(
    conservation: &ContentConservation,
) -> BTreeSet<ContentStructuralPlace> {
    content_conservation_projections(conservation)
        .into_iter()
        .map(|(_, subject)| subject)
        .collect()
}

fn content_conservation_projections(
    conservation: &ContentConservation,
) -> Vec<(ContentProjectionIdentity, ContentStructuralPlace)> {
    fn collect(
        term: &ContentTerm,
        projections: &mut Vec<(ContentProjectionIdentity, ContentStructuralPlace)>,
    ) {
        match term {
            ContentTerm::Projection {
                projection,
                subject,
            } => projections.push((*projection, subject.clone())),
            ContentTerm::Separate(terms) => {
                for term in terms {
                    collect(term, projections);
                }
            }
        }
    }
    let mut projections = Vec::new();
    collect(conservation.left(), &mut projections);
    collect(conservation.right(), &mut projections);
    projections
}

fn register_partition_projections(
    registry: &mut IdRegistry,
    conservation: &ContentConservation,
) -> Result<(), ModuleError> {
    for (projection, _) in content_conservation_projections(conservation) {
        if let Some(previous) = registry
            .content_projection_algebras
            .insert(projection, conservation.algebra().clone())
            && previous != *conservation.algebra()
        {
            return Err(ModuleError::ContentProjectionAlgebraMismatch(projection));
        }
    }
    Ok(())
}

fn content_places_overlap(left: &ContentStructuralPlace, right: &ContentStructuralPlace) -> bool {
    if left.version != right.version || left.root != right.root {
        return false;
    }
    let shared = left.segments.len().min(right.segments.len());
    left.segments[..shared] == right.segments[..shared]
}

fn validate_contract_clause_kind(
    proposition: &Proposition,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match proposition {
        Proposition::Conjunction(conjuncts) => {
            for conjunct in conjuncts {
                validate_contract_clause_kind(conjunct, contract, clause)?;
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_contract_clause_kind(premise, contract, clause)?;
            validate_contract_clause_kind(conclusion, contract, clause)
        }
        Proposition::ContentConservation(_) if clause == ContractClauseKind::Requires => {
            Err(ModuleError::ContentConservationRequiresEnsures { contract })
        }
        _ => Ok(()),
    }
}

fn validate_contract_scope(
    proposition: &Proposition,
    allowed: &BTreeSet<ValueId>,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match proposition {
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => Ok(()),
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            validate_term_scope(left, allowed, contract, clause)?;
            validate_term_scope(right, allowed, contract, clause)
        }
        Proposition::Conjunction(conjuncts) => {
            for conjunct in conjuncts {
                validate_contract_scope(conjunct, allowed, contract, clause)?;
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_contract_scope(premise, allowed, contract, clause)?;
            validate_contract_scope(conclusion, allowed, contract, clause)
        }
        Proposition::ContentConservation(_) => Ok(()),
    }
}

fn validate_term_scope(
    term: &ScalarTerm,
    allowed: &BTreeSet<ValueId>,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match term {
        ScalarTerm::Value { id, .. } => {
            if !allowed.contains(id) {
                return Err(ModuleError::ContractValueOutsideScope {
                    contract,
                    clause,
                    value: *id,
                });
            }
        }
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerSubtract { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. }
        | ScalarTerm::WrappingIntegerDivide { left, right, .. }
        | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
        | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
        | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. }
        | ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            validate_term_scope(left, allowed, contract, clause)?;
            validate_term_scope(right, allowed, contract, clause)?;
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            validate_term_scope(value, allowed, contract, clause)?;
            validate_term_scope(count, allowed, contract, clause)?;
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => {
            validate_term_scope(operand, allowed, contract, clause)?;
        }
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
    }
    Ok(())
}

fn validate_control_flow(
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<(), ModuleError> {
    let globally_defined = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    let mut definition_blocks = BTreeMap::new();
    for block in blocks.values() {
        for parameter in &block.parameters {
            definition_blocks.insert(parameter.id, block.id);
        }
        for operation in &block.operations {
            definition_blocks.insert(operation.result.id, block.id);
        }
    }

    let mut successors = BTreeMap::<BlockId, Vec<BlockId>>::new();
    let mut predecessors = blocks
        .keys()
        .map(|block| (*block, Vec::<BlockId>::new()))
        .collect::<BTreeMap<_, _>>();
    for block in blocks.values() {
        let targets = match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        for target in &targets {
            if !blocks.contains_key(target) {
                return Err(ModuleError::UnknownTargetBlock(*target));
            }
            predecessors
                .get_mut(target)
                .expect("known target has a predecessor row")
                .push(block.id);
        }
        successors.insert(block.id, targets);
    }

    let mut reachable = BTreeSet::new();
    let mut pending = vec![machine.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(
                successors
                    .get(&block)
                    .expect("every block has successors")
                    .iter()
                    .copied(),
            );
        }
    }
    if reachable.len() != blocks.len() {
        let block = blocks
            .keys()
            .find(|block| !reachable.contains(block))
            .copied()
            .expect("different set lengths guarantee an unreachable block");
        return Err(ModuleError::UnreachableBlock(block));
    }

    let mut indegree = predecessors
        .iter()
        .map(|(block, incoming)| (*block, incoming.len()))
        .collect::<BTreeMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(blocks.len());
    while let Some(block) = ready.pop_first() {
        order.push(block);
        for target in successors.get(&block).expect("every block has successors") {
            let count = indegree
                .get_mut(target)
                .expect("known target has an indegree");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }
    if order.len() != blocks.len() {
        let block = indegree
            .iter()
            .find_map(|(block, count)| (*count != 0).then_some(*block))
            .expect("a cyclic graph leaves positive indegree");
        return Err(ModuleError::ControlCycle(block));
    }

    let mut dominators = BTreeMap::<BlockId, BTreeSet<BlockId>>::new();
    for block in &order {
        let incoming = predecessors
            .get(block)
            .expect("every block has predecessors");
        let mut set = if *block == machine.entry {
            BTreeSet::new()
        } else {
            let mut incoming = incoming.iter();
            let first = incoming
                .next()
                .expect("reachable non-entry block has a predecessor");
            let mut intersection = dominators
                .get(first)
                .expect("topological predecessor has dominators")
                .clone();
            for predecessor in incoming {
                intersection = intersection
                    .intersection(
                        dominators
                            .get(predecessor)
                            .expect("topological predecessor has dominators"),
                    )
                    .copied()
                    .collect();
            }
            intersection
        };
        set.insert(*block);
        dominators.insert(*block, set);
    }

    for block_id in order {
        let block = blocks
            .get(&block_id)
            .copied()
            .expect("topological order contains known blocks");
        let block_dominators = dominators
            .get(&block_id)
            .expect("every ordered block has dominators");
        let mut defined = globally_defined.clone();
        defined.extend(block.parameters.iter().map(|parameter| parameter.id));
        defined.extend(definition_blocks.iter().filter_map(|(value, definition)| {
            (*definition != block_id && block_dominators.contains(definition)).then_some(*value)
        }));
        for operation in &block.operations {
            validate_operation_operands(operation, machines, value_types, &defined)?;
            defined.insert(operation.result.id);
        }
        match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                arguments,
            } => validate_successor_bindings(
                *edge,
                *target,
                arguments,
                blocks,
                value_types,
                &defined,
            )?,
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                require_defined(*condition, value_types, &defined)?;
                let actual = value_types[condition];
                if actual != ScalarType::Boolean {
                    return Err(ModuleError::ConditionalConditionTypeMismatch {
                        block: block.id,
                        condition: *condition,
                        actual,
                    });
                }
                for successor in [when_true, when_false] {
                    validate_successor_bindings(
                        successor.edge,
                        successor.target,
                        &successor.arguments,
                        blocks,
                        value_types,
                        &defined,
                    )?;
                }
            }
            Terminator::Return { value, .. } => {
                let Some(result) = machine.result.scalar() else {
                    return Err(ModuleError::ScalarReturnFromUnitMachine {
                        machine: machine.id,
                        block: block.id,
                    });
                };
                require_defined(*value, value_types, &defined)?;
                let value_type = value_types[value];
                if value_type != result.scalar_type {
                    return Err(ModuleError::ReturnTypeMismatch {
                        machine: machine.id,
                        value: value_type,
                        result: result.scalar_type,
                    });
                }
            }
            Terminator::ReturnUnit { .. } => {
                if matches!(machine.result, TerminalMachineResult::Scalar(_)) {
                    return Err(ModuleError::UnitReturnFromScalarMachine {
                        machine: machine.id,
                        block: block.id,
                    });
                }
            }
            Terminator::Crash { site_guard, .. } => {
                for predicate in site_guard {
                    validate_contract_scope(
                        predicate.proposition(),
                        &defined,
                        machine.contract.id,
                        ContractClauseKind::Crash,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_operation_operands(
    operation: &psi_terminal::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if let OperationKind::Call {
        callee, arguments, ..
    } = &operation.kind
    {
        let callee = machines
            .get(callee)
            .copied()
            .expect("call target was validated during operation registration");
        if arguments.len() != callee.parameters.len() {
            return Err(ModuleError::CallArgumentArityMismatch {
                operation: operation.id,
                expected: callee.parameters.len(),
                actual: arguments.len(),
            });
        }
        for (argument, parameter) in arguments.iter().zip(&callee.parameters) {
            require_defined(*argument, value_types, defined)?;
            let actual = value_types[argument];
            if actual != parameter.scalar_type {
                return Err(ModuleError::CallArgumentTypeMismatch {
                    operation: operation.id,
                    argument: *argument,
                    expected: parameter.scalar_type,
                    actual,
                });
            }
        }
        return Ok(());
    }
    if let OperationKind::IntegerExactCast { operand, .. } = operation.kind.clone() {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        let expected = operation.result.scalar_type;
        let (ScalarType::Integer(source), ScalarType::Integer(target)) = (actual, expected) else {
            return Err(ModuleError::IntegerExactCastOperandTypeMismatch {
                operation: operation.id,
                source: actual,
                target: expected,
            });
        };
        if !source.can_exact_cast_to(target) || source.can_widen_to(target) || source == target {
            return Err(ModuleError::IntegerExactCastOperandTypeMismatch {
                operation: operation.id,
                source: actual,
                target: expected,
            });
        }
        return Ok(());
    }
    if let OperationKind::IntegerWiden { operand } = operation.kind.clone() {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        let expected = operation.result.scalar_type;
        let (ScalarType::Integer(source), ScalarType::Integer(target)) = (actual, expected) else {
            return Err(ModuleError::IntegerWidenOperandTypeMismatch {
                operation: operation.id,
                source: actual,
                target: expected,
            });
        };
        if !source.can_widen_to(target) {
            return Err(ModuleError::IntegerWidenOperandTypeMismatch {
                operation: operation.id,
                source: actual,
                target: expected,
            });
        }
        return Ok(());
    }
    if let OperationKind::IntegerBitwiseNot { operand } = operation.kind.clone() {
        require_defined(operand, value_types, defined)?;
        let expected = operation.result.scalar_type;
        let actual = value_types[&operand];
        if !matches!(expected, ScalarType::Integer(_)) || actual != expected {
            return Err(ModuleError::IntegerBitwiseNotOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual,
            });
        }
        return Ok(());
    }
    if let OperationKind::BooleanNot { operand } = operation.kind.clone() {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        if actual != ScalarType::Boolean {
            return Err(ModuleError::BooleanNotOperandTypeMismatch {
                operation: operation.id,
                operand,
                actual,
            });
        }
        return Ok(());
    }
    if let OperationKind::BooleanEqual { left, right } = operation.kind.clone() {
        for operand in [left, right] {
            require_defined(operand, value_types, defined)?;
            let actual = value_types[&operand];
            if actual != ScalarType::Boolean {
                return Err(ModuleError::BooleanEqualOperandTypeMismatch {
                    operation: operation.id,
                    operand,
                    actual,
                });
            }
        }
        return Ok(());
    }
    if let OperationKind::IntegerEqual { left, right } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let left_type = value_types[&left];
        let right_type = value_types[&right];
        if !matches!(left_type, ScalarType::Integer(_)) || right_type != left_type {
            return Err(ModuleError::IntegerEqualOperandTypeMismatch {
                operation: operation.id,
                left: left_type,
                right: right_type,
            });
        }
        return Ok(());
    }
    if let OperationKind::IntegerLessThan { left, right }
    | OperationKind::IntegerLessOrEqual { left, right } = operation.kind.clone()
    {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let left_type = value_types[&left];
        let right_type = value_types[&right];
        if !matches!(left_type, ScalarType::Integer(_)) || right_type != left_type {
            return Err(ModuleError::IntegerOrderingOperandTypeMismatch {
                operation: operation.id,
                left: left_type,
                right: right_type,
            });
        }
        return Ok(());
    }
    if let OperationKind::IntegerBitwiseAnd { left, right }
    | OperationKind::IntegerBitwiseOr { left, right }
    | OperationKind::IntegerBitwiseXor { left, right } = operation.kind.clone()
    {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.scalar_type;
        let left_type = value_types[&left];
        let right_type = value_types[&right];
        if !matches!(expected, ScalarType::Integer(_))
            || left_type != expected
            || right_type != expected
        {
            return Err(ModuleError::IntegerBitwiseOperandTypeMismatch {
                operation: operation.id,
                expected,
                left: left_type,
                right: right_type,
            });
        }
        return Ok(());
    }
    if let OperationKind::WrappingIntegerShiftLeft { value, count }
    | OperationKind::WrappingIntegerShiftRight { value, count } = operation.kind.clone()
    {
        require_defined(value, value_types, defined)?;
        require_defined(count, value_types, defined)?;
        let expected_value = operation.result.scalar_type;
        let actual_value = value_types[&value];
        let actual_count = value_types[&count];
        if !matches!(expected_value, ScalarType::Integer(_))
            || actual_value != expected_value
            || !matches!(actual_count, ScalarType::Integer(_))
        {
            return Err(ModuleError::WrappingIntegerShiftOperandTypeMismatch {
                operation: operation.id,
                expected_value,
                actual_value,
                actual_count,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerShiftLeft { value, count, .. }
    | OperationKind::ExactIntegerShiftRight { value, count, .. } = operation.kind.clone()
    {
        require_defined(value, value_types, defined)?;
        require_defined(count, value_types, defined)?;
        let expected_value = operation.result.scalar_type;
        let actual_value = value_types[&value];
        let actual_count = value_types[&count];
        if !matches!(expected_value, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_value != expected_value
            || !matches!(actual_count, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
        {
            return Err(ModuleError::ExactIntegerShiftOperandTypeMismatch {
                operation: operation.id,
                expected_value,
                actual_value,
                actual_count,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerAdd { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::ExactIntegerAddOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerSubtract { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::ExactIntegerSubtractOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerMultiply { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::ExactIntegerMultiplyOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerDivide { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::ExactIntegerDivideOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerRemainder { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::ExactIntegerRemainderOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::WrappingIntegerDivide { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::WrappingIntegerDivideOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::WrappingIntegerRemainder { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::WrappingIntegerRemainderOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::SaturatingIntegerDivide { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::SaturatingIntegerDivideOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::SaturatingIntegerRemainder { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::SaturatingIntegerRemainderOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    let Some((left, right, arithmetic)) = (match operation.kind.clone() {
        OperationKind::WrappingIntegerAdd { left, right } => {
            Some((left, right, ArithmeticOperandKind::WrappingAdd))
        }
        OperationKind::SaturatingIntegerAdd { left, right } => {
            Some((left, right, ArithmeticOperandKind::SaturatingAdd))
        }
        OperationKind::WrappingIntegerSubtract { left, right } => {
            Some((left, right, ArithmeticOperandKind::WrappingSubtract))
        }
        OperationKind::SaturatingIntegerSubtract { left, right } => {
            Some((left, right, ArithmeticOperandKind::SaturatingSubtract))
        }
        OperationKind::WrappingIntegerMultiply { left, right } => {
            Some((left, right, ArithmeticOperandKind::WrappingMultiply))
        }
        OperationKind::SaturatingIntegerMultiply { left, right } => {
            Some((left, right, ArithmeticOperandKind::SaturatingMultiply))
        }
        OperationKind::IntegerConstant { .. }
        | OperationKind::BooleanConstant { .. }
        | OperationKind::BooleanNot { .. }
        | OperationKind::BooleanEqual { .. }
        | OperationKind::IntegerEqual { .. }
        | OperationKind::IntegerLessThan { .. }
        | OperationKind::IntegerLessOrEqual { .. }
        | OperationKind::IntegerBitwiseNot { .. }
        | OperationKind::IntegerWiden { .. }
        | OperationKind::IntegerExactCast { .. }
        | OperationKind::IntegerBitwiseAnd { .. }
        | OperationKind::IntegerBitwiseOr { .. }
        | OperationKind::IntegerBitwiseXor { .. }
        | OperationKind::WrappingIntegerShiftLeft { .. }
        | OperationKind::WrappingIntegerShiftRight { .. }
        | OperationKind::ExactIntegerShiftLeft { .. }
        | OperationKind::ExactIntegerShiftRight { .. }
        | OperationKind::ExactIntegerAdd { .. }
        | OperationKind::ExactIntegerSubtract { .. }
        | OperationKind::ExactIntegerMultiply { .. } => None,
        OperationKind::ExactIntegerDivide { .. } => None,
        OperationKind::ExactIntegerRemainder { .. } => None,
        OperationKind::WrappingIntegerDivide { .. } => None,
        OperationKind::WrappingIntegerRemainder { .. } => None,
        OperationKind::SaturatingIntegerDivide { .. } => None,
        OperationKind::SaturatingIntegerRemainder { .. } => None,
        OperationKind::Call { .. } => None,
    }) else {
        return Ok(());
    };
    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
        unreachable!("operation shape validation requires an integer result")
    };
    for operand in [left, right] {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        let expected = ScalarType::Integer(integer_type);
        if actual != expected {
            return Err(match arithmetic {
                ArithmeticOperandKind::SaturatingAdd => {
                    ModuleError::SaturatingIntegerAddOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::WrappingAdd => {
                    ModuleError::WrappingIntegerAddOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::WrappingSubtract => {
                    ModuleError::WrappingIntegerSubtractOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::SaturatingSubtract => {
                    ModuleError::SaturatingIntegerSubtractOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::WrappingMultiply => {
                    ModuleError::WrappingIntegerMultiplyOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::SaturatingMultiply => {
                    ModuleError::SaturatingIntegerMultiplyOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
            });
        }
    }
    Ok(())
}

fn proposition_contains_content(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::ContentConservation(_) => true,
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_contains_content),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_contains_content(premise) || proposition_contains_content(conclusion),
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::Equal(_, _)
        | Proposition::LessThan(_, _)
        | Proposition::LessOrEqual(_, _) => false,
    }
}

fn require_defined(
    value: ValueId,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if !defined.contains(&value) {
        return Err(ModuleError::ValueUsedBeforeDefinition(value));
    }
    if !value_types.contains_key(&value) {
        return Err(ModuleError::UnknownValue(value));
    }
    Ok(())
}

fn validate_successor_bindings(
    edge: EdgeId,
    target: BlockId,
    arguments: &[ValueId],
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let target_block = blocks
        .get(&target)
        .copied()
        .ok_or(ModuleError::UnknownTargetBlock(target))?;
    if target_block.parameters.len() != arguments.len() {
        return Err(ModuleError::JumpArityMismatch {
            edge,
            expected: target_block.parameters.len(),
            actual: arguments.len(),
        });
    }
    for (argument, parameter) in arguments.iter().zip(&target_block.parameters) {
        require_defined(*argument, value_types, defined)?;
        let argument_type = value_types[argument];
        if argument_type != parameter.scalar_type {
            return Err(ModuleError::JumpTypeMismatch {
                edge,
                argument: argument_type,
                parameter: parameter.scalar_type,
            });
        }
    }
    Ok(())
}

enum ArithmeticOperandKind {
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
}

pub(crate) fn machine_value_types(
    machine: &TerminalMachine,
) -> impl Iterator<Item = (ValueId, ScalarType)> + '_ {
    machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.operations.iter().map(|operation| &operation.result)),
        )
        .map(|declaration| (declaration.id, declaration.scalar_type))
}

fn insert_value(
    values: &mut BTreeMap<ValueId, ScalarType>,
    module_values: &mut BTreeSet<ValueId>,
    id: ValueId,
    scalar_type: ScalarType,
) -> Result<(), ModuleError> {
    if values.insert(id, scalar_type).is_some() || !module_values.insert(id) {
        return Err(ModuleError::DuplicateValue(id));
    }
    Ok(())
}

fn insert_unique<T: Ord + Copy>(
    set: &mut BTreeSet<T>,
    value: T,
    error: impl FnOnce(T) -> ModuleError,
) -> Result<(), ModuleError> {
    if !set.insert(value) {
        return Err(error(value));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractClauseKind {
    Requires,
    Ensures,
    Crash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleError {
    EmptyModule,
    DuplicatePropositionDeclaration(PropositionId),
    DuplicatePropositionApplication(PropositionId),
    NonDensePropositionDeclaration {
        expected: PropositionId,
        actual: PropositionId,
    },
    NonDensePropositionApplication {
        expected: PropositionId,
        actual: PropositionId,
    },
    DuplicatePropositionName(String),
    UnknownPropositionDeclaration(PropositionId),
    InvalidPropositionBinder(PropositionId),
    PropositionApplicationArityMismatch(PropositionId),
    PropositionApplicationBinderMismatch(PropositionId),
    EmptyPropositionIdentity,
    DuplicateMachine(MachineId),
    DuplicateBlock(BlockId),
    DuplicateContract(ContractId),
    DuplicateOperation(OperationId),
    DuplicateEdge(EdgeId),
    DuplicateObligation(ObligationId),
    DuplicateValue(ValueId),
    DuplicatePlace(PlaceId),
    DuplicateClaim(ClaimId),
    DuplicateStructuralPlaceRoot {
        machine: MachineId,
        kind: psi_core::StructuralPlaceKind,
    },
    UnitMachineHasResultStructuralPlace {
        machine: MachineId,
        place: PlaceId,
    },
    UnknownEntryMachine(MachineId),
    MachineHasNoBlocks(MachineId),
    UnknownEntryBlock {
        machine: MachineId,
        block: BlockId,
    },
    EntryBlockCannotHaveParameters(BlockId),
    ContractValueOutsideScope {
        contract: ContractId,
        clause: ContractClauseKind,
        value: ValueId,
    },
    NonCanonicalCrashRoutes(MachineId),
    EmptyCrashRouteBucket {
        machine: MachineId,
        cause: CrashCause,
    },
    NonCanonicalCrashRouteAlternatives {
        machine: MachineId,
        cause: CrashCause,
    },
    NonCanonicalCrashSiteGuard(BlockId),
    CrashRouteUncovered {
        block: BlockId,
        cause: CrashCause,
    },
    NonCanonicalCrashFrontier(BlockId),
    CrashFrontierMismatch {
        block: BlockId,
    },
    NonDenseContentEntryClaim {
        expected: ClaimId,
        actual: ClaimId,
    },
    ContentEntryClaimHasNoProjections(ClaimId),
    NonCanonicalContentEntryProjectionOrder(ClaimId),
    ContentEntryClaimRequiresEntryParameter(ClaimId),
    DuplicateContentEntryClaimInput(ContentStructuralPlace),
    OverlappingContentEntryClaimInput {
        first: ContentStructuralPlace,
        second: ContentStructuralPlace,
    },
    ContentIdentityReshuffleHasNoProjections(ClaimId),
    ContentIdentityClaimHasNoEntryBinding(ClaimId),
    ContentIdentityEntryBindingMismatch(ClaimId),
    NonCanonicalContentIdentityProjectionOrder(ClaimId),
    ContentIdentityReshuffleRequiresEntryParameter(ClaimId),
    ContentIdentityReshuffleRequiresCurrentResult(ClaimId),
    DuplicateContentIdentityInput(ContentStructuralPlace),
    DuplicateContentIdentityOutput(ContentStructuralPlace),
    OverlappingContentIdentityInput {
        first: ContentStructuralPlace,
        second: ContentStructuralPlace,
    },
    OverlappingContentIdentityOutput {
        first: ContentStructuralPlace,
        second: ContentStructuralPlace,
    },
    ContentProjectionAlgebraMismatch(ContentProjectionIdentity),
    DuplicateContentPartitionComposition,
    ContentPartitionCompositionHasNoInputClaims,
    NonCanonicalContentPartitionInputClaims,
    NonCanonicalContentPartitionSubstitutions,
    DuplicateContentPartitionSubstitutionTarget,
    ContentPartitionAlgebraMismatch,
    ContentPartitionSourceHasNoSeparation,
    DuplicateContentPartitionSourcePlace(PlaceId),
    DuplicateContentPartitionSourceRoot(StructuralPlaceKind),
    InvalidContentPartitionSubstitutionShape,
    ContentPartitionSubstitutionCoverageMismatch,
    ContentPartitionReplayMismatch,
    ContentPartitionInputProjectionNotClaimBound(ContentStructuralPlace),
    ContentPartitionInputClaimNotListed(ClaimId),
    ContentPartitionInputClaimUnused,
    ContentConservationRequiresEnsures {
        contract: ContractId,
    },
    UnknownTargetBlock(BlockId),
    UnknownValue(ValueId),
    ValueUsedBeforeDefinition(ValueId),
    UnknownCallTarget {
        operation: OperationId,
        callee: MachineId,
    },
    NonCanonicalCallCrashContinuations(OperationId),
    CallCrashContinuationsMismatch {
        operation: OperationId,
        callee: MachineId,
    },
    CallCrashContinuationUncovered {
        operation: OperationId,
        cause: CrashCause,
    },
    CallTargetHasStructuralContract {
        operation: OperationId,
        callee: MachineId,
    },
    CallTargetReturnsUnit {
        operation: OperationId,
        callee: MachineId,
    },
    CallResultTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual: ScalarType,
    },
    CallArgumentArityMismatch {
        operation: OperationId,
        expected: usize,
        actual: usize,
    },
    CallArgumentTypeMismatch {
        operation: OperationId,
        argument: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    CallRequirementArityMismatch {
        operation: OperationId,
        expected: usize,
        actual: usize,
    },
    RecursiveCallSliceNotYetSupported(MachineId),
    IntegerConstantRequiresIntegerResult(OperationId),
    IntegerConstantOutsideResultType(OperationId),
    BooleanConstantRequiresBooleanResult(OperationId),
    BooleanNotRequiresBooleanResult(OperationId),
    BooleanNotOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        actual: ScalarType,
    },
    BooleanEqualRequiresBooleanResult(OperationId),
    BooleanEqualOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        actual: ScalarType,
    },
    IntegerEqualRequiresBooleanResult(OperationId),
    IntegerEqualOperandTypeMismatch {
        operation: OperationId,
        left: ScalarType,
        right: ScalarType,
    },
    IntegerOrderingRequiresBooleanResult(OperationId),
    IntegerOrderingOperandTypeMismatch {
        operation: OperationId,
        left: ScalarType,
        right: ScalarType,
    },
    IntegerBitwiseRequiresIntegerResult(OperationId),
    IntegerWidenRequiresIntegerResult(OperationId),
    IntegerWidenOperandTypeMismatch {
        operation: OperationId,
        source: ScalarType,
        target: ScalarType,
    },
    IntegerExactCastRequiresIntegerResult(OperationId),
    IntegerExactCastOperandTypeMismatch {
        operation: OperationId,
        source: ScalarType,
        target: ScalarType,
    },
    IntegerBitwiseNotOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual: ScalarType,
    },
    IntegerBitwiseOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    WrappingIntegerShiftRequiresIntegerResult(OperationId),
    WrappingIntegerShiftOperandTypeMismatch {
        operation: OperationId,
        expected_value: ScalarType,
        actual_value: ScalarType,
        actual_count: ScalarType,
    },
    ExactIntegerShiftRequiresIntegerResult(OperationId),
    ExactIntegerShiftOperandTypeMismatch {
        operation: OperationId,
        expected_value: ScalarType,
        actual_value: ScalarType,
        actual_count: ScalarType,
    },
    ExactIntegerAddRequiresIntegerResult(OperationId),
    ExactIntegerAddOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerSubtractRequiresIntegerResult(OperationId),
    ExactIntegerSubtractOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerMultiplyRequiresIntegerResult(OperationId),
    ExactIntegerMultiplyOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerDivideRequiresIntegerResult(OperationId),
    ExactIntegerDivideOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerRemainderRequiresIntegerResult(OperationId),
    ExactIntegerRemainderOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    WrappingIntegerDivideRequiresIntegerResult(OperationId),
    WrappingIntegerDivideOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    WrappingIntegerRemainderRequiresIntegerResult(OperationId),
    WrappingIntegerRemainderOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    SaturatingIntegerDivideRequiresIntegerResult(OperationId),
    SaturatingIntegerDivideOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    SaturatingIntegerRemainderRequiresIntegerResult(OperationId),
    SaturatingIntegerRemainderOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    WrappingIntegerAddRequiresIntegerResult(OperationId),
    SaturatingIntegerAddRequiresIntegerResult(OperationId),
    WrappingIntegerSubtractRequiresIntegerResult(OperationId),
    SaturatingIntegerSubtractRequiresIntegerResult(OperationId),
    WrappingIntegerMultiplyRequiresIntegerResult(OperationId),
    SaturatingIntegerMultiplyRequiresIntegerResult(OperationId),
    WrappingIntegerAddOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerAddOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    WrappingIntegerSubtractOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerSubtractOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    WrappingIntegerMultiplyOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerMultiplyOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    JumpArityMismatch {
        edge: EdgeId,
        expected: usize,
        actual: usize,
    },
    JumpTypeMismatch {
        edge: EdgeId,
        argument: ScalarType,
        parameter: ScalarType,
    },
    ConditionalConditionTypeMismatch {
        block: BlockId,
        condition: ValueId,
        actual: ScalarType,
    },
    ReturnTypeMismatch {
        machine: MachineId,
        value: ScalarType,
        result: ScalarType,
    },
    ScalarReturnFromUnitMachine {
        machine: MachineId,
        block: BlockId,
    },
    UnitReturnFromScalarMachine {
        machine: MachineId,
        block: BlockId,
    },
    ControlCycle(BlockId),
    UnreachableBlock(BlockId),
    MalformedProposition(PropositionError),
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ModuleError {}
