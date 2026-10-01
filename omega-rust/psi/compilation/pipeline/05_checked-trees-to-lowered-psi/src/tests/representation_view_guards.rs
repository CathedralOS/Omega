//! Retained scalar re-views produce and independently verify.
//!
//! A recast read reaches Terminal Psi as the viewed field's own observation at
//! its declared carrier followed by the replayed view: equal-width integer
//! views read the modular image, so the verifier never meets a field read whose
//! carrier its declaration does not name. Boolean and IEEE float cross-carrier
//! views retain a Terminal re-view instead of retagging the field read.
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
        machine Main::main(context: &mut Main) {
            let bits: &u32 = &context.value as &u32;
            transition bits == 4294967294 && (&context.small as &u8) == 200 {
                true -> good(context)
                _ -> bad(context)
            }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
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
fn boolean_and_float_recast_reads_retain_terminal_bit_views() {
    for source in [
        r#"
        data Main { flag: bool; }
        machine Main::main(context: &mut Main) {
            let bits: &u8 = &context.flag as &u8;
            transition bits == 1 { true -> good(context) _ -> bad(context) }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
        }
        "#,
        r#"
        data Main { value: f32; }
        machine Main::main(context: &mut Main) {
            let bits: &u32 = &context.value as &u32;
            transition bits == 0 { true -> good(context) _ -> bad(context) }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
        }
        "#,
    ] {
        let module = produce(source).expect("cross-family re-views retain their stored bits");
        assert!(
            module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    operation.kind,
                    terminal_psi::OperationKind::ScalarRepresentationView { .. }
                ))
        );
    }
}

#[test]
fn byte_boolean_guards_retain_and_discharge_representation_landing() {
    let module = produce(
        r#"
        data Main where byte <= 1, { byte: u8; }
        machine Main::main(context: &mut Main) {
            let flag: &bool = &context.byte as &bool;
            transition flag && (&context.byte as &bool) {
                true -> good(context)
                _ -> bad(context)
            }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
        }
        "#,
    )
    .expect("byte-to-bool guards prove the source byte belongs to the bool representation");
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::ScalarRepresentationView {
                    landing_obligation: Some(_),
                    ..
                }
            ))
    );
}

/// A shared record-as-record view erases at Terminal: each view field reads
/// the source leaf at the same offset under the source's own declaration, so
/// the verifier checks an ordinary field read of storage the source declares.
/// The unread `f32` leaves keep the geometry comparison honest; float
/// comparison needs the library operator this front end does not load.
#[test]
fn shared_record_view_reads_rejoin_source_leaves_and_verify() {
    produce(
        r#"
        data Narrow where count >= 0 && count <= 10, { count: i32; ratio: f32; }
        data Wide where total >= 0 && total <= 20, { total: i32; scale: f32; }
        data Main { narrow: Narrow; }
        machine Main::main(context: &mut Main) {
            let view: &Wide = &context.narrow as &Wide;
            transition view.total == context.narrow.count {
                true -> good(context)
                _ -> bad(context)
            }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
        }
        "#,
    )
    .expect("record view reads lower as source leaf reads and verify");
}

/// A view field over an equal-width integer leaf of the other sign reads the
/// leaf at its declared carrier and takes the modular image, and a mutable
/// view stores the image of the view's value back into that leaf.
#[test]
fn record_view_integer_leaves_of_the_other_sign_read_and_store_their_image() {
    produce(
        r#"
        data Signed where value >= 0 && value <= 100, { value: i16; flag: bool; }
        data Unsigned where value >= 0 && value <= 100, { value: u16; active: bool; }
        data Main { signed: Signed; }
        machine Main::main(context: &mut Main) {
            context.signed.value = 7;
            let view: &mut Unsigned = &mut context.signed as &mut Unsigned;
            view.value = 99;
            view.active = true;
            let seen: u16 = view.value;
            transition seen == 99 && context.signed.value == 99 && context.signed.flag {
                true -> good(context)
                _ -> bad(context)
            }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
        }
        "#,
    )
    .expect("sign-changing record view leaves lower through the modular image and verify");
}

/// A record view field reads its source carrier before the same Terminal bit
/// re-view used by whole scalar places.
#[test]
fn record_view_leaf_with_another_carrier_kind_verifies() {
    let module = produce(
        r#"
        data Reading { value: f32; }
        data Bits { value: u32; }
        data Main { reading: Reading; }
        machine Main::main(context: &mut Main) {
            let view: &Bits = &context.reading as &Bits;
            transition view.value == 0 {
                true -> good(context)
                _ -> bad(context)
            }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
        }
        "#,
    )
    .expect("record leaf bit views replay through the scalar operation");
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::ScalarRepresentationView { .. }
            ))
    );
}

#[test]
fn scalar_loan_operands_with_data_predicates_produce_and_verify() {
    for source in [
        r#"
        boundary operator == Float::equal(left: f32, right: f32) -> bool;
        data Main where value >= 0.0 && value <= 1.0, { value: f32; }
        machine Main::main(context: &mut Main) {
            transition context.value == 0.0f32 { true -> good(context) _ -> bad(context) }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
        }
        "#,
        r#"
        boundary operator == Float::equal(left: f32, right: f32) -> bool;
        data Main where value >= 0.0 && value <= 1.0, { value: f32; }
        machine Main::main(context: &mut Main) {
            let wide: &f32 = &context.value;
            transition wide == 0.0f32 { true -> good(context) _ -> bad(context) }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
        }
        "#,
        r#"
        boundary operator == Float::equal(left: f32, right: f32) -> bool;
        data Main where value >= 0.0 && value <= 1.0, { value: f32; }
        machine Main::main(context: &mut Main) {
            let wide: &f32 = &context.value as &f32;
            let bits: &u32 = &context.value as &u32;
            transition wide == 0.0f32 && bits == 0 { true -> good(context) _ -> bad(context) }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
        }
        "#,
        r#"
        boundary operator == Float::equal(left: f32, right: f32) -> bool;
        data Main { value: f32; }
        machine Main::main(context: &mut Main) {
            let wide: &mut f32 = &mut context.value;
            transition wide == 0.0f32 { true -> good(context) _ -> bad(context) }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
        }
        "#,
        r#"
        data Main where byte >= 0 && byte <= 1, { flag: bool; byte: u8; }
        machine Main::main(context: &mut Main) {
            context.flag = true;
            context.byte = 1;
            let flag_bits: &u8 = &context.flag as &u8;
            let byte_flag: &bool = &context.byte as &bool;
            transition flag_bits == 1 && byte_flag { true -> good(context) _ -> bad(context) }
            state good(context: &mut Main) {}
            state bad(context: &mut Main) {}
        }
        "#,
    ] {
        produce(source).unwrap_or_else(|error| {
            panic!("scalar loan source {source} failed: {error}");
        });
    }
}

#[test]
fn scalar_operator_projection_never_reads_a_write_only_reference() {
    let errors = crate::front_end::checked_program_result(
        r#"
        boundary operator == Float::equal(left: f32, right: f32) -> bool;
        machine observe(value: &write f32) -> bool { value == 0.0f32 }
        "#,
    )
    .expect_err("an operator cannot read through write-only authority");
    assert!(
        errors
            .iter()
            .any(|error| error.to_string().contains("write-only")),
        "{errors:?}"
    );
}
