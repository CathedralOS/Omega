//! Exact call-composition policy rows.

use psi_terminal::OperationKind;

use super::{OperationSemanticError, OperationSemanticTag};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallTargetRule {
    ExactModuleMachine,
    DynamicDescriptorTable,
    ExactBoundaryMachine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallResultRule {
    ScalarCalleeResult,
    UnitCalleeResult,
    StructuralCalleeResult,
    BoundaryDeclaredResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallArgumentRule {
    ScalarPositionalArguments,
    StructuralPositionalArguments,
    DynamicDescriptorSource,
    BoundaryScalarAndStructuralPositionalArguments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallRequirementRule {
    EnumerateScalarRequires,
    EnumerateStructuralRequires,
    ValidateBoundaryRequirements,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallTransferRule {
    NoStructuralTransfer,
    ExactClaimTransfers,
    ExactClaimAndStructuralResultTransfers,
    ExactCompletionReceipts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallOutcomeRule {
    ImportScalarEnsures,
    ImportStructuralEnsures,
    BoundaryDeclaredCompletion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallCrashRule {
    ComposeScalarCrashRoutes,
    ComposeStructuralCrashRoutes,
    BoundaryHasNoCallRoutes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallEvidenceRule {
    SameUnitOrCertifiedScalarEvidence,
    SameUnitOrCertifiedStructuralEvidence,
    ExactBoundaryDeclarationEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallFuelPolicy {
    ConsumeOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallFrontierRule {
    KeepsScalarFrontier,
    TransfersStructuralFrontier,
    SettlesBoundaryFrontier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CallCompositionSchema {
    target: CallTargetRule,
    result: CallResultRule,
    arguments: CallArgumentRule,
    requirements: CallRequirementRule,
    transfers: CallTransferRule,
    outcomes: CallOutcomeRule,
    crash: CallCrashRule,
    evidence: CallEvidenceRule,
    fuel: CallFuelPolicy,
    frontier: CallFrontierRule,
}

impl CallCompositionSchema {
    pub const fn target(self) -> CallTargetRule {
        self.target
    }

    pub const fn result(self) -> CallResultRule {
        self.result
    }

    pub const fn arguments(self) -> CallArgumentRule {
        self.arguments
    }

    pub const fn requirements(self) -> CallRequirementRule {
        self.requirements
    }

    pub const fn transfers(self) -> CallTransferRule {
        self.transfers
    }

    pub const fn outcomes(self) -> CallOutcomeRule {
        self.outcomes
    }

    pub const fn crash(self) -> CallCrashRule {
        self.crash
    }

    pub const fn evidence(self) -> CallEvidenceRule {
        self.evidence
    }

    pub const fn fuel(self) -> CallFuelPolicy {
        self.fuel
    }

    pub const fn frontier(self) -> CallFrontierRule {
        self.frontier
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallCompositionSemanticRow {
    tag: OperationSemanticTag,
    schema: CallCompositionSchema,
}

impl CallCompositionSemanticRow {
    pub const ALL: [Self; 6] = [
        Self {
            tag: OperationSemanticTag::Call,
            schema: CallCompositionSchema {
                target: CallTargetRule::ExactModuleMachine,
                result: CallResultRule::ScalarCalleeResult,
                arguments: CallArgumentRule::ScalarPositionalArguments,
                requirements: CallRequirementRule::EnumerateScalarRequires,
                transfers: CallTransferRule::NoStructuralTransfer,
                outcomes: CallOutcomeRule::ImportScalarEnsures,
                crash: CallCrashRule::ComposeScalarCrashRoutes,
                evidence: CallEvidenceRule::SameUnitOrCertifiedScalarEvidence,
                fuel: CallFuelPolicy::ConsumeOne,
                frontier: CallFrontierRule::KeepsScalarFrontier,
            },
        },
        Self {
            tag: OperationSemanticTag::CallUnit,
            schema: CallCompositionSchema {
                target: CallTargetRule::ExactModuleMachine,
                result: CallResultRule::UnitCalleeResult,
                arguments: CallArgumentRule::StructuralPositionalArguments,
                requirements: CallRequirementRule::EnumerateStructuralRequires,
                transfers: CallTransferRule::ExactClaimTransfers,
                outcomes: CallOutcomeRule::ImportStructuralEnsures,
                crash: CallCrashRule::ComposeStructuralCrashRoutes,
                evidence: CallEvidenceRule::SameUnitOrCertifiedStructuralEvidence,
                fuel: CallFuelPolicy::ConsumeOne,
                frontier: CallFrontierRule::TransfersStructuralFrontier,
            },
        },
        Self {
            tag: OperationSemanticTag::CallStructuralScalar,
            schema: CallCompositionSchema {
                target: CallTargetRule::ExactModuleMachine,
                result: CallResultRule::ScalarCalleeResult,
                arguments: CallArgumentRule::StructuralPositionalArguments,
                requirements: CallRequirementRule::EnumerateStructuralRequires,
                transfers: CallTransferRule::ExactClaimTransfers,
                outcomes: CallOutcomeRule::ImportStructuralEnsures,
                crash: CallCrashRule::ComposeStructuralCrashRoutes,
                evidence: CallEvidenceRule::SameUnitOrCertifiedStructuralEvidence,
                fuel: CallFuelPolicy::ConsumeOne,
                frontier: CallFrontierRule::TransfersStructuralFrontier,
            },
        },
        Self {
            tag: OperationSemanticTag::CallDynamicScalar,
            schema: CallCompositionSchema {
                target: CallTargetRule::DynamicDescriptorTable,
                result: CallResultRule::ScalarCalleeResult,
                arguments: CallArgumentRule::DynamicDescriptorSource,
                requirements: CallRequirementRule::EnumerateStructuralRequires,
                transfers: CallTransferRule::NoStructuralTransfer,
                outcomes: CallOutcomeRule::ImportStructuralEnsures,
                crash: CallCrashRule::ComposeStructuralCrashRoutes,
                evidence: CallEvidenceRule::SameUnitOrCertifiedStructuralEvidence,
                fuel: CallFuelPolicy::ConsumeOne,
                frontier: CallFrontierRule::TransfersStructuralFrontier,
            },
        },
        Self {
            tag: OperationSemanticTag::CallStructural,
            schema: CallCompositionSchema {
                target: CallTargetRule::ExactModuleMachine,
                result: CallResultRule::StructuralCalleeResult,
                arguments: CallArgumentRule::StructuralPositionalArguments,
                requirements: CallRequirementRule::EnumerateStructuralRequires,
                transfers: CallTransferRule::ExactClaimAndStructuralResultTransfers,
                outcomes: CallOutcomeRule::ImportStructuralEnsures,
                crash: CallCrashRule::ComposeStructuralCrashRoutes,
                evidence: CallEvidenceRule::SameUnitOrCertifiedStructuralEvidence,
                fuel: CallFuelPolicy::ConsumeOne,
                frontier: CallFrontierRule::TransfersStructuralFrontier,
            },
        },
        Self {
            tag: OperationSemanticTag::BoundaryCall,
            schema: CallCompositionSchema {
                target: CallTargetRule::ExactBoundaryMachine,
                result: CallResultRule::BoundaryDeclaredResult,
                arguments: CallArgumentRule::BoundaryScalarAndStructuralPositionalArguments,
                requirements: CallRequirementRule::ValidateBoundaryRequirements,
                transfers: CallTransferRule::ExactCompletionReceipts,
                outcomes: CallOutcomeRule::BoundaryDeclaredCompletion,
                crash: CallCrashRule::BoundaryHasNoCallRoutes,
                evidence: CallEvidenceRule::ExactBoundaryDeclarationEvidence,
                fuel: CallFuelPolicy::ConsumeOne,
                frontier: CallFrontierRule::SettlesBoundaryFrontier,
            },
        },
    ];

    pub const fn tag(self) -> OperationSemanticTag {
        self.tag
    }

    pub const fn schema(self) -> CallCompositionSchema {
        self.schema
    }
}

const fn is_call_composition_tag(tag: OperationSemanticTag) -> bool {
    matches!(
        tag,
        OperationSemanticTag::Call
            | OperationSemanticTag::CallUnit
            | OperationSemanticTag::CallStructuralScalar
            | OperationSemanticTag::CallDynamicScalar
            | OperationSemanticTag::CallStructural
            | OperationSemanticTag::BoundaryCall
    )
}

pub fn exact_call_composition_semantic_row_in(
    tag: OperationSemanticTag,
    rows: &[CallCompositionSemanticRow],
) -> Result<Option<&CallCompositionSemanticRow>, OperationSemanticError> {
    if !is_call_composition_tag(tag) {
        return Ok(None);
    }
    let mut matches = rows.iter().filter(|row| row.tag == tag);
    let row = matches
        .next()
        .ok_or(OperationSemanticError::MissingCallCompositionRow(tag))?;
    if matches.next().is_some() {
        return Err(OperationSemanticError::DuplicateCallCompositionRow(tag));
    }
    Ok(Some(row))
}

fn canonical_schema(tag: OperationSemanticTag) -> Option<CallCompositionSchema> {
    CallCompositionSemanticRow::ALL
        .iter()
        .find(|row| row.tag == tag)
        .map(|row| row.schema)
}

pub fn validate_call_composition_semantic_rows(
    rows: &[CallCompositionSemanticRow],
) -> Result<(), OperationSemanticError> {
    for row in rows {
        if !is_call_composition_tag(row.tag) {
            return Err(OperationSemanticError::UnexpectedCallCompositionRow(
                row.tag,
            ));
        }
    }
    for tag in [
        OperationSemanticTag::Call,
        OperationSemanticTag::CallUnit,
        OperationSemanticTag::CallStructuralScalar,
        OperationSemanticTag::CallStructural,
        OperationSemanticTag::BoundaryCall,
    ] {
        let row = exact_call_composition_semantic_row_in(tag, rows)?
            .expect("the requested tag belongs to call composition");
        if Some(row.schema) != canonical_schema(tag) {
            return Err(OperationSemanticError::CallCompositionSchemaMismatch(tag));
        }
    }
    Ok(())
}

pub fn call_composition_semantic_row(
    operation: &OperationKind,
) -> Result<Option<&'static CallCompositionSemanticRow>, OperationSemanticError> {
    validate_call_composition_semantic_rows(&CallCompositionSemanticRow::ALL)?;
    exact_call_composition_semantic_row_in(
        OperationSemanticTag::for_operation(operation),
        &CallCompositionSemanticRow::ALL,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use psi_core::{BoundaryMachineId, MachineId, ValueId};

    use super::*;

    fn scalar_call() -> OperationKind {
        OperationKind::Call {
            callee: MachineId::new(1).unwrap(),
            arguments: vec![ValueId::new(1).unwrap()],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }
    }

    fn structural_scalar_call() -> OperationKind {
        OperationKind::CallStructuralScalar {
            callee: MachineId::new(2).unwrap(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }
    }

    fn boundary_call() -> OperationKind {
        OperationKind::BoundaryCall {
            boundary: BoundaryMachineId::new(1).unwrap(),
            arguments: vec![ValueId::new(2).unwrap(), ValueId::new(1).unwrap()],
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        }
    }

    fn dynamic_scalar_call() -> OperationKind {
        OperationKind::CallDynamicScalar {
            descriptor_ordinal: 0,
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }
    }

    #[test]
    fn inventory_is_exact_unique_and_exposes_every_independent_axis() {
        assert_eq!(CallCompositionSemanticRow::ALL.len(), 6);
        assert_eq!(
            CallCompositionSemanticRow::ALL
                .iter()
                .map(|row| row.tag())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                OperationSemanticTag::Call,
                OperationSemanticTag::CallUnit,
                OperationSemanticTag::CallStructuralScalar,
                OperationSemanticTag::CallDynamicScalar,
                OperationSemanticTag::CallStructural,
                OperationSemanticTag::BoundaryCall,
            ]),
        );
        let scalar = call_composition_semantic_row(&scalar_call())
            .unwrap()
            .unwrap()
            .schema();
        assert_eq!(scalar.target(), CallTargetRule::ExactModuleMachine);
        assert_eq!(scalar.result(), CallResultRule::ScalarCalleeResult);
        assert_eq!(
            scalar.arguments(),
            CallArgumentRule::ScalarPositionalArguments,
        );
        assert_eq!(
            scalar.requirements(),
            CallRequirementRule::EnumerateScalarRequires,
        );
        assert_eq!(scalar.transfers(), CallTransferRule::NoStructuralTransfer);
        assert_eq!(scalar.outcomes(), CallOutcomeRule::ImportScalarEnsures);
        assert_eq!(scalar.crash(), CallCrashRule::ComposeScalarCrashRoutes);
        assert_eq!(
            scalar.evidence(),
            CallEvidenceRule::SameUnitOrCertifiedScalarEvidence,
        );
        assert_eq!(scalar.fuel(), CallFuelPolicy::ConsumeOne);
        assert_eq!(scalar.frontier(), CallFrontierRule::KeepsScalarFrontier);

        let structural_scalar = call_composition_semantic_row(&structural_scalar_call())
            .unwrap()
            .unwrap()
            .schema();
        assert_eq!(
            structural_scalar.result(),
            CallResultRule::ScalarCalleeResult
        );
        assert_eq!(
            structural_scalar.arguments(),
            CallArgumentRule::StructuralPositionalArguments
        );
        assert_eq!(
            structural_scalar.transfers(),
            CallTransferRule::ExactClaimTransfers
        );
        assert_eq!(
            structural_scalar.frontier(),
            CallFrontierRule::TransfersStructuralFrontier
        );

        let dynamic = call_composition_semantic_row(&dynamic_scalar_call())
            .unwrap()
            .unwrap()
            .schema();
        assert_eq!(dynamic.target(), CallTargetRule::DynamicDescriptorTable);
        assert_eq!(
            dynamic.arguments(),
            CallArgumentRule::DynamicDescriptorSource
        );

        let boundary = call_composition_semantic_row(&boundary_call())
            .unwrap()
            .unwrap()
            .schema();
        assert_eq!(
            boundary.arguments(),
            CallArgumentRule::BoundaryScalarAndStructuralPositionalArguments,
        );
        assert_eq!(
            boundary.transfers(),
            CallTransferRule::ExactCompletionReceipts
        );
    }

    #[test]
    fn lookup_rejects_missing_duplicate_cross_kind_and_axis_drift() {
        let tag = OperationSemanticTag::Call;
        let canonical =
            *exact_call_composition_semantic_row_in(tag, &CallCompositionSemanticRow::ALL)
                .unwrap()
                .unwrap();
        let missing = CallCompositionSemanticRow::ALL
            .iter()
            .copied()
            .filter(|row| row.tag != tag)
            .collect::<Vec<_>>();
        assert_eq!(
            validate_call_composition_semantic_rows(&missing),
            Err(OperationSemanticError::MissingCallCompositionRow(tag)),
        );
        let mut duplicate = CallCompositionSemanticRow::ALL.to_vec();
        duplicate.push(canonical);
        assert_eq!(
            validate_call_composition_semantic_rows(&duplicate),
            Err(OperationSemanticError::DuplicateCallCompositionRow(tag)),
        );

        let mut crossed = CallCompositionSemanticRow::ALL;
        crossed[0].schema = crossed[1].schema;
        assert_eq!(
            validate_call_composition_semantic_rows(&crossed),
            Err(OperationSemanticError::CallCompositionSchemaMismatch(tag)),
        );

        let mut weakened = CallCompositionSemanticRow::ALL;
        weakened[5].schema.frontier = CallFrontierRule::KeepsScalarFrontier;
        assert_eq!(
            validate_call_composition_semantic_rows(&weakened),
            Err(OperationSemanticError::CallCompositionSchemaMismatch(
                OperationSemanticTag::BoundaryCall,
            )),
        );
    }

    #[test]
    fn every_call_policy_axis_rejects_independent_drift() {
        let tag = OperationSemanticTag::Call;
        let canonical = CallCompositionSemanticRow::ALL[0].schema;
        let mut drifts = Vec::new();

        let mut drift = canonical;
        drift.target = CallTargetRule::ExactBoundaryMachine;
        drifts.push(drift);
        let mut drift = canonical;
        drift.result = CallResultRule::UnitCalleeResult;
        drifts.push(drift);
        let mut drift = canonical;
        drift.arguments = CallArgumentRule::StructuralPositionalArguments;
        drifts.push(drift);
        let mut drift = canonical;
        drift.requirements = CallRequirementRule::EnumerateStructuralRequires;
        drifts.push(drift);
        let mut drift = canonical;
        drift.transfers = CallTransferRule::ExactClaimTransfers;
        drifts.push(drift);
        let mut drift = canonical;
        drift.outcomes = CallOutcomeRule::ImportStructuralEnsures;
        drifts.push(drift);
        let mut drift = canonical;
        drift.crash = CallCrashRule::ComposeStructuralCrashRoutes;
        drifts.push(drift);
        let mut drift = canonical;
        drift.evidence = CallEvidenceRule::SameUnitOrCertifiedStructuralEvidence;
        drifts.push(drift);
        let mut drift = canonical;
        drift.frontier = CallFrontierRule::TransfersStructuralFrontier;
        drifts.push(drift);

        for schema in drifts {
            let mut rows = CallCompositionSemanticRow::ALL;
            rows[0].schema = schema;
            assert_eq!(
                validate_call_composition_semantic_rows(&rows),
                Err(OperationSemanticError::CallCompositionSchemaMismatch(tag)),
            );
        }
    }
}
