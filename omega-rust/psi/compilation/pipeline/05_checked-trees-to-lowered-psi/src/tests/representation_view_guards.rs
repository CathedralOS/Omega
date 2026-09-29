//! Retained scalar re-views produce and independently verify.
//!
//! A recast read reaches Terminal Psi as the viewed field's own observation at
//! its declared carrier followed by the replayed view: equal-width integer
//! views read the modular image, so the verifier never meets a field read whose
//! carrier its declaration does not name. Boolean and IEEE float cross-carrier
//! views name the Terminal re-view vocabulary they are still owed instead of
//! retagging the field read.
// `TerminalMachineSelection` must come through `terminal_production`'s
// re-export, not `crate::`: in the lib test target `crate` is the test
// instance of this crate while `terminal_production` holds the library
// instance, and the two types never unify.
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

fn produce(source: &str) -> Result<terminal_psi::TerminalModule, String> {
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .map_err(|rejection| format!("{rejection}"))?
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .map_err(|error| format!("verify error: {error:?}"))?;
    Ok(module)
}

#[test]
fn equal_width_integer_recast_guards_read_declared_fields_and_verify() {
    let module = produce(
        r#"
        data Main { value: i32; small: i8; }
        machine Main::main(&mut self) {
            let bits: &u32 = &self.value as &u32;
            transition bits == 4294967294 && (&self.small as &u8) == 200 {
                true -> good()
                _ -> bad()
            }
            state good(&mut self) {}
            state bad(&mut self) {}
        }
        "#,
    )
    .expect("equal-width integer re-views replay as their modular image");
    let field_reads = module
        .machines
        .iter()
        .flat_map(|machine| machine.blocks.iter())
        .flat_map(|block| block.operations.iter())
        .filter_map(|operation| match operation.kind {
            terminal_psi::OperationKind::IntegerStructuralField { .. } => operation
                .result
                .scalar_ref()
                .map(|result| result.scalar_type),
            _ => None,
        })
        .collect::<Vec<_>>();
    let signed = |bits| {
        semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, bits)
                .unwrap(),
        )
    };
    // Both viewed fields are observed at their declared signed carriers.
    assert!(field_reads.contains(&signed(32)), "{field_reads:?}");
    assert!(field_reads.contains(&signed(8)), "{field_reads:?}");
    assert!(
        field_reads
            .iter()
            .all(|read| *read == signed(32) || *read == signed(8)),
        "no field read is retagged to its view carrier: {field_reads:?}"
    );
}

#[test]
fn boolean_and_float_recast_reads_name_the_missing_terminal_re_view() {
    for source in [
        r#"
        data Main { flag: bool; }
        machine Main::main(&mut self) {
            let bits: &u8 = &self.flag as &u8;
            transition bits == 1 { true -> good() _ -> bad() }
            state good(&mut self) {}
            state bad(&mut self) {}
        }
        "#,
        r#"
        data Main { value: f32; }
        machine Main::main(&mut self) {
            let bits: &u32 = &self.value as &u32;
            transition bits == 0 { true -> good() _ -> bad() }
            state good(&mut self) {}
            state bad(&mut self) {}
        }
        "#,
    ] {
        let error = produce(source).expect_err("no Terminal re-view spells this pair yet");
        assert!(error.contains("no Terminal Psi vocabulary"), "{error}");
    }
}
