//! Backward scalar demand through current operation results and control edges.
//! Block parameters are demand sites, not uses: an edge argument is live only
//! when the parameter it feeds is live.

use semantic_vocabulary::{BlockId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{OperationKind as O, TerminalMachine, Terminator};

pub(super) fn eliminate(
    machine: &mut TerminalMachine,
    source_calls: &[lowered_psi::LoweredSourceCallOccurrence],
    retained_values: &[ValueId],
) {
    let operations = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let mut pending = retained_values.to_vec();
    for operation in &operations {
        if !terminal_semantics::is_unconditionally_total_scalar(&operation.kind)
            && !inputs(&operation.kind, &mut pending)
        {
            // Dynamic catalogs and guarded call provenance carry additional
            // scalar references outside the local operand list.
            return;
        }
    }
    let mut incoming_arguments: BTreeMap<BlockId, Vec<Vec<ValueId>>> = BTreeMap::new();
    let mut structural_case_targets = BTreeSet::new();
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => {
                incoming_arguments
                    .entry(*target)
                    .or_default()
                    .push(arguments.clone());
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                pending.push(*condition);
                for edge in [when_true, when_false] {
                    incoming_arguments
                        .entry(edge.target)
                        .or_default()
                        .push(edge.arguments.clone());
                }
            }
            Terminator::Return { value, .. } => pending.push(*value),
            Terminator::Crash { .. } => return, // Retain exact guard-term inputs.
            Terminator::StructuralCase { cases, .. } => {
                structural_case_targets.extend(cases.iter().map(|case| case.target));
            }
            Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. } => {}
        }
    }
    // Block parameters survive removal only in machines without ranking
    // evidence, outside `StructuralCase` payload targets, and with at least
    // one inventoried incoming edge. Entry blocks declare no parameters and
    // crash continuations declare only empty parameter tables, so neither
    // needs special handling.
    let mut parameter_owner: BTreeMap<ValueId, (BlockId, usize)> = BTreeMap::new();
    let mut parameter_eligible = BTreeSet::new();
    for block in &machine.blocks {
        for (position, parameter) in block.parameters.iter().enumerate() {
            parameter_owner.insert(parameter.id, (block.id, position));
        }
        if machine.ranked_scc.is_none()
            && !structural_case_targets.contains(&block.id)
            && incoming_arguments.contains_key(&block.id)
        {
            parameter_eligible.insert(block.id);
        } else {
            pending.extend(block.parameters.iter().map(|parameter| parameter.id));
        }
    }
    for occurrence in source_calls {
        if operations
            .iter()
            .any(|operation| operation.id == occurrence.terminal_operation)
        {
            pending.extend(
                occurrence
                    .source_values_before_call
                    .iter()
                    .map(|value| value.id),
            );
        }
    }
    let mut live = BTreeSet::new();
    while let Some(value) = pending.pop() {
        if !live.insert(value) {
            continue;
        }
        if let Some(producer) = operations.iter().find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|result| result.id == value)
        }) {
            if !inputs(&producer.kind, &mut pending) {
                return;
            }
        } else if let Some(&(block, position)) = parameter_owner.get(&value)
            && let Some(edges) = incoming_arguments.get(&block)
        {
            for arguments in edges {
                pending.push(arguments[position]);
            }
        }
    }
    let mut removed_positions: BTreeMap<BlockId, Vec<usize>> = BTreeMap::new();
    for block in &mut machine.blocks {
        block.operations.retain(|operation| {
            !terminal_semantics::is_unconditionally_total_scalar(&operation.kind)
                || operation
                    .result
                    .scalar()
                    .is_none_or(|result| live.contains(&result.id))
        });
        if parameter_eligible.contains(&block.id) {
            let removed = block
                .parameters
                .iter()
                .enumerate()
                .filter_map(|(position, parameter)| {
                    (!live.contains(&parameter.id)).then_some(position)
                })
                .collect::<Vec<_>>();
            if !removed.is_empty() {
                removed_positions.insert(block.id, removed);
            }
            block
                .parameters
                .retain(|parameter| live.contains(&parameter.id));
        }
    }
    for block in &mut machine.blocks {
        match &mut block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => drop_removed(arguments, removed_positions.get(target)),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    drop_removed(&mut edge.arguments, removed_positions.get(&edge.target));
                }
            }
            _ => {}
        }
    }
}

/// Drop the listed old positions from one edge's scalar arguments.
fn drop_removed(arguments: &mut Vec<ValueId>, removed: Option<&Vec<usize>>) {
    let Some(removed) = removed else { return };
    let mut position = 0;
    arguments.retain(|_| {
        let retained = !removed.contains(&position);
        position += 1;
        retained
    });
}

/// False means the operand inventory is indirect and the machine is retained.
/// The exhaustive match forces new operation variants to declare that fact.
fn inputs(operation: &O, values: &mut Vec<ValueId>) -> bool {
    match operation {
        // This pass never removes structural storage. A primitive read has no
        // scalar operand, but remains an observation even when its result dies.
        O::PrimitiveScalarRead { .. } => {}
        O::StructuralByteSequenceFieldByteStore {
            index,
            value,
            length,
            ..
        }
        | O::ByteSequenceWrite {
            index,
            value,
            length,
            ..
        } => {
            values.extend([*index, *value, *length]);
        }
        O::StructuralByteSequenceFieldStore { length, .. } => values.push(*length),
        O::ByteSequenceRead { index, length, .. } => values.extend([*index, *length]),
        O::ByteSequenceSubslice {
            start, end, length, ..
        } => values.extend([*start, *end, *length]),
        O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::IntegerStructuralField { .. }
        | O::ByteSequenceLength { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::BooleanStructuralField { .. }
        | O::StructuralCaseMembership { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishReference { .. }
        | O::ReleaseReference { .. }
        | O::PortWrite { .. } => {}
        O::EstablishScalarCase { fields, .. } => {
            values.extend(fields.iter().map(|field| field.value));
        }
        O::EstablishRecord { fields } => {
            values.extend(fields.iter().filter_map(|field| match field.value {
                terminal_psi::RecordFieldValue::Scalar { value, .. } => Some(value),
                terminal_psi::RecordFieldValue::Structural(_) => None,
            }));
        }
        O::EstablishScalarArray { elements } => values.extend(elements),
        O::BooleanNot { operand }
        | O::IntegerBitwiseNot { operand }
        | O::IntegerWiden { operand }
        | O::IntegerExactCast { operand, .. } => values.push(*operand),
        O::BooleanEqual { left, right }
        | O::IeeeFloatCompare { left, right, .. }
        | O::IntegerEqual { left, right }
        | O::IntegerLessThan { left, right }
        | O::IntegerLessOrEqual { left, right }
        | O::IntegerBitwiseAnd { left, right }
        | O::IntegerBitwiseOr { left, right }
        | O::IntegerBitwiseXor { left, right }
        | O::WrappingIntegerAdd { left, right }
        | O::SaturatingIntegerAdd { left, right }
        | O::WrappingIntegerSubtract { left, right }
        | O::SaturatingIntegerSubtract { left, right }
        | O::WrappingIntegerMultiply { left, right }
        | O::SaturatingIntegerMultiply { left, right }
        | O::ExactIntegerAdd { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. } => values.extend([*left, *right]),
        O::WrappingIntegerShiftLeft { value, count }
        | O::WrappingIntegerShiftRight { value, count }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => values.extend([*value, *count]),
        O::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
        } => values.extend([*left, *right, *addend]),
        O::EstablishPrimitiveLocal { value }
        | O::WriteOnlyPrimitiveStore { value, .. }
        | O::StructuralScalarFieldStore { value, .. } => values.push(*value),
        O::BoundaryCall { arguments, .. } => values.extend(arguments),
        O::Call {
            arguments,
            crash_continuations,
            ..
        }
        | O::CallUnit {
            arguments,
            crash_continuations,
            ..
        }
        | O::CallStructuralScalar {
            arguments,
            crash_continuations,
            ..
        }
        | O::CallStructuralWithScalarArguments {
            arguments,
            crash_continuations,
            ..
        } => {
            if !crash_continuations.is_empty() {
                return false;
            }
            values.extend(arguments);
        }
        O::CallStructural { .. }
        | O::CallDynamicScalar { .. }
        | O::CallDynamicUnit { .. }
        | O::CallDynamicParameterScalar { .. }
        | O::CallDynamicParameterUnit { .. }
        | O::StoreDynamicDescriptor { .. } => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{ObligationId, PlaceId, StructuralFieldId};

    #[test]
    fn scalar_array_construction_retains_its_elements_and_removes_unrelated_values() {
        use semantic_vocabulary::{
            BlockId, ContractId, EdgeId, MachineId, OperationId, ScalarType, StructuralTypeId,
        };
        use terminal_psi::{
            Block, MachineContract, Operation, OperationResult, StructuralMultiplicity,
            StructuralOperationResult, StructuralResultDeclaration, TerminalMachineResult,
            ValueDeclaration,
        };

        let array_type = StructuralTypeId::new(1).unwrap();
        let array_place = PlaceId::new(1).unwrap();
        let result_place = PlaceId::new(2).unwrap();
        for elements in [
            vec![],
            vec![ValueId::new(3).unwrap(), ValueId::new(2).unwrap()],
        ] {
            let mut operations = (1..=3)
                .map(|ordinal| Operation {
                    static_reach_binding: None,
                    id: OperationId::new(ordinal).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: ValueId::new(ordinal).unwrap(),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: O::BooleanConstant {
                        value: ordinal == 2,
                    },
                })
                .collect::<Vec<_>>();
            operations.push(Operation {
                static_reach_binding: None,
                id: OperationId::new(4).unwrap(),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: array_place,
                    structural_type: array_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: O::EstablishScalarArray {
                    elements: elements.clone(),
                },
            });
            let mut machine = TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: MachineId::new(1).unwrap(),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                    reference_sources: Vec::new(),
                    place: result_place,
                    structural_type: array_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }),
                structural_places: vec![
                    terminal_psi::StructuralPlaceDeclaration {
                        id: array_place,
                        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer: OperationId::new(4).unwrap(),
                            structural_type: array_type,
                        },
                    },
                    terminal_psi::StructuralPlaceDeclaration {
                        id: result_place,
                        kind: semantic_vocabulary::StructuralPlaceKind::Result,
                    },
                ],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(1).unwrap(),
                blocks: vec![Block {
                    structural_parameters: Vec::new(),
                    id: BlockId::new(1).unwrap(),
                    parameters: Vec::new(),
                    operations,
                    terminator: Terminator::ReturnStructural {
                        edge: EdgeId::new(1).unwrap(),
                        source: array_place,
                        returned_claims: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: ContractId::new(1).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            };
            eliminate(&mut machine, &[], &[]);
            let retained = &machine.blocks[0].operations;
            assert_eq!(retained.len(), elements.len() + 1);
            for element in &elements {
                assert!(retained.iter().any(|operation| {
                    operation
                        .result
                        .scalar()
                        .is_some_and(|result| result.id == *element)
                }));
            }
            assert_eq!(
                retained.last().unwrap().kind,
                O::EstablishScalarArray { elements }
            );
        }
    }

    #[test]
    fn returned_block_parameter_survives_while_an_unused_one_drops_its_argument() {
        use semantic_vocabulary::{
            BlockId, ContractId, EdgeId, MachineId, OperationId, ScalarType,
        };
        use terminal_psi::{
            Block, MachineContract, Operation, OperationResult, TerminalMachineResult,
            ValueDeclaration,
        };

        let declaration = |ordinal: u64| ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(ordinal).unwrap(),
            scalar_type: ScalarType::Boolean,
        };
        let operations = [10, 11]
            .map(|ordinal| Operation {
                static_reach_binding: None,
                id: OperationId::new(ordinal).unwrap(),
                result: OperationResult::Scalar(declaration(ordinal)),
                kind: O::BooleanConstant { value: true },
            })
            .to_vec();
        let target = BlockId::new(2).unwrap();
        let mut machine = TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(3)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: vec![
                Block {
                    structural_parameters: Vec::new(),
                    id: BlockId::new(1).unwrap(),
                    parameters: Vec::new(),
                    operations,
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1).unwrap(),
                        target,
                        arguments: vec![ValueId::new(10).unwrap(), ValueId::new(11).unwrap()],
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
                    id: target,
                    parameters: vec![declaration(20), declaration(21)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(2).unwrap(),
                        value: ValueId::new(20).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        };
        eliminate(&mut machine, &[], &[]);
        let entry = &machine.blocks[0];
        assert_eq!(entry.operations.len(), 1);
        assert_eq!(
            entry.operations[0].result.scalar().unwrap().id,
            ValueId::new(10).unwrap()
        );
        let Terminator::Jump { arguments, .. } = &entry.terminator else {
            panic!("entry terminator remains a jump");
        };
        assert_eq!(arguments, &[ValueId::new(10).unwrap()]);
        assert_eq!(
            machine.blocks[1]
                .parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect::<Vec<_>>(),
            vec![ValueId::new(20).unwrap()]
        );
    }

    #[test]
    fn block_without_inventoried_incoming_edges_keeps_its_parameters() {
        use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId, ScalarType};
        use terminal_psi::{Block, MachineContract, TerminalMachineResult, ValueDeclaration};

        let declaration = |ordinal: u64| ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(ordinal).unwrap(),
            scalar_type: ScalarType::Boolean,
        };
        let mut machine = TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: vec![
                Block {
                    structural_parameters: Vec::new(),
                    id: BlockId::new(1).unwrap(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(1).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
                    id: BlockId::new(2).unwrap(),
                    parameters: vec![declaration(20)],
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(2).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        };
        eliminate(&mut machine, &[], &[]);
        assert_eq!(
            machine.blocks[1]
                .parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect::<Vec<_>>(),
            vec![ValueId::new(20).unwrap()],
            "a parameterized block no inventoried edge reaches stays live"
        );
    }

    #[test]
    fn scalar_case_construction_demands_each_payload_operand() {
        let payloads = [ValueId::new(2).unwrap(), ValueId::new(1).unwrap()];
        let operation = O::EstablishScalarCase {
            result_case: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
            fields: payloads
                .iter()
                .enumerate()
                .map(|(position, value)| terminal_psi::ScalarCaseField {
                    field: StructuralFieldId::new(position as u64 + 1).unwrap(),
                    value: *value,
                    range_obligation: None,
                })
                .collect(),
        };
        let mut pending = Vec::new();
        assert!(inputs(&operation, &mut pending));
        assert_eq!(pending, payloads);
        assert!(!terminal_semantics::is_unconditionally_total_scalar(
            &operation
        ));
    }

    #[test]
    fn indexed_byte_store_demands_all_scalar_operands() {
        let index = ValueId::new(1).unwrap();
        let value = ValueId::new(2).unwrap();
        let length = ValueId::new(3).unwrap();
        let operation = O::StructuralByteSequenceFieldByteStore {
            destination: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            field: StructuralFieldId::new(1).unwrap(),
            index,
            value,
            length,
            obligation: ObligationId::new(1).unwrap(),
        };
        let mut pending = Vec::new();
        assert!(inputs(&operation, &mut pending));
        assert_eq!(pending, vec![index, value, length]);
    }

    #[test]
    fn mutable_byte_view_write_retains_its_effect_and_scalar_operands() {
        let index = ValueId::new(1).unwrap();
        let value = ValueId::new(2).unwrap();
        let length = ValueId::new(3).unwrap();
        let operation = O::ByteSequenceWrite {
            destination: PlaceId::new(1).unwrap(),
            index,
            value,
            length,
            obligation: ObligationId::new(1).unwrap(),
        };
        let mut pending = Vec::new();
        assert!(inputs(&operation, &mut pending));
        assert_eq!(pending, vec![index, value, length]);
        assert!(!terminal_semantics::is_unconditionally_total_scalar(
            &operation
        ));
    }

    #[test]
    fn byte_field_length_has_structural_source_not_scalar_operand() {
        let operation = O::StructuralByteSequenceFieldLength {
            source: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            field: StructuralFieldId::new(1).unwrap(),
        };
        let mut pending = Vec::new();
        assert!(inputs(&operation, &mut pending));
        assert!(pending.is_empty());
    }

    #[test]
    fn primitive_storage_initialization_demands_its_value_and_reads_remain_observations() {
        let initializer = ValueId::new(1).unwrap();
        let establishment = O::EstablishPrimitiveLocal { value: initializer };
        let read = O::PrimitiveScalarRead {
            source: PlaceId::new(1).unwrap(),
        };
        let mut pending = Vec::new();
        assert!(inputs(&establishment, &mut pending));
        assert_eq!(pending, vec![initializer]);
        pending.clear();
        assert!(inputs(&read, &mut pending));
        assert!(pending.is_empty());
        assert!(!terminal_semantics::is_unconditionally_total_scalar(
            &establishment
        ));
        assert!(!terminal_semantics::is_unconditionally_total_scalar(&read));
    }
}
