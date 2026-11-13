use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation as O,
    AbstractOperationPlan, AbstractParameter, AbstractResult, AbstractStructuralBinding,
    AbstractStructuralCaseSuccessor, AbstractSuccessor,
};
use optimization_unit::{
    PsiOptimizationUnit, PsiRewritePatch, reconstruct_psi_optimization_unit_seed,
};
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, OperationId, PlaceId, ScalarType, StructuralCaseId,
    StructuralTypeId, ValueId, WorkScheduleIdentity,
};
use terminal_psi::{
    SemanticFingerprint, StructuralAccess, StructuralArgument, StructuralCaseDeclaration,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalIdentityTableBuilder, TerminalPsiIdentity, VocabularyMarker,
};

use super::{block_components, dominators, graph::control_flow, loops};
use crate::PsiOptimizationRule;
use crate::analyses::AnalysisRevision;
use crate::rules::dead_scalar_elimination::DeadScalarLiteralEliminationRule;
use crate::semantic_analyses::{place_access_summaries, value_liveness};

fn block(value: u64) -> BlockId {
    BlockId::new(value).unwrap()
}

fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}

fn parameter(value: u64) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(value),
        position: 0,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }
}

fn case(edge: u64, case: u64, target: u64) -> AbstractStructuralCaseSuccessor {
    AbstractStructuralCaseSuccessor {
        psi_edge: EdgeId::new(edge).unwrap(),
        target: block(target),
        case: StructuralCaseId::new(case).unwrap(),
        payloads: Vec::new(),
        structural_bindings: Vec::new(),
        cleanup_actions: Vec::new(),
    }
}

// The computed scalar is used only after a case edge; another is truly dead.
// A nested case reads the sum through
// an edge-bound place, and block 5 is genuinely unreachable. Build the edges via
// the production representation constructor, not a hand-authored successor table.
fn case_plan() -> AbstractOperationPlan {
    let mut identities = TerminalIdentityTableBuilder::default();
    let sum_identity = identities.intern("test::Choice");
    let cases = ["First", "Second"]
        .into_iter()
        .enumerate()
        .map(|(position, name)| StructuralCaseDeclaration {
            id: StructuralCaseId::new(position as u64 + 1).unwrap(),
            identity: identities.intern(name),
            fields: Vec::new(),
        })
        .collect();
    let mut first = case(1, 1, 2);
    first.structural_bindings.push(AbstractStructuralBinding {
        parameter: place(2),
        argument: StructuralArgument::whole(place(1), StructuralAccess::SharedBorrow),
    });
    let returning = |edge, value| O::Return {
        psi_edge: EdgeId::new(edge).unwrap(),
        result: ValueId::new(3).unwrap(),
        value: ValueId::new(value).unwrap(),
        scalar_type: ScalarType::Boolean,
        cleanup_actions: Vec::new(),
    };
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        },
        entry: MachineId::new(1).unwrap(),
        structural_types: vec![StructuralTypeDeclaration {
            quotient_carrier: None,
            id: StructuralTypeId::new(1).unwrap(),
            identity: sum_identity,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            shape: StructuralTypeShape::Sum { cases },
            scalar_restriction: None,
            requires: Vec::new(),
        }]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        dynamic_dispatch: Default::default(),
        identities: identities.build(),
        functions: vec![AbstractFunction {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            nominal_cleanup_subject: None,
            entry: block(1),
            parameters: vec![AbstractParameter {
                value: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Boolean,
            }],
            structural_parameters: vec![parameter(1)],
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(3).unwrap(),
                scalar_type: ScalarType::Boolean,
            }),
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: (1..=5)
                .map(|position| AbstractBlockEntry {
                    block: block(position),
                    structural_parameters: if position == 2 {
                        vec![parameter(2)]
                    } else {
                        Vec::new()
                    },
                    parameters: Vec::new(),
                    operation_offset: if position == 1 {
                        0
                    } else {
                        position as usize + 1
                    },
                })
                .collect(),
            operations: vec![
                O::BooleanConstant {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: ValueId::new(2).unwrap(),
                    value: true,
                },
                O::BooleanConstant {
                    psi_operation: OperationId::new(2).unwrap(),
                    result: ValueId::new(4).unwrap(),
                    value: false,
                },
                O::StructuralCase {
                    source: place(1),
                    path: Vec::new(),
                    cases: vec![first, case(2, 2, 3)],
                },
                O::StructuralCase {
                    source: place(2),
                    path: Vec::new(),
                    cases: vec![case(3, 1, 4), case(4, 2, 4)],
                },
                returning(5, 2),
                returning(6, 2),
                returning(7, 1),
            ],
        }],
    }
}

fn unit(plan: &AbstractOperationPlan) -> PsiOptimizationUnit {
    reconstruct_psi_optimization_unit_seed(plan, WorkScheduleIdentity::new(1).unwrap()).unwrap()
}

#[test]
fn case_successors_drive_reachability_and_dominance() {
    let unit = unit(&case_plan());
    let graph = control_flow(&unit);
    let rows = &graph.functions[0].blocks;
    assert_eq!(rows[0].successors, [block(2), block(3)]);
    assert_eq!(rows[1].successors, [block(4)]);
    assert_eq!(rows[3].predecessors, [block(2)]);
    assert!(rows[..4].iter().all(|row| row.reachable));
    assert!(!rows[4].reachable);
    let machine = unit.entry;
    assert_eq!(
        dominators(&unit, false).rows(machine),
        [
            (block(1), vec![block(1)]),
            (block(2), vec![block(1), block(2)]),
            (block(3), vec![block(1), block(3)]),
            (block(4), vec![block(1), block(2), block(4)]),
        ]
    );
    assert_eq!(
        dominators(&unit, true).rows(machine),
        [
            (block(1), vec![block(1)]),
            (block(2), vec![block(2), block(4)]),
            (block(3), vec![block(3)]),
            (block(4), vec![block(4)]),
        ]
    );
}

#[test]
fn case_successors_keep_values_live() {
    let unit = unit(&case_plan());
    let liveness = value_liveness(&unit);
    assert_eq!(liveness.blocks[0].exit, [ValueId::new(2).unwrap()]);
    assert_eq!(liveness.blocks[1].exit, [ValueId::new(2).unwrap()]);
    assert_eq!(liveness.blocks[0].nodes[0].exit, [ValueId::new(2).unwrap()]);
    assert!(
        !liveness.blocks[0].nodes[1]
            .exit
            .contains(&ValueId::new(4).unwrap())
    );
    let mut analysis = AnalysisRevision::bind(&unit).unwrap();
    let required = DeadScalarLiteralEliminationRule::contract().required_analyses();
    let candidates = DeadScalarLiteralEliminationRule
        .propose(&unit, analysis.require_all(required).unwrap())
        .unwrap();
    assert_eq!(
        candidates.len(),
        1,
        "only the unused constant may be removed"
    );
    assert!(matches!(
        candidates[0].patch_ref(),
        PsiRewritePatch::RemoveDeadScalarNode(patch) if patch.result == ValueId::new(4).unwrap()
    ));
}

#[test]
fn jump_and_conditional_share_the_case_graph() {
    let case_unit = unit(&case_plan());
    let mut plan = case_plan();
    let successor = |edge: AbstractStructuralCaseSuccessor| AbstractSuccessor {
        psi_edge: edge.psi_edge,
        target: edge.target,
        bindings: Vec::new(),
        structural_bindings: edge.structural_bindings,
        cleanup_actions: edge.cleanup_actions,
    };
    let O::StructuralCase { cases, .. } = &plan.functions[0].operations[2] else {
        panic!("entry case");
    };
    plan.functions[0].operations[2] = O::Conditional {
        condition: ValueId::new(1).unwrap(),
        when_true: successor(cases[0].clone()),
        when_false: successor(cases[1].clone()),
    };
    plan.functions[0].operations[3] = O::Jump {
        psi_edge: EdgeId::new(3).unwrap(),
        target: block(4),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    let branch_unit = unit(&plan);
    assert_eq!(control_flow(&case_unit), control_flow(&branch_unit));
    assert_eq!(
        value_liveness(&branch_unit).blocks[0].exit,
        [ValueId::new(2).unwrap()]
    );
}

#[test]
fn case_successors_preserve_place_origins() {
    let unit = unit(&case_plan());
    let access = place_access_summaries(&unit);
    assert!(access.functions[0].external_reads.is_empty());
    assert_eq!(access.functions[0].parameters[0].reads, [Vec::new()]);
}

#[test]
fn case_backedge_is_a_loop_not_an_exit() {
    let mut plan = case_plan();
    let O::StructuralCase { cases, .. } = &mut plan.functions[0].operations[3] else {
        panic!("nested case");
    };
    cases[0].target = block(2);
    cases[0]
        .structural_bindings
        .push(AbstractStructuralBinding {
            parameter: place(2),
            argument: StructuralArgument::whole(place(2), StructuralAccess::SharedBorrow),
        });
    let unit = unit(&plan);
    assert!(
        block_components(&unit)
            .cyclic_machines(&unit)
            .contains(&unit.entry)
    );
    let regions = &loops(&unit).functions[0].1;
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].header, Some(block(2)));
    assert_eq!(regions[0].blocks, [block(2)]);
    assert!(!regions[0].irreducible);
}
