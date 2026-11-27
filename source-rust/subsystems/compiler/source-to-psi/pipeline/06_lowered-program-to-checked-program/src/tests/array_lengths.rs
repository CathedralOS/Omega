//! The length of a fixed array, `array.length`. Read from source, as a Build
//! program: a Build that holds a table asks how many rows it has.
//!
//! domains.md: "`[u8; N] in D` is the same exact `N`-byte fixed-array carrier
//! as `[u8; N]`; its `.length` is `N`". Literal extents stay literal; symbolic
//! extents use their own constant input. The body and its guarantees must
//! describe that same value, whichever fields lead to the array.

use super::build_purpose::judge_build;
use super::*;

/// `accepted`, `rejected` with what the program could not establish, or the
/// refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected { unestablished, .. }) => match &unestablished[..] {
            [first, ..] => format!("rejected: {}", first.detail),
            [] => "rejected".to_owned(),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_length_of_a_fixed_array_is_the_extent_its_type_states() {
    // A field of data lent to the machine, and a field of a field.
    assert_eq!(
        verdict(
            "data Holder { slots: [u8; 4]; }
data Outer { holder: Holder; }
machine held(holder: @Holder) -> u64 ensures result == 4 { holder.slots.length }
machine nested(outer: @Outer) -> u64 ensures result == 4 { outer.holder.slots.length }
machine build_entry() { let outer: Outer; let seen: u64 = held(@outer.holder) + nested(@outer); }"
        ),
        "accepted"
    );
    // A local array.
    assert_eq!(
        verdict(
            "machine build_entry() -> u64 ensures result == 3 { let values: [u32; 3] = [1, 2, 3]; values.length }"
        ),
        "accepted"
    );
}

#[test]
fn a_guarantee_of_another_length_is_rejected() {
    // The same machine as above, promising one row more than the type has.
    let rejected = verdict(
        "data Holder { slots: [u8; 4]; }
machine held(holder: @Holder) -> u64 ensures result == 5 { holder.slots.length }
machine build_entry() { let holder: Holder; let seen: u64 = held(@holder); }",
    );
    assert!(rejected.starts_with("rejected"), "{rejected}");
}

#[test]
fn the_length_of_an_array_that_must_be_evaluated_first_is_refused() {
    // A call result is no place: the call would have to run for its length
    // to be read, and nothing is lowered for a length.
    assert_eq!(
        verdict(
            "machine make() -> [u8; 4] { [0, 0, 0, 0] }
machine build_entry() -> u64 { make().length }"
        ),
        "expression length of a fixed array that is not a field path while reading the body of \
         `build_entry`"
    );
}

fn rigid_array_form(text: &str) -> crate::lowering::Elaboration {
    let typed =
        crate::preparation::prepare_typed_trees(super::boundary_calls::typed(text), &[], &[]).unwrap();
    let entry = super::boundary_calls::entry(typed.trees());
    crate::lowering::elaborate_generic(typed.trees(), entry).unwrap()
}

#[test]
fn a_shared_array_element_keeps_its_generic_contents_when_returned() {
    let form = rigid_array_form(
        "machine entry<const Count: u64, Element [copy]>(items: @[Element; Count]) -> Element
         requires Count > 0 { items[Count - 1] }",
    );
    let result = judge(Ok(form)).unwrap();
    assert!(matches!(result, RouteVerdict::Accepted(_)), "{result:?}");
}

#[test]
fn a_rigid_array_extent_names_its_const_input_without_requiring_positivity() {
    for carrier in ["u64", "u32"] {
        let mut form = rigid_array_form(&format!(
            "machine entry<const N: {carrier}>(items: @[u8; N]) -> {carrier} ensures result == N {{ N }}"
        ));
        let parameters =
            terminal_check_tables::obligations::array_extent_parameters(&form.module).unwrap();
        assert_eq!(parameters, vec![(id(1), vec![ValueId::new(1).unwrap()])]);
        assert!(
            terminal_check_tables::obligations::integer_width_parameters(&form.module).unwrap()[0]
                .1
                .is_empty()
        );
        let found = discharge(&mut form, &mut Default::default()).unwrap();
        assert!(
            found
                .machines
                .iter()
                .all(|(_, machine)| machine.verdicts.iter().all(|(_, verdict)| verdict.is_ok())),
            "{found:?}"
        );
        assert!(terminal_codec::ClosedTerminalModule::new(form.module.clone()).is_err());
    }
}

#[test]
fn a_rigid_array_extent_cannot_name_another_machine_or_an_undeclared_input() {
    for foreign_owner in [false, true] {
        let mut form =
            rigid_array_form("machine entry<const N: u64>(items: @[u8; N]) -> u64 { N }");
        let extent = form
            .module
            .types.structural_types
            .iter_mut()
            .find_map(|declaration| match &mut declaration.shape {
                terminal_psi::StructuralTypeShape::FixedArray { length, .. } => Some(length),
                _ => None,
            })
            .unwrap();
        *extent = terminal_psi::ArrayExtent::Parameter {
            owner: id(if foreign_owner { 2 } else { 1 }),
            value: ValueId::new(if foreign_owner { 1 } else { 99 }).unwrap(),
        };
        assert!(discharge(&mut form, &mut Default::default()).is_err());
    }
}

fn array_subject(text: &str) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let typed =
        crate::preparation::prepare_typed_trees(super::boundary_calls::typed(text), &[], &[]).unwrap();
    let entry = super::boundary_calls::entry(typed.trees());
    crate::judge_compilation(
        crate::lowering::prepare_compilation(
            &typed,
            crate::lowering::Compilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[None],
                purpose: source_provenance::DependencyScope::Product,
                selected: &[],
                calling: None,
                providers: None,
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("fixture must lower to checker input")
        .program,
    )
}

#[test]
fn a_whole_subject_checks_the_original_nested_const_array_body() {
    for length in [0, 3] {
        let source = format!(
            "machine extent<const N: u64>(items: @[u8; N]) -> u64 {{ finish(items) machine finish(items: @[u8; N]) -> u64 {{ N }} }}\nmachine entry() -> u64 {{ let values: [u8; {length}]; extent(@values) }}"
        );
        let verdicts = array_subject(&source);
        assert!(
            verdicts
                .iter()
                .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
            "{verdicts:?}"
        );
        assert_array_invocation_value(verdicts, length);
    }
}

#[test]
fn a_specialized_call_checks_its_inferred_array_equation() {
    let source = r#"
        machine capacity<Array [copy], const Count: u64>(items: Array) -> u64
            requires Array == [u16; Count] { Count }
        machine entry() -> u64 {
            let items: [u16; 3] = [7, 11, 19];
            capacity(items)
        }
    "#;
    assert_array_invocation_value(array_subject(source), 3);
}

#[test]
fn a_closed_array_equation_cannot_prove_unequal_extents() {
    let failure = verdict("machine build_entry() ensures [u16; 2] == [u16; 3] {}");
    assert!(failure.starts_with("rejected"), "{failure}");
}

fn assert_array_invocation_value(
    mut verdicts: Vec<Result<RouteVerdict, crate::CompilationRefusal>>,
    expected: u128,
) {
    let Ok(RouteVerdict::Accepted(accepted)) = verdicts.remove(0) else {
        panic!("the invocation was refused");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program =
        terminal_interpreter::TerminalProgram::borrow(publication.artifact.view()).unwrap();
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(status, terminal_interpreter::TerminalExecutionStatus::Complete(
        terminal_interpreter::TerminalExecutionResult::Scalar(terminal_interpreter::TerminalScalarValue::Integer {
            value: semantic_vocabulary::IntegerValue::Unsigned(value), ..
        })) if value == expected),
        "{status:?}"
    );
}

#[test]
fn one_array_instance_cannot_authorize_a_generic_constant_extent_guarantee() {
    let source = "machine extent<const N: u64>(items: @[u8; N]) -> u64 ensures result == 3 { finish(items) machine finish(items: @[u8; N]) -> u64 { N } }\nmachine entry() -> u64 { let values: [u8; 3]; extent(@values) }";
    let verdicts = array_subject(source);
    assert!(verdicts.iter().any(|verdict| matches!(verdict, Ok(RouteVerdict::Rejected { unproved, .. }) if !unproved.is_empty())), "{verdicts:?}");
}

#[test]
fn a_rigid_array_index_needs_its_own_bound_including_a_literal_selector() {
    for (requirement, admitted) in [("requires N > 0u64", true), ("", false)] {
        let form = rigid_array_form(&format!(
            "machine entry<const N: u64>(items: @[u8; N]) -> u8 {requirement} {{ items[0] }}"
        ));
        let verdict = judge(Ok(form)).unwrap();
        if admitted {
            assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
        } else {
            assert!(
                matches!(verdict, RouteVerdict::Rejected { unproved, .. } if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. })))
            );
        }
    }
}

fn concrete_array_form(text: &str) -> crate::lowering::Elaboration {
    let typed = super::boundary_calls::typed(text);
    let entry = super::boundary_calls::entry(&typed);
    crate::lowering::elaborate_for(
        &typed,
        crate::lowering::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source_provenance::DependencyScope::Product,
            selected: &[],
            calling: None,
            providers: None,
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap()
}

#[test]
fn shared_array_inputs_bind_one_consistent_extent_for_structural_specialization() {
    let mut generic = rigid_array_form(
        "machine entry<const N: u64>(first: @[u8; N], second: @[u8; N], value: u64) -> u64 requires value <= N ensures result <= N { value }",
    );
    let found = discharge(&mut generic, &mut Default::default()).unwrap();
    let generic_scope =
        crate::prepare_replay(&mut generic, &mut Default::default()).unwrap();
    for (first, second, element, admitted) in [
        (0, 0, "u8", true),
        (3, 3, "u8", true),
        (3, 4, "u8", false),
        (3, 3, "u16", false),
    ] {
        let mut actual = concrete_array_form(&format!(
            "machine entry(first: @[u8; {first}], second: @[{element}; {second}], value: u64) -> u64 requires value <= {first}u64 ensures result <= {first}u64 {{ value }}"
        ));
        let mut actual_scope =
            crate::prepare_replay(&mut actual, &mut Default::default()).unwrap();
        let result = crate::reverify_specialization(
            &generic_scope,
            &found,
            id(1),
            &mut actual_scope,
            id(1),
            &[],
            &mut Default::default(),
        );
        if admitted {
            assert!(
                result
                    .as_ref()
                    .is_ok_and(|verdicts| verdicts.iter().all(|(_, verdict)| verdict.is_ok())),
                "{result:?}"
            );
        } else {
            assert!(result.is_err(), "{result:?}");
        }
    }
}

#[test]
fn a_parameter_array_view_keeps_the_actual_extent_and_its_guard() {
    for (requirement, admitted) in [("requires N > 0u64", true), ("", false)] {
        let form = rigid_array_form(&format!(
            "machine entry<const N: u64>(items: @[u8; N]) -> u64 {requirement} ensures result > 0u64 {{ let view: @[u8] = @items[..]; view.length }}"
        ));
        let verdict = judge(Ok(form)).unwrap();
        if admitted {
            assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
        } else {
            assert!(
                matches!(verdict, RouteVerdict::Rejected { unproved, .. } if !unproved.is_empty())
            );
        }
    }
}

#[test]
fn a_generic_scratch_array_lends_a_view_of_its_actual_const_extent() {
    let mut form = rigid_array_form(
        "machine entry<const N: u64>() -> u64 ensures result == N { let mutable scratch: [u8; N]; let view: @mutable [u8] = @mutable scratch[..]; view.length }",
    );
    let discharged = discharge(&mut form, &mut Default::default()).unwrap();
    assert!(
        discharged
            .machines
            .iter()
            .all(|(_, machine)| machine.verdicts.iter().all(|(_, verdict)| verdict.is_ok())),
        "{discharged:?}"
    );
    for length in [0, 3] {
        let source = format!(
            "machine scratch<const N: u64>() -> u64 ensures result == N {{ let mutable scratch: [u8; N]; let view: @mutable [u8] = @mutable scratch[..]; view.length }} machine entry() -> u64 {{ scratch::<{length}u64>() }}"
        );
        let verdicts = array_subject(&source);
        assert!(
            verdicts
                .iter()
                .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
            "{verdicts:?}"
        );
        assert_array_invocation_value(verdicts, length);
    }
}

#[test]
fn a_parameter_array_lends_a_typed_view_without_changing_its_element() {
    let mut form = rigid_array_form(
        "machine entry<const N: u64>(items: @[u32; N]) -> u64 ensures result == N { let view: @[u32] = @items[..]; view.length }",
    );
    let found = discharge(&mut form, &mut Default::default()).unwrap();
    assert!(
        found
            .machines
            .iter()
            .all(|(_, machine)| machine.verdicts.iter().all(|(_, verdict)| verdict.is_ok())),
        "{found:?}"
    );
    for length in [0, 3] {
        let source = format!(
            "machine typed<const N: u64>() -> u64 ensures result == N {{ let mutable scratch: [u32; N]; let view: @mutable [u32] = @mutable scratch[..]; view.length }} machine entry() -> u64 {{ typed::<{length}u64>() }}"
        );
        let verdicts = array_subject(&source);
        assert!(
            verdicts
                .iter()
                .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
            "{verdicts:?}"
        );
        assert_array_invocation_value(verdicts, length);
    }
}

#[test]
fn a_const_call_cannot_bind_an_array_extent_to_another_static_input() {
    for array in ["[u8; N]", "[[u8; N]; 2]"] {
        let mut form = rigid_array_form(&format!(
            "machine entry<const N: u64, const M: u64>(items: @{array}) -> u64 {{ child(items) machine child(items: @{array}) -> u64 {{ N }} }}"
        ));
        let caller = &mut form.module.machines[0];
        let call = caller
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| operation.declared_invocation().is_some())
            .unwrap();
        match &mut call.kind {
            OperationKind::CallStructuralScalar { arguments, .. }
            | OperationKind::CallUnit { arguments, .. } => arguments[0] = ValueId::new(2).unwrap(),
            other => panic!("{other:?}"),
        }
        assert!(discharge(&mut form, &mut Default::default()).is_err());
    }
}

#[test]
fn a_call_preserves_a_const_argument_that_the_record_does_not_store() {
    let source = "
data Token<const Key: u64> { payload: u16; }
machine entry<const N: u64, const M: u64>(token: @Token<N>) -> u64 {
    child(token)
    machine child(token: @Token<N>) -> u64 { N }
}";
    let baseline = rigid_array_form(source);
    let mut valid = baseline.clone();
    let checked = discharge(&mut valid, &mut Default::default()).unwrap();
    assert!(
        checked
            .machines
            .iter()
            .all(|(_, machine)| machine.verdicts.iter().all(|(_, verdict)| verdict.is_ok())),
        "{checked:?}"
    );

    let mut changed = baseline.clone();
    let caller = &mut changed.module.machines[0];
    let original = caller.parameters[0].id;
    let other = caller.parameters[1].id;
    let call = caller
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.declared_invocation().is_some())
        .unwrap();
    let OperationKind::CallStructuralScalar { arguments, .. } = &mut call.kind else {
        panic!("{call:?}");
    };
    assert_eq!(arguments[0], original);
    // The layout of Token does not mention either value. Passing M where
    // the callee expects N still changes the nominal type of its input.
    arguments[0] = other;
    let error = discharge(&mut changed, &mut Default::default()).unwrap_err();
    assert!(
        error
            .detail
            .contains("nominal call type changed its complete type or const actuals"),
        "{error:?}"
    );

    let mut missing = baseline.clone();
    missing.nominal_calls.clear();
    let error = discharge(&mut missing, &mut Default::default()).unwrap_err();
    assert!(
        error
            .detail
            .contains("nominal call type has no complete type correspondence"),
        "{error:?}"
    );

    let mut duplicate = baseline.clone();
    duplicate
        .nominal_calls
        .push(duplicate.nominal_calls[0].clone());
    let error = discharge(&mut duplicate, &mut Default::default()).unwrap_err();
    assert!(
        error.detail.contains("duplicated or unordered"),
        "{error:?}"
    );

    let mut stale = baseline;
    stale.nominal_calls[0].actual_constructor =
        semantic_vocabulary::StructuralTypeId::new(u64::MAX).unwrap();
    let error = discharge(&mut stale, &mut Default::default()).unwrap_err();
    assert!(
        error
            .detail
            .contains("nominal call type changed its complete type or const actuals"),
        "{error:?}"
    );
}

#[test]
fn a_nominal_result_belongs_to_the_callers_const_application() {
    use terminal_psi::generic_calls::CallTypePosition;

    let source = "
data Counter<const Limit: i32> { value: i32; }
machine entry<const N: i32>(counter: Counter<N>) -> Counter<N> {
    transition counter.value > 0 { true -> forwarded(counter), _ -> (counter) }
    machine forwarded(counter: Counter<N>) -> Counter<N> { counter }
}";
    let mut form = rigid_array_form(source);
    let result = form
        .nominal_calls
        .iter()
        .position(|row| row.position == CallTypePosition::Result)
        .expect("the two machines own distinct presentations of Counter<N>");
    assert_ne!(
        form.nominal_calls[result].actual,
        form.nominal_calls[result].expected
    );
    let checked = discharge(&mut form.clone(), &mut Default::default()).unwrap();
    assert!(
        checked
            .machines
            .iter()
            .all(|(_, machine)| machine.verdicts.iter().all(|(_, verdict)| verdict.is_ok())),
        "{checked:?}"
    );

    form.nominal_calls.remove(result);
    let error = discharge(&mut form, &mut Default::default()).unwrap_err();
    assert!(
        error.detail.contains("no complete type correspondence"),
        "{error:?}"
    );
}

#[test]
fn a_const_record_requirement_follows_the_callers_bound_value() {
    let source = "
data Slots<const Count: u64> requires Count >= 1, active < Count {
    items: [u64; Count];
    active: u64;
    other: u64;
}
machine entry<const Count: u64>(slots: @Slots<Count>) -> u64
requires Count >= 1 {
    read_slot(slots, 0)
    machine read_slot(slots: @Slots<Count>, index: u64) -> u64
    requires Count >= 1 && index < Count { slots.items[index] }
}";
    let baseline = rigid_array_form(source);
    let mut valid = baseline.clone();
    let checked = discharge(&mut valid, &mut Default::default()).unwrap();
    assert!(
        checked
            .machines
            .iter()
            .all(|(_, machine)| machine.verdicts.iter().all(|(_, verdict)| verdict.is_ok())),
        "{checked:?}"
    );

    let mut changed = baseline;
    let expected = changed.nominal_calls[0].expected_constructor;
    let declaration = changed
        .module
        .types
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == expected)
        .unwrap();
    let semantic_vocabulary::Proposition::LessThan(
        semantic_vocabulary::ScalarTerm::IntegerField { path, scalar_type, .. },
        _,
    ) = &mut declaration.requires[1]
    else {
        panic!("the record must retain its field bound");
    };
    let [semantic_vocabulary::CanonicalStructuralPathSegment::Field(field)] = path.as_mut_slice()
    else {
        panic!("the bound must name one field");
    };
    *field = declaration
        .shape
        .fields()
        .find(|other| {
            other.id != *field
                && other.field_type
                    == terminal_psi::StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Integer(*scalar_type),
                    )
        })
        .expect("the alternate field should have the original carrier")
        .id;
    let error = discharge(&mut changed, &mut Default::default()).unwrap_err();
    assert!(
        error
            .detail
            .contains("nominal call type changed its complete type or const actuals"),
        "{error:?}"
    );
}

#[test]
fn a_const_record_call_keeps_the_borrow_ceiling_when_its_fields_have_local_ids() {
    let source = "
data Slots<const Count: u64> requires Count >= 1 { items: [u64; Count]; }
machine entry<const Count: u64>(slots: @mutable Slots<Count>)
requires Count >= 1 {
    write_slot(slots, 0);
    machine write_slot(slots: @mutable Slots<Count>, index: u64)
    requires Count >= 1 && index < Count {
        let value: u64 = 42;
        slots.items[index] = value;
    }
}";
    let mut form = rigid_array_form(source);
    let checked = discharge(&mut form, &mut Default::default()).unwrap();
    assert!(
        checked
            .machines
            .iter()
            .all(|(_, machine)| machine.verdicts.iter().all(|(_, verdict)| verdict.is_ok())),
        "{checked:?}"
    );
}

#[test]
fn a_literal_const_call_keeps_the_supplied_element_permissions() {
    let source = "
data Buffer<Element, const N: u64> { items: [Element; N]; }
machine measure<Element, const N: u64>(value: @Buffer<Element, N>) -> u64 { value.items.length }
machine entry<Kind>(value: @Kind) -> u64
requires Kind == Buffer<u8, 4> || Kind == Buffer<u16, 4>
{
    transition {
        Kind == Buffer<u8, 4> -> measure::<u8, 4>(value),
        _ -> measure::<u16, 4>(value)
    }
}";
    // Both branches belong to the generic declaration. Check it as a source
    // subject so the harness exercises each admitted type for Kind.
    let verdict = super::entryless::judged(source, false);
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn a_recursive_const_call_checks_its_extent_even_when_the_local_types_are_equal() {
    for array in ["[u8; N]", "[[u8; N]; 2]"] {
        let mut form = rigid_array_form(&format!(
            "machine entry<const N: u64, const M: u64>(items: @{array}) -> u64 {{ N }}"
        ));
        let checked = discharge(&mut form, &mut Default::default())
            .expect("the generic body supplying the invocation types must check");
        assert!(
            checked
                .machines
                .iter()
                .all(|(_, machine)| machine.verdicts.iter().all(|(_, verdict)| verdict.is_ok()))
        );
        let owner = &form.module.machines[0];
        let array = owner.structural_parameters[0].structural_type;
        let mut arguments: Vec<_> = owner.parameters.iter().map(|formal| formal.id).collect();
        assert_eq!(arguments.len(), 2);
        let matches = |arguments: &[ValueId]| {
            terminal_semantics::calls::const_array_argument_matches(
                &form.module,
                owner,
                owner,
                arguments,
                array,
                array,
            )
        };
        assert!(matches(&arguments), "N forwarded to N retains its equation");
        arguments[0] = arguments[1];
        assert!(
            !matches(&arguments),
            "M cannot replace N even when caller, callee and array types are identical"
        );
    }
}

#[test]
fn const_extent_correspondence_only_treats_explicit_abstract_referents_as_leaves() {
    let mut form =
        rigid_array_form("machine entry<const N: u64, const M: u64>(items: @[u8; N]) -> u64 { N }");
    let unknown = semantic_vocabulary::StructuralTypeId::new(99).unwrap();
    let agrees = |module: &terminal_psi::TerminalModule, first| {
        let owner = &module.machines[0];
        terminal_semantics::calls::const_array_argument_matches(
            module,
            owner,
            owner,
            &[ValueId::new(first).unwrap(), ValueId::new(2).unwrap()],
            unknown,
            unknown,
        )
    };
    assert!(
        !agrees(&form.module, 1),
        "an unknown type is not an opaque leaf"
    );
    let mut concrete = form
        .module
        .types.structural_types
        .iter()
        .find(|declaration| {
            matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Parameter { .. },
                    ..
                }
            )
        })
        .unwrap()
        .clone();
    form.module
        .types.structural_referents
        .push(terminal_psi::StructuralReferentIdentity {
            id: unknown,
            identity: concrete.identity,
            foreign_descriptor: true,
            abstract_opaque: false,
        });
    assert!(
        !agrees(&form.module, 1),
        "a referent row alone grants no opacity"
    );
    form.module
        .types.structural_referents
        .last_mut()
        .unwrap()
        .abstract_opaque = true;
    assert!(agrees(&form.module, 1));
    concrete.id = unknown;
    form.module.types.structural_types.push(concrete);
    form.module
        .types.structural_types
        .sort_unstable_by_key(|declaration| declaration.id);
    assert!(agrees(&form.module, 1));
    assert!(
        !agrees(&form.module, 2),
        "an opaque marker cannot hide a concrete extent equation"
    );
}

#[test]
fn fixed_array_contract_lengths_keep_the_exact_field_carrier() {
    let source = r#"
data Holder { short: [u8; 2]; long: [u8; 4]; }
machine bounded(holder: @Holder, index: u64)
requires index < holder.long.length
{}
machine build_entry(holder: @Holder, index: u64)
requires index < 4u64
{ bounded(holder, index); }
"#;
    assert_eq!(verdict(source), "accepted");
    let rejected = verdict(&source.replace("holder.long.length", "holder.short.length"));
    assert!(rejected.starts_with("rejected"), "{rejected}");
    let unestablished = verdict(
        r#"
data Choice { case Empty; case Filled(slots: [u8; 4]); }
machine build_entry(choice: @Choice, index: u64)
requires index < choice.slots.length
{}
"#,
    );
    assert!(
        unestablished.contains("fixed array length through an unestablished case payload"),
        "{unestablished}"
    );
}

/// A program calling a boundary requirement whose array formal is sized by
/// its own `const` binder: the closure first, then the requirement's own check
/// form. generics.md, "Publication and assumptions": the template is checked
/// once; the program carries only its closed application.
fn sized_boundary_program(
    contract: &str,
    call: &str,
) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let typed = crate::preparation::prepare_typed_trees(
        super::boundary_calls::typed(&format!(
            "boundary trait Sink {{
    machine take<const Count: u64>(items: @[u8; Count], used: i32) -> u32
    reaches none
    {contract}
    crashes none;
}}
machine entry() -> u32 {{ let buffer: [u8; 4] = [1, 2, 3, 0]; {call} }}"
        )),
        &[],
        &[],
    )
    .unwrap();
    let entry = super::boundary_calls::entry(typed.trees());
    crate::judge_compilation(
        crate::lowering::prepare_compilation(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[None],
                source_provenance::DependencyScope::Product,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("fixture must lower to checker input")
        .program,
    )
}

fn accepted(verdict: &Result<RouteVerdict, crate::CompilationRefusal>) -> bool {
    matches!(verdict, Ok(RouteVerdict::Accepted(_)))
}

#[test]
fn forwarding_an_extent_to_a_boundary_checks_the_declared_contract() {
    let source = r#"
boundary trait Other {}
boundary trait Text {
    machine send<const Count: u64>(bytes: @[u8; Count]) -> u64
    requires Count > 0u64
    requires bytes[Count - 1u64] == 0u8
    reaches none
    crashes none;
}
machine forward<const Size: u64>(bytes: @[u8; Size]) -> u64
requires Size > 0u64
requires bytes[Size - 1u64] == 0u8
reaches none
crashes none
{ Text::send::<Size>(bytes) }
machine entry() -> u64 {
    let bytes: [u8; 4] = [1, 2, 3, 0];
    forward::<4u64>(@bytes)
}
"#;
    let valid = array_subject(source);
    assert!(valid.iter().all(accepted), "{valid:?}");
    // The caller must account for a synchronous entry published by the
    // method. Two bodyless services distinguish an exact grant from a near miss.
    let grants = source.replace("reaches none", "reaches none\ninvokes Text");
    let valid = array_subject(&grants);
    assert!(valid.iter().all(accepted), "{valid:?}");
    let missing = array_subject(&grants.replace("invokes Text\ncrashes none\n{", "invokes Other\ncrashes none\n{"));
    assert!(missing.iter().any(|verdict| matches!(verdict, Ok(RouteVerdict::Rejected { exceeded, .. })
        if exceeded.iter().any(|row| row.message().contains("synchronous invocation")))), "{missing:?}");
    let different = source.replace("Size - 1u64] == 0u8", "Size - 1u64] == 7u8")
        .replace("[1, 2, 3, 0]", "[1, 2, 3, 7]");
    let wrong_byte = array_subject(&different);
    assert!(wrong_byte.iter().any(|verdict| matches!(verdict, Ok(RouteVerdict::Rejected { unproved, .. })
        if !unproved.is_empty())), "{wrong_byte:?}");
}

#[test]
fn a_generic_array_implementation_can_return_one_of_its_elements() {
    let typed = crate::preparation::prepare_typed_trees(
        super::boundary_calls::typed(
            "boundary trait Elements {
                machine last<const Count: u64, Element [copy]>(items: @[Element; Count]) -> Element
                    reaches none requires Count > 0 crashes none;
            }
            Selected satisfies Elements {
                machine last<const Count: u64, Element [copy]>(items: @[Element; Count]) -> Element
                    requires Count > 0 { items[Count - 1] }
            }
            machine entry() {}",
        ),
        &[],
        &[],
    )
    .unwrap();
    let verdicts = super::boundary_calls::compilation_results_for_subjects(&typed, &[None]);
    assert!(
        verdicts.len() > 1,
        "the requirement and its implementation must be checked"
    );
    assert!(verdicts.iter().all(accepted), "{verdicts:?}");
}

#[test]
fn a_size_generic_requirement_is_checked_once_for_every_size() {
    // At the call, `Count` is 4 and both contracts are formed. The template
    // is judged for every `Count`: `Count - 1` underflows at zero unless the
    // contract first requires `Count > 0`.
    let verdicts = sized_boundary_program(
        "requires Count > 0 && used <= Count - 1",
        "Sink::take::<4>(@buffer, 0)",
    );
    assert_eq!(verdicts.len(), 2, "{verdicts:?}");
    assert!(verdicts.iter().all(accepted), "{verdicts:?}");

    let verdicts =
        sized_boundary_program("requires used <= Count - 1", "Sink::take::<4>(@buffer, 0)");
    assert_eq!(verdicts.len(), 2, "{verdicts:?}");
    assert!(accepted(&verdicts[0]), "{verdicts:?}");
    assert!(
        matches!(verdicts[1], Ok(RouteVerdict::Rejected { .. })),
        "{verdicts:?}"
    );
}

#[test]
fn a_program_checks_a_size_generic_requirement_at_its_concrete_size() {
    // The closed call has a real four-byte array: `items[0]` is in bounds and
    // holds 1; `items[Count]` is index 4, outside it.
    let verdicts = sized_boundary_program(
        "requires Count > 0 && items[0] == 1",
        "Sink::take::<4>(@buffer, 0)",
    );
    let Ok(RouteVerdict::Accepted(closure)) = &verdicts[0] else {
        panic!("the closed application is formed and established: {verdicts:?}")
    };
    // The program carries no open extent, so it publishes.
    closure.publish_invocation().unwrap();
    // For every positive Count the literal selector names an existing byte.
    assert!(accepted(&verdicts[1]), "{verdicts:?}");

    let verdicts = sized_boundary_program(
        "requires Count > 0 && items[Count] == 1",
        "Sink::take::<4>(@buffer, 0)",
    );
    assert!(verdicts.iter().all(|row| !accepted(row)), "{verdicts:?}");
}

#[test]
fn a_symbolic_array_length_uses_its_owned_extent_input() {
    for carrier in ["u32", "u64"] {
        let form = rigid_array_form(&format!(
            "machine entry<const N: {carrier}>(items: @[u8; N]) -> u64 ensures result == (N as u64) {{ items.length }}"
        ));
        let verdict = judge(Ok(form)).unwrap();
        assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    }
}

#[test]
fn a_closed_call_checks_an_index_computed_from_its_size() {
    // system's `send_string<const Count>` requires `text[Count - 1] == 0`. At
    // `take<4>` the index is the closed arithmetic `4 - 1`; the buffer's last
    // byte is 0.
    let verdicts = sized_boundary_program(
        "requires Count > 0 && items[Count - 1] == 0",
        "Sink::take::<4>(@buffer, 0)",
    );
    let Ok(RouteVerdict::Accepted(closure)) = &verdicts[0] else {
        panic!("the closed index is formed and established: {verdicts:?}")
    };
    closure.publish_invocation().unwrap();
    assert!(accepted(&verdicts[1]), "{verdicts:?}");

    // Specializing at four cannot discharge a subtraction that underflows
    // in the declaration at zero.
    let verdicts = sized_boundary_program(
        "requires items[Count - 1] == 0",
        "Sink::take::<4>(@buffer, 0)",
    );
    assert!(accepted(&verdicts[0]), "{verdicts:?}");
    assert!(
        matches!(verdicts[1], Ok(RouteVerdict::Rejected { .. })),
        "{verdicts:?}"
    );

    // The same index, a byte the buffer does not hold there.
    let verdicts = sized_boundary_program(
        "requires Count > 0 && items[Count - 1] == 1",
        "Sink::take::<4>(@buffer, 0)",
    );
    assert!(
        matches!(verdicts[0], Ok(RouteVerdict::Rejected { .. })),
        "{verdicts:?}"
    );

    assert!(accepted(&verdicts[1]), "{verdicts:?}");

    // `4 + 1` is outside the four-byte array.
    let verdicts = sized_boundary_program(
        "requires Count > 0 && items[Count + 1] == 0",
        "Sink::take::<4>(@buffer, 0)",
    );
    assert!(!accepted(&verdicts[0]), "{verdicts:?}");
}

#[test]
fn a_program_that_never_applies_a_template_requirement_leaves_its_clauses_to_its_own_form() {
    // The program reads the trait but calls no closed `take`. The template's
    // clauses are read only by its own form, so the program's source capture
    // must not ask for their operators (system's `ObjectiveC::send_byte_string`).
    let verdicts = sized_boundary_program("requires Count > 0 && used <= Count", "0u32");
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "{verdicts:?}"
    );
}

#[test]
fn an_indexed_element_contract_needs_the_count_of_its_own_view() {
    for (bound, admitted) in [("slot < first.length", true), ("slot < second.length", false)] {
        let source = format!(
            "machine entry(first: @[u32], second: @[u32], slot: u64) -> u32
            requires {bound}
            requires first[slot] == 7u32
            {{ 7u32 }}"
        );
        let verdict = judge(Ok(concrete_array_form(&source))).unwrap();
        assert_eq!(
            matches!(verdict, RouteVerdict::Accepted(_)),
            admitted,
            "{verdict:?}"
        );
    }
}
