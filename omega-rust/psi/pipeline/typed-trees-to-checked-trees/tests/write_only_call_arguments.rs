//! `&write` projected subloans are non-observing place arguments admitted at
//! every direct checked-call boundary: statement calls, expression calls, and
//! named state-transition targets all deliver arguments to callee parameters
//! through the same shared borrow-call facts, so the same gate applies.
//!
//! The gate itself is unchanged — only literal-index/member paths of a
//! write-only root with an unrestricted primitive leaf are admitted, the
//! argument still must satisfy the callee's `&write` contract, and ordinary
//! reads through write-only projections remain rejected everywhere else.

use checked_trees::{
    BorrowAccessKind, CheckedStructuralAccess, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment,
};

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    typed_trees_to_checked_trees::lower_typed_trees(
        typed(source)?,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
}

fn typed(source: &str) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(
            std::path::PathBuf::from("write_only_call_arguments.omg"),
            source.to_owned(),
        )
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let syntax = syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
        syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest::new(syntax),
    )?;
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
        },
    )?;
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])
}

fn rendered(source: &str) -> String {
    check(source)
        .expect_err("expected the write-only slice to reject this source")
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

fn unit_plan<'a>(
    checked: &'a checked_trees::CheckedTrees,
    name: &str,
) -> &'a CheckedUnitEffectMachinePlan {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("authored machine")
        .symbol;
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine)
        .expect("authored machine retains its checked Unit plan")
}

fn scalar_call_structural_arguments(
    plan: &CheckedUnitEffectMachinePlan,
) -> &[checked_trees::CheckedUnitStructuralArgumentPlan] {
    plan.operations
        .iter()
        .find_map(|operation| {
            let CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            } = operation
            else {
                return None;
            };
            Some(structural_arguments.as_slice())
        })
        .expect("the expression call retains one checked scalar-call operation")
}

#[test]
fn expression_call_argument_admits_literal_indexed_write_only_subloan() {
    let checked = check(
        "machine replace(destination: &write u16, value: u16) -> u16 {
             destination = value;
             value
         }
         machine forward(values: &write [u16; 4], value: u16) -> u16 {
             let out: u16 = replace(&write values[1], value);
             out
         }",
    )
    .expect("an expression-call argument is the same direct checked-call boundary a statement call presents");

    let arguments = scalar_call_structural_arguments(unit_plan(&checked, "forward"));
    assert_eq!(arguments.len(), 1);
    let argument = &arguments[0];
    assert_eq!(argument.access, CheckedStructuralAccess::WriteOnlyBorrow);
    assert_eq!(
        argument.path,
        vec![CheckedUnitStructuralPathSegment::FixedIndex(1)],
        "the projected subloan retains its exact fixed element, not the whole array"
    );
    assert!(
        matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
        ),
        "the argument source is the caller's own `values` parameter"
    );
    assert_eq!(argument.type_identity, "named(name(u16))");
}

#[test]
fn expression_call_argument_admits_interleaved_member_index_subloan() {
    let checked = check(
        "data Record [copy] { value: u16; }
         machine replace(destination: &write u16, value: u16) -> u16 {
             destination = value;
             value
         }
         machine forward(records: &write [Record; 2], value: u16) -> u16 {
             let out: u16 = replace(&write records[1].value, value);
             out
         }",
    )
    .expect("an indexed record-field subloan is a content-independent place argument");

    let arguments = scalar_call_structural_arguments(unit_plan(&checked, "forward"));
    assert_eq!(
        arguments[0].path,
        vec![
            CheckedUnitStructuralPathSegment::FixedIndex(1),
            CheckedUnitStructuralPathSegment::Field("value".to_owned())
        ],
        "member and literal-index hops compose in authored order"
    );
    assert_eq!(
        arguments[0].access,
        CheckedStructuralAccess::WriteOnlyBorrow
    );
}

#[test]
fn expression_call_argument_admits_record_field_write_only_subloan() {
    let checked = check(
        "data Holder { slot: u16; spare: u16; }
         machine poke(destination: &write u16, value: u16) -> u16 {
             destination = value;
             value
         }
         machine forward(holder: &write Holder, value: u16) -> u16 {
             let out: u16 = poke(&write holder.slot, value);
             out
         }",
    )
    .expect("a record-field subloan is admitted at the expression-call boundary");

    let arguments = scalar_call_structural_arguments(unit_plan(&checked, "forward"));
    assert_eq!(
        arguments[0].path,
        vec![CheckedUnitStructuralPathSegment::Field("slot".to_owned())]
    );
    assert_eq!(
        arguments[0].access,
        CheckedStructuralAccess::WriteOnlyBorrow
    );
}

#[test]
fn transition_target_argument_admits_literal_indexed_write_only_subloan() {
    let checked = check(
        "machine forward(values: &write [u16; 4], marker: u16) {
             transition { _ -> done(&write values[1], marker) }
             state done(slot: &write u16, marker: u16) {
                 slot = marker;
             }
         }",
    )
    .expect("a named state-transition target is a direct checked-call boundary");

    let forward = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("authored machine");
    let entry = checked
        .machine_states(forward)
        .iter()
        .find(|state| state.name.as_str() == "entry")
        .expect("entry state");
    let values = checked
        .state_parameters(entry)
        .iter()
        .find(|parameter| parameter.name.as_str() == "values")
        .expect("values parameter")
        .symbol;

    // The shared borrow-call facts record the transition target's argument
    // with its exact root and literal index — the same evidence a statement
    // or expression call produces.
    let borrow = &checked.facts.borrow;
    let state_facts = borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == entry.symbol)
        .expect("entry borrow state");
    let calls = borrow.calls.span_or_empty(state_facts.calls);
    assert!(
        calls.iter().any(|call| {
            borrow
                .argument_accesses
                .span_or_empty(call.accesses)
                .iter()
                .any(|access| {
                    access.kind == BorrowAccessKind::WriteOnly
                        && access.root_symbol == values
                        && borrow.access_segments.span_or_empty(access.segments)
                            == [facts::PlaceSegment::FixedIndex { index: 1 }]
                })
        }),
        "the transition target retains the write-only projected access"
    );
}

#[test]
fn expression_call_arguments_still_reject_dynamic_index_subloans() {
    let rendered = rendered(
        "machine replace(destination: &write u16, value: u16) -> u16 {
             destination = value;
             value
         }
         machine forward(values: &write [u16; 4], value: u16, i: u16) -> u16 {
             let out: u16 = replace(&write values[i], value);
             out
         }",
    );
    assert!(
        rendered.contains("unsupported projection or computed expression"),
        "dynamic indexes remain outside the literal-coordinate gate: {rendered}"
    );
}

#[test]
fn expression_call_arguments_still_reject_aggregate_element_subloans() {
    let rendered = rendered(
        "machine replace(destination: &write [u16; 2], value: u16) -> u16 {
             destination[0] = value;
             value
         }
         machine forward(matrix: &write [[u16; 2]; 2], value: u16) -> u16 {
             let out: u16 = replace(&write matrix[1], value);
             out
         }",
    );
    assert!(
        rendered.contains("unsupported projection or computed expression"),
        "aggregate elements stay atomic at this rung: {rendered}"
    );
}

#[test]
fn expression_call_arguments_still_reject_sibling_observation() {
    let rendered = rendered(
        "machine replace(destination: &write u16, value: u16) -> u16 {
             destination = value;
             value
         }
         machine forward(values: &write [u16; 4], value: u16) -> u16 {
             let out: u16 = replace(&write values[1], values[0]);
             out
         }",
    );
    assert!(
        rendered.contains("reads through index projection of write-only parameter `values`"),
        "admitting one argument must not observe another projected element: {rendered}"
    );
}

#[test]
fn transition_target_arguments_still_reject_dynamic_index_subloans() {
    let rendered = rendered(
        "machine forward(values: &write [u16; 4], marker: u16, i: u16) {
             transition { _ -> done(&write values[i], marker) }
             state done(slot: &write u16, marker: u16) {
                 slot = marker;
             }
         }",
    );
    assert!(
        rendered.contains("unsupported projection or computed expression"),
        "transition targets keep the same literal-coordinate gate: {rendered}"
    );
}

#[test]
fn value_call_arguments_require_explicit_write_only_attenuation() {
    let rendered = rendered(
        "machine replace(destination: &write u16, value: u16) -> u16 {
             destination = value;
             value
         }
         machine forward(values: &mut [u16; 4], value: u16) -> u16 {
             let out: u16 = replace(values[1], value);
             out
         }",
    );
    assert!(
        rendered.contains("requires explicit write-only attenuation"),
        "a bare place argument cannot silently supply `&write` authority in value position: {rendered}"
    );
}

#[test]
fn value_call_arguments_reject_write_only_widening() {
    let rendered = rendered(
        "machine observe(destination: &mut u16, value: u16) -> u16 {
             destination = value;
             value
         }
         machine forward(values: &write [u16; 4], value: u16) -> u16 {
             let out: u16 = observe(&write values[1], value);
             out
         }",
    );
    assert!(
        rendered.contains("write-only authority cannot widen to shared or mutable access"),
        "a value-position call must not widen `&write` to `&mut`: {rendered}"
    );
}
