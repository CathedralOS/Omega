//! Shared-borrow `&T` match results rejoin borrowed custody at the selection's
//! block parameter. The referent's owner is never transferred: each arm's
//! exact source place stays constrained for the result's whole live range,
//! the retained plan is replayed against the authored `&place` expressions,
//! and the join is emitted as a `SharedBorrow` block parameter rather than a
//! fabricated owned transfer. The terminal verifier admits the record-shaped
//! join and the canonical `PrimitiveScalar` leaf join against the exact root
//! and projected path, keeps the referent pinned for the block that observes
//! it, and the interpreter binds the same view without copying the payload.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use std::collections::BTreeMap;

use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};
use terminal_psi::{OperationKind, StructuralAccess, StructuralTypeShape, TerminalModule};

use super::{execute, unsigned};

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

/// Passing the join result to a call is the only consumer a borrowed primitive
/// local can have today, so the record referent is asked the same question.
/// `read`'s `&u64` body plans a source-independent `PrimitiveScalarRead`,
/// `read(view)` rejoins `view` as a whole `PrimitiveScalar` shared argument,
/// and the selection's join is itself a `PrimitiveScalar` shared-borrow block
/// parameter — the canonical referent shape the verifier admits — so the
/// whole program lowers and verifies without copying the leaf.
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

/// The same forwarding without a selection: a bare `&a` initializer
/// establishes the shared-borrow carrier directly, so the call observes the
/// local's exact place without a join in between.
const DIRECT_FORWARD_SOURCE: &str = "data Payload { left: u64; right: u64; }
    machine read(value: &Payload) -> u64 { value.left ^ value.right }
    machine choose() -> u64 {
        let a: Payload = Payload { left: 1, right: 2 };
        let view: &Payload = &a;
        read(view)
    }";

/// Each planned `SharedBorrow` arm as (root local name, its authored field
/// path joined by `.`), sorted, beside the machine's lowering outcome: `None`
/// when it lowers, otherwise the exact rejection. A whole-place borrow such as
/// `&a` carries no segments and reports an empty path.
fn planned_borrow_arms(source: &str) -> (Vec<(String, String)>, Option<String>) {
    let checked = crate::front_end::checked_program(source);
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
    let lowering = match checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("choose"),
    ) {
        Ok(_) => None,
        Err(error) => Some(format!("{error:?}")),
    };
    (arms, lowering)
}

/// Lowers `choose`, round-trips the semantic module through the codec, and
/// verifies the decoded copy — the produced-module evidence for a shape the
/// interpreter has no executable lane for yet.
fn verify_lowered(source: &str) -> TerminalModule {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("choose"),
    )
    .unwrap_or_else(|error| panic!("lowering {source}: {error:#?}"));
    let semantic_bytes =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof");
    let module = terminal_codec::decode_module(&semantic_bytes).expect("decode semantics");
    let proof = terminal_codec::decode_proof_bundle(&proof_bytes).expect("decode proof");
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the produced module verifies independently");
    module
}

/// A `&Payload` local forwards the referent it already loans to a shared
/// formal: the callee observes the exact joined place rather than a copied
/// record. The primitive referent runs the identical call through lowering
/// and verification: `read`'s `&u64` body is an admitted source-independent
/// `PrimitiveScalarRead` plan, `read(view)` retains `view` as a whole
/// `PrimitiveScalar` `StructuralLocal` `SharedBorrow` argument, and the
/// established `&u64` carrier's own join is the same canonical
/// `PrimitiveScalar` shared-borrow block parameter the callee formal
/// declares — no owned scalar copy of the referent exists anywhere in
/// between.
#[test]
fn borrowed_selection_call_consumer_forwards_the_established_view() {
    let (record_arms, record_lowering) = planned_borrow_arms(RECORD_CALL_SOURCE);
    assert_eq!(
        record_arms,
        vec![
            ("a".to_owned(), String::new()),
            ("b".to_owned(), String::new())
        ],
        "each whole-record arm keeps its authored root"
    );
    assert_eq!(record_lowering, None, "the forwarded record view lowers");

    for (other, expected) in [(true, 3), (false, 7)] {
        let (module, execution) =
            execute(RECORD_CALL_SOURCE, &[TerminalScalarValue::Boolean(other)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "other={other}"
        );
        let choose = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .expect("entry choose");
        // The join is still one shared-borrow block parameter, and the call
        // observes that exact place — no owned copy of the record.
        let join = choose
            .blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .filter(|parameter| parameter.access == StructuralAccess::SharedBorrow)
            .collect::<Vec<_>>();
        assert_eq!(join.len(), 1, "one shared-borrow join parameter");
        let calls = choose
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                } => Some(structural_arguments),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [call_arguments] = calls.as_slice() else {
            panic!("choose invokes read exactly once: {calls:?}");
        };
        let [argument] = call_arguments.as_slice() else {
            panic!("read takes one structural argument: {call_arguments:?}");
        };
        assert_eq!(argument.access, StructuralAccess::SharedBorrow);
        assert!(argument.path.is_empty());
        assert_eq!(
            argument.place, join[0].place,
            "the callee observes the join's exact shared place"
        );
    }

    // The primitive referent plans the same call: `read(view)` retains a
    // `StructuralLocal` `SharedBorrow` argument for `view`, and `read` itself
    // is an admitted source-independent `PrimitiveScalarRead` callee. The
    // established `&u64` join is now admitted too, so the lowered module is
    // asserted directly below.
    let checked = crate::front_end::checked_program(PRIMITIVE_CALL_SOURCE);
    let (_, view_symbol) = local(&checked, "view");
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
    let call_arguments: Vec<_> = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter_map(|(_, root)| {
            let node = checked
                .facts
                .values
                .scalar_computations
                .nodes
                .get(root.root);
            match &node.kind {
                checked_trees::CheckedScalarComputationKind::Call {
                    structural_arguments,
                    ..
                } if root.state == state.symbol => Some(
                    checked
                        .facts
                        .values
                        .scalar_computations
                        .structural_arguments
                        .span(*structural_arguments),
                ),
                _ => None,
            }
        })
        .collect();
    let [Some(arguments)] = call_arguments.as_slice() else {
        panic!("choose plans exactly one structural call: {call_arguments:?}");
    };
    let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] = *arguments
    else {
        panic!("read's argument is one place: {arguments:?}");
    };
    assert_eq!(
        argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: view_symbol
        },
        "the call observes `view` itself"
    );
    assert_eq!(
        argument.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    assert!(argument.path.is_empty());

    // The same `&u64` view now lowers and verifies end to end: the
    // selection's join declares the canonical `PrimitiveScalar` referent
    // under `SharedBorrow`, and `read` observes that exact place whole. The
    // interpreter's own operand lane for a primitive-typed block parameter is
    // a separate consumer boundary — this test pins the produced module, not
    // execution.
    let module = verify_lowered(PRIMITIVE_CALL_SOURCE);
    let choose = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry choose");
    let join = choose
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .filter(|parameter| parameter.access == StructuralAccess::SharedBorrow)
        .collect::<Vec<_>>();
    let [join] = join.as_slice() else {
        panic!("one shared-borrow join parameter: {join:?}");
    };
    let referent = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == join.structural_type)
        .expect("the join's declared type exists");
    assert!(
        matches!(referent.shape, StructuralTypeShape::PrimitiveScalar(_)),
        "the `&u64` join declares the canonical scalar referent: {referent:?}"
    );
    let calls = choose
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } => Some(structural_arguments),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [call_arguments] = calls.as_slice() else {
        panic!("choose invokes read exactly once: {calls:?}");
    };
    let [call_argument] = call_arguments.as_slice() else {
        panic!("read takes one structural argument: {call_arguments:?}");
    };
    assert_eq!(call_argument.access, StructuralAccess::SharedBorrow);
    assert!(call_argument.path.is_empty());
    assert_eq!(
        call_argument.place, join.place,
        "the callee observes the join's exact shared place"
    );
}

/// A non-selection `let view: &Payload = &a` establishment produces the same
/// `SharedBorrow` `Reference` root a selection arm does, so the checked unit
/// sequence emits its `EstablishStructuralValue` and the forwarded `read(view)`
/// call observes the exact borrowed place end to end.
///
/// Everything downstream of the establishment was already in place: the loan
/// `view` takes on `a` is recorded for exactly its live range, and the
/// `read(view)` call's scalar computation already plans `view` as a
/// `SharedBorrow` `StructuralLocal` argument through the shared
/// nominal-argument lane. What the driver was missing is the establishment
/// itself: the `&a` initializer is a bare `ExpressionNode::Borrow`, which only
/// `borrowed_place` can plan -- `is_shared_borrow_value` now routes a
/// `LocalData` bare-`Borrow` initializer with a `shared_record_reference`
/// carrier through it, so the statement gains its `Reference` root and the
/// unit-effect lane covers `choose`.
#[test]
fn established_reference_local_call_forwards_the_direct_borrow() {
    let checked = crate::front_end::checked_program(DIRECT_FORWARD_SOURCE);
    let (view_statement, view_symbol) = local(&checked, "view");
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

    // The established-borrow evidence already exists: `view` loans `a` for
    // exactly the initializer-to-call range the selection join records.
    let loans = checked
        .facts
        .borrow
        .loans
        .iter()
        .map(|(_, loan)| loan)
        .collect::<Vec<_>>();
    let [loan] = loans.as_slice() else {
        panic!("`let view = &a` records exactly one loan");
    };
    assert_eq!(loan.owner_symbol, view_symbol);
    assert_eq!(loan.statement_index, view_statement);
    assert_eq!(loan.last_use_statement_index, view_statement + 1);
    assert!(matches!(loan.kind, checked_trees::BorrowAccessKind::Read));
    let a_symbol = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local)
                if checked.typed.symbols.name(local.symbol) == "a" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("local `a` exists");
    assert_eq!(loan.root_symbol, a_symbol, "the direct borrow lends `a`");

    // The call consumer is already planned: `read(view)`'s scalar computation
    // forwards `view` as the shared-borrow `StructuralLocal` argument, exactly
    // as the selection-join lane emits. Only the establishment plan is absent.
    let call_argument = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter_map(|(_, root)| {
            let node = checked
                .facts
                .values
                .scalar_computations
                .nodes
                .get(root.root);
            match &node.kind {
                checked_trees::CheckedScalarComputationKind::Call {
                    structural_arguments,
                    ..
                } if root.state == state.symbol => Some(
                    checked
                        .facts
                        .values
                        .scalar_computations
                        .structural_arguments
                        .span(*structural_arguments),
                ),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    let [Some(arguments)] = call_argument.as_slice() else {
        panic!("choose plans exactly one structural call: {call_argument:?}");
    };
    let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] = *arguments
    else {
        panic!("read's argument is one place: {arguments:?}");
    };
    assert_eq!(
        argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: view_symbol
        },
        "the call observes `view` itself"
    );
    assert_eq!(
        argument.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    assert!(argument.path.is_empty());

    // The establishment itself now plans: `&a` produces a `Reference` root at
    // the statement whose `SharedBorrow` source names the exact local `a`
    // with no projection, exactly as a selection arm's `&a` would.
    let root = checked
        .facts
        .values
        .structural_values
        .root_at(state.symbol, u32::try_from(view_statement).unwrap())
        .expect("a bare `&a` initializer produces a structural-value root");
    let node = checked.facts.values.structural_values.nodes.get(root.root);
    let checked_trees::CheckedStructuralValueKind::Reference { source } = &node.kind else {
        panic!("the direct borrow establishes a shared-borrow reference: {node:?}");
    };
    assert_eq!(
        source.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: a_symbol
        },
        "the borrowed establishment names its exact local"
    );
    assert_eq!(
        source.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    assert!(source.path.is_empty(), "a whole-place borrow has no path");

    // The unit-effect lane carries `choose` end to end. The scalar-graph lane
    // still does not retain a `Reference` root, exactly as a selection join's
    // `Dispatch` root stays outside `record_value_root`.
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .is_some(),
        "choose has a unit-effect plan to lower"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine.symbol)
            .is_none(),
        "choose still has no scalar-graph plan"
    );

    // `read(view)` observes the exact borrowed place: `a` is copied nowhere
    // and the call reads `1 ^ 2` through the established view.
    let (_, execution) = execute(DIRECT_FORWARD_SOURCE, &[]);
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(unsigned(3)),
        "the forwarded direct borrow executes"
    );
}

#[test]
fn borrowed_selection_rejoins_each_arms_exact_source() {
    let checked = crate::front_end::checked_program(CHAINED_SOURCE);
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
        match crate::front_end::checked_program_result(&source) {
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
        crate::front_end::checked_program_result(&mutable).is_err(),
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
    let errors = crate::front_end::checked_program_result(sum)
        .expect_err("a whole sum borrow keeps rejecting");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("match result requires a reference or non-plain-owned branch custody join")),
        "{errors:?}"
    );
    // Mixing a borrow arm with an owned carrier stays type-incompatible.
    let mixed = UNCHAINED_SOURCE.replace("false -> &b.second", "false -> b.second");
    assert!(
        crate::front_end::checked_program_result(&mixed).is_err(),
        "a `&Payload` result cannot join an owned `Payload` arm"
    );
}

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

#[test]
fn borrowed_selection_indexed_replay_rejects_mutated_provenance() {
    let checked = crate::front_end::checked_program(INDEXED_PARAMETERS_SOURCE);
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
        let error = checked_trees_to_lowered_psi::lower_machine(
            &changed,
            TerminalMachineSelection::Name("choose"),
        )
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
