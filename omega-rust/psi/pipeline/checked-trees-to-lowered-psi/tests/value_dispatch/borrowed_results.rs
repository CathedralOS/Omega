//! Shared-borrow `&T` match results rejoin borrowed custody at the selection's
//! block parameter. The referent's owner is never transferred: each arm's
//! exact source place stays constrained for the result's whole live range,
//! the retained plan is replayed against the authored `&place` expressions,
//! and the join is emitted as a `SharedBorrow` block parameter rather than a
//! fabricated owned transfer. The terminal verifier admits the record-shaped
//! join against the exact root and projected path, keeps the referent pinned
//! for the block that observes it, and the interpreter binds the same view
//! without copying the payload.

use std::collections::BTreeMap;

use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_interpreter::{
    MeasuredTerminalExecution, TerminalExecutionResult, TerminalScalarValue,
    TerminalStructuralScalarFieldValue, TerminalStructuralValue,
};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralPathSegment, StructuralTypeShape,
    TerminalModule,
};

use super::{check_source, execute, execute_machine_with_structural_inputs, unsigned};

/// `a` is itself a prior selection's join result: the second match borrows
/// `a.first` through block-parameter custody on one arm and a plain local's
/// field on the other.
const CHAINED_SOURCE: &str = "data Payload { left: u64; right: u64; }
    data Pair { first: Payload; second: Payload; }
    machine choose(selected: bool, other: bool) -> u64 {
        let x: Pair = Pair {
            first: Payload { left: 1, right: 2 },
            second: Payload { left: 3, right: 4 }
        };
        let y: Pair = Pair {
            first: Payload { left: 5, right: 6 },
            second: Payload { left: 7, right: 8 }
        };
        let a: Pair = match selected { true -> x, false -> y };
        let b: Pair = Pair {
            first: Payload { left: 9, right: 10 },
            second: Payload { left: 11, right: 12 }
        };
        let view: &Payload = match other {
            true -> &a.first,
            false -> &b.second
        };
        view.left ^ view.right
    }";

const UNCHAINED_SOURCE: &str = "data Payload { left: u64; right: u64; }
    data Pair { first: Payload; second: Payload; }
    machine choose(selected: bool, other: bool) -> u64 {
        let x: Pair = Pair {
            first: Payload { left: 1, right: 2 },
            second: Payload { left: 3, right: 4 }
        };
        let b: Pair = Pair {
            first: Payload { left: 9, right: 10 },
            second: Payload { left: 11, right: 12 }
        };
        let view: &Payload = match other {
            true -> &x.first,
            false -> &b.second
        };
        view.left ^ view.right
    }";

/// One authored local's statement index and symbol, by name.
fn local(checked: &checked_trees::CheckedTrees, name: &str) -> (usize, symbols::SymbolHandle) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| checked.typed.symbols.name(machine.symbol) == "choose")
        .expect("choose machine");
    let state = checked
        .machine_states(machine)
        .iter()
        .next()
        .expect("single state");
    checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .find_map(|(index, statement)| match statement {
            checked_trees::statement::StatementNode::LocalData(local)
                if checked.typed.symbols.name(local.symbol) == name =>
            {
                Some((index, local.symbol))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("local {name} exists"))
}

/// The SharedBorrow `Reference` argument plans retained for one source's
/// match arms, keyed by root local name.
fn borrowed_arms(
    checked: &checked_trees::CheckedTrees,
) -> Vec<&checked_trees::CheckedUnitStructuralArgumentPlan> {
    checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            checked_trees::CheckedStructuralValueKind::Reference { source }
                if source.access == checked_trees::CheckedStructuralAccess::SharedBorrow =>
            {
                Some(source)
            }
            _ => None,
        })
        .collect()
}

/// Asserts the exact loans and retained arm provenance, returning both maps
/// keyed by root local name.
fn checked_borrowed_selection(
    checked: &checked_trees::CheckedTrees,
) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let (view_statement, view_symbol) = local(checked, "view");
    // Every arm's source stays borrowed for the result's whole live range:
    // the selected edge is decided at run time, so each arm's loan constrains
    // its exact source place until `view` dies.
    let mut lent = BTreeMap::new();
    for (_, loan) in checked.facts.borrow.loans.iter() {
        assert_eq!(loan.owner_symbol, view_symbol, "every arm loan owns `view`");
        assert_eq!(loan.statement_index, view_statement);
        assert_eq!(loan.last_use_statement_index, view_statement + 1);
        assert!(matches!(loan.kind, checked_trees::BorrowAccessKind::Read));
        let [facts::PlaceSegment::Field { symbol }] = checked.facts.borrow.loan_segments(loan)
        else {
            panic!("each borrowed arm lends exactly one projected field");
        };
        lent.insert(
            checked.typed.symbols.name(loan.root_symbol).to_owned(),
            checked.typed.symbols.name(*symbol).to_owned(),
        );
    }
    // The retained plan keeps each arm's authored borrow: a SharedBorrow
    // argument rooted at the exact local along exactly its authored path.
    let mut planned = BTreeMap::new();
    for arm in borrowed_arms(checked) {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
            arm.source
        else {
            panic!("borrowed arm must root at an exact local: {arm:?}");
        };
        let [checked_trees::CheckedUnitStructuralPathSegment::Field(field)] = arm.path.as_slice()
        else {
            panic!("borrowed arm keeps exactly the authored field path: {arm:?}");
        };
        planned.insert(checked.typed.symbols.name(symbol).to_owned(), field.clone());
    }
    (lent, planned)
}

/// A `&u64` result: the referent is a primitive, so it has no data
/// declaration for the record referent rule to resolve. The view is left
/// unread because no source spelling reads through a borrowed primitive local.
const PRIMITIVE_REFERENT_SOURCE: &str = "data Payload { left: u64; right: u64; }
    machine choose(other: bool) -> u64 {
        let a: Payload = Payload { left: 1, right: 2 };
        let b: Payload = Payload { left: 3, right: 4 };
        let view: &u64 = match other { true -> &a.left, false -> &b.right };
        0
    }";

/// The already-admitted record referent under the same unread view. Comparing
/// against this separates a primitive-specific gap from a consumer-shape gap.
const RECORD_UNREAD_SOURCE: &str = "data Payload { left: u64; right: u64; }
    data Pair { first: Payload; second: Payload; }
    machine choose(other: bool) -> u64 {
        let x: Pair = Pair {
            first: Payload { left: 1, right: 2 },
            second: Payload { left: 3, right: 4 }
        };
        let b: Pair = Pair {
            first: Payload { left: 9, right: 10 },
            second: Payload { left: 11, right: 12 }
        };
        let view: &Payload = match other { true -> &x.first, false -> &b.second };
        0
    }";

/// Passing the join result to a call is the only consumer a borrowed primitive
/// local could ever have, so the record referent is asked the same question.
const PRIMITIVE_CALL_SOURCE: &str = "data Payload { left: u64; right: u64; }
    machine read(value: &u64) -> u64 { value }
    machine choose(other: bool) -> u64 {
        let a: Payload = Payload { left: 1, right: 2 };
        let b: Payload = Payload { left: 3, right: 4 };
        let view: &u64 = match other { true -> &a.left, false -> &b.right };
        read(view)
    }";

const RECORD_CALL_SOURCE: &str = "data Payload { left: u64; right: u64; }
    machine read(value: &Payload) -> u64 { value.left ^ value.right }
    machine choose(other: bool) -> u64 {
        let a: Payload = Payload { left: 1, right: 2 };
        let b: Payload = Payload { left: 3, right: 4 };
        let view: &Payload = match other { true -> &a, false -> &b };
        read(view)
    }";

/// Each planned `SharedBorrow` arm as (root local name, its authored field
/// path joined by `.`), sorted, beside the machine's lowering outcome: `None`
/// when it lowers, otherwise the exact rejection. A whole-place borrow such as
/// `&a` carries no segments and reports an empty path.
fn planned_borrow_arms(source: &str) -> (Vec<(String, String)>, Option<String>) {
    let checked =
        check_source(source).unwrap_or_else(|errors| panic!("source checks: {errors:#?}"));
    let mut arms: Vec<(String, String)> = borrowed_arms(&checked)
        .iter()
        .map(|arm| {
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
                arm.source
            else {
                panic!("borrowed arm roots at an exact local: {arm:?}");
            };
            let path = arm
                .path
                .iter()
                .map(|segment| match segment {
                    checked_trees::CheckedUnitStructuralPathSegment::Field(field) => field.clone(),
                    other => panic!("borrowed arm keeps only authored field segments: {other:?}"),
                })
                .collect::<Vec<_>>()
                .join(".");
            (checked.typed.symbols.name(symbol).to_owned(), path)
        })
        .collect();
    arms.sort();
    let lowering = match checked_trees_to_lowered_psi::lower_machine(&checked, "choose") {
        Ok(_) => None,
        Err(error) => Some(format!("{error:?}")),
    };
    (arms, lowering)
}

/// The checked arm planner builds a carrier for a primitive referent. It
/// previously built none at all: `shared_record_reference` resolved the
/// referent through its data declaration, and `u64` has none, so a `&u64`
/// selection produced zero `SharedBorrow` argument plans against two for a
/// record. Both arms now keep their authored root and field, which is what the
/// lowering replay and the terminal verifier each re-derive independently.
///
/// Lowering still stops, and not at the carrier: the record referent with the
/// same unread view stops at the identical diagnostic. That parity is the
/// point of this assertion — the remaining gap belongs to the consumer shape,
/// not to the primitive.
#[test]
fn borrowed_selection_plans_a_primitive_referent_carrier() {
    let (arms, lowering) = planned_borrow_arms(PRIMITIVE_REFERENT_SOURCE);
    assert_eq!(
        arms,
        vec![
            ("a".to_owned(), "left".to_owned()),
            ("b".to_owned(), "right".to_owned()),
        ],
        "each primitive arm keeps its authored root and field"
    );
    let (record_arms, record_lowering) = planned_borrow_arms(RECORD_UNREAD_SOURCE);
    assert_eq!(record_arms.len(), 2, "the record control plans both arms");
    assert_eq!(
        lowering, record_lowering,
        "a primitive referent stops exactly where an admitted record referent does"
    );
    assert_eq!(
        lowering.as_deref(),
        Some(r#"Unsupported("structural local carried a borrow event with no recorded loan")"#),
        "an unread view has no loan to replay, for either referent"
    );
}

/// The one record shape that lowers end to end reads its view with a member
/// access, and a borrowed primitive local has no such spelling:
/// `primitive_reference_read` admits only state parameters. A call is
/// therefore the only consumer `&u64` could have, and it is not supported for
/// the admitted record referent either. Closing it serves both referents; this
/// pins that it is one gap rather than two.
#[test]
fn borrowed_selection_call_consumers_reject_for_every_referent() {
    let (primitive_arms, primitive_lowering) = planned_borrow_arms(PRIMITIVE_CALL_SOURCE);
    let (record_arms, record_lowering) = planned_borrow_arms(RECORD_CALL_SOURCE);
    assert_eq!(primitive_arms.len(), 2);
    assert_eq!(record_arms.len(), 2);
    assert_eq!(
        primitive_lowering, record_lowering,
        "a call consumer rejects identically for both referents"
    );
    assert_eq!(
        primitive_lowering.as_deref(),
        Some(r#"Unsupported("machine has no source-independent checked scalar control plan")"#),
    );
}

#[test]
fn borrowed_selection_rejoins_each_arms_exact_source() {
    let checked = check_source(CHAINED_SOURCE).expect("borrowed chained selection checks");
    let (lent, planned) = checked_borrowed_selection(&checked);
    assert_eq!(
        lent.keys().collect::<Vec<_>>(),
        ["a", "b"],
        "the chained join result and the plain local each lend one field"
    );
    assert_eq!(lent["a"], "first");
    assert_eq!(lent["b"], "second");
    assert_eq!(planned.keys().collect::<Vec<_>>(), ["a", "b"]);

    for (selected, other, expected) in [
        (true, true, 3),
        (false, true, 3),
        (true, false, 7),
        (false, false, 7),
    ] {
        let (module, execution) = execute(
            CHAINED_SOURCE,
            &[
                TerminalScalarValue::Boolean(selected),
                TerminalScalarValue::Boolean(other),
            ],
        );
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "selected={selected} other={other}"
        );
        // The join is one record-shaped shared-borrow block parameter: no
        // owned transfer is fabricated for either arm's projected referent.
        assert_eq!(
            module.machines[0]
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters)
                .filter(|parameter| parameter.access == StructuralAccess::SharedBorrow)
                .count(),
            1,
            "one shared-borrow join parameter"
        );
    }
}

#[test]
fn borrowed_selection_rejoins_direct_local_sources() {
    let checked = check_source(UNCHAINED_SOURCE).expect("borrowed selection checks");
    let (lent, planned) = checked_borrowed_selection(&checked);
    assert_eq!(lent.keys().collect::<Vec<_>>(), ["b", "x"]);
    assert_eq!(lent["x"], "first");
    assert_eq!(lent["b"], "second");
    assert_eq!(planned.keys().collect::<Vec<_>>(), ["b", "x"]);

    for (other, expected) in [(true, 3), (false, 7)] {
        let (_, execution) = execute(
            UNCHAINED_SOURCE,
            &[
                TerminalScalarValue::Boolean(true),
                TerminalScalarValue::Boolean(other),
            ],
        );
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "other={other}"
        );
    }
}

#[test]
fn borrowed_selection_loans_constrain_sources_while_the_view_is_live() {
    let source = "data Payload { left: u64; right: u64; }
        data Pair { first: Payload; second: Payload; }
        machine choose(selected: bool) -> u64 {
            let mut a: Pair = Pair {
                first: Payload { left: 1, right: 2 },
                second: Payload { left: 3, right: 4 }
            };
            let mut b: Pair = Pair {
                first: Payload { left: 9, right: 10 },
                second: Payload { left: 11, right: 12 }
            };
            let view: &Payload = match selected {
                true -> &a.first,
                false -> &b.second
            };
            MUTATION
            view.left ^ view.right
        }";
    // Mutating either lent field while the view is live must reject; a
    // disjoint field of the same owner stays free.
    for (mutation, allowed) in [
        ("a.first = Payload { left: 0, right: 0 };", false),
        ("b.second = Payload { left: 0, right: 0 };", false),
        ("b.first = Payload { left: 0, right: 0 };", true),
    ] {
        let source = source.replace("MUTATION", mutation);
        match check_source(&source) {
            Ok(_) => assert!(allowed, "{mutation} cannot observe through the live view"),
            Err(errors) => {
                assert!(!allowed, "{mutation} must stay free: {errors:#?}");
                assert!(
                    errors
                        .iter()
                        .any(|error| error.message.contains("while local borrow")),
                    "{mutation} must reject through the recorded loan: {errors:?}"
                );
            }
        }
    }
}

#[test]
fn borrowed_selection_rejects_exclusive_and_aggregate_referents() {
    // `&mut` arms keep their own custody lane and never join as shared.
    let mutable = CHAINED_SOURCE
        .replace("let a: Pair", "let mut a: Pair")
        .replace("let b: Pair", "let mut b: Pair")
        .replace(
            "let view: &Payload = match other {
            true -> &a.first,
            false -> &b.second
        };",
            "let view: &mut Payload = match other {
            true -> &mut a.first,
            false -> &mut b.second
        };",
        );
    assert!(
        check_source(&mutable).is_err(),
        "exclusive match arms cannot join a shared-borrow result"
    );
    // A whole sum borrow is outside the record-only admission.
    let sum = "data Choice { case Empty; case Some(value: u32); }
        machine choose(selected: bool) -> bool {
            let left: Choice = Choice::Some { value: 37 };
            let right: Choice = Choice::Empty;
            let view: &Choice = match selected { true -> &left, false -> &right };
            view in Choice::Some
        }";
    let errors = check_source(sum).expect_err("a whole sum borrow keeps rejecting");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("match result requires a reference or non-plain-owned branch custody join")),
        "{errors:?}"
    );
    // Mixing a borrow arm with an owned carrier stays type-incompatible.
    let mixed = UNCHAINED_SOURCE.replace("false -> &b.second", "false -> b.second");
    assert!(
        check_source(&mixed).is_err(),
        "a `&Payload` result cannot join an owned `Payload` arm"
    );
}

/// A literal fixed-index projection is the same exact place a record field
/// is: each arm lends its exact element place for the view's whole live
/// range, and the retained `SharedBorrow` plan carries the literal ordinal in
/// its path so lowering and the verifier replay it segment for segment.
/// Array-literal fields produce no structural value node, so these locals
/// check but cannot lower; this fixture pins the recorded provenance.
const INDEXED_SOURCE: &str = "data Payload { left: u64; right: u64; }
    data Holder { items: [Payload; 2]; }
    machine choose(selected: bool) -> u64 {
        let x: Holder = Holder {
            items: [Payload { left: 1, right: 2 }, Payload { left: 3, right: 4 }]
        };
        let y: Holder = Holder {
            items: [Payload { left: 5, right: 6 }, Payload { left: 7, right: 8 }]
        };
        let view: &Payload = match selected {
            true -> &x.items[0],
            false -> &y.items[1]
        };
        view.left ^ view.right
    }";

/// The same indexed selection with the holders arriving as structural
/// parameters — the carrier this lowering route establishes end to end.
/// Each arm's source plan names the parameter's dense structural position,
/// never a fabricated local.
const INDEXED_PARAMETERS_SOURCE: &str = "data Payload { left: u64; right: u64; }
    data Holder { items: [Payload; 2]; }
    machine choose(selected: bool, x: Holder, y: Holder) -> u64 {
        let view: &Payload = match selected {
            true -> &x.items[0],
            false -> &y.items[1]
        };
        view.left ^ view.right
    }";

/// The exact loans behind `view`'s indexed arms, keyed by root name: each
/// arm lends its `items[ordinal]` element place for the view's whole live
/// range.
fn indexed_loans(checked: &checked_trees::CheckedTrees) -> BTreeMap<String, usize> {
    let (view_statement, view_symbol) = local(checked, "view");
    let mut lent = BTreeMap::new();
    for (_, loan) in checked.facts.borrow.loans.iter() {
        assert_eq!(loan.owner_symbol, view_symbol, "every arm loan owns `view`");
        assert_eq!(loan.statement_index, view_statement);
        assert_eq!(loan.last_use_statement_index, view_statement + 1);
        assert!(matches!(loan.kind, checked_trees::BorrowAccessKind::Read));
        let [
            facts::PlaceSegment::Field { symbol },
            facts::PlaceSegment::FixedIndex { index },
        ] = checked.facts.borrow.loan_segments(loan)
        else {
            panic!("each indexed arm lends an exact field-plus-ordinal element place");
        };
        assert_eq!(checked.typed.symbols.name(*symbol), "items");
        lent.insert(
            checked.typed.symbols.name(loan.root_symbol).to_owned(),
            *index,
        );
    }
    lent
}

/// Each retained `SharedBorrow` indexed arm's source plan paired with its
/// projected `items` ordinal, in plan order.
fn indexed_arms(
    checked: &checked_trees::CheckedTrees,
) -> Vec<(checked_trees::CheckedUnitStructuralArgumentSourcePlan, u64)> {
    borrowed_arms(checked)
        .iter()
        .map(|arm| {
            let [
                checked_trees::CheckedUnitStructuralPathSegment::Field(field),
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index),
            ] = arm.path.as_slice()
            else {
                panic!("indexed borrowed arm keeps the authored field+ordinal path: {arm:?}");
            };
            assert_eq!(field.as_str(), "items");
            (arm.source.clone(), *index)
        })
        .collect()
}

/// One record field's semantic identity within a retained structural type.
fn record_field(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    identity: &str,
) -> StructuralFieldId {
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .expect("retained input type");
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        panic!("record input");
    };
    fields
        .iter()
        .find(|field| field.identity == identity)
        .expect("exact authored field")
        .id
}

/// The element type `Holder.items` projects to, resolved through the
/// declaration chain so the test never assumes an id assignment.
fn holder_element_type(module: &TerminalModule, holder: StructuralTypeId) -> StructuralTypeId {
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == holder)
        .expect("retained holder type");
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        panic!("holder is a record");
    };
    let items = fields
        .iter()
        .find(|field| field.identity == "items")
        .expect("items field");
    let StructuralFieldType::Structural(array) = items.field_type else {
        panic!("items is a structural field");
    };
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == array)
        .expect("retained array type");
    let StructuralTypeShape::FixedArray { element, .. } = declaration.shape else {
        panic!("items is a fixed array");
    };
    element
}

/// Executes `INDEXED_PARAMETERS_SOURCE` with `selected`, supplying both
/// holder arguments with every element leaf bound: `x` carries
/// `[{1,2},{3,4}]` and `y` carries `[{5,6},{7,8}]` so `x.items[0]` reads
/// `1 ^ 2` and `y.items[1]` reads `7 ^ 8`.
fn execute_indexed_parameters(selected: bool) -> (TerminalModule, MeasuredTerminalExecution) {
    execute_machine_with_structural_inputs(
        INDEXED_PARAMETERS_SOURCE,
        "choose",
        &[TerminalScalarValue::Boolean(selected)],
        |module| {
            let entry = module
                .machines
                .iter()
                .find(|machine| machine.id == module.entry)
                .expect("entry machine");
            assert_eq!(
                entry.structural_parameters.len(),
                2,
                "x and y are the dense structural parameters"
            );
            for parameter in &entry.structural_parameters {
                assert_eq!(parameter.access, StructuralAccess::Owned);
            }
            let holder = entry.structural_parameters[0].structural_type;
            assert_eq!(entry.structural_parameters[1].structural_type, holder);
            let payload = holder_element_type(module, holder);
            let left = record_field(module, payload, "left");
            let right = record_field(module, payload, "right");
            let mut arguments = Vec::new();
            let mut fields = Vec::new();
            for (argument_index, elements) in [[(1u128, 2u128), (3, 4)], [(5, 6), (7, 8)]]
                .into_iter()
                .enumerate()
            {
                arguments.push(TerminalStructuralValue {
                    opaque_identity: 71 + argument_index as u64,
                    structural_type: holder,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                });
                for (ordinal, (left_value, right_value)) in elements.into_iter().enumerate() {
                    for (field, value) in [(left, left_value), (right, right_value)] {
                        fields.push(TerminalStructuralScalarFieldValue {
                            argument_index: argument_index as u32,
                            path: vec![
                                StructuralPathSegment::Field("items".to_owned()),
                                StructuralPathSegment::FixedIndex(ordinal as u64),
                            ],
                            field,
                            value: unsigned(value),
                        });
                    }
                }
            }
            (arguments, fields)
        },
    )
}

#[test]
fn borrowed_selection_keeps_indexed_local_provenance() {
    let checked = check_source(INDEXED_SOURCE).expect("indexed borrowed selection checks");
    let lent = indexed_loans(&checked);
    assert_eq!(lent.keys().collect::<Vec<_>>(), ["x", "y"]);
    assert_eq!(lent["x"], 0, "the true arm lends `x.items[0]`");
    assert_eq!(lent["y"], 1, "the false arm lends `y.items[1]`");
    let mut planned = BTreeMap::new();
    for (source, index) in indexed_arms(&checked) {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
            source
        else {
            panic!("a local-rooted indexed arm stays a structural local: {source:?}");
        };
        planned.insert(checked.typed.symbols.name(symbol).to_owned(), index);
    }
    assert_eq!(planned.keys().collect::<Vec<_>>(), ["x", "y"]);
    assert_eq!(planned["x"], 0);
    assert_eq!(planned["y"], 1);
}

#[test]
fn borrowed_selection_rejoins_parameter_rooted_indexed_places() {
    let checked = check_source(INDEXED_PARAMETERS_SOURCE)
        .expect("parameter-rooted indexed borrowed selection checks");
    let lent = indexed_loans(&checked);
    assert_eq!(lent.keys().collect::<Vec<_>>(), ["x", "y"]);
    assert_eq!(lent["x"], 0, "the true arm lends `x.items[0]`");
    assert_eq!(lent["y"], 1, "the false arm lends `y.items[1]`");
    let mut planned = BTreeMap::new();
    for (source, index) in indexed_arms(&checked) {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            source
        else {
            panic!("a parameter-rooted indexed arm keeps its dense position: {source:?}");
        };
        planned.insert(parameter_index, index);
    }
    assert_eq!(planned.keys().copied().collect::<Vec<u32>>(), [0, 1]);
    assert_eq!(planned[&0], 0, "parameter 0 (`x`) projects items[0]");
    assert_eq!(planned[&1], 1, "parameter 1 (`y`) projects items[1]");

    for (selected, expected) in [(true, 3), (false, 15)] {
        let (module, execution) = execute_indexed_parameters(selected);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "selected={selected}"
        );
        // The join is still one record-shaped shared-borrow block parameter:
        // the indexed arms add path segments, not owned transfers.
        assert_eq!(
            module.machines[0]
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters)
                .filter(|parameter| parameter.access == StructuralAccess::SharedBorrow)
                .count(),
            1,
            "one shared-borrow join parameter"
        );
    }
}

#[test]
fn borrowed_selection_indexed_loans_constrain_their_exact_elements() {
    // Mutating the lent element while the view is live must reject; a sibling
    // element of the same array field and the other root's element stay free.
    for (mutation, allowed) in [
        ("x.items[0] = Payload { left: 0, right: 0 };", false),
        ("y.items[1] = Payload { left: 0, right: 0 };", false),
        ("x.items[1] = Payload { left: 0, right: 0 };", true),
    ] {
        let source = INDEXED_SOURCE
            .replace("let x: Holder", "let mut x: Holder")
            .replace("let y: Holder", "let mut y: Holder")
            .replace(
                "view.left ^ view.right",
                "MUTATION\n            view.left ^ view.right",
            )
            .replace("MUTATION", mutation);
        match check_source(&source) {
            Ok(_) => assert!(allowed, "{mutation} cannot observe through the live view"),
            Err(errors) => {
                assert!(!allowed, "{mutation} must stay free: {errors:#?}");
                assert!(
                    errors
                        .iter()
                        .any(|error| error.message.contains("while local borrow")),
                    "{mutation} must reject through the recorded loan: {errors:?}"
                );
            }
        }
    }
}

#[test]
fn borrowed_selection_indexed_replay_rejects_mutated_provenance() {
    let checked = check_source(INDEXED_PARAMETERS_SOURCE)
        .expect("parameter-rooted indexed borrowed selection checks");
    let arm_handles: Vec<_> = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .filter_map(|(handle, node)| match &node.kind {
            checked_trees::CheckedStructuralValueKind::Reference { source }
                if source.access == checked_trees::CheckedStructuralAccess::SharedBorrow =>
            {
                Some(handle)
            }
            _ => None,
        })
        .collect();
    assert_eq!(arm_handles.len(), 2);
    for mutation in 0..4 {
        let mut changed = checked.clone();
        for handle in &arm_handles {
            let checked_trees::CheckedStructuralValueKind::Reference { source } = &mut changed
                .facts
                .values
                .structural_values
                .nodes
                .get_mut(*handle)
                .kind
            else {
                continue;
            };
            match mutation {
                // A changed ordinal can no longer replay the authored
                // `&x.items[0]` / `&y.items[1]` expressions.
                0 => {
                    source.path = vec![
                        checked_trees::CheckedUnitStructuralPathSegment::Field("items".to_owned()),
                        checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(9),
                    ];
                }
                // Dropping the ordinal fabricates a whole-field borrow the
                // authored target never named.
                1 => {
                    source.path = vec![checked_trees::CheckedUnitStructuralPathSegment::Field(
                        "items".to_owned(),
                    )];
                }
                // Pointing each arm at the other parameter moves the
                // borrow's exact root.
                2 => {
                    source.source =
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                            parameter_index: match source.source {
                                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                    parameter_index,
                                } => 1 - parameter_index,
                                _ => panic!("indexed arms keep parameter roots"),
                            },
                        };
                }
                // An owned join fabricates custody the shared borrow never
                // carried.
                _ => source.access = checked_trees::CheckedStructuralAccess::Owned,
            }
        }
        let error = checked_trees_to_lowered_psi::lower_machine(&changed, "choose")
            .expect_err("mutated indexed provenance must reject before the verifier boundary");
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::Unsupported(_)
            ),
            "mutation {mutation} fails at source replay: {error:?}"
        );
    }
}

#[test]
fn borrowed_selection_replay_rejects_mutated_arm_provenance() {
    let checked = check_source(CHAINED_SOURCE).expect("borrowed chained selection checks");
    // Every SharedBorrow arm mutates through the same plan site: swap each
    // arm's root symbol for the other arm's root, then its field path.
    let arm_roots: Vec<symbols::SymbolHandle> = borrowed_arms(&checked)
        .iter()
        .map(|arm| match arm.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                symbol
            }
            _ => panic!("borrowed arm roots at an exact local"),
        })
        .collect();
    assert_eq!(arm_roots.len(), 2);
    let arm_handles: Vec<_> = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .filter_map(|(handle, node)| match &node.kind {
            checked_trees::CheckedStructuralValueKind::Reference { source }
                if source.access == checked_trees::CheckedStructuralAccess::SharedBorrow =>
            {
                Some(handle)
            }
            _ => None,
        })
        .collect();
    assert_eq!(arm_handles.len(), 2);
    for mutation in 0..3 {
        let mut changed = checked.clone();
        for handle in &arm_handles {
            let checked_trees::CheckedStructuralValueKind::Reference { source } = &mut changed
                .facts
                .values
                .structural_values
                .nodes
                .get_mut(*handle)
                .kind
            else {
                continue;
            };
            match mutation {
                // Substituting the other arm's root moves the borrow's origin.
                0 => {
                    source.source =
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                            symbol: arm_roots
                                .iter()
                                .copied()
                                .find(|root| match source.source {
                                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                                        *root != symbol
                                    }
                                    _ => false,
                                })
                                .expect("the sibling arm supplies another root"),
                        };
                }
                // A changed field path can no longer replay the authored
                // `&place` expression.
                1 => {
                    source.path = vec![checked_trees::CheckedUnitStructuralPathSegment::Field(
                        "right".to_owned(),
                    )];
                }
                // An owned join fabricates custody the shared borrow never
                // carried.
                _ => source.access = checked_trees::CheckedStructuralAccess::Owned,
            }
        }
        let error = checked_trees_to_lowered_psi::lower_machine(&changed, "choose")
            .expect_err("mutated borrowed provenance must reject before the verifier boundary");
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::Unsupported(_)
            ),
            "mutation {mutation} fails at source replay: {error:?}"
        );
    }
}
