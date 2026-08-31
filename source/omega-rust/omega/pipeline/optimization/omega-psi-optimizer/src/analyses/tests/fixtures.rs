//! Typed fixtures shared by the analysis-family tests.

use std::collections::BTreeSet;

use omega_abstract_operations::{AbstractOperation as O, AbstractSuccessor, ValueBinding};
use omega_optimization_unit::{
    EffectLink, OptimizationBlock, OptimizationEdge, OptimizationFact, OptimizationNode,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, ValueDefinition,
    ValueDefinitionSite, recompute_psi_optimization_unit_identity,
};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, OperationId, ScalarType,
    ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

#[derive(Clone)]
pub(super) enum Terminator {
    Return,
    Crash,
    Jump(u64),
    Branch(u64, u64),
}

pub(super) fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero test identity")
}

pub(super) fn edge(raw: u64, target: u64) -> AbstractSuccessor {
    AbstractSuccessor {
        psi_edge: id(raw, EdgeId::new),
        target: id(target, BlockId::new),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

pub(super) fn operation(raw: u64, terminator: Terminator) -> O {
    match terminator {
        Terminator::Return => O::ReturnUnit {
            psi_edge: id(raw * 10 + 1, EdgeId::new),
            cleanup_actions: Vec::new(),
        },
        Terminator::Crash => O::Crash {
            psi_edge: id(raw * 10 + 1, EdgeId::new),
            cause: psi_terminal::CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        },
        Terminator::Jump(target) => O::Jump {
            psi_edge: id(raw * 10 + 1, EdgeId::new),
            target: id(target, BlockId::new),
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        Terminator::Branch(when_true, when_false) => O::Conditional {
            condition: id(raw * 10 + 2, ValueId::new),
            when_true: edge(raw * 10 + 3, when_true),
            when_false: edge(raw * 10 + 4, when_false),
        },
    }
}

pub(super) fn node(operation: O) -> OptimizationNode {
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

pub(super) fn function(
    machine: u64,
    entry: u64,
    blocks: Vec<(u64, Terminator)>,
) -> PsiOptimizationFunction {
    PsiOptimizationFunction {
        machine: id(machine, MachineId::new),
        attachment: None,
        entry: id(entry, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        structural_places: Vec::new(),
        result: omega_abstract_operations::AbstractFunctionResult::Unit,
        declared_places: BTreeSet::new(),
        entry_claim_declarations: Vec::new(),
        content_entry_claims: Vec::new(),
        verified_contract: None,
        evidence_contract_lanes: Vec::new(),
        entry_claims: BTreeSet::new(),
        published_service_ceiling: Vec::new(),
        facts: Vec::new(),
        blocks: blocks
            .into_iter()
            .map(|(block, terminator)| OptimizationBlock {
                id: id(block, BlockId::new),
                parameters: Vec::new(),
                nodes: vec![node(operation(block, terminator))],
            })
            .collect(),
    }
}

pub(super) fn unit(
    functions: Vec<PsiOptimizationFunction>,
    _revision: &[u8],
) -> PsiOptimizationUnit {
    let mut unit = PsiOptimizationUnit {
        identity: omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(
            b"pending test content",
        ),
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        entry: functions[0].machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new().into(),
        services: Vec::new().into(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        accepted_obligation_facts: Vec::new(),
        proof_questions: Vec::new(),
        ownership_frontier_facts: Vec::new(),
        pruned_machines: Vec::new(),
        functions,
    };
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    unit
}

pub(super) fn block_parameter_constant_unit(
    known_condition: Option<bool>,
    right_constant: IntegerValue,
) -> (PsiOptimizationUnit, ValueId, [OperationId; 3], [EdgeId; 4]) {
    let mut function = function(
        100,
        1,
        vec![
            (1, Terminator::Branch(2, 3)),
            (2, Terminator::Jump(4)),
            (3, Terminator::Jump(4)),
            (4, Terminator::Return),
        ],
    );
    let condition = id(12, ValueId::new);
    let left = id(70, ValueId::new);
    let right = id(71, ValueId::new);
    let parameter = id(72, ValueId::new);
    let scalar_type = ScalarType::Integer(
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
    );
    function.parameters = vec![ValueDefinition {
        value: condition,
        scalar_type: ScalarType::Boolean,
        site: ValueDefinitionSite::FunctionParameter(0),
    }];
    function.blocks[3].parameters = vec![ValueDefinition {
        value: parameter,
        scalar_type,
        site: ValueDefinitionSite::BlockParameter {
            block: id(4, BlockId::new),
            position: 0,
        },
    }];
    let true_edge = id(13, EdgeId::new);
    let false_edge = id(14, EdgeId::new);
    function.blocks[0].nodes[0].successors = vec![
        OptimizationEdge {
            psi_edge: true_edge,
            target: id(2, BlockId::new),
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
            provenance: vec![PsiProvenance::Edge(true_edge)],
            fuel: vec![omega_optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(true_edge),
                units: 1,
            }],
        },
        OptimizationEdge {
            psi_edge: false_edge,
            target: id(3, BlockId::new),
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
            provenance: vec![PsiProvenance::Edge(false_edge)],
            fuel: vec![omega_optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(false_edge),
                units: 1,
            }],
        },
    ];
    let left_edge = id(21, EdgeId::new);
    let right_edge = id(31, EdgeId::new);
    for (block_index, edge, argument) in [(1, left_edge, left), (2, right_edge, right)] {
        let binding = ValueBinding {
            parameter,
            argument,
            scalar_type,
        };
        let O::Jump { bindings, .. } = &mut function.blocks[block_index].nodes[0].operation else {
            unreachable!()
        };
        *bindings = vec![binding];
        function.blocks[block_index].nodes[0].successors = vec![OptimizationEdge {
            psi_edge: edge,
            target: id(4, BlockId::new),
            bindings: vec![binding],
            trivial_affine_discards: Vec::new(),
            provenance: vec![PsiProvenance::Edge(edge)],
            fuel: vec![omega_optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(edge),
                units: 1,
            }],
        }];
    }
    let condition_support = id(600, OperationId::new);
    let left_support = id(601, OperationId::new);
    let right_support = id(602, OperationId::new);
    function.facts = vec![
        OptimizationFact::IntegerConstant {
            value: left,
            constant: IntegerValue::Unsigned(7),
            support: left_support,
        },
        OptimizationFact::IntegerConstant {
            value: right,
            constant: right_constant,
            support: right_support,
        },
    ];
    if let Some(condition) = known_condition {
        function.facts.push(OptimizationFact::BooleanConstant {
            value: id(12, ValueId::new),
            constant: condition,
            support: condition_support,
        });
    }
    (
        unit(vec![function], b"block-parameter-constants"),
        parameter,
        [condition_support, left_support, right_support],
        [true_edge, false_edge, left_edge, right_edge],
    )
}
