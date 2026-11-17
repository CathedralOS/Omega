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

fn elaborate(text: &str) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
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

const RETURNED_VIEW: &str = r#"
data Token { id: u64; }
boundary trait Fs { machine required_path(output: &Token) -> &[u8] reaches none crashes none; }
machine path(output: &Token) -> &[u8] reaches Fs invokes Fs { Fs::required_path(output) }
machine see(bytes: &[u8]) { }
machine entry() reaches Fs invokes Fs {
    let name: [u8; 4] = [1, 2, 3, 4];
    let token: Token = Token { id: 1 };
    see(&name[..]);
    see(path(&token));
}
"#;

/// The local array view demands reference flow for every callee, including
/// the helper that hands on the boundary's returned view.
#[test]
fn a_view_a_boundary_returned_is_handed_on_by_a_machine() {
    assert_eq!(verdict(RETURNED_VIEW), "accepted");
}

#[test]
fn a_provider_view_keeps_its_input_loan_without_borrowing_independent_storage() {
    let source = r#"
data Token { id: u64; }
boundary trait Fs { machine path(source: &Token) -> &[u8] reaches none crashes none; }
machine helper(source: &Token) -> &[u8] reaches Fs invokes Fs { Fs::path(source) }
machine see(bytes: &[u8]) {}
machine use_path(other: &mut Token, bytes: &[u8]) { see(bytes); other.id = 4; }
machine entry() reaches Fs invokes Fs {
    let mut source: Token = Token { id: 1 };
    let mut other: Token = Token { id: 2 };
    let bytes: &[u8] = helper(&source);
    use_path(&mut other, bytes);
    source.id = 9;
}
"#;
    assert_eq!(verdict(source), "accepted");
    let invalid = source.replace(
        "use_path(&mut other, bytes)",
        "use_path(&mut source, bytes)",
    );
    assert_ne!(source, invalid);
    let rejected = crate::judge(elaborate(&invalid));
    let Ok(RouteVerdict::Rejected { conflicts, .. }) = rejected else {
        panic!("the Token input must remain borrowed through use_path: {rejected:?}");
    };
    assert!(
        conflicts.iter().any(|conflict| matches!(
            conflict.conflict,
            crate::LoanConflict::ExclusiveAndShared | crate::LoanConflict::ChangeWhileBorrowed
        )),
        "changing the retained Token input must conflict: {conflicts:?}"
    );
}

#[test]
fn a_provider_view_permission_cannot_be_claimed_as_owned_storage() {
    use terminal_check_tables::obligations::Lender;
    let input = elaborate(RETURNED_VIEW).unwrap();
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
        .expect("the helper lends the provider's returned view");
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

#[test]
fn indexed_view_payloads_keep_their_actual_lender() {
    use terminal_check_tables::bindings::{PlaceOrigin, Segment};
    use terminal_check_tables::obligations::Lender;
    let local = r#"
machine see(value: &u8) {}
machine entry() {
    let values: [u8; 2] = [1, 2];
    let view: &[u8] = &values[..];
    see(&view[0]);
}
"#;
    let provider = r#"
data Token { id: u64; }
boundary trait Fs { machine path(token: &Token) -> &[u8] ensures result.len > 0u64 reaches none crashes none; }
machine see(value: &u8) {}
machine entry() reaches Fs invokes Fs {
    let token: Token = Token { id: 1 };
    let view: &[u8] = Fs::path(&token);
    see(&view[0]);
}
"#;
    for (source, provider) in [(local, false), (provider, true)] {
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
            if provider {
                actual == Lender::Provider
            } else {
                matches!(actual, Lender::Loan(_))
            },
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
boundary trait Fs {{ machine echo(bytes: &[u8]) -> &[u8] reaches none crashes none; }}
machine pass(bytes: &[u8]) -> &[u8] reaches Fs invokes Fs {{ Fs::echo(bytes) }}
machine see(bytes: &[u8]) {{ }}
machine entry() reaches Fs invokes Fs {{
    let mut name: [u8; 4] = [1, 2, 3, 4];
    let rest: &[u8] = pass(&name[..]);
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

/// The provider may keep its bytes outside `Token`; its declared lifetime
/// still keeps that token borrowed until the returned view's last use.
#[test]
fn a_boundary_view_keeps_its_declared_input_loan() {
    let program = |before: &str, after: &str| {
        format!(
            r#"
data Token {{ id: u64; }}
boundary trait Fs {{ machine path(hint: u64, token: &Token) -> &[u8] reaches none crashes none; }}
machine see(bytes: &[u8]) {{ }}
machine eat(token: Token) {{ }}
machine entry() reaches Fs invokes Fs {{
    let token: Token = Token {{ id: 1 }};
    let path: &[u8] = Fs::path(7, &token);
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
                    "machine forward<'a>(token: &'a Token) -> &'a [u8] reaches Fs invokes Fs { Fs::path(7, token) }\nmachine see(",
                )
                .replace("Fs::path(7, &token);", "forward(&token);")
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
    machine path(token: &Token) -> &[u8]
        reaches none crashes none;
}
machine see(bytes: &[u8]) { }
machine eat(token: Token) { }
machine entry() reaches Fs invokes Fs {
    let token: Token = Token { id: 1 };
    let other: Token = Token { id: 2 };
    let path: &[u8] = Fs::path(&token);
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
boundary trait Fs { machine path(hint: u64, token: &Token) -> &[u8] reaches none crashes none; }
machine see(bytes: &[u8]) { }
machine entry() reaches Fs invokes Fs {
    let token: Token = Token { id: 1 };
    let path: &[u8] = Fs::path(7, &token);
    see(path);
}
"#;

#[test]
fn boundary_result_lifetime_metadata_is_independently_validated() {
    use terminal_psi::{BoundaryLifetimeDependency, StructuralAccess};
    let baseline = elaborate(PROVIDER_VIEW).unwrap();
    let boundary = &baseline.module.boundary_machines[0];
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
        terminal_semantics::boundary_result_lifetimes(&baseline.module.structural_types, boundary,)
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
            baseline.module.structural_types.iter(), boundary, result, receipts,
            operation.kind.structural_arguments(), |place| caller.structural_place_type(place),
        ), !receipts, "a completion receipt keeps its independent custody obligation");
    }
    for corruption in 0..7 {
        let mut malformed = baseline.clone();
        let boundary = &mut malformed.module.boundary_machines[0];
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
                &malformed.module.structural_types,
                boundary,
            )
            .is_err(),
            "corruption {corruption}"
        );
        assert!(
            !matches!(crate::judge(Ok(malformed)), Ok(RouteVerdict::Accepted(_))),
            "the sole checker accepted corruption {corruption}"
        );
    }
}

#[test]
fn a_caller_chosen_boundary_view_has_no_input_loan() {
    let source = r#"
data Token { id: u64; }
boundary trait Fs { machine path(hint: u64) -> &[u8] reaches none crashes none; }
machine see(bytes: &[u8]) { }
machine eat(token: Token) { }
machine entry() reaches Fs invokes Fs {
    let token: Token = Token { id: 1 };
    let path: &[u8] = Fs::path(7);
    eat(token);
    see(path);
}
"#;
    let elaborated = elaborate(source).unwrap();
    let groups = elaborated.module.boundary_machines[0]
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
        &elaborated.module.structural_types,
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
data Path<'a> { bytes: &'a [u8]; }
boundary trait Fs { machine path(token: &Token) -> Path reaches none crashes none; }
machine forward(token: &Token) -> Path reaches Fs invokes Fs { Fs::path(token) }
machine see(bytes: &[u8]) { }
machine eat(token: Token) { }
machine entry() reaches Fs invokes Fs {
    let token: Token = Token { id: 1 };
    let path: Path = forward(&token);
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
    let boundary = &left.boundary_machines[0];
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
        .structural_types
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
    right.boundary_machines[0]
        .result_lifetimes
        .as_mut()
        .unwrap()[0]
        .results[0][0] = terminal_psi::StructuralPathSegment::Field(relocated);
    assert!(
        terminal_semantics::boundary_result_lifetimes_match(
            left,
            boundary,
            &right,
            &right.boundary_machines[0],
        )
        .unwrap()
    );
    right.boundary_machines[0]
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
            &right.boundary_machines[0],
        )
        .unwrap(),
        "equal field identities cannot hide different lifetime dependencies"
    );
}

#[test]
fn a_boundary_view_keeps_the_backing_of_a_stored_input_view() {
    let source = r#"
data View<'a> { bytes: &'a [u8]; }
boundary trait Fs { machine path(view: View) -> &[u8] reaches none crashes none; }
machine see(bytes: &[u8]) { }
machine entry() reaches Fs invokes Fs {
    let mut name: [u8; 4] = [1, 2, 3, 4];
    let view: View = View { bytes: &name[..] };
    let path: &[u8] = Fs::path(view);
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
