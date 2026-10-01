//! Independent checks for `ScalarRepresentationView`.
//!
//! The operation carries only its operand and an optional landing
//! obligation. The carrier pair is rederived here from the operand's and the
//! result's declarations through the closed table in
//! `terminal_semantics::ScalarRepresentationViewPair`: equal width and a
//! family change (IEEE float and integer, Boolean and byte), nothing else. A
//! producer cannot name the pair, so it cannot widen it.
//!
//! The landing obligation must be present exactly when the pair lands an
//! integer on `bool`. Its proposition is not encoded either: reconstruction
//! (`operation_facts.rs`) rebuilds `0 <= operand <= 1` from the operand's
//! declared carrier, and the proof bundle must discharge that goal from the
//! path's facts (a bounded field, a data `where` row, a guard) or the module
//! fails verification. An omitted obligation on a landing, or a supplied one
//! on a total pair, rejects before reconstruction.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{ScalarType, ValueId};
use terminal_psi::{Operation, OperationKind};
use terminal_semantics::ScalarRepresentationViewPair;

use super::ModuleError;
use super::operations::require_defined;

/// The admitted pair of one well-formed re-view. Validation has already
/// registered the operand and result types this reads.
pub(crate) fn scalar_representation_view_pair(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<ScalarRepresentationViewPair, ModuleError> {
    let OperationKind::ScalarRepresentationView {
        operand,
        landing_obligation,
    } = operation.kind
    else {
        unreachable!("dispatched a scalar representation view")
    };
    let source = value_types
        .get(&operand)
        .copied()
        .ok_or(ModuleError::UnknownValue(operand))?;
    let target = operation
        .result
        .scalar_ref()
        .map(|result| result.scalar_type)
        .ok_or(ModuleError::ScalarOperationHasUnitResult(operation.id))?;
    let pair = ScalarRepresentationViewPair::classify(source, target).ok_or(
        ModuleError::ScalarRepresentationViewPairMismatch {
            operation: operation.id,
            source,
            target,
        },
    )?;
    if pair.requires_landing() != landing_obligation.is_some() {
        return Err(ModuleError::ScalarRepresentationViewLandingMismatch {
            operation: operation.id,
            source,
            target,
        });
    }
    Ok(pair)
}

pub(super) fn validate_operands(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ScalarRepresentationView { operand, .. } = operation.kind else {
        unreachable!("dispatched a scalar representation view")
    };
    require_defined(operand, value_types, defined)?;
    scalar_representation_view_pair(operation, value_types).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId, OperationId};
    use terminal_psi::{OperationResult, ValueDeclaration};

    #[test]
    fn byte_boolean_landing_and_carrier_changes_are_independently_checked() {
        let operand = ValueId::new(1).unwrap();
        let byte = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let mut types = BTreeMap::from([(operand, byte)]);
        let mut operation = Operation {
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(2).unwrap(),
                scalar_type: ScalarType::Boolean,
                qualifications: Default::default(),
            }),
            static_reach_binding: None,
            suspension_crossing: None,
            kind: OperationKind::ScalarRepresentationView {
                operand,
                landing_obligation: Some(ObligationId::new(2).unwrap()),
            },
        };
        let defined = BTreeSet::from([operand]);
        assert!(validate_operands(&operation, &types, &defined).is_ok());
        assert!(validate_operands(&operation, &types, &BTreeSet::new()).is_err());
        operation.kind = OperationKind::ScalarRepresentationView {
            operand,
            landing_obligation: None,
        };
        assert!(matches!(
            scalar_representation_view_pair(&operation, &types),
            Err(ModuleError::ScalarRepresentationViewLandingMismatch { .. })
        ));
        types.insert(
            operand,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
        );
        assert!(matches!(
            scalar_representation_view_pair(&operation, &types),
            Err(ModuleError::ScalarRepresentationViewPairMismatch { .. })
        ));
    }

    #[test]
    fn a_landing_identity_without_evidence_cannot_turn_two_into_true() {
        use semantic_vocabulary::{BlockId, ContractId, EdgeId, IntegerValue, MachineId};
        use terminal_psi::{
            Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule,
            Terminator,
        };
        let machine = MachineId::new(1).unwrap();
        let operand = ValueId::new(1).unwrap();
        let landing = ObligationId::new(2).unwrap();
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let byte = ScalarType::Integer(integer);
        let mut module = TerminalModule::for_entry(machine);
        module.machines.push(TerminalMachine {
            id: machine,
            attachment: None,
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: vec![Block {
                id: BlockId::new(1).unwrap(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(1).unwrap(),
                        static_reach_binding: None,
                        suspension_crossing: None,
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: operand,
                            scalar_type: byte,
                            qualifications: Default::default(),
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(2),
                        },
                    },
                    Operation {
                        id: OperationId::new(2).unwrap(),
                        static_reach_binding: None,
                        suspension_crossing: None,
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: ValueId::new(2).unwrap(),
                            scalar_type: ScalarType::Boolean,
                            qualifications: Default::default(),
                        }),
                        kind: OperationKind::ScalarRepresentationView {
                            operand,
                            landing_obligation: Some(landing),
                        },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        });
        crate::validate_module(&module).expect("a landing ID is structurally well-formed");
        let obligations = crate::reconstruct_operation_obligations(&module).unwrap();
        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].obligation.id, landing);
        assert_eq!(
            obligations[0].obligation.proposition,
            ScalarRepresentationViewPair::ByteBoolean { integer }
                .landing_goal(operand)
                .unwrap()
        );
        assert!(
            matches!(crate::verify_module(&module, &crate::ProofBundle::default(),
            &proof_admission::AdmissionProfile::default()),
            Err(crate::VerificationError::MissingEvidence(id)) if id == landing)
        );
    }
}
