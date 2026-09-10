//! Input correspondence controls; selection remains a separate admission boundary.

use super::*;
mod legalized;
use semantic_vocabulary::{EdgeId, OperationId, PlaceId, StructuralCaseId, StructuralFieldId};

use crate::tests::legalization::structural_case::fixture;

#[test]
fn structural_case_graph_replays_exact_payloads_and_cleanup() {
    for native in [
        ::target::NativeTarget::linux_x64(),
        ::target::NativeTarget::linux_arm64(),
    ] {
        let (plan, target, unit) = fixture(native);
        super::super::validate_target(
            &target.functions[0],
            &plan.functions[0],
            &unit.functions[0],
            &target,
            &plan,
            &unit,
        )
        .unwrap();
        let legal = crate::legalize_target_operations(&target, &plan, &unit).unwrap();
        crate::validate_legalized_operations(&target, &plan, &unit, legal.plan().clone()).unwrap();
        let function = &legal.plan().scalar_functions[0];
        assert_eq!(
            function
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            unit.functions[0]
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>()
        );
        let legalized_operations::LegalizedScalarTerminator::StructuralCase {
            cases,
            source: subject,
            ..
        } = &function.blocks[0].terminator
        else {
            panic!("case")
        };
        assert_eq!(subject.place(), PlaceId::new(1).unwrap());
        assert_eq!(cases[0].target, BlockId::new(20).unwrap());
        assert_eq!(cases[1].target, BlockId::new(30).unwrap());
        assert_eq!(
            cases[1].payloads[0].parameter.definition_site,
            optimization_unit::ValueDefinitionSite::BlockParameter {
                block: BlockId::new(30).unwrap(),
                position: 0
            }
        );
        assert_ne!(
            crate::legalization_validator_identity(),
            optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.terminal-target-legalization-independent-replay.v37"
            )
        );
    }
}

#[test]
fn structural_case_graph_rejects_substituted_target_custody() {
    let (plan, target, unit) = fixture(::target::NativeTarget::linux_x64());
    for mutation in [
        "edge",
        "case",
        "tag",
        "roster",
        "target",
        "parameter block",
        "parameter value",
        "parameter type",
        "field",
        "offset",
        "producer",
        "place",
        "layout",
        "lost cleanup",
        "duplicate cleanup",
        "missing payload",
        "arm producer",
    ] {
        let mut changed = target.clone();
        let TargetOperation::ControlGraph(graph) = &mut changed.functions[0].operation else {
            panic!("graph")
        };
        let join = graph.blocks[2].block;
        let TargetControlTerminator::StructuralCase { source, cases } =
            &mut graph.blocks[0].terminator
        else {
            panic!("case")
        };
        match mutation {
            "edge" => cases[0].psi_edge = EdgeId::new(99).unwrap(),
            "case" => cases[0].case = StructuralCaseId::new(99).unwrap(),
            "tag" => cases[0].case_tag = 1,
            "roster" => cases.swap(0, 1),
            "target" => cases[0].target = cases[1].target,
            "parameter block" => cases[1].payloads[0].parameter.block = join,
            "parameter value" => cases[1].payloads[0].parameter.value = ValueId::new(99).unwrap(),
            "parameter type" => {
                cases[1].payloads[0].parameter.scalar_type = ScalarType::Integer(u32_type())
            }
            "field" => cases[1].payloads[0].field = StructuralFieldId::new(99).unwrap(),
            "offset" => cases[1].payloads[0].field_byte_offset = 0,
            "producer" => {
                let target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation,
                    ..
                } = &mut source.origin
                else {
                    panic!("operation");
                };
                *operation = OperationId::new(99).unwrap();
            }
            "place" => {
                let target_operations::TargetStructuralHomeOrigin::OperationResult {
                    result, ..
                } = &mut source.origin
                else {
                    panic!("operation");
                };
                result.place = PlaceId::new(99).unwrap();
            }
            "layout" => {
                let target_operations::TargetStructuralHomeLayout::Sum(layout) = &mut source.layout
                else {
                    panic!("sum")
                };
                layout.payload_byte_offset = 0;
            }
            "lost cleanup" => cases[0].trivial_affine_discards.clear(),
            "duplicate cleanup" => cases[0]
                .trivial_affine_discards
                .push(PlaceId::new(1).unwrap()),
            "missing payload" => cases[1].payloads.clear(),
            "arm producer" => {
                let producer = graph.blocks[0].operations.remove(0);
                graph.blocks[1].operations.push(producer);
            }
            _ => unreachable!(),
        }
        assert!(
            super::super::validate_target(
                &changed.functions[0],
                &plan.functions[0],
                &unit.functions[0],
                &changed,
                &plan,
                &unit
            )
            .is_err(),
            "accepted {mutation}"
        );
    }
}
