//! Optimizer module role: test leaf. Established-case membership specialization admission, realization, and custody evidence through the rule.
//!
//! Every fixture is driven through the one live route: the
//! `RepresentationSpecialization` pass runs `CaseMembershipSpecializationRule`
//! to its fixed point, the committed candidate carries the proposed plan,
//! the run's session carries the folded unit, and forged rows are replayed
//! against `validate_case_membership_specialization_candidate` — the
//! independent validator the pass manager itself consults.

use crate::rules::CaseMembershipSpecializationRule;
use crate::{
    OptimizationRun, PsiOptimizationCommit, VerifiedPsiOptimizationSession, run_psi_pipeline,
};
use abstract_operations::AbstractOperation;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use optimization_core::{
    Optimization, OptimizationSelections, OptimizationUnitIdentity, OptimizationWorkBudget,
};
use optimization_unit::{
    CaseMembershipSpecializationRewrite, NodeLocation, ProvenanceDisposition, PsiOptimizationUnit,
    PsiProvenance, PsiRealizationSite, PsiRewriteCandidate, PsiRewriteCandidateError,
    PsiRewritePatch, recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_case_membership_specialization_candidate,
};
use semantic_vocabulary::{MachineId, PlaceId, StructuralPlaceKind};
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;

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

/// A membership descending into a parameter's array element: the fixed
/// index resolves to the element's declared sole-case roster, which proves
/// the verdict regardless of which element the index names.
const ARRAY_ELEMENT_SOURCE: &str = r#"
    data Tag { case Only; }
    machine probe(t: [Tag; 2]) -> bool {
        t[1] in Tag::Only
    }
"#;

/// A two-segment path descends through a parameter's record field into an
/// array element: the same sole-case roster at the resolved position proves
/// the verdict.
const NESTED_ARRAY_SOURCE: &str = r#"
    data Tag { case Only; }
    data Rec { inner: [Tag; 2]; }
    machine probe(r: Rec) -> bool {
        r.inner[0] in Tag::Only
    }
"#;

/// A three-segment path crosses an array element mid-path: the index lands
/// on a record element whose own field holds the sole-case sum — the roster
/// at the resolved end proves the verdict at every depth.
const DEEP_ARRAY_PATH_SOURCE: &str = r#"
    data Tag { case Only; }
    data Inner { tag: Tag; }
    data Mid { arr: [Inner; 2]; }
    machine probe(m: Mid) -> bool {
        m.arr[0].tag in Tag::Only
    }
"#;

/// A fixed index into a machine receiver's own array field: the receiver is
/// the machine's `is_self` parameter place and the same roster proof folds
/// the observation.
const SELF_ARRAY_SOURCE: &str = r#"
    data Tag { case Only; }
    data Root { arr: [Tag; 3]; }
    machine Root::run(&mut self) {
        transition self.arr[1] in Tag::Only { true -> good() _ -> bad() }
        state good(&mut self) {}
        state bad(&mut self) {}
    }
"#;

/// A path descending into a multi-case array element carries no roster
/// proof: the membership observes a position whose case the unit cannot
/// fix.
const ARRAY_MULTI_CASE_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    machine probe(t: [Choice; 2]) -> bool {
        t[1] in Choice::Some
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
    let unit = lowered_unit_entry(ESTABLISHED_MATCHING_SOURCE, "matching membership", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (place, producer, established) = established_place(&input, machine);
    let (site, membership) = membership_on(&input, machine, place).expect("membership exists");

    let run = specialize(unit);
    let (commit, patch) = single_commit(&run);
    assert_eq!(patch.machine, machine);
    assert_eq!(patch.place, place);
    assert_eq!(patch.producer, Some(producer));
    assert_eq!(commit.input, input.identity);
    assert_ne!(commit.output, input.identity);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site, site);
    assert_eq!(row.psi_operation, membership.0);
    assert_eq!(row.result, membership.1);
    assert_eq!(row.source, place);
    assert_eq!(row.producer, Some(producer));
    assert_eq!(row.observed_case, membership.2);
    assert_eq!(row.proven_case, established);
    assert!(row.outcome);

    // The proposal is deterministic: an independent run commits the same
    // candidate, custody, and output revision.
    let replayed = specialize(lowered_unit_entry(
        ESTABLISHED_MATCHING_SOURCE,
        "matching membership",
        "probe",
    ));
    assert_eq!(replayed.commits(), run.commits());

    let output = run.session().unit();
    let output_function = output
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
    let input_node = &input
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
    // operation provenance realized at the same node — exactly the custody
    // the validator accepted for the commit.
    let [record] = run.transformation_ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(record.input, input.identity);
    assert_eq!(record.output, output.identity);
    assert_eq!(record.provenance, commit.provenance);
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

    // The single commit is the pass's fixed point: no membership on the
    // place survives to draw a second candidate.
    assert!(membership_on(output, machine, place).is_none());
}

#[test]
fn established_other_case_membership_folds_to_false() {
    let unit = lowered_unit_entry(ESTABLISHED_OTHER_SOURCE, "other-case membership", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (_place, _, _) = established_place(&input, machine);

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert!(!row.outcome, "the wrong-case membership proves false");

    let function = run
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    let folded = &function
        .blocks
        .iter()
        .find(|block| block.id == row.site.block)
        .expect("block retained")
        .nodes[usize::try_from(row.site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: false, .. }
    ));
}

#[test]
fn two_memberships_on_one_place_fold_together() {
    let unit = lowered_unit_entry(TWO_MEMBERSHIPS_SOURCE, "two memberships", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (place, _, _) = established_place(&input, machine);

    let run = specialize(unit);
    let (commit, patch) = single_commit(&run);
    assert_eq!(patch.place, place);
    let [first, second] = patch.memberships.as_slice() else {
        panic!("two folded memberships")
    };
    assert_ne!(first.outcome, second.outcome);
    assert_ne!(first.observed_case, second.observed_case);

    let function = run
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
            .find(|block| block.id == row.site.block)
            .expect("block retained")
            .nodes[usize::try_from(row.site.node).expect("index")];
        let AbstractOperation::BooleanConstant { value, .. } = folded.operation else {
            panic!("membership folds to BooleanConstant")
        };
        assert_eq!(value, row.outcome);
    }
    let [record] = run.transformation_ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(
        record.provenance.len(),
        2,
        "one custody row per folded site"
    );
    assert_eq!(record.provenance, commit.provenance);
}

#[test]
fn parameter_membership_yields_no_candidate() {
    assert_declines(lowered_unit_entry(
        PARAMETER_SOURCE,
        "parameter decline",
        "probe",
    ));
}

#[test]
fn unobserved_establishment_yields_no_candidate() {
    let unit = lowered_unit_entry(NO_MEMBERSHIP_SOURCE, "no-membership decline", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (place, producer, _) = established_place(&input, machine);
    // The place is proven — its plan carries the establishment witness — but
    // no observation reads it, so the plan is empty and the rule proposes
    // nothing.
    let plan = super::propose::plan(&input, &input.functions[0], place).expect("proven place");
    assert_eq!(plan.producer, Some(producer));
    assert!(plan.memberships.is_empty());
    // A candidate claiming the empty plan cannot even be constructed: a
    // membership patch must fold at least one observation.
    assert!(
        PsiRewriteCandidate::new_case_membership_specialization(
            input.identity,
            CaseMembershipSpecializationRule::contract(),
            Vec::new(),
            Vec::new(),
            0,
            plan,
        )
        .is_err()
    );
    assert_declines(unit);
}

#[test]
fn replay_rejects_forged_membership_rows() {
    let unit = lowered_unit_entry(ESTABLISHED_MATCHING_SOURCE, "matching membership", "probe");
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _) = single_commit(&run);

    // A forged verdict.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].outcome = !patch.memberships[0].outcome;
        }),
    );

    // A forged observed case — a case identity no source operation carries.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].observed_case = semantic_vocabulary::StructuralCaseId::new(
                patch.memberships[0].observed_case.get() + 7,
            )
            .expect("forged case identity");
        }),
    );

    // A forged site coordinate.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].site.node += 1;
        }),
    );

    // A forged producer identity.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.producer = Some(patch.memberships[0].psi_operation);
        }),
    );

    // The untampered declaration still validates to the committed output.
    let validated = validate_case_membership_specialization_candidate(&input, &commit.declaration)
        .expect("the exact candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
}

#[test]
fn replay_rejects_stale_candidate_revision() {
    let unit = lowered_unit_entry(ESTABLISHED_MATCHING_SOURCE, "matching membership", "probe");
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _) = single_commit(&run);

    // A candidate pinned to a revision that is not the input.
    let stale = rebuild(
        &commit.declaration,
        OptimizationUnitIdentity::from_canonical_bytes(b"stale-input"),
        |_| {},
    )
    .expect("a stale input identity is still a well-formed declaration");
    assert_eq!(
        validate_case_membership_specialization_candidate(&input, &stale).err(),
        Some(OptimizationUnitValidationError::CandidateInputMismatch)
    );

    // Committing moves the revision; the original declaration is stale
    // against the transformed unit afterward.
    assert_eq!(
        validate_case_membership_specialization_candidate(
            run.session().unit(),
            &commit.declaration
        )
        .err(),
        Some(OptimizationUnitValidationError::CandidateInputMismatch)
    );
}

/// The committed unit revalidates independently, while a unit whose folded
/// node drops its fuel settlement — settling fewer sources than the custody
/// it names — is refused by transformed validation. A forged commit output
/// identity is refused by publication replay in
/// `pass_manager::tests::evidence_matrix::representation_specialization::forged_run_axes_fail_publication_replay`.
#[test]
fn transformed_unit_rejects_forged_folded_custody() {
    let unit = lowered_unit_entry(ESTABLISHED_MATCHING_SOURCE, "matching membership", "probe");
    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let row = &patch.memberships[0];
    let machine = patch.machine;
    let verified_input = run.session().input().clone();

    assert!(
        VerifiedPsiOptimizationSession::from_transformed(
            verified_input.clone(),
            run.session().unit().clone(),
        )
        .is_ok(),
        "the committed folded revision revalidates independently"
    );

    let mut malformed = run.session().unit().clone();
    let folded = malformed
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                node.operation,
                AbstractOperation::BooleanConstant { psi_operation, .. }
                    if psi_operation == row.psi_operation
            )
        })
        .expect("folded node exists");
    assert_eq!(
        folded.provenance,
        vec![PsiProvenance::Operation(row.psi_operation)]
    );
    folded.fuel.pop();
    malformed.identity = recompute_psi_optimization_unit_identity(&malformed);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(verified_input, malformed),
        Err(
            OptimizationUnitValidationError::FuelDoesNotMatchProvenance { machine: rejected, .. }
        ) if rejected == machine
    ));
}

#[test]
fn sole_case_parameter_membership_folds_without_producer() {
    let unit = lowered_unit_entry(SOLE_CASE_PARAMETER_SOURCE, "sole-case parameter", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    let (site, membership) = membership_on(&input, machine, place).expect("membership exists");

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    assert_eq!(patch.machine, machine);
    assert_eq!(patch.place, place);
    assert_eq!(patch.producer, None);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site, site);
    assert_eq!(row.psi_operation, membership.0);
    assert_eq!(row.source, place);
    assert_eq!(row.producer, None);
    assert_eq!(row.observed_case, row.proven_case);
    assert!(row.outcome);

    let folded = &run.session().unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
    assert!(membership_on(run.session().unit(), machine, place).is_none());
}

#[test]
fn sole_case_local_keeps_establishment_basis() {
    let unit = lowered_unit_entry(SOLE_CASE_LOCAL_SOURCE, "sole-case local", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (place, producer, _) = established_place(&input, machine);

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    assert_eq!(patch.producer, Some(producer));
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.source, place);
    assert_eq!(row.producer, Some(producer));
    assert!(row.outcome);
}

#[test]
fn replay_rejects_forged_roster_rows() {
    let unit = lowered_unit_entry(SOLE_CASE_PARAMETER_SOURCE, "sole-case parameter", "probe");
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _) = single_commit(&run);

    // Claiming a producer on a roster-proven row mismatches the replayed plan.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.producer = Some(patch.memberships[0].psi_operation);
        }),
    );
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].producer = Some(patch.memberships[0].psi_operation);
        }),
    );

    // A forged proven case on the roster basis mismatches too.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].proven_case = semantic_vocabulary::StructuralCaseId::new(
                patch.memberships[0].proven_case.get() + 7,
            )
            .expect("forged case identity");
            patch.memberships[0].outcome =
                patch.memberships[0].proven_case == patch.memberships[0].observed_case;
        }),
    );

    let validated = validate_case_membership_specialization_candidate(&input, &commit.declaration)
        .expect("the exact roster candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
}

#[test]
fn path_field_membership_folds_on_sole_case_end() {
    let unit = lowered_unit_entry(PATH_FIELD_SOURCE, "path-field membership", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    let (site, membership) = membership_on(&input, machine, place).expect("membership exists");

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    assert_eq!(patch.machine, machine);
    assert_eq!(patch.place, place);
    assert_eq!(patch.producer, None);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site, site);
    assert_eq!(row.psi_operation, membership.0);
    assert_eq!(row.source, place);
    assert_eq!(row.producer, None);
    assert_eq!(row.observed_case, row.proven_case);
    assert!(row.outcome);

    let folded = &run.session().unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
    assert!(membership_on(run.session().unit(), machine, place).is_none());
}

#[test]
fn nested_path_membership_folds_through_records() {
    let unit = lowered_unit_entry(NESTED_PATH_SOURCE, "nested-path membership", "probe");

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert!(row.outcome);

    let folded = &run.session().unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == row.site.block)
        .expect("block retained")
        .nodes[usize::try_from(row.site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
}

#[test]
fn multi_case_path_membership_yields_no_candidate() {
    assert_declines(lowered_unit_entry(
        PATH_MULTI_CASE_SOURCE,
        "multi-case path",
        "probe",
    ));
}

#[test]
fn split_path_memberships_fold_only_the_proven_position() {
    let unit = lowered_unit_entry(PATH_SPLIT_SOURCE, "split-path memberships", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    let memberships = memberships_on(&input, machine, place);
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

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.psi_operation, folded_op);
    assert_ne!(row.psi_operation, unproven_op);
    assert!(row.outcome);

    let function = run
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
    let unit = lowered_unit_entry(PATH_FIELD_SOURCE, "path-field membership", "probe");
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _) = single_commit(&run);

    // A forged producer on a path-proven row claims an establishment basis
    // that cannot prove a nested position.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].producer = Some(patch.memberships[0].psi_operation);
        }),
    );

    // A forged proven case at the resolved position mismatches the replayed
    // roster.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].proven_case = semantic_vocabulary::StructuralCaseId::new(
                patch.memberships[0].proven_case.get() + 7,
            )
            .expect("forged case identity");
            patch.memberships[0].outcome =
                patch.memberships[0].proven_case == patch.memberships[0].observed_case;
        }),
    );

    let validated = validate_case_membership_specialization_candidate(&input, &commit.declaration)
        .expect("the exact path candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
}

/// The folded row's path ends on the sole-case roster below a `FixedIndex`:
/// the array element's declared element type proves the verdict. One test
/// asserts each reachable indexed shape — a root index, an index below a
/// record field, and an index mid-path with a field below it.
#[test]
fn array_element_membership_folds_on_sole_case() {
    let unit = lowered_unit_entry(ARRAY_ELEMENT_SOURCE, "array element membership", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    let memberships = memberships_on(&input, machine, place);
    let [(site, membership, path)] = memberships.as_slice() else {
        panic!("one membership observes the array")
    };
    assert!(
        matches!(
            path.as_slice(),
            [terminal_psi::StructuralPathSegment::FixedIndex(1)]
        ),
        "the fixture observes one fixed-index element"
    );

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    assert_eq!(patch.machine, machine);
    assert_eq!(patch.place, place);
    assert_eq!(patch.producer, None);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site, *site);
    assert_eq!(row.psi_operation, membership.0);
    assert_eq!(row.source, place);
    assert_eq!(row.producer, None);
    assert_eq!(row.observed_case, row.proven_case);
    assert!(row.outcome);

    // The proposal is deterministic: an independent run commits the same
    // candidate, custody, and output revision.
    let replayed = specialize(lowered_unit_entry(
        ARRAY_ELEMENT_SOURCE,
        "array element membership",
        "probe",
    ));
    assert_eq!(replayed.commits(), run.commits());

    let folded = &run.session().unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
    assert!(membership_on(run.session().unit(), machine, place).is_none());
}

#[test]
fn nested_array_membership_folds_through_field_index() {
    let unit = lowered_unit_entry(NESTED_ARRAY_SOURCE, "nested array membership", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    let memberships = memberships_on(&input, machine, place);
    let [(site, membership, path)] = memberships.as_slice() else {
        panic!("one membership observes the nested element")
    };
    assert!(
        matches!(
            path.as_slice(),
            [
                terminal_psi::StructuralPathSegment::Field(_),
                terminal_psi::StructuralPathSegment::FixedIndex(0)
            ]
        ),
        "the fixture descends a field then a fixed index"
    );

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site, *site);
    assert_eq!(row.psi_operation, membership.0);
    assert!(row.outcome);

    let folded = &run.session().unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
}

#[test]
fn array_element_path_membership_folds_at_depth() {
    let unit = lowered_unit_entry(
        DEEP_ARRAY_PATH_SOURCE,
        "deep array path membership",
        "probe",
    );
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    let memberships = memberships_on(&input, machine, place);
    let [(site, _, path)] = memberships.as_slice() else {
        panic!("one membership observes below the element")
    };
    assert!(
        matches!(
            path.as_slice(),
            [
                terminal_psi::StructuralPathSegment::Field(_),
                terminal_psi::StructuralPathSegment::FixedIndex(0),
                terminal_psi::StructuralPathSegment::Field(_)
            ]
        ),
        "the fixture crosses an array element mid-path"
    );

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site, *site);
    assert!(row.outcome);
}

#[test]
fn self_field_array_membership_folds() {
    let unit = lowered_unit_entry(SELF_ARRAY_SOURCE, "self array membership", "Root::run");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    let declaration = input.functions[0]
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)
        .expect("the receiver is rostered");
    assert!(
        matches!(
            declaration.kind,
            StructuralPlaceKind::Parameter { is_self: true, .. }
        ),
        "the fixture's membership observes the machine's own receiver"
    );
    let memberships = memberships_on(&input, machine, place);
    let [(site, _, path)] = memberships.as_slice() else {
        panic!("one membership observes the receiver's element")
    };
    assert!(
        matches!(
            path.as_slice(),
            [
                terminal_psi::StructuralPathSegment::Field(_),
                terminal_psi::StructuralPathSegment::FixedIndex(1)
            ]
        ),
        "the fixture descends the receiver's array field"
    );

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site, *site);
    assert_eq!(row.source, place);
    assert!(row.outcome);
}

#[test]
fn multi_case_array_element_membership_yields_no_candidate() {
    let unit = lowered_unit_entry(ARRAY_MULTI_CASE_SOURCE, "multi-case array element", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    let memberships = memberships_on(&input, machine, place);
    let [(_, _, path)] = memberships.as_slice() else {
        panic!("the fixture must contain a membership on the array")
    };
    assert!(
        matches!(
            path.as_slice(),
            [terminal_psi::StructuralPathSegment::FixedIndex(1)]
        ),
        "the fixture observes one fixed-index element"
    );
    assert_declines(unit);
}

#[test]
fn replay_rejects_forged_array_rows() {
    let unit = lowered_unit_entry(ARRAY_ELEMENT_SOURCE, "array element membership", "probe");
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _) = single_commit(&run);

    // A forged proven case at the resolved element mismatches the replayed
    // roster: the element's declared sole case is the basis, not the row.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].proven_case = semantic_vocabulary::StructuralCaseId::new(
                patch.memberships[0].proven_case.get() + 7,
            )
            .expect("forged case identity");
            patch.memberships[0].outcome =
                patch.memberships[0].proven_case == patch.memberships[0].observed_case;
        }),
    );

    // A forged producer claims an establishment basis on a roster-proven
    // indexed read.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].producer = Some(patch.memberships[0].psi_operation);
        }),
    );

    // A forged site coordinate relocates the fold off the membership.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].site.node += 1;
        }),
    );

    let validated = validate_case_membership_specialization_candidate(&input, &commit.declaration)
        .expect("the exact array candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
}

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(96, 64, 64, 64, 64).expect("budget")
}

fn selections() -> OptimizationSelections {
    OptimizationSelections::new([Optimization::RepresentationSpecialization])
        .expect("representation-specialization selection")
}

/// Runs the representation-specialization pass — the case-membership rule
/// beside its field-value sibling, which no fixture here exercises — to its
/// fixed point through the public pipeline entrance.
fn specialize(unit: VerifiedPsiOptimizationUnit) -> OptimizationRun {
    run_psi_pipeline(unit, &selections(), budget()).expect("the selected pass runs")
}

/// The run's single commit — the pass reached its fixed point after one
/// candidate — with its case-membership patch.
fn single_commit(
    run: &OptimizationRun,
) -> (&PsiOptimizationCommit, CaseMembershipSpecializationRewrite) {
    let [commit] = run.commits() else {
        panic!("exactly one commit: one candidate covers the place")
    };
    assert_eq!(
        commit.rule,
        CaseMembershipSpecializationRule::contract().identity()
    );
    let PsiRewritePatch::SpecializeCaseMembership(patch) = commit.declaration.patch() else {
        panic!("a case-membership patch")
    };
    (commit, patch)
}

/// The whole selected pass declines the unit and leaves it byte-exact.
fn assert_declines(unit: VerifiedPsiOptimizationUnit) {
    let input_identity = unit.unit().identity;
    let run = specialize(unit);
    assert!(run.commits().is_empty(), "no candidate specializes");
    assert_eq!(run.session().unit().identity, input_identity);
}

/// The committed declaration rebuilt with `input` as its revision and
/// `mutate` applied to its patch, keeping every other declared axis.
fn rebuild(
    declaration: &PsiRewriteCandidate,
    input: OptimizationUnitIdentity,
    mutate: impl FnOnce(&mut CaseMembershipSpecializationRewrite),
) -> Result<PsiRewriteCandidate, PsiRewriteCandidateError> {
    let PsiRewritePatch::SpecializeCaseMembership(mut patch) = declaration.patch() else {
        panic!("a case-membership patch")
    };
    mutate(&mut patch);
    PsiRewriteCandidate::new_case_membership_specialization(
        input,
        CaseMembershipSpecializationRule::contract(),
        declaration.affected_blocks().to_vec(),
        declaration.provenance().to_vec(),
        declaration.predicted_cost_delta(),
        patch,
    )
}

fn forged(
    declaration: &PsiRewriteCandidate,
    mutate: impl FnOnce(&mut CaseMembershipSpecializationRewrite),
) -> Result<PsiRewriteCandidate, PsiRewriteCandidateError> {
    rebuild(declaration, declaration.input(), mutate)
}

/// A forged row is refused either at candidate construction — the patch
/// invariants already disagree — or by the independent replay, which
/// re-admits every row against `input` rather than trusting it.
fn assert_rejects_rows(
    input: &PsiOptimizationUnit,
    forged: Result<PsiRewriteCandidate, PsiRewriteCandidateError>,
) {
    let Ok(candidate) = forged else {
        return;
    };
    assert!(matches!(
        validate_case_membership_specialization_candidate(input, &candidate),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch
            | OptimizationUnitValidationError::CandidateProvenanceMismatch
            | OptimizationUnitValidationError::CandidateLocationMissing
            | OptimizationUnitValidationError::CandidateOutsideRegionMismatch)
    ));
}

/// The machine's parameter place.
fn parameter_place(unit: &PsiOptimizationUnit) -> PlaceId {
    unit.functions[0]
        .structural_places
        .iter()
        .find(|declaration| matches!(declaration.kind, StructuralPlaceKind::Parameter { .. }))
        .expect("parameter place exists")
        .id
}

/// The machine's structural block-parameter place — the state parameter an
/// edge binding delivers.
fn block_parameter_place(unit: &PsiOptimizationUnit, machine: MachineId) -> PlaceId {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    function
        .structural_places
        .iter()
        .find(|declaration| matches!(declaration.kind, StructuralPlaceKind::BlockParameter { .. }))
        .expect("block parameter exists")
        .id
}

/// The place the machine's membership observes — the state's block
/// parameter in bound fixtures — plus the one place its structural
/// bindings name, which the fixture requires to be the same place across
/// every binding.
fn bound_observation(unit: &PsiOptimizationUnit, machine: MachineId) -> (PlaceId, PlaceId) {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let place = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::StructuralCaseMembership { source, .. } => Some(*source),
            _ => None,
        })
        .expect("a membership observes the parameter");
    let declaration = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)
        .expect("the observed place is rostered");
    assert!(
        matches!(declaration.kind, StructuralPlaceKind::BlockParameter { .. }),
        "the fixture's membership observes a block parameter"
    );
    let mut bound = None;
    for edge in function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
    {
        for binding in &edge.structural_bindings {
            if binding.parameter != place {
                continue;
            }
            let argument = &binding.argument;
            assert!(
                argument.path.is_empty(),
                "the fixture binds the whole place"
            );
            match bound {
                None => bound = Some(argument.place),
                Some(seen) => assert_eq!(seen, argument.place, "the fixture binds uniformly"),
            }
        }
    }
    (place, bound.expect("the parameter is bound"))
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
    memberships_on(unit, machine, place)
        .into_iter()
        .next()
        .map(|(site, membership, _)| (site, membership))
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

/// A case established in the machine body arrives at the observing state
/// as a structural block parameter: the state's incoming edge binds the
/// parameter to the one `EstablishScalarCase` result through an owned,
/// whole-place argument, so the bound place's establishment proves the
/// membership even though the parameter itself holds no producer.
const BOUND_CASE_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    data Root {}
    machine Root::run() {
        let c: Choice = Choice::Some { value: 37 };
        transition { _ -> check(c) }
        state check(c: Choice) {
            let m: bool = c in Choice::Some;
            transition m { true -> good() _ -> bad() }
        }
        state good() {}
        state bad() {}
    }
"#;

/// Two predecessor edges bind the parameter to different established
/// places: no uniform binding exists, so nothing proves which case the
/// parameter holds even though every candidate place is itself
/// established.
const DIVERGENT_BINDING_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    data Root {}
    machine Root::run(flag: bool) {
        let a: Choice = Choice::Some { value: 37 };
        let b: Choice = Choice::Empty;
        transition flag { true -> check(a) _ -> check(b) }
        state check(c: Choice) {
            let m: bool = c in Choice::Some;
            transition m { true -> good() _ -> bad() }
        }
        state good() {}
        state bad() {}
    }
"#;

/// A parameter delivered through a relay state resolves transitively: the
/// membership's parameter binds the relay's parameter, which binds the
/// established place — the establishment crosses each uniform binding in
/// turn.
const CHAINED_BINDING_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    data Root {}
    machine Root::run() {
        let c: Choice = Choice::Some { value: 37 };
        transition { _ -> relay(c) }
        state relay(d: Choice) {
            transition { _ -> check(d) }
        }
        state check(c: Choice) {
            let m: bool = c in Choice::Some;
            transition m { true -> good() _ -> bad() }
        }
        state good() {}
        state bad() {}
    }
"#;

#[test]
fn bound_parameter_membership_folds_through_uniform_binding() {
    let unit = lowered_unit_entry(BOUND_CASE_SOURCE, "bound parameter membership", "Root::run");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    // The membership observes the state's block parameter while the proof
    // is the `EstablishScalarCase` result the incoming edge binds it to
    // whole.
    let (place, bound) = bound_observation(&input, machine);
    let (established, producer, established_case) = established_place(&input, machine);
    assert_eq!(bound, established);
    let (site, membership) = membership_on(&input, machine, place).expect("membership exists");

    let run = specialize(unit);
    let (commit, patch) = single_commit(&run);
    assert_eq!(patch.machine, machine);
    assert_eq!(patch.place, place);
    assert_eq!(
        patch.producer,
        Some(producer),
        "the bound place's establishment is the patch witness"
    );
    assert_eq!(commit.input, input.identity);
    assert_ne!(commit.output, input.identity);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site, site);
    assert_eq!(row.psi_operation, membership.0);
    assert_eq!(row.result, membership.1);
    assert_eq!(row.source, place);
    assert_eq!(row.producer, Some(producer));
    assert_eq!(row.observed_case, membership.2);
    assert_eq!(row.proven_case, established_case);
    assert!(row.outcome);

    // The proposal is deterministic: an independent run commits the same
    // candidate, custody, and output revision.
    let replayed = specialize(lowered_unit_entry(
        BOUND_CASE_SOURCE,
        "bound parameter membership",
        "Root::run",
    ));
    assert_eq!(replayed.commits(), run.commits());

    let folded = &run.session().unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
    assert_eq!(
        folded.provenance,
        vec![PsiProvenance::Operation(membership.0)]
    );
    assert!(membership_on(run.session().unit(), machine, place).is_none());
}

#[test]
fn divergent_incoming_bindings_decline() {
    let unit = lowered_unit_entry(
        DIVERGENT_BINDING_SOURCE,
        "divergent bindings decline",
        "Root::run",
    );
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = block_parameter_place(&input, machine);
    // The fixture must actually route both established places into the one
    // observing parameter — the bindings diverge by place identity.
    assert!(
        membership_on(&input, machine, place).is_some(),
        "the fixture must actually contain a membership on the parameter"
    );
    let bound: std::collections::BTreeSet<_> = input.functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .flat_map(|edge| &edge.structural_bindings)
        .filter(|binding| binding.parameter == place)
        .map(|binding| binding.argument.place)
        .collect();
    assert_eq!(bound.len(), 2, "two divergent bound places");
    assert_declines(unit);
}

#[test]
fn chained_binding_forwards_through_relay() {
    let unit = lowered_unit_entry(CHAINED_BINDING_SOURCE, "chained binding", "Root::run");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    // The membership observes `check`'s parameter, which binds `relay`'s
    // parameter, which binds the established place — the proof crosses
    // both uniform bindings.
    let (place, bound) = bound_observation(&input, machine);
    let declaration = input.functions[0]
        .structural_places
        .iter()
        .find(|declaration| declaration.id == bound)
        .expect("the relay parameter is rostered");
    assert!(
        matches!(declaration.kind, StructuralPlaceKind::BlockParameter { .. }),
        "the membership's parameter binds another block parameter"
    );
    let (_, producer, established_case) = established_place(&input, machine);

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    assert_eq!(patch.place, place);
    assert_eq!(patch.producer, Some(producer));
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.source, place);
    assert_eq!(
        row.producer,
        Some(producer),
        "the establishment crosses both uniform bindings"
    );
    assert_eq!(row.proven_case, established_case);
    assert!(row.outcome);
}

#[test]
fn replay_rejects_forged_bound_rows() {
    let unit = lowered_unit_entry(BOUND_CASE_SOURCE, "bound parameter membership", "Root::run");
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _patch) = single_commit(&run);

    // A forged row witness — claiming the roster basis — is refused:
    // replay re-derives the witness through the binding rather than
    // trusting it.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].producer = None;
        }),
    );

    // A forged patch-level producer — claiming the observation's own
    // custody identity rather than the forwarded establishment.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.producer = Some(patch.memberships[0].psi_operation);
        }),
    );

    // A forged proven case on the bound read.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.memberships[0].proven_case = semantic_vocabulary::StructuralCaseId::new(
                patch.memberships[0].proven_case.get() + 7,
            )
            .expect("forged case identity");
            patch.memberships[0].outcome =
                patch.memberships[0].proven_case == patch.memberships[0].observed_case;
        }),
    );

    // The untampered declaration still validates to the committed output.
    let validated = validate_case_membership_specialization_candidate(&input, &commit.declaration)
        .expect("the exact candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
}

fn lowered_unit_entry(source: &str, label: &str, entry: &str) -> VerifiedPsiOptimizationUnit {
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
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap_or_else(|error| panic!("check {label}: {error:?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(entry),
    )
    .unwrap_or_else(|error| panic!("lower {label}: {error:?}"));
    let input = terminal_psi_to_abstract_operations::lower_artifact(
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
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .unwrap_or_else(|error| panic!("optimizer-only {label} admission: {error:?}"));
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap_or_else(|error| panic!("build {label} optimizer unit: {error:?}"))
}
