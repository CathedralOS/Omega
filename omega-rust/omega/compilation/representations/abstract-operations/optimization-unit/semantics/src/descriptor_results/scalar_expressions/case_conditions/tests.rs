//! Case tests retain their evaluated subject only across unchanged SSA forwarding.

use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation as Operation,
    AbstractParameter, AbstractResult, AbstractStructuralCallResult, AbstractSuccessor,
    ValueBinding,
};
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, OperationId, PlaceId, ScalarType, StructuralCaseId,
    StructuralTypeId, ValueId,
};
use std::borrow::Cow;
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralCaseDeclaration, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralStoreDisposition, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalIdentityIndex,
};
use terminal_semantics::descriptor_lifetimes::DescriptorLocation;
use terminal_semantics::descriptor_results::{DescriptorCasePredicate, DescriptorMachine};

fn parameter(
    place: u64,
    position: u32,
    access: StructuralAccess,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: PlaceId::new(place).unwrap(),
        position,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn replacement(destination: u64, source: u64) -> Operation {
    Operation::StoreStructuralField {
        disposition: StructuralStoreDisposition::ReplaceDiscardable,
        psi_operation: OperationId::new(2).unwrap(),
        destination: parameter(destination, 0, StructuralAccess::MutableBorrow).into(),
        path: Vec::new(),
        field: None,
        value: StructuralArgument {
            place: PlaceId::new(source).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        },
    }
}

fn function(
    machine: u64,
    parameters: Vec<StructuralParameterDeclaration>,
    block_entries: Vec<AbstractBlockEntry>,
    operations: Vec<Operation>,
) -> AbstractFunction {
    AbstractFunction {
        machine: MachineId::new(machine).unwrap(),
        attachment: None,
        nominal_cleanup_subject: None,
        entry: block_entries[0].block,
        parameters: Vec::new(),
        structural_parameters: parameters,
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries,
        operations,
    }
}

fn forwarded_graph(intervening: Vec<Operation>, forwarded: u64) -> DescriptorMachine {
    let block = |number| BlockId::new(number).unwrap();
    let successor = |number| AbstractSuccessor {
        psi_edge: EdgeId::new(number).unwrap(),
        target: block(number + 1),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    let mut operations = vec![Operation::StructuralCaseMembership {
        psi_operation: OperationId::new(1).unwrap(),
        result: AbstractResult {
            value: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Boolean,
        },
        source: PlaceId::new(1).unwrap(),
        path: Vec::new(),
        case: StructuralCaseId::new(1).unwrap(),
    }];
    operations.extend(intervening);
    operations.push(Operation::Jump {
        psi_edge: EdgeId::new(1).unwrap(),
        target: block(2),
        bindings: vec![ValueBinding {
            parameter: ValueId::new(2).unwrap(),
            argument: ValueId::new(forwarded).unwrap(),
            scalar_type: ScalarType::Boolean,
        }],
        structural_bindings: Vec::new(),
        cleanup_actions: Vec::new(),
    });
    let conditional = operations.len();
    operations.push(Operation::Conditional {
        condition: ValueId::new(2).unwrap(),
        when_true: successor(2),
        when_false: successor(3),
    });
    operations.extend([4, 5].map(|number| Operation::ReturnUnit {
        psi_edge: EdgeId::new(number).unwrap(),
        cleanup_actions: Vec::new(),
    }));
    let entries = [0, conditional, conditional + 1, conditional + 2]
        .into_iter()
        .enumerate()
        .map(|(index, operation_offset)| AbstractBlockEntry {
            block: block(index as u64 + 1),
            structural_parameters: Vec::new(),
            parameters: if index == 1 {
                vec![AbstractParameter {
                    value: ValueId::new(2).unwrap(),
                    scalar_type: ScalarType::Boolean,
                }]
            } else {
                Vec::new()
            },
            operation_offset,
        })
        .collect();
    let caller = function(
        1,
        vec![
            parameter(1, 0, StructuralAccess::MutableBorrow),
            parameter(2, 1, StructuralAccess::Owned),
        ],
        entries,
        operations,
    );
    let callee = function(
        2,
        vec![
            parameter(3, 0, StructuralAccess::MutableBorrow),
            parameter(4, 1, StructuralAccess::Owned),
        ],
        vec![AbstractBlockEntry {
            block: block(5),
            structural_parameters: Vec::new(),
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        vec![
            replacement(3, 4),
            Operation::ReturnUnit {
                psi_edge: EdgeId::new(6).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ],
    );
    let functions = [caller, callee]
        .iter()
        .map(optimization_unit::reconstruct_psi_optimization_function_seed)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let types = [StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: TerminalIdentityIndex(1),
        shape: StructuralTypeShape::Sum {
            cases: [1, 2]
                .map(|number| StructuralCaseDeclaration {
                    id: StructuralCaseId::new(number).unwrap(),
                    identity: TerminalIdentityIndex(number as u32 + 1),
                    fields: Vec::new(),
                })
                .to_vec(),
        },
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: false,
        copyable: true,
        descriptor_cleanup: false,
        scalar_restriction: None,
    }];
    crate::descriptor_results::optimized_reference_catalog_graph(
        &functions,
        [],
        &types,
        &Default::default(),
        |_, node| Some(Cow::Borrowed(&node.operation)),
    )
    .unwrap()
    .remove(0)
}

fn expected_polarities(truth: bool) -> Vec<(DescriptorCasePredicate, bool)> {
    let predicate = DescriptorCasePredicate {
        subject: DescriptorLocation::whole(PlaceId::new(1).unwrap()),
        case: StructuralCaseId::new(1).unwrap(),
    };
    vec![(predicate.clone(), truth), (predicate, !truth)]
}

#[test]
fn forwarded_case_membership_preserves_both_edge_polarities() {
    let graph = forwarded_graph(Vec::new(), 1);
    assert_eq!(
        graph.case_successors.get(&(BlockId::new(2).unwrap(), 0)),
        Some(&expected_polarities(true)),
    );
}

#[test]
fn forwarded_negated_case_membership_reverses_edge_polarities() {
    let graph = forwarded_graph(
        vec![Operation::BooleanNot {
            psi_operation: OperationId::new(2).unwrap(),
            result: ValueId::new(3).unwrap(),
            operand: ValueId::new(1).unwrap(),
        }],
        3,
    );
    assert_eq!(
        graph.case_successors.get(&(BlockId::new(2).unwrap(), 0)),
        Some(&expected_polarities(false)),
    );
}

#[test]
fn saved_case_membership_does_not_survive_replacement_or_call() {
    let call = Operation::CallStructural {
        psi_operation: OperationId::new(2).unwrap(),
        callee: MachineId::new(2).unwrap().into(),
        arguments: Vec::new(),
        structural_arguments: vec![
            StructuralArgument {
                place: PlaceId::new(1).unwrap(),
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            },
            StructuralArgument {
                place: PlaceId::new(2).unwrap(),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        ],
        claim_transfers: Vec::new(),
        result: AbstractStructuralCallResult::Unit,
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    for (operation, intervening) in [("replacement", replacement(1, 2)), ("call", call)] {
        let graph = forwarded_graph(vec![intervening], 1);
        assert!(
            !graph
                .case_successors
                .contains_key(&(BlockId::new(2).unwrap(), 0)),
            "saved membership survived an intervening {operation}",
        );
    }
}
