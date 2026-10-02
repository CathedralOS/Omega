//! Ordinary byte completions retain their owned carrier and declared predicates
//! when the optimizer independently replays the current graph.
use crate::tests::indexed_byte_fields::indexed_field_source_unit;
use crate::tests::support::{refresh_identity, refresh_node_derivatives};
use crate::validate_psi_optimization_unit;
use abstract_operations::{AbstractOperation, AbstractStructuralCallResult};
use optimization_unit::PsiOptimizationUnit;
use terminal_psi::{ByteSequenceCarrier, StructuralAccess, StructuralTypeShape};

#[test]
fn byte_arrival_length_rejects_same_carrier_non_byte_domain() {
    for body in [
        "input.len",
        "transition { _ -> arrived(input) }\n\
         state arrived(input: &mut [u8;16] in Utf8) { input.len }",
    ] {
        let source = format!(
            "domain [u8;16]::Utf8 requires valid_utf8(self);\n\
             data Record {{ value: u64; }}\n\
             machine Record::replace(&mut self, input: &mut [u8;16] in Utf8) -> u64 {{ {body} }}"
        );
        let mut unit = indexed_field_source_unit(&source);
        validate_psi_optimization_unit(&unit)
            .expect("primitive-qualified entry and state owners expose live length");
        let (machine, block, node, domain) = unit
            .functions
            .iter()
            .find_map(|function| {
                function.blocks.iter().find_map(|block| {
                    block
                        .nodes
                        .iter()
                        .enumerate()
                        .find_map(|(node, operation)| {
                            let AbstractOperation::ByteSequenceLength { source, .. } =
                                operation.operation
                            else {
                                return None;
                            };
                            let parameter = function.arrival_parameter(source)?;
                            let [domain] = parameter.qualifications.as_slice() else {
                                return None;
                            };
                            Some((
                                function.machine,
                                block.id,
                                u32::try_from(node).unwrap(),
                                *domain,
                            ))
                        })
                })
            })
            .expect("length observes an exact qualified arrival");
        let mut domains = unit.structural_domains.to_vec();
        let declaration = domains
            .iter_mut()
            .find(|candidate| candidate.id == domain)
            .unwrap();
        assert!(declaration.byte_predicate.take().is_some());
        // Keep the exact domain identity and carrier. Catalog membership alone
        // must not substitute for the primitive byte-domain contract.
        unit.structural_domains = domains.into();
        refresh_identity(&mut unit);
        assert_eq!(
            validate_psi_optimization_unit(&unit),
            Err(
                crate::OptimizationUnitValidationError::InvalidByteSequenceLength {
                    machine,
                    block,
                    node,
                }
            ),
            "{body}",
        );
    }
}

fn mutable_byte_arrival_unit() -> PsiOptimizationUnit {
    indexed_field_source_unit(
        r#"
        domain [u8;16]::Utf8 requires valid_utf8(self);
        data Record { text: [u8;16] in Utf8; }
        machine fill(output: &mut [u8;16] in Utf8) ensures output in Utf8 {
            transition { _ -> first(output) }
            state first(output: &mut [u8;16] in Utf8) {
                output = "Gate";
                transition { _ -> second(output) }
            }
            state second(output: &mut [u8;16] in Utf8) { output = "é"; }
        }
        machine Record::replace(&mut self) { fill(&mut self.text); }
        "#,
    )
}

#[test]
fn mutable_byte_arrivals_replay_whole_store_contracts() {
    let unit = mutable_byte_arrival_unit();
    validate_psi_optimization_unit(&unit).expect("qualified mutable byte arrivals replay");
    let stores = unit.functions.iter().flat_map(|function| &function.blocks)
        .flat_map(|block| block.nodes.iter().filter(move |node| {
            matches!(node.operation, AbstractOperation::ByteSequenceStore { destination, .. }
                if block.structural_parameters.iter().any(|parameter| parameter.place == destination))
        })).count();
    assert_eq!(
        stores, 2,
        "both state-local owners receive whole replacements"
    );
}

#[test]
fn mutable_byte_arrivals_reject_wrong_declaration_and_store_evidence() {
    let baseline = mutable_byte_arrival_unit();
    let (function_index, block_index, node_index, destination) = baseline
        .functions
        .iter()
        .enumerate()
        .find_map(|(function_index, function)| {
            function
                .blocks
                .iter()
                .enumerate()
                .find_map(|(block_index, block)| {
                    block
                        .nodes
                        .iter()
                        .enumerate()
                        .find_map(|(node_index, node)| {
                            let AbstractOperation::ByteSequenceStore { destination, .. } =
                                node.operation
                            else {
                                return None;
                            };
                            block
                                .structural_parameters
                                .iter()
                                .any(|parameter| parameter.place == destination)
                                .then_some((function_index, block_index, node_index, destination))
                        })
                })
        })
        .expect("whole store through a block declaration");
    for corruption in [
        "shared access",
        "qualification",
        "position",
        "duplicate",
        "source",
        "length",
    ] {
        let mut unit = baseline.clone();
        if matches!(corruption, "source" | "length") {
            let other_length = unit.functions[function_index]
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .find_map(|node| match node.operation {
                    AbstractOperation::ByteSequenceStore {
                        destination: other,
                        length,
                        ..
                    } if other != destination => Some(length),
                    _ => None,
                })
                .expect("second replacement has its own current length");
            let AbstractOperation::ByteSequenceStore { source, length, .. } =
                &mut unit.functions[function_index].blocks[block_index].nodes[node_index].operation
            else {
                panic!("selected whole store");
            };
            if corruption == "source" {
                *source = destination;
            } else {
                *length = other_length;
            }
            refresh_node_derivatives(&mut unit, function_index, block_index, node_index);
        } else {
            let parameters =
                &mut unit.functions[function_index].blocks[block_index].structural_parameters;
            let position = parameters
                .iter()
                .position(|parameter| parameter.place == destination)
                .unwrap();
            match corruption {
                "shared access" => parameters[position].access = StructuralAccess::SharedBorrow,
                "qualification" => {
                    assert!(!parameters[position].qualifications.is_empty());
                    parameters[position].qualifications.clear();
                }
                "position" => parameters[position].position += 1,
                "duplicate" => parameters.push(parameters[position].clone()),
                _ => unreachable!(),
            }
            refresh_identity(&mut unit);
        }
        assert!(
            validate_psi_optimization_unit(&unit).is_err(),
            "{corruption}"
        );
    }
}

fn owned_byte_call_unit() -> PsiOptimizationUnit {
    indexed_field_source_unit(
        r#"
        domain [u8;16]::Utf8 requires valid_utf8(self);
        data Record { text: [u8;16] in Utf8; }
        machine label(short: bool) -> [u8;16] in Utf8 {
            transition short { true -> "Gate" false -> "Branch Room" }
        }
        machine Record::replace(&mut self) {
            self.text = label(false);
            self.text = label(true);
            transition { _ -> finish() }
            state finish() {}
        }
        "#,
    )
}

#[test]
fn owned_byte_call_results_replay_predicates_and_return_contracts() {
    let unit = owned_byte_call_unit();
    validate_psi_optimization_unit(&unit)
        .expect("owned completion, measured length, and field replacement replay");
    assert!(
        unit.functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.nodes)
            .any(|node| matches!(
                node.operation,
                AbstractOperation::EstablishByteSequenceConcatenation { .. }
            ))
    );
}

#[test]
fn owned_byte_call_results_reject_changed_qualification_and_carrier() {
    let baseline = owned_byte_call_unit();
    for corruption in [
        "callee qualification",
        "caller qualification",
        "borrowed carrier",
    ] {
        let mut unit = baseline.clone();
        if corruption == "borrowed carrier" {
            let mut declarations = unit.structural_types.to_vec();
            let declaration = declarations
                .iter_mut()
                .find(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                            capacity: 16
                        })
                    )
                })
                .expect("owned return carrier");
            declaration.shape =
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None,
                });
            unit.structural_types = declarations.into();
        } else {
            let result =
                unit.functions
                    .iter_mut()
                    .flat_map(|function| &mut function.blocks)
                    .flat_map(|block| &mut block.nodes)
                    .find_map(|node| match &mut node.operation {
                        AbstractOperation::EstablishByteSequenceConcatenation {
                            result, ..
                        } if corruption == "callee qualification" => Some(result),
                        AbstractOperation::CallStructural {
                            result: AbstractStructuralCallResult::Structural { result, .. },
                            ..
                        } if corruption == "caller qualification" => Some(result),
                        _ => None,
                    })
                    .expect("owned producer result");
            assert!(!result.qualifications.is_empty(), "{corruption}");
            result.qualifications.clear();
        }
        // Reject semantic corruption, not just stale cached node metadata or
        // the unit identity left over from the admitted graph.
        for function_index in 0..unit.functions.len() {
            for block_index in 0..unit.functions[function_index].blocks.len() {
                for node_index in 0..unit.functions[function_index].blocks[block_index]
                    .nodes
                    .len()
                {
                    refresh_node_derivatives(&mut unit, function_index, block_index, node_index);
                }
            }
        }
        assert!(
            validate_psi_optimization_unit(&unit).is_err(),
            "{corruption}"
        );
    }
}
