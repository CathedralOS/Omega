use std::collections::BTreeSet;

use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, ValidatedRegisterConstraintCatalog,
};
use omega_target::NativeTarget;

use crate::{TerminalSelectedConstraintKeys, terminal_machine_effect_catalog_identity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalMachineEffectCatalogIdentity([u8; 32]);

impl TerminalMachineEffectCatalogIdentity {
    pub(crate) fn from_canonical_bytes(bytes: &[u8]) -> Self {
        use sha2::{Digest, Sha256};
        Self(Sha256::digest(bytes).into())
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalMachineSemanticKind {
    CompareI64Zero,
    MaterializeI64,
    CopyI64,
    ExactAddI64,
    ExactAddI64Immediate,
    ExactSubtractI64,
    ConditionalBranchNonZero,
    ReturnI64,
}

impl TerminalMachineSemanticKind {
    pub const ALL: [Self; 8] = [
        Self::CompareI64Zero,
        Self::MaterializeI64,
        Self::CopyI64,
        Self::ExactAddI64,
        Self::ExactAddI64Immediate,
        Self::ExactSubtractI64,
        Self::ConditionalBranchNonZero,
        Self::ReturnI64,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalMachineAlternativeFamily {
    CompareI64Zero,
    MaterializeI64,
    CopyI64,
    ExactAddI64,
    ExactAddI64Immediate,
    ExactSubtractI64,
    ConditionalBranchNonZero,
    ReturnI64,
}

impl From<TerminalMachineSemanticKind> for TerminalMachineAlternativeFamily {
    fn from(value: TerminalMachineSemanticKind) -> Self {
        match value {
            TerminalMachineSemanticKind::CompareI64Zero => Self::CompareI64Zero,
            TerminalMachineSemanticKind::MaterializeI64 => Self::MaterializeI64,
            TerminalMachineSemanticKind::CopyI64 => Self::CopyI64,
            TerminalMachineSemanticKind::ExactAddI64 => Self::ExactAddI64,
            TerminalMachineSemanticKind::ExactAddI64Immediate => Self::ExactAddI64Immediate,
            TerminalMachineSemanticKind::ExactSubtractI64 => Self::ExactSubtractI64,
            TerminalMachineSemanticKind::ConditionalBranchNonZero => Self::ConditionalBranchNonZero,
            TerminalMachineSemanticKind::ReturnI64 => Self::ReturnI64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalMachineAlternativeKey {
    pub family: TerminalMachineAlternativeFamily,
    pub variant: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalMachineAlternativeApplicability {
    Always,
    ResultAliasesOperand {
        result: u16,
        operand: u16,
    },
    ResultAliasesOperandAndDistinctFromOperand {
        result: u16,
        aliased_operand: u16,
        distinct_operand: u16,
    },
    ResultAliasesOperands {
        result: u16,
        left: u16,
        right: u16,
    },
    ResultDistinctFromOperands {
        result: u16,
        left: u16,
        right: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMachineMemoryEffect {
    NoneV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMachineTrapBehavior {
    NeverV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMachineBarrier {
    None,
    ControlFlow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMachineCallEffect {
    NoneV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMachineCleanupEffect {
    NoneV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMachineSizeKnowledge {
    ExactBytes(u16),
    EncoderResolved {
        minimum_bytes: u16,
        maximum_bytes: Option<u16>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMachineLatencyKnowledge {
    StableBaselineUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalMachineAlternative {
    pub key: TerminalMachineAlternativeKey,
    pub applicability: TerminalMachineAlternativeApplicability,
    pub size: TerminalMachineSizeKnowledge,
    pub latency: TerminalMachineLatencyKnowledge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachineEffectDeclaration {
    pub semantic: TerminalMachineSemanticKind,
    pub constraint: RegisterConstraintKey,
    pub memory: TerminalMachineMemoryEffect,
    pub trap: TerminalMachineTrapBehavior,
    pub barrier: TerminalMachineBarrier,
    pub call: TerminalMachineCallEffect,
    pub cleanup: TerminalMachineCleanupEffect,
    pub alternatives: Vec<TerminalMachineAlternative>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachineEffectCatalog {
    pub target: NativeTarget,
    pub register_constraints: RegisterConstraintCatalogIdentity,
    pub selected_keys: TerminalSelectedConstraintKeys,
    pub declarations: Vec<TerminalMachineEffectDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalMachineEffectCatalog {
    catalog: TerminalMachineEffectCatalog,
    identity: TerminalMachineEffectCatalogIdentity,
}

impl ValidatedTerminalMachineEffectCatalog {
    pub const fn catalog(&self) -> &TerminalMachineEffectCatalog {
        &self.catalog
    }

    pub const fn identity(&self) -> TerminalMachineEffectCatalogIdentity {
        self.identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMachineEffectCatalogValidationError {
    TargetArchitectureMismatch,
    RegisterConstraintRootMismatch,
    DuplicateSelectedConstraintKey,
    NonCanonicalDeclarations,
    DeclarationRosterMismatch,
    UnknownConstraint(TerminalMachineSemanticKind),
    NonCanonicalAlternatives(TerminalMachineSemanticKind),
    EmptyAlternatives(TerminalMachineSemanticKind),
    AlternativeFamilyMismatch(TerminalMachineSemanticKind),
    InvalidAlternativeApplicability(TerminalMachineSemanticKind),
    InvalidSizeKnowledge(TerminalMachineSemanticKind),
    BarrierMismatch(TerminalMachineSemanticKind),
}

impl std::fmt::Display for TerminalMachineEffectCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Terminal machine-effect catalog: {self:?}"
        )
    }
}

impl std::error::Error for TerminalMachineEffectCatalogValidationError {}

pub fn validate_terminal_machine_effect_catalog(
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: TerminalMachineEffectCatalog,
) -> Result<ValidatedTerminalMachineEffectCatalog, TerminalMachineEffectCatalogValidationError> {
    if catalog.target.architecture != constraints.architecture() {
        return Err(TerminalMachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    if catalog.register_constraints != constraints.identity() {
        return Err(TerminalMachineEffectCatalogValidationError::RegisterConstraintRootMismatch);
    }
    let selected = catalog.selected_keys.in_identity_order();
    if selected.iter().copied().collect::<BTreeSet<_>>().len() != selected.len() {
        return Err(TerminalMachineEffectCatalogValidationError::DuplicateSelectedConstraintKey);
    }
    if catalog
        .declarations
        .windows(2)
        .any(|pair| pair[0].semantic >= pair[1].semantic)
    {
        return Err(TerminalMachineEffectCatalogValidationError::NonCanonicalDeclarations);
    }
    let expected = TerminalMachineSemanticKind::ALL
        .map(|semantic| (semantic, catalog.selected_keys.for_semantic(semantic)));
    if catalog.declarations.len() != expected.len()
        || catalog
            .declarations
            .iter()
            .zip(expected)
            .any(|(actual, (semantic, constraint))| {
                actual.semantic != semantic || actual.constraint != constraint
            })
    {
        return Err(TerminalMachineEffectCatalogValidationError::DeclarationRosterMismatch);
    }
    for declaration in &catalog.declarations {
        let row = constraints
            .catalog()
            .constraints
            .iter()
            .find(|row| row.key == declaration.constraint)
            .ok_or(
                TerminalMachineEffectCatalogValidationError::UnknownConstraint(
                    declaration.semantic,
                ),
            )?;
        validate_declaration(row, declaration)?;
    }
    let identity = terminal_machine_effect_catalog_identity(&catalog);
    Ok(ValidatedTerminalMachineEffectCatalog { catalog, identity })
}

fn validate_declaration(
    constraint: &RegisterInstructionConstraint,
    declaration: &TerminalMachineEffectDeclaration,
) -> Result<(), TerminalMachineEffectCatalogValidationError> {
    let semantic = declaration.semantic;
    let expected_barrier = if matches!(
        semantic,
        TerminalMachineSemanticKind::ConditionalBranchNonZero
            | TerminalMachineSemanticKind::ReturnI64
    ) {
        TerminalMachineBarrier::ControlFlow
    } else {
        TerminalMachineBarrier::None
    };
    if declaration.barrier != expected_barrier {
        return Err(TerminalMachineEffectCatalogValidationError::BarrierMismatch(semantic));
    }
    if declaration.alternatives.is_empty() {
        return Err(TerminalMachineEffectCatalogValidationError::EmptyAlternatives(semantic));
    }
    if declaration
        .alternatives
        .windows(2)
        .any(|pair| pair[0].key >= pair[1].key)
    {
        return Err(
            TerminalMachineEffectCatalogValidationError::NonCanonicalAlternatives(semantic),
        );
    }
    let expected_family = semantic.into();
    for alternative in &declaration.alternatives {
        if alternative.key.family != expected_family {
            return Err(
                TerminalMachineEffectCatalogValidationError::AlternativeFamilyMismatch(semantic),
            );
        }
        validate_applicability(constraint, alternative.applicability).map_err(|()| {
            TerminalMachineEffectCatalogValidationError::InvalidAlternativeApplicability(semantic)
        })?;
        match alternative.size {
            TerminalMachineSizeKnowledge::ExactBytes(0)
            | TerminalMachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 0, ..
            } => {
                return Err(
                    TerminalMachineEffectCatalogValidationError::InvalidSizeKnowledge(semantic),
                );
            }
            TerminalMachineSizeKnowledge::EncoderResolved {
                minimum_bytes,
                maximum_bytes: Some(maximum),
            } if maximum < minimum_bytes => {
                return Err(
                    TerminalMachineEffectCatalogValidationError::InvalidSizeKnowledge(semantic),
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_applicability(
    constraint: &RegisterInstructionConstraint,
    applicability: TerminalMachineAlternativeApplicability,
) -> Result<(), ()> {
    let operand = |number| {
        constraint
            .operands
            .iter()
            .find(|operand| operand.operand == number)
    };
    let reads = |access| {
        matches!(
            access,
            RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
        )
    };
    let writes = |access| {
        matches!(
            access,
            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
        )
    };
    match applicability {
        TerminalMachineAlternativeApplicability::Always => Ok(()),
        TerminalMachineAlternativeApplicability::ResultAliasesOperand {
            result,
            operand: input,
        } => {
            let (Some(result), Some(input)) = (operand(result), operand(input)) else {
                return Err(());
            };
            (result.operand != input.operand
                && writes(result.access)
                && reads(input.access)
                && result.class == input.class)
                .then_some(())
                .ok_or(())
        }
        TerminalMachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result,
            aliased_operand,
            distinct_operand,
        } => {
            let (Some(result), Some(aliased), Some(distinct)) = (
                operand(result),
                operand(aliased_operand),
                operand(distinct_operand),
            ) else {
                return Err(());
            };
            (result.operand != aliased.operand
                && result.operand != distinct.operand
                && aliased.operand != distinct.operand
                && writes(result.access)
                && reads(aliased.access)
                && reads(distinct.access)
                && result.class == aliased.class
                && result.class == distinct.class)
                .then_some(())
                .ok_or(())
        }
        TerminalMachineAlternativeApplicability::ResultAliasesOperands {
            result,
            left,
            right,
        } => {
            let (Some(result), Some(left), Some(right)) =
                (operand(result), operand(left), operand(right))
            else {
                return Err(());
            };
            (result.operand != left.operand
                && result.operand != right.operand
                && left.operand != right.operand
                && writes(result.access)
                && reads(left.access)
                && reads(right.access)
                && result.class == left.class
                && result.class == right.class)
                .then_some(())
                .ok_or(())
        }
        TerminalMachineAlternativeApplicability::ResultDistinctFromOperands {
            result,
            left,
            right,
        } => {
            let (Some(result), Some(left), Some(right)) =
                (operand(result), operand(left), operand(right))
            else {
                return Err(());
            };
            (result.operand != left.operand
                && result.operand != right.operand
                && left.operand != right.operand
                && writes(result.access)
                && reads(left.access)
                && reads(right.access)
                && result.class == left.class
                && result.class == right.class)
                .then_some(())
                .ok_or(())
        }
    }
}

impl TerminalSelectedConstraintKeys {
    pub const fn in_identity_order(self) -> [RegisterConstraintKey; 8] {
        [
            self.materialize_i64,
            self.copy_i64,
            self.add_i64,
            self.add_i64_immediate,
            self.subtract_i64,
            self.compare_i64_zero,
            self.conditional_branch,
            self.return_i64,
        ]
    }

    pub const fn for_semantic(
        self,
        semantic: TerminalMachineSemanticKind,
    ) -> RegisterConstraintKey {
        match semantic {
            TerminalMachineSemanticKind::CompareI64Zero => self.compare_i64_zero,
            TerminalMachineSemanticKind::MaterializeI64 => self.materialize_i64,
            TerminalMachineSemanticKind::CopyI64 => self.copy_i64,
            TerminalMachineSemanticKind::ExactAddI64 => self.add_i64,
            TerminalMachineSemanticKind::ExactAddI64Immediate => self.add_i64_immediate,
            TerminalMachineSemanticKind::ExactSubtractI64 => self.subtract_i64,
            TerminalMachineSemanticKind::ConditionalBranchNonZero => self.conditional_branch,
            TerminalMachineSemanticKind::ReturnI64 => self.return_i64,
        }
    }
}
