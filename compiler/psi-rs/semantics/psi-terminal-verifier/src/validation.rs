use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    BlockId, ContractId, EdgeId, MachineId, ObligationId, OperationId, Proposition,
    PropositionContext, PropositionError, ScalarTerm, ScalarType, ValueId,
};
use psi_terminal::{OperationKind, SemanticVersion, TerminalMachine, TerminalModule, Terminator};

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
        PropositionContext::from_value_types(machine_value_types(machine))
            .map_err(ModuleError::MalformedProposition)
    }
}

pub fn validate_module(
    module: &TerminalModule,
) -> Result<ValidatedTerminalModule<'_>, ModuleError> {
    if !matches!(
        module.semantic_version,
        SemanticVersion::V1 | SemanticVersion::V2 | SemanticVersion::V3
    ) {
        return Err(ModuleError::UnsupportedSemanticVersion(
            module.semantic_version,
        ));
    }
    if module.machines.is_empty() {
        return Err(ModuleError::EmptyModule);
    }

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
        validate_machine(module.semantic_version, machine, &mut registry)?;
    }
    if !registry.machines.contains(&module.entry) {
        return Err(ModuleError::UnknownEntryMachine(module.entry));
    }

    Ok(ValidatedTerminalModule { module })
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
}

fn validate_machine(
    semantic_version: SemanticVersion,
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
) -> Result<(), ModuleError> {
    if machine.blocks.is_empty() {
        return Err(ModuleError::MachineHasNoBlocks(machine.id));
    }

    let mut blocks = BTreeMap::new();
    let mut value_types = BTreeMap::new();
    for declaration in machine
        .parameters
        .iter()
        .chain(std::iter::once(&machine.result))
    {
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
            match operation.kind {
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
                    if semantic_version < SemanticVersion::V2 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V2,
                            actual: semantic_version,
                        });
                    }
                    if operation.result.scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanConstantRequiresBooleanResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerAdd { .. } => {
                    if semantic_version < SemanticVersion::V3 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V3,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::WrappingIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
            }
        }
        insert_unique(
            &mut registry.edges,
            block.terminator.edge(),
            ModuleError::DuplicateEdge,
        )?;
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

    let context =
        PropositionContext::from_value_types(value_types.iter().map(|(id, ty)| (*id, *ty)))
            .map_err(ModuleError::MalformedProposition)?;
    let requires_values = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    let mut ensures_values = requires_values.clone();
    ensures_values.insert(machine.result.id);
    for proposition in &machine.contract.requires {
        validate_proposition_semantic_version(
            proposition,
            semantic_version,
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
        validate_proposition_semantic_version(
            &clause.proposition,
            semantic_version,
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

    validate_straight_line_flow(machine, &blocks, &value_types)
}

fn validate_proposition_semantic_version(
    proposition: &Proposition,
    semantic_version: SemanticVersion,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match proposition {
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => Ok(()),
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        Proposition::Conjunction(conjuncts) => {
            for conjunct in conjuncts {
                validate_proposition_semantic_version(
                    conjunct,
                    semantic_version,
                    contract,
                    clause,
                )?;
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_proposition_semantic_version(premise, semantic_version, contract, clause)?;
            validate_proposition_semantic_version(conclusion, semantic_version, contract, clause)
        }
    }
}

fn validate_term_semantic_version(
    term: &ScalarTerm,
    semantic_version: SemanticVersion,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match term {
        ScalarTerm::WrappingIntegerAdd { left, right, .. } => {
            if semantic_version < SemanticVersion::V3 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V3,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => Ok(()),
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
        ScalarTerm::WrappingIntegerAdd { left, right, .. } => {
            validate_term_scope(left, allowed, contract, clause)?;
            validate_term_scope(right, allowed, contract, clause)?;
        }
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
    }
    Ok(())
}

fn validate_straight_line_flow(
    machine: &TerminalMachine,
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<(), ModuleError> {
    let mut defined = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    let mut visited = BTreeSet::new();
    let mut current = machine.entry;

    loop {
        if !visited.insert(current) {
            return Err(ModuleError::ControlCycle(current));
        }
        let block = blocks
            .get(&current)
            .copied()
            .ok_or(ModuleError::UnknownTargetBlock(current))?;
        for parameter in &block.parameters {
            defined.insert(parameter.id);
        }
        for operation in &block.operations {
            if let OperationKind::WrappingIntegerAdd { left, right } = operation.kind {
                let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                    unreachable!("operation shape validation requires an integer result")
                };
                for operand in [left, right] {
                    if !defined.contains(&operand) {
                        return Err(ModuleError::ValueUsedBeforeDefinition(operand));
                    }
                    let actual = value_types
                        .get(&operand)
                        .copied()
                        .ok_or(ModuleError::UnknownValue(operand))?;
                    let expected = ScalarType::Integer(integer_type);
                    if actual != expected {
                        return Err(ModuleError::WrappingIntegerAddOperandTypeMismatch {
                            operation: operation.id,
                            operand,
                            expected,
                            actual,
                        });
                    }
                }
            }
            defined.insert(operation.result.id);
        }
        match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => {
                let target_block = blocks
                    .get(target)
                    .copied()
                    .ok_or(ModuleError::UnknownTargetBlock(*target))?;
                if target_block.parameters.len() != arguments.len() {
                    return Err(ModuleError::JumpArityMismatch {
                        edge: block.terminator.edge(),
                        expected: target_block.parameters.len(),
                        actual: arguments.len(),
                    });
                }
                for (argument, parameter) in arguments.iter().zip(&target_block.parameters) {
                    if !defined.contains(argument) {
                        return Err(ModuleError::ValueUsedBeforeDefinition(*argument));
                    }
                    let argument_type = value_types
                        .get(argument)
                        .copied()
                        .ok_or(ModuleError::UnknownValue(*argument))?;
                    if argument_type != parameter.scalar_type {
                        return Err(ModuleError::JumpTypeMismatch {
                            edge: block.terminator.edge(),
                            argument: argument_type,
                            parameter: parameter.scalar_type,
                        });
                    }
                }
                current = *target;
            }
            Terminator::Return { value, .. } => {
                if !defined.contains(value) {
                    return Err(ModuleError::ValueUsedBeforeDefinition(*value));
                }
                let value_type = value_types
                    .get(value)
                    .copied()
                    .ok_or(ModuleError::UnknownValue(*value))?;
                if value_type != machine.result.scalar_type {
                    return Err(ModuleError::ReturnTypeMismatch {
                        machine: machine.id,
                        value: value_type,
                        result: machine.result.scalar_type,
                    });
                }
                break;
            }
        }
    }

    if visited.len() != blocks.len() {
        let block = blocks
            .keys()
            .find(|block| !visited.contains(block))
            .copied()
            .expect("different set lengths guarantee an unvisited block");
        return Err(ModuleError::UnreachableBlock(block));
    }
    Ok(())
}

pub(crate) fn machine_value_types(
    machine: &TerminalMachine,
) -> impl Iterator<Item = (ValueId, ScalarType)> + '_ {
    machine
        .parameters
        .iter()
        .chain(std::iter::once(&machine.result))
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleError {
    UnsupportedSemanticVersion(SemanticVersion),
    EmptyModule,
    DuplicateMachine(MachineId),
    DuplicateBlock(BlockId),
    DuplicateContract(ContractId),
    DuplicateOperation(OperationId),
    DuplicateEdge(EdgeId),
    DuplicateObligation(ObligationId),
    DuplicateValue(ValueId),
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
    UnknownTargetBlock(BlockId),
    UnknownValue(ValueId),
    ValueUsedBeforeDefinition(ValueId),
    IntegerConstantRequiresIntegerResult(OperationId),
    IntegerConstantOutsideResultType(OperationId),
    BooleanConstantRequiresBooleanResult(OperationId),
    WrappingIntegerAddRequiresIntegerResult(OperationId),
    WrappingIntegerAddOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    OperationRequiresSemanticVersion {
        operation: OperationId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    PropositionRequiresSemanticVersion {
        contract: ContractId,
        clause: ContractClauseKind,
        required: SemanticVersion,
        actual: SemanticVersion,
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
    ReturnTypeMismatch {
        machine: MachineId,
        value: ScalarType,
        result: ScalarType,
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
