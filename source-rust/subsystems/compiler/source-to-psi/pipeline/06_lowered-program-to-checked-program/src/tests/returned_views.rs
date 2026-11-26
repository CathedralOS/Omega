//! A view a boundary returned, handed on by the machine that called it.
//! Read from source, as the Build prelude's `RequiredOutput::path` is: a
//! one-line machine around `BuildFilesystemOperations::required_path`
//! (`samples/cli/facts/required_output_path`).
//!
//! A boundary requirement has no body and may return provider-owned bytes
//! outside every input object (foreign_storage.md, "Provider-owned views and
//! completion"). Its declared lifetime groups still name the input loans
//! that must stay live. A forwarding machine carries that provider provenance
//! and those logical dependencies without inventing spatial containment.

use super::*;

fn elaborate(text: &str) -> Checked<crate::lowering::Elaboration> {
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
            bindings: &[],
            supplies: &[],
            calling: None,
            providers: None,
        },
        terminal_codec::TerminalPublication::Invocation,
    )
}

/// What the checker says of the Product `text`, judged from its `entry`:
/// `accepted`, `rejected` with its conflicts, or the refusal's text.
fn verdict(text: &str) -> String {
    match crate::judge(elaborate(text)) {
        Ok(RouteVerdict::Accepted(_)) => "accepted".to_owned(),
        Ok(RouteVerdict::FallBack(refusal)) | Err(refusal) => refusal.detail,
        Ok(RouteVerdict::Rejected { conflicts, .. }) => {
            format!("rejected: {} conflicts", conflicts.len())
        }
        Ok(other) => panic!("{other:?}"),
    }
}

#[test]
fn returning_a_child_keeps_its_earlier_parent_disposal() {
    let source = r#"
data Tag [copy] { case A; case B; }
data Cell [copy] { value: u64; }
data Boxes [copy] { cells: [Cell; 2]; }
machine first(boxes: @mutable Boxes, tag: Tag, branch: bool) -> @mutable Cell {
    let cells: @mutable [Cell] = @mutable boxes.cells[..];
    transition branch {
        true -> selected(cells, tag),
        false -> @mutable cells[0]
    }
    machine selected(items: @mutable [Cell], tag: Tag) -> @mutable Cell
    requires items.length == 2 { @mutable items[0] }
}
machine entry() -> u64 {
    let mutable boxes: Boxes;
    let cell: @mutable Cell = first(@mutable boxes, Tag::A, true);
    cell.value = 7u64;
    boxes.cells[0].value
}
"#;
    assert_eq!(verdict(source), "accepted");
    let write_while_lent = source.replace(
        "    cell.value = 7u64;",
        "    boxes.cells[0].value = 3u64;\n    cell.value = 7u64;",
    );
    assert!(matches!(
        crate::judge(elaborate(&write_while_lent)),
        Ok(RouteVerdict::Rejected { conflicts, .. }) if !conflicts.is_empty()
    ));

    let mut incomplete = elaborate(source).unwrap();
    let removed = incomplete
        .module
        .machines
        .iter_mut()
        .filter(|machine| machine.result.structural().is_some())
        .flat_map(|machine| &mut machine.blocks)
        .filter(|block| {
            block
                .operations
                .iter()
                .any(|op| op.declared_invocation().is_some())
        })
        .find_map(|block| {
            let terminal_psi::Terminator::Jump {
                cleanup_actions, ..
            } = &mut block.terminator
            else {
                return None;
            };
            let parent = cleanup_actions.iter().position(|action| {
                matches!(
                    action,
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(_)
                )
            })?;
            Some(cleanup_actions.remove(parent))
        });
    assert!(
        removed.is_some(),
        "the helper return disposes its parent on an earlier edge"
    );
    let rejected = crate::judge(Ok(incomplete));
    let refusal = match rejected {
        Err(refusal) | Ok(RouteVerdict::FallBack(refusal)) => refusal,
        other => panic!("missing parent cleanup reached another verdict: {other:?}"),
    };
    assert!(
        refusal
            .detail
            .contains("normal edge affine disposal differs from its canonical schedule"),
        "{refusal:?}"
    );
}

#[test]
fn a_local_collection_window_keeps_its_parent_across_observations() {
    let source = r#"
data Cell [copy] { number: u64; }
data Room [copy] { cells: [Cell; 2]; }
machine count(view: @[Cell]) -> u64 { view.length }
machine entry() -> u64 {
    let mutable room: Room;
    let whole: @[Cell] = @room.cells[..];
    let window: @[Cell] = @whole[1u64..2u64];
    window.length
}
"#;
    assert_eq!(verdict(source), "accepted");
    assert_eq!(
        verdict(&source.replace("window.length", "count(window)")),
        "accepted"
    );
    let conflicting = source.replace(
        "    window.length",
        "    room.cells[1u64].number = 5u64;\n    window[0u64].number",
    );
    let result = crate::judge(elaborate(&conflicting));
    assert!(
        matches!(&result, Ok(RouteVerdict::Rejected { conflicts, .. }) if !conflicts.is_empty()),
        "{result:?}"
    );
}

#[test]
fn a_boundary_normal_guarantee_observes_its_returned_view() {
    let source = r#"
data Token { id: u64; }
boundary trait Fs {
    machine path(token: @Token) -> @[u8]
    ensures result.length > 0u64
    reaches none crashes none;
}
machine needs_bytes(bytes: @[u8]) requires bytes.length > 0u64 {}
machine consume(token: @Token) reaches Fs invokes Fs {
    let view: @[u8] = Fs::path(token);
    needs_bytes(view);
}
machine entry() reaches Fs invokes Fs {
    let token: Token = Token { id: 1 };
    consume(@token);
}
"#;
    assert_eq!(verdict(source), "accepted");
    let unpromised = source.replace("ensures result.length > 0u64", "");
    let result = crate::judge(elaborate(&unpromised)).unwrap();
    assert!(
        matches!(&result, RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{result:?}"
    );
}

#[test]
fn a_saved_boundary_result_observation_requires_its_declared_shape() {
    let source = r#"
data Packet { count: u64; }
boundary trait Reader {
    machine issue(count: u64) -> Packet
    requires count == 2u64 ensures result.count == 2u64 reaches none crashes none;
}
machine entry() -> u64 reaches Reader invokes Reader {
    let packet: Packet = Reader::issue(2u64);
    packet.count
}
"#;
    let prepared = super::calling_policies::prepared(
        super::boundary_calls::typed(source),
        &["entry"],
        terminal_codec::TerminalPublication::Portable,
    );
    let verdicts = crate::judge_compilation(prepared);
    let [Ok(RouteVerdict::Accepted(accepted))] = verdicts.as_slice() else {
        panic!("{verdicts:?}");
    };
    let bytes = terminal_codec::encode_module(&accepted.elaboration.module).unwrap();
    let mut premature = terminal_codec::decode_module(&bytes).unwrap();
    let boundary = premature
        .interfaces
        .boundary_machines
        .iter_mut()
        .find(|boundary| !boundary.scalar_ensures.is_empty())
        .unwrap();
    boundary.scalar_requires[0] = boundary.scalar_ensures[0].clone();
    let forged = terminal_codec::encode_module(&premature).unwrap();
    let refused = terminal_codec::decode_module(&forged).unwrap_err();
    assert!(
        matches!(
            refused,
            terminal_codec::CodecError::MalformedStructuralFoundation(
                "boundary scalar requires names an undeclared formal"
                    | "scalar field predicate has no readable declared root"
            )
        ),
        "{refused:?}"
    );

    let mut module = terminal_codec::decode_module(&bytes).unwrap();
    let boundary = module
        .interfaces
        .boundary_machines
        .iter_mut()
        .find(|boundary| !boundary.scalar_ensures.is_empty())
        .unwrap();
    let result = boundary.structural_contract_result().unwrap().0;
    let Proposition::Equal(left, right) = &mut boundary.scalar_ensures[0] else {
        panic!("the published record field guarantee must be an equality");
    };
    let observed = [left, right]
        .into_iter()
        .find(|term| matches!(term, ScalarTerm::IntegerField { root, .. } if *root == result))
        .unwrap();
    let ScalarType::Integer(carrier) = observed.scalar_type() else {
        unreachable!()
    };
    *observed = ScalarTerm::ViewExtent {
        root: result,
        path: Vec::new(),
        scalar_type: carrier,
    };
    let forged = terminal_codec::encode_module(&module).unwrap();
    assert!(matches!(
        terminal_codec::decode_module(&forged),
        Err(terminal_codec::CodecError::MalformedStructuralFoundation(
            "scalar field predicate disagrees with its declaration path or scalar carrier"
        ))
    ));
}

#[test]
fn a_guarded_read_of_a_provider_view_keeps_its_input_alive() {
    let source = r#"
data Token { bytes: [u8; 2]; }
boundary trait Fs {
    machine path(token: @Token) -> @[u8] reaches none crashes none;
}
machine read(value: @u8) -> u8 { $value }
machine entry() -> u8 reaches Fs invokes Fs {
    let mutable token: Token = Token { bytes: [47u8, 61u8] };
    let view: @[u8] = Fs::path(@token);
    match view.length > 0u64 { true -> read(@view[0]), false -> 0u8 }
}
"#;
    assert_eq!(verdict(source), "accepted");
    let changed = source.replace(
        "    match view.length",
        "    token.bytes[0] = 5u8;\n    match view.length",
    );
    let result = crate::judge(elaborate(&changed)).unwrap();
    assert!(
        matches!(&result, RouteVerdict::Rejected { conflicts, .. }
            if conflicts.iter().any(|row| row.conflict == crate::LoanConflict::ChangeWhileBorrowed)),
        "{result:?}"
    );
}

#[test]
fn a_boundary_can_lend_a_provider_owned_integer_for_an_immediate_read() {
    let source = r#"
boundary trait Errno { machine slot() -> @i32 reaches none crashes none; }
machine read(slot: @i32) -> i32 { $slot }
machine entry() -> i32 reaches Errno invokes Errno { read(Errno::slot()) }
"#;
    assert_eq!(verdict(source), "accepted");
}

#[test]
fn a_provider_reference_keeps_its_declared_input_borrow_until_the_read() {
    let source = r#"
data Token { id: u64; }
boundary trait ErrorState { machine slot(token: @Token) -> @i32 reaches none crashes none; }
machine read(other: @mutable Token, slot: @i32) -> i32 { other.id = 4; $slot }
machine entry() -> i32 reaches ErrorState invokes ErrorState {
    let mutable token: Token = Token { id: 1 };
    let mutable other: Token = Token { id: 2 };
    let slot: @i32 = ErrorState::slot(@token);
    read(@mutable other, slot)
}
"#;
    assert_eq!(verdict(source), "accepted");
    let conflicts = match crate::judge(elaborate(
        &source.replace("read(@mutable other, slot)", "read(@mutable token, slot)"),
    )) {
        Ok(RouteVerdict::Rejected { conflicts, .. }) => conflicts,
        other => panic!("the provider reference must keep token borrowed: {other:?}"),
    };
    assert!(conflicts.iter().any(|conflict| matches!(
        conflict.conflict,
        crate::LoanConflict::ExclusiveAndShared
            | crate::LoanConflict::ChangeWhileBorrowed
    )));
}

const RETURNED_VIEW: &str = r#"
data Token { id: u64; }
boundary trait Fs { machine required_path(output: @Token) -> @[u8] reaches none crashes none; }
machine path(output: @Token) -> @[u8] reaches Fs invokes Fs { Fs::required_path(output) }
machine see(bytes: @[u8]) { }
machine entry() reaches Fs invokes Fs {
    let name: [u8; 4] = [1, 2, 3, 4];
    let token: Token = Token { id: 1 };
    see(@name[..]);
    see(path(@token));
}
"#;

/// The local array view demands reference flow for every callee, including
/// the helper that hands on the boundary's returned view.
#[test]
fn a_view_a_boundary_returned_is_handed_on_by_a_machine() {
    assert_eq!(verdict(RETURNED_VIEW), "accepted");
}

#[test]
fn a_parked_provider_result_requires_its_original_lender_and_producing_call() {
    let source = r"
data Token { id: u64; }
boundary trait Fs { machine path(source: @Token) -> @[u8] reaches none crashes none; }
machine helper(source: @Token) -> @[u8] reaches Fs invokes Fs { Fs::path(source) }
machine see(bytes: @[u8]) {}
machine keep(token: @Token) {}
machine pause() suspends {}
machine entry() reaches Fs invokes Fs suspends {
    let source: Token = Token { id: 1 };
    let bytes: @[u8] = helper(@source);
    suspend pause();
    see(bytes);
    keep(@source);
}";
    let RouteVerdict::Accepted(checked) = judge(elaborate(source)).unwrap() else {
        panic!("the provider result and its lender survive the ordinary suspension");
    };
    let publication = checked.publish_invocation().unwrap();
    let m = publication.artifact.view().module();
    let entry = &m.machines[0];
    let stop = entry
        .operations()
        .find(|op| op.suspension_crossing.is_some())
        .unwrap();
    let cut = (entry.id, stop.id);
    terminal_semantics::continuation::CheckedPrefix::check(m, cut, m, cut)
        .expect("the returned view has its original provider and live input");
    let mut missing = m.clone();
    let plan = missing
        .execution
        .suspension_call_plans
        .iter_mut()
        .find(|plan| Some(plan.crossing) == stop.suspension_crossing)
        .unwrap();
    let before = plan.live_values.len();
    plan.live_values
        .retain(|row| row.loans != terminal_psi::TerminalSuspensionValueLoans::ViewFree);
    assert!(plan.live_values.len() < before);
    plan.live_value_count = plan.live_values.len() as u32;
    assert!(
        terminal_semantics::continuation::CheckedPrefix::check(&missing, cut, &missing, cut)
            .is_err()
    );
}

#[test]
fn a_provider_view_keeps_its_input_loan_without_borrowing_independent_storage() {
    let source = r#"
data Token { id: u64; }
boundary trait Fs { machine path(source: @Token) -> @[u8] reaches none crashes none; }
machine helper(source: @Token) -> @[u8] reaches Fs invokes Fs { Fs::path(source) }
machine see(bytes: @[u8]) {}
machine use_path(other: @mutable Token, bytes: @[u8]) { see(bytes); other.id = 4; }
machine entry() reaches Fs invokes Fs {
    let mutable source: Token = Token { id: 1 };
    let mutable other: Token = Token { id: 2 };
    let bytes: @[u8] = helper(@source);
    use_path(@mutable other, bytes);
    source.id = 9;
}
"#;
    assert_eq!(verdict(source), "accepted");
    let invalid = source.replace(
        "use_path(@mutable other, bytes)",
        "use_path(@mutable source, bytes)",
    );
    assert_ne!(source, invalid);
    let rejected = crate::judge(elaborate(&invalid));
    let Ok(RouteVerdict::Rejected { conflicts, .. }) = rejected else {
        panic!("the Token input must remain borrowed through use_path: {rejected:?}");
    };
    assert!(
        conflicts.iter().any(|conflict| matches!(
            conflict.conflict,
            crate::LoanConflict::ExclusiveAndShared
                | crate::LoanConflict::ChangeWhileBorrowed
        )),
        "changing the retained Token input must conflict: {conflicts:?}"
    );
}

#[test]
fn a_provider_view_permission_cannot_be_claimed_as_owned_storage() {
    provider_permission_cannot_be_claimed_as_owned_storage(RETURNED_VIEW);
}

fn provider_permission_cannot_be_claimed_as_owned_storage(source: &str) {
    use terminal_check_tables::obligations::Lender;
    let input = elaborate(source).unwrap();
    assert!(matches!(
        crate::judge(Ok(input.clone())),
        Ok(RouteVerdict::Accepted(_))
    ));
    let mut changed = input;
    let (machine, obligation) = changed
        .tables
        .machines
        .iter()
        .find_map(|(machine, tables)| {
            tables.obligations.iter().find_map(|(obligation, row)| {
                matches!(
                    row.kind,
                    ObligationKind::Loan {
                        lender: Lender::Provider,
                        ..
                    }
                )
                .then_some((machine, obligation))
            })
        })
        .expect("the helper lends the provider's returned permission");
    let row = changed
        .tables
        .machines
        .get_mut(machine)
        .unwrap()
        .obligations
        .get_mut(obligation)
        .unwrap();
    let ObligationKind::Loan { lender, .. } = &mut row.kind else {
        panic!("the selected obligation is a loan");
    };
    *lender = Lender::Owner;
    assert!(
        matches!(crate::judge(Ok(changed)),
        Err(Refusal { reason: RefusalReason::ForeignIdentity, detail, .. })
        if detail.starts_with(
            "reborrow names a different immediate parent occurrence:"
        )),
        "a provider result cannot claim ownership of its payload"
    );
}

/// The provider-backed case also needs its executable call-flow account.
#[test]
fn provider_indexed_view_payloads_keep_their_actual_lender() {
    let program = r#"
data Token { id: u64; }
boundary trait Fs { machine path(token: @Token) -> @[u8] ensures result.length > 0u64 reaches none crashes none; }
machine see(value: @u8) {}
machine entry() reaches Fs invokes Fs {
    let token: Token = Token { id: 1 };
    let view: @[u8] = Fs::path(@token);
    see(@view[0]);
}
"#;
    let source = program;
    use terminal_check_tables::bindings::{PlaceOrigin, Segment};
    use terminal_check_tables::obligations::Lender;
    let mut input = elaborate(source).unwrap();
    let verdict = crate::judge(Ok(input.clone()));
    assert!(
        matches!(verdict, Ok(RouteVerdict::Accepted(_))),
        "{verdict:?}"
    );
    let (machine, obligation, actual) = input
        .tables
        .machines
        .iter()
        .find_map(|(machine, tables)| {
            tables.obligations.iter().find_map(|(obligation, row)| {
                let ObligationKind::Loan {
                    captured, lender, ..
                } = row.kind
                else {
                    return None;
                };
                matches!(
                    tables.places.get(captured)?.origin,
                    PlaceOrigin::Projection {
                        segment: Segment::FixedIndex(_) | Segment::RuntimeIndex { .. },
                        ..
                    }
                )
                .then_some((machine, obligation, lender))
            })
        })
        .expect("the indexed element has its own captured loan");
    assert!(
        actual == Lender::Provider,
        "an element keeps the descriptor's actual lender: {actual:?}"
    );
    let ObligationKind::Loan { lender, .. } = &mut input
        .tables
        .machines
        .get_mut(machine)
        .unwrap()
        .obligations
        .get_mut(obligation)
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    *lender = Lender::Owner;
    let forged = crate::judge(Ok(input));
    assert!(
        matches!(forged,
        Err(Refusal { reason: RefusalReason::ForeignIdentity, ref detail, .. })
        if detail.starts_with("reborrow names a different immediate parent occurrence")),
        "owning a view descriptor cannot grant ownership of its payload: {forged:?}"
    );
}

/// The twin: nothing says the returned view is not the one passed in, so the
/// array it views stays borrowed while the result is used. Writing the array
/// before the result's last use is a conflict; after it, the array is the
/// machine's again.
#[test]
fn a_view_of_unknown_origin_keeps_what_the_call_was_given() {
    let program = |before: &str, after: &str| {
        format!(
            r#"
boundary trait Fs {{ machine echo(bytes: @[u8]) -> @[u8] reaches none crashes none; }}
machine pass(bytes: @[u8]) -> @[u8] reaches Fs invokes Fs {{ Fs::echo(bytes) }}
machine see(bytes: @[u8]) {{ }}
machine entry() reaches Fs invokes Fs {{
    let mutable name: [u8; 4] = [1, 2, 3, 4];
    let rest: @[u8] = pass(@name[..]);
    {before}
    see(rest);
    {after}
}}
"#
        )
    };
    let write = "name[0] = 9;";
    assert_eq!(verdict(&program("", write)), "accepted");
    assert_eq!(verdict(&program(write, "")), "rejected: 1 conflicts");
}

/// The helper receives both views, so the indirect call must retain their
/// actual input loans. Construction still owes the selected body's promised
/// first-input origin; the second view has its own independent lifetime.
#[test]
fn a_stored_callable_keeps_its_first_inputs_returned_view() {
    let source = r#"
machine first_view<'first, 'second>(first: @'first [u8], second: @'second [u8]) -> @'first [u8] {
    first
}
machine second_view<'first, 'second>(first: @'first [u8], second: @'second [u8]) -> @'second [u8] {
    second
}
machine see(bytes: @[u8]) {}
data SelectFirst = machine<'left, 'right>(left: @'left [u8], right: @'right [u8]) -> @'left [u8]
        reaches none crashes none;
machine use_selected(
    select: SelectFirst,
    first: @[u8], second: @[u8]
) {
    let view: @[u8] = select(first, second);
    see(view);
}
machine entry() {
    let select: SelectFirst = first_view;
    use_selected(select, "first", "second");
}
"#;
    assert_eq!(verdict(source), "accepted");
    let wrong_input = source.replace("SelectFirst = first_view;", "SelectFirst = second_view;");
    assert_ne!(source, wrong_input);
    let refused = verdict(&wrong_input);
    assert_eq!(
        refused,
        "dynamic row returned view exceeds its public input regions in machine `second_view`",
        "the selected body cannot return the independently borrowed second input: {refused}"
    );
}

/// The provider may keep its bytes outside `Token`; its declared lifetime
/// still keeps that token borrowed until the returned view's last use.
#[test]
fn a_boundary_view_keeps_its_declared_input_loan() {
    let program = |before: &str, after: &str| {
        format!(
            r#"
data Token {{ id: u64; }}
boundary trait Fs {{ machine path(hint: u64, token: @Token) -> @[u8] reaches none crashes none; }}
machine see(bytes: @[u8]) {{ }}
machine eat(token: Token) {{ }}
machine entry() reaches Fs invokes Fs {{
    let token: Token = Token {{ id: 1 }};
    let path: @[u8] = Fs::path(7, @token);
    {before}
    see(path);
    {after}
}}
"#
        )
    };
    for forwarded in [false, true] {
        let source = |before, after| {
            let text = program(before, after);
            if forwarded {
                text.replace(
                    "machine see(",
                    "machine forward<'a>(token: @'a Token) -> @'a [u8] reaches Fs invokes Fs { Fs::path(7, token) }\nmachine see(",
                )
                .replace("Fs::path(7, @token);", "forward(@token);")
            } else {
                text
            }
        };
        assert_eq!(verdict(&source("", "eat(token);")), "accepted");
        assert_eq!(verdict(&source("eat(token);", "")), "rejected: 1 conflicts");
    }
}

#[test]
fn a_boundary_view_does_not_lend_unrelated_storage() {
    let source = r#"
data Token { id: u64; }
boundary trait Fs {
    machine path(token: @Token) -> @[u8]
        reaches none crashes none;
}
machine see(bytes: @[u8]) { }
machine eat(token: Token) { }
machine entry() reaches Fs invokes Fs {
    let token: Token = Token { id: 1 };
    let other: Token = Token { id: 2 };
    let path: @[u8] = Fs::path(@token);
    eat(other);
    see(path);
    eat(token);
}
"#;
    assert_eq!(verdict(source), "accepted");
    assert_eq!(
        verdict(&source.replace(
            "    eat(other);\n    see(path);\n    eat(token);",
            "    eat(token);\n    see(path);\n    eat(other);",
        )),
        "rejected: 1 conflicts"
    );
}

const PROVIDER_VIEW: &str = r#"
data Token { id: u64; }
boundary trait Fs { machine path(hint: u64, token: @Token) -> @[u8] reaches none crashes none; }
machine see(bytes: @[u8]) { }
machine entry() reaches Fs invokes Fs {
    let token: Token = Token { id: 1 };
    let path: @[u8] = Fs::path(7, @token);
    see(path);
}
"#;

const MUTABLE_PROVIDER_SETTING: &str = r#"
data Group { disabled: bool; }
data Facet {}
boundary trait Registration {
    machine issue(facet: @Facet) -> @mutable Group reaches none crashes none;
}
machine observe(group: @Group) -> bool { group.disabled }
machine keep<'a>(group: @'a Group) -> @'a Group { group }
machine entry() -> bool reaches Registration invokes Registration {
    let facet: Facet = Facet {};
    let setting: @mutable Group = Registration::issue(@facet);
    setting.disabled = true;
    observe(setting)
}
"#;

#[test]
fn mutable_provider_record_store_requires_its_own_live_permission() {
    assert_eq!(verdict(MUTABLE_PROVIDER_SETTING), "accepted");
    let shared = MUTABLE_PROVIDER_SETTING.replace("@mutable Group", "@Group");
    let shared_access = verdict(&shared);
    let borrowed = MUTABLE_PROVIDER_SETTING.replace(
        "setting.disabled = true;\n    observe(setting)",
        "let held: @Group = keep(setting);\n    setting.disabled = true;\n    observe(held)",
    );
    assert_ne!(borrowed, MUTABLE_PROVIDER_SETTING);
    let retained_access = verdict(&borrowed);
    let exclusive_input = MUTABLE_PROVIDER_SETTING
        .replace("facet: @Facet", "facet: @mutable Facet")
        .replace("let facet: Facet =", "let mutable facet: Facet =")
        .replace("issue(@facet)", "issue(@mutable facet)");
    let exclusive_input = verdict(&exclusive_input);
    assert!(shared_access.starts_with("rejected:"), "{shared_access}");
    assert_ne!(retained_access, "accepted");
    assert!(
        exclusive_input.contains("a writable provider borrow needs its exact payload region"),
        "{exclusive_input}"
    );
}

#[test]
fn a_call_can_mutate_provider_scalar_cells_without_a_guest_address() {
    let source = r#"
data Fields { value: i32; }
data Settings { fields: Fields; enabled: bool; }
boundary trait Source {
    machine issue() -> @mutable Settings reaches none crashes none;
    machine configure(settings: @mutable Settings) reaches none crashes none;
}
machine entry() -> i32 reaches Source invokes Source {
    let settings: @mutable Settings = Source::issue();
    Source::configure(settings);
    settings.fields.value
}
"#;
    provider_reference_has_no_captured_address(source);
    provider_reference_value_is_unknown(source);
}

const PROVIDER_REFERENCE: &str = r#"
boundary trait Source { machine slot() -> @i32 reaches none crashes none; }
machine read(slot: @i32) -> i32 { $slot }
machine entry() -> i32 reaches Source invokes Source { read(Source::slot()) }
"#;

const PROVIDER_RECORD_REFERENCE: &str = r#"
data Pair { left: i32; right: i32; }
boundary trait Source { machine cell() -> @Pair reaches none crashes none; }
machine read(pair: @Pair) -> i32 { pair.left }
machine entry() -> i32 reaches Source invokes Source { read(Source::cell()) }
"#;

const DIRECT_PROVIDER_FIELD: &str = r#"
data Pair { left: i32; right: i32; }
boundary trait Source { machine cell() -> @Pair reaches none crashes none; }
machine entry() -> i32 reaches Source invokes Source {
    let pair: @Pair = Source::cell();
    pair.left
}
"#;

const PROVIDER_NESTED_RECORD_REFERENCE: &str = r#"
data Values { left: i32; right: i32; }
data Pair { values: Values; other: i32; }
boundary trait Source { machine cell() -> @Pair reaches none crashes none; }
machine read(pair: @Pair) -> i32 { pair.values.left }
machine entry() -> i32 reaches Source invokes Source { read(Source::cell()) }
"#;

#[test]
fn boundary_reference_completion_reads_inline_record_leaves() {
    for source in [
        PROVIDER_RECORD_REFERENCE,
        PROVIDER_NESTED_RECORD_REFERENCE,
        DIRECT_PROVIDER_FIELD,
    ] {
        let verdict = crate::judge(elaborate(source));
        assert!(
            matches!(verdict, Ok(RouteVerdict::Accepted(_))),
            "the shared provider contract establishes its typed record leaves: {verdict:?}"
        );
    }
}

#[test]
fn boundary_reference_completion_reads_a_caller_chosen_provider_result() {
    let elaborated =
        elaborate(PROVIDER_REFERENCE).expect("the admitted reference keeps its provider contract");
    let boundary = &elaborated.module.interfaces.boundary_machines[0];
    assert!(boundary.result_reference_regions.is_empty());
    let groups = boundary.result_lifetimes.as_ref().unwrap();
    assert_eq!(groups.len(), 1);
    assert!(groups[0].inputs.is_empty());
    assert_eq!(groups[0].results, vec![Vec::new()]);
    let verdict = crate::judge(Ok(elaborated));
    assert!(
        matches!(verdict, Ok(RouteVerdict::Accepted(_))),
        "the shared provider reference must retain its exact permission: {verdict:?}"
    );
}

#[test]
fn boundary_reference_completion_does_not_invent_a_captured_address() {
    provider_reference_has_no_captured_address(PROVIDER_REFERENCE);
}

fn provider_reference_has_no_captured_address(source: &str) {
    use terminal_semantics::descriptor_lifetimes::descriptor_location;
    use terminal_semantics::descriptor_results::{
        DescriptorAnalysis, DescriptorProviderOutput, DescriptorSourceOrigin,
        terminal_borrowed_storage_graph_from,
    };
    let input = elaborate(source).unwrap();
    let (machine, at, actual) = input
        .module
        .machines
        .iter()
        .find_map(|machine| {
            machine.blocks.iter().find_map(|block| {
                block
                    .operations
                    .iter()
                    .enumerate()
                    .find_map(|(ordinal, operation)| {
                        let call = operation.declared_invocation()?;
                        let actual = call.structural_arguments.first()?;
                        (actual.path == [terminal_psi::StructuralPathSegment::Referent]).then_some(
                            (machine, (block.id, u32::try_from(ordinal).unwrap()), actual),
                        )
                    })
            })
        })
        .expect("the original read borrows the provider's returned referent");
    let graph = terminal_borrowed_storage_graph_from(&input.module, [machine.id], &[]).unwrap();
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&input.module.types.structural_types, &graph);
    let descriptor =
        terminal_semantics::descriptor_lifetimes::DescriptorLocation::whole(actual.place);
    let origins = analysis
        .symbolic_source_origins(machine.id, at, &descriptor)
        .unwrap();
    let [DescriptorSourceOrigin::Provider(provider)] = origins.as_slice() else {
        panic!("the actual call retains its exact provider provenance: {origins:?}");
    };
    assert_eq!(
        provider.output,
        DescriptorProviderOutput::Result(Vec::new())
    );
    assert_eq!(provider.invocations.len(), 1);
    let referent = descriptor_location(
        &input.module.types.structural_types,
        actual.place,
        machine.structural_place_type(actual.place).unwrap(),
        &actual.path,
        None,
    )
    .unwrap();
    assert_eq!(
        analysis.captured_storage_family(machine.id, at, &referent),
        Err("reference descriptor has no supported captured address account"),
        "provider permission must not fabricate an original storage address",
    );
    assert!(matches!(
        crate::judge(Ok(input)),
        Ok(RouteVerdict::Accepted(_))
    ));
}

#[test]
fn boundary_reference_completion_does_not_claim_an_unknown_scalar_value() {
    provider_reference_value_is_unknown(PROVIDER_REFERENCE);
}

fn provider_reference_value_is_unknown(source: &str) {
    let changed = source.replace(
        "machine entry() -> i32",
        "machine entry() -> i32 ensures result == 0i32",
    );
    assert_ne!(changed, source);
    let verdict = crate::judge(elaborate(&changed));
    let Ok(RouteVerdict::Rejected {
        formations,
        violations,
        unproved,
        unestablished,
        conflicts,
        exceeded,
    }) = verdict
    else {
        panic!("an initialized provider referent has no promised zero value: {verdict:?}");
    };
    assert!(
        formations.is_empty()
            && violations.is_empty()
            && unestablished.is_empty()
            && conflicts.is_empty()
            && exceeded.is_empty()
    );
    assert!(
        unproved
            .iter()
            .any(|failure| { matches!(failure.kind, ObligationKind::NormalGuarantee { .. }) })
    );
    assert!(unproved.iter().all(|failure| matches!(
        failure.kind,
        ObligationKind::NormalGuarantee { .. } | ObligationKind::PublishedGuarantee { .. }
    )));
}

#[test]
fn boundary_reference_completion_keeps_record_values_unknown() {
    for source in [
        PROVIDER_RECORD_REFERENCE,
        PROVIDER_NESTED_RECORD_REFERENCE,
        DIRECT_PROVIDER_FIELD,
    ] {
        provider_reference_value_is_unknown(source);
    }
}

#[test]
fn boundary_reference_completion_keeps_record_addresses_opaque() {
    for source in [PROVIDER_RECORD_REFERENCE, PROVIDER_NESTED_RECORD_REFERENCE] {
        provider_reference_has_no_captured_address(source);
        provider_permission_cannot_be_claimed_as_owned_storage(source);
    }
}

#[test]
fn boundary_reference_completion_cannot_claim_owned_backing() {
    provider_permission_cannot_be_claimed_as_owned_storage(PROVIDER_REFERENCE);
}

#[test]
fn boundary_reference_completion_keeps_its_logical_input_loan() {
    let source = r#"
data Token { id: u64; }
boundary trait Source { machine slot(token: @Token) -> @i32 reaches none crashes none; }
machine read(slot: @i32) -> i32 { $slot }
machine eat(token: Token) {}
machine entry() -> i32 reaches Source invokes Source {
    let token: Token = Token { id: 1 };
    let slot: @i32 = Source::slot(@token);
    let value: i32 = read(slot);
    eat(token);
    value
}
"#;
    assert_eq!(verdict(source), "accepted");
    let changed = source.replace(
        "    let value: i32 = read(slot);\n    eat(token);",
        "    eat(token);\n    let value: i32 = read(slot);",
    );
    assert_ne!(changed, source);
    let result = crate::judge(elaborate(&changed));
    let Ok(RouteVerdict::Rejected { conflicts, .. }) = result else {
        panic!("a provider reference must retain its exact input loan: {result:?}");
    };
    assert!(conflicts.iter().any(|conflict| matches!(
        conflict.conflict,
        crate::LoanConflict::ExclusiveAndShared
            | crate::LoanConflict::ChangeWhileBorrowed
    )));
}

/// Reference and view frontiers share provider provenance. This lowered
/// contract control changes only the borrowed leaf's constructor; no body
/// or physical address supplies backing to the boundary declaration.
#[test]
fn boundary_reference_completion_validates_its_provider_frontier() {
    use terminal_semantics::descriptor_results::{
        DescriptorProviderOutput, DescriptorSlotKind, DescriptorStoredOrigin,
    };
    let mut baseline = elaborate(PROVIDER_VIEW).unwrap();
    let boundary = &baseline.module.interfaces.boundary_machines[0];
    let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result else {
        panic!("the provider returns a borrowed structural leaf");
    };
    let result_type = result.structural_type;
    let referent = boundary.structural_parameters[0].structural_type;
    baseline
        .module
        .types.structural_types
        .iter_mut()
        .find(|row| row.id == result_type)
        .unwrap()
        .shape = terminal_psi::StructuralTypeShape::Reference {
        referent,
        access: terminal_psi::StructuralAccess::SharedBorrow,
    };
    let boundary = &baseline.module.interfaces.boundary_machines[0];
    let operation_result = baseline
        .module
        .machines
        .iter()
        .flat_map(|machine| machine.operations())
        .find_map(|operation| {
            (operation.declared_invocation()?.callee
                == terminal_psi::TargetMachineCallable::Boundary(boundary.id))
            .then(|| operation.result.as_structural())
            .flatten()
        })
        .unwrap();
    let slots = |boundary: &terminal_psi::BoundaryMachineDeclaration| {
        terminal_semantics::boundary_result_slots(
            &baseline.module.types.structural_types,
            boundary,
            DescriptorSlotKind::BorrowedStorage,
            operation_result,
        )
    };
    let results = slots(boundary).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].path.is_empty());
    let [DescriptorStoredOrigin::Provider(provider)] = results[0].origins.as_slice() else {
        panic!("the reference keeps the boundary's exact provider origin");
    };
    assert_eq!(provider.boundary, boundary.id);
    assert_eq!(
        provider.output,
        DescriptorProviderOutput::Result(Vec::new())
    );
    assert!(provider.invocations.is_empty());
    assert_eq!(provider.lifetime_dependencies.len(), 1);
    assert!(matches!(
        &provider.lifetime_dependencies[0].origin,
        DescriptorStoredOrigin::Parameter {
            position: 0,
            projection,
        } if *projection == terminal_semantics::borrowed_storage::Projection::new(&[])
    ));
    assert_eq!(
        provider.lifetime_dependencies[0].access,
        terminal_psi::StructuralAccess::SharedBorrow
    );
    for corruption in 0..6 {
        let mut changed = boundary.clone();
        match corruption {
            0 => changed.result_lifetimes = None,
            1 => changed.result_lifetimes = Some(Vec::new()),
            2 => changed.result_lifetimes.as_mut().unwrap()[0]
                .results
                .push(Vec::new()),
            3 => changed.result_lifetimes.as_mut().unwrap()[0].results[0]
                .push(terminal_psi::StructuralPathSegment::Referent),
            4 => {
                changed.result_lifetimes.as_mut().unwrap()[0].inputs[0] =
                    terminal_psi::BoundaryLifetimeDependency::Referent {
                        argument_index: 1,
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    }
            }
            5 => {
                changed.result_lifetimes.as_mut().unwrap()[0].inputs[0] =
                    terminal_psi::BoundaryLifetimeDependency::Referent {
                        argument_index: 0,
                        access: terminal_psi::StructuralAccess::MutableBorrow,
                    }
            }
            _ => unreachable!(),
        }
        assert!(slots(&changed).is_err(), "corruption {corruption}");
    }
}

#[test]
fn boundary_result_lifetime_metadata_is_independently_validated() {
    use terminal_psi::{BoundaryLifetimeDependency, StructuralAccess};
    let baseline = elaborate(PROVIDER_VIEW).unwrap();
    let boundary = &baseline.module.interfaces.boundary_machines[0];
    assert!(boundary.result_reference_regions.is_empty());
    let groups = boundary.result_lifetimes.as_ref().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].results, vec![Vec::new()]);
    assert_eq!(
        groups[0].inputs,
        vec![BoundaryLifetimeDependency::Referent {
            argument_index: 0,
            access: StructuralAccess::SharedBorrow,
        }]
    );
    assert!(
        terminal_semantics::boundary_result_lifetimes(&baseline.module.types.structural_types, boundary,)
            .is_ok()
    );
    let caller = baseline
        .module
        .machines
        .iter()
        .find(|machine| {
            machine.operations().any(|operation| {
                operation.result.as_structural().is_some()
                    && operation.declared_invocation().is_some()
            })
        })
        .unwrap();
    let operation = caller
        .operations()
        .find(|operation| {
            operation.result.as_structural().is_some() && operation.declared_invocation().is_some()
        })
        .unwrap();
    let result = terminal_semantics::descriptor_results::BoundaryCompletionResult::Structural(
        operation.result.as_structural().unwrap().structural_type,
    );
    for receipts in [false, true] {
        assert_eq!(terminal_semantics::descriptor_results::boundary_completion_preserves_descriptor_storage(
            baseline.module.types.structural_types.iter(), boundary, result, receipts,
            operation.kind.structural_arguments(), |place| caller.structural_place_type(place),
        ), !receipts, "a completion receipt keeps its independent custody obligation");
    }
    for corruption in 0..7 {
        let mut malformed = baseline.clone();
        let boundary = &mut malformed.module.interfaces.boundary_machines[0];
        match corruption {
            0 => boundary.result_lifetimes = None,
            1 => boundary.result_lifetimes = Some(Vec::new()),
            2 => boundary.result_lifetimes.as_mut().unwrap()[0]
                .results
                .clear(),
            3 => boundary.result_lifetimes.as_mut().unwrap()[0]
                .results
                .push(Vec::new()),
            4 => {
                boundary.result_lifetimes.as_mut().unwrap()[0].inputs[0] =
                    BoundaryLifetimeDependency::Referent {
                        argument_index: 1,
                        access: StructuralAccess::SharedBorrow,
                    }
            }
            5 => {
                boundary.result_lifetimes.as_mut().unwrap()[0].inputs[0] =
                    BoundaryLifetimeDependency::Referent {
                        argument_index: 0,
                        access: StructuralAccess::MutableBorrow,
                    }
            }
            6 => boundary.result_reference_regions.push(
                terminal_psi::TerminalDynamicReferenceRegion {
                    path: Vec::new(),
                    origin: terminal_psi::TerminalDynamicReferenceOrigin::Parameter {
                        argument_index: 0,
                    },
                    source_path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                },
            ),
            _ => unreachable!(),
        }
        assert!(
            terminal_semantics::boundary_result_lifetimes(
                &malformed.module.types.structural_types,
                boundary,
            )
            .is_err(),
            "corruption {corruption}"
        );
        assert!(
            !matches!(
                crate::judge(Ok(malformed)),
                Ok(RouteVerdict::Accepted(_))
            ),
            "the sole checker accepted corruption {corruption}"
        );
    }
}

#[test]
fn a_caller_chosen_boundary_view_has_no_input_loan() {
    let source = r#"
data Token { id: u64; }
boundary trait Fs { machine path(hint: u64) -> @[u8] reaches none crashes none; }
machine see(bytes: @[u8]) { }
machine eat(token: Token) { }
machine entry() reaches Fs invokes Fs {
    let token: Token = Token { id: 1 };
    let path: @[u8] = Fs::path(7);
    eat(token);
    see(path);
}
"#;
    let elaborated = elaborate(source).unwrap();
    let groups = elaborated.module.interfaces.boundary_machines[0]
        .result_lifetimes
        .as_ref()
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert!(groups[0].inputs.is_empty());
    assert_eq!(verdict(source), "accepted");
}

#[test]
fn a_dynamic_view_without_declared_logical_lifetimes_is_refused() {
    let elaborated = elaborate(PROVIDER_VIEW).unwrap();
    let caller = elaborated
        .module
        .machines
        .iter()
        .find(|machine| {
            machine
                .operations()
                .any(|operation| operation.declared_invocation().is_some())
        })
        .unwrap();
    let mut operation = caller
        .operations()
        .find(|operation| {
            operation.result.as_structural().is_some() && operation.declared_invocation().is_some()
        })
        .unwrap()
        .clone();
    operation.kind = terminal_psi::OperationKind::CallDynamic {
        selector: semantic_vocabulary::ValueId::new(1).unwrap(),
        interface: 0,
        slot: 0,
        arguments: Vec::new(),
        family_actuals: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let mut analysis = terminal_semantics::descriptor_results::DescriptorAnalysis::new(
        &elaborated.module.types.structural_types,
        &[],
    );
    let refusal = match terminal_semantics::reference_call_lifetimes(
        &elaborated.module,
        caller,
        &operation,
        &mut analysis,
    ) {
        Err(error) => error,
        Ok(_) => panic!("a dynamic view still needs its declared lifetime contract"),
    };
    assert_eq!(
        refusal.detail(),
        Some("returned dynamic view needs its declared lifetime contract")
    );
}

#[test]
fn a_nested_boundary_view_keeps_its_input_loan() {
    let source = r#"
data Token { id: u64; }
data Path<'a> { bytes: @'a [u8]; }
boundary trait Fs { machine path(token: @Token) -> Path reaches none crashes none; }
machine forward(token: @Token) -> Path reaches Fs invokes Fs { Fs::path(token) }
machine see(bytes: @[u8]) { }
machine eat(token: Token) { }
machine entry() reaches Fs invokes Fs {
    let token: Token = Token { id: 1 };
    let path: Path = forward(@token);
    see(path.bytes);
    eat(token);
}
"#;
    assert_eq!(verdict(source), "accepted");
    let conflict = source.replace(
        "    see(path.bytes);\n    eat(token);",
        "    eat(token);\n    see(path.bytes);",
    );
    assert_ne!(source, conflict);
    assert_eq!(verdict(&conflict), "rejected: 1 conflicts");

    // Component modules may assign this same field another identity index.
    // Compare the boundary frontier, not unrelated module/source records.
    let elaborated = elaborate(source).unwrap();
    let left = &elaborated.module;
    let mut right = left.clone();
    let boundary = &left.interfaces.boundary_machines[0];
    let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result else {
        panic!("the boundary returns a record");
    };
    let terminal_psi::StructuralPathSegment::Field(original) =
        &boundary.result_lifetimes.as_ref().unwrap()[0].results[0][0]
    else {
        panic!("the returned descriptor belongs to a field");
    };
    let original = *original;
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    for position in 0..=original.0 {
        identities.intern(&format!("unrelated component identity {position}"));
    }
    let relocated = identities.intern(left.identities.text(original).unwrap());
    assert_ne!(original, relocated);
    right.identities = identities.build();
    let record = right
        .types.structural_types
        .iter_mut()
        .find(|row| row.id == result.structural_type)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &mut record.shape else {
        panic!("the result is a record");
    };
    fields
        .iter_mut()
        .find(|field| field.identity == original)
        .unwrap()
        .identity = relocated;
    right.interfaces.boundary_machines[0]
        .result_lifetimes
        .as_mut()
        .unwrap()[0]
        .results[0][0] = terminal_psi::StructuralPathSegment::Field(relocated);
    assert!(
        terminal_semantics::boundary_result_lifetimes_match(
            left,
            boundary,
            &right,
            &right.interfaces.boundary_machines[0],
        )
        .unwrap()
    );
    right.interfaces.boundary_machines[0]
        .result_lifetimes
        .as_mut()
        .unwrap()[0]
        .inputs
        .clear();
    assert!(
        !terminal_semantics::boundary_result_lifetimes_match(
            left,
            boundary,
            &right,
            &right.interfaces.boundary_machines[0],
        )
        .unwrap(),
        "equal field identities cannot hide different lifetime dependencies"
    );
}

#[test]
fn a_boundary_view_keeps_the_backing_of_a_stored_input_view() {
    let source = r#"
data View<'a> { bytes: @'a [u8]; }
boundary trait Fs { machine path(view: View) -> @[u8] reaches none crashes none; }
machine see(bytes: @[u8]) { }
machine entry() reaches Fs invokes Fs {
    let mutable name: [u8; 4] = [1, 2, 3, 4];
    let view: View = View { bytes: @name[..] };
    let path: @[u8] = Fs::path(view);
    see(path);
    name[0] = 9;
}
"#;
    assert_eq!(verdict(source), "accepted");
    let conflict = source.replace(
        "    see(path);\n    name[0] = 9;",
        "    name[0] = 9;\n    see(path);",
    );
    assert_ne!(source, conflict);
    assert_eq!(verdict(&conflict), "rejected: 1 conflicts");
}

#[test]
fn a_forwarded_mutable_view_lends_its_element_to_the_caller() {
    let source = r#"
data Cell [copy] { value: u32; }
data Pair { cells: [Cell; 2]; }
machine select(pair: @mutable Pair) -> @mutable Cell {
    let cells: @mutable [Cell] = @mutable pair.cells[..];
    transition { _ -> second(cells) }
    machine second(cells: @mutable [Cell]) -> @mutable Cell
    requires cells.length == 2u64 {
        transition { _ -> @mutable cells[1u64] }
    }
}
machine entry() crashes Trap {
    let mutable pair: Pair;
    pair.cells[0u64].value = 7u32;
    let cell: @mutable Cell = select(@mutable pair);
    cell.value = 42u32;
    transition pair.cells[1u64].value == 42u32 && pair.cells[0u64].value == 7u32 {
        true -> done(), false -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    assert_eq!(
        super::stored_references::run(source),
        Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        ))
    );
    let conflict = source.replace(
        "    cell.value = 42u32;",
        "    pair.cells[1u64].value = 11u32;\n    cell.value = 42u32;",
    );
    assert_eq!(verdict(&conflict), "rejected: 1 conflicts");
}

#[test]
fn a_returned_dynamic_element_keeps_its_captured_index() {
    let source = r#"
data Cell [copy] { value: u32; }
data Pair { cells: [Cell; 2]; }
machine select(pair: @mutable Pair, index: u64) -> @mutable Cell
requires index < 2u64 {
    let cells: @mutable [Cell] = @mutable pair.cells[..];
    transition { _ -> pick(cells, index) }
    machine pick(cells: @mutable [Cell], index: u64) -> @mutable Cell
    requires cells.length == 2u64, index < 2u64 {
        @mutable cells[index]
    }
}
machine entry() crashes Trap {
    let mutable pair: Pair;
    pair.cells[0u64].value = 7u32;
    let mutable selector: u64 = 1u64;
    let cell: @mutable Cell = select(@mutable pair, selector);
    selector = 0u64;
    cell.value = 42u32;
    transition pair.cells[selector].value == 7u32 && pair.cells[1u64].value == 42u32 {
        true -> done(), false -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    assert_eq!(
        super::stored_references::run(source),
        Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        ))
    );
    let conflict = source.replace(
        "    cell.value =",
        "    pair.cells[1u64].value = 29u32;\n    cell.value =",
    );
    assert_eq!(verdict(&conflict), "rejected: 1 conflicts");
    let beyond = source.replace("@mutable cells[index]", "@mutable cells[index + 2u64]");
    let result = crate::judge(elaborate(&beyond)).unwrap();
    assert!(
        matches!(&result, RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))),
        "{result:?}"
    );
}

#[test]
fn indexed_view_payloads_keep_their_actual_lender() {
    let source = r#"
machine see(value: @u8) {}
machine entry() {
    let values: [u8; 2] = [1, 2];
    let view: @[u8] = @values[..];
    see(@view[0]);
}
"#;
    use terminal_check_tables::bindings::{PlaceOrigin, Segment};
    use terminal_check_tables::obligations::Lender;
    let mut input = elaborate(source).unwrap();
    let verdict = crate::judge(Ok(input.clone()));
    assert!(
        matches!(verdict, Ok(RouteVerdict::Accepted(_))),
        "{verdict:?}"
    );
    let beyond = source.replace("view[0]", "view[2]");
    let unbounded = crate::judge(elaborate(&beyond));
    assert!(
        matches!(&unbounded, Ok(RouteVerdict::Rejected { unproved, .. })
            if unproved.iter().any(|failure| matches!(failure.kind, ObligationKind::OperationSafety { .. }))),
        "the borrowed element still owes its own live bound: {unbounded:?}"
    );
    let wrong_type = source.replace("value: @u8", "value: @u16");
    let mismatch = elaborate(&wrong_type);
    assert!(
        matches!(&mismatch, Err(Refusal { reason: RefusalReason::Violation, detail, .. })
            if detail.starts_with("this value has scalar carrier `u8` but the parameter it is passed for requires `u16`: there is no implicit scalar conversion at a receiving type")),
        "the parameter cannot change the byte's actual carrier: {mismatch:?}"
    );
    let (machine, obligation, actual) = input
        .tables
        .machines
        .iter()
        .find_map(|(machine, tables)| {
            tables.obligations.iter().find_map(|(obligation, row)| {
                let ObligationKind::Loan {
                    captured, lender, ..
                } = row.kind
                else {
                    return None;
                };
                matches!(
                    tables.places.get(captured)?.origin,
                    PlaceOrigin::Projection {
                        segment: Segment::FixedIndex(_) | Segment::RuntimeIndex { .. },
                        ..
                    }
                )
                .then_some((machine, obligation, lender))
            })
        })
        .expect("the indexed element has its own captured loan");
    assert!(
        matches!(actual, Lender::Loan(_)),
        "an element keeps the descriptor's actual lender: {actual:?}"
    );
    let ObligationKind::Loan { lender, .. } = &mut input
        .tables
        .machines
        .get_mut(machine)
        .unwrap()
        .obligations
        .get_mut(obligation)
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    *lender = Lender::Owner;
    let forged = crate::judge(Ok(input));
    assert!(
        matches!(forged,
        Err(Refusal { reason: RefusalReason::ForeignIdentity, ref detail, .. })
        if detail.starts_with("reborrow names a different immediate parent occurrence")),
        "owning a view descriptor cannot grant ownership of its payload: {forged:?}"
    );
}
