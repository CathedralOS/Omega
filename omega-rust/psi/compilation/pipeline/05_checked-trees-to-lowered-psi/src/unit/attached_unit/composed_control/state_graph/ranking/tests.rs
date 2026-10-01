const WALK: &str = "
    data Root {}
    machine Root::walk(&mut self, items: &[u32])
    terminates by items -> Slice::Length; {
        transition items.len > 0 {
            true -> walk(items[1..])
            false -> done()
        }
        state done(&mut self) {}
    }
";

#[test]
fn element_view_rank_retains_its_length_carrier() {
    let checked = crate::front_end::checked_program(WALK);
    let lowered = crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Root::walk"),
    )
    .expect("a shrinking element view retains an element-count rank");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the rank is independently proved from the arriving element view");
}

#[test]
fn element_view_rank_rejects_the_previous_descriptor_as_arrival() {
    let checked = crate::front_end::checked_program(WALK);
    let mut lowered = crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Root::walk"),
    )
    .expect("valid element-view rank");
    let machine = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.ranked_scc.is_some())
        .expect("ranked walk");
    let Some(terminal_psi::TerminalRankedScc::Natural(components)) = &machine.ranked_scc else {
        panic!("natural rank");
    };
    let edge = components[0]
        .edges
        .iter()
        .find(|edge| edge.comparison == terminal_psi::TerminalNaturalRankComparison::Strict)
        .expect("decreasing tail edge");
    let successor_rank = edge.successor_rank;
    let entry_view = machine
        .structural_parameters
        .iter()
        .find(|parameter| {
            lowered
                .semantic_module
                .structural_types
                .iter()
                .any(|declaration| {
                    declaration.id == parameter.structural_type
                        && matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::ElementView { .. }
                        )
                })
        })
        .expect("original element descriptor")
        .place;
    let operation = machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|value| value.id == successor_rank)
        })
        .expect("arriving rank observation");
    let terminal_psi::OperationKind::ElementViewLength { source } = &mut operation.kind else {
        panic!("element-count observation");
    };
    assert_ne!(
        *source, entry_view,
        "the tail has a new descriptor identity"
    );
    *source = entry_view;
    assert!(
        matches!(
            terminal_verifier::validate_module(&lowered.semantic_module),
            Err(terminal_verifier::ModuleError::InvalidRankedScc(_))
        ),
        "a dominating but different descriptor does not prove the arriving rank"
    );
}

#[test]
fn element_view_rank_requires_its_decrease_evidence() {
    let checked = crate::front_end::checked_program(WALK);
    let mut lowered = crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Root::walk"),
    )
    .expect("valid element-view rank");
    assert!(!lowered.proof_bundle.control_cycles.is_empty());
    lowered.proof_bundle.control_cycles.clear();
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "correct length carriers alone are not termination evidence"
    );
}
