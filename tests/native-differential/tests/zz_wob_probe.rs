//! Scratch probe: which candidate write-only-borrow shapes produce artifacts,
//! and where does Omega admission reject them?

fn artifact(source: &str, entry: &str) -> Result<terminal_codec::CanonicalTerminalArtifact, String> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .map_err(|e| format!("lex: {e:?}"))?;
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).map_err(|e| format!("parse: {e:?}"))?;
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .map_err(|e| format!("resolve: {e:?}"))?;
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|e| format!("type: {e:?}"))?;
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .map_err(|e| format!("check: {e:?}"))?;
    terminal_production::TerminalProductionRequest::new(&checked, entry)
        .produce_artifact()
        .map_err(|e| format!("produce: {e:?}"))
}

fn optimize(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) -> Result<abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan, String> {
    let selections = optimization_core::OptimizationSelections::new([]).unwrap();
    native_realization::optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .map_err(|e| format!("optimize: {e:?}"))
}

/// Full physical publication leg: optimized plan -> target/physical pipeline ->
/// function fragment -> fixed-frame text -> object -> image -> installation.
fn publish(
    optimized: abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan,
) -> Result<(), String> {
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target::NativeTarget::linux_x64(),
            &[],
        )
        .map_err(|e| format!("physical: {e:?}"))?;
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .map_err(|e| format!("fragments: {e:?}"))?;
    let framed = machine_emission::stage_function_fragment_frame_application(fragments)
        .map_err(|e| format!("frame: {e:?}"))?;
    let placed = machine_emission::stage_optimized_fixed_frame_text_section(framed)
        .map_err(|e| format!("placed: {e:?}"))?;
    machine_emission::validate_optimized_fixed_frame_text_section(&placed)
        .map_err(|e| format!("placed-validate: {e:?}"))?;
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(placed)
            .map_err(|e| format!("object-stage: {e:?}"))?,
    );
    let object = image_emission::build_function_fragment_object_artifact(source)
        .map_err(|e| format!("object: {e:?}"))?;
    let image = image_emission::emit_executable_image(&object, 3)
        .map_err(|e| format!("image: {e:?}"))?;
    let installed = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .map_err(|e| format!("installation: {e:?}"))?;
    let bytes = image_emission::encode_installation_record(&installed)
        .map_err(|e| format!("installation-encode: {e:?}"))?;
    let decoded = image_emission::decode_installation_record(&bytes)
        .map_err(|e| format!("installation-decode: {e:?}"))?;
    image_emission::validate_installation_record(&decoded, &image)
        .map_err(|e| format!("installation-validate: {e:?}"))?;
    Ok(())
}

#[test]
fn probe_candidate_shapes() {
    let candidates: &[(&str, &str, &str)] = &[
        (
            "mid_body_let_borrow",
            "forward",
            "data Record [copy] { value: u16; }
            machine Record::replace(&write self, value: u16) { self.value = value; }
            machine forward(records: &write [Record; 2], value: u16) {
                records[0].replace(value);
                let held: &write Record = &write records[1];
                held.replace(value);
            }",
        ),
        (
            "let_borrow_as_store_root",
            "forward",
            "machine forward(values: &write [u16; 4]) {
                let held: &write [u16; 4] = &write values;
                held[2] = 17;
            }",
        ),
        (
            "let_borrow_as_call_argument",
            "forward",
            "machine stamp(slot: &write u64, value: u64) { slot = value; }
            machine forward(root: &mut u64, value: u64) {
                let held: &write u64 = &write root;
                stamp(held, value);
            }",
        ),
        (
            "let_borrow_reborrow_as_call_argument",
            "forward",
            "machine stamp(slot: &write u64, value: u64) { slot = value; }
            machine forward(root: &mut u64, value: u64) {
                let held: &write u64 = &write root;
                stamp(&write held, value);
            }",
        ),
        (
            "owned_record_field_borrow_argument",
            "enter",
            "data Pair [copy] { left: u64; right: u64; }
            machine stamp(left: &write u64, value: u64) { left = value; }
            machine enter(value: u64) -> u64 {
                let mut pair: Pair = Pair { left: 1, right: 2 };
                stamp(&write pair.left, value);
                pair.left
            }",
        ),
        (
            "whole_aggregate_store_through_borrow",
            "forward",
            "data Pair [copy] { left: u64; right: u64; }
            machine forward(pair: &write Pair, left: u64, right: u64) {
                pair = Pair { left: left, right: right };
            }",
        ),
        (
            "dynamic_index_store",
            "forward",
            "machine forward(values: &mut [u16; 4], index: u8) {
                values[index] = 17;
            }",
        ),
        (
            "dynamic_index_store_u64",
            "forward",
            "machine forward(values: &mut [u16; 4], index: u64) {
                values[index] = 17;
            }",
        ),
        (
            "dynamic_index_store_guarded",
            "forward",
            "machine forward(values: &mut [u16; 4], index: u64) {
                transition index < 4 { true -> store(values, index) _ -> done() }
                state store(&mut self, values: &mut [u16; 4], index: u64) { values[index] = 17; }
                state done(&mut self) {}
            }",
        ),
        (
            "dynamic_index_receiver_call",
            "forward",
            "data Record [copy] { value: u16; }
            machine Record::replace(&write self, value: u16) { self.value = value; }
            machine forward(records: &write [Record; 4], index: u64, value: u16) {
                transition index < 4 { true -> stamp(&write records, index, value) _ -> done() }
                state stamp(&mut self, records: &write [Record; 4], index: u64, value: u16) { records[index].replace(value); }
                state done(&mut self) {}
            }",
        ),
        (
            "reference_result_relay",
            "exercise",
            "machine relay(value: &mut i32) -> &mut i32 { value }
            machine replace(value: &mut i32) { value = 29; }
            machine exercise(value: &mut i32) -> i32 {
                let held: &mut i32 = relay(value);
                replace(held);
                value
            }",
        ),
        (
            "local_reference_record",
            "exercise",
            "data View { body: &mut i32; }
            machine replace(value: &mut i32) { value = 29; }
            machine exercise(value: &mut i32) -> i32 {
                let held: View = View { body: value };
                replace(held.body);
                value
            }",
        ),
        (
            "borrowed_parent_mut_receiver_with_subloan",
            "forward",
            "data Record [copy] { value: u16; }
            machine Record::replace(&write self, value: u16) { self.value = value; }
            machine Record::bump(&mut self) { self.value = self.value; }
            machine forward(records: &mut [Record; 2], value: u16) {
                let held: &write Record = &write records[1];
                held.replace(value);
                records[0].bump();
            }",
        ),
        (
            "computed_ieee_store",
            "forward",
            "machine forward(values: &mut [f64; 4], left: f64, right: f64) {
                values[2] = left + right;
            }",
        ),
        (
            "scalar_let_borrow_store_root",
            "forward",
            "machine forward(value: &mut u64) {
                let held: &write u64 = &write value;
                held = 17;
            }",
        ),
        (
            "let_mut_borrow_as_call_argument",
            "forward",
            "machine stamp(slot: &mut u64, value: u64) { slot = value; }
            machine forward(root: &mut u64, value: u64) {
                let held: &mut u64 = &mut root;
                stamp(held, value);
            }",
        ),
        (
            "let_mut_borrow_store_root",
            "forward",
            "machine forward(root: &mut u64) {
                let held: &mut u64 = &mut root;
                held = 17;
            }",
        ),
        (
            "let_mut_borrow_receiver_call",
            "forward",
            "data Record [copy] { value: u16; }
            machine Record::replace(&write self, value: u16) { self.value = value; }
            machine forward(records: &mut [Record; 2], value: u16) {
                let held: &mut [Record; 2] = &mut records;
                held[1].replace(value);
            }",
        ),
        (
            "reborrowed_mut_let",
            "forward",
            "machine stamp(slot: &mut u64, value: u64) { slot = value; }
            machine forward(root: &mut u64, value: u64) {
                let held: &mut u64 = &mut root;
                stamp(&mut held, value);
            }",
        ),
        (
            "shared_scalar_callee_body",
            "forward",
            "machine read(root: &u64) -> u64 { root }
            machine forward(root: &mut u64) -> u64 { read(root) }",
        ),
        (
            "write_self_on_borrowed_parent_with_live_subloan",
            "forward",
            "data Record [copy] { value: u16; }
            machine Record::replace(&write self, value: u16) { self.value = value; }
            machine forward(records: &mut [Record; 2], value: u16) {
                let held: &write Record = &write records[1];
                records[0].replace(value);
                held.replace(value);
            }",
        ),
    ];
    for (name, entry, source) in candidates {
        match artifact(source, entry) {
            Ok(artifact) => {
                let module =
                    terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
                for machine in &module.machines {
                    for block in &machine.blocks {
                        for op in &block.operations {
                            eprintln!("PROBE {name} OP: {:?}", op.kind);
                        }
                        eprintln!("PROBE {name} TERM: {:?}", block.terminator);
                    }
                }
                for machine in &module.machines {
                    eprintln!(
                        "PROBE {name} machine params={:?} result={:?} places={:?}",
                        machine.structural_parameters, machine.result, machine.structural_places
                    );
                }
                eprintln!(
                    "PROBE {name} structural_types={:?}",
                    module.structural_types
                );
                match optimize(&artifact) {
                    Ok(optimized) => match publish(optimized) {
                        Ok(()) => eprintln!("PROBE {name}: artifact OK, optimize OK, publish OK"),
                        Err(error) => {
                            eprintln!("PROBE {name}: artifact OK, optimize OK, PUBLISH FAIL: {error}")
                        }
                    },
                    Err(error) => {
                        eprintln!("PROBE {name}: artifact OK, OPTIMIZE FAIL: {error}")
                    }
                }
            }
            Err(error) => eprintln!("PROBE {name}: NO ARTIFACT: {error}"),
        }
    }
}
