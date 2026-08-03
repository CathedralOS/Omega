use psi_core::{
    BlockId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    ContentTerm, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, PlaceId, Proposition, PropositionError, ScalarTerm,
    ScalarType, StructuralPlaceKind, ValueId,
};
use psi_proof_kernel::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemVersion,
};
use psi_terminal::{
    Block, ClaimContentProjection, ContentIdentityReshuffle, ContentPartitionComposition,
    ContentPlaceSubstitution, ContractClause, MachineContract, Operation, OperationKind,
    SemanticVersion, StructuralPlaceDeclaration, TerminalMachine, TerminalModule, Terminator,
    ValueDeclaration,
};
use psi_terminal_verifier::{
    ContractClauseKind, ModuleError, ObligationEvidence, ProofBundle, VerificationError,
    validate_module, verify_module,
};

#[test]
fn straight_line_integer_contract_is_reconstructed_and_verified() {
    let fixture = Fixture::new();
    let verified = verify_module(
        &fixture.module,
        &fixture.proof_bundle(),
        &AdmissionProfile::default(),
    )
    .expect("terminal module verifies independently of producer state");

    assert_eq!(verified.module(), &fixture.module);
    assert_eq!(verified.accepted_facts().len(), 1);
    assert_eq!(verified.accepted_facts()[0].obligation, fixture.obligation);
}

#[test]
fn verifier_reconstructs_every_contract_obligation() {
    let fixture = Fixture::new();
    assert_eq!(
        verify_module(
            &fixture.module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect_err("proof bundle cannot omit a bodyful guarantee"),
        VerificationError::MissingEvidence(fixture.obligation)
    );
}

#[test]
fn v2_boolean_constant_axiom_proves_the_return_contract() {
    let constant = ValueId::new(10).expect("constant");
    let result = ValueId::new(11).expect("result");
    let obligation = ObligationId::new(10).expect("obligation");
    let term = |id| ScalarTerm::value(id, ScalarType::Boolean);
    let goal = Proposition::Equal(term(result), ScalarTerm::boolean(true));
    let module = TerminalModule {
        semantic_version: SemanticVersion::V2,
        entry: MachineId::new(10).expect("machine"),
        machines: vec![TerminalMachine {
            id: MachineId::new(10).expect("machine"),
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Boolean,
            },
            structural_places: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(10).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(10).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(10).expect("operation"),
                    result: ValueDeclaration {
                        id: constant,
                        scalar_type: ScalarType::Boolean,
                    },
                    kind: OperationKind::BooleanConstant { value: true },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(10).expect("edge"),
                    value: constant,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(10).expect("contract"),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal.clone(),
                }],
            },
        }],
    };
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(result), term(constant)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(constant), ScalarTerm::boolean(true)),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(10).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v2 Boolean semantics should reconstruct both axioms");
}

#[test]
fn v9_content_conservation_accepts_a_replaceable_certificate() {
    let (module, goal, obligation) = reflexive_content_module();
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(80).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                },
            }),
        }],
    };

    let verified = verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v9 content proposition and certificate");
    assert_eq!(verified.accepted_facts().len(), 1);
}

#[test]
fn v10_identity_reshuffle_reconstructs_content_equality_as_a_semantic_axiom() {
    let (module, goal, obligation) = identity_reshuffle_module();
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(90).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::SemanticAxiom { index: 0 },
                },
            }),
        }],
    };

    let verified = verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("an exact identity reshuffle should establish its content equality");
    assert_eq!(verified.accepted_facts().len(), 1);
}

#[test]
fn v11_sum_case_identity_reshuffle_reconstructs_content_equality() {
    let (mut module, _, obligation) = identity_reshuffle_module();
    module.semantic_version = SemanticVersion::V11;
    let segments = vec![
        ContentPlaceSegment::Case("Present".to_owned()),
        ContentPlaceSegment::Field("region".to_owned()),
    ];
    let reshuffle = &mut module.machines[0].content_identity_reshuffles[0];
    reshuffle.input.segments = segments.clone();
    reshuffle.output.segments = segments;
    let goal = reshuffle
        .inferred_propositions()
        .next()
        .expect("one projection yields one proposition");
    module.machines[0].contract.ensures[0].proposition = goal.clone();
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(91).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::SemanticAxiom { index: 0 },
                },
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("a v11 case-plus-field reshuffle should establish its content equality");
}

#[test]
fn v12_partition_composition_replays_an_authored_theorem_as_a_semantic_axiom() {
    let (module, goal, obligation) = partition_composition_module();
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(92).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::SemanticAxiom { index: 1 },
                },
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("an exact v12 theorem substitution should be reconstructed");
}

#[test]
fn v12_partition_composition_rejects_old_versions_and_theorem_drift() {
    let (mut old, _, _) = partition_composition_module();
    old.semantic_version = SemanticVersion::V11;
    assert!(matches!(
        validate_module(&old),
        Err(
            ModuleError::ContentPartitionCompositionsRequireSemanticVersion {
                required: SemanticVersion::V12,
                actual: SemanticVersion::V11,
                ..
            }
        )
    ));

    let (mut drifted, _, _) = partition_composition_module();
    let composition = &mut drifted.machines[0].content_partition_compositions[0];
    let ContentTerm::Separate(children) = composition.derived.right().clone() else {
        panic!("fixture has a separated result")
    };
    composition.derived = ContentConservation::new(
        composition.derived.algebra().clone(),
        composition.derived.left().clone(),
        children[0].clone(),
    );
    assert_eq!(
        validate_module(&drifted).expect_err("the derived equation must replay exactly"),
        ModuleError::ContentPartitionReplayMismatch
    );
}

#[test]
fn sum_case_content_paths_require_v11_and_nonempty_case_names() {
    let (mut old_version, _, _) = identity_reshuffle_module();
    old_version.machines[0].content_identity_reshuffles[0]
        .input
        .segments = vec![ContentPlaceSegment::Case("Present".to_owned())];
    assert!(matches!(
        validate_module(&old_version),
        Err(
            ModuleError::ContentIdentityCasePathRequiresSemanticVersion {
                required: SemanticVersion::V11,
                actual: SemanticVersion::V10,
                ..
            }
        )
    ));

    let (mut empty, _, _) = identity_reshuffle_module();
    empty.semantic_version = SemanticVersion::V11;
    empty.machines[0].content_identity_reshuffles[0]
        .input
        .segments = vec![ContentPlaceSegment::Case(String::new())];
    assert_eq!(
        validate_module(&empty).expect_err("case spellings are semantic identity"),
        ModuleError::MalformedProposition(PropositionError::EmptyContentCaseName)
    );
}

#[test]
fn v10_identity_reshuffles_fail_closed_when_malformed() {
    let (mut old_version, _, _) = identity_reshuffle_module();
    old_version.semantic_version = SemanticVersion::V9;
    assert!(matches!(
        validate_module(&old_version),
        Err(
            ModuleError::ContentIdentityReshufflesRequireSemanticVersion {
                required: SemanticVersion::V10,
                actual: SemanticVersion::V9,
                ..
            }
        )
    ));

    let (mut empty, _, _) = identity_reshuffle_module();
    empty.machines[0].content_identity_reshuffles[0]
        .projections
        .clear();
    assert_eq!(
        validate_module(&empty).expect_err("a claim must preserve named content"),
        ModuleError::ContentIdentityReshuffleHasNoProjections(ClaimId::new(90).expect("claim"))
    );

    let (mut wrong_input, _, _) = identity_reshuffle_module();
    wrong_input.machines[0].content_identity_reshuffles[0]
        .input
        .version = ContentPlaceVersion::Current;
    assert_eq!(
        validate_module(&wrong_input).expect_err("input must denote parameter entry content"),
        ModuleError::ContentIdentityReshuffleRequiresEntryParameter(
            ClaimId::new(90).expect("claim")
        )
    );

    let (mut duplicate, _, _) = identity_reshuffle_module();
    let mut second = duplicate.machines[0].content_identity_reshuffles[0].clone();
    second.claim = ClaimId::new(91).expect("second claim");
    second.output.segments = vec![ContentPlaceSegment::Field("second".to_owned())];
    duplicate.machines[0]
        .content_identity_reshuffles
        .push(second);
    assert!(matches!(
        validate_module(&duplicate),
        Err(ModuleError::DuplicateContentIdentityInput(_))
    ));

    let (mut overlapping_input, _, _) = identity_reshuffle_module();
    overlapping_input.machines[0].content_identity_reshuffles[0]
        .input
        .segments = vec![ContentPlaceSegment::Field("payload".to_owned())];
    let mut child = overlapping_input.machines[0].content_identity_reshuffles[0].clone();
    child.claim = ClaimId::new(91).expect("second claim");
    child
        .input
        .segments
        .push(ContentPlaceSegment::Field("child".to_owned()));
    child.output.segments = vec![ContentPlaceSegment::Field("second".to_owned())];
    overlapping_input.machines[0]
        .content_identity_reshuffles
        .push(child);
    assert!(matches!(
        validate_module(&overlapping_input),
        Err(ModuleError::OverlappingContentIdentityInput { .. })
    ));

    let (mut overlapping_output, _, _) = identity_reshuffle_module();
    overlapping_output.machines[0].content_identity_reshuffles[0]
        .input
        .segments = vec![ContentPlaceSegment::Field("first".to_owned())];
    overlapping_output.machines[0].content_identity_reshuffles[0]
        .output
        .segments = vec![ContentPlaceSegment::Field("payload".to_owned())];
    let mut child = overlapping_output.machines[0].content_identity_reshuffles[0].clone();
    child.claim = ClaimId::new(91).expect("second claim");
    child.input.segments = vec![ContentPlaceSegment::Field("second".to_owned())];
    child
        .output
        .segments
        .push(ContentPlaceSegment::Field("child".to_owned()));
    overlapping_output.machines[0]
        .content_identity_reshuffles
        .push(child);
    assert!(matches!(
        validate_module(&overlapping_output),
        Err(ModuleError::OverlappingContentIdentityOutput { .. })
    ));

    let (mut mismatched_algebra, _, _) = identity_reshuffle_module();
    mismatched_algebra.machines[0].content_identity_reshuffles[0]
        .input
        .segments = vec![ContentPlaceSegment::Field("first".to_owned())];
    mismatched_algebra.machines[0].content_identity_reshuffles[0]
        .output
        .segments = vec![ContentPlaceSegment::Field("first".to_owned())];
    let mut second = mismatched_algebra.machines[0].content_identity_reshuffles[0].clone();
    second.claim = ClaimId::new(91).expect("second claim");
    second.input.segments = vec![ContentPlaceSegment::Field("second".to_owned())];
    second.output.segments = vec![ContentPlaceSegment::Field("second".to_owned())];
    second.projections[0].algebra.parameter = "Element".to_owned();
    mismatched_algebra.machines[0]
        .content_identity_reshuffles
        .push(second);
    assert_eq!(
        validate_module(&mismatched_algebra)
            .expect_err("one projection identity cannot name two algebras"),
        ModuleError::ContentProjectionAlgebraMismatch(ContentProjectionIdentity {
            domain: ContentDomainId::new(90).expect("content domain"),
            projection_fingerprint: 0x9055,
        })
    );
}

fn identity_reshuffle_module() -> (TerminalModule, Proposition, ObligationId) {
    let parameter = ValueId::new(90).expect("parameter");
    let result = ValueId::new(91).expect("result");
    let input_root = PlaceId::new(90).expect("input place");
    let output_root = PlaceId::new(91).expect("output place");
    let reshuffle = ContentIdentityReshuffle {
        claim: ClaimId::new(90).expect("claim"),
        input: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: input_root,
            segments: Vec::new(),
        },
        output: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: output_root,
            segments: Vec::new(),
        },
        projections: vec![ClaimContentProjection {
            projection: ContentProjectionIdentity {
                domain: ContentDomainId::new(90).expect("content domain"),
                projection_fingerprint: 0x9055,
            },
            algebra: ContentAlgebra {
                kind: ContentAlgebraKind::CountedQuantity,
                parameter: "Byte".to_owned(),
            },
        }],
    };
    let goal = reshuffle
        .inferred_propositions()
        .next()
        .expect("one projection yields one proposition");
    let obligation = ObligationId::new(90).expect("obligation");
    let machine = TerminalMachine {
        id: MachineId::new(90).expect("machine"),
        parameters: vec![ValueDeclaration {
            id: parameter,
            scalar_type: ScalarType::Boolean,
        }],
        result: ValueDeclaration {
            id: result,
            scalar_type: ScalarType::Boolean,
        },
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: input_root,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: output_root,
                kind: StructuralPlaceKind::Result,
            },
        ],
        content_identity_reshuffles: vec![reshuffle],
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(90).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(90).expect("block"),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: EdgeId::new(90).expect("edge"),
                value: parameter,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(90).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V10,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

fn partition_composition_module() -> (TerminalModule, Proposition, ObligationId) {
    let (mut module, _, obligation) = identity_reshuffle_module();
    module.semantic_version = SemanticVersion::V12;
    let machine = &mut module.machines[0];
    let input_root = machine.content_identity_reshuffles[0].input.root;
    let result_root = machine.content_identity_reshuffles[0].output.root;
    let projection = machine.content_identity_reshuffles[0].projections[0].projection;
    let algebra = machine.content_identity_reshuffles[0].projections[0]
        .algebra
        .clone();
    let source_input_root = PlaceId::new(190).expect("source input");
    let source_result_root = PlaceId::new(191).expect("source result");
    let place = |version, root, segments| ContentStructuralPlace {
        version,
        root,
        segments,
    };
    let term = |subject| ContentTerm::Projection {
        projection,
        subject,
    };
    let source_input = place(ContentPlaceVersion::Entry, source_input_root, Vec::new());
    let source_left = place(
        ContentPlaceVersion::Current,
        source_result_root,
        vec![ContentPlaceSegment::Field("left".to_owned())],
    );
    let source_right = place(
        ContentPlaceVersion::Current,
        source_result_root,
        vec![ContentPlaceSegment::Field("right".to_owned())],
    );
    let target_input = place(ContentPlaceVersion::Entry, input_root, Vec::new());
    let target_left = place(
        ContentPlaceVersion::Current,
        result_root,
        vec![ContentPlaceSegment::Field("left".to_owned())],
    );
    let target_right = place(
        ContentPlaceVersion::Current,
        result_root,
        vec![ContentPlaceSegment::Field("right".to_owned())],
    );
    machine.content_identity_reshuffles[0].output = target_left.clone();
    let source = ContentConservation::new(
        algebra.clone(),
        term(source_input.clone()),
        ContentTerm::separate([term(source_left.clone()), term(source_right.clone())])
            .expect("separated source"),
    );
    let derived = ContentConservation::new(
        algebra,
        term(target_input.clone()),
        ContentTerm::separate([term(target_left.clone()), term(target_right.clone())])
            .expect("separated target"),
    );
    let mut substitutions = vec![
        ContentPlaceSubstitution {
            source: source_input,
            target: target_input,
        },
        ContentPlaceSubstitution {
            source: source_left,
            target: target_left,
        },
        ContentPlaceSubstitution {
            source: source_right,
            target: target_right,
        },
    ];
    substitutions.sort();
    machine.content_partition_compositions = vec![ContentPartitionComposition {
        source_fingerprint: 0xfeed_face_dead_beef,
        source_structural_places: vec![
            StructuralPlaceDeclaration {
                id: source_input_root,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: source_result_root,
                kind: StructuralPlaceKind::Result,
            },
        ],
        source,
        input_claims: vec![machine.content_identity_reshuffles[0].claim],
        substitutions,
        derived: derived.clone(),
    }];
    let goal = Proposition::ContentConservation(derived);
    machine.contract.ensures[0].proposition = goal.clone();
    (module, goal, obligation)
}

#[test]
fn content_conservation_is_v9_ensures_only_and_entry_cannot_name_result() {
    let (mut old_module, _, _) = reflexive_content_module();
    old_module.semantic_version = SemanticVersion::V8;
    old_module.machines[0].structural_places.clear();
    assert_eq!(
        validate_module(&old_module).expect_err("content propositions require v9"),
        ModuleError::PropositionRequiresSemanticVersion {
            contract: ContractId::new(80).expect("contract"),
            clause: ContractClauseKind::Ensures,
            required: SemanticVersion::V9,
            actual: SemanticVersion::V8,
        }
    );

    let (mut requires_module, goal, _) = reflexive_content_module();
    requires_module.machines[0].contract.ensures.clear();
    requires_module.machines[0].contract.requires.push(goal);
    assert_eq!(
        validate_module(&requires_module).expect_err("content is post-state only"),
        ModuleError::ContentConservationRequiresEnsures {
            contract: ContractId::new(80).expect("contract"),
        }
    );

    let (mut result_entry_module, _, _) = reflexive_content_module();
    result_entry_module.machines[0].structural_places[0].kind = StructuralPlaceKind::Result;
    assert_eq!(
        validate_module(&result_entry_module).expect_err("result has no entry version"),
        ModuleError::MalformedProposition(PropositionError::EntryResultStructuralPlace(
            PlaceId::new(80).expect("place")
        ))
    );

    let (mut duplicate_parameter, _, _) = reflexive_content_module();
    duplicate_parameter.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: PlaceId::new(81).expect("second place"),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        });
    assert_eq!(
        validate_module(&duplicate_parameter)
            .expect_err("one parameter position has one structural root"),
        ModuleError::DuplicateStructuralPlaceRoot {
            machine: MachineId::new(80).expect("machine"),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        }
    );
}

fn reflexive_content_module() -> (TerminalModule, Proposition, ObligationId) {
    let parameter = ValueId::new(80).expect("parameter");
    let result = ValueId::new(81).expect("result");
    let place = PlaceId::new(80).expect("place");
    let subject = ContentTerm::Projection {
        projection: ContentProjectionIdentity {
            domain: ContentDomainId::new(80).expect("content domain"),
            projection_fingerprint: 0x8055,
        },
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: place,
            segments: Vec::new(),
        },
    };
    let goal = Proposition::ContentConservation(ContentConservation::new(
        ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "Byte".to_owned(),
        },
        subject.clone(),
        subject,
    ));
    let obligation = ObligationId::new(80).expect("obligation");
    let machine = TerminalMachine {
        id: MachineId::new(80).expect("machine"),
        parameters: vec![ValueDeclaration {
            id: parameter,
            scalar_type: ScalarType::Boolean,
        }],
        result: ValueDeclaration {
            id: result,
            scalar_type: ScalarType::Boolean,
        },
        structural_places: vec![StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(80).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(80).expect("block"),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: EdgeId::new(80).expect("edge"),
                value: parameter,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(80).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V9,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

#[test]
fn v3_wrapping_add_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = wrapping_add_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let sum = ScalarTerm::wrapping_integer_add(integer, term(20), term(21)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(23), term(22)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(22), sum),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(20).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v3 wrapping-add semantics should reconstruct both axioms");
}

#[test]
fn wrapping_add_requires_v3_and_defined_exact_type_operands() {
    let (mut old_version, _, _) = wrapping_add_module();
    old_version.semantic_version = SemanticVersion::V2;
    assert!(matches!(
        validate_module(&old_version),
        Err(ModuleError::OperationRequiresSemanticVersion {
            required: SemanticVersion::V3,
            actual: SemanticVersion::V2,
            ..
        })
    ));

    let (mut use_before_definition, _, _) = wrapping_add_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::WrappingIntegerAdd {
            left: ValueId::new(22).expect("sum result"),
            right: ValueId::new(21).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(22).expect("sum result"))
    );

    let (mut wrong_type, _, _) = wrapping_add_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::WrappingIntegerAddOperandTypeMismatch {
            operation: OperationId::new(20).expect("add operation"),
            operand: ValueId::new(21).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn v4_saturating_add_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = saturating_add_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let sum = ScalarTerm::saturating_integer_add(integer, term(30), term(31)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(33), term(32)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(32), sum),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(30).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v4 saturating-add semantics should reconstruct both axioms");
}

#[test]
fn saturating_add_requires_v4_and_defined_exact_type_operands() {
    let (mut old_version, _, _) = saturating_add_module();
    old_version.semantic_version = SemanticVersion::V3;
    assert!(matches!(
        validate_module(&old_version),
        Err(ModuleError::OperationRequiresSemanticVersion {
            required: SemanticVersion::V4,
            actual: SemanticVersion::V3,
            ..
        })
    ));

    let (mut use_before_definition, _, _) = saturating_add_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::SaturatingIntegerAdd {
            left: ValueId::new(32).expect("sum result"),
            right: ValueId::new(31).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(32).expect("sum result"))
    );

    let (mut wrong_type, _, _) = saturating_add_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::SaturatingIntegerAddOperandTypeMismatch {
            operation: OperationId::new(30).expect("add operation"),
            operand: ValueId::new(31).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn v5_wrapping_subtract_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = wrapping_subtract_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let difference = ScalarTerm::wrapping_integer_subtract(integer, term(40), term(41)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(43), term(42)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(42), difference),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(40).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v5 wrapping-subtract semantics should reconstruct both axioms");
}

#[test]
fn wrapping_subtract_requires_v5_and_defined_exact_type_operands() {
    let (mut old_version, _, _) = wrapping_subtract_module();
    old_version.semantic_version = SemanticVersion::V4;
    assert!(matches!(
        validate_module(&old_version),
        Err(ModuleError::OperationRequiresSemanticVersion {
            required: SemanticVersion::V5,
            actual: SemanticVersion::V4,
            ..
        })
    ));

    let (mut use_before_definition, _, _) = wrapping_subtract_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::WrappingIntegerSubtract {
            left: ValueId::new(42).expect("difference result"),
            right: ValueId::new(41).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(42).expect("difference result"))
    );

    let (mut wrong_type, _, _) = wrapping_subtract_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::WrappingIntegerSubtractOperandTypeMismatch {
            operation: OperationId::new(40).expect("subtract operation"),
            operand: ValueId::new(41).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn v6_saturating_subtract_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = saturating_subtract_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let difference = ScalarTerm::saturating_integer_subtract(integer, term(50), term(51)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(53), term(52)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(52), difference),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(50).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v6 saturating-subtract semantics should reconstruct both axioms");
}

#[test]
fn saturating_subtract_requires_v6_and_defined_exact_type_operands() {
    let (mut old_version, _, _) = saturating_subtract_module();
    old_version.semantic_version = SemanticVersion::V5;
    assert!(matches!(
        validate_module(&old_version),
        Err(ModuleError::OperationRequiresSemanticVersion {
            required: SemanticVersion::V6,
            actual: SemanticVersion::V5,
            ..
        })
    ));

    let (mut use_before_definition, _, _) = saturating_subtract_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::SaturatingIntegerSubtract {
            left: ValueId::new(52).expect("difference result"),
            right: ValueId::new(51).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(52).expect("difference result"))
    );

    let (mut wrong_type, _, _) = saturating_subtract_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::SaturatingIntegerSubtractOperandTypeMismatch {
            operation: OperationId::new(50).expect("subtract operation"),
            operand: ValueId::new(51).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn v7_wrapping_multiply_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = wrapping_multiply_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let product = ScalarTerm::wrapping_integer_multiply(integer, term(60), term(61)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(63), term(62)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(62), product),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(60).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v7 wrapping-multiply semantics should reconstruct both axioms");
}

#[test]
fn wrapping_multiply_requires_v7_and_defined_exact_type_operands() {
    let (mut old_version, _, _) = wrapping_multiply_module();
    old_version.semantic_version = SemanticVersion::V6;
    assert!(matches!(
        validate_module(&old_version),
        Err(ModuleError::OperationRequiresSemanticVersion {
            required: SemanticVersion::V7,
            actual: SemanticVersion::V6,
            ..
        })
    ));

    let (mut use_before_definition, _, _) = wrapping_multiply_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::WrappingIntegerMultiply {
            left: ValueId::new(62).expect("product result"),
            right: ValueId::new(61).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(62).expect("product result"))
    );

    let (mut wrong_type, _, _) = wrapping_multiply_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::WrappingIntegerMultiplyOperandTypeMismatch {
            operation: OperationId::new(60).expect("multiply operation"),
            operand: ValueId::new(61).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn v8_saturating_multiply_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = saturating_multiply_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let product = ScalarTerm::saturating_integer_multiply(integer, term(70), term(71)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(73), term(72)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(72), product),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(70).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v8 saturating-multiply semantics should reconstruct both axioms");
}

#[test]
fn saturating_multiply_requires_v8_and_defined_exact_type_operands() {
    let (mut old_version, _, _) = saturating_multiply_module();
    old_version.semantic_version = SemanticVersion::V7;
    assert!(matches!(
        validate_module(&old_version),
        Err(ModuleError::OperationRequiresSemanticVersion {
            required: SemanticVersion::V8,
            actual: SemanticVersion::V7,
            ..
        })
    ));

    let (mut use_before_definition, _, _) = saturating_multiply_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::SaturatingIntegerMultiply {
            left: ValueId::new(72).expect("product result"),
            right: ValueId::new(71).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(72).expect("product result"))
    );

    let (mut wrong_type, _, _) = saturating_multiply_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::SaturatingIntegerMultiplyOperandTypeMismatch {
            operation: OperationId::new(70).expect("multiply operation"),
            operand: ValueId::new(71).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn initial_control_vocabulary_rejects_unreachable_semantic_axioms() {
    let mut fixture = Fixture::new();
    fixture.module.machines[0].blocks.push(Block {
        id: BlockId::new(3).expect("unreachable block"),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Return {
            edge: EdgeId::new(3).expect("unreachable edge"),
            value: fixture.constant,
        },
    });
    assert_eq!(
        validate_module(&fixture.module).expect_err("unreachable facts must not enter proofs"),
        ModuleError::UnreachableBlock(BlockId::new(3).expect("unreachable block"))
    );
}

#[test]
fn entry_requirements_cannot_assume_an_internal_operation_value() {
    let mut fixture = Fixture::new();
    fixture.module.machines[0]
        .contract
        .requires
        .push(Proposition::Equal(
            ScalarTerm::value(fixture.constant, ScalarType::Integer(fixture.integer)),
            ScalarTerm::integer(fixture.integer, IntegerValue::Signed(7)).expect("seven"),
        ));
    assert_eq!(
        validate_module(&fixture.module).expect_err("internal facts do not exist at entry"),
        ModuleError::ContractValueOutsideScope {
            contract: ContractId::new(1).expect("contract"),
            clause: ContractClauseKind::Requires,
            value: fixture.constant,
        }
    );
}

fn wrapping_add_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(20).expect("left parameter");
    let right = ValueId::new(21).expect("right parameter");
    let sum = ValueId::new(22).expect("sum result");
    let result = ValueId::new(23).expect("machine result");
    let obligation = ObligationId::new(20).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_add(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        id: MachineId::new(20).expect("machine"),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        structural_places: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(20).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(20).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(20).expect("add operation"),
                result: ValueDeclaration {
                    id: sum,
                    scalar_type,
                },
                kind: OperationKind::WrappingIntegerAdd { left, right },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(20).expect("return edge"),
                value: sum,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(20).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V3,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

fn saturating_add_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(30).expect("left parameter");
    let right = ValueId::new(31).expect("right parameter");
    let sum = ValueId::new(32).expect("sum result");
    let result = ValueId::new(33).expect("machine result");
    let obligation = ObligationId::new(30).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::saturating_integer_add(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        id: MachineId::new(30).expect("machine"),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        structural_places: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(30).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(30).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(30).expect("add operation"),
                result: ValueDeclaration {
                    id: sum,
                    scalar_type,
                },
                kind: OperationKind::SaturatingIntegerAdd { left, right },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(30).expect("return edge"),
                value: sum,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(30).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V4,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

fn wrapping_subtract_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(40).expect("left parameter");
    let right = ValueId::new(41).expect("right parameter");
    let difference = ValueId::new(42).expect("difference result");
    let result = ValueId::new(43).expect("machine result");
    let obligation = ObligationId::new(40).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_subtract(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        id: MachineId::new(40).expect("machine"),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        structural_places: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(40).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(40).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(40).expect("subtract operation"),
                result: ValueDeclaration {
                    id: difference,
                    scalar_type,
                },
                kind: OperationKind::WrappingIntegerSubtract { left, right },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(40).expect("return edge"),
                value: difference,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(40).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V5,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

fn saturating_subtract_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(50).expect("left parameter");
    let right = ValueId::new(51).expect("right parameter");
    let difference = ValueId::new(52).expect("difference result");
    let result = ValueId::new(53).expect("machine result");
    let obligation = ObligationId::new(50).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::saturating_integer_subtract(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        id: MachineId::new(50).expect("machine"),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        structural_places: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(50).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(50).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(50).expect("subtract operation"),
                result: ValueDeclaration {
                    id: difference,
                    scalar_type,
                },
                kind: OperationKind::SaturatingIntegerSubtract { left, right },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(50).expect("return edge"),
                value: difference,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(50).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V6,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

fn wrapping_multiply_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(60).expect("left parameter");
    let right = ValueId::new(61).expect("right parameter");
    let product = ValueId::new(62).expect("product result");
    let result = ValueId::new(63).expect("machine result");
    let obligation = ObligationId::new(60).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_multiply(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        id: MachineId::new(60).expect("machine"),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        structural_places: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(60).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(60).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(60).expect("multiply operation"),
                result: ValueDeclaration {
                    id: product,
                    scalar_type,
                },
                kind: OperationKind::WrappingIntegerMultiply { left, right },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(60).expect("return edge"),
                value: product,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(60).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V7,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

fn saturating_multiply_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(70).expect("left parameter");
    let right = ValueId::new(71).expect("right parameter");
    let product = ValueId::new(72).expect("product result");
    let result = ValueId::new(73).expect("machine result");
    let obligation = ObligationId::new(70).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::saturating_integer_multiply(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        id: MachineId::new(70).expect("machine"),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        structural_places: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(70).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(70).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(70).expect("multiply operation"),
                result: ValueDeclaration {
                    id: product,
                    scalar_type,
                },
                kind: OperationKind::SaturatingIntegerMultiply { left, right },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(70).expect("return edge"),
                value: product,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(70).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V8,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

struct Fixture {
    module: TerminalModule,
    integer: IntegerType,
    constant: ValueId,
    forwarded: ValueId,
    result: ValueId,
    obligation: ObligationId,
}

impl Fixture {
    fn new() -> Self {
        let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let scalar_type = ScalarType::Integer(integer);
        let constant = ValueId::new(1).expect("constant value");
        let forwarded = ValueId::new(2).expect("forwarded value");
        let result = ValueId::new(3).expect("result value");
        let obligation = ObligationId::new(1).expect("ensures obligation");
        let seven = ScalarTerm::integer(integer, IntegerValue::Signed(7)).expect("seven");
        let goal = Proposition::Equal(ScalarTerm::value(result, scalar_type), seven);

        let machine = TerminalMachine {
            id: MachineId::new(1).expect("machine"),
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: result,
                scalar_type,
            },
            structural_places: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).expect("entry block"),
            blocks: vec![
                Block {
                    id: BlockId::new(1).expect("entry block"),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(1).expect("constant operation"),
                        result: ValueDeclaration {
                            id: constant,
                            scalar_type,
                        },
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(7),
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1).expect("jump edge"),
                        target: BlockId::new(2).expect("exit block"),
                        arguments: vec![constant],
                    },
                },
                Block {
                    id: BlockId::new(2).expect("exit block"),
                    parameters: vec![ValueDeclaration {
                        id: forwarded,
                        scalar_type,
                    }],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(2).expect("return edge"),
                        value: forwarded,
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).expect("contract"),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
            },
        };
        Self {
            module: TerminalModule {
                semantic_version: SemanticVersion::CURRENT,
                entry: machine.id,
                machines: vec![machine],
            },
            integer,
            constant,
            forwarded,
            result,
            obligation,
        }
    }

    fn proof_bundle(&self) -> ProofBundle {
        let scalar_type = ScalarType::Integer(self.integer);
        let term = |id| ScalarTerm::value(id, scalar_type);
        let seven = ScalarTerm::integer(self.integer, IntegerValue::Signed(7)).expect("seven");
        let constant_fact = Proposition::Equal(term(self.constant), seven.clone());
        let forwarding_fact = Proposition::Equal(term(self.forwarded), term(self.constant));
        let return_fact = Proposition::Equal(term(self.result), term(self.forwarded));
        let forwarded_is_seven = Proposition::Equal(term(self.forwarded), seven.clone());
        let goal = Proposition::Equal(term(self.result), seven);
        let proof = ProofNode {
            conclusion: goal,
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(ProofNode {
                    conclusion: return_fact,
                    rule: ProofRule::SemanticAxiom { index: 2 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: forwarded_is_seven,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: forwarding_fact,
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: constant_fact,
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                }),
            },
        };
        ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation: self.obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(1).expect("certificate"),
                    proof_system_version: ProofSystemVersion::CURRENT,
                    proof,
                }),
            }],
        }
    }
}
