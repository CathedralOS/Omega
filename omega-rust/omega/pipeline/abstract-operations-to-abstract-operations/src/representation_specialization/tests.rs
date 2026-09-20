//! Optimizer module role: test leaf. Established-case membership specialization proposal, replay, and custody evidence.

use super::super::VerifiedPsiOptimizationSession;
use crate::{
    CaseMembershipSpecializationCandidate, CaseMembershipSpecializationError,
    apply_case_membership_specialization, propose_case_membership_specializations,
    validate_case_membership_specialization,
};
use abstract_operations::AbstractOperation;
use optimization_unit::{
    NodeLocation, ProvenanceDisposition, PsiOptimizationUnit, PsiProvenance, PsiRealizationSite,
    recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::OptimizationUnitValidationError;
use semantic_vocabulary::{MachineId, PlaceId, StructuralPlaceKind};

/// A scalar machine establishes `Choice::Some` once and observes membership
/// in that same case: the observation folds to `true`.
const ESTABLISHED_MATCHING_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    machine probe() -> bool {
        let c: Choice = Choice::Some { value: 37 };
        c in Choice::Some
    }
"#;

/// The same established `Some` place observed against `Empty`: the
/// membership folds to `false` — a non-matching verdict specializes too.
const ESTABLISHED_OTHER_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    machine probe() -> bool {
        let c: Choice = Choice::Some { value: 37 };
        c in Choice::Empty
    }
"#;

/// Two memberships observe the same established place: both fold in one
/// candidate, one `true` and one `false`.
const TWO_MEMBERSHIPS_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    machine probe() -> bool {
        let c: Choice = Choice::Some { value: 37 };
        (c in Choice::Some) == (c in Choice::Empty)
    }
"#;

/// A membership on a machine parameter place over a multi-case roster
/// carries no establishment proof and no sole-case roster: the parameter
/// arrives with whatever case the caller supplied, so nothing may
/// specialize.
const PARAMETER_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    machine probe(c: Choice) -> bool {
        c in Choice::Some
    }
"#;

/// A membership on a machine parameter place whose declared sum roster holds
/// exactly one case: the roster alone proves the case regardless of caller,
/// so the observation folds to `true`.
const SOLE_CASE_PARAMETER_SOURCE: &str = r#"
    data Tag { case Only; }
    machine probe(t: Tag) -> bool {
        t in Tag::Only
    }
"#;

/// A locally established single-case sum still folds through the
/// establishment proof: the `EstablishScalarCase` producer is the basis, not
/// the roster.
const SOLE_CASE_LOCAL_SOURCE: &str = r#"
    data Tag { case Only; }
    machine probe() -> bool {
        let t: Tag = Tag::Only;
        t in Tag::Only
    }
"#;

/// A membership descending into a parameter's sole-case record field: the
/// nested position's closed roster proves the verdict even though the
/// record root itself has no cases and no producer.
const PATH_FIELD_SOURCE: &str = r#"
    data Tag { case Only; }
    data Rec { inner: Tag; }
    machine probe(r: Rec) -> bool {
        r.inner in Tag::Only
    }
"#;

/// A two-segment path resolves through nested records to the same sole-case
/// roster: the end type proves the verdict regardless of depth.
const NESTED_PATH_SOURCE: &str = r#"
    data Tag { case Only; }
    data Mid { inner: Tag; }
    data Out { mid: Mid; }
    machine probe(r: Out) -> bool {
        r.mid.inner in Tag::Only
    }
"#;

/// A path descending into a multi-case field carries no roster proof: the
/// membership observes a position whose case the unit cannot fix.
const PATH_MULTI_CASE_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    data Rec { inner: Choice; }
    machine probe(r: Rec) -> bool {
        r.inner in Choice::Some
    }
"#;

/// One parameter place observed at two path positions: the sole-case field
/// folds while the multi-case field's membership stays an observation.
const PATH_SPLIT_SOURCE: &str = r#"
    data Tag { case Only; }
    data Choice { case Empty; case Some(value: u32); }
    data Duo { a: Tag; b: Choice; }
    machine probe(r: Duo) -> bool {
        (r.a in Tag::Only) == (r.b in Choice::Some)
    }
"#;

/// No membership observes the established place at all: no candidate exists.
const NO_MEMBERSHIP_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    machine probe() -> u32 {
        let c: Choice = Choice::Some { value: 37 };
        3
    }
"#;

#[test]
fn established_case_membership_folds_to_proven_verdict() {
    let session =
        lowered_session_entry(ESTABLISHED_MATCHING_SOURCE, "matching membership", "probe");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let (place, producer, established) = established_place(&unit, machine);
    let (site, membership) = membership_on(&unit, machine, place).expect("membership exists");

    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    assert_eq!(candidate.machine(), machine);
    assert_eq!(candidate.place(), place);
    assert_eq!(candidate.producer(), Some(producer));
    assert_eq!(candidate.input(), unit.identity);
    assert_ne!(candidate.output(), unit.identity);
    let [row] = candidate.memberships() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site(), site);
    assert_eq!(row.psi_operation(), membership.0);
    assert_eq!(row.result(), membership.1);
    assert_eq!(row.source(), place);
    assert_eq!(row.producer(), Some(producer));
    assert_eq!(row.observed_case(), membership.2);
    assert_eq!(row.proven_case(), established);
    assert!(row.outcome());

    // The proposal is deterministic and the folded site identity is bound
    // into the candidate identity.
    let replayed = propose_case_membership_specializations(&session, 4).expect("replay runs");
    assert_eq!(replayed, candidates);

    let validated =
        validate_case_membership_specialization(&session, candidate).expect("independent replay");
    let applied = apply_case_membership_specialization(session, validated).expect("apply");
    let next = applied.session();
    let output_function = next
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");

    // The folded node is a BooleanConstant at the same site, keeping the
    // membership's custody identity, result value, and fuel settlement.
    let folded = &output_function
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    let AbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value,
    } = &folded.operation
    else {
        panic!("membership folds to BooleanConstant")
    };
    assert_eq!(*psi_operation, membership.0);
    assert_eq!(*result, membership.1);
    assert!(*value);
    assert_eq!(
        folded.provenance,
        vec![PsiProvenance::Operation(membership.0)]
    );
    assert_eq!(
        folded.fuel,
        vec![optimization_unit::FuelSettlement {
            site: PsiProvenance::Operation(membership.0),
            units: 1,
        }]
    );
    let input_node = &unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("input machine")
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("input block")
        .nodes[usize::try_from(site.node).expect("index")];
    assert_eq!(folded.definitions, input_node.definitions);
    assert_eq!(folded.uses, input_node.uses);
    assert_eq!(folded.successors, input_node.successors);

    // The ledger records the folded site's retained custody: the membership's
    // operation provenance realized at the same node.
    let [record] = applied.ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(record.input, unit.identity);
    assert_eq!(record.output, next.unit().identity);
    assert_eq!(
        record.provenance,
        vec![optimization_unit::ProvenanceRewrite {
            input: PsiRealizationSite::Node(site),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(site)),
            sources: vec![PsiProvenance::Operation(membership.0)],
            fuel: vec![optimization_unit::FuelSettlement {
                site: PsiProvenance::Operation(membership.0),
                units: 1,
            }],
        }]
    );

    // The applied session is an exact fixed point for this family.
    assert!(
        propose_case_membership_specializations(applied.session(), 4)
            .expect("fixed-point proposal runs")
            .is_empty(),
        "the specialization reaches a fixed point"
    );
}

#[test]
fn established_other_case_membership_folds_to_false() {
    let session = lowered_session_entry(ESTABLISHED_OTHER_SOURCE, "other-case membership", "probe");
    let unit = session.unit();
    let machine = unit.functions[0].machine;
    let (_place, _, _) = established_place(unit, machine);

    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };
    let [row] = candidate.memberships() else {
        panic!("one folded membership")
    };
    assert!(!row.outcome(), "the wrong-case membership proves false");

    let validated =
        validate_case_membership_specialization(&session, candidate).expect("independent replay");
    let applied = apply_case_membership_specialization(session, validated).expect("apply");
    let function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    let folded = &function
        .blocks
        .iter()
        .find(|block| block.id == row.site().block)
        .expect("block retained")
        .nodes[usize::try_from(row.site().node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: false, .. }
    ));
}

#[test]
fn two_memberships_on_one_place_fold_together() {
    let session = lowered_session_entry(TWO_MEMBERSHIPS_SOURCE, "two memberships", "probe");
    let machine = session.unit().functions[0].machine;
    let (place, _, _) = established_place(session.unit(), machine);

    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one candidate covering both memberships")
    };
    assert_eq!(candidate.place(), place);
    let [first, second] = candidate.memberships() else {
        panic!("two folded memberships")
    };
    assert_ne!(first.outcome(), second.outcome());
    assert_ne!(first.observed_case(), second.observed_case());

    let validated =
        validate_case_membership_specialization(&session, candidate).expect("independent replay");
    let applied = apply_case_membership_specialization(session, validated).expect("apply");
    let function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    for row in [first, second] {
        let folded = &function
            .blocks
            .iter()
            .find(|block| block.id == row.site().block)
            .expect("block retained")
            .nodes[usize::try_from(row.site().node).expect("index")];
        let AbstractOperation::BooleanConstant { value, .. } = folded.operation else {
            panic!("membership folds to BooleanConstant")
        };
        assert_eq!(value, row.outcome());
    }
    let [record] = applied.ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(
        record.provenance.len(),
        2,
        "one custody row per folded site"
    );
}

#[test]
fn parameter_membership_yields_no_candidate() {
    let session = lowered_session_entry(PARAMETER_SOURCE, "parameter decline", "probe");
    assert!(
        propose_case_membership_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty()
    );
}

#[test]
fn unobserved_establishment_yields_no_candidate() {
    let session = lowered_session_entry(NO_MEMBERSHIP_SOURCE, "no-membership decline", "probe");
    assert!(
        propose_case_membership_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty()
    );
    let unit = session.unit();
    let machine = unit.functions[0].machine;
    let (place, producer, _) = established_place(unit, machine);
    let candidate = CaseMembershipSpecializationCandidate {
        identity: optimization_core::OptimizationCandidateIdentity::from_canonical_bytes(
            b"forged-unobserved-candidate",
        ),
        input: unit.identity,
        output: unit.identity,
        machine,
        place,
        producer: Some(producer),
        memberships: Vec::new(),
    };
    assert_eq!(
        validate_case_membership_specialization(&session, &candidate).err(),
        Some(CaseMembershipSpecializationError::AlreadySpecialized)
    );
}

#[test]
fn replay_rejects_forged_membership_rows() {
    let session =
        lowered_session_entry(ESTABLISHED_MATCHING_SOURCE, "matching membership", "probe");
    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };

    // A forged verdict.
    let mut forged = candidate.clone();
    forged.memberships[0].outcome = !forged.memberships[0].outcome;
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );

    // A forged observed case — a case identity no source operation carries.
    let mut forged = candidate.clone();
    forged.memberships[0].observed_case =
        semantic_vocabulary::StructuralCaseId::new(forged.memberships[0].observed_case.get() + 7)
            .expect("forged case identity");
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );

    // A forged site coordinate.
    let mut forged = candidate.clone();
    forged.memberships[0].site.node += 1;
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );

    // A forged producer identity.
    let mut forged = candidate.clone();
    forged.producer = Some(forged.memberships[0].psi_operation);
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );

    // A forged candidate identity.
    let mut forged = candidate.clone();
    forged.identity =
        optimization_core::OptimizationCandidateIdentity::from_canonical_bytes(b"forged-identity");
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );

    // A forged output revision.
    let mut forged = candidate.clone();
    forged.output =
        optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"forged-output");
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );

    // The untampered candidate still validates.
    assert!(
        validate_case_membership_specialization(&session, candidate).is_ok(),
        "the exact candidate still validates"
    );
}

#[test]
fn replay_rejects_stale_candidate_revision() {
    let session =
        lowered_session_entry(ESTABLISHED_MATCHING_SOURCE, "matching membership", "probe");
    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };

    let mut stale = candidate.clone();
    stale.input = optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"stale-input");
    assert_eq!(
        validate_case_membership_specialization(&session, &stale).err(),
        Some(CaseMembershipSpecializationError::StaleCandidateRevision {
            candidate: stale.input,
            current: session.unit().identity,
        })
    );

    // Applying moves the revision; the original candidate is stale afterward.
    let validated =
        validate_case_membership_specialization(&session, candidate).expect("independent replay");
    let applied = apply_case_membership_specialization(session, validated).expect("apply");
    assert_eq!(
        validate_case_membership_specialization(applied.session(), candidate).err(),
        Some(CaseMembershipSpecializationError::StaleCandidateRevision {
            candidate: candidate.input(),
            current: applied.session().unit().identity,
        })
    );
}

#[test]
fn candidate_budget_is_exact() {
    let session =
        lowered_session_entry(ESTABLISHED_MATCHING_SOURCE, "matching membership", "probe");
    assert_eq!(
        propose_case_membership_specializations(&session, 0).err(),
        Some(
            CaseMembershipSpecializationError::CandidateBudgetExhausted {
                required: 1,
                limit: 0,
            }
        )
    );
    assert_eq!(
        propose_case_membership_specializations(&session, 1)
            .expect("proposal runs")
            .len(),
        1
    );
}

#[test]
fn transformed_replay_rejects_forged_folded_custody() {
    let session =
        lowered_session_entry(ESTABLISHED_MATCHING_SOURCE, "matching membership", "probe");
    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };
    let row = &candidate.memberships()[0];
    let machine = candidate.machine();
    let verified_input = session.input().clone();
    let validated =
        validate_case_membership_specialization(&session, candidate).expect("independent replay");

    // A unit whose folded node claims a different custody source is not the
    // specialization this candidate pins: replay rebuilds the plan's own
    // output and the forged revision identity mismatches.
    let mut corrupted = validated.output.clone();
    let folded = corrupted
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                node.operation,
                AbstractOperation::BooleanConstant { psi_operation, .. }
                    if psi_operation == row.psi_operation()
            )
        })
        .expect("folded node exists");
    assert_eq!(
        folded.provenance,
        vec![PsiProvenance::Operation(row.psi_operation())]
    );
    folded.provenance[0] = PsiProvenance::Operation(row.producer().expect("establishment basis"));
    folded.fuel[0].site = PsiProvenance::Operation(row.producer().expect("establishment basis"));
    corrupted.identity = recompute_psi_optimization_unit_identity(&corrupted);
    let mut forged = candidate.clone();
    forged.output = corrupted.identity;
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );

    // Dropping the folded node's fuel settlement while keeping its custody
    // claim leaves a unit whose node settles fewer sources than it names:
    // transformed validation rejects the forged fuel/provenance pair.
    let mut malformed = validated.output.clone();
    malformed
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                node.operation,
                AbstractOperation::BooleanConstant { psi_operation, .. }
                    if psi_operation == row.psi_operation()
            )
        })
        .expect("folded node exists")
        .fuel
        .pop();
    malformed.identity = recompute_psi_optimization_unit_identity(&malformed);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(verified_input, malformed),
        Err(
            OptimizationUnitValidationError::FuelDoesNotMatchProvenance { machine: rejected, .. }
        ) if rejected == machine
    ));

    let applied = apply_case_membership_specialization(session, validated).expect("apply");
    assert!(
        VerifiedPsiOptimizationSession::from_transformed(
            applied.session().input().clone(),
            applied.session().unit().clone(),
        )
        .is_ok(),
        "the applied folded revision revalidates independently"
    );
}

#[test]
fn sole_case_parameter_membership_folds_without_producer() {
    let session = lowered_session_entry(SOLE_CASE_PARAMETER_SOURCE, "sole-case parameter", "probe");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let function = &unit.functions[0];
    let place = function
        .structural_places
        .iter()
        .find(|declaration| matches!(declaration.kind, StructuralPlaceKind::Parameter { .. }))
        .expect("parameter place exists")
        .id;
    let (site, membership) = membership_on(&unit, machine, place).expect("membership exists");

    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    assert_eq!(candidate.machine(), machine);
    assert_eq!(candidate.place(), place);
    assert_eq!(candidate.producer(), None);
    let [row] = candidate.memberships() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site(), site);
    assert_eq!(row.psi_operation(), membership.0);
    assert_eq!(row.source(), place);
    assert_eq!(row.producer(), None);
    assert_eq!(row.observed_case(), row.proven_case());
    assert!(row.outcome());

    let validated =
        validate_case_membership_specialization(&session, candidate).expect("independent replay");
    let applied = apply_case_membership_specialization(session, validated).expect("apply");
    let folded = &applied.session().unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
    assert!(
        propose_case_membership_specializations(applied.session(), 4)
            .expect("fixed-point proposal runs")
            .is_empty(),
        "the specialization reaches a fixed point"
    );
}

#[test]
fn sole_case_local_keeps_establishment_basis() {
    let session = lowered_session_entry(SOLE_CASE_LOCAL_SOURCE, "sole-case local", "probe");
    let unit = session.unit();
    let machine = unit.functions[0].machine;
    let (place, producer, _) = established_place(unit, machine);

    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };
    assert_eq!(candidate.producer(), Some(producer));
    let [row] = candidate.memberships() else {
        panic!("one folded membership")
    };
    assert_eq!(row.source(), place);
    assert!(row.outcome());
}

#[test]
fn replay_rejects_forged_roster_rows() {
    let session = lowered_session_entry(SOLE_CASE_PARAMETER_SOURCE, "sole-case parameter", "probe");
    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };

    // Claiming a producer on a roster-proven row mismatches the replayed plan.
    let mut forged = candidate.clone();
    forged.producer = Some(forged.memberships[0].psi_operation);
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );
    let mut forged = candidate.clone();
    forged.memberships[0].producer = Some(forged.memberships[0].psi_operation);
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );

    // A forged proven case on the roster basis mismatches too.
    let mut forged = candidate.clone();
    forged.memberships[0].proven_case =
        semantic_vocabulary::StructuralCaseId::new(forged.memberships[0].proven_case.get() + 7)
            .expect("forged case identity");
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );

    assert!(
        validate_case_membership_specialization(&session, candidate).is_ok(),
        "the exact roster candidate still validates"
    );
}

#[test]
fn path_field_membership_folds_on_sole_case_end() {
    let session = lowered_session_entry(PATH_FIELD_SOURCE, "path-field membership", "probe");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let function = &unit.functions[0];
    let place = function
        .structural_places
        .iter()
        .find(|declaration| matches!(declaration.kind, StructuralPlaceKind::Parameter { .. }))
        .expect("parameter place exists")
        .id;
    let (site, membership) = membership_on(&unit, machine, place).expect("membership exists");

    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    assert_eq!(candidate.machine(), machine);
    assert_eq!(candidate.place(), place);
    assert_eq!(candidate.producer(), None);
    let [row] = candidate.memberships() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site(), site);
    assert_eq!(row.psi_operation(), membership.0);
    assert_eq!(row.source(), place);
    assert_eq!(row.producer(), None);
    assert_eq!(row.observed_case(), row.proven_case());
    assert!(row.outcome());

    let validated =
        validate_case_membership_specialization(&session, candidate).expect("independent replay");
    let applied = apply_case_membership_specialization(session, validated).expect("apply");
    let folded = &applied.session().unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
    assert!(
        propose_case_membership_specializations(applied.session(), 4)
            .expect("fixed-point proposal runs")
            .is_empty(),
        "the specialization reaches a fixed point"
    );
}

#[test]
fn nested_path_membership_folds_through_records() {
    let session = lowered_session_entry(NESTED_PATH_SOURCE, "nested-path membership", "probe");

    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    let [row] = candidate.memberships() else {
        panic!("one folded membership")
    };
    assert!(row.outcome());

    let validated =
        validate_case_membership_specialization(&session, candidate).expect("independent replay");
    let applied = apply_case_membership_specialization(session, validated).expect("apply");
    let folded = &applied.session().unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == row.site().block)
        .expect("block retained")
        .nodes[usize::try_from(row.site().node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
}

#[test]
fn multi_case_path_membership_yields_no_candidate() {
    let session = lowered_session_entry(PATH_MULTI_CASE_SOURCE, "multi-case path", "probe");
    assert!(
        propose_case_membership_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty()
    );
}

#[test]
fn split_path_memberships_fold_only_the_proven_position() {
    let session = lowered_session_entry(PATH_SPLIT_SOURCE, "split-path memberships", "probe");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let function = &unit.functions[0];
    let place = function
        .structural_places
        .iter()
        .find(|declaration| matches!(declaration.kind, StructuralPlaceKind::Parameter { .. }))
        .expect("parameter place exists")
        .id;
    let memberships = memberships_on(&unit, machine, place);
    let find = |field: &str| {
        memberships
            .iter()
            .find(|(_, _, path)| {
                matches!(
                    path.as_slice(),
                    [terminal_psi::StructuralPathSegment::Field(name)] if name == field
                )
            })
            .map(|(site, membership, _)| (*site, *membership))
            .expect("membership at the named field exists")
    };
    let (_, (folded_op, _, _)) = find("a");
    let (unproven_site, (unproven_op, _, unproven_case)) = find("b");

    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    let [row] = candidate.memberships() else {
        panic!("one folded membership")
    };
    assert_eq!(row.psi_operation(), folded_op);
    assert_ne!(row.psi_operation(), unproven_op);
    assert!(row.outcome());

    let validated =
        validate_case_membership_specialization(&session, candidate).expect("independent replay");
    let applied = apply_case_membership_specialization(session, validated).expect("apply");
    let function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    let unproven_node = &function
        .blocks
        .iter()
        .find(|block| block.id == unproven_site.block)
        .expect("block retained")
        .nodes[usize::try_from(unproven_site.node).expect("index")];
    // The unproven membership stays an observation over the same case.
    assert!(matches!(
        &unproven_node.operation,
        AbstractOperation::StructuralCaseMembership {
            psi_operation,
            case,
            ..
        } if *psi_operation == unproven_op && *case == unproven_case
    ));
}

#[test]
fn replay_rejects_forged_path_rows() {
    let session = lowered_session_entry(PATH_FIELD_SOURCE, "path-field membership", "probe");
    let candidates = propose_case_membership_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };

    // A forged producer on a path-proven row claims an establishment basis
    // that cannot prove a nested position.
    let mut forged = candidate.clone();
    forged.memberships[0].producer = Some(forged.memberships[0].psi_operation);
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );

    // A forged proven case at the resolved position mismatches the replayed
    // roster.
    let mut forged = candidate.clone();
    forged.memberships[0].proven_case =
        semantic_vocabulary::StructuralCaseId::new(forged.memberships[0].proven_case.get() + 7)
            .expect("forged case identity");
    assert_eq!(
        validate_case_membership_specialization(&session, &forged).err(),
        Some(CaseMembershipSpecializationError::CandidateMismatch)
    );

    assert!(
        validate_case_membership_specialization(&session, candidate).is_ok(),
        "the exact path candidate still validates"
    );
}

/// The only `OperationResult` place in these fixtures, its
/// `EstablishScalarCase` producer, and the fixed case.
fn established_place(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
) -> (
    PlaceId,
    semantic_vocabulary::OperationId,
    semantic_vocabulary::StructuralCaseId,
) {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let (place, producer) = function
        .structural_places
        .iter()
        .find_map(|declaration| match declaration.kind {
            StructuralPlaceKind::OperationResult { producer, .. } => {
                Some((declaration.id, producer))
            }
            _ => None,
        })
        .expect("established place exists");
    let established = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::EstablishScalarCase {
                psi_operation,
                result_case,
                ..
            } if *psi_operation == producer => Some(*result_case),
            _ => None,
        })
        .expect("producer is an established case");
    (place, producer, established)
}

/// The single `StructuralCaseMembership` site observing `place`, returning
/// its node location plus (custody identity, result value, observed case).
fn membership_on(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    place: PlaceId,
) -> Option<(
    NodeLocation,
    (
        semantic_vocabulary::OperationId,
        semantic_vocabulary::ValueId,
        semantic_vocabulary::StructuralCaseId,
    ),
)> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)?;
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let AbstractOperation::StructuralCaseMembership {
                psi_operation,
                result,
                source,
                case,
                ..
            } = &node.operation
            else {
                continue;
            };
            if *source == place {
                return Some((
                    NodeLocation {
                        machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("node index fits u32"),
                    },
                    (*psi_operation, result.value, *case),
                ));
            }
        }
    }
    None
}

/// Every `StructuralCaseMembership` observing `place`, in node order — its
/// node location, (custody identity, result value, observed case), and path.
fn memberships_on(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    place: PlaceId,
) -> Vec<(
    NodeLocation,
    (
        semantic_vocabulary::OperationId,
        semantic_vocabulary::ValueId,
        semantic_vocabulary::StructuralCaseId,
    ),
    Vec<terminal_psi::StructuralPathSegment>,
)> {
    let mut memberships = Vec::new();
    let Some(function) = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
    else {
        return memberships;
    };
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let AbstractOperation::StructuralCaseMembership {
                psi_operation,
                result,
                source,
                path,
                case,
            } = &node.operation
            else {
                continue;
            };
            if *source == place {
                memberships.push((
                    NodeLocation {
                        machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("node index fits u32"),
                    },
                    (*psi_operation, result.value, *case),
                    path.clone(),
                ));
            }
        }
    }
    memberships
}

fn lowered_session_entry(source: &str, label: &str, entry: &str) -> VerifiedPsiOptimizationSession {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|error| panic!("tokenize {label}: {error:?}"));
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .unwrap_or_else(|error| panic!("parse {label}: {error:?}"));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap_or_else(|error| panic!("resolve {label}: {error:?}"));
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|error| panic!("type {label}: {error:?}"));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|error| panic!("check {label}: {error:?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
        .unwrap_or_else(|error| panic!("lower {label}: {error:?}"));
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&lowered.semantic_module)
                .unwrap_or_else(|error| panic!("encode {label} semantics: {error:?}")),
            proof_bytes: &terminal_codec::encode_proof_section(
                &lowered.semantic_module,
                &lowered.proof_bundle,
            )
            .unwrap_or_else(|error| panic!("encode {label} proof: {error:?}")),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap_or_else(|error| panic!("optimizer-only {label} admission: {error:?}"));
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap_or_else(|error| panic!("build {label} optimizer unit: {error:?}"));
    VerifiedPsiOptimizationSession::new(verified)
        .unwrap_or_else(|error| panic!("verified {label} session: {error:?}"))
}
