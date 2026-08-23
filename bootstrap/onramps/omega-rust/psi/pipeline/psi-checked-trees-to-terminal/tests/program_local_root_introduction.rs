use psi_core::{
    ContentAlgebra, ContentAlgebraKind, ProgramLocalCapacityExpression, ProgramLocalCapacityScalar,
};
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::program_local_root_introduction_identity;
use psi_terminal_codec::{decode_module, encode_module};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

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

fn lowered() -> psi_checked_trees_to_terminal::LoweredTerminalPsi {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    psi_checked_trees_to_terminal::lower_machine(&checked, "Root::run")
        .expect("lower program-local introduction schema")
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
    assert_eq!(
        schema.algebra,
        ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "named(name(ByteUnit))".to_owned(),
        }
    );
    assert_eq!(
        schema.capacity,
        ProgramLocalCapacityExpression::CountedQuantity(ProgramLocalCapacityScalar::Add(
            Box::new(ProgramLocalCapacityScalar::SubjectField(vec![
                "length".to_owned()
            ])),
            Box::new(ProgramLocalCapacityScalar::Natural("1".to_owned())),
        ))
    );
    assert_eq!(
        schema.identity,
        program_local_root_introduction_identity(
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

    psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("schema verifies");
    let encoded = encode_module(module).expect("encode schema");
    assert_eq!(decode_module(&encoded).expect("decode schema"), *module);
}

#[test]
fn verifier_rejects_tampering_of_every_schema_identity_input() {
    let lowered = lowered();
    let module = lowered.semantic_module;

    fn rejects(mutated: psi_terminal::TerminalModule) {
        assert!(
            psi_terminal_verifier::validate_module_representation(&mutated).is_err(),
            "tampered schema must reject"
        );
    }

    let mutate = |edit: fn(&mut psi_terminal::ProgramLocalRootIntroductionSchema)| {
        let mut changed = module.clone();
        edit(&mut changed.boundary_machines[0].program_local_root_introductions[0]);
        rejects(changed);
    };

    mutate(|schema| schema.argument_index += 1);
    mutate(|schema| schema.source_parameter_position += 1);
    mutate(|schema| schema.qualification = psi_core::StructuralDomainId::new(99).unwrap());
    mutate(|schema| schema.carrier = psi_core::StructuralTypeId::new(99).unwrap());
    mutate(|schema| schema.projection.domain = psi_core::ContentDomainId::new(99).unwrap());
    mutate(|schema| schema.projection.projection_fingerprint ^= 1);
    mutate(|schema| schema.algebra.kind = ContentAlgebraKind::IntervalSet);
    mutate(|schema| schema.algebra.parameter.push_str("Drift"));
    mutate(|schema| {
        schema.capacity = ProgramLocalCapacityExpression::CountedQuantity(
            ProgramLocalCapacityScalar::Natural("2".to_owned()),
        );
    });
    mutate(|schema| schema.identity ^= 1);
}
