use super::*;

fn selected_return() -> PsiOptimizationUnit {
    source_machine_unit(
        "data Tag { case First; case Second; }
         machine choose(selector: u64) -> Tag {
             match selector { 0 -> Tag::First, _ -> Tag::Second }
         }",
        "choose",
        true,
    )
}

#[test]
fn selected_case_returns_the_exact_owned_block_binding() {
    validate_psi_optimization_unit(&selected_return())
        .expect("selected owner returns through its ordinary join");
}

#[test]
fn selected_case_return_rejects_join_custody_substitution() {
    let original = selected_return();
    validate_psi_optimization_unit(&original).unwrap();
    for mutation in ["access", "position", "source", "ownership"] {
        let mut unit = original.clone();
        let function_index = unit
            .functions
            .iter()
            .position(|function| function.machine == unit.entry)
            .unwrap();
        let function = &mut unit.functions[function_index];
        let (block_index, node_index, source) = function
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block_index, block)| {
                block
                    .nodes
                    .iter()
                    .enumerate()
                    .find_map(|(node_index, node)| {
                        if let AbstractOperation::ReturnStructural { source, .. } = node.operation {
                            Some((block_index, node_index, source))
                        } else {
                            None
                        }
                    })
            })
            .unwrap();
        match mutation {
            "access" | "position" => {
                let parameter = function
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.structural_parameters)
                    .find(|parameter| parameter.place == source)
                    .unwrap();
                if mutation == "access" {
                    parameter.access = terminal_psi::StructuralAccess::SharedBorrow;
                } else {
                    parameter.position += 1;
                }
            }
            "source" => {
                let producer = function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes)
                    .find_map(|node| {
                        if let AbstractOperation::EstablishScalarCase { result, .. } =
                            &node.operation
                        {
                            Some(result.place)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                let AbstractOperation::ReturnStructural { source, .. } =
                    &mut function.blocks[block_index].nodes[node_index].operation
                else {
                    panic!("return");
                };
                *source = producer;
            }
            "ownership" => {
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
        if mutation != "ownership" {
            refresh_node_derivatives(&mut unit, function_index, block_index, node_index);
        }
        refresh_identity(&mut unit);
        assert!(validate_psi_optimization_unit(&unit).is_err(), "{mutation}");
    }
}
