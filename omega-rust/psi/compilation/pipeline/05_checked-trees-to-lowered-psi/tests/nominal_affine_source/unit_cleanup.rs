//! Fixtures shared by the unit cleanup tests: the nominal cleanup sources.

#[path = "unit_cleanup/contextual_and_multi_root_cleanups.rs"]
mod contextual_and_multi_root_cleanups;

const SCALAR_SOURCE: &str = r#"
    data Token { flag: bool; tag: u8; delta: i16; payload: u64; address: addr; }
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const FINITE_CONTEXTUAL_SOURCE: &str = r#"
    data Token { ready: bool; audited: bool; armed: bool; }
    machine Token::drop(&mut self)
    requires
        self.armed;
        self.ready
    {}

    data Root {}
    machine Root::enter(token: Token)
    requires
        token.armed;
        token.audited;
        token.ready
    {}
"#;

#[test]
fn nominal_cleanup_selection_retains_named_and_legacy_borrowed_subjects() {
    for (subject, receiver) in [("token: &mut Token", "token"), ("&mut self", "self")] {
        let source = format!(
            r#"
            data Helper {{}}
            machine Helper::note(value: bool) {{}}
            data Token {{ ready: bool; }}
            machine Token::drop({subject}) requires {receiver}.ready {{
                Helper::note({receiver}.ready);
                {receiver}.ready = false;
            }}
            data Root {{}}
            machine Root::enter(token: Token) requires token.ready {{}}
            "#
        );
        let checked = crate::front_end::checked_program(&source);
        let root = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::enter")
            .expect("authored consuming caller");
        let nominal = checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .machines
            .iter()
            .find(|plan| plan.machine.machine == root.symbol)
            .unwrap_or_else(|| panic!("the {subject} hook must select a nominal cleanup plan"));
        let [cleanup] = nominal.cleanups.as_slice() else {
            panic!("one owned argument has one selected cleanup action")
        };
        let hook = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .find(|machine| machine.machine == cleanup.cleanup_machine)
            .expect("selected hook retains its ordinary checked body");
        let [parameter] = hook.structural_parameters.as_slice() else {
            panic!("the hook body reads and writes its single borrowed subject")
        };
        assert_eq!(parameter.type_identity, cleanup.type_identity);
        assert_eq!(parameter.position, 0);
        assert_eq!(
            parameter.access,
            checked_trees::CheckedStructuralAccess::MutableBorrow,
        );
        assert!(hook.operations.iter().any(|operation| matches!(
            operation,
            checked_trees::CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
        )));

        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            checked_trees_to_lowered_psi::TerminalMachineSelection::Name("Root::enter"),
        )
        .expect("both subject spellings retain executable nominal cleanup");
        let module = &lowered.semantic_module;
        terminal_verifier::verify_module(
            module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("independent verification rejoins the exact owner and receiver");
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let argument = &entry.structural_parameters[0];
        let terminal_psi::StructuralTypeShape::Record { fields } = &module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == argument.structural_type)
            .unwrap()
            .shape
        else {
            panic!("Token remains a record")
        };
        let field = fields[0].id;
        let terminal_psi::Terminator::ReturnUnitNominalAffine { cleanups, .. } =
            &entry.blocks[0].terminator
        else {
            panic!("one exact nominal return")
        };
        let cleanup = &cleanups[0];
        let target = module
            .machines
            .iter()
            .find(|machine| machine.id == cleanup.cleanup_machine)
            .unwrap();
        assert_eq!(target.attachment, Some(argument.structural_type));
        assert_eq!(
            cleanup.cleanup_receiver,
            Some(target.structural_parameters[0].place)
        );
        let semantics = terminal_codec::encode_module(module).unwrap();
        assert_eq!(terminal_codec::decode_module(&semantics).unwrap(), *module);
        let proof = terminal_codec::encode_proof_section(module, &lowered.proof_bundle).unwrap();
        let arguments = [terminal_interpreter::TerminalStructuralValue {
            opaque_identity: 1,
            structural_type: argument.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }];
        let booleans = [terminal_interpreter::TerminalStructuralBooleanFieldValue {
            argument_index: 0,
            path: Vec::new(),
            field,
            value: true,
        }];
        let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
            &semantics,
            &proof,
            &proof_admission::AdmissionProfile::default(),
            &[],
            terminal_interpreter::TerminalStructuralInputs {
                arguments: &arguments,
                boolean_fields: &booleans,
                ..Default::default()
            },
        )
        .unwrap();
        let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
        let mut complete = false;
        for _ in 0..64 {
            let status = execution
                .resume(&mut meter, &mut terminal_interpreter::AcceptTerminalEffects)
                .unwrap();
            match status {
                terminal_interpreter::TerminalExecutionStatus::Complete(value) => {
                    assert_eq!(value, terminal_interpreter::TerminalExecutionResult::Unit);
                    complete = true;
                    break;
                }
                terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_) => {
                    let usage = meter.usage().clone();
                    assert!(matches!(
                        execution
                            .resume(&mut meter, &mut terminal_interpreter::AcceptTerminalEffects)
                            .unwrap(),
                        terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_)
                    ));
                    assert_eq!(
                        *meter.usage(),
                        usage,
                        "retry without fuel repeats no action"
                    );
                    meter.replenish(1).unwrap();
                }
                other => panic!("unexpected hook status: {other:?}"),
            }
        }
        assert!(complete, "finite hook execution completes");
        for machine in &module.machines {
            for block in &machine.blocks {
                assert_eq!(
                    meter
                        .usage()
                        .at(terminal_fuel::FuelChargeSite::Edge(block.terminator.edge()))
                        .unwrap()
                        .executions(),
                    1,
                    "each hook/helper return executes once"
                );
                for operation in &block.operations {
                    assert_eq!(
                        meter
                            .usage()
                            .at(terminal_fuel::FuelChargeSite::Operation(operation.id))
                            .unwrap()
                            .executions(),
                        1,
                        "field read, helper call and field store execute once"
                    );
                }
            }
        }
        for mutation in [
            "missing owner",
            "missing receiver",
            "shared receiver",
            "missing requirement",
            "missing caller premise",
            "duplicate cleanup",
        ] {
            let mut forged = module.clone();
            let target = forged
                .machines
                .iter_mut()
                .find(|machine| machine.id == cleanup.cleanup_machine)
                .unwrap();
            match mutation {
                "missing owner" => target.attachment = None,
                "shared receiver" => {
                    target.structural_parameters[0].access =
                        terminal_psi::StructuralAccess::SharedBorrow
                }
                _ => {}
            }
            let entry = forged
                .machines
                .iter_mut()
                .find(|machine| machine.id == module.entry)
                .unwrap();
            if mutation == "missing caller premise" {
                entry.contract.requires.clear();
            }
            let terminal_psi::Terminator::ReturnUnitNominalAffine { cleanups, .. } =
                &mut entry.blocks[0].terminator
            else {
                unreachable!()
            };
            match mutation {
                "missing receiver" => cleanups[0].cleanup_receiver = None,
                "duplicate cleanup" => cleanups.push(cleanups[0].clone()),
                "missing requirement" => cleanups[0].requirement_obligations.clear(),
                _ => {}
            }
            assert!(
                terminal_verifier::verify_module(
                    &forged,
                    &lowered.proof_bundle,
                    &proof_admission::AdmissionProfile::default()
                )
                .is_err(),
                "{mutation} must reject"
            );
        }
    }
}
