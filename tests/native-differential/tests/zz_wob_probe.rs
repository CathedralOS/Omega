//! Candidate write-only-borrow shapes pushed through artifact production,
//! optimization, and the full installation-publication leg. Reference carriers
//! and the witnessed borrow lanes must publish; the remaining candidates stay
//! exploratory and only contribute to the failure report.

fn artifact(
    source: &str,
    entry: &str,
) -> Result<terminal_codec::CanonicalTerminalArtifact, String> {
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
    let image =
        image_emission::emit_executable_image(&object, 3).map_err(|e| format!("image: {e:?}"))?;
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
            "domain_qualified_record_field_store",
            "forward",
            "domain [u8; 8]::Utf8 requires valid_utf8(self);
            data Limited { label: [u8; 8] in Utf8; }
            machine forward(limited: &write Limited, next: [u8; 8] in Utf8) {
                limited.label = next;
            }",
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
        (
            "transient_write_reborrow_of_shared_local",
            "forward",
            "machine stamp(slot: &write u64) { slot = 7; }
            machine forward(source: &u64) {
                let r: &u64 = source;
                stamp(&write r);
            }",
        ),
        (
            "let_bound_write_reborrow_of_shared_local",
            "forward",
            "machine forward(source: &u64) {
                let r: &u64 = source;
                let w: &write u64 = &write r;
            }",
        ),
        (
            "let_bound_write_reborrow_of_shared_parameter",
            "forward",
            "machine forward(source: &u64) {
                let w: &write u64 = &write source;
            }",
        ),
    ];
    let mut report = Vec::new();
    for (name, entry, source) in candidates {
        let outcome = match artifact(source, entry) {
            Ok(artifact) => match optimize(&artifact) {
                Ok(optimized) => match publish(optimized) {
                    Ok(()) => "publish OK".to_string(),
                    Err(error) => format!("PUBLISH FAIL: {error}"),
                },
                Err(error) => format!("OPTIMIZE FAIL: {error}"),
            },
            Err(error) => format!("NO ARTIFACT: {error}"),
        };
        report.push((*name, outcome));
    }
    let outcome_of = |wanted: &str| -> &str {
        report
            .iter()
            .find(|(name, _)| *name == wanted)
            .map(|(_, outcome)| outcome.as_str())
            .unwrap_or("missing from candidate list")
    };
    for required in [
        "reference_result_relay",
        "local_reference_record",
        "let_mut_borrow_receiver_call",
        "write_self_on_borrowed_parent_with_live_subloan",
    ] {
        assert_eq!(
            outcome_of(required),
            "publish OK",
            "{required} regressed; full sweep:\n{}",
            report
                .iter()
                .map(|(name, outcome)| format!("{name}: {outcome}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    // `&write` on a shared-reference binding is a referent reborrow and must
    // face the lattice's access-pair rule in every formation position —
    // transient call argument or `let`-bound alike. Each of these compiled
    // before the transient-reborrow rung: the direct argument skipped the
    // lattice entirely and the `let`-bound parameter root carried no parent
    // loan to rebase through.
    for rejected in [
        "transient_write_reborrow_of_shared_local",
        "let_bound_write_reborrow_of_shared_local",
        "let_bound_write_reborrow_of_shared_parameter",
    ] {
        let outcome = outcome_of(rejected);
        assert!(
            outcome.starts_with("NO ARTIFACT:")
                && outcome.contains(
                    "cannot derive WriteOnly reborrow authority from an exact Read parent loan"
                ),
            "{rejected} must stay rejected by the reborrow access-pair rule; full sweep:\n{}",
            report
                .iter()
                .map(|(name, outcome)| format!("{name}: {outcome}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
