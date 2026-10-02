use super::*;

const CASE_CLONE: &str = r#"
data Payload { value: u64; }
machine Payload::clone(source: &Payload) -> Payload { Payload { value: source.value } }
data Sum { case Found(payload: Payload); case Other(payload: Payload); case Empty; }
machine Sum::clone(source: &Sum) -> Sum {
    transition source {
        Sum::Found { payload } -> (Sum::Found { payload: Payload::clone(payload) })
        Sum::Other { payload } -> (Sum::Other { payload: Payload::clone(payload) })
        Sum::Empty -> (Sum::Empty)
    }
}
data Envelope { choice: Sum; }
machine inspect(source: &Envelope) {}
"#;

#[test]
fn case_clone_retains_the_structural_return_graph() {
    let checked = crate::tests::front_end::checked_program(CASE_CLONE);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Sum::clone")
        .unwrap();
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine.symbol)
        .expect("both case payload calls retain their structural result producer");
    assert!(matches!(
        plan.result,
        checked_trees::CheckedControlResultPlan::Structural(_)
    ));
}

fn with_case_argument(check: impl FnOnce(ParameterArgument<'_>, &[(SymbolHandle, SymbolHandle)])) {
    let program = crate::tests::front_end::typed_program(CASE_CLONE);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let root = program.state_parameters(state)[0].symbol;
    let owner = |name: &str| {
        program
            .data_definitions()
            .iter()
            .find(|declaration| declaration.name.as_str() == name)
            .unwrap()
    };
    let checked_trees::data::DataMember::Field(choice) =
        &program.data_members(owner("Envelope"))[0]
    else {
        panic!("record field");
    };
    let cases = program
        .data_members(owner("Sum"))
        .iter()
        .filter_map(|member| match member {
            checked_trees::data::DataMember::Variant(case)
                if !program.data_payload_fields(case).is_empty() =>
            {
                Some((case.symbol, program.data_payload_fields(case)[0].symbol))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let clone_machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Payload::clone")
        .unwrap();
    let target =
        program.state_parameters(&program.machine_states(clone_machine)[0])[0].type_reference;
    let target_identity = base_type_identity(&program, target, &[]).unwrap();
    let place = crate::flow::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(root),
        segments: vec![
            facts::PlaceSegment::Field {
                symbol: choice.symbol,
            },
            facts::PlaceSegment::Case {
                variant: cases[0].0,
            },
            facts::PlaceSegment::Field { symbol: cases[0].1 },
        ],
    };
    check(
        ParameterArgument {
            program: &program,
            machine: machine.symbol,
            state: state.symbol,
            statement_index: 0,
            place: &place,
            target,
            target_identity: &target_identity,
        },
        &cases,
    );
}

fn shared_custody() -> ArgumentCustody {
    ArgumentCustody {
        access: SharedBorrow,
        multiplicity: Multiplicity::Unrestricted,
        unqualified: true,
        target_access: Some(SharedBorrow),
    }
}

#[test]
fn case_clone_static_loan_preserves_exact_nested_case_and_field() {
    with_case_argument(|argument, cases| {
        fn admit(argument: ParameterArgument<'_>) -> Option<ArgumentPath> {
            ordinary_parameter_path(
                argument,
                shared_custody(),
                MachineSupplyMode::CheckedBody,
                true,
            )
        }
        assert_eq!(
            admit(argument).unwrap(),
            vec![
                CheckedUnitStructuralPathSegment::Field("choice".into()),
                CheckedUnitStructuralPathSegment::Case("Found".into()),
                CheckedUnitStructuralPathSegment::Field("payload".into()),
            ]
        );
        let mut place = argument.place.clone();
        place.segments[1] = facts::PlaceSegment::Case {
            variant: cases[1].0,
        };
        assert!(
            admit(ParameterArgument {
                place: &place,
                ..argument
            })
            .is_none(),
            "sibling case cannot authorize the first payload field"
        );
        place.segments[2] = facts::PlaceSegment::Field { symbol: cases[1].1 };
        assert!(
            matches!(admit(ParameterArgument { place: &place, ..argument }).unwrap().as_slice(), [_, CheckedUnitStructuralPathSegment::Case(case), _] if case == "Other")
        );
        place.segments.pop();
        assert!(
            admit(ParameterArgument {
                place: &place,
                ..argument
            })
            .is_none(),
            "a case without its declared field supplies no payload"
        );
        assert_ne!(cases[0].1, cases[1].1);
    });
}

#[test]
fn case_clone_static_loan_does_not_grant_transfer_or_access() {
    with_case_argument(|argument, _| {
        for custody in [
            ArgumentCustody {
                target_access: Some(MutableBorrow),
                ..shared_custody()
            },
            ArgumentCustody {
                access: WriteOnlyBorrow,
                ..shared_custody()
            },
            ArgumentCustody {
                access: Owned,
                target_access: Some(Owned),
                multiplicity: Multiplicity::Affine,
                ..shared_custody()
            },
            ArgumentCustody {
                unqualified: false,
                ..shared_custody()
            },
        ] {
            assert!(
                ordinary_parameter_path(argument, custody, MachineSupplyMode::CheckedBody, true)
                    .is_none()
            );
        }
        assert!(
            ordinary_parameter_path(
                argument,
                shared_custody(),
                MachineSupplyMode::Boundary,
                true
            )
            .is_none()
        );
        assert!(boundary_parameter_path(argument, shared_custody(), &[]).is_none());
    });
}
