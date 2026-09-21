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

    // The audit's named hole class: FMA is ingest-refused upstream, but a
    // fabricated node reaching admission still classifies, never panics.
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
