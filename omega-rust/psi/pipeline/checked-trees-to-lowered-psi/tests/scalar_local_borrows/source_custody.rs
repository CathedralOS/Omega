use super::*;

#[test]
fn scalar_local_graph_rejects_erased_and_swapped_binding_destinations() {
    let original = publish_original(TWO_LOCALS);
    let root = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap()
        .symbol;
    for mutation in [
        "erase establishment",
        "erase assignment",
        "swap establishments",
        "swap assignment",
        "delete binding",
    ] {
        let mut changed = original.clone();
        let graph = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == root)
            .expect("real scalar root graph");
        assert_eq!(graph.states.len(), 1);
        let bindings = &mut graph.states[0].bindings;
        let initializers = bindings
            .iter()
            .enumerate()
            .filter_map(|(binding_index, binding)| {
                matches!(
                    binding.destination,
                    CheckedScalarBindingDestination::StorageInitialize { .. }
                )
                .then_some(binding_index)
            })
            .collect::<Vec<_>>();
        assert_eq!(initializers.len(), 2);
        let assignment = bindings
            .iter()
            .position(|binding| {
                matches!(
                    binding.destination,
                    CheckedScalarBindingDestination::StorageAssign { .. }
                )
            })
            .expect("local assignment");
        let CheckedScalarBindingDestination::StorageInitialize { symbol: spare } =
            bindings[initializers[1]].destination
        else {
            panic!("spare local");
        };
        match mutation {
            "erase establishment" => {
                bindings[initializers[0]].destination = CheckedScalarBindingDestination::Immutable
            }
            "erase assignment" => {
                bindings[assignment].destination = CheckedScalarBindingDestination::Immutable
            }
            "swap establishments" => {
                let first = bindings[initializers[0]].destination;
                bindings[initializers[0]].destination = bindings[initializers[1]].destination;
                bindings[initializers[1]].destination = first;
            }
            "swap assignment" => {
                bindings[assignment].destination =
                    CheckedScalarBindingDestination::StorageAssign { symbol: spare }
            }
            "delete binding" => {
                bindings.remove(initializers[0]);
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn scalar_local_borrow_actual_cannot_be_another_same_typed_local() {
    let original = publish_original(TWO_LOCALS);
    let arguments = original
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .iter()
        .filter_map(|(handle, argument)| match argument.as_place()?.source {
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } => {
                Some((handle, symbol))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let identities =
        arguments
            .iter()
            .map(|(_, symbol)| *symbol)
            .fold(Vec::new(), |mut identities, symbol| {
                if !identities.contains(&symbol) {
                    identities.push(symbol);
                }
                identities
            });
    assert_eq!(identities.len(), 2);
    for synchronize_borrow in [false, true] {
        let mut changed = original.clone();
        for (handle, symbol) in &arguments {
            let replacement = *identities.iter().find(|other| *other != symbol).unwrap();
            changed
                .facts
                .values
                .scalar_computations
                .structural_arguments
                .get_mut(*handle)
                .as_place_mut()
                .expect("retained primitive local place")
                .source = CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                symbol: replacement,
            };
        }
        if synchronize_borrow {
            let mut mutations = 0;
            for (handle, access) in original.facts.borrow.argument_accesses.iter() {
                if identities.contains(&access.root_symbol) {
                    let replacement = *identities
                        .iter()
                        .find(|other| **other != access.root_symbol)
                        .unwrap();
                    changed
                        .facts
                        .borrow
                        .argument_accesses
                        .get_mut(handle)
                        .root_symbol = replacement;
                    mutations += 1;
                }
            }
            assert!(mutations > 0, "fixture retains local borrow accesses");
        }
        reject(
            &changed,
            if synchronize_borrow {
                "coherent checked actual and borrow substitution"
            } else {
                "same-typed local actual substitution"
            },
        );
    }
}

#[test]
fn scalar_local_computed_read_cannot_use_another_local_or_scalar_namespace() {
    let source = TWO_LOCALS.replace(
        "    slot = answer;",
        "    slot = first(slot, stamp(&mut spare, before));",
    );
    let original = publish_original(&source);
    let reads = original
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter_map(|(handle, node)| {
            let CheckedScalarComputationKind::Value(CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type,
            }) = node.kind
            else {
                return None;
            };
            Some((handle, symbol, primitive_type))
        })
        .collect::<Vec<_>>();
    assert!(!reads.is_empty(), "fixture retains computed storage reads");
    let local_identities = original
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .iter()
        .filter_map(|(_, argument)| match argument.as_place()?.source {
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } => Some(symbol),
            _ => None,
        })
        .fold(Vec::new(), |mut identities, symbol| {
            if !identities.contains(&symbol) {
                identities.push(symbol);
            }
            identities
        });
    assert_eq!(local_identities.len(), 2);
    for mutation in ["other local", "scalar parameter", "missing occurrence"] {
        let mut changed = original.clone();
        for (handle, symbol, primitive_type) in &reads {
            let node = changed
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(*handle);
            match mutation {
                "other local" => {
                    node.kind =
                        CheckedScalarComputationKind::Value(CheckedScalarExpression::StorageRead {
                            symbol: *local_identities
                                .iter()
                                .find(|other| *other != symbol)
                                .unwrap(),
                            primitive_type: *primitive_type,
                        })
                }
                "scalar parameter" => {
                    node.kind =
                        CheckedScalarComputationKind::Value(CheckedScalarExpression::Parameter {
                            position: 0,
                            primitive_type: *primitive_type,
                        })
                }
                "missing occurrence" => node.value_source = Default::default(),
                _ => unreachable!(),
            }
        }
        reject(&changed, mutation);
    }
}

#[test]
fn scalar_local_snapshot_read_rejects_erased_or_reordered_source_namespace() {
    let original = publish_original(TWO_LOCALS);
    let root = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let state = original.machine_states(root)[0].symbol;
    let expressions = &original.facts.values.scalar_expressions;
    let (handle, binding) = expressions
        .source_bindings
        .iter()
        .find(|(_, binding)| {
            binding.state == state
                && expressions.expressions.iter().any(|expression| {
                    expression.state == state
                        && expression.statement_ordinal == binding.statement_ordinal
                        && expression.role == binding.role
                        && matches!(
                            expression.expression,
                            CheckedScalarExpression::StorageRead { .. }
                        )
                })
        })
        .expect("snapshot retains its source namespace");
    let symbols = expressions.binding_symbols.span(binding.symbols).unwrap();
    assert!(symbols.len() >= 2);
    assert_ne!(symbols[0], symbols[1]);
    for erase in [false, true] {
        let mut changed = original.clone();
        let expressions = &mut changed.facts.values.scalar_expressions;
        let replacement = if erase {
            arena::HandleSpan::empty()
        } else {
            let mut reordered = symbols.to_vec();
            reordered.swap(0, 1);
            expressions.binding_symbols.insert_many(reordered)
        };
        expressions.source_bindings.get_mut(handle).symbols = replacement;
        reject(
            &changed,
            if erase {
                "erased read namespace"
            } else {
                "reordered same-typed namespace"
            },
        );
    }
}

#[test]
fn independent_verifier_rejects_local_place_tampering_after_valid_publication() {
    let original = publish_original(SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(&original, "enter")
        .produce_artifact()
        .unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let foreign_place = module
        .machines
        .iter()
        .find_map(|machine| machine.structural_parameters.first())
        .expect("stamp owns a same-typed borrowed parameter")
        .place;
    for mutation in ["read source", "store destination", "missing establishment"] {
        let mut changed = module.clone();
        let root = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let mut mutations = 0;
        for block in &mut root.blocks {
            if mutation == "missing establishment" {
                let previous = block.operations.len();
                block.operations.retain(|operation| {
                    !matches!(
                        operation.kind,
                        OperationKind::EstablishPrimitiveLocal { .. }
                    )
                });
                mutations += previous - block.operations.len();
                continue;
            }
            for operation in &mut block.operations {
                match &mut operation.kind {
                    OperationKind::PrimitiveScalarRead { source } if mutation == "read source" => {
                        assert_ne!(*source, foreign_place);
                        *source = foreign_place;
                        mutations += 1;
                    }
                    OperationKind::WriteOnlyPrimitiveStore { destination, .. }
                        if mutation == "store destination" =>
                    {
                        assert_ne!(*destination, foreign_place);
                        *destination = foreign_place;
                        mutations += 1;
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(
            mutations, 1,
            "one retained local operation tampered: {mutation}"
        );
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &proof,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "verifier accepted {mutation}"
        );
    }
}
