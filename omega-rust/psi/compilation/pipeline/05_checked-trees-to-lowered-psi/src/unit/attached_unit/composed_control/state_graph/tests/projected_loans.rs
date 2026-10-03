//! State arguments retain the same declared loan regardless of borrow spelling.
use super::admission;
use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedStructuralAccess,
    CheckedStructuralControlTransferSourcePlan, CheckedUnitStructuralPathSegment,
};

#[test]
fn shared_state_loans_compose_with_case_selection_and_forwarding() {
    for operand in ["payload", "&payload"] {
        let source = format!(
            "data Record {{ value: u64; }}
             data Envelope {{ case Present(payload: Record); case Other(payload: Record); case Missing; }}
             machine route(envelope: &Envelope) -> u64 {{
                 transition envelope {{
                     Envelope::Present {{ payload }} -> observe({operand})
                     _ -> (0)
                 }}
                 state observe(record: &Record) -> u64 {{
                     transition {{ _ -> finish(record) }}
                 }}
                 state finish(record: &Record) -> u64 {{ record.value }}
             }}"
        );
        let checked = crate::front_end::checked_program(&source);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "route")
            .expect("route");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine.symbol)
            .expect("loan state graph");
        assert!(
            matches!(
                plan.states[1].terminator.successors()[0].transfers[0].source,
                CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
            ),
            "a whole forwarded parameter retains its existing loan transport"
        );
        let lowered =
            crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("route"))
                .unwrap_or_else(|error| panic!("{operand}: {error:?}"));
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("case membership, source storage and forwarded loan verify");
        // Matching payload types do not establish the selected case. Corrupt
        // the source-free continuation, before proof-envelope identity checks,
        // to name the sibling case's equally typed field instead.
        let mut module = lowered.semantic_module.clone();
        let other = module
            .structural_types
            .iter()
            .find_map(|declaration| {
                let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
                    return None;
                };
                cases.iter().find(|case| case.identity == "Other")
            })
            .expect("sibling case");
        let replacement = vec![
            terminal_psi::StructuralPathSegment::Case(other.id),
            terminal_psi::StructuralPathSegment::Field(other.fields[0].identity.clone()),
        ];
        let argument = module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .find_map(|block| {
                let arguments = match &mut block.terminator {
                    terminal_psi::Terminator::Jump {
                        structural_arguments,
                        ..
                    } => structural_arguments,
                    terminal_psi::Terminator::Conditional { when_true, .. } => {
                        &mut when_true.structural_arguments
                    }
                    _ => return None,
                };
                arguments.iter_mut().find(|argument| {
                    argument.path.iter().any(|segment| {
                        matches!(segment, terminal_psi::StructuralPathSegment::Case(_))
                    })
                })
            })
            .expect("case-qualified loan on a continuation");
        argument.path = replacement;
        terminal_verifier::validate_module(&module)
            .expect("the sibling payload has identical type and access");
        assert!(
            matches!(
                terminal_verifier::verify_module(
                    &module,
                    &lowered.proof_bundle,
                    &proof_admission::AdmissionProfile::default(),
                ),
                Err(terminal_verifier::VerificationError::Module(
                    terminal_verifier::ModuleError::InvalidStructuralSuccessorArgument { .. }
                ))
            ),
            "a same-typed sibling payload requires its own active-case evidence"
        );
    }
}

#[test]
fn implicit_field_state_loans_reject_substituted_paths_and_access() {
    let checked = crate::front_end::checked_program(
        "data Record { value: u64; }
         data Pair { left: Record; right: Record; }
         machine route(pair: &Pair) -> u64 {
             transition { _ -> finish(pair.left) }
             state finish(record: &Record) -> u64 { record.value }
         }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "route")
        .expect("route");
    let original = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine.symbol)
        .expect("implicit field loan has a composed graph");
    admission::admit(&checked, original).expect("declared shared projection");
    let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("route"))
        .expect("ordinary field projection uses the same loan route");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("ordinary projected loan verifies");
    for changed_access in [false, true] {
        let mut changed = original.clone();
        let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
            &mut changed.states[0].terminator
        else {
            panic!("entry jump")
        };
        let CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } =
            &mut successor.transfers[0].source
        else {
            panic!("projected loan")
        };
        if changed_access {
            subject.access = CheckedStructuralAccess::MutableBorrow;
        } else {
            subject.path = vec![CheckedUnitStructuralPathSegment::Field("right".into())];
        }
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "a shared loan cannot gain mutation or substitute a same-typed sibling"
        );
    }
}
