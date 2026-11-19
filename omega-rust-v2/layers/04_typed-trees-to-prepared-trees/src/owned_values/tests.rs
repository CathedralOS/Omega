use super::*;
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
    let obligation = trees
        .obligations
        .iter()
        .find(|obligation| obligation.node == request.node)?;
    let ObligationKind::EstablishData(owner) = &obligation.kind else {
        return None;
    };
    let tree = trees.trees().nth(request.node.file)?;
    let construction = tree.construction(request.node.node)?;
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
    Some(CheckedDataEstablishment {
        trees,
        obligation,
        carrier: construction.carrier.clone(),
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
    assert!(
        evaluator
            .canonical_identity(&value, &StaticValueDemand::Closed(find("ALLOWED").clone()))
            .is_none()
    );
    for constant in ["OTHER_OCCURRENCE", "FALSE"] {
        assert!(matches!(
            evaluator.evaluate_value_request(find(constant), None),
            Err(EvaluationError::Refused(NEEDS_CONSTRUCTION_PROOF)),
        ));
    }
    evaluator.nominal.establishments[0].fields = [PreparedValue::Integer(4.into())].into();
    assert!(matches!(
        evaluator.evaluate_value_request(find("ALLOWED"), None),
        Err(EvaluationError::Refused(NEEDS_CONSTRUCTION_PROOF)),
    ));

    let mut unrelated_pass = ConstantEvaluation::new(&trees, &[]);
    unrelated_pass.nominal.establishments = evaluator.nominal.establishments;
    let other_trees = typed(source);
    // Both trees have the same source and node ordinals; only the producing
    // typed stage is authorized to reuse the checked judgment.
    unrelated_pass.trees = &other_trees;
    assert!(matches!(
        unrelated_pass.evaluate_value_request(find("ALLOWED"), None),
        Err(EvaluationError::Refused(NEEDS_CONSTRUCTION_PROOF)),
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
        Err(EvaluationError::Refused(NEEDS_CONSTRUCTION_PROOF))
    ));
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
        round.evaluate(evidence)
    };

    let wrong_use = CheckedCanonicalIdentity {
        demand: StaticValueDemand::Closed(trees.constants[0].initializer.clone()),
        ..identity(3)
    };
    for judgment in [None, Some(wrong_use), Some(identity(4))] {
        let error = prepare(judgment)
            .err()
            .expect("unchecked canonical identity refused");
        assert!(error.message.contains("no canonical static index identity"), "{error:?}");
    }

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
