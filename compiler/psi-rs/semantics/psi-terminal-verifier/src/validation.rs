use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    BlockId, ClaimId, ContentAlgebra, ContentConservation, ContentPlaceSegment,
    ContentProjectionIdentity, ContentStructuralPlace, ContentTerm, ContractId, EdgeId, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, PropositionContext, PropositionError,
    PropositionId, ScalarTerm, ScalarType, StructuralPlaceKind, ValueId,
};
use psi_language_semantics::crash::scope_covers_minimum;
use psi_terminal::{
    ContentPartitionComposition, CrashCause, OperationKind, PropositionBinderArgumentKind,
    PropositionBinderKind, PropositionEvidence, SemanticVersion, TerminalMachine, TerminalModule,
    Terminator,
};

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
    if !matches!(
        module.semantic_version,
        SemanticVersion::V1
            | SemanticVersion::V2
            | SemanticVersion::V3
            | SemanticVersion::V4
            | SemanticVersion::V5
            | SemanticVersion::V6
            | SemanticVersion::V7
            | SemanticVersion::V8
            | SemanticVersion::V9
            | SemanticVersion::V10
            | SemanticVersion::V11
            | SemanticVersion::V12
            | SemanticVersion::V13
            | SemanticVersion::V14
            | SemanticVersion::V15
            | SemanticVersion::V16
            | SemanticVersion::V17
            | SemanticVersion::V18
            | SemanticVersion::V19
            | SemanticVersion::V20
            | SemanticVersion::V21
            | SemanticVersion::V22
            | SemanticVersion::V23
            | SemanticVersion::V24
            | SemanticVersion::V25
            | SemanticVersion::V26
            | SemanticVersion::V27
            | SemanticVersion::V28
            | SemanticVersion::V29
            | SemanticVersion::V30
            | SemanticVersion::V31
            | SemanticVersion::V32
            | SemanticVersion::V33
            | SemanticVersion::V34
            | SemanticVersion::V35
    ) {
        return Err(ModuleError::UnsupportedSemanticVersion(
            module.semantic_version,
        ));
    }
    if module.machines.is_empty() {
        return Err(ModuleError::EmptyModule);
    }
    if module.semantic_version < SemanticVersion::V27 && module_uses_address_carrier(module) {
        return Err(ModuleError::AddressCarrierRequiresSemanticVersion {
            required: SemanticVersion::V27,
            actual: module.semantic_version,
        });
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
        validate_machine(module.semantic_version, machine, &mut registry)?;
    }
    if !registry.machines.contains(&module.entry) {
        return Err(ModuleError::UnknownEntryMachine(module.entry));
    }

    Ok(ValidatedTerminalModule { module })
}

fn module_uses_address_carrier(module: &TerminalModule) -> bool {
    module.machines.iter().any(|machine| {
        machine
            .parameters
            .iter()
            .chain(std::iter::once(&machine.result))
            .any(|declaration| scalar_type_uses_address(declaration.scalar_type))
            || machine.blocks.iter().any(|block| {
                block
                    .parameters
                    .iter()
                    .any(|declaration| scalar_type_uses_address(declaration.scalar_type))
                    || block
                        .operations
                        .iter()
                        .any(|operation| scalar_type_uses_address(operation.result.scalar_type))
            })
            || machine
                .contract
                .requires
                .iter()
                .any(proposition_uses_address)
            || machine
                .contract
                .ensures
                .iter()
                .any(|clause| proposition_uses_address(&clause.proposition))
    })
}

fn scalar_type_uses_address(scalar_type: ScalarType) -> bool {
    matches!(scalar_type, ScalarType::Integer(integer_type) if integer_type.is_address())
}

fn proposition_uses_address(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_address(left) || scalar_term_uses_address(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_address),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_address(premise) || proposition_uses_address(conclusion),
    }
}

fn scalar_term_uses_address(term: &ScalarTerm) -> bool {
    scalar_type_uses_address(term.scalar_type())
        || match term {
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_address(operand),
            ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                scalar_term_uses_address(value) || scalar_term_uses_address(count)
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                scalar_term_uses_address(left) || scalar_term_uses_address(right)
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
                scalar_term_uses_address(value) || scalar_term_uses_address(count)
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
        }
}

fn validate_proposition_vocabulary(module: &TerminalModule) -> Result<(), ModuleError> {
    if module.semantic_version < SemanticVersion::V16
        && (!module.proposition_declarations.is_empty()
            || !module.proposition_applications.is_empty())
    {
        return Err(ModuleError::PropositionVocabularyRequiresSemanticVersion {
            required: SemanticVersion::V16,
            actual: module.semantic_version,
        });
    }
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
    semantic_version: SemanticVersion,
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
) -> Result<(), ModuleError> {
    if machine.blocks.is_empty() {
        return Err(ModuleError::MachineHasNoBlocks(machine.id));
    }

    let mut blocks = BTreeMap::new();
    let mut value_types = BTreeMap::new();
    let mut structural_roots = BTreeSet::new();
    if semantic_version < SemanticVersion::V9 && !machine.structural_places.is_empty() {
        return Err(ModuleError::StructuralPlacesRequireSemanticVersion {
            machine: machine.id,
            required: SemanticVersion::V9,
            actual: semantic_version,
        });
    }
    if semantic_version < SemanticVersion::V10 && !machine.content_identity_reshuffles.is_empty() {
        return Err(
            ModuleError::ContentIdentityReshufflesRequireSemanticVersion {
                machine: machine.id,
                required: SemanticVersion::V10,
                actual: semantic_version,
            },
        );
    }
    if semantic_version < SemanticVersion::V14 && !machine.content_entry_claims.is_empty() {
        return Err(ModuleError::ContentEntryClaimsRequireSemanticVersion {
            machine: machine.id,
            required: SemanticVersion::V14,
            actual: semantic_version,
        });
    }
    if semantic_version < SemanticVersion::V12 && !machine.content_partition_compositions.is_empty()
    {
        return Err(
            ModuleError::ContentPartitionCompositionsRequireSemanticVersion {
                machine: machine.id,
                required: SemanticVersion::V12,
                actual: semantic_version,
            },
        );
    }
    let mut structural_place_kinds = BTreeMap::new();
    for place in &machine.structural_places {
        insert_unique(&mut registry.places, place.id, ModuleError::DuplicatePlace)?;
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
                OperationKind::BooleanNot { .. } => {
                    if semantic_version < SemanticVersion::V15 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V15,
                            actual: semantic_version,
                        });
                    }
                    if operation.result.scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanNotRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::BooleanEqual { .. } => {
                    if semantic_version < SemanticVersion::V17 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V17,
                            actual: semantic_version,
                        });
                    }
                    if operation.result.scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanEqualRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::IntegerEqual { .. } => {
                    if semantic_version < SemanticVersion::V18 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V18,
                            actual: semantic_version,
                        });
                    }
                    if operation.result.scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::IntegerEqualRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::IntegerLessThan { .. }
                | OperationKind::IntegerLessOrEqual { .. } => {
                    if semantic_version < SemanticVersion::V19 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V19,
                            actual: semantic_version,
                        });
                    }
                    if operation.result.scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::IntegerOrderingRequiresBooleanResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerBitwiseAnd { .. }
                | OperationKind::IntegerBitwiseOr { .. }
                | OperationKind::IntegerBitwiseXor { .. } => {
                    if semantic_version < SemanticVersion::V20 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V20,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::IntegerBitwiseRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerBitwiseNot { .. } => {
                    if semantic_version < SemanticVersion::V25 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V25,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::IntegerBitwiseRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerWiden { .. } => {
                    if semantic_version < SemanticVersion::V26 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V26,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::IntegerWidenRequiresIntegerResult(operation.id));
                    }
                }
                OperationKind::IntegerExactCast { obligation, .. } => {
                    if semantic_version < SemanticVersion::V28 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V28,
                            actual: semantic_version,
                        });
                    }
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
                    if semantic_version < SemanticVersion::V21 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V21,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::WrappingIntegerShiftRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::ExactIntegerShiftRight { obligation, .. } => {
                    if semantic_version < SemanticVersion::V29 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V29,
                            actual: semantic_version,
                        });
                    }
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
                    if semantic_version < SemanticVersion::V30 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V30,
                            actual: semantic_version,
                        });
                    }
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
                    if semantic_version < SemanticVersion::V31 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V31,
                            actual: semantic_version,
                        });
                    }
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
                    if semantic_version < SemanticVersion::V32 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V32,
                            actual: semantic_version,
                        });
                    }
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
                    if semantic_version < SemanticVersion::V33 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V33,
                            actual: semantic_version,
                        });
                    }
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
                    if semantic_version < SemanticVersion::V34 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V34,
                            actual: semantic_version,
                        });
                    }
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
                    if semantic_version < SemanticVersion::V35 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V35,
                            actual: semantic_version,
                        });
                    }
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
                OperationKind::SaturatingIntegerAdd { .. } => {
                    if semantic_version < SemanticVersion::V4 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V4,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::SaturatingIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerSubtract { .. } => {
                    if semantic_version < SemanticVersion::V5 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V5,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::WrappingIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerSubtract { .. } => {
                    if semantic_version < SemanticVersion::V6 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V6,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::SaturatingIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerMultiply { .. } => {
                    if semantic_version < SemanticVersion::V7 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V7,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::WrappingIntegerMultiplyRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerMultiply { .. } => {
                    if semantic_version < SemanticVersion::V8 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V8,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::SaturatingIntegerMultiplyRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
            }
        }
        if semantic_version < SemanticVersion::V13
            && matches!(block.terminator, Terminator::Conditional { .. })
        {
            return Err(ModuleError::ConditionalRequiresSemanticVersion {
                block: block.id,
                required: SemanticVersion::V13,
                actual: semantic_version,
            });
        }
        if semantic_version < SemanticVersion::V22
            && matches!(block.terminator, Terminator::Crash { .. })
        {
            return Err(ModuleError::CrashRequiresSemanticVersion {
                block: block.id,
                required: SemanticVersion::V22,
                actual: semantic_version,
            });
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
    validate_content_entry_claims(
        machine,
        semantic_version,
        registry,
        &structural_place_kinds,
        &context,
    )?;
    validate_content_identity_reshuffles(
        machine,
        semantic_version,
        registry,
        &structural_place_kinds,
        &context,
    )?;
    validate_content_partition_compositions(
        machine,
        semantic_version,
        registry,
        &structural_place_kinds,
        &context,
    )?;
    validate_crash_frontiers(machine, semantic_version)?;
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

    validate_control_flow(machine, &blocks, &value_types)
}

fn validate_crash_frontiers(
    machine: &TerminalMachine,
    semantic_version: SemanticVersion,
) -> Result<(), ModuleError> {
    if semantic_version < SemanticVersion::V24 && !machine.contract.crash_context.is_empty() {
        return Err(ModuleError::CrashContextRequiresSemanticVersion {
            machine: machine.id,
            required: SemanticVersion::V24,
            actual: semantic_version,
        });
    }
    if machine
        .contract
        .crash_context
        .windows(2)
        .any(|pair| pair[0].cause >= pair[1].cause)
    {
        return Err(ModuleError::NonCanonicalCrashContext(machine.id));
    }
    if let Some(maximum) = machine
        .contract
        .crash_context
        .iter()
        .find(|maximum| maximum.maximum_scope.is_empty())
    {
        return Err(ModuleError::EmptyCrashContextMaximum {
            machine: machine.id,
            cause: maximum.cause,
        });
    }
    let expected = machine
        .content_entry_claims
        .iter()
        .map(|binding| binding.claim)
        .collect::<Vec<_>>();
    for block in &machine.blocks {
        let Terminator::Crash {
            cause,
            damage_minimum,
            containment_demand,
            frontier_lower_bound,
            ..
        } = &block.terminator
        else {
            continue;
        };
        if damage_minimum.is_empty() {
            return Err(ModuleError::EmptyCrashDamageMinimum(block.id));
        }
        if containment_demand.is_empty() {
            return Err(ModuleError::EmptyCrashContainmentDemand(block.id));
        }
        if semantic_version < SemanticVersion::V23 && damage_minimum != containment_demand {
            return Err(ModuleError::SeparatedCrashScopesRequireSemanticVersion {
                block: block.id,
                required: SemanticVersion::V23,
                actual: semantic_version,
            });
        }
        if !scope_covers_minimum(damage_minimum, containment_demand) {
            return Err(ModuleError::CrashContainmentDemandTooNarrow { block: block.id });
        }
        if semantic_version >= SemanticVersion::V24 {
            let maximum = machine
                .contract
                .crash_context
                .iter()
                .find(|maximum| maximum.cause == *cause)
                .ok_or(ModuleError::MissingCrashContextMaximum {
                    block: block.id,
                    cause: *cause,
                })?;
            if !scope_covers_minimum(containment_demand, &maximum.maximum_scope) {
                return Err(ModuleError::CrashContextMaximumTooNarrow { block: block.id });
            }
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
    semantic_version: SemanticVersion,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    if semantic_version < SemanticVersion::V14 {
        return Ok(());
    }
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
    semantic_version: SemanticVersion,
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
        if semantic_version >= SemanticVersion::V14 {
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
        }
        if semantic_version < SemanticVersion::V11
            && reshuffle
                .input
                .segments
                .iter()
                .chain(&reshuffle.output.segments)
                .any(|segment| matches!(segment, ContentPlaceSegment::Case(_)))
        {
            return Err(
                ModuleError::ContentIdentityCasePathRequiresSemanticVersion {
                    claim: reshuffle.claim,
                    required: SemanticVersion::V11,
                    actual: semantic_version,
                },
            );
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
    semantic_version: SemanticVersion,
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
        validate_partition_case_version(composition, semantic_version)?;
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
            let matching = if semantic_version >= SemanticVersion::V14 {
                machine
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
                    .collect::<Vec<_>>()
            } else {
                machine
                    .content_identity_reshuffles
                    .iter()
                    .filter(|reshuffle| {
                        reshuffle.input == subject
                            && reshuffle.projections.iter().any(|content| {
                                content.projection == projection
                                    && content.algebra == *composition.derived.algebra()
                            })
                    })
                    .map(|reshuffle| reshuffle.claim)
                    .collect::<Vec<_>>()
            };
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

fn validate_partition_case_version(
    composition: &ContentPartitionComposition,
    semantic_version: SemanticVersion,
) -> Result<(), ModuleError> {
    if semantic_version < SemanticVersion::V11
        && [
            composition.source.left(),
            composition.source.right(),
            composition.derived.left(),
            composition.derived.right(),
        ]
        .into_iter()
        .any(content_term_uses_case)
    {
        return Err(
            ModuleError::ContentPartitionCasePathRequiresSemanticVersion {
                required: SemanticVersion::V11,
                actual: semantic_version,
            },
        );
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
        Proposition::ContentConservation(conservation) => {
            if semantic_version < SemanticVersion::V9 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V9,
                    actual: semantic_version,
                });
            }
            if clause != ContractClauseKind::Ensures {
                return Err(ModuleError::ContentConservationRequiresEnsures { contract });
            }
            if semantic_version < SemanticVersion::V11
                && (content_term_uses_case(conservation.left())
                    || content_term_uses_case(conservation.right()))
            {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V11,
                    actual: semantic_version,
                });
            }
            Ok(())
        }
    }
}

fn content_term_uses_case(term: &ContentTerm) -> bool {
    match term {
        ContentTerm::Projection { subject, .. } => subject
            .segments
            .iter()
            .any(|segment| matches!(segment, ContentPlaceSegment::Case(_))),
        ContentTerm::Separate(terms) => terms.iter().any(content_term_uses_case),
    }
}

fn validate_term_semantic_version(
    term: &ScalarTerm,
    semantic_version: SemanticVersion,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match term {
        ScalarTerm::IntegerBitwiseNot { operand, .. } => {
            if semantic_version < SemanticVersion::V25 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V25,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(operand, semantic_version, contract, clause)
        }
        ScalarTerm::IntegerWiden { operand, .. } => {
            if semantic_version < SemanticVersion::V26 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V26,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(operand, semantic_version, contract, clause)
        }
        ScalarTerm::IntegerExactCast { operand, .. } => {
            if semantic_version < SemanticVersion::V28 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V28,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(operand, semantic_version, contract, clause)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            if semantic_version < SemanticVersion::V21 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V21,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(value, semantic_version, contract, clause)?;
            validate_term_semantic_version(count, semantic_version, contract, clause)
        }
        ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            if semantic_version < SemanticVersion::V29 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V29,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(value, semantic_version, contract, clause)?;
            validate_term_semantic_version(count, semantic_version, contract, clause)
        }
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. } => {
            if semantic_version < SemanticVersion::V30 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V30,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(value, semantic_version, contract, clause)?;
            validate_term_semantic_version(count, semantic_version, contract, clause)
        }
        ScalarTerm::ExactIntegerAdd { left, right, .. } => {
            if semantic_version < SemanticVersion::V31 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V31,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::ExactIntegerSubtract { left, right, .. } => {
            if semantic_version < SemanticVersion::V32 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V32,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
            if semantic_version < SemanticVersion::V33 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V33,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::ExactIntegerDivide { left, right, .. } => {
            if semantic_version < SemanticVersion::V34 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V34,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::ExactIntegerRemainder { left, right, .. } => {
            if semantic_version < SemanticVersion::V35 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V35,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            if semantic_version < SemanticVersion::V20 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V20,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. } => {
            if semantic_version < SemanticVersion::V19 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V19,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::IntegerEqual { left, right, .. } => {
            if semantic_version < SemanticVersion::V18 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V18,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::BooleanEqual { left, right } => {
            if semantic_version < SemanticVersion::V17 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V17,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::BooleanNot { operand } => {
            if semantic_version < SemanticVersion::V15 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V15,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(operand, semantic_version, contract, clause)
        }
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
        ScalarTerm::SaturatingIntegerAdd { left, right, .. } => {
            if semantic_version < SemanticVersion::V4 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V4,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::WrappingIntegerSubtract { left, right, .. } => {
            if semantic_version < SemanticVersion::V5 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V5,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::SaturatingIntegerSubtract { left, right, .. } => {
            if semantic_version < SemanticVersion::V6 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V6,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::WrappingIntegerMultiply { left, right, .. } => {
            if semantic_version < SemanticVersion::V7 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V7,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            if semantic_version < SemanticVersion::V8 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V8,
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
            Terminator::Return { .. } | Terminator::Crash { .. } => Vec::new(),
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
            validate_operation_operands(operation, value_types, &defined)?;
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
                require_defined(*value, value_types, &defined)?;
                let value_type = value_types[value];
                if value_type != machine.result.scalar_type {
                    return Err(ModuleError::ReturnTypeMismatch {
                        machine: machine.id,
                        value: value_type,
                        result: machine.result.scalar_type,
                    });
                }
            }
            Terminator::Crash { .. } => {}
        }
    }
    Ok(())
}

fn validate_operation_operands(
    operation: &psi_terminal::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if let OperationKind::IntegerExactCast { operand, .. } = operation.kind {
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
    if let OperationKind::IntegerWiden { operand } = operation.kind {
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
    if let OperationKind::IntegerBitwiseNot { operand } = operation.kind {
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
    if let OperationKind::BooleanNot { operand } = operation.kind {
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
    if let OperationKind::BooleanEqual { left, right } = operation.kind {
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
    if let OperationKind::IntegerEqual { left, right } = operation.kind {
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
    | OperationKind::IntegerLessOrEqual { left, right } = operation.kind
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
    | OperationKind::IntegerBitwiseXor { left, right } = operation.kind
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
    | OperationKind::WrappingIntegerShiftRight { value, count } = operation.kind
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
    | OperationKind::ExactIntegerShiftRight { value, count, .. } = operation.kind
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
    if let OperationKind::ExactIntegerAdd { left, right, .. } = operation.kind {
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
    if let OperationKind::ExactIntegerSubtract { left, right, .. } = operation.kind {
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
    if let OperationKind::ExactIntegerMultiply { left, right, .. } = operation.kind {
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
    if let OperationKind::ExactIntegerDivide { left, right, .. } = operation.kind {
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
    if let OperationKind::ExactIntegerRemainder { left, right, .. } = operation.kind {
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
    let Some((left, right, arithmetic)) = (match operation.kind {
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
    AddressCarrierRequiresSemanticVersion {
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    EmptyModule,
    PropositionVocabularyRequiresSemanticVersion {
        required: SemanticVersion,
        actual: SemanticVersion,
    },
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
    StructuralPlacesRequireSemanticVersion {
        machine: MachineId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    ContentIdentityReshufflesRequireSemanticVersion {
        machine: MachineId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    ContentEntryClaimsRequireSemanticVersion {
        machine: MachineId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    ContentPartitionCompositionsRequireSemanticVersion {
        machine: MachineId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    ConditionalRequiresSemanticVersion {
        block: BlockId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    CrashRequiresSemanticVersion {
        block: BlockId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    CrashContextRequiresSemanticVersion {
        machine: MachineId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    SeparatedCrashScopesRequireSemanticVersion {
        block: BlockId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    EmptyCrashDamageMinimum(BlockId),
    EmptyCrashContainmentDemand(BlockId),
    CrashContainmentDemandTooNarrow {
        block: BlockId,
    },
    NonCanonicalCrashContext(MachineId),
    EmptyCrashContextMaximum {
        machine: MachineId,
        cause: CrashCause,
    },
    MissingCrashContextMaximum {
        block: BlockId,
        cause: CrashCause,
    },
    CrashContextMaximumTooNarrow {
        block: BlockId,
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
    ContentIdentityCasePathRequiresSemanticVersion {
        claim: ClaimId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
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
    ContentPartitionCasePathRequiresSemanticVersion {
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    ContentConservationRequiresEnsures {
        contract: ContractId,
    },
    UnknownTargetBlock(BlockId),
    UnknownValue(ValueId),
    ValueUsedBeforeDefinition(ValueId),
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
