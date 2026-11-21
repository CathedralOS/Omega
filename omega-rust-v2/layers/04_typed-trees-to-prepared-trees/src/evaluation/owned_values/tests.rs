use super::*;
use crate::reasons::{PreparationReason, PreparationRefusal, RefusalCause};
use omega_parser::NodeKind;
use omega_resolver::{ParsedSource, StageSource, Symbol};
use omega_typer::{resolve_source_package, type_source_package};

fn resolve(source: &str) -> omega_typer::SourcePackage<'_> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    resolve_source_package(&sources).unwrap()
}

fn typed(source: &str) -> TypedTrees<'_> {
    type_source_package(&resolve(source)).unwrap()
}

/// The probe certifies only a declaration whose complete default-domain
/// predicate is the checked literal `true`. It binds the conclusion to one
/// typed constructor and the supplied concrete fields.
fn prove_literal_default_domain<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    constant: &str,
    fields: Arc<[PreparedValue]>,
) -> Option<CheckedDataEstablishment<'trees, 'text>> {
    let request = &trees
        .constants
        .iter()
        .find(|constant_row| constant_row.declaration.path == constant)?
        .initializer;
    prove_literal_construction(trees, request.node, None, fields)
}

fn prove_literal_construction<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    node: omega_resolved::TreeNodeId,
    application: Option<&omega_typed::TypedCall>,
    fields: Arc<[PreparedValue]>,
) -> Option<CheckedDataEstablishment<'trees, 'text>> {
    let obligation = trees
        .obligations
        .iter()
        .find(|obligation| obligation.node == node && matches!(obligation.kind, ObligationKind::EstablishData(_)))?;
    let ObligationKind::EstablishData(owner) = &obligation.kind else {
        return None;
    };
    let tree = trees.trees().nth(node.file)?;
    let construction = tree.construction(node.node)?;
    if &construction.owner != owner || construction.initializers.len() != fields.len() {
        return None;
    }
    let file = tree.resolved().file;
    let declaration = (0..file.nodes.len()).find(|&node| tree.resolved().declaration_at(node) == Some(owner))?;
    let clauses: Vec<_> = file.nodes[declaration]
        .children
        .iter()
        .copied()
        .filter(|&node| file.nodes[node].kind == NodeKind::Requires)
        .collect();
    let [clause] = clauses.as_slice() else { return None };
    let [predicate] = *file.nodes[*clause].children.as_slice() else { return None };
    if !matches!(tree.resolved().symbol(predicate), Some(Symbol::Builtin(name)) if name == "true")
        || !tree
            .semantics_at(predicate)
            .all(|record| matches!(record, NodeSemantics::Expression(expression) if expression.ty == ValueType::Bool))
    {
        return None;
    }
    let carrier = application.map_or_else(|| construction.carrier.clone(), |call| call.instantiate(&construction.carrier));
    if carrier.contains_parameter() {
        return None;
    }
    Some(CheckedDataEstablishment {
        trees,
        obligation,
        carrier,
        fields,
        target_dependencies: Vec::new(),
    })
}

#[test]
fn construction_uses_evidence_for_its_exact_fields_and_typed_occurrence() {
    let source = "data Checked [copy] requires true {amount:u32;}
                  data Empty [copy] requires false {}
                  const ALLOWED:Checked = Checked{amount:3};
                  const OTHER_OCCURRENCE:Checked = Checked{amount:3};
                  const FALSE:Empty = Empty{};";
    let trees = typed(source);
    let find = |path| {
        &trees
            .constants
            .iter()
            .find(|constant| constant.declaration.path == path)
            .unwrap()
            .initializer
    };
    let fields = [PreparedValue::Integer(3.into())].into();
    assert!(trees.static_data_shapes[&find("ALLOWED").carrier].requires_establishment);
    let proof = prove_literal_default_domain(&trees, "ALLOWED", fields).unwrap();
    assert!(prove_literal_default_domain(&trees, "FALSE", [].into()).is_none());
    let mut evaluator = ConstantEvaluation::new(&trees, &[]);
    evaluator.nominal.establishments.push(proof);
    let value = evaluator.evaluate_value_request(find("ALLOWED"), None).unwrap();
    assert!(matches!(&value, PreparedValue::Record { fields, .. } if fields.as_ref() == [PreparedValue::Integer(3.into())]));
    assert_eq!(
        evaluator.canonical_identity(&value, &StaticValueDemand::Closed(find("ALLOWED").clone())),
        Err(PreparationReason::NoCanonicalIndexIdentity)
    );
    for constant in ["OTHER_OCCURRENCE", "FALSE"] {
        assert!(matches!(
            evaluator.evaluate_value_request(find(constant), None),
            Err(EvaluationError::Refused(RefusalCause::Preparation(NEEDS_CONSTRUCTION_PROOF), None)),
        ));
    }
    evaluator.nominal.establishments[0].fields = [PreparedValue::Integer(4.into())].into();
    assert!(matches!(
        evaluator.evaluate_value_request(find("ALLOWED"), None),
        Err(EvaluationError::Refused(RefusalCause::Preparation(NEEDS_CONSTRUCTION_PROOF), None)),
    ));

    evaluator.nominal.establishments[0].fields = [PreparedValue::Integer(3.into())].into();
    assert!(evaluator.evaluate_value_request(find("ALLOWED"), None).is_ok());
    let mut unrelated_pass = ConstantEvaluation::new(&trees, &[]);
    unrelated_pass.nominal.establishments = evaluator.nominal.establishments;
    let other_trees = typed(source);
    // Both trees have the same source and node ordinals; only the producing
    // typed stage is authorized to reuse the checked judgment.
    unrelated_pass.trees = &other_trees;
    assert!(matches!(
        unrelated_pass.evaluate_value_request(find("ALLOWED"), None),
        Err(EvaluationError::Refused(RefusalCause::Preparation(NEEDS_CONSTRUCTION_PROOF), None)),
    ));
}

#[test]
fn a_constructor_proof_replays_its_target_dependency() {
    use crate::{FloatFormat, FloatOperation, FloatRealization, FloatSemantics, TargetIdentity, TargetSemantics};

    let trees = typed(
        "data Checked [copy] requires true {amount:u32;}
                      const ALLOWED:Checked = Checked{amount:3};",
    );
    let request = &trees.constants[0].initializer;
    let mut proof = prove_literal_default_domain(&trees, "ALLOWED", [PreparedValue::Integer(3.into())].into()).unwrap();
    let realization = FloatRealization {
        format: FloatFormat::Binary32,
        operation: FloatOperation::Equal,
        semantics: FloatSemantics::Ieee754NearestEvenV1,
    };
    let target = |version| {
        TargetSemantics::new(
            TargetIdentity {
                profile: "test".into(),
                semantics_version: version,
                realization: (&b"ieee arithmetic"[..]).into(),
            },
            [],
            [realization],
        )
        .unwrap()
    };
    let (original, changed) = (target(1), target(2));
    let observation = TargetObservation::FloatRealization {
        target: original.identity().clone(),
        realization,
    };
    // Exercise replay for conservative proof evidence tied to this capsule.
    proof.target_dependencies.push(observation.clone());
    let mut evaluator = ConstantEvaluation::new(&trees, &[]);
    evaluator.nominal.establishments.push(proof);
    evaluator.target = Some(&changed);
    assert!(matches!(
        evaluator.evaluate_value_request(request, None),
        Err(EvaluationError::Refused(RefusalCause::Preparation(NEEDS_CONSTRUCTION_PROOF), None))
    ));
    assert!(evaluator.observations.borrow().is_empty());
    // No target: the same establishment waits for target selection instead
    // of reporting a missing proof.
    evaluator.target = None;
    assert!(matches!(
        evaluator.evaluate_value_request(request, None),
        Err(EvaluationError::Refused(RefusalCause::Preparation(PreparationReason::ProofNeedsTarget), None))
    ));
    // Freezing the package keeps ALLOWED as a recipe at its initializer.
    let mut freezing = ConstantEvaluation::new(&trees, &[]);
    freezing
        .nominal
        .establishments
        .push(evaluator.nominal.establishments.pop().unwrap());
    assert!(freezing.finish().unwrap().is_empty());
    let retained = &freezing.open_target_constants[&trees.constants[0].declaration];
    assert_eq!((&retained.file, retained.span), (&request.expression.file, request.expression.span));
    assert_eq!(
        retained.message,
        "static evaluation cannot admit this invocation: the checked proof depends on target input and no target is selected"
    );
    evaluator
        .nominal
        .establishments
        .push(freezing.nominal.establishments.pop().unwrap());
    assert!(evaluator.observations.borrow().is_empty());
    evaluator.target = Some(&original);
    assert!(evaluator.evaluate_value_request(request, None).is_ok());
    assert_eq!(evaluator.observations.borrow().as_slice(), &[observation]);
}

#[test]
fn constrained_index_requires_a_verified_identity_at_the_exact_demand() {
    use crate::{CheckedPreparationEvidence, PreparationProgress, PreparationRound, PreparationStep};
    use omega_typer::TypingProgress;

    let source = "data Checked [copy] requires true {amount:u32;}
                  const VALUE:Checked = Checked{amount:3};
                  data Tag<const Key:Checked>{}
                  machine carry(x:Tag<VALUE>){}";
    let package = resolve(source);
    let round = PreparationRound::begin(&package, &[], None).unwrap();
    let TypingProgress::Pending(pending) = round.typing() else {
        panic!("the typer requests the concrete record index")
    };
    let [demand] = pending.demands() else { panic!("one concrete index demand") };
    let trees = pending.trees();
    let ValueType::Nominal(owner) = &demand.request().carrier else {
        panic!("the request retains the constrained nominal carrier")
    };
    let shape = &trees.static_data_shapes[&demand.request().carrier];
    assert!(shape.requires_establishment);
    assert!(shape.cases.is_empty());
    assert_eq!(
        shape.fields,
        [ValueType::Integer {
            signed: false,
            width: Some(32)
        }]
    );

    let integer = |value: u32| PreparedValue::Integer(value.into());
    let establishment = || prove_literal_default_domain(trees, "VALUE", [integer(3)].into()).unwrap();
    // This probe proves canonical form for an ordinary record with one
    // exact, already canonical u32 field and a universally true predicate.
    let identity = |value| {
        let establishment = prove_literal_default_domain(trees, "VALUE", [integer(value)].into()).unwrap();
        CheckedCanonicalIdentity {
            trees,
            demand: demand.clone(),
            representative: PreparedValue::Record {
                owner: owner.as_ref().clone(),
                fields: establishment.fields,
            },
            target_dependencies: vec![],
        }
    };
    let prepare = |judgment| {
        let mut evidence = CheckedPreparationEvidence::default().with_data_establishment(establishment());
        if let Some(judgment) = judgment {
            evidence = evidence.with_canonical_identity(judgment);
        }
        round.evaluate_with_cause(evidence)
    };

    let wrong_use = CheckedCanonicalIdentity {
        demand: StaticValueDemand::Closed(trees.constants[0].initializer.clone()),
        ..identity(3)
    };
    // Stage input: the round checks each supplied identity against the
    // typer's actual demand. The refusal belongs to the index occurrence.
    let index = source.rfind("VALUE").unwrap();
    let index = (index, index + "VALUE".len());
    assert_eq!(
        (demand.request().expression.span.get_start_offset(), demand.request().expression.span.get_end_offset()),
        index
    );
    for judgment in [None, Some(wrong_use), Some(identity(4))] {
        let refusal = prepare(judgment)
            .err()
            .expect("unchecked canonical identity refused");
        spec_corpus::assert_preparation_cause!(
            refusal,
            RefusalCause::Preparation(PreparationReason::NoCanonicalIndexIdentity),
            "app",
            "main.omg",
            index,
            "static evaluation cannot admit this invocation: the prepared value has no canonical static index identity"
        );
    }
    // The exact identity checked against a target: before target selection
    // the index demand waits for that target instead of refusing.
    let observation = TargetObservation::FloatRealization {
        target: crate::TargetIdentity {
            profile: "test".into(),
            semantics_version: 1,
            realization: (&b"ieee arithmetic"[..]).into(),
        },
        realization: crate::FloatRealization {
            format: crate::FloatFormat::Binary32,
            operation: crate::FloatOperation::Equal,
            semantics: crate::FloatSemantics::Ieee754NearestEvenV1,
        },
    };
    let targeted = CheckedCanonicalIdentity {
        target_dependencies: vec![observation],
        ..identity(3)
    };
    let evidence = CheckedPreparationEvidence::default()
        .with_data_establishment(establishment())
        .with_canonical_identity(targeted);
    let crate::PreparationAnswers::OpenTarget { demands, answers } = crate::prepare_typing_pass_with_cause(round.typing(), &[], None, evidence)
        .unwrap()
        .answers
    else {
        panic!("the index waits for target selection")
    };
    assert!(answers.is_empty());
    let [open] = demands.as_slice() else { panic!("one open index") };
    assert_eq!(&open.demand, demand);
    spec_corpus::assert_preparation_cause!(
        open,
        RefusalCause::Preparation(PreparationReason::ProofNeedsTarget),
        "app",
        "main.omg",
        index,
        "static evaluation cannot admit this invocation: the checked proof depends on target input and no target is selected"
    );

    let expected = StaticValue::Record {
        owner: owner.as_ref().clone(),
        fields: [StaticValue::Integer(3u32.into())].into(),
    };
    let evaluated = prepare(Some(identity(3))).unwrap();
    let PreparationStep::Evaluate(next) = round.advance(evaluated).unwrap() else {
        panic!("the identity still has to finish the typer's package")
    };
    let TypingProgress::Complete(finished_trees) = next.typing() else {
        panic!("the exact canonical identity closes the only demand")
    };
    let evidence =
        CheckedPreparationEvidence::default().with_data_establishment(prove_literal_default_domain(finished_trees, "VALUE", [integer(3)].into()).unwrap());
    let evaluated = next.evaluate(evidence).unwrap();
    let PreparationStep::Finished(PreparationProgress::Complete(prepared)) = next.advance(evaluated).unwrap() else {
        panic!("the prepared product should contain the exact closed index")
    };
    assert_eq!(prepared.typed().evaluated_indices[0].1, expected);
    let consumer = prepared
        .typed()
        .signatures
        .iter()
        .find(|signature| signature.machine.path == "carry")
        .unwrap();
    let ValueType::Applied(tag, arguments) = &consumer.parameters[0].1 else {
        panic!("consumer's closed applied type")
    };
    assert_eq!(tag.path, "Tag");
    assert!(matches!(arguments.as_slice(), [ValueType::StaticValue { value, .. }] if value == &expected));
}

#[test]
fn construction_proof_is_bound_to_one_closed_application() {
    let source = "data Checked<const Tag:bool> [copy] requires true {amount:u32;}
                       machine make<const Tag:bool>()->Checked<Tag>{Checked::<Tag>{amount:3}}
                       const YES:Checked<true> = make::<true>();
                       const NO:Checked<false> = make::<false>();";
    let trees = typed(source);
    let constructors: Vec<_> = trees
        .obligations
        .iter()
        .filter(|o| matches!(o.kind, ObligationKind::EstablishData(_)))
        .collect();
    let [constructor] = constructors.as_slice() else {
        panic!("expected one shared constructor occurrence")
    };
    let request = |name| {
        &trees
            .constants
            .iter()
            .find(|row| row.declaration.path == name)
            .unwrap()
            .initializer
    };

    for (covered, other) in [("YES", "NO"), ("NO", "YES")] {
        let call = trees
            .calls
            .iter()
            .find(|call| call.node == request(covered).node)
            .unwrap();
        let fields = [PreparedValue::Integer(3.into())].into();
        let proof = prove_literal_construction(&trees, constructor.node, Some(call), fields).unwrap();
        assert_eq!(proof.carrier, request(covered).carrier);
        let mut evaluator = ConstantEvaluation::new(&trees, &[]);
        evaluator.nominal.establishments.push(proof);
        assert!(matches!(evaluator.evaluate_value_request(request(covered), None).unwrap(),
            PreparedValue::Record { fields, .. } if fields.as_ref() == [PreparedValue::Integer(3.into())]));
        let error = evaluator
            .evaluate_value_request(request(other), None)
            .expect_err("a sibling application needs its own proof");
        let EvaluationError::Located(refusal) = error else {
            panic!("expected a refusal at {other}, got {error:?}")
        };
        // Typed input: the supplied proof names one application's carrier.
        let occurrence = if other == "YES" { "make::<true>()" } else { "make::<false>()" };
        let start = source.rfind(occurrence).unwrap();
        spec_corpus::assert_preparation_cause!(
            refusal,
            RefusalCause::Preparation(PreparationReason::ConstructionNeedsProof),
            "app",
            "main.omg",
            (start, start + occurrence.len()),
            "static evaluation cannot admit this invocation: the selected constructor requires proof outside the prepared value fragment"
        );
        let PreparationRefusal { diagnostic, .. } = refusal;
        assert_eq!(diagnostic.span, request(other).expression.span);
    }
}

mod refusal_reasons;
