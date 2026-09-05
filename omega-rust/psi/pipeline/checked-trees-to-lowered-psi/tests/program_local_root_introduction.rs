use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentProjectionExpression, ContentProjectionScalar,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{
    VerifiedProgramLocalRootProducerCatalog, decode_module, encode_module, terminal_psi_identity,
};
use terminal_psi::program_local_root_introduction_compatibility_report_identity;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data ByteUnit {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> {
        machine project(subject: &Self) -> A;
    }

    data Region [linear] { length: u64; }
    domain Region::Owned
    established by RegionEntry::enter;
    machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
    satisfies Content<CountedQuantity<ByteUnit>>::project
    {
        CountedQuantity { magnitude: region.length + 1 }
    }

    boundary trait RegionEntry {
        machine enter(region: Region in Owned);
    }

    data Root {}
    machine Root::run<machine Enter>(region: Region in Owned)
    where machine Enter satisfies RegionEntry::enter;
    {
        Enter(region);
    }
"#;

const INDEXED_SOURCE: &str = r#"
    data ByteUnit {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> {
        machine project(subject: &Self) -> A;
    }

    data Region [linear] { length: u64; }
    domain<T, const N: u64> T::Owned<N>
    established by RegionEntry::enter;
    machine Owned::content(region: &Region in Owned<4>) -> CountedQuantity<ByteUnit>
    satisfies Content<CountedQuantity<ByteUnit>>::project
    {
        CountedQuantity { magnitude: region.length }
    }

    boundary trait RegionEntry {
        machine enter(region: Region in Owned<4>);
    }

    data Root {}
    machine Root::run<machine Enter>(region: Region in Owned<4>)
    where machine Enter satisfies RegionEntry::enter;
    {
        Enter(region);
    }
"#;

fn lowered_source(source: &str) -> lowered_psi::LoweredPsi {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    checked_trees_to_lowered_psi::lower_machine(&checked, "Root::run")
        .expect("lower program-local introduction schema")
}

fn lowered() -> lowered_psi::LoweredPsi {
    lowered_source(SOURCE)
}

fn owned_producer_catalog() -> VerifiedProgramLocalRootProducerCatalog {
    let lowered = lowered();
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("program-local producer verifies");
    VerifiedProgramLocalRootProducerCatalog::from_verified(&verified)
        .expect("verified producer catalog")
}

#[test]
fn source_route_lowers_exact_source_free_program_local_schema() {
    let lowered = lowered();
    let module = &lowered.semantic_module;
    let [boundary] = module.boundary_machines.as_slice() else {
        panic!("one retained boundary requirement")
    };
    let [schema] = boundary.program_local_root_introductions.as_slice() else {
        panic!("one routed program-local schema")
    };
    assert_eq!(
        module
            .boundary_machines
            .iter()
            .map(|requirement| requirement.program_local_root_introductions.len())
            .sum::<usize>(),
        1,
        "the ordinary Enter(region) call forwards its existing claim and must not create another producer schema"
    );
    let domain = module
        .structural_domains
        .iter()
        .find(|domain| domain.id == schema.qualification)
        .expect("qualified domain");

    assert!(boundary.identity.contains("RegionEntry::enter"));
    assert_eq!(schema.argument_index, 0);
    assert_eq!(schema.source_parameter_position, 0);
    assert_eq!(
        schema.carrier,
        boundary.structural_parameters[0].structural_type
    );
    assert_eq!(domain.carrier, schema.carrier);
    assert_eq!(schema.projection.domain.get(), domain.semantic_domain.get());
    let owner_projection = domain
        .content_projection
        .as_ref()
        .expect("content-bearing domain retains its owner projection");
    assert_eq!(owner_projection.identity, schema.projection);
    assert_eq!(owner_projection.algebra, schema.algebra);
    assert_eq!(owner_projection.expression, schema.capacity);
    assert_eq!(
        schema.algebra,
        ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "named(name(ByteUnit))".to_owned(),
        }
    );
    assert_eq!(
        schema.capacity,
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Add(
            Box::new(ContentProjectionScalar::SubjectField(vec![
                "length".to_owned()
            ])),
            Box::new(ContentProjectionScalar::Natural("1".to_owned())),
        ))
    );
    assert_eq!(
        schema.compatibility_report_identity,
        program_local_root_introduction_compatibility_report_identity(
            &boundary.identity,
            &domain.identity,
            &module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == schema.carrier)
                .expect("carrier declaration")
                .identity,
            schema,
        )
    );

    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("schema verifies");
    let encoded = encode_module(module).expect("encode schema");
    assert_eq!(decode_module(&encoded).expect("decode schema"), *module);
}

#[test]
fn verified_catalog_owns_canonical_source_free_producer_rows() {
    // Construction happens in the helper so its module and verifier wrapper
    // are both gone before this owned catalog is inspected.
    let catalog = owned_producer_catalog();
    let lowered = lowered();
    assert_eq!(
        catalog.terminal_psi(),
        terminal_psi_identity(&lowered.semantic_module).expect("terminal identity")
    );
    assert_eq!(catalog.terminal_entry(), lowered.semantic_module.entry);

    let [row] = catalog.schemas() else {
        panic!("one verified program-local producer row")
    };
    let boundary = &lowered.semantic_module.boundary_machines[0];
    let schema = &boundary.program_local_root_introductions[0];
    let domain = lowered
        .semantic_module
        .structural_domains
        .iter()
        .find(|domain| domain.id == schema.qualification)
        .expect("qualified domain");
    let carrier = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|carrier| carrier.id == schema.carrier)
        .expect("structural carrier");

    assert_eq!(row.boundary_requirement_identity(), boundary.identity);
    assert_eq!(row.qualification_identity(), domain.identity);
    assert_eq!(row.carrier_identity(), carrier.identity);
    assert_eq!(row.schema(), schema);
}

#[test]
fn verifier_rejects_tampering_of_every_schema_report_identity_input() {
    let lowered = lowered();
    let module = lowered.semantic_module;

    fn rejects(mutated: terminal_psi::TerminalModule) {
        assert!(
            terminal_verifier::validate_module_representation(&mutated).is_err(),
            "tampered schema must reject"
        );
    }

    let mutate = |edit: fn(&mut terminal_psi::ProgramLocalRootIntroductionSchema)| {
        let mut changed = module.clone();
        edit(&mut changed.boundary_machines[0].program_local_root_introductions[0]);
        rejects(changed);
    };

    mutate(|schema| schema.argument_index += 1);
    mutate(|schema| schema.source_parameter_position += 1);
    mutate(|schema| {
        schema.qualification = semantic_vocabulary::StructuralDomainId::new(99).unwrap()
    });
    mutate(|schema| schema.carrier = semantic_vocabulary::StructuralTypeId::new(99).unwrap());
    mutate(|schema| {
        schema.projection.domain = semantic_vocabulary::ContentDomainId::new(99).unwrap()
    });
    mutate(|schema| schema.projection.projection_report_fingerprint ^= 1);
    mutate(|schema| schema.algebra.kind = ContentAlgebraKind::IntervalSet);
    mutate(|schema| schema.algebra.parameter.push_str("Drift"));
    mutate(|schema| {
        schema.capacity = ContentProjectionExpression::CountedQuantity(
            ContentProjectionScalar::Natural("2".to_owned()),
        );
    });
    mutate(|schema| schema.compatibility_report_identity ^= 1);
}

#[test]
fn coherently_understated_route_schema_cannot_rewrite_its_owner_projection() {
    let lowered = lowered();
    let mut understated = lowered.semantic_module;
    let boundary = &mut understated.boundary_machines[0];
    let schema = &mut boundary.program_local_root_introductions[0];
    schema.capacity = ContentProjectionExpression::CountedQuantity(
        ContentProjectionScalar::Natural("1".to_owned()),
    );
    schema.projection.projection_report_fingerprint =
        language_semantics::content::terminal_projection_report_fingerprint(
            &schema.algebra,
            &schema.capacity,
        );
    let domain = understated
        .structural_domains
        .iter()
        .find(|domain| domain.id == schema.qualification)
        .expect("qualified domain");
    let carrier = understated
        .structural_types
        .iter()
        .find(|carrier| carrier.id == schema.carrier)
        .expect("carrier");
    schema.compatibility_report_identity =
        program_local_root_introduction_compatibility_report_identity(
            &boundary.identity,
            &domain.identity,
            &carrier.identity,
            schema,
        );

    assert!(matches!(
        terminal_verifier::validate_module_representation(&understated),
        Err(terminal_verifier::ModuleError::InvalidProgramLocalRootIntroduction { .. })
    ));
}

#[test]
fn owner_projection_is_mandatory_and_independently_replayed() {
    let lowered = lowered();
    let mut missing = lowered.semantic_module.clone();
    missing.structural_domains[0].content_projection = None;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&missing),
        Err(terminal_verifier::ModuleError::InvalidProgramLocalRootIntroduction { .. })
    ));

    let mut drifted = lowered.semantic_module;
    let owner = drifted.structural_domains[0]
        .content_projection
        .as_mut()
        .expect("owner content projection");
    owner.expression = ContentProjectionExpression::CountedQuantity(
        ContentProjectionScalar::Natural("1".to_owned()),
    );
    assert!(matches!(
        terminal_verifier::validate_module_representation(&drifted),
        Err(terminal_verifier::ModuleError::InvalidStructuralDomainContentProjection(_))
    ));
}

#[test]
fn duplicate_schema_cannot_cross_the_verified_catalog_gate() {
    let lowered = lowered();
    let mut duplicate = lowered.semantic_module;
    let schema = duplicate.boundary_machines[0].program_local_root_introductions[0].clone();
    duplicate.boundary_machines[0]
        .program_local_root_introductions
        .push(schema);

    assert!(
        terminal_verifier::verify_module(
            &duplicate,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "a duplicate producer row cannot yield the VerifiedTerminalModule required by the catalog"
    );
}

#[test]
fn closed_indexed_domain_applications_remain_exact_program_local_root_families() {
    let first = lowered_source(INDEXED_SOURCE);
    let second_source = INDEXED_SOURCE.replace("Owned<4>", "Owned<8>");
    let second = lowered_source(&second_source);

    let first_schema =
        &first.semantic_module.boundary_machines[0].program_local_root_introductions[0];
    let second_schema =
        &second.semantic_module.boundary_machines[0].program_local_root_introductions[0];
    let first_domain = first
        .semantic_module
        .structural_domains
        .iter()
        .find(|domain| domain.id == first_schema.qualification)
        .expect("first closed indexed qualification");
    let second_domain = second
        .semantic_module
        .structural_domains
        .iter()
        .find(|domain| domain.id == second_schema.qualification)
        .expect("second closed indexed qualification");

    assert!(first_domain.identity.contains("integer:u64:4"));
    assert!(second_domain.identity.contains("integer:u64:8"));
    assert_ne!(first_domain.identity, second_domain.identity);
    assert_ne!(
        first_schema.compatibility_report_identity,
        second_schema.compatibility_report_identity
    );
    assert_eq!(
        first_domain
            .content_projection
            .as_ref()
            .expect("first owner projection")
            .identity,
        first_schema.projection
    );
    assert_eq!(
        second_domain
            .content_projection
            .as_ref()
            .expect("second owner projection")
            .identity,
        second_schema.projection
    );

    terminal_verifier::verify_module(
        &first.semantic_module,
        &first.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("first closed family instance verifies");
    terminal_verifier::verify_module(
        &second.semantic_module,
        &second.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("second closed family instance verifies");

    let mut substituted = second.semantic_module;
    substituted.boundary_machines[0].program_local_root_introductions[0] = first_schema.clone();
    assert!(
        terminal_verifier::validate_module_representation(&substituted).is_err(),
        "one closed family instance cannot substitute another"
    );
}
