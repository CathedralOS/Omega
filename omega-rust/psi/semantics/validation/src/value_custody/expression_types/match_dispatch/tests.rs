//! Selected-arm custody admission for value dispatch.
//!
//! These pin the branch-custody gate itself, not the downstream plan: the
//! checked and lowering stages replay each admitted arm's exact root and path
//! independently, and a shape this gate rejects never reaches them.

use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

const CUSTODY_JOIN_REJECTION: &str =
    "match result requires a reference or non-plain-owned branch custody join";

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolved source");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed source")
}

/// Every dispatch authored in the source, validated in the one machine state
/// that owns it. Each source below declares exactly one machine, so its entry
/// state is the only scope any of these dispatches can belong to; validating
/// an arm under a foreign scope would report unrelated diagnostics.
fn choose_dispatch_diagnostics(source: &str) -> Vec<String> {
    let program = typed(source);
    let [machine] = program.machines() else {
        panic!("these fixtures declare exactly one machine");
    };
    let state = program
        .machine_states(machine)
        .first()
        .expect("the machine has an entry state");
    let mut messages = Vec::new();
    for (expression, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Match(dispatch) = node else {
            continue;
        };
        let mut diagnostics = Vec::new();
        super::validate_match_dispatch(
            &program,
            machine,
            state,
            expression,
            dispatch,
            &mut diagnostics,
        );
        messages.extend(diagnostics.into_iter().map(|entry| entry.message));
    }
    messages.sort();
    messages.dedup();
    messages
}

const LINEAR_TOKEN: &str = "data Token [linear] { code: u64; }\n";

/// A shared borrow observes its referent and moves nothing, so a linear
/// referent's exactly-once claim never reaches the selection join: each arm's
/// own owner keeps it on every edge. Whole places and exact projections out of
/// an affine holder are the same admission.
#[test]
fn shared_borrow_arms_join_a_linear_referent_without_an_owned_custody_join() {
    for (label, source) in [
        (
            "whole immutable locals",
            format!(
                "{LINEAR_TOKEN}machine choose(other: bool, a: Token, b: Token) -> u64 {{
                     let view: &Token = match other {{ true -> &a, false -> &b }};
                     view.code
                 }}"
            ),
        ),
        (
            "whole state parameters",
            format!(
                "{LINEAR_TOKEN}machine choose(other: bool, first: Token, second: Token) -> u64 {{
                     let view: &Token = match other {{ true -> &first, false -> &second }};
                     view.code
                 }}"
            ),
        ),
        (
            "projected linear fields of affine holders",
            format!(
                "{LINEAR_TOKEN}data Holder {{ left: Token; right: Token; }}
                 machine choose(other: bool, x: Holder, y: Holder) -> u64 {{
                     let view: &Token = match other {{ true -> &x.left, false -> &y.right }};
                     view.code
                 }}"
            ),
        ),
    ] {
        assert!(
            choose_dispatch_diagnostics(&source).is_empty(),
            "{label}: a shared borrow of a linear referent joins without an owned custody join: {:?}",
            choose_dispatch_diagnostics(&source),
        );
    }
}

/// A primitive referent joins on exactly the shared-borrow terms a record one
/// does. `&u64` denotes the original storage, so the selection still merges no
/// owned custody; the only thing it lacked was a carrier, and the structural
/// pipeline builds that from the primitive itself. Whole places and exact
/// field projections are the same admission here as for a record referent.
#[test]
fn shared_borrow_arms_join_a_primitive_referent_without_a_data_declaration() {
    for (label, source) in [
        (
            "projected primitive fields",
            "data Payload { left: u64; right: u64; }
             machine choose(other: bool) -> u64 {
                 let a: Payload = Payload { left: 1, right: 2 };
                 let b: Payload = Payload { left: 3, right: 4 };
                 let view: &u64 = match other { true -> &a.left, false -> &b.right };
                 0
             }",
        ),
        (
            "whole primitive state parameters",
            "machine choose(other: bool, first: u64, second: u64) -> u64 {
                 let view: &u64 = match other { true -> &first, false -> &second };
                 0
             }",
        ),
    ] {
        let messages = choose_dispatch_diagnostics(source);
        assert!(
            messages.is_empty(),
            "{label}: a shared borrow of a primitive referent joins without an owned custody join: {messages:?}",
        );
    }
}

/// A literal fixed-index segment is the same exact place a record field is:
/// the authored target canonicalizes to a named root plus Field/FixedIndex
/// segments, the indexed collection's element declaration supplies the
/// referent record, and the borrowed leaf still moves nothing. The join
/// replays the retained path segment for segment.
#[test]
fn shared_borrow_arms_join_a_fixed_index_projection() {
    let source = "data Payload { left: u64; right: u64; }
         data Holder { items: [Payload; 2]; }
         machine choose(other: bool) -> u64 {
             let x: Holder = Holder {
                 items: [Payload { left: 1, right: 2 }, Payload { left: 3, right: 4 }]
             };
             let y: Holder = Holder {
                 items: [Payload { left: 5, right: 6 }, Payload { left: 7, right: 8 }]
             };
             let view: &Payload = match other {
                 true -> &x.items[0],
                 false -> &y.items[1]
             };
             view.left ^ view.right
         }";
    assert!(
        choose_dispatch_diagnostics(source).is_empty(),
        "a fixed-index projection of a carrier record joins borrowed custody: {:?}",
        choose_dispatch_diagnostics(source),
    );
}

/// The admitted carrier is still the record shape the structural pipeline
/// carries. A referent holding a loan is not that shape: the join would have
/// to name a borrowed leaf's own lifetime, which this gate cannot prove.
#[test]
fn shared_borrow_arms_reject_a_linear_referent_that_holds_a_loan() {
    let source = "data Watched [linear] { seen: &u64; }
         machine choose(other: bool, a: Watched, b: Watched) -> u64 {
             let view: &Watched = match other { true -> &a, false -> &b };
             0
         }";
    let messages = choose_dispatch_diagnostics(source);
    assert!(
        messages
            .iter()
            .any(|message| message.contains(CUSTODY_JOIN_REJECTION)),
        "a referent carrying a loan is outside the admitted carrier: {messages:?}",
    );
}

/// Linear tolerance is about the referent's claim, not about access or shape.
/// An exclusive carrier has affine custody of its own and a case-bearing
/// referent is outside the record-shaped frontier; both keep rejecting.
#[test]
fn shared_borrow_arms_still_reject_exclusive_and_case_bearing_linear_referents() {
    for (label, source) in [
        (
            "exclusive access",
            format!(
                "{LINEAR_TOKEN}machine choose(other: bool, a: Token, b: Token) -> u64 {{
                     let mut left: Token = a;
                     let mut right: Token = b;
                     let view: &mut Token = match other {{ true -> &mut left, false -> &mut right }};
                     0
                 }}"
            ),
        ),
        (
            "case-bearing referent",
            "data Choice [linear] { case Empty; case Some(value: u32); }
             machine choose(other: bool, a: Choice, b: Choice) -> u64 {
                 let view: &Choice = match other { true -> &a, false -> &b };
                 0
             }"
            .to_owned(),
        ),
    ] {
        let messages = choose_dispatch_diagnostics(&source);
        assert!(
            messages
                .iter()
                .any(|message| message.contains(CUSTODY_JOIN_REJECTION)),
            "{label}: must still reject with a diagnostic naming the missing join: {messages:?}",
        );
    }
}

/// The owned linear side is admitted at every claimed place: an exact
/// projection moves the child claim its path names, and the whole root that
/// holds linear children joins the same way, with the transfer's claim set
/// naming each consumed child claim. A root carries one claim place per
/// frontier child and none for the whole place, so a whole-root arm records
/// an empty moved path plus one claim row per frontier child — the receipt,
/// not an invented aggregate root claim, is what discharges the children.
#[test]
fn whole_affine_root_with_linear_children_joins_like_its_projected_child() {
    const ONE_CHILD: &str = "data Holder { left: Token; }\n";
    const TWO_CHILDREN: &str = "data Pair { left: Token; right: Token; }\n";
    let projected = format!(
        "{LINEAR_TOKEN}{ONE_CHILD}machine choose(other: bool, x: Holder) -> u64 {{
             let picked: Token = match other {{ true -> x.left, false -> x.left }};
             0
         }}"
    );
    assert!(
        choose_dispatch_diagnostics(&projected).is_empty(),
        "an exact projection of a linear child joins on its own claim path: {:?}",
        choose_dispatch_diagnostics(&projected),
    );
    for (label, source) in [
        (
            "one linear child, uniform source",
            format!(
                "{LINEAR_TOKEN}{ONE_CHILD}machine choose(other: bool, x: Holder) -> u64 {{
                     let picked: Holder = match other {{ true -> x, false -> x }};
                     0
                 }}"
            ),
        ),
        (
            "one linear child, distinct sources",
            format!(
                "{LINEAR_TOKEN}{ONE_CHILD}machine choose(other: bool, x: Holder, y: Holder) -> u64 {{
                     let picked: Holder = match other {{ true -> x, false -> y }};
                     0
                 }}"
            ),
        ),
        (
            "two linear children, uniform source",
            format!(
                "{LINEAR_TOKEN}{TWO_CHILDREN}machine choose(other: bool, x: Pair) -> u64 {{
                     let picked: Pair = match other {{ true -> x, false -> x }};
                     0
                 }}"
            ),
        ),
        (
            "two linear children, distinct sources",
            format!(
                "{LINEAR_TOKEN}{TWO_CHILDREN}machine choose(other: bool, x: Pair, y: Pair) -> u64 {{
                     let picked: Pair = match other {{ true -> x, false -> y }};
                     0
                 }}"
            ),
        ),
    ] {
        let messages = choose_dispatch_diagnostics(&source);
        assert!(
            messages.is_empty(),
            "{label}: the whole carrier root joins by naming each child's exact claim: {messages:?}",
        );
    }
}

/// A constant-folded index names the same fixed ordinal a literal spells:
/// the gate follows `constant_integer_value`, which is exactly the fold the
/// checked canonicalizer bakes into a `FixedIndex` segment, so `items[0 + 1]`
/// joins under the identical projection rule as `items[1]`. The affine leaf
/// and the linear leaf claim ride the same widened rule.
#[test]
fn projected_arms_join_a_constant_folded_index_like_a_literal() {
    for (label, source) in [
        (
            "affine leaf",
            "data Cell { tag: u64; }
             data Pack { items: [Cell; 2]; }
             machine choose(other: bool, x: Pack, y: Pack) -> u64 {
                 let picked: Cell = match other { true -> x.items[0 + 1], false -> y.items[1] };
                 picked.tag
             }",
        ),
        (
            "linear leaf",
            "data Token [linear] { code: u64; }
             data Crate { items: [Token; 2]; }
             machine choose(other: bool, x: Crate, y: Crate) -> u64 {
                 let picked: Token = match other { true -> x.items[4 - 3], false -> y.items[1] };
                 0
             }",
        ),
        (
            "nested fold under a field",
            "data Cell { tag: u64; }
             data Pack { items: [Cell; 2]; }
             data Shelf { pack: Pack; }
             machine choose(other: bool, shelf: Shelf) -> u64 {
                 let picked: Cell = match other { true -> shelf.pack.items[(6 / 3) - 1], false -> shelf.pack.items[0] };
                 picked.tag
             }",
        ),
    ] {
        let messages = choose_dispatch_diagnostics(source);
        assert!(
            messages.is_empty(),
            "{label}: a constant fold is the same fixed ordinal a literal is: {messages:?}",
        );
    }
}

/// The widened index rule still names only fixed ordinals: a place read, a
/// partially dynamic fold, or a constant fold that leaves `usize` (a
/// negative ordinal) has no statically checkable position, so it keeps
/// failing the join with the same rejection a dynamic index always drew.
#[test]
fn projected_arms_still_reject_indexes_without_a_fixed_ordinal() {
    for (label, index) in [
        ("place read", "slot"),
        ("partially dynamic fold", "1 + slot"),
        ("negative fold", "0 - 1"),
    ] {
        let source = format!(
            "data Cell {{ tag: u64; }}
             data Pack {{ items: [Cell; 2]; }}
             machine choose(other: bool, slot: u64, x: Pack, y: Pack) -> u64 {{
                 let picked: Cell = match other {{ true -> x.items[{index}], false -> y.items[1] }};
                 picked.tag
             }}"
        );
        let messages = choose_dispatch_diagnostics(&source);
        assert!(
            messages
                .iter()
                .any(|message| message.contains(CUSTODY_JOIN_REJECTION)),
            "{label}: `{index}` has no fixed ordinal and must keep rejecting: {messages:?}",
        );
    }
}
