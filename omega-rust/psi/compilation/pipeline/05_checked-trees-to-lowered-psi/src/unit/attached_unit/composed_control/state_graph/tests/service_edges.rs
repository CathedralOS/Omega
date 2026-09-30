//! A state edge preserves the authored Binding occurrence, not just its type.
use super::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlTerminatorPlan, admission,
};
use checked_trees::{
    CheckedStructuralAccess, CheckedStructuralControlTransferSourcePlan, CheckedTrees,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use source::{SourceMap, SourceOrigin};
use std::path::PathBuf;

fn fixture(projected: bool) -> CheckedTrees {
    let binding = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/binding.omg"
    ));
    let body = if projected {
        "machine route(first: &mut Context, second: &mut Context, value: u64) reaches Host {
            transition { _ -> pass(value, &mut first.left) }
            state pass(value: u64, host: &mut Binding<Host>) {
                transition { _ -> finish(host, value) }
            }
            state finish(host: &mut Binding<Host>, value: u64) {}
        }"
    } else {
        "machine route(first: &mut Binding<Host>, second: &mut Binding<Host>, value: u64) reaches Host {
            transition { _ -> finish(value, first) }
            state finish(value: u64, host: &mut Binding<Host>) {}
        }"
    };
    let program = format!(
        "pub boundary trait Host {{ machine mark(value: u64); }}
        pub data Context {{ left: Binding<Host>; right: Binding<Host>; }} {body}"
    );
    let mut sources = SourceMap::default();
    let binding_source = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/binding.omg"),
            binding.into(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let program_source = sources
        .add(PathBuf::from("tests/main.omg"), program.clone())
        .source_id;
    let mut typed = crate::front_end::typed_program_from_source_map(
        sources,
        &[(binding_source, binding), (program_source, &program)],
    );
    let authorizations = typed
        .traits()
        .iter()
        .filter(|row| row.is_boundary)
        .map(
            |row| typed_trees::typed_trees::FusedServiceErasureAuthorization {
                requirement: row.symbol,
            },
        )
        .collect();
    typed
        .bind_fused_service_erasures(authorizations)
        .expect("selected Fused fixture");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("service state source checks")
}

fn plan(checked: &CheckedTrees) -> CheckedComposedUnitControlMachinePlan {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "route")
        .expect("route source")
        .symbol;
    checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "service successor graph omitted: {:?}",
                checked.facts.flow.terminal_unit_effects.omissions
            )
        })
        .clone()
}

#[test]
fn binding_state_edges_lower_and_verify_exact_loans() {
    for projected in [false, true] {
        let checked = fixture(projected);
        let plan = plan(&checked);
        admission::admit(&checked, &plan).expect("state-local receipts and exact loan operands");
        let lowered =
            crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("route"))
                .expect("service state edges lower");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("service state loans verify");
        assert!(
            lowered
                .semantic_module
                .structural_types
                .iter()
                .any(|row| matches!(
                    row.shape,
                    terminal_psi::StructuralTypeShape::FusedServiceBinding
                ))
        );
    }
}

#[test]
fn binding_projected_state_edges_reject_substituted_custody() {
    let checked = fixture(true);
    let original = plan(&checked);
    admission::admit(&checked, &original).expect("unmodified projected loan");
    for mutation in 0..7 {
        let mut changed = original.clone();
        if mutation >= 4 {
            let parameter = &mut changed.states[1].structural_parameters[0];
            match mutation {
                4 => parameter.fused_service_erasure = None,
                5 => {
                    parameter
                        .fused_service_erasure
                        .as_mut()
                        .unwrap()
                        .source_parameter = changed.machine
                }
                6 => parameter.position = 0,
                _ => unreachable!(),
            }
        } else {
            let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
                &mut changed.states[0].terminator
            else {
                panic!("projected jump");
            };
            let transfer = &mut successor.transfers[0];
            let CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } =
                &mut transfer.source
            else {
                panic!("field loan transfer");
            };
            match mutation {
                0 => subject.path = vec![CheckedUnitStructuralPathSegment::Field("right".into())],
                1 => {
                    subject.source =
                        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 }
                }
                2 => transfer.target_parameter_index = 1,
                3 => subject.access = CheckedStructuralAccess::SharedBorrow,
                _ => unreachable!(),
            }
        }
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "mutation={mutation}"
        );
    }
}

#[test]
fn binding_whole_state_edges_reject_same_type_source_swaps() {
    let checked = fixture(false);
    let mut changed = plan(&checked);
    admission::admit(&checked, &changed).expect("original whole Binding edge");
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut changed.states[0].terminator
    else {
        panic!("whole jump");
    };
    successor.transfers[0].source =
        CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 };
    assert!(admission::admit(&checked, &changed).is_err());
}
