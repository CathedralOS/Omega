//! Checks-internal borrow admission evidence tests.
//!
//! These fixtures build the checked fact arenas directly so a loan pair can be
//! admitted or rejected through `check_flow_call_borrows` without compiling a
//! whole source program.

use checked_trees::expression::ExpressionNode;
use checked_trees::{
    BorrowAccessKind, BorrowCompatibilityConclusion, BorrowCompatibilityDerivation,
    BorrowCompatibilityFormation, BorrowLoanFact, BorrowLoanLineage, BorrowLoanOwnerSegment,
    CapturedPlace, CapturedPlaceContainment, CheckFacts, CheckedBorrowCompatibilityCertificate,
    CheckedBorrowMutationCertificate, FlowConstraintKind, FlowConstraintRef, FlowStateFact,
    FlowStatementFact, StateBorrowFact,
};
use numerics::literals::IntegerLiteral;
use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};

fn symbol(index: u32) -> SymbolHandle {
    SymbolHandle::from_arena_index(index)
}

const MACHINE: u32 = 1;
const STATE: u32 = 2;
const SELF_ROOT: u32 = 3;
const CARRIER: u32 = 4;
const TARGET_LOCAL: u32 = 5;
const X_LOCAL: u32 = 6;
const Y_LOCAL: u32 = 7;
const UNRELATED_LOCAL: u32 = 8;
const A_FIELD: u32 = 10;
const X_FIELD: u32 = 12;
const Y_FIELD: u32 = 13;
const S_FIELD: u32 = 14;
const Z_FIELD: u32 = 15;

fn field(index: u32) -> facts::PlaceSegment {
    facts::PlaceSegment::Field {
        symbol: symbol(index),
    }
}

/// One machine with one state whose body holds `let d = <aggregate>` followed
/// by `let p = <borrow>`; the exact loan facts are installed separately.
fn statement_program() -> typed_trees::TypedTrees {
    let mut program = typed_trees::TypedTrees::default();
    let type_reference =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated_static("u64"),
            });
    let initial_value = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
    let mut machine = Machine {
        symbol: symbol(MACHINE),
        name: Identifier::generated_static("m"),
        ..Default::default()
    };
    let mut state = State {
        symbol: symbol(STATE),
        name: Identifier::generated_static("s"),
        return_type: type_reference,
        ..Default::default()
    };
    for (local, name) in [(CARRIER, "d"), (TARGET_LOCAL, "p")] {
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol: symbol(local),
                name: Identifier::generated_static(name),
                type_reference,
                initial_value,
                is_mutable: false,
                ..Default::default()
            }),
        );
    }
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    program
}

struct LoanSpec<'a> {
    statement_index: usize,
    owner: u32,
    owner_path: &'a [u32],
    source_owner: u32,
    root: u32,
    place: &'a [u32],
    kind: BorrowAccessKind,
    lineage: BorrowLoanLineage,
}

fn install_loan(facts: &mut CheckFacts, spec: LoanSpec<'_>) -> arena::Handle<BorrowLoanFact> {
    let owner_path = facts
        .borrow
        .owner_segments
        .insert_many(
            spec.owner_path
                .iter()
                .map(|index| BorrowLoanOwnerSegment::Field(symbol(*index))),
        )
        .start();
    let owner_path = arena::HandleSpan::from_parts(owner_path, spec.owner_path.len() as u32);
    let segments = facts
        .borrow
        .access_segments
        .insert_many(spec.place.iter().map(|index| field(*index)))
        .start();
    let segments = arena::HandleSpan::from_parts(segments, spec.place.len() as u32);
    facts.borrow.loans.insert(BorrowLoanFact {
        statement_index: spec.statement_index,
        last_use_statement_index: usize::MAX,
        owner_symbol: symbol(spec.owner),
        owner_path,
        source_owner_symbol: if spec.source_owner == 0 {
            SymbolHandle::invalid()
        } else {
            symbol(spec.source_owner)
        },
        lineage: spec.lineage,
        root_symbol: symbol(spec.root),
        segments,
        kind: spec.kind,
    })
}

/// `let p = &mut *d.<selector>` while `d`'s aggregate-carried loans are active:
/// the forming loan derives through local `d` (`source_owner_symbol = d`) but
/// each carried active loan owns a different `d.<field>` projection covering
/// its own captured place.
fn carried_transfer_facts(
    active_places: &[&[u32]],
    forming_place: &[u32],
) -> (CheckFacts, Vec<arena::Handle<BorrowLoanFact>>) {
    let mut facts = CheckFacts::default();
    let mut active_loans = Vec::new();
    for (index, place) in active_places.iter().enumerate() {
        let owner_path = [A_FIELD + index as u32];
        active_loans.push(install_loan(
            &mut facts,
            LoanSpec {
                statement_index: 0,
                owner: CARRIER,
                owner_path: &owner_path,
                source_owner: X_LOCAL + index as u32,
                root: SELF_ROOT,
                place,
                kind: BorrowAccessKind::Mutable,
                lineage: BorrowLoanLineage::UnretainedDerived,
            },
        ));
    }
    let forming_loan = install_loan(
        &mut facts,
        LoanSpec {
            statement_index: 1,
            owner: SELF_ROOT,
            owner_path: &[],
            source_owner: CARRIER,
            root: SELF_ROOT,
            place: forming_place,
            kind: BorrowAccessKind::Mutable,
            lineage: BorrowLoanLineage::UnretainedDerived,
        },
    );

    let mut loans = arena::HandleSpan::empty();
    for handle in active_loans.iter().copied().chain([forming_loan]) {
        loans.push_contiguous(handle);
    }
    facts.borrow.states.insert(StateBorrowFact {
        machine_symbol: symbol(MACHINE),
        state_symbol: symbol(STATE),
        loans,
        ..Default::default()
    });

    let entry_constraints =
        facts
            .flow
            .contexts
            .constraint_refs
            .insert_many(active_loans.iter().copied().map(|loan| FlowConstraintRef {
                kind: FlowConstraintKind::BorrowLoan { loan },
            }));
    let statement = facts.flow.control.statements.insert(FlowStatementFact {
        statement_index: 1,
        entry_semantic_contexts: arena::HandleSpan::empty(),
        entry_constraints,
    });
    facts.flow.control.states.insert(FlowStateFact {
        machine_symbol: symbol(MACHINE),
        state_symbol: symbol(STATE),
        statements: arena::HandleSpan::from_parts(statement, 1),
        ..Default::default()
    });

    let mut handles = active_loans;
    handles.push(forming_loan);
    (facts, handles)
}

fn check(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let summaries = crate::flow::StateMutationSummaryCache::default();
    super::check_flow_call_borrows(
        program,
        facts,
        &summaries,
        None,
        &super::IncomingGuardIndex::build(program, None),
    )
}

/// One machine with one state whose body is `let p = 0; x = 0`; the
/// assignment's entry constraints carry the already-active loans installed
/// separately by `mutation_facts`.
fn mutation_program() -> typed_trees::TypedTrees {
    let mut program = typed_trees::TypedTrees::default();
    let type_reference =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated_static("u64"),
            });
    let initial_value = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
    let target = program.expression_table.insert(ExpressionNode::Name(
        typed_trees::expression::TableNamePath {
            members: arena::HandleSpan::empty(),
            member_symbols: arena::HandleSpan::empty(),
            head_symbol: symbol(X_LOCAL),
            symbol: symbol(X_LOCAL),
        },
    ));
    let mut machine = Machine {
        symbol: symbol(MACHINE),
        name: Identifier::generated_static("m"),
        ..Default::default()
    };
    let mut state = State {
        symbol: symbol(STATE),
        name: Identifier::generated_static("s"),
        return_type: type_reference,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol: symbol(TARGET_LOCAL),
            name: Identifier::generated_static("p"),
            type_reference,
            initial_value,
            is_mutable: false,
            ..Default::default()
        }),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Assignment(typed_trees::statement::TableAssignment {
            target,
            value: initial_value,
        }),
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    program
}

/// Facts for the mutation program: one active loan owned by `p` whose captured
/// place is `root.place`, live across the assignment at statement index 1.
fn mutation_facts(root: u32, place: &[u32]) -> (CheckFacts, arena::Handle<BorrowLoanFact>) {
    let mut facts = CheckFacts::default();
    let active_loan = install_loan(
        &mut facts,
        LoanSpec {
            statement_index: 0,
            owner: TARGET_LOCAL,
            owner_path: &[],
            source_owner: TARGET_LOCAL,
            root,
            place,
            kind: BorrowAccessKind::Mutable,
            lineage: BorrowLoanLineage::UnretainedDerived,
        },
    );
    let mut loans = arena::HandleSpan::empty();
    loans.push_contiguous(active_loan);
    facts.borrow.states.insert(StateBorrowFact {
        machine_symbol: symbol(MACHINE),
        state_symbol: symbol(STATE),
        loans,
        ..Default::default()
    });

    let entry_constraints = facts
        .flow
        .contexts
        .constraint_refs
        .insert_many([FlowConstraintRef {
            kind: FlowConstraintKind::BorrowLoan { loan: active_loan },
        }]);
    let statement = facts.flow.control.statements.insert(FlowStatementFact {
        statement_index: 1,
        entry_semantic_contexts: arena::HandleSpan::empty(),
        entry_constraints,
    });
    facts.flow.control.states.insert(FlowStateFact {
        machine_symbol: symbol(MACHINE),
        state_symbol: symbol(STATE),
        statements: arena::HandleSpan::from_parts(statement, 1),
        ..Default::default()
    });
    (facts, active_loan)
}

/// `&mut *d.b` while `d`'s carried `d.a`/`d.b` loans stay active. The `d.b`
/// pair overlaps exactly -- same captured place, exclusive on both sides -- so
/// compatibility alone cannot admit it; only the recorded carried-authority
/// edge does. That admission must still publish a replayable certificate.
#[test]
fn carried_authority_transfer_retains_replayable_certificates() {
    let program = statement_program();
    let (mut facts, handles) = carried_transfer_facts(&[&[X_FIELD], &[Y_FIELD]], &[Y_FIELD]);
    let active_a = handles[0];
    let active_b = handles[1];
    let forming = handles[2];

    check(&program, &mut facts).expect("carried authority transfer stays admitted");

    let certificates = facts
        .borrow
        .compatibility_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        certificates.len(),
        2,
        "every admitted forming/active loan pair retains a certificate"
    );
    let disjoint_pair = certificates
        .iter()
        .find(|certificate| {
            certificate.forming_loan == forming && certificate.active_loan == active_a
        })
        .expect("sibling carried loan pair");
    assert!(disjoint_pair.conclusion.non_interfering);
    assert!(disjoint_pair.conclusion.disjoint);

    let carried_pair = certificates
        .iter()
        .find(|certificate| {
            certificate.forming_loan == forming && certificate.active_loan == active_b
        })
        .expect("exact carried-authority pair");
    assert!(
        !carried_pair.conclusion.non_interfering,
        "the carried pair overlaps; the certificate must record the honest verdict"
    );
    assert_eq!(
        carried_pair.conclusion.containment,
        CapturedPlaceContainment::Same
    );

    // The retained rows replay through the same path on a second pass: each is
    // consumed by its exact pair and rebuilt identically.
    check(&program, &mut facts).expect("retained carried certificates replay");
    let replayed = facts
        .borrow
        .compatibility_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    assert_eq!(replayed, certificates);
}

/// `&mut *d.a` while `d`'s carried `d.b` loan covers a strictly smaller place
/// inside it. The bare owner-symbol relation used to excuse every `d`-owned
/// active loan, including this live child; containment runs the other way, so
/// no carried-authority edge exists and the pair is a real conflict.
#[test]
fn overlapping_same_owner_loan_without_transfer_edge_is_rejected() {
    let program = statement_program();
    // `d.a` covers `self.s`; `d.b` covers `self.s.y`; the forming `&mut *d.a`
    // covers `self.s`, which contains the still-active `d.b` loan.
    let (mut facts, _) = carried_transfer_facts(&[&[S_FIELD], &[S_FIELD, Y_FIELD]], &[S_FIELD]);

    let diagnostics =
        check(&program, &mut facts).expect_err("a live child loan blocks the parent reborrow");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("still active")),
        "expected an overlapping-borrow diagnostic, got {diagnostics:?}"
    );
}

/// A retained certificate claiming an interfering pair was admitted replays
/// the provenance edge, not just the spatial verdict: without a recorded
/// carried-authority edge the row cannot stand.
#[test]
fn interfering_certificate_replay_requires_carried_authority() {
    let program = statement_program();
    let mut facts = CheckFacts::default();
    let forming = install_loan(
        &mut facts,
        LoanSpec {
            statement_index: 0,
            owner: SELF_ROOT,
            owner_path: &[],
            source_owner: Y_LOCAL,
            root: SELF_ROOT,
            place: &[Z_FIELD],
            kind: BorrowAccessKind::Mutable,
            lineage: BorrowLoanLineage::UnretainedDerived,
        },
    );
    let active = install_loan(
        &mut facts,
        LoanSpec {
            statement_index: 0,
            owner: UNRELATED_LOCAL,
            owner_path: &[A_FIELD],
            source_owner: Y_LOCAL,
            root: SELF_ROOT,
            place: &[Z_FIELD],
            kind: BorrowAccessKind::Mutable,
            lineage: BorrowLoanLineage::UnretainedDerived,
        },
    );
    facts.borrow.states.insert(StateBorrowFact {
        machine_symbol: symbol(MACHINE),
        state_symbol: symbol(STATE),
        loans: arena::HandleSpan::from_parts(forming, 2),
        ..Default::default()
    });
    let place = CapturedPlace {
        root_symbol: symbol(SELF_ROOT),
        segments: vec![field(Z_FIELD)],
    };
    let certificate = CheckedBorrowCompatibilityCertificate {
        formation: BorrowCompatibilityFormation {
            machine_symbol: symbol(MACHINE),
            state_symbol: symbol(STATE),
            statement_index: 0,
        },
        forming_loan: forming,
        active_loan: active,
        forming_place: place.clone(),
        active_place: place,
        selector_snapshot: Vec::new(),
        derivation: BorrowCompatibilityDerivation::Structural,
        premises: Vec::new(),
        conclusion: BorrowCompatibilityConclusion {
            disjoint: false,
            containment: CapturedPlaceContainment::Same,
            non_interfering: false,
        },
    };

    let unrelated_source = super::replay_checked_borrow_compatibility_certificate(
        &program,
        &facts,
        &certificate,
        &super::IncomingGuardIndex::build(&program, None),
        None,
    )
    .expect_err("an interfering admission with no recorded provenance must not replay");
    assert!(
        unrelated_source.message.contains("carried authority"),
        "unexpected diagnostic: {unrelated_source:?}"
    );

    // The same pair with the forming loan's recorded source owner equal to
    // the active loan's owner does carry authority: the replayed edge matches
    // the certificate's claim.
    facts.borrow.loans.get_mut(forming).source_owner_symbol = symbol(UNRELATED_LOCAL);
    assert_eq!(
        facts.borrow.loans.get(forming).source_owner_symbol,
        facts.borrow.loans.get(active).owner_symbol
    );
    super::replay_checked_borrow_compatibility_certificate(
        &program,
        &facts,
        &certificate,
        &super::IncomingGuardIndex::build(&program, None),
        None,
    )
    .expect("a recorded carried-authority edge replays");
}

/// The provenance edge itself is narrow: it needs an exclusive active loan, a
/// replayed containment verdict placing the forming place inside it, and the
/// lineage-recorded parent (or, for unretained transfers, the rebasing owner).
#[test]
fn carried_authority_requires_the_exact_recorded_edge() {
    let mut facts = CheckFacts::default();
    let mut make_loan = |lineage: BorrowLoanLineage| {
        install_loan(
            &mut facts,
            LoanSpec {
                statement_index: 0,
                owner: CARRIER,
                owner_path: &[],
                source_owner: CARRIER,
                root: SELF_ROOT,
                place: &[Z_FIELD],
                kind: BorrowAccessKind::Mutable,
                lineage,
            },
        )
    };
    let parent = make_loan(BorrowLoanLineage::DirectRoot);
    let other = make_loan(BorrowLoanLineage::DirectRoot);
    let reborrow_child = make_loan(BorrowLoanLineage::Reborrow {
        parent_loan: parent,
    });
    let unretained_child = make_loan(BorrowLoanLineage::UnretainedDerived);
    let direct = make_loan(BorrowLoanLineage::DirectRoot);

    let parent_loan = facts.borrow.loans.get(parent);
    let carried = |forming: arena::Handle<BorrowLoanFact>,
                   containment: CapturedPlaceContainment,
                   access: &BorrowAccessKind| {
        super::statements::carried_authority(
            facts.borrow.loans.get(forming),
            parent,
            parent_loan,
            access,
            containment,
        )
    };

    // A retained reborrow rejoins only its exact recorded parent.
    assert!(carried(
        reborrow_child,
        CapturedPlaceContainment::Same,
        &BorrowAccessKind::Mutable
    ));
    assert!(!super::statements::carried_authority(
        facts.borrow.loans.get(reborrow_child),
        other,
        facts.borrow.loans.get(other),
        &BorrowAccessKind::Mutable,
        CapturedPlaceContainment::Same,
    ));

    // An unretained transfer needs the recorded source owner plus a replayed
    // containment verdict placing the forming place inside the active loan.
    assert!(carried(
        unretained_child,
        CapturedPlaceContainment::RightContainsLeft,
        &BorrowAccessKind::Mutable
    ));
    for (containment, access) in [
        (
            CapturedPlaceContainment::LeftContainsRight,
            BorrowAccessKind::Mutable,
        ),
        (CapturedPlaceContainment::None, BorrowAccessKind::Mutable),
        (CapturedPlaceContainment::Same, BorrowAccessKind::Read),
    ] {
        assert!(
            !carried(unretained_child, containment, &access),
            "no authority edge for {containment:?} under {access:?}"
        );
    }

    // A direct root never carries authority through an active loan.
    assert!(!carried(
        direct,
        CapturedPlaceContainment::Same,
        &BorrowAccessKind::Mutable
    ));
}

/// `x = 0` beside the still-active borrow of `y` is admitted because the roots
/// are disjoint. The admission retains one mutation certificate naming the
/// exact formation statement, the judged places, and the honest verdict; a
/// second pass replays the retained row and rebuilds it identically.
#[test]
fn statement_mutation_admission_retains_replayable_certificate() {
    let program = mutation_program();
    let (mut facts, active_loan) = mutation_facts(Y_LOCAL, &[]);

    check(&program, &mut facts).expect("disjoint mutation stays admitted");

    let certificates = facts
        .borrow
        .mutation_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    let [certificate] = certificates.as_slice() else {
        panic!("the admitted mutation/loan pair must retain exactly one certificate")
    };
    assert_eq!(certificate.formation.machine_symbol, symbol(MACHINE));
    assert_eq!(certificate.formation.state_symbol, symbol(STATE));
    assert_eq!(certificate.formation.statement_index, 1);
    assert_eq!(certificate.active_loan, active_loan);
    assert_eq!(
        certificate.mutated_place,
        CapturedPlace {
            root_symbol: symbol(X_LOCAL),
            segments: Vec::new(),
        }
    );
    assert_eq!(
        certificate.active_place,
        CapturedPlace {
            root_symbol: symbol(Y_LOCAL),
            segments: Vec::new(),
        }
    );
    assert!(certificate.conclusion.non_interfering);
    assert!(certificate.conclusion.disjoint);
    assert_eq!(
        certificate.derivation,
        BorrowCompatibilityDerivation::Structural
    );
    assert!(certificate.premises.is_empty());

    check(&program, &mut facts).expect("retained mutation certificate replays");
    let replayed = facts
        .borrow
        .mutation_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    assert_eq!(replayed, certificates);
}

/// `x = 0` beside a still-active borrow of `x` stays rejected: the verdict is
/// interfering and no certificate is retained.
#[test]
fn overlapping_statement_mutation_is_rejected() {
    let program = mutation_program();
    let (mut facts, _active_loan) = mutation_facts(X_LOCAL, &[]);

    let diagnostics = check(&program, &mut facts)
        .expect_err("a live borrow of the mutated place blocks the write");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("still active")),
        "expected an overlapping-borrow diagnostic, got {diagnostics:?}"
    );
    assert!(facts.borrow.mutation_certificates.is_empty());
}

/// A retained mutation certificate whose recorded write target no longer
/// re-derives from the formation statement cannot replay.
#[test]
fn drifted_mutation_certificate_is_rejected() {
    let program = mutation_program();
    let (mut facts, _active_loan) = mutation_facts(Y_LOCAL, &[]);
    check(&program, &mut facts).expect("disjoint mutation stays admitted");
    let handle = facts
        .borrow
        .mutation_certificates
        .iter()
        .map(|(handle, _)| handle)
        .next()
        .expect("one retained certificate");
    facts
        .borrow
        .mutation_certificates
        .get_mut(handle)
        .mutated_place
        .segments
        .push(field(Z_FIELD));

    let diagnostics =
        check(&program, &mut facts).expect_err("a drifted write target must not replay");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("mutated place drifted")),
        "expected a mutated-place drift diagnostic, got {diagnostics:?}"
    );
}

/// A mutation certificate only stands while its named active loan was actually
/// live at the formation statement: a row pairing the write with a loan that
/// was not in the statement's entry constraints cannot replay.
#[test]
fn mutation_certificate_replay_rejects_a_not_live_loan() {
    let program = mutation_program();
    let (mut facts, active_loan) = mutation_facts(Y_LOCAL, &[]);
    // A second, never-live loan owned by the same state.
    let dormant_loan = install_loan(
        &mut facts,
        LoanSpec {
            statement_index: 0,
            owner: TARGET_LOCAL,
            owner_path: &[],
            source_owner: TARGET_LOCAL,
            root: UNRELATED_LOCAL,
            place: &[],
            kind: BorrowAccessKind::Mutable,
            lineage: BorrowLoanLineage::UnretainedDerived,
        },
    );
    let state_handle = facts
        .borrow
        .states
        .iter()
        .map(|(handle, _)| handle)
        .next()
        .expect("borrow state");
    facts
        .borrow
        .states
        .get_mut(state_handle)
        .loans
        .push_contiguous(dormant_loan);
    let certificate = CheckedBorrowMutationCertificate {
        formation: BorrowCompatibilityFormation {
            machine_symbol: symbol(MACHINE),
            state_symbol: symbol(STATE),
            statement_index: 1,
        },
        mutated_place: CapturedPlace {
            root_symbol: symbol(X_LOCAL),
            segments: Vec::new(),
        },
        active_loan: dormant_loan,
        active_place: CapturedPlace {
            root_symbol: symbol(UNRELATED_LOCAL),
            segments: Vec::new(),
        },
        selector_snapshot: Vec::new(),
        premises: Vec::new(),
        derivation: BorrowCompatibilityDerivation::Structural,
        conclusion: BorrowCompatibilityConclusion {
            disjoint: true,
            containment: CapturedPlaceContainment::None,
            non_interfering: true,
        },
    };
    assert_ne!(dormant_loan, active_loan);
    facts.borrow.mutation_certificates.insert(certificate);

    let diagnostics = check(&program, &mut facts)
        .expect_err("a mutation certificate naming a not-live loan must not replay");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("was not live")),
        "expected a liveness diagnostic, got {diagnostics:?}"
    );
}

/// End-to-end witness: a real source program writing a local beside a live
/// borrowed view retains mutation certificates naming exact statements.
#[test]
fn real_source_mutation_retains_mutation_certificates() {
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::bump(&mut self) -> u64 {
            let view: &mut [i32] = self.items[0..4];
            let mut total: u64 = 0;
            total = 1;
            view.len
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let mut checked = crate::lower_typed_trees(typed)
        .expect("a disjoint write beside a live borrow stays admitted");

    let certificates = checked
        .facts
        .borrow
        .mutation_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    assert!(
        !certificates.is_empty(),
        "the disjoint `total = 1` write beside the live `view` borrow must retain evidence"
    );
    for certificate in &certificates {
        assert!(
            certificate.conclusion.non_interfering,
            "every retained mutation certificate records only a non-interfering verdict"
        );
    }

    // Re-checking the retained facts replays each certificate and republishes
    // the same rows.
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("retained mutation certificates replay");
    let replayed = checked
        .facts
        .borrow
        .mutation_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    assert_eq!(replayed, certificates);
}

/// A mutation certificate may only record the replayed non-interfering
/// verdict: a row claiming an actually-overlapping pair has no admission
/// basis, even when its recorded conclusion is honest.
#[test]
fn interfering_mutation_certificate_has_no_admission_basis() {
    let program = mutation_program();
    let (facts, active_loan) = mutation_facts(X_LOCAL, &[]);
    let certificate = CheckedBorrowMutationCertificate {
        formation: BorrowCompatibilityFormation {
            machine_symbol: symbol(MACHINE),
            state_symbol: symbol(STATE),
            statement_index: 1,
        },
        mutated_place: CapturedPlace {
            root_symbol: symbol(X_LOCAL),
            segments: Vec::new(),
        },
        active_loan,
        active_place: CapturedPlace {
            root_symbol: symbol(X_LOCAL),
            segments: Vec::new(),
        },
        selector_snapshot: Vec::new(),
        premises: Vec::new(),
        derivation: BorrowCompatibilityDerivation::Structural,
        conclusion: BorrowCompatibilityConclusion {
            disjoint: false,
            containment: CapturedPlaceContainment::Same,
            non_interfering: false,
        },
    };

    let diagnostic = super::replay_checked_borrow_mutation_certificate(
        &program,
        &facts,
        &certificate,
        &super::IncomingGuardIndex::build(&program, None),
        None,
    )
    .expect_err("an interfering mutation verdict has no admission basis");
    assert!(
        diagnostic.message.contains("no admission basis"),
        "unexpected diagnostic: {diagnostic:?}"
    );
}
