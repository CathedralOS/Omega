use super::*;

#[path = "owned_results/returns.rs"]
mod returns;

#[test]
fn owned_call_result_transfers_to_case_parameter() {
    let unit = owned_result_unit();
    validate_psi_optimization_unit(&unit)
        .expect("owned call result reaches an ordinary case block");
}

fn owned_result_unit() -> PsiOptimizationUnit {
    source_machine_unit(
        r#"
        data Outcome { case Empty; case Value(value: u64); }
        machine choose(selector: u64, value: u64) -> Outcome {
            transition selector == 0 { true -> empty() false -> value(value) }
            state empty() -> Outcome { Outcome::Empty }
            state value(value: u64) -> Outcome { Outcome::Value { value: value } }
        }
        machine collect(out: &mut [u8], selector: u64, value: u64) {
            let result: Outcome = choose(selector, value);
            transition { _ -> inspect(out, result) }
            state inspect(out: &mut [u8], result: Outcome) {
                transition result {
                    Outcome::Empty -> writable(out, 0)
                    Outcome::Value { value } -> writable(out, value)
                }
            }
            state writable(out: &mut [u8], value: u64) {
                transition out.len > 0 { true -> store(out) false -> done() }
            }
            state store(out: &mut [u8]) { out[0] = 42; }
            state done() {}
        }
    "#,
        "collect",
        true,
    )
}

#[test]
fn owned_result_transfer_rejects_current_custody_corruption() {
    let original = owned_result_unit();
    validate_psi_optimization_unit(&original).unwrap();
    for mutation in [
        "result origin",
        "result multiplicity",
        "arrival type",
        "arrival access",
        "producer identity",
        "lost frontier",
    ] {
        let mut changed = original.clone();
        let function_index = changed
            .functions
            .iter()
            .position(|function| function.machine == changed.entry)
            .unwrap();
        let function = &mut changed.functions[function_index];
        let (block_index, node_index) = function
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block_index, block)| {
                block
                    .nodes
                    .iter()
                    .position(|node| {
                        matches!(node.operation, AbstractOperation::CallStructural { .. })
                    })
                    .map(|node_index| (block_index, node_index))
            })
            .unwrap();
        match mutation {
            "result origin" | "result multiplicity" => {
                let AbstractOperation::CallStructural { result, .. } =
                    &mut function.blocks[block_index].nodes[node_index].operation
                else {
                    unreachable!()
                };
                if mutation == "result origin" {
                    result.place = id(999, PlaceId::new);
                } else {
                    result.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
                }
            }
            "arrival type" | "arrival access" => {
                let parameter = function
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.structural_parameters)
                    .find(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned)
                    .unwrap();
                if mutation == "arrival type" {
                    parameter.structural_type = id(999, semantic_vocabulary::StructuralTypeId::new);
                } else {
                    parameter.access = terminal_psi::StructuralAccess::SharedBorrow;
                }
            }
            "producer identity" => {
                let AbstractOperation::CallStructural { result, .. } =
                    &function.blocks[block_index].nodes[node_index].operation
                else {
                    unreachable!()
                };
                let place = function
                    .structural_places
                    .iter_mut()
                    .find(|place| place.id == result.place)
                    .unwrap();
                let semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } =
                    &mut place.kind
                else {
                    unreachable!()
                };
                *producer = id(999, OperationId::new);
            }
            "lost frontier" => {
                assert!(
                    !function.blocks[block_index].nodes[node_index]
                        .ownership
                        .is_empty()
                );
                function.blocks[block_index].nodes[node_index]
                    .ownership
                    .clear();
            }
            _ => unreachable!(),
        }
        if mutation != "lost frontier" {
            refresh_node_derivatives(&mut changed, function_index, block_index, node_index);
        }
        refresh_identity(&mut changed);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn established_scalar_case_transfers_to_owned_block_parameter() {
    let mut unit = owned_result_unit();
    let empty_case = unit
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::EstablishScalarCase {
                result_case,
                fields,
                ..
            } if fields.is_empty() => Some(*result_case),
            _ => None,
        })
        .unwrap();
    let function_index = unit
        .functions
        .iter()
        .position(|function| function.machine == unit.entry)
        .unwrap();
    let function = &mut unit.functions[function_index];
    let (block_index, node_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .nodes
                .iter()
                .position(|node| matches!(node.operation, AbstractOperation::CallStructural { .. }))
                .map(|node_index| (block_index, node_index))
        })
        .unwrap();
    let operation = &mut function.blocks[block_index].nodes[node_index].operation;
    let AbstractOperation::CallStructural {
        psi_operation,
        result,
        ..
    } = operation
    else {
        unreachable!()
    };
    *operation = AbstractOperation::EstablishScalarCase {
        psi_operation: *psi_operation,
        result: result.clone(),
        result_case: empty_case,
        fields: Vec::new(),
    };
    refresh_node_derivatives(&mut unit, function_index, block_index, node_index);
    validate_psi_optimization_unit(&unit)
        .expect("direct establishment has the same whole owned edge custody");
}
