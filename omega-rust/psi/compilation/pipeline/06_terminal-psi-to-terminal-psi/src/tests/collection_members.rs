//! Member meaning is selected once from the actual receiver type. A field
//! called `len` remains that field; an unknown receiver gains no builtin.

use super::boundary_calls::typed;
use typed_trees::expression::{CollectionMeasure, ExpressionNode, MemberSelection};

fn members(text: &str) -> Vec<MemberSelection> {
    typed(text)
        .expression_table
        .expression_entries()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Member(member) => Some(member.selection),
            _ => None,
        })
        .collect()
}

#[test]
fn collection_length_is_selected_on_the_real_borrowed_receiver() {
    assert_eq!(
        members("machine observe(items: &[u8]) -> u64 { items.len }"),
        vec![MemberSelection::CollectionMeasure(
            CollectionMeasure::Length
        )]
    );
}

#[test]
fn a_same_spelled_record_field_keeps_its_declaration() {
    let program =
        typed("data Counter { len: u64 } machine observe(value: Counter) -> u64 { value.len }");
    let field = program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_member_fields(program.data_members(data)))
        .find(|field| field.name.as_str() == "len")
        .unwrap()
        .symbol;
    assert!(field.is_valid());
    let selections: Vec<_> = program
        .expression_table
        .expression_entries()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Member(member) => Some(member.selection),
            _ => None,
        })
        .collect();
    assert_eq!(selections, vec![MemberSelection::Field(field)]);
}

#[test]
fn a_noncollection_cannot_acquire_length_from_its_spelling() {
    assert_eq!(
        members("machine observe(value: u64) -> u64 { value.len }"),
        vec![MemberSelection::Unresolved]
    );
}

fn judged(text: &str) -> super::RouteVerdict {
    super::judge(elaborated(text)).unwrap()
}

fn judged_invocation(text: &str) -> super::RouteVerdict {
    let program = typed(text);
    let entry = super::boundary_calls::entry(&program);
    super::judge(typed_trees_to_terminal_psi::elaborate_for(
        &program,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    ))
    .unwrap()
}

fn elaborated(
    text: &str,
) -> terminal_check_tables::Checked<terminal_check_tables::program::Elaboration> {
    let program = typed(text);
    let entry = super::boundary_calls::entry(&program);
    typed_trees_to_terminal_psi::elaborate_closure(&program, entry)
}

#[test]
fn a_length_read_uses_the_exact_formals_entry_extent() {
    for view in ["&[u8]", "&mut [u8]"] {
        let result = judged(&format!(
            "machine entry(items: {view}) -> u64 requires items.len > 0 {{ items.len - 1 }}"
        ));
        let super::RouteVerdict::Accepted(accepted) = result else {
            panic!("{view}: {result:?}")
        };
        let machine = &accepted.elaboration.module.machines[0];
        assert!(machine.blocks.iter().flat_map(|block| &block.operations).any(|operation| {
            matches!(&operation.kind, terminal_psi::OperationKind::ByteSequenceLength { source }
                if source.place == machine.structural_parameters[0].place && source.path.is_empty())
        }));
    }
}

#[test]
fn another_views_extent_cannot_justify_this_length() {
    let result = judged(
        "machine entry(left: &[u8], right: &[u8]) -> u64 requires left.len > 0 { right.len - 1 }",
    );
    assert!(
        matches!(result, super::RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, super::ObligationKind::OperationSafety { .. }))),
        "{result:?}"
    );
}

#[test]
fn a_copied_view_keeps_its_formals_extent_without_an_extra_source_read() {
    let result = judged(
        r#"
machine entry<'a>(items: &'a [u8]) -> u64 requires items.len > 0 {
    let local: &'a [u8] = items;
    local.len - 1
}
"#,
    );
    let super::RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    let machine = &accepted.elaboration.module.machines[0];
    let reads: Vec<_> = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::ByteSequenceLength { source } => Some(source),
            _ => None,
        })
        .collect();
    assert_eq!(reads.len(), 1);
    assert_ne!(reads[0].place, machine.structural_parameters[0].place);
}

#[test]
fn a_call_binds_length_to_its_actual_descriptor_version() {
    for (parameters, requirement, body) in [
        ("items: &'a [u8]", "items.len > 0", "required(items)"),
        (
            "items: &'a [u8]",
            "items.len > 0",
            "let local: &'a [u8] = items; required(local)",
        ),
        (
            "holder: &Holder<'a>",
            "holder.left.len > 0",
            "required(holder.left)",
        ),
    ] {
        let source = format!(
            r#"
data Holder<'held> {{ left: &'held [u8]; right: &'held [u8]; }}
machine required<'a>(items: &'a [u8]) -> u64 requires items.len > 0 {{ items.len - 1 }}
machine entry<'a>({parameters}) -> u64 requires {requirement} {{ {body} }}
"#
        );
        let result = judged(&source);
        assert!(
            matches!(result, super::RouteVerdict::Accepted(_)),
            "{body}: {result:?}"
        );
    }
}

#[test]
fn a_call_cannot_use_another_fields_length() {
    let result = judged(
        r#"
data Holder<'held> { left: &'held [u8]; right: &'held [u8]; }
machine required<'a>(items: &'a [u8]) -> u64 requires items.len > 0 { items.len - 1 }
machine entry<'a>(holder: &Holder<'a>) -> u64 requires holder.left.len > 0 {
    required(holder.right)
}
"#,
    );
    assert!(
        matches!(result, super::RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, super::ObligationKind::CallRequirement { .. }))),
        "{result:?}"
    );
}

#[test]
fn a_call_cannot_reuse_length_from_before_an_overlapping_mutable_call() {
    let result = judged(
        r#"
data Holder<'held> { bytes: &'held [u8]; }
machine replace<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) { holder.bytes = replacement; }
machine required<'a>(items: &'a [u8]) -> u64 requires items.len > 0 { items.len - 1 }
machine entry<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) -> u64
requires holder.bytes.len > 0 {
    let before: u64 = holder.bytes.len;
    replace(holder, replacement);
    required(holder.bytes)
}
"#,
    );
    assert!(
        matches!(result, super::RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, super::ObligationKind::CallRequirement { .. }))),
        "{result:?}"
    );
}

#[test]
fn a_later_argument_preserves_an_explicit_descriptor_copy() {
    // Replacing the source slot does not replace the descriptor already
    // captured in a distinct local before the call's arguments execute.
    let result = judged(
        r#"
data Holder<'held> { bytes: &'held [u8]; }
machine replace<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) -> u64 {
    holder.bytes = replacement;
    0
}
machine required<'a>(items: &'a [u8], ignored: u64) -> u64
requires items.len > 0 { items.len - 1 }
machine entry<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) -> u64
requires holder.bytes.len > 0 {
    let captured: &'a [u8] = holder.bytes;
    required(captured, replace(holder, replacement))
}
"#,
    );
    assert!(
        matches!(result, super::RouteVerdict::Accepted(_)),
        "{result:?}"
    );
}

#[test]
fn a_direct_view_argument_keeps_its_length_across_later_argument_effects() {
    let result = judged(
        r#"
data Holder<'held> { bytes: &'held [u8]; }
machine replace<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) -> u64 {
    holder.bytes = replacement;
    0
}
machine required<'a>(items: &'a [u8], ignored: u64) -> u64
requires items.len > 0 { items.len - 1 }
machine entry<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) -> u64
requires holder.bytes.len > 0 {
    required(holder.bytes, replace(holder, replacement))
}
"#,
    );
    assert!(
        matches!(result, super::RouteVerdict::Accepted(_)),
        "{result:?}"
    );
}

#[test]
fn a_direct_view_argument_executes_with_its_captured_descriptor() {
    let result = judged_invocation(
        r#"
data Holder<'held> { bytes: &'held [u8]; }
machine replace<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) -> u64 {
    holder.bytes = replacement;
    0
}
machine length(items: &[u8], ignored: u64) -> u64 { items.len }
machine entry<'a>() -> u64 {
    let original: &'a [u8] = "long";
    let replacement: &'a [u8] = "x";
    let mut holder: Holder<'a> = Holder { bytes: original };
    length(holder.bytes, replace(&mut holder, replacement))
}
"#,
    );
    let super::RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut psi_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(4),
                        ..
                    }
                )
            )
        ),
        "{status:?}"
    );
}

#[test]
fn a_returned_view_keeps_its_incoming_backing_after_argument_home_retirement() {
    let result = judged_invocation(
        r#"
data Holder<'held> { bytes: &'held [u8]; }
machine echo<'a>(items: &'a [u8]) -> &'a [u8] { items }
machine entry<'a>(holder: &Holder<'a>) -> &'a [u8] { echo(holder.bytes) }
"#,
    );
    let super::RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    accepted.publish_invocation().unwrap();
}

#[test]
fn a_returned_view_executes_after_its_argument_home_and_source_slot_end() {
    let result = judged_invocation(
        r#"
data Holder<'held> { bytes: &'held [u8]; }
machine echo<'a>(items: &'a [u8]) -> &'a [u8] { items }
machine replace<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) { holder.bytes = replacement; }
machine entry<'a>() -> u64 {
    let original: &'a [u8] = "long";
    let replacement: &'a [u8] = "x";
    let mut holder: Holder<'a> = Holder { bytes: original };
    let returned: &'a [u8] = echo(holder.bytes);
    replace(&mut holder, replacement);
    returned.len
}
"#,
    );
    let super::RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut psi_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(4),
                        ..
                    }
                )
            )
        ),
        "{status:?}"
    );
}

#[test]
fn a_staged_view_keeps_its_loan_through_a_crashing_transfer() {
    let result = judged_invocation(
        r#"
machine fail(items: &[u8]) crashes Trap { crash Trap; }
machine entry() crashes Trap { fail("x"); }
"#,
    );
    let super::RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut psi_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(status, terminal_interpreter::TerminalExecutionStatus::Crashed(ref crash)
        if crash.cause == semantic_vocabulary::CrashCause::Trap),
        "{status:?}"
    );
}

#[test]
fn replacing_a_stored_descriptor_cannot_reuse_its_old_extent() {
    let result = judged(
        r#"
data Holder<'held> { bytes: &'held [u8]; }
machine entry<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) -> u64 requires holder.bytes.len > 0 {
    holder.bytes = replacement;
    holder.bytes.len - 1
}
"#,
    );
    assert!(
        matches!(result, super::RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, super::ObligationKind::OperationSafety { .. }))),
        "{result:?}"
    );
}

#[test]
fn measuring_a_call_result_keeps_the_bases_real_call_requirement() {
    let result = judged(
        r#"
machine produce<'a>(items: &'a [u8]) -> &'a [u8] requires 1u64 == 0u64 { items }
machine entry(items: &[u8]) -> u64 { produce(items).len }
"#,
    );
    assert!(
        matches!(result, super::RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, super::ObligationKind::CallRequirement { .. }))),
        "{result:?}"
    );
}

#[test]
fn a_mutable_call_keeps_a_saved_scalar_but_invalidates_a_fresh_extent() {
    let source = |returned| {
        format!(
            r#"
data Holder<'held> {{ bytes: &'held [u8]; }}
machine replace<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) {{ holder.bytes = replacement; }}
machine entry<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) -> u64
requires holder.bytes.len > 0 {{
    let before: u64 = holder.bytes.len;
    replace(holder, replacement);
    {returned}
}}
"#
        )
    };
    let saved = judged(&source("before - 1"));
    assert!(
        matches!(saved, super::RouteVerdict::Accepted(_)),
        "{saved:?}"
    );
    let fresh = judged(&source("holder.bytes.len - 1"));
    assert!(
        matches!(fresh, super::RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, super::ObligationKind::OperationSafety { .. }))),
        "{fresh:?}"
    );
}

/// Source storage cycles are currently unserved. Exercise the actual
/// storage owner on a canonical backedge instead: keep the source-derived
/// operations, parameter places, values and write rows, and route a latch
/// call back to the fresh read. This asserts no source-loop acceptance.
#[test]
fn an_overlapping_mutable_call_on_a_backedge_invalidates_the_next_read() {
    use semantic_vocabulary::BlockId;
    use terminal_check_tables::edges::{Destination, EdgeRow};
    use terminal_psi::{Block, OperationKind, Terminator};

    for mutable in [false, true] {
        let access = if mutable { "&mut" } else { "&" };
        let body = if mutable {
            "holder.bytes = replacement;"
        } else {
            ""
        };
        let source = format!(
            r#"
data Holder<'held> {{ bytes: &'held [u8]; }}
machine touch<'a>(holder: {access} Holder<'a>, replacement: &'a [u8]) {{ {body} }}
machine entry<'a>(holder: &mut Holder<'a>, replacement: &'a [u8]) -> u64 {{
    let before: u64 = holder.bytes.len;
    touch(holder, replacement);
    holder.bytes.len
}}
"#
        );
        let elaboration = elaborated(&source).unwrap();
        let mut candidates = elaboration.module.machines.iter().filter(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| {
                    matches!(operation.kind, OperationKind::ByteSequenceLength { .. })
                })
                .count()
                == 2
        });
        let mut lowered = candidates.next().unwrap().clone();
        assert!(candidates.next().is_none());
        let mut tables = elaboration.tables.machines.get(lowered.id).unwrap().clone();
        let start = lowered.entry;
        let latch: BlockId = super::id(
            lowered
                .blocks
                .iter()
                .map(|block| block.id.get())
                .max()
                .unwrap()
                + 1,
        );
        let entry_position = lowered
            .blocks
            .iter()
            .position(|block| block.id == start)
            .unwrap();
        let mut entry = lowered.blocks.remove(entry_position);
        let initial_read = entry
            .operations
            .iter()
            .position(|operation| {
                matches!(operation.kind, OperationKind::ByteSequenceLength { .. })
            })
            .unwrap();
        let staged_call = entry.operations.split_off(initial_read + 1);
        assert_eq!(
            staged_call
                .iter()
                .filter(|operation| operation.kind.declared_invocation().is_some())
                .count(),
            1
        );
        let header_position = lowered
            .blocks
            .iter()
            .position(|block| {
                block.operations.iter().any(|operation| {
                    matches!(operation.kind, OperationKind::ByteSequenceLength { .. })
                })
            })
            .unwrap();
        let head = lowered.blocks[header_position].id;
        // Move staging and the call to the latch, keeping their actual
        // retirement jump, cleanup actions and edge identity. Each loop
        // iteration then creates and retires its own descriptor temporary.
        let retirement = entry.terminator.clone();
        let Terminator::Jump { edge, .. } = &retirement else {
            panic!("staged call has no retirement continuation")
        };
        tables.edges.get_mut(*edge).unwrap().source = latch;
        let mut jump = |from, target| {
            let edge = tables.edges.push(EdgeRow {
                source: from,
                destination: Destination::Block(target),
                dispositions: Vec::new(),
            });
            Terminator::Jump {
                edge,
                target,
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            }
        };
        entry.terminator = jump(start, head);
        lowered.blocks[header_position].terminator = jump(head, latch);
        let mut feedback = Block::plain(latch, Vec::new(), retirement);
        feedback.operations = staged_call;
        lowered.blocks.insert(0, entry);
        lowered.blocks.push(feedback);
        let graph = crate::graph::Graph::new(lowered.id, &lowered, &tables).unwrap();
        let types = tables.values.map(|_, row| row.scalar_type);
        let storage = crate::places::Storage::new(
            lowered.id,
            &lowered,
            &elaboration.module.structural_types,
            &elaboration.domains,
            &graph,
            &tables,
            &types,
        )
        .unwrap();
        let first = lowered.blocks[0]
            .operations
            .iter()
            .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceLength { .. }))
            .unwrap();
        let fresh = lowered
            .blocks
            .iter()
            .find(|block| block.id == head)
            .unwrap()
            .operations
            .iter()
            .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceLength { .. }))
            .unwrap();
        assert!(storage.read_equation(&tables, first).unwrap().is_some());
        assert_eq!(
            storage.read_equation(&tables, fresh).unwrap().is_some(),
            !mutable
        );
    }
}
