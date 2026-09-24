//! Backend vocabulary audit pin: every `AbstractOperation` family that has no
//! legalized row must classify as `NodeRejection::UnsupportedFamily` at
//! admission, and `validate` must surface that classification as the named
//! `LegalizationError::UnsupportedScalarOperation` diagnostic — never a panic
//! and never the custody-defect class. The complement holds too: a listed
//! non-scalar family passes admission through without an instruction row.

use super::nodes::{NodeRejection, admit, validate};
use super::{AbstractOperation, PsiOptimizationFunction};
use crate::LegalizationError;
use abstract_operations::AbstractFunctionResult;
use optimization_unit::{EffectLink, OptimizationBlock, OptimizationNode};
use semantic_vocabulary::{
    BlockId, EdgeId, IeeeFloatFormat, MachineId, OperationId, ServiceId, ValueId,
};
use std::collections::BTreeSet;

fn node(operation: AbstractOperation) -> OptimizationNode {
    OptimizationNode {
        operation,
        provenance: Vec::new(),
        fuel: Vec::new(),
        effect: EffectLink {
            input: 0,
            output: 1,
        },
        definitions: Vec::new(),
        uses: Vec::new(),
        successors: Vec::new(),
        ownership: Vec::new(),
    }
}

fn function(machine: u64) -> PsiOptimizationFunction {
    PsiOptimizationFunction {
        machine: MachineId::new(machine).expect("machine id"),
        attachment: None,
        entry: BlockId::new(1).expect("entry block"),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        structural_places: Vec::new(),
        result: AbstractFunctionResult::Unit,
        declared_places: BTreeSet::new(),
        entry_claim_declarations: Vec::new(),
        content_entry_claims: Vec::new(),
        verified_contract: None,
        evidence_contract_lanes: Vec::new(),
        entry_claims: BTreeSet::new(),
        published_service_ceiling: Vec::new(),
        declared_service_reach: Vec::new(),
        facts: Vec::new(),
        blocks: Vec::new(),
    }
}

#[test]
fn refused_families_classify_as_unsupported_family() {
    let port_write = node(AbstractOperation::PortWrite {
        psi_operation: OperationId::new(1).expect("operation"),
        service: ServiceId::new(2).expect("service"),
        port: 1,
        value: 2,
    });
    assert!(matches!(
        admit(&port_write),
        Err(NodeRejection::UnsupportedFamily)
    ));

    // FMA now has a target-unit ingest arm (control_flow/sources.rs and
    // unit/ieee_float.rs); the abstract node still classifies here as an
    // unsupported scalar-graph family rather than panicking, because the
    // graph admits only settled target-unit forms, not abstract FMA.
    let fused_multiply_add = node(AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: OperationId::new(3).expect("operation"),
        result: ValueId::new(4).expect("result"),
        format: IeeeFloatFormat::Binary64,
        left: ValueId::new(5).expect("left"),
        right: ValueId::new(6).expect("right"),
        addend: ValueId::new(7).expect("addend"),
    });
    assert!(matches!(
        admit(&fused_multiply_add),
        Err(NodeRejection::UnsupportedFamily)
    ));
}

#[test]
fn unsupported_family_surfaces_as_named_diagnostic() {
    let machine = 8;
    let optimized = function(machine);
    let block = OptimizationBlock {
        id: BlockId::new(1).expect("block"),
        structural_parameters: Vec::new(),
        parameters: Vec::new(),
        nodes: vec![
            node(AbstractOperation::PortWrite {
                psi_operation: OperationId::new(9).expect("operation"),
                service: ServiceId::new(10).expect("service"),
                port: 1,
                value: 2,
            }),
            node(AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(11).expect("edge"),
                cleanup_actions: Vec::new(),
            }),
        ],
    };
    let error =
        validate(&block, &optimized).expect_err("an unlisted family must refuse, not validate");
    let LegalizationError::UnsupportedScalarOperation {
        machine: reported,
        operation,
    } = error
    else {
        panic!("expected UnsupportedScalarOperation, got {error:?}");
    };
    assert_eq!(reported, optimized.machine);
    assert!(matches!(operation, AbstractOperation::PortWrite { .. }));
}

/// Backend vocabulary audit pin: every `AbstractOperation` family takes one
/// classified admission route, and the classification cannot drift silently.
///
/// The match below has no wildcard arm, so adding a family to
/// `AbstractOperation` stops this file compiling until someone states which
/// route it takes. That is the "no silent omission" half of this audit. The
/// diagnostic-quality half -- selection refusals carry no operation or target
/// identity -- is recorded in
/// `wiki/drafts/backend_vocabulary_rejection_audit.md`.
#[derive(Debug, PartialEq, Eq)]
enum AdmissionRoute {
    /// Reaches `admit` at body position and yields an operation identity.
    BodyAdmitted,
    /// Split off by `validate` as the block terminator and checked by
    /// `control::validate`; it never reaches `admit`.
    Terminator,
    /// Falls to `scalar_instruction`'s catch-all and surfaces as the named
    /// `LegalizationError::UnsupportedScalarOperation`.
    NamedRejection,
}

fn admission_route(operation: &AbstractOperation) -> AdmissionRoute {
    match operation {
        AbstractOperation::BooleanConstant { .. }
        | AbstractOperation::BooleanEqual { .. }
        | AbstractOperation::BooleanNot { .. }
        | AbstractOperation::BooleanStructuralField { .. }
        | AbstractOperation::BoundaryCall { .. }
        | AbstractOperation::ByteSequenceLength { .. }
        | AbstractOperation::ByteSequenceRead { .. }
        | AbstractOperation::ByteSequenceSubslice { .. }
        | AbstractOperation::ByteSequenceWrite { .. }
        | AbstractOperation::Call { .. }
        | AbstractOperation::CallDynamicParameterScalar { .. }
        | AbstractOperation::CallDynamicParameterUnit { .. }
        | AbstractOperation::CallStructural { .. }
        | AbstractOperation::CallStructuralScalar { .. }
        | AbstractOperation::CallUnit { .. }
        | AbstractOperation::DynamicDescriptorParameter { .. }
        | AbstractOperation::ElementViewLength { .. }
        | AbstractOperation::ElementViewRead { .. }
        | AbstractOperation::ElementViewSubslice { .. }
        | AbstractOperation::EstablishByteSequenceLiteral { .. }
        | AbstractOperation::EstablishElementView { .. }
        | AbstractOperation::EstablishPrimitiveLocal { .. }
        | AbstractOperation::EstablishRecord { .. }
        | AbstractOperation::EstablishReference { .. }
        | AbstractOperation::EstablishScalarArray { .. }
        | AbstractOperation::EstablishScalarCase { .. }
        | AbstractOperation::ExactIntegerAdd { .. }
        | AbstractOperation::ExactIntegerDivide { .. }
        | AbstractOperation::ExactIntegerMultiply { .. }
        | AbstractOperation::ExactIntegerRemainder { .. }
        | AbstractOperation::ExactIntegerShiftLeft { .. }
        | AbstractOperation::ExactIntegerShiftRight { .. }
        | AbstractOperation::ExactIntegerSubtract { .. }
        | AbstractOperation::IeeeFloatCompare { .. }
        | AbstractOperation::IeeeFloatConstant { .. }
        | AbstractOperation::IntegerBitwiseAnd { .. }
        | AbstractOperation::IntegerBitwiseNot { .. }
        | AbstractOperation::IntegerBitwiseOr { .. }
        | AbstractOperation::IntegerBitwiseXor { .. }
        | AbstractOperation::IntegerConstant { .. }
        | AbstractOperation::IntegerEqual { .. }
        | AbstractOperation::IndexedPrimitiveRead { .. }
        | AbstractOperation::IntegerExactCast { .. }
        | AbstractOperation::IntegerLessOrEqual { .. }
        | AbstractOperation::IntegerLessThan { .. }
        | AbstractOperation::IntegerStructuralField { .. }
        | AbstractOperation::IntegerWiden { .. }
        | AbstractOperation::PrimitiveLocalStore { .. }
        | AbstractOperation::PrimitiveScalarRead { .. }
        | AbstractOperation::ReleaseReference { .. }
        | AbstractOperation::SaturatingIntegerAdd { .. }
        | AbstractOperation::SaturatingIntegerDivide { .. }
        | AbstractOperation::SaturatingIntegerMultiply { .. }
        | AbstractOperation::SaturatingIntegerRemainder { .. }
        | AbstractOperation::SaturatingIntegerSubtract { .. }
        | AbstractOperation::StructuralByteSequenceFieldByteStore { .. }
        | AbstractOperation::StructuralByteSequenceFieldLength { .. }
        | AbstractOperation::StructuralByteSequenceFieldStore { .. }
        | AbstractOperation::StructuralCaseMembership { .. }
        | AbstractOperation::StructuralLeafCopy { .. }
        | AbstractOperation::StructuralScalarFieldStore { .. }
        | AbstractOperation::TrappingInteger { .. }
        | AbstractOperation::WrappingIntegerAdd { .. }
        | AbstractOperation::WrappingIntegerDivide { .. }
        | AbstractOperation::WrappingIntegerMultiply { .. }
        | AbstractOperation::WrappingIntegerRemainder { .. }
        | AbstractOperation::WrappingIntegerShiftLeft { .. }
        | AbstractOperation::WrappingIntegerShiftRight { .. }
        | AbstractOperation::WrappingIntegerSubtract { .. }
        | AbstractOperation::WriteOnlyIndexedPrimitiveStore { .. }
        | AbstractOperation::WriteOnlyPrimitiveStore { .. } => AdmissionRoute::BodyAdmitted,
        AbstractOperation::Conditional { .. }
        | AbstractOperation::Crash { .. }
        | AbstractOperation::Jump { .. }
        | AbstractOperation::Return { .. }
        | AbstractOperation::ReturnStructural { .. }
        | AbstractOperation::ReturnUnit { .. }
        | AbstractOperation::StructuralCase { .. } => AdmissionRoute::Terminator,
        AbstractOperation::AtomicEvent { .. }
        | AbstractOperation::CallDynamicScalar { .. }
        | AbstractOperation::CallDynamicUnit { .. }
        | AbstractOperation::CallStoredDynamicScalar { .. }
        | AbstractOperation::CallStructuralScalarWithDynamicArguments { .. }
        | AbstractOperation::CallUnitWithDynamicArguments { .. }
        | AbstractOperation::EstablishTrivialAffineLocal { .. }
        | AbstractOperation::MoveStructuralField { .. }
        | AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { .. }
        | AbstractOperation::PortWrite { .. }
        | AbstractOperation::StoreDynamicDescriptor { .. }
        | AbstractOperation::StoreStructuralField { .. } => AdmissionRoute::NamedRejection,
    }
}

#[test]
fn the_classified_route_agrees_with_admission() {
    // The exhaustive match is the pin: a new family stops this file
    // compiling. This checks the classification is not merely well formed but
    // agrees with what `admit` actually does for the two families the audit
    // routes to owners.
    let port_write = AbstractOperation::PortWrite {
        psi_operation: OperationId::new(1).expect("operation"),
        service: ServiceId::new(2).expect("service"),
        port: 1,
        value: 2,
    };
    assert_eq!(admission_route(&port_write), AdmissionRoute::NamedRejection);
    assert!(matches!(
        admit(&node(port_write)),
        Err(NodeRejection::UnsupportedFamily)
    ));

    let fused_multiply_add = AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: OperationId::new(3).expect("operation"),
        result: ValueId::new(4).expect("result"),
        format: IeeeFloatFormat::Binary64,
        left: ValueId::new(5).expect("left"),
        right: ValueId::new(6).expect("right"),
        addend: ValueId::new(7).expect("addend"),
    };
    assert_eq!(
        admission_route(&fused_multiply_add),
        AdmissionRoute::NamedRejection,
    );
    assert!(matches!(
        admit(&node(fused_multiply_add)),
        Err(NodeRejection::UnsupportedFamily)
    ));
}
