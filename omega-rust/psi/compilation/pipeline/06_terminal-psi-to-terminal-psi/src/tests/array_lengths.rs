//! The length of a fixed array, `array.len`. Read from source, as a Build
//! program: a Build that holds a table asks how many rows it has.
//!
//! domains.md: "`[u8; N] in D` is the same exact `N`-byte fixed-array carrier
//! as `[u8; N]`; its `.len` is `N`". The length is the extent the type
//! states, so the reader gives the expression that number and lowers no
//! operation for it. A guarantee over the result is then held to the number,
//! whichever fields the array is reached through.

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
machine held(holder: &Holder) -> u64 ensures result == 4 { holder.slots.len }
machine nested(outer: &Outer) -> u64 ensures result == 4 { outer.holder.slots.len }
machine build_entry() { let outer: Outer; let seen: u64 = held(&outer.holder) + nested(&outer); }"
        ),
        "accepted"
    );
    // A local array.
    assert_eq!(
        verdict(
            "machine build_entry() -> u64 ensures result == 3 { let values: [u32; 3] = [1, 2, 3]; values.len }"
        ),
        "accepted"
    );
}

#[test]
fn a_guarantee_of_another_length_is_rejected() {
    // The same machine as above, promising one row more than the type has.
    let rejected = verdict(
        "data Holder { slots: [u8; 4]; }
machine held(holder: &Holder) -> u64 ensures result == 5 { holder.slots.len }
machine build_entry() { let holder: Holder; let seen: u64 = held(&holder); }",
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
machine build_entry() -> u64 { make().len }"
        ),
        "expression length of a fixed array that is not a field path while reading the body of \
         `build_entry`"
    );
}

fn rigid_array_form(text: &str) -> typed_trees_to_terminal_psi::Elaboration {
    let (typed, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        super::boundary_calls::typed(text),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let entry = super::boundary_calls::entry(&typed);
    typed_trees_to_terminal_psi::elaborate_generic(&typed, entry).unwrap()
}

#[test]
fn a_rigid_array_extent_names_its_const_input_without_requiring_positivity() {
    for carrier in ["u64", "u32"] {
        let mut form = rigid_array_form(&format!(
            "machine entry<const N: {carrier}>(items: &[u8; N]) -> {carrier} ensures result == N {{ N }}"
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
            rigid_array_form("machine entry<const N: u64>(items: &[u8; N]) -> u64 { N }");
        let extent = form
            .module
            .structural_types
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
    let (typed, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        super::boundary_calls::typed(text),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let entry = super::boundary_calls::entry(&typed);
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[None],
                purpose: source::DependencyScope::Product,
                selected: &[],
                calling: None,
                providers: None,
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    )
}

#[test]
fn a_whole_subject_checks_the_original_nested_const_array_body() {
    for length in [0, 3] {
        let source = format!(
            "machine extent<const N: u64>(items: &[u8; N]) -> u64 {{ finish(items) machine finish(items: &[u8; N]) -> u64 {{ N }} }}\nmachine entry() -> u64 {{ let values: [u8; {length}]; extent(&values) }}"
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

fn assert_array_invocation_value(
    mut verdicts: Vec<Result<RouteVerdict, crate::CompilationRefusal>>,
    expected: u128,
) {
    let Ok(RouteVerdict::Accepted(accepted)) = verdicts.remove(0) else {
        panic!("the invocation was refused");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
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
    let source = "machine extent<const N: u64>(items: &[u8; N]) -> u64 ensures result == 3 { finish(items) machine finish(items: &[u8; N]) -> u64 { N } }\nmachine entry() -> u64 { let values: [u8; 3]; extent(&values) }";
    let verdicts = array_subject(source);
    assert!(verdicts.iter().any(|verdict| matches!(verdict, Ok(RouteVerdict::Rejected { unproved, .. }) if !unproved.is_empty())), "{verdicts:?}");
}

#[test]
fn a_rigid_array_index_needs_its_own_bound_including_a_literal_selector() {
    for (requirement, admitted) in [("requires N > 0u64", true), ("", false)] {
        let form = rigid_array_form(&format!(
            "machine entry<const N: u64>(items: &[u8; N]) -> u8 {requirement} {{ items[0] }}"
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

fn concrete_array_form(text: &str) -> typed_trees_to_terminal_psi::Elaboration {
    let typed = super::boundary_calls::typed(text);
    let entry = super::boundary_calls::entry(&typed);
    typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
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
        "machine entry<const N: u64>(first: &[u8; N], second: &[u8; N], value: u64) -> u64 requires value <= N ensures result <= N { value }",
    );
    let found = discharge(&mut generic, &mut Default::default()).unwrap();
    let generic_scope = crate::prepare_replay(&mut generic, &mut Default::default()).unwrap();
    for (first, second, element, admitted) in [
        (0, 0, "u8", true),
        (3, 3, "u8", true),
        (3, 4, "u8", false),
        (3, 3, "u16", false),
    ] {
        let mut actual = concrete_array_form(&format!(
            "machine entry(first: &[u8; {first}], second: &[{element}; {second}], value: u64) -> u64 requires value <= {first}u64 ensures result <= {first}u64 {{ value }}"
        ));
        let mut actual_scope = crate::prepare_replay(&mut actual, &mut Default::default()).unwrap();
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
            "machine entry<const N: u64>(items: &[u8; N]) -> u64 {requirement} ensures result > 0u64 {{ let view: &[u8] = &items[..]; view.len }}"
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
        "machine entry<const N: u64>() -> u64 ensures result == N { let mut scratch: [u8; N]; let view: &mut [u8] = &mut scratch[..]; view.len }",
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
            "machine scratch<const N: u64>() -> u64 ensures result == N {{ let mut scratch: [u8; N]; let view: &mut [u8] = &mut scratch[..]; view.len }} machine entry() -> u64 {{ scratch<{length}u64>() }}"
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
        "machine entry<const N: u64>(items: &[u32; N]) -> u64 ensures result == N { let view: &[u32] = &items[..]; view.len }",
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
            "machine typed<const N: u64>() -> u64 ensures result == N {{ let mut scratch: [u32; N]; let view: &mut [u32] = &mut scratch[..]; view.len }} machine entry() -> u64 {{ typed<{length}u64>() }}"
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
            "machine entry<const N: u64, const M: u64>(items: &{array}) -> u64 {{ child(items) machine child(items: &{array}) -> u64 {{ N }} }}"
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
fn a_recursive_const_call_checks_its_extent_even_when_the_local_types_are_equal() {
    for array in ["[u8; N]", "[[u8; N]; 2]"] {
        let mut form = rigid_array_form(&format!(
            "machine entry<const N: u64, const M: u64>(items: &{array}) -> u64 {{ N }}"
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
        rigid_array_form("machine entry<const N: u64, const M: u64>(items: &[u8; N]) -> u64 { N }");
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
        .structural_types
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
        .structural_referents
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
        .structural_referents
        .last_mut()
        .unwrap()
        .abstract_opaque = true;
    assert!(agrees(&form.module, 1));
    concrete.id = unknown;
    form.module.structural_types.push(concrete);
    form.module
        .structural_types
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
machine bounded(holder: &Holder, index: u64)
requires index < holder.long.len
{}
machine build_entry(holder: &Holder, index: u64)
requires index < 4u64
{ bounded(holder, index); }
"#;
    assert_eq!(verdict(source), "accepted");
    let rejected = verdict(&source.replace("holder.long.len", "holder.short.len"));
    assert!(rejected.starts_with("rejected"), "{rejected}");
    let unestablished = verdict(
        r#"
data Choice { case Empty; case Filled(slots: [u8; 4]); }
machine build_entry(choice: &Choice, index: u64)
requires index < choice.slots.len
{}
"#,
    );
    assert!(
        unestablished.contains("fixed array length through an unestablished case payload"),
        "{unestablished}"
    );
}
