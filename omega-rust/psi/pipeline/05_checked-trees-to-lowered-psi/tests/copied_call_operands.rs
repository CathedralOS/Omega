//! Copy-valued fields are evaluated into owned call operands without moving
//! their borrowed backing storage.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_production::{
    TerminalProductionCustody, TerminalProductionRequest, TerminalProductionTimings,
};

const WITNESS: &str = r#"
    data SourceId [copy] { value: u32; }
    data Source { id: SourceId; length: u32; }
    data Holder { source: Source; }
    data Target { seen: u32; }
    machine Target::initialize(&mut self, id: SourceId, length: u64) {
        self.seen = id.value;
    }
    data Main { holder: Holder; target: Target; }
    machine Main::main(&mut self) {
        self.target.initialize(self.holder.source.id, self.holder.source.length as u64);
    }
"#;

#[test]
fn borrowed_record_field_is_copied_before_an_ordinary_call() {
    assert_copied_operand(WITNESS);
}

#[test]
fn copied_call_composes_with_surrounding_stores() {
    let source = WITNESS.replace(
        "self.target.initialize(self.holder.source.id, self.holder.source.length as u64);",
        "self.target.seen = 9;\n\
         self.target.initialize(self.holder.source.id, self.holder.source.length as u64);\n\
         self.holder.source.length = 7;",
    );
    assert_ne!(source, WITNESS);
    assert_copied_operand(&source);
}

const ASSIGNMENT_WITNESS: &str = r#"
    data SourceId [copy] { value: u32; }
    data Target { seen: u32; }
    machine Target::read(&self, id: SourceId) -> u32 { id.value }
    data Main { source: SourceId; target: Target; result: u32; }
    machine Main::main(&mut self) {
        self.target.seen = 9;
        self.result = self.target.read(self.source);
        self.target.seen = 7;
    }
"#;

#[test]
fn copied_argument_precedes_a_scalar_call_result_store() {
    let checked = crate::front_end::checked_program(ASSIGNMENT_WITNESS);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Target::read"),
    )
    .expect("the scalar callee has a standalone body");
    assert_copied_operand(ASSIGNMENT_WITNESS);
}

#[test]
fn copied_argument_precedes_a_structural_call_result_store() {
    let source = ASSIGNMENT_WITNESS
        .replace("-> u32 { id.value }", "-> SourceId { id }")
        .replace("result: u32", "result: SourceId");
    assert_copied_operand(&source);
}

#[test]
fn copied_call_result_stores_compose_with_state_transitions() {
    let scalar = ASSIGNMENT_WITNESS.replace(
        "self.target.seen = 7;",
        "transition { _ -> done() } state done(&mut self) { self.target.seen = 7; }",
    );
    assert_copied_operand(&scalar);
    let structural = scalar
        .replace("-> u32 { id.value }", "-> SourceId { id }")
        .replace("result: u32", "result: SourceId");
    assert_copied_operand(&structural);
}

#[test]
fn copied_argument_preserves_displaced_affine_field_cleanup() {
    let source = ASSIGNMENT_WITNESS
        .replace("data Target", "data BoxedId { value: u32; } data Target")
        .replace(
            "-> u32 { id.value }",
            "-> BoxedId { BoxedId { value: id.value } }",
        )
        .replace("result: u32", "result: BoxedId");
    assert_copied_operand(&source);
    assert_copied_operand(&source.replace(
        "self.target.seen = 7;",
        "transition { _ -> done() } state done(&mut self) { self.target.seen = 7; }",
    ));
}

#[test]
fn assignment_call_cannot_move_a_noncopy_borrowed_field() {
    let source = ASSIGNMENT_WITNESS.replace("SourceId [copy]", "SourceId");
    crate::front_end::checked_program_result(&source)
        .expect_err("a result store does not authorize moving its argument out of a borrow");
}

#[test]
fn copied_assignment_operand_rejects_substituted_call_coordinates() {
    for corrupt_formal in [false, true] {
        let mut checked = crate::front_end::checked_program(ASSIGNMENT_WITNESS);
        let source = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|plan| &mut plan.operations)
            .find_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    operand_source: Some(source),
                    ..
                } => Some(source),
                _ => None,
            })
            .expect("the copied argument retains its call coordinate");
        let checked_trees::CheckedArrayConstructionSource::CallArgument {
            call_ordinal,
            parameter_position,
        } = source
        else {
            panic!("the copied value belongs to a call argument");
        };
        if corrupt_formal {
            *parameter_position = 0;
        } else {
            *call_ordinal = u32::MAX;
        }
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Main::main"),
        )
        .expect_err("source replay must retain the exact call and formal");
    }
}

fn assert_copied_operand(source: &str) {
    let checked = crate::front_end::checked_program(source);
    let artifact =
        TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::main"))
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("an ordinary call accepts a copied field from its borrowed receiver")
            .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent verification accepts the copied operand and retained loan");
    let copies = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::StructuralLeafCopy { .. }
            )
        })
        .count();
    assert_eq!(copies, 1, "the argument has one fresh owned copy");
}

#[test]
fn noncopy_field_cannot_be_moved_out_of_a_borrowed_receiver() {
    let source = WITNESS.replace("SourceId [copy]", "SourceId");
    crate::front_end::checked_program_result(&source)
        .expect_err("removing copy semantics must reject the borrowed-field move");
}

#[test]
fn copied_operand_rejects_a_substituted_source_path() {
    let mut checked = crate::front_end::checked_program(WITNESS);
    let plans = &mut checked.facts.values.structural_values;
    let copy = plans
        .nodes
        .iter()
        .find_map(|(handle, node)| {
            matches!(
                node.kind,
                checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
            )
            .then_some(handle)
        })
        .expect("the call argument retains its checked copy source");
    let checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } =
        &mut plans.nodes.get_mut(copy).kind
    else {
        unreachable!();
    };
    assert!(!source.path.is_empty());
    source.path.clear();
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect_err("independent source replay rejects an exchanged copy path");
}
