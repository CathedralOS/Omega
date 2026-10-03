//! Optimizer module role: test leaf. Established-case membership specialization admission, realization, and custody evidence through the rule.
//!
//! Every fixture is driven through the one live route: the
//! `RepresentationSpecialization` pass runs `CaseMembershipSpecializationRule`
//! to its fixed point, the committed candidate carries the proposed plan,
//! the run's session carries the folded unit, and forged rows are replayed
//! against `validate_case_membership_specialization_candidate` — the
//! independent validator the pass manager itself consults.

use crate::rules::CaseMembershipSpecializationRule;
use crate::{OptimizationRun, PsiOptimizationCommit, run_psi_pipeline};
use abstract_operations::AbstractOperation;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use optimization_core::{
    Optimization, OptimizationSelections, OptimizationUnitIdentity, OptimizationWorkBudget,
};
use optimization_unit::{
    CaseMembershipSpecializationRewrite, NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate,
    PsiRewriteCandidateError, PsiRewritePatch,
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

/// A membership on a machine parameter place whose declared sum roster holds
/// exactly one case: the roster alone proves the case regardless of caller,
/// so the observation folds to `true`.
const SOLE_CASE_PARAMETER_SOURCE: &str = r#"
    data Tag { case Only; }
    machine probe(t: Tag) -> bool {
        t in Tag::Only
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
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.memberships[0].outcome = !patch.memberships[0].outcome;
            },
        ),
    );

    // A forged observed case — a case identity no source operation carries.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.memberships[0].observed_case = semantic_vocabulary::StructuralCaseId::new(
                    patch.memberships[0].observed_case.get() + 7,
                )
                .expect("forged case identity");
            },
        ),
    );

    // A forged site coordinate.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.memberships[0].site.node += 1;
            },
        ),
    );

    // A forged producer identity.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.producer = Some(patch.memberships[0].psi_operation);
            },
        ),
    );

    // The untampered declaration still validates to the committed output.
    let validated = validate_case_membership_specialization_candidate(
        &input,
        commit.declaration.rewrite().expect("rewrite declaration"),
    )
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
        commit.declaration.rewrite().expect("rewrite declaration"),
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
            commit.declaration.rewrite().expect("rewrite declaration")
        )
        .err(),
        Some(OptimizationUnitValidationError::CandidateInputMismatch)
    );
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
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.producer = Some(patch.memberships[0].psi_operation);
            },
        ),
    );
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.memberships[0].producer = Some(patch.memberships[0].psi_operation);
            },
        ),
    );

    // A forged proven case on the roster basis mismatches too.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.memberships[0].proven_case = semantic_vocabulary::StructuralCaseId::new(
                    patch.memberships[0].proven_case.get() + 7,
                )
                .expect("forged case identity");
                patch.memberships[0].outcome =
                    patch.memberships[0].proven_case == patch.memberships[0].observed_case;
            },
        ),
    );

    let validated = validate_case_membership_specialization_candidate(
        &input,
        commit.declaration.rewrite().expect("rewrite declaration"),
    )
    .expect("the exact roster candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
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

/// A state parameter uniformly bound to a record whose `c` member arrives
/// as a stored-whole zeroed child: a zeroed establishment selects each
/// admissible sum's first declared case at every position of its storage,
/// so `h.c in Choice::Empty` folds to `true` and `h.c in Choice::Some`
/// folds to `false` under the same `EstablishZeroedStructural` witness.
const ZEROED_SUM_MEMBER_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    data Holder { c: Choice; flag: bool; }
    data Root {}
    machine Root::run() {
        let h: Holder = Holder { flag: true };
        transition { _ -> check(h) }
        state check(h: Holder) {
            let e: bool = h.c in Choice::Empty;
            let s: bool = h.c in Choice::Some;
            transition e && !s { true -> good() _ -> bad() }
        }
        state good() {}
        state bad() {}
    }
"#;

/// The same proof reached through the whole-record form: `Holder {}`
/// mints one `EstablishZeroedStructural` for the record itself, and the
/// state parameter's uniform binding forwards it to the membership's
/// establishment chain.
const ZEROED_ROOT_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    data Holder { c: Choice; flag: bool; }
    data Root {}
    machine Root::run() {
        let h: Holder = Holder { };
        transition { _ -> check(h) }
        state check(h: Holder) {
            let m: bool = h.c in Choice::Empty;
            transition m { true -> good() _ -> bad() }
        }
        state good() {}
        state bad() {}
    }
"#;

/// The `EstablishZeroedStructural` custody identity in `machine` — every
/// fixture carries exactly one.
fn zeroed_producer(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
) -> semantic_vocabulary::OperationId {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::EstablishZeroedStructural { psi_operation, .. } => {
                Some(*psi_operation)
            }
            _ => None,
        })
        .expect("the fixture establishes a zeroed structural")
}

#[test]
fn zeroed_sum_member_memberships_fold_both_verdicts() {
    let unit = lowered_unit_entry(ZEROED_SUM_MEMBER_SOURCE, "zeroed sum member", "Root::run");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = block_parameter_place(&input, machine);
    let memberships = memberships_on(&input, machine, place);
    assert_eq!(
        memberships.len(),
        2,
        "two memberships observe the zeroed sum member"
    );
    let zeroed = zeroed_producer(&input, machine);

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    assert_eq!(patch.place, place);
    assert_eq!(
        patch.producer, None,
        "the block parameter carries no root basis of its own"
    );
    let [empty_row, some_row] = patch.memberships.as_slice() else {
        panic!("both memberships fold in one candidate")
    };
    for row in [empty_row, some_row] {
        assert_eq!(row.source, place);
        assert_eq!(row.producer, Some(zeroed));
        assert_eq!(
            row.proven_case, empty_row.observed_case,
            "both memberships prove the first declared case"
        );
    }
    assert!(empty_row.outcome, "the observed case is the proven case");
    assert_ne!(
        some_row.observed_case, some_row.proven_case,
        "the second membership observes a later case"
    );
    assert!(
        !some_row.outcome,
        "observing a non-first case under zeroed storage folds to false"
    );
}

#[test]
fn zeroed_root_membership_folds_through_binding() {
    let unit = lowered_unit_entry(ZEROED_ROOT_SOURCE, "zeroed root", "Root::run");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = block_parameter_place(&input, machine);
    let (site, (psi_operation, _, _)) =
        membership_on(&input, machine, place).expect("membership exists");
    let zeroed = zeroed_producer(&input, machine);

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site, site);
    assert_eq!(row.psi_operation, psi_operation);
    assert_eq!(row.source, place);
    assert_eq!(row.producer, Some(zeroed));
    assert_eq!(row.proven_case, row.observed_case);
    assert!(row.outcome);

    // The folded node is a `BooleanConstant` holding `true` at the same
    // coordinates.
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
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
}

#[test]
fn replay_rejects_forged_zeroed_rows() {
    let unit = lowered_unit_entry(ZEROED_ROOT_SOURCE, "zeroed root forged", "Root::run");
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _) = single_commit(&run);

    // A roster-basis claim on an establishment-proven row mismatches the
    // replayed proof: the resolved roster holds two cases, not one.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.memberships[0].producer = None;
            },
        ),
    );

    // A forged proven case mismatches the zeroed first-case proof.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.memberships[0].proven_case = semantic_vocabulary::StructuralCaseId::new(
                    patch.memberships[0].proven_case.get() + 7,
                )
                .expect("forged case identity");
            },
        ),
    );

    // A forged witness — a different operation claimed as the establishment.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.memberships[0].producer = Some(patch.memberships[0].psi_operation);
            },
        ),
    );

    let validated = validate_case_membership_specialization_candidate(
        &input,
        commit.declaration.rewrite().expect("rewrite declaration"),
    )
    .expect("the exact zeroed candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
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

/// A case-match arm binds `lease` — the place a case-payload
/// `StructuralLeafCopy` mints — and hands it whole to `inspect`, whose
/// block parameter then observes its own case at the empty path: the
/// uniform binding reaches the copy, whose `Case(Wrap)` then
/// `Field(lease)` projection resolves beneath its source to the
/// stored-whole payload child, whose own establishment proves
/// `Lease::Live` permanently.
const CASE_PAYLOAD_COPY_ROOT_SOURCE: &str = r#"
    data Lease [copy] { case Live; case Free; }
    data Outer { case Wrap(lease: Lease); case None; }
    data Root {}
    machine Root::run() {
        let o: Outer = Outer::Wrap { lease: Lease::Live };
        transition { _ -> read(o) }
        state read(o: Outer) {
            transition o {
                Outer::Wrap { lease } -> inspect(lease)
                Outer::None -> bad()
            }
        }
        state inspect(lease: Lease) {
            let m: bool = lease in Lease::Live;
            transition m { true -> good() _ -> bad() }
        }
        state good() {}
        state bad() {}
    }
"#;

/// Two memberships read below the payload record the arm copy hands to
/// `inspect`: `pair.left` resolves through the uniform binding, the copy's
/// canonical projection, the `EstablishCase` payload child, and the
/// record's stored-whole `left` child to `Lease::Live`, while `pair.right`
/// proves `Lease::Free` — so observing `Live` there folds to `false` under
/// its own witness.
const CASE_PAYLOAD_COPY_MEMBER_SOURCE: &str = r#"
    data Lease [copy] { case Live; case Free; }
    data Pair [copy] { left: Lease; right: Lease; }
    data Outer { case Wrap(pair: Pair); case None; }
    data Root {}
    machine Root::run() {
        let o: Outer = Outer::Wrap { pair: Pair { left: Lease::Live, right: Lease::Free } };
        transition { _ -> read(o) }
        state read(o: Outer) {
            transition o {
                Outer::Wrap { pair } -> inspect(pair)
                Outer::None -> bad()
            }
        }
        state inspect(pair: Pair) {
            let l: bool = pair.left in Lease::Live;
            let r: bool = pair.right in Lease::Live;
            transition l && !r { true -> good() _ -> bad() }
        }
        state good() {}
        state bad() {}
    }
"#;

/// The same arm over a source established under the other case: the copy's
/// `Case(Wrap)` step names a payload overlay the `Outer::Pair`
/// establishment never wrote, so the membership stays an observation.
const CASE_PAYLOAD_COPY_OTHER_CASE_SOURCE: &str = r#"
    data Lease [copy] { case Live; case Free; }
    data Outer { case Wrap(lease: Lease); case Pair(lease: Lease); }
    data Root {}
    machine Root::run() {
        let o: Outer = Outer::Pair { lease: Lease::Live };
        transition { _ -> read(o) }
        state read(o: Outer) {
            transition o {
                Outer::Wrap { lease } -> inspect(lease)
                Outer::Pair { lease } -> bad()
            }
        }
        state inspect(lease: Lease) {
            let m: bool = lease in Lease::Live;
            transition m { true -> good() _ -> bad() }
        }
        state good() {}
        state bad() {}
    }
"#;

/// The case-payload copy place in `machine` — the place a
/// `Case { payload }` arm binding mints — with its source place and
/// case-qualified canonical projection. `None` when no arm binding copies
/// a case payload.
fn case_payload_copy(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
) -> Option<(
    PlaceId,
    PlaceId,
    Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
)> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)?;
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::StructuralLeafCopy {
                result,
                source,
                path: abstract_operations::StructuralCopyPath::Case(path),
                ..
            } => Some((result.place, *source, path.clone())),
            _ => None,
        })
}

/// The producer `place`'s operation-result kind names.
fn producer_of(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    place: PlaceId,
) -> semantic_vocabulary::OperationId {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let declaration = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)
        .expect("the place is rostered");
    let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
        panic!("the place is an operation result")
    };
    producer
}

/// The payload child place the fixture's `EstablishCase` stores whole,
/// with its own producer — the nested establishment a copy-crossing
/// membership resolves to.
fn payload_child(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
) -> (PlaceId, semantic_vocabulary::OperationId) {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let child = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::EstablishCase { fields, .. } => {
                fields
                    .iter()
                    .find_map(|initializer| match &initializer.value {
                        terminal_psi::RecordFieldValue::Structural(argument) => {
                            Some(argument.place)
                        }
                        _ => None,
                    })
            }
            _ => None,
        })
        .expect("the fixture's case stores a structural payload");
    (child, producer_of(unit, machine, child))
}

/// The producer of the `index`-th structural member the fixture's payload
/// record establishes whole — `left` then `right` in declaration order.
fn payload_member_producer(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    index: usize,
) -> semantic_vocabulary::OperationId {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let (_, record_producer) = payload_child(unit, machine);
    let member = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::EstablishRecord {
                psi_operation,
                fields,
                ..
            } if *psi_operation == record_producer => fields
                .iter()
                .filter_map(|initializer| match &initializer.value {
                    terminal_psi::RecordFieldValue::Structural(argument) => Some(argument.place),
                    _ => None,
                })
                .nth(index),
            _ => None,
        })
        .expect("the payload record stores the member whole");
    producer_of(unit, machine, member)
}

#[test]
fn case_payload_copy_root_membership_folds_through_structural_case() {
    let unit = lowered_unit_entry(
        CASE_PAYLOAD_COPY_ROOT_SOURCE,
        "case payload copy root membership",
        "Root::run",
    );
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (copy, source, projection) =
        case_payload_copy(&input, machine).expect("the arm binding copies the payload");
    assert!(
        matches!(
            projection.as_slice(),
            [
                semantic_vocabulary::CanonicalStructuralPathSegment::Case(_),
                semantic_vocabulary::CanonicalStructuralPathSegment::Field(_)
            ]
        ),
        "the arm binding copies the case payload field"
    );
    // The membership observes `inspect`'s block parameter, bound uniformly
    // to the copy — the chain crosses binding then copy then payload.
    let (place, bound) = copy_bound_parameter(&input, machine);
    assert_eq!(bound, copy, "the parameter binds the case-payload copy");
    let (site, (psi_operation, _, observed_case)) =
        membership_on(&input, machine, place).expect("a membership observes the parameter");
    let (child, witness) = payload_child(&input, machine);
    // The proof chain ends at the stored-whole payload child's own
    // establishment — a different operation than any place on the
    // observed side of the copy.
    assert_ne!(child, copy);
    assert_ne!(child, source);

    let run = specialize(unit);
    let (commit, patch) =
        commit_for(&run, place).expect("the copy's membership folds through the payload chain");
    assert_eq!(patch.place, place);
    assert_eq!(
        patch.producer,
        Some(witness),
        "the payload child's establishment is the copy's root basis"
    );
    let [row] = patch.memberships.as_slice() else {
        panic!("one folded membership")
    };
    assert_eq!(row.site, site);
    assert_eq!(row.psi_operation, psi_operation);
    assert_eq!(row.source, place);
    assert_eq!(row.producer, Some(witness));
    assert_eq!(row.proven_case, observed_case);
    assert!(row.outcome);

    // The run also folds the dispatch membership ahead of this commit, so
    // the stored declaration names the intermediate revision as its input;
    // the same patch replayed against the original input validates the
    // proof chain end to end.
    let declaration = rebuild(
        commit.declaration.rewrite().expect("rewrite declaration"),
        input.identity,
        |_| {},
    )
    .expect("the committed patch rebuilds against the original input");
    validate_case_membership_specialization_candidate(&input, &declaration)
        .expect("the exact case-payload membership candidate replays independently");
}

#[test]
fn case_payload_copy_member_memberships_fold_both_verdicts() {
    let unit = lowered_unit_entry(
        CASE_PAYLOAD_COPY_MEMBER_SOURCE,
        "case payload copy member memberships",
        "Root::run",
    );
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (copy, _, _) =
        case_payload_copy(&input, machine).expect("the arm binding copies the payload");
    let (place, bound) = copy_bound_parameter(&input, machine);
    assert_eq!(bound, copy, "the parameter binds the case-payload copy");
    let memberships = memberships_on(&input, machine, place);
    assert_eq!(
        memberships.len(),
        2,
        "two memberships observe below the copied payload"
    );
    let member_op = |member: &str| {
        memberships
            .iter()
            .find(|(_, _, path)| {
                matches!(
                    path.as_slice(),
                    [terminal_psi::StructuralPathSegment::Field(name)] if name == member
                )
            })
            .map(|(_, membership, _)| (membership.0, membership.2))
            .expect("membership at the named member exists")
    };
    let (left_op, left_observed) = member_op("left");
    let (right_op, right_observed) = member_op("right");
    let left_witness = payload_member_producer(&input, machine, 0);
    let right_witness = payload_member_producer(&input, machine, 1);

    let run = specialize(unit);
    let (_, patch) = commit_for(&run, place)
        .expect("the copy's member memberships fold through the payload chain");
    assert_eq!(patch.place, place);
    assert_eq!(
        patch.producer, None,
        "the copied record holds no root case of its own"
    );
    let [left_row, right_row] = patch.memberships.as_slice() else {
        panic!("both memberships fold in one candidate")
    };
    assert_eq!(left_row.psi_operation, left_op);
    assert_eq!(left_row.producer, Some(left_witness));
    assert_eq!(left_row.proven_case, left_observed);
    assert!(left_row.outcome);
    assert_eq!(right_row.psi_operation, right_op);
    assert_eq!(right_row.producer, Some(right_witness));
    assert_ne!(
        right_row.proven_case, right_observed,
        "the right member holds `Lease::Free`, not the observed `Live`"
    );
    assert!(!right_row.outcome);
}

#[test]
fn case_payload_copy_membership_under_other_case_declines() {
    let unit = lowered_unit_entry(
        CASE_PAYLOAD_COPY_OTHER_CASE_SOURCE,
        "case payload copy membership other case",
        "Root::run",
    );
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    // The membership observes `inspect`'s block parameter, bound to the
    // `Wrap` arm's copy — while the source establishes `Pair`, so the
    // `Case(Wrap)` step the copy's projection names declines. The dispatch
    // membership on `read`'s parameter still folds to `false`, so the run
    // commits the other place's plan alone.
    let (place, _) = copy_bound_parameter(&input, machine);
    let (site, (psi_operation, _, _)) =
        membership_on(&input, machine, place).expect("a membership observes the parameter");

    let run = specialize(unit);
    assert!(
        commit_for(&run, place).is_none(),
        "no candidate covers the copy's membership"
    );
    // The folded unit still carries the observation at the same site.
    let function = run
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    let node = &function
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    assert!(matches!(
        &node.operation,
        AbstractOperation::StructuralCaseMembership {
            psi_operation: kept,
            source,
            ..
        } if *kept == psi_operation && *source == place
    ));
}

#[test]
fn replay_rejects_forged_case_payload_membership_rows() {
    let unit = lowered_unit_entry(
        CASE_PAYLOAD_COPY_ROOT_SOURCE,
        "case payload copy membership forged",
        "Root::run",
    );
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (place, _) = copy_bound_parameter(&input, machine);
    let run = specialize(unit);
    let (commit, _) = commit_for(&run, place).expect("the copy's plan committed");
    // The dispatch membership commits ahead of this one, so the stored
    // declaration names the intermediate revision as its input; every
    // forgery below is rebuilt against `input` — the revision the
    // proof chain actually resolves on — so replay, not the input
    // guard, does the rejecting.
    let declaration = commit.declaration.rewrite().expect("rewrite declaration");
    let forge = |mutate: &mut dyn FnMut(&mut CaseMembershipSpecializationRewrite)| {
        rebuild(declaration, input.identity, |patch| mutate(patch))
    };

    // A forged verdict.
    assert_rejects_rows(
        &input,
        forge(&mut |patch| {
            patch.memberships[0].outcome = !patch.memberships[0].outcome;
        }),
    );

    // A roster-basis claim on an establishment-proven row: the resolved
    // `Lease` roster holds two cases, not one.
    assert_rejects_rows(
        &input,
        forge(&mut |patch| {
            patch.memberships[0].producer = None;
        }),
    );

    // A forged witness — the membership operation claimed as the
    // establishment the copy forwards to.
    assert_rejects_rows(
        &input,
        forge(&mut |patch| {
            patch.memberships[0].producer = Some(patch.memberships[0].psi_operation);
        }),
    );

    // A forged plan witness: the copy's root basis is the payload child's
    // establishment, not the membership operation.
    assert_rejects_rows(
        &input,
        forge(&mut |patch| {
            patch.producer = Some(patch.memberships[0].psi_operation);
        }),
    );

    // A forged proven case mismatches the payload child's own case.
    assert_rejects_rows(
        &input,
        forge(&mut |patch| {
            patch.memberships[0].proven_case = semantic_vocabulary::StructuralCaseId::new(
                patch.memberships[0].proven_case.get() + 7,
            )
            .expect("forged case identity");
        }),
    );

    validate_case_membership_specialization_candidate(
        &input,
        &forge(&mut |_| {}).expect("the unmutated patch rebuilds"),
    )
    .expect("the exact candidate still validates");
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
    let (commit, patch) = commit_patch(commit);
    (commit, patch)
}

/// The run's commit whose case-membership plan observes `place`, or `None`
/// when no candidate covers it — copy fixtures also fold the dispatch's own
/// membership, so a run may hold commits for several places.
fn commit_for(
    run: &OptimizationRun,
    place: PlaceId,
) -> Option<(&PsiOptimizationCommit, CaseMembershipSpecializationRewrite)> {
    run.commits().iter().find_map(|commit| {
        assert_eq!(
            commit.rule,
            CaseMembershipSpecializationRule::contract().identity(),
            "only case-membership candidates run in these fixtures"
        );
        let PsiRewritePatch::SpecializeCaseMembership(patch) = commit
            .declaration
            .rewrite()
            .expect("rewrite declaration")
            .patch()
        else {
            panic!("a case-membership patch")
        };
        (patch.place == place).then_some((commit, patch))
    })
}

/// The commit's case-membership patch; the rule contract pins this
/// candidate family.
fn commit_patch(
    commit: &PsiOptimizationCommit,
) -> (&PsiOptimizationCommit, CaseMembershipSpecializationRewrite) {
    assert_eq!(
        commit.rule,
        CaseMembershipSpecializationRule::contract().identity()
    );
    let PsiRewritePatch::SpecializeCaseMembership(patch) = commit
        .declaration
        .rewrite()
        .expect("rewrite declaration")
        .patch()
    else {
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

/// The block parameter a case-payload `StructuralLeafCopy` feeds in
/// `machine`, plus the copy place itself — the observed place and its
/// forwarding copy in the `Case { payload } -> inspect(payload)` fixtures.
/// Every structural binding's argument is checked so the fixture binds the
/// copy whole and uniformly.
fn copy_bound_parameter(unit: &PsiOptimizationUnit, machine: MachineId) -> (PlaceId, PlaceId) {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let is_case_copy = |place: PlaceId| {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                matches!(
                    &node.operation,
                    AbstractOperation::StructuralLeafCopy {
                        result,
                        path: abstract_operations::StructuralCopyPath::Case(_),
                        ..
                    } if result.place == place
                )
            })
    };
    let mut found = None;
    for edge in function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
    {
        for binding in &edge.structural_bindings {
            let argument = &binding.argument;
            assert!(argument.path.is_empty(), "the fixture binds the whole copy");
            if is_case_copy(argument.place) {
                match found {
                    None => found = Some((binding.parameter, argument.place)),
                    Some(seen) => {
                        assert_eq!(
                            seen,
                            (binding.parameter, argument.place),
                            "the fixture binds the copy uniformly"
                        );
                    }
                }
            }
        }
    }
    found.expect("a case-payload copy feeds a block parameter")
}

/// The only `OperationResult` place in these fixtures, its
/// `EstablishCase` producer, and the fixed case.
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
            AbstractOperation::EstablishCase {
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

fn lowered_unit_entry(source: &str, label: &str, entry: &str) -> VerifiedPsiOptimizationUnit {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|error| panic!("tokenize {label}: {error:?}"));
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .unwrap_or_else(|error| panic!("parse {label}: {error:?}"));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
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
