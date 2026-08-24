//! Content conservation, identity reshuffle, and partition regressions.

use super::*;

fn source_plan_with_domain(semantic_domain: SemanticDomainId) -> ContentConservationPlan {
    let entry = source_projection(
        CheckedContentPlaceVersion::Entry,
        CheckedContentPlaceRoot::Parameter {
            position: 0,
            symbol: SymbolHandle::from_arena_index(10),
            name: "extent".to_owned(),
            is_self: false,
        },
        &[],
        semantic_domain,
    );
    let left = source_projection(
        CheckedContentPlaceVersion::Current,
        CheckedContentPlaceRoot::Result,
        &[("left", 11)],
        semantic_domain,
    );
    let right = source_projection(
        CheckedContentPlaceVersion::Current,
        CheckedContentPlaceRoot::Result,
        &[("right", 12)],
        semantic_domain,
    );
    let algebra = CheckedContentAlgebraIdentity::IntervalSet {
        coordinate_space: "Address".to_owned(),
    };
    let equation = ContentConservationEquation::new(
        entry,
        CheckedContentConservationTerm::separate([right, left]),
    );
    let fingerprint = conservation_fingerprint(&algebra, &equation);
    ContentConservationPlan {
        owner_kind: ContentConservationOwnerKind::Machine,
        owner: SymbolHandle::from_arena_index(20),
        callable: SymbolHandle::from_arena_index(21),
        algebra,
        equation,
        fingerprint,
    }
}

fn source_plan() -> ContentConservationPlan {
    source_plan_with_domain(SemanticDomainId(9))
}

fn direct_source_plan(
    semantic_domain: SemanticDomainId,
    output_field: &str,
) -> ContentConservationPlan {
    let entry = source_projection(
        CheckedContentPlaceVersion::Entry,
        CheckedContentPlaceRoot::Parameter {
            position: 0,
            symbol: SymbolHandle::from_arena_index(10),
            name: "extent".to_owned(),
            is_self: false,
        },
        &[],
        semantic_domain,
    );
    let output = source_projection(
        CheckedContentPlaceVersion::Current,
        CheckedContentPlaceRoot::Result,
        &[(output_field, 11)],
        semantic_domain,
    );
    let algebra = CheckedContentAlgebraIdentity::IntervalSet {
        coordinate_space: "Address".to_owned(),
    };
    let equation = ContentConservationEquation::new(entry, output);
    let fingerprint = conservation_fingerprint(&algebra, &equation);
    ContentConservationPlan {
        owner_kind: ContentConservationOwnerKind::Machine,
        owner: SymbolHandle::from_arena_index(20),
        callable: SymbolHandle::from_arena_index(21),
        algebra,
        equation,
        fingerprint,
    }
}

fn case_direct_source_plan(semantic_domain: SemanticDomainId) -> ContentConservationPlan {
    let segments = || {
        vec![
            CheckedContentPlaceSegment::Case(ContentCaseSegment {
                symbol: SymbolHandle::from_arena_index(30),
                name: "Present".to_owned(),
            }),
            CheckedContentPlaceSegment::Field(ContentFieldSegment {
                symbol: SymbolHandle::from_arena_index(31),
                name: "region".to_owned(),
            }),
        ]
    };
    let projection = |version, root| CheckedContentConservationTerm::Projection {
        domain: SymbolHandle::from_arena_index(70),
        semantic_domain,
        projection_machine: SymbolHandle::from_arena_index(71),
        projection_fingerprint: 0xfeed,
        subject: CheckedContentStructuralPlace {
            version,
            root,
            segments: segments(),
        },
    };
    let equation = ContentConservationEquation::new(
        projection(
            CheckedContentPlaceVersion::Entry,
            CheckedContentPlaceRoot::Parameter {
                position: 0,
                symbol: SymbolHandle::from_arena_index(10),
                name: "envelope".to_owned(),
                is_self: false,
            },
        ),
        projection(
            CheckedContentPlaceVersion::Current,
            CheckedContentPlaceRoot::Result,
        ),
    );
    let algebra = CheckedContentAlgebraIdentity::IntervalSet {
        coordinate_space: "Address".to_owned(),
    };
    let fingerprint = conservation_fingerprint(&algebra, &equation);
    ContentConservationPlan {
        owner_kind: ContentConservationOwnerKind::Machine,
        owner: SymbolHandle::from_arena_index(20),
        callable: SymbolHandle::from_arena_index(21),
        algebra,
        equation,
        fingerprint,
    }
}

fn identity_fact(
    semantic_domain: SemanticDomainId,
    output_field: &str,
    ordinal: u32,
) -> ContentIdentityReshuffleFact {
    ContentIdentityReshuffleFact {
        machine_symbol: SymbolHandle::from_arena_index(20),
        state_symbol: SymbolHandle::from_arena_index(21),
        claim_identity: PermissionClaimIdentity::Established {
            machine_symbol: SymbolHandle::from_arena_index(20),
            state_symbol: SymbolHandle::from_arena_index(21),
            source: PermissionEventSource::StateEntry,
            ordinal,
        },
        input_parameter_symbol: SymbolHandle::from_arena_index(10),
        input_segments: Default::default(),
        output_segments: Default::default(),
        plan: direct_source_plan(semantic_domain, output_field),
    }
}

fn partition_composition_fact() -> ContentPartitionCompositionFact {
    fn subjects(
        term: &CheckedContentConservationTerm,
        output: &mut Vec<CheckedContentStructuralPlace>,
    ) {
        match term {
            CheckedContentConservationTerm::Projection { subject, .. } => {
                if !output.contains(subject) {
                    output.push(subject.clone());
                }
            }
            CheckedContentConservationTerm::Separate(terms) => {
                for term in terms {
                    subjects(term, output);
                }
            }
        }
    }

    let mut source_plan = source_plan();
    source_plan.owner = SymbolHandle::from_arena_index(30);
    source_plan.callable = SymbolHandle::from_arena_index(31);
    let mut plan = source_plan.clone();
    plan.owner = SymbolHandle::from_arena_index(20);
    plan.callable = SymbolHandle::from_arena_index(21);
    let mut places = Vec::new();
    subjects(source_plan.equation.left(), &mut places);
    subjects(source_plan.equation.right(), &mut places);
    let claim_identity = identity_fact(SemanticDomainId(9), "left", 1).claim_identity;
    let CheckedContentConservationTerm::Projection { subject, .. } = source_plan.equation.left()
    else {
        panic!("fixture source input is a projection")
    };
    let entry_place = subject.clone();
    ContentPartitionCompositionFact {
        machine_symbol: plan.owner,
        state_symbol: plan.callable,
        source_callable: source_plan.callable,
        source_fingerprint: source_plan.fingerprint,
        source_derivation_depth: 0,
        source_plan,
        statement_index: 4,
        call_ordinal: 2,
        input_claim_identities: vec![claim_identity],
        input_claim_bindings: vec![psi_checked_trees::ContentPartitionInputClaimBinding {
            claim_identity,
            entry_place,
        }],
        result_rewrites: Vec::new(),
        substitutions: places
            .into_iter()
            .map(
                |place| psi_checked_trees::ContentPartitionPlaceSubstitution {
                    source: place.clone(),
                    target: place,
                },
            )
            .collect(),
        plan,
    }
}

#[test]
fn checked_content_plan_lowers_without_arena_local_identity() {
    let plan = source_plan();
    let lowered = lower_content_conservation_plan(&plan).expect("lowered conservation");

    assert_eq!(lowered.source_fingerprint, plan.fingerprint);
    assert_eq!(
        lowered.structural_places,
        vec![
            StructuralPlaceDeclaration {
                id: PlaceId::new(1).expect("parameter place"),
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: PlaceId::new(RESULT_STRUCTURAL_PLACE_ID).expect("result place"),
                kind: StructuralPlaceKind::Result,
            },
        ]
    );
    let structural_places = lowered
        .structural_places
        .iter()
        .map(|place| (place.id, place.kind))
        .collect();
    let Proposition::ContentConservation(conservation) = &lowered.proposition else {
        panic!("content proposition")
    };
    assert_eq!(
        psi_core::content_conservation_fingerprint(conservation, &structural_places),
        Some(plan.fingerprint),
        "terminal reconstruction must preserve the checked-plan identity preimage"
    );

    let Proposition::ContentConservation(conservation) = lowered.proposition else {
        panic!("content proposition")
    };
    assert_eq!(
        conservation.algebra(),
        &ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: "Address".to_owned(),
        }
    );
    let ContentTerm::Projection {
        projection,
        subject,
    } = conservation.left()
    else {
        panic!("entry projection")
    };
    assert_eq!(projection.domain.get(), 9);
    assert_eq!(projection.projection_fingerprint, 0xfeed);
    assert_eq!(subject.version, ContentPlaceVersion::Entry);
    assert_eq!(subject.root.get(), 1);
    assert!(subject.segments.is_empty());
    let ContentTerm::Separate(parts) = conservation.right() else {
        panic!("separated result")
    };
    assert_eq!(parts.len(), 2);
    assert!(matches!(
        &parts[0],
        ContentTerm::Projection { subject, .. }
            if subject.segments == [ContentPlaceSegment::Field("left".to_owned())]
    ));
    assert!(matches!(
        &parts[1],
        ContentTerm::Projection { subject, .. }
            if subject.segments == [ContentPlaceSegment::Field("right".to_owned())]
    ));
}

#[test]
fn checked_crash_frontier_maps_only_through_dense_terminal_claims() {
    let first = PermissionClaimIdentity::Established {
        machine_symbol: SymbolHandle::from_arena_index(1),
        state_symbol: SymbolHandle::from_arena_index(2),
        source: PermissionEventSource::StateEntry,
        ordinal: 0,
    };
    let second = PermissionClaimIdentity::Established {
        machine_symbol: SymbolHandle::from_arena_index(1),
        state_symbol: SymbolHandle::from_arena_index(2),
        source: PermissionEventSource::Statement { statement_index: 1 },
        ordinal: 1,
    };
    let first_id = ClaimId::new(1).expect("claim");
    let second_id = ClaimId::new(2).expect("claim");
    assert_eq!(
        lower_checked_crash_frontier(&[first, second], &[(second, second_id), (first, first_id)],),
        Ok(vec![first_id, second_id])
    );

    let missing = PermissionClaimIdentity::Established {
        machine_symbol: SymbolHandle::from_arena_index(1),
        state_symbol: SymbolHandle::from_arena_index(2),
        source: PermissionEventSource::Statement { statement_index: 2 },
        ordinal: 2,
    };
    assert_eq!(
        lower_checked_crash_frontier(&[missing], &[(first, first_id)]),
        Err(LoweringError::CrashFrontierClaimNotLowered(missing)),
        "terminal production must not silently omit a checked abandoned claim"
    );
}

#[test]
fn checked_partition_composition_lowers_with_exact_source_and_dense_claims() {
    let identity = identity_fact(SemanticDomainId(9), "left", 1);
    let mut identities =
        lower_content_identity_reshuffles(&[identity]).expect("identity fact lowers");
    let fact = partition_composition_fact();
    let lowered =
        lower_content_partition_compositions(std::slice::from_ref(&fact), &mut identities)
            .expect("exact theorem substitution lowers");

    assert_eq!(lowered.compositions.len(), 1);
    let row = &lowered.compositions[0];
    assert_eq!(
        row.producer_coordinate,
        SourceCallCoordinate {
            state: fact.state_symbol,
            statement_index: fact.statement_index,
            call_ordinal: fact.call_ordinal,
        }
    );
    assert_eq!(row.source_callable, fact.source_callable);
    assert_eq!(row.source_fingerprint, fact.source_fingerprint);
    assert_eq!(row.input_claims, vec![ClaimId::new(1).expect("claim")]);
    assert_eq!(row.substitutions.len(), 3);
    assert_eq!(row.source, row.derived);

    let mut staged = fact.clone();
    let source = staged.substitutions[0].source.clone();
    let target = staged.substitutions[0].target.clone();
    staged
        .result_rewrites
        .push(psi_checked_trees::ContentPartitionResultRewrite {
            claim_identity: identity_fact(SemanticDomainId(9), "left", 2).claim_identity,
            source,
            target,
        });
    let identities_before_error = identities.clone();
    assert_eq!(
        lower_content_partition_compositions(&[staged], &mut identities),
        Err(LoweringError::ContentPartitionResultRewriteUnsupported)
    );
    assert_eq!(identities, identities_before_error);

    let mut derived_source = fact.clone();
    derived_source.source_derivation_depth = 1;
    assert_eq!(
        lower_content_partition_compositions(&[derived_source], &mut identities),
        Err(LoweringError::ContentPartitionDerivedSourceUnsupported)
    );

    let mut drifted = fact;
    let projection = drifted.plan.equation.left().clone();
    drifted.plan.equation = ContentConservationEquation::new(
        projection.clone(),
        CheckedContentConservationTerm::separate([projection.clone(), projection]),
    );
    assert_eq!(
        lower_content_partition_compositions(&[drifted], &mut identities),
        Err(LoweringError::ContentPartitionReplayMismatch)
    );
}

#[test]
fn checked_partition_composition_lowers_a_partition_only_entry_claim() {
    let fact = partition_composition_fact();
    let mut identities =
        lower_content_identity_reshuffles(&[]).expect("empty identity evidence lowers");
    let lowered =
        lower_content_partition_compositions(std::slice::from_ref(&fact), &mut identities)
            .expect("partition input binding lowers independently of output equality");

    assert!(identities.reshuffles.is_empty());
    assert_eq!(identities.entry_claims.len(), 1);
    assert_eq!(
        identities.entry_claims[0].claim,
        ClaimId::new(1).expect("dense claim")
    );
    assert_eq!(
        identities.entry_claims[0].input.version,
        ContentPlaceVersion::Entry
    );
    assert_eq!(
        lowered.compositions[0].input_claims,
        vec![ClaimId::new(1).expect("dense claim")]
    );
}

#[test]
fn checked_content_plan_fails_closed_on_corrupt_identity() {
    let mut plan = source_plan();
    plan.fingerprint ^= 1;
    assert!(matches!(
        lower_content_conservation_plan(&plan),
        Err(LoweringError::ContentConservationFingerprintMismatch { .. })
    ));

    let plan = source_plan_with_domain(SemanticDomainId::NULL);
    assert_eq!(
        lower_content_conservation_plan(&plan),
        Err(LoweringError::InvalidContentDomainIdentity)
    );
}

#[test]
fn checked_identity_facts_group_exact_projections_into_canonical_terminal_rows() {
    let first = identity_fact(SemanticDomainId(9), "payload", 0);
    let second = identity_fact(SemanticDomainId(10), "payload", 0);
    let lowered = lower_content_identity_reshuffles(&[second.clone(), first.clone()])
        .expect("exact checked identity facts lower");
    let reordered = lower_content_identity_reshuffles(&[first, second])
        .expect("source fact order is irrelevant");

    assert_eq!(lowered, reordered);
    assert_eq!(lowered.structural_places.len(), 2);
    assert_eq!(lowered.reshuffles.len(), 1);
    let row = &lowered.reshuffles[0];
    assert_eq!(row.claim, ClaimId::new(1).expect("dense claim"));
    assert_eq!(row.input.version, ContentPlaceVersion::Entry);
    assert_eq!(row.input.root, PlaceId::new(1).expect("parameter root"));
    assert_eq!(row.output.version, ContentPlaceVersion::Current);
    assert_eq!(
        row.output.root,
        PlaceId::new(RESULT_STRUCTURAL_PLACE_ID).expect("result root")
    );
    assert_eq!(
        row.output.segments,
        [ContentPlaceSegment::Field("payload".to_owned())]
    );
    assert_eq!(
        row.projections
            .iter()
            .map(|projection| projection.projection.domain.get())
            .collect::<Vec<_>>(),
        vec![9, 10]
    );
    assert_eq!(row.inferred_propositions().count(), 2);
}

#[test]
fn checked_identity_fact_lowers_stable_sum_case_paths_without_arena_identity() {
    let mut fact = identity_fact(SemanticDomainId(9), "unused", 0);
    fact.plan = case_direct_source_plan(SemanticDomainId(9));

    let lowered =
        lower_content_identity_reshuffles(&[fact]).expect("sum-case identity fact lowers");
    let [row] = lowered.reshuffles.as_slice() else {
        panic!("one terminal reshuffle row");
    };
    let expected = [
        ContentPlaceSegment::Case("Present".to_owned()),
        ContentPlaceSegment::Field("region".to_owned()),
    ];
    assert_eq!(row.input.segments, expected);
    assert_eq!(row.output.segments, expected);
}

#[test]
fn checked_identity_fact_lowering_revalidates_claim_and_equation_shape() {
    let mut unknown = identity_fact(SemanticDomainId(9), "payload", 0);
    unknown.claim_identity = PermissionClaimIdentity::Unknown;
    assert_eq!(
        lower_content_identity_reshuffles(&[unknown]),
        Err(LoweringError::UnknownContentClaimIdentity)
    );

    let mut partition = identity_fact(SemanticDomainId(9), "payload", 0);
    partition.plan = source_plan();
    assert_eq!(
        lower_content_identity_reshuffles(&[partition]),
        Err(LoweringError::ContentIdentityNotDirectEquality)
    );

    let mut moved_twice = identity_fact(SemanticDomainId(9), "left", 0);
    let second_destination = identity_fact(SemanticDomainId(10), "right", 0);
    assert_eq!(
        lower_content_identity_reshuffles(&[moved_twice.clone(), second_destination]),
        Err(LoweringError::ContentIdentityClaimMapsMultiplePlaces)
    );

    moved_twice.plan.owner = SymbolHandle::from_arena_index(99);
    assert_eq!(
        lower_content_identity_reshuffles(&[moved_twice]),
        Err(LoweringError::ContentIdentityFactOwnerMismatch)
    );
}
