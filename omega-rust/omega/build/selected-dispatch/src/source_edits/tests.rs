//! One-field mutation matrix over the retained selected-dispatch source-edit
//! journal.
//!
//! `SelectedDispatchSourceEdits` is the custody record a settlement owner
//! seals after rewriting dispatch sites: every batch retains the replaced
//! nodes plus a `GraphGuard` over the post-dispatch program, and
//! `source_trees` is the only replay — it validates the last batch against the
//! settled program before any node is restored, revalidates each earlier
//! batch mid-restore, then returns one scratch source view. Every
//! representable journal field is substituted independently below: the batch
//! roster and each batch's edit roster (omission, duplication, reordering,
//! foreign commitments), every `ExpressionEdit` and `ExpressionArguments`
//! field, and the sealed guard itself. Each substitution either rejects before
//! the restored view is published or restores an observably different program;
//! substitutions replaying identically are named canonicalizations, not
//! committed identity.

use super::guard::GraphGuard;
use super::records::{Batch, ExpressionArguments};
use super::{SelectedDispatchSourceEdits, SourceEditBuilder};
use arena::{Handle, HandleSpan};
use std::borrow::Cow;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
    TableUnaryExpression, UnaryOperator,
};
use typed_trees::name::Identifier;

fn static_argument(symbol: SymbolHandle) -> StaticMachineArgument {
    StaticMachineArgument {
        path: Box::default(),
        application: None,
        const_literal: None,
        evidence_projection: None,
        symbol,
    }
}

fn call_node(
    receiver: ExpressionHandle,
    target_symbol: SymbolHandle,
    static_target: SymbolHandle,
    arguments: HandleSpan<ExpressionHandle>,
) -> ExpressionNode {
    ExpressionNode::Call(TableCallExpression {
        receiver,
        target_symbol,
        static_machine_parameter: SymbolHandle::invalid(),
        target: Identifier::generated("run"),
        static_requirement_dispatch: None,
        machine_arguments: vec![static_argument(static_target)].into_boxed_slice(),
        quotient_operation: None,
        private_layout_operation: None,
        arguments,
        evidence_arguments: Box::default(),
        operational_acknowledgement: Default::default(),
    })
}

/// The pre-dispatch program plus the handles later stages rewrite.
struct Stages {
    program: TypedTrees,
    call: ExpressionHandle,
    unary: ExpressionHandle,
    receiver: ExpressionHandle,
    operand: ExpressionHandle,
    extra: ExpressionHandle,
    first_argument: ExpressionHandle,
    second_argument: ExpressionHandle,
    argument_span: HandleSpan<ExpressionHandle>,
    equal_argument_span: HandleSpan<ExpressionHandle>,
    foreign_argument_span: HandleSpan<ExpressionHandle>,
    target: SymbolHandle,
    static_target: SymbolHandle,
}

fn stages() -> Stages {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let target = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("Runs"));
    let static_target = symbols.insert_root(SymbolKind::Module, SymbolNameRef::Borrowed("Static"));
    program.symbols = symbols.finish();

    let receiver = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let first_argument = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let second_argument = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let operand = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let extra = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    // One canonical argument list plus equal-content and foreign-content spans
    // so `ExpressionArguments.span` can be substituted without touching the
    // recorded argument list.
    let argument_span = program
        .expression_table
        .insert_expression_handles([first_argument, second_argument]);
    let equal_argument_span = program
        .expression_table
        .insert_expression_handles([first_argument, second_argument]);
    let foreign_argument_span = program
        .expression_table
        .insert_expression_handles([second_argument]);
    let call =
        program
            .expression_table
            .insert(call_node(receiver, target, static_target, argument_span));
    let unary = program
        .expression_table
        .insert(ExpressionNode::Unary(TableUnaryExpression {
            operator: UnaryOperator::LogicalNot,
            operand,
        }));

    Stages {
        program,
        call,
        unary,
        receiver,
        operand,
        extra,
        first_argument,
        second_argument,
        argument_span,
        equal_argument_span,
        foreign_argument_span,
        target,
        static_target,
    }
}

/// An honest journal plus the programs it replays between: `original` is the
/// pre-dispatch source and `settled` is the post-dispatch program the journal
/// is sealed against.
struct JournalFixture {
    original: TypedTrees,
    settled: TypedTrees,
    journal: SelectedDispatchSourceEdits,
    call: ExpressionHandle,
    unary: ExpressionHandle,
    receiver: ExpressionHandle,
    operand: ExpressionHandle,
    extra: ExpressionHandle,
    first_argument: ExpressionHandle,
    second_argument: ExpressionHandle,
    argument_span: HandleSpan<ExpressionHandle>,
    equal_argument_span: HandleSpan<ExpressionHandle>,
    foreign_argument_span: HandleSpan<ExpressionHandle>,
    target: SymbolHandle,
    static_target: SymbolHandle,
}

impl JournalFixture {
    fn new(stages: Stages, settled: TypedTrees, journal: SelectedDispatchSourceEdits) -> Self {
        Self {
            original: stages.program,
            settled,
            journal,
            call: stages.call,
            unary: stages.unary,
            receiver: stages.receiver,
            operand: stages.operand,
            extra: stages.extra,
            first_argument: stages.first_argument,
            second_argument: stages.second_argument,
            argument_span: stages.argument_span,
            equal_argument_span: stages.equal_argument_span,
            foreign_argument_span: stages.foreign_argument_span,
            target: stages.target,
            static_target: stages.static_target,
        }
    }
}

fn journal_fixture() -> JournalFixture {
    let stages = stages();
    let mut builder = SourceEditBuilder::default();
    builder.expression(&stages.program, stages.call);
    builder.expression(&stages.program, stages.unary);
    let mut settled = stages.program.clone();
    *settled.expression_table.expression_mut(stages.call) = ExpressionNode::Boolean(true);
    *settled.expression_table.expression_mut(stages.unary) = ExpressionNode::Boolean(true);
    let journal = builder.finish(&settled).expect("honest journal seals");
    JournalFixture::new(stages, settled, journal)
}

/// A journal sealed from two settlement batches in order. `overlap` selects
/// whether the second batch rewrote the same call site (ordered custody) or
/// the disjoint unary site.
fn two_batch_journal(overlap: bool) -> JournalFixture {
    let stages = stages();
    let mut first = SourceEditBuilder::default();
    first.expression(&stages.program, stages.call);
    let mut intermediate = stages.program.clone();
    // The second batch can only record a supported edit shape, so the
    // intermediate settled call keeps a unary shape when it is rewritten again.
    *intermediate.expression_table.expression_mut(stages.call) = if overlap {
        ExpressionNode::Unary(TableUnaryExpression {
            operator: UnaryOperator::LogicalNot,
            operand: stages.operand,
        })
    } else {
        ExpressionNode::Boolean(true)
    };
    let first = first.finish(&intermediate).expect("first batch seals");

    let second_site = if overlap { stages.call } else { stages.unary };
    let mut second = SourceEditBuilder::default();
    second.expression(&intermediate, second_site);
    let mut settled = intermediate.clone();
    *settled.expression_table.expression_mut(second_site) = ExpressionNode::Boolean(true);
    let second = second.finish(&settled).expect("second batch seals");

    let mut journal = first;
    journal.append(second);
    JournalFixture::new(stages, settled, journal)
}

/// One batch sealed against an unrelated program. Its handles coincide with
/// fixture handles, so substitution is a real commitment collision, not a
/// stale index; its covered operand content diverges so the foreign guard can
/// never validate against the fixture program.
fn foreign_batch() -> Batch {
    let mut original = TypedTrees::default();
    let operand = original
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let site = original
        .expression_table
        .insert(ExpressionNode::Unary(TableUnaryExpression {
            operator: UnaryOperator::BitwiseNot,
            operand,
        }));
    let mut builder = SourceEditBuilder::default();
    builder.expression(&original, site);
    let mut settled = original.clone();
    *settled.expression_table.expression_mut(site) = ExpressionNode::Boolean(false);
    builder
        .finish(&settled)
        .expect("foreign journal seals")
        .batches
        .into_iter()
        .next()
        .expect("one batch")
}

fn expect_replay_rejects(journal: &SelectedDispatchSourceEdits, settled: &TypedTrees, axis: &str) {
    assert!(
        journal.source_trees(settled).is_err(),
        "{axis}: replay must reject"
    );
}

fn expect_replays_original(
    journal: &SelectedDispatchSourceEdits,
    settled: &TypedTrees,
    original: &TypedTrees,
    axis: &str,
) {
    match journal.source_trees(settled) {
        Ok(source) => assert_eq!(source.as_ref(), original, "{axis}"),
        Err(diagnostics) => {
            panic!("{axis}: replay rejected an equivalent journal: {diagnostics:?}")
        }
    }
}

fn expect_replay_diverges(
    journal: &SelectedDispatchSourceEdits,
    settled: &TypedTrees,
    original: &TypedTrees,
    axis: &str,
) -> TypedTrees {
    match journal.source_trees(settled) {
        Ok(source) => {
            assert_ne!(
                source.as_ref(),
                original,
                "{axis}: replay restored the recorded source from a substituted journal"
            );
            source.into_owned()
        }
        Err(diagnostics) => {
            panic!("{axis}: expected a divergent restore, got rejection {diagnostics:?}")
        }
    }
}

#[test]
fn source_edit_journal_replays_honestly_in_exactly_one_direction() {
    let fixture = journal_fixture();
    let source = fixture
        .journal
        .source_trees(&fixture.settled)
        .expect("honest journal replays");
    assert!(
        matches!(source, Cow::Owned(_)),
        "recorded edits force an owned restore"
    );
    assert_eq!(source.as_ref(), &fixture.original);
    assert_ne!(source.as_ref(), &fixture.settled);
    // The guard binds the settled direction only: replaying the journal
    // against the already restored source rejects instead of double-restoring.
    expect_replay_rejects(
        &fixture.journal,
        &source,
        "journal replayed against its own output",
    );
    // A foreign settled program rejects at last-batch validation before any
    // node is restored.
    let mut foreign_settled = fixture.settled.clone();
    *foreign_settled
        .expression_table
        .expression_mut(fixture.operand) = ExpressionNode::Boolean(true);
    expect_replay_rejects(
        &fixture.journal,
        &foreign_settled,
        "journal replayed against a foreign settled program",
    );
}

#[test]
fn journal_batch_roster_rejects_every_one_field_substitution() {
    let disjoint = two_batch_journal(false);
    let overlapping = two_batch_journal(true);
    expect_replays_original(
        &disjoint.journal,
        &disjoint.settled,
        &disjoint.original,
        "honest disjoint two-batch journal",
    );
    expect_replays_original(
        &overlapping.journal,
        &overlapping.settled,
        &overlapping.original,
        "honest overlapping two-batch journal",
    );

    // Omitting every batch borrows the settled program verbatim — observably
    // not the recorded source, and visibly not a rejection path.
    let mut journal = disjoint.journal.clone();
    journal.batches.clear();
    let source = journal
        .source_trees(&disjoint.settled)
        .expect("an empty journal borrows the settled program");
    assert!(matches!(source, Cow::Borrowed(_)));
    assert_eq!(source.as_ref(), &disjoint.settled);
    assert_ne!(source.as_ref(), &disjoint.original);

    // Omitting either batch restores a program that still carries that
    // batch's settled nodes.
    let mut journal = disjoint.journal.clone();
    journal.batches.remove(0);
    expect_replay_diverges(
        &journal,
        &disjoint.settled,
        &disjoint.original,
        "dropped first batch",
    );
    let mut journal = disjoint.journal.clone();
    journal.batches.pop();
    expect_replay_diverges(
        &journal,
        &disjoint.settled,
        &disjoint.original,
        "dropped last batch",
    );

    // Disjoint batches commute: settlement order between disjoint custody is
    // not committed identity.
    let mut journal = disjoint.journal.clone();
    journal.batches.swap(0, 1);
    expect_replays_original(
        &journal,
        &disjoint.settled,
        &disjoint.original,
        "reordered disjoint batches restore the same source",
    );
    // Overlapping batches are ordered through the guard: the moved last batch
    // was sealed over an earlier settled state and rejects before writes.
    let mut journal = overlapping.journal.clone();
    journal.batches.swap(0, 1);
    expect_replay_rejects(
        &journal,
        &overlapping.settled,
        "reordered overlapping batches",
    );

    // A duplicated batch passes the first validation, restores its edits, then
    // fails its own mid-restore revalidation.
    let mut journal = disjoint.journal.clone();
    journal
        .batches
        .push(journal.batches.last().expect("two batches").clone());
    expect_replay_rejects(&journal, &disjoint.settled, "duplicated batch");

    // Foreign batches reject in both directions: as the last batch before any
    // write, and as an earlier batch at mid-restore revalidation.
    let mut journal = disjoint.journal.clone();
    journal.batches.push(foreign_batch());
    expect_replay_rejects(&journal, &disjoint.settled, "appended foreign batch");
    let mut journal = disjoint.journal.clone();
    journal.batches.insert(0, foreign_batch());
    expect_replay_rejects(&journal, &disjoint.settled, "prepended foreign batch");
    let mut journal = disjoint.journal.clone();
    journal.batches = vec![foreign_batch()];
    expect_replay_rejects(&journal, &disjoint.settled, "wholesale foreign roster");

    // An edit-free batch is envelope-only: sealed over the same settled
    // program it validates and restores nothing.
    let mut journal = disjoint.journal.clone();
    journal.batches.insert(
        0,
        Batch {
            edits: Vec::new(),
            statement_edits: Vec::new(),
            guard: GraphGuard::capture(&disjoint.settled, &[], &[], &[], &[])
                .expect("empty guard seals"),
        },
    );
    expect_replays_original(
        &journal,
        &disjoint.settled,
        &disjoint.original,
        "prepended edit-free batch canonicalizes",
    );
    let mut journal = disjoint.journal.clone();
    journal.batches.push(Batch {
        edits: Vec::new(),
        statement_edits: Vec::new(),
        guard: GraphGuard::capture(&disjoint.settled, &[], &[], &[], &[])
            .expect("empty guard seals"),
    });
    expect_replays_original(
        &journal,
        &disjoint.settled,
        &disjoint.original,
        "appended edit-free batch canonicalizes",
    );
}

#[test]
fn journal_edit_roster_rejects_every_one_field_substitution() {
    let fixture = journal_fixture();

    // Omitting an edit is not detected by the journal — the roster is the
    // custody — but replay restores an observably different program: the
    // dropped site keeps its settled node.
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits.remove(0);
    let source = expect_replay_diverges(
        &journal,
        &fixture.settled,
        &fixture.original,
        "dropped call edit",
    );
    assert_eq!(
        source.expression_table.expression(fixture.call),
        &ExpressionNode::Boolean(true),
        "the dropped edit's settled node survives"
    );
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits.pop();
    let source = expect_replay_diverges(
        &journal,
        &fixture.settled,
        &fixture.original,
        "dropped unary edit",
    );
    assert_eq!(
        source.expression_table.expression(fixture.unary),
        &ExpressionNode::Boolean(true),
        "the dropped edit's settled node survives"
    );

    // A duplicated edit rewrites the same site with the same original: an
    // idempotent envelope entry that canonicalizes.
    let mut journal = fixture.journal.clone();
    let edit = journal.batches[0].edits[0].clone();
    journal.batches[0].edits.push(edit);
    expect_replays_original(
        &journal,
        &fixture.settled,
        &fixture.original,
        "duplicated edit",
    );

    // Disjoint edit order is not committed: rewrites at different sites
    // commute.
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits.swap(0, 1);
    expect_replays_original(
        &journal,
        &fixture.settled,
        &fixture.original,
        "reordered disjoint edits",
    );

    // A foreign edit carries a foreign original onto a coinciding handle.
    let foreign = foreign_batch();
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits.push(foreign.edits[0].clone());
    let source = expect_replay_diverges(
        &journal,
        &fixture.settled,
        &fixture.original,
        "foreign appended edit",
    );
    assert!(
        matches!(
            source.expression_table.expression(fixture.first_argument),
            ExpressionNode::Unary(_)
        ),
        "the foreign edit's original lands on the coinciding handle"
    );
}

#[test]
fn journal_edit_fields_reject_every_one_field_substitution() {
    let fixture = journal_fixture();

    // A substituted handle redirects the restore write and strands the settled
    // node on the honest site.
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits[0].handle = fixture.extra;
    let source = expect_replay_diverges(
        &journal,
        &fixture.settled,
        &fixture.original,
        "foreign edit handle",
    );
    assert!(
        matches!(
            source.expression_table.expression(fixture.extra),
            ExpressionNode::Call(_)
        ),
        "the recorded original lands on the substituted handle"
    );
    assert_eq!(
        source.expression_table.expression(fixture.call),
        &ExpressionNode::Boolean(true),
        "the honest site keeps its settled node"
    );

    // An invalid handle is absorbed by arena dummy storage: no panic, no
    // honest-site restore.
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits[0].handle = ExpressionHandle::invalid();
    let source = expect_replay_diverges(
        &journal,
        &fixture.settled,
        &fixture.original,
        "invalid edit handle",
    );
    assert_eq!(
        source.expression_table.expression(fixture.call),
        &ExpressionNode::Boolean(true),
        "the honest site keeps its settled node"
    );

    // A substituted original restores substituted content.
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits[0].original = ExpressionNode::Boolean(false);
    let source = expect_replay_diverges(
        &journal,
        &fixture.settled,
        &fixture.original,
        "foreign original node",
    );
    assert_eq!(
        source.expression_table.expression(fixture.call),
        &ExpressionNode::Boolean(false),
        "the substituted original is what replay restores"
    );

    // `original_call` is a consistency envelope over the settled argument
    // span, not restored content: dropping the check canonicalizes while
    // forged contents reject.
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits[0].original_call = None;
    expect_replays_original(
        &journal,
        &fixture.settled,
        &fixture.original,
        "dropped original_call envelope",
    );
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits[1].original_call = Some(ExpressionArguments {
        span: fixture.argument_span,
        arguments: vec![fixture.first_argument, fixture.second_argument],
    });
    expect_replays_original(
        &journal,
        &fixture.settled,
        &fixture.original,
        "an added original_call envelope that still holds",
    );
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits[1].original_call = Some(ExpressionArguments {
        span: fixture.argument_span,
        arguments: vec![fixture.second_argument],
    });
    expect_replay_rejects(
        &journal,
        &fixture.settled,
        "forged original_call on a non-call edit",
    );
}

#[test]
fn journal_argument_span_fields_reject_every_one_field_substitution() {
    let fixture = journal_fixture();

    // The recorded span is bound through its contents, not its identity: a
    // foreign span holding the same argument list canonicalizes while any
    // content difference rejects.
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits[0]
        .original_call
        .as_mut()
        .expect("call edit carries argument custody")
        .span = fixture.equal_argument_span;
    expect_replays_original(
        &journal,
        &fixture.settled,
        &fixture.original,
        "equal-content span substitution",
    );
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits[0]
        .original_call
        .as_mut()
        .expect("call edit carries argument custody")
        .span = fixture.foreign_argument_span;
    expect_replay_rejects(&journal, &fixture.settled, "foreign-content span");
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits[0]
        .original_call
        .as_mut()
        .expect("call edit carries argument custody")
        .span = HandleSpan::empty();
    expect_replay_rejects(&journal, &fixture.settled, "empty span");
    let mut journal = fixture.journal.clone();
    journal.batches[0].edits[0]
        .original_call
        .as_mut()
        .expect("call edit carries argument custody")
        .span = HandleSpan::from_parts(Handle::from_parts(65_536, 0), 2);
    expect_replay_rejects(&journal, &fixture.settled, "out-of-range span");

    // The recorded argument list is order-sensitive custody: drop, duplicate,
    // reorder, and foreign elements all reject against the live span.
    for (axis, arguments) in [
        ("dropped element", vec![fixture.first_argument]),
        ("empty list", Vec::new()),
        (
            "duplicated element",
            vec![
                fixture.first_argument,
                fixture.first_argument,
                fixture.second_argument,
            ],
        ),
        (
            "reordered elements",
            vec![fixture.second_argument, fixture.first_argument],
        ),
        (
            "foreign element",
            vec![fixture.first_argument, fixture.extra],
        ),
    ] {
        let mut journal = fixture.journal.clone();
        journal.batches[0].edits[0]
            .original_call
            .as_mut()
            .expect("call edit carries argument custody")
            .arguments = arguments;
        expect_replay_rejects(&journal, &fixture.settled, axis);
    }
}

#[test]
fn journal_sealed_guard_rejects_every_substitution() {
    let fixture = journal_fixture();

    // A foreign guard cannot replay the settled program.
    let mut journal = fixture.journal.clone();
    journal.batches[0].guard = foreign_batch().guard;
    expect_replay_rejects(&journal, &fixture.settled, "foreign sealed guard");

    // A guard sealed over the pre-dispatch program is the wrong direction:
    // custody binds the settled state, not the source it restores.
    let mut journal = fixture.journal.clone();
    journal.batches[0].guard = GraphGuard::capture(
        &fixture.original,
        &[
            fixture.receiver,
            fixture.first_argument,
            fixture.second_argument,
            fixture.call,
            fixture.operand,
            fixture.unary,
        ],
        &[],
        &[fixture.target],
        &[static_argument(fixture.static_target)],
    )
    .expect("wrong-direction guard seals");
    expect_replay_rejects(
        &journal,
        &fixture.settled,
        "guard sealed over the pre-dispatch program",
    );

    // A guard sealed over a foreign settled program cannot stand in either.
    let mut foreign_settled = fixture.settled.clone();
    *foreign_settled
        .expression_table
        .expression_mut(fixture.operand) = ExpressionNode::Boolean(true);
    let mut journal = fixture.journal.clone();
    journal.batches[0].guard = GraphGuard::capture(
        &foreign_settled,
        &[
            fixture.receiver,
            fixture.first_argument,
            fixture.second_argument,
            fixture.call,
            fixture.operand,
            fixture.unary,
        ],
        &[],
        &[fixture.target],
        &[static_argument(fixture.static_target)],
    )
    .expect("foreign-state guard seals");
    expect_replay_rejects(&journal, &fixture.settled, "foreign-state guard");
}
