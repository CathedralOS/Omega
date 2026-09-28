//! Source-produced attached receiver stores through canonical Terminal admission.

use checked_trees::CheckedUnitEffectOperationPlan;
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};
use terminal_psi::StructuralAccess;

#[path = "receiver_scalar_store_source/store_custody.rs"]
mod store_custody;

const SOURCE: &str = r#"
    data Pair { prefix: u8; value: u16; }
    data Inner { prefix: u8; value: u16; }
    data Outer { prefix: u8; inner: Inner; }
    data Cell [copy] { prefix: u8; value: u16; }
    data Matrix { prefix: u8; cells: [Cell; 3]; }

    machine Pair::direct(&mut self) {
        self.value = 17;
    }

    machine Outer::nested(&mut self) {
        self.inner.value = 19;
    }

    machine Matrix::indexed(&mut self) {
        self.cells[2].value = 29;
    }

    machine Pair::parameter(&mut self, replacement: u16) {
        self.value = replacement;
    }
"#;

#[test]
fn canonical_verifier_rejects_shared_access_substituted_for_mutable_receiver() {
    let checked = crate::front_end::checked_program(SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Pair::direct"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    let mut module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    entry.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        terminal_verifier::validate_module(&module),
        Err(terminal_verifier::ModuleError::InvalidStructuralScalarFieldStore { .. })
    ));
}

#[test]
fn ranged_field_store_proves_a_guard_bounded_binary_operand() {
    // `y = p + dir` where `p: [0..=8]` is declared and `dir` is bounded only
    // by the sole incoming edge guard `dir >= 0 && dir <= 1`. The target
    // state's seeded environment carries both guard conjuncts as
    // unconditional facts, so the [0..=9] store obligation composes the
    // declared endpoint with the guard endpoint through one exact add whose
    // operands reach their bounds only along cited field-read equalities.
    let source = r#"
        data Main { p: i32 [0..=8]; dir: i32; y: i32 [0..=9]; }
        machine Main::main(&mut self) {
            self.p = 8;
            self.dir = 1;
            transition self.dir >= 0 && self.dir <= 1 {
                true -> store()
                false -> bad()
            }
            state store(&mut self) {
                self.y = self.p + self.dir;
            }
            state bad(&mut self) {}
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the guard-bounded operand proves the sum into the declared range")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
}

#[test]
fn ranged_field_store_rejects_a_guard_bounded_sum_past_the_field() {
    // Same shape with `y: [0..=8]`: the cited endpoints only reach
    // `8 + 1 = 9`, so the closure must not relax the sum into the field.
    let source = r#"
        data Main { p: i32 [0..=8]; dir: i32; y: i32 [0..=8]; }
        machine Main::main(&mut self) {
            self.p = 8;
            self.dir = 1;
            transition self.dir >= 0 && self.dir <= 1 {
                true -> store()
                false -> bad()
            }
            state store(&mut self) {
                self.y = self.p + self.dir;
            }
            state bad(&mut self) {}
        }
    "#;
    let diagnostics = match crate::front_end::checked_program_result(source) {
        Ok(_) => panic!("a guarded sum wider than the field cannot prove the store"),
        Err(diagnostics) => diagnostics,
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove assignment value `self.p + self.dir`")
            && diagnostic.message.contains("expected 0..=8")
    }));
}
