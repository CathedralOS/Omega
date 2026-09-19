use checked_trees_to_lowered_psi::lower_machine;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{BoundaryMachineId, IntegerSign, IntegerType, IntegerValue};
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentProjectionExpression, ContentProjectionScalar,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{VerifiedProgramLocalRootProducerCatalog, decode_module, encode_module};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalScalarValue, TerminalStructuralInputs, TerminalStructuralScalarFieldValue,
    TerminalStructuralValue,
};
use terminal_psi::program_local_root_introduction_compatibility_report_identity;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

/// The authored callback-registration contract: `Registration` is the linear
/// authority token a provider hands the program, `Registration::Live` is the
/// routed domain one live registration occupies. `Live::content` bounds live
/// registrations at exactly one `RegistrationSlot` each, so the program-local
/// root capacity counts outstanding registrations. `register` is the routed
/// result that admits the future root; `unregister` is the introduction
/// route that re-presents it at teardown.
const SOURCE: &str = r#"
    data RegistrationSlot {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> {
        machine project(subject: &Self) -> A;
    }

    data Registration [linear] { slot: u64; }

    domain Registration::Live
    established by Registrar::register, Registrar::unregister;

    machine Live::content(registration: &Registration) -> CountedQuantity<RegistrationSlot>
    satisfies Content<CountedQuantity<RegistrationSlot>>::project
    {
        CountedQuantity { magnitude: 1 }
    }

    boundary trait Registrar {
        machine register(registration: Registration) -> Registration
        ensures
            result in Registration::Live;

        machine unregister(registration: Registration in Live);
    }

    data Customer {}
    machine Customer::run(&mut self, registered: Registration in Live)
    reaches Registrar invokes Registrar;
    {
        Registrar::unregister(registered);
    }
"#;

fn lowered() -> lowered_psi::LoweredPsi {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    lower_machine(&checked, "Customer::run").expect("lower registration program")
}

fn unregister_boundary(
    lowered: &lowered_psi::LoweredPsi,
) -> &terminal_psi::BoundaryMachineDeclaration {
    let [boundary] = lowered.semantic_module.boundary_machines.as_slice() else {
        panic!("one retained boundary requirement")
    };
    assert!(boundary.identity.contains("Registrar::unregister"));
    boundary
}

#[test]
fn unregister_route_lowers_capacity_bounded_program_local_schema() {
    let lowered = lowered();
    let module = &lowered.semantic_module;
    let boundary = unregister_boundary(&lowered);
    let [schema] = boundary.program_local_root_introductions.as_slice() else {
        panic!("unregister carries one program-local root introduction schema")
    };
    let domain = module
        .structural_domains
        .iter()
        .find(|domain| domain.id == schema.qualification)
        .expect("qualified domain");

    assert_eq!(schema.argument_index, 0);
    assert_eq!(schema.source_parameter_position, 0);
    assert_eq!(
        schema.carrier,
        boundary.structural_parameters[0].structural_type
    );
    assert_eq!(domain.carrier, schema.carrier);
    assert!(domain.identity.contains("Registration::Live"));
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
            parameter: "named(name(RegistrationSlot))".to_owned(),
        }
    );
    assert_eq!(
        schema.capacity,
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural(
            "1".to_owned()
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
        .expect("registration schema verifies");
    let encoded = encode_module(module).expect("encode module");
    assert_eq!(
        decode_module(&encoded).expect("decode module"),
        *module,
        "codec round-trip preserves the registration schema"
    );
}

#[test]
fn verified_catalog_exposes_the_unregister_producer_row() {
    let lowered = lowered();
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("module verifies");
    let catalog = VerifiedProgramLocalRootProducerCatalog::from_verified(&verified)
        .expect("verified producer catalog");

    let [row] = catalog.schemas() else {
        panic!("one verified program-local producer row")
    };
    let boundary = unregister_boundary(&lowered);
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

struct ObserveBoundaryCalls {
    calls: Vec<(BoundaryMachineId, Vec<TerminalStructuralValue>)>,
}

impl TerminalEffectHandler for ObserveBoundaryCalls {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            boundary,
            structural_arguments,
            ..
        } = effect
        else {
            panic!("only boundary effects are expected")
        };
        self.calls.push((*boundary, structural_arguments.clone()));
        Ok(())
    }
}

/// The authored program end to end: the provider-installed `Registration`
/// root enters `Customer::run` as an entry claim and `unregister` forwards it
/// to the provider boundary — the dispatch the registration runtime owns.
#[test]
fn interpreted_unregister_dispatch_forwards_the_live_registration() {
    let lowered = lowered();
    let module = &lowered.semantic_module;
    let boundary = unregister_boundary(&lowered);
    let registration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.identity.contains("Registration"))
        .expect("Registration declaration");
    let domain = module
        .structural_domains
        .iter()
        .find(|domain| domain.identity.contains("Registration::Live"))
        .expect("Live domain");
    let terminal_psi::StructuralTypeShape::Record { fields } = &registration.shape else {
        panic!("Registration is a record")
    };
    let slot = fields
        .iter()
        .find(|field| field.identity.contains("slot"))
        .expect("slot field");

    let module_bytes = encode_module(module).expect("encode module");
    let proof_bytes =
        terminal_codec::encode_proof_section(module, &lowered.proof_bundle).expect("encode proof");

    let registered = TerminalStructuralValue {
        opaque_identity: 41,
        structural_type: registration.id,
        qualifications: vec![domain.id],
        path: Vec::new(),
    };
    let inputs = TerminalStructuralInputs {
        arguments: std::slice::from_ref(&registered),
        scalar_fields: &[TerminalStructuralScalarFieldValue {
            argument_index: 0,
            path: Vec::new(),
            field: slot.id,
            value: TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                value: IntegerValue::Unsigned(3),
            },
        }],
        ..Default::default()
    };
    let mut observer = ObserveBoundaryCalls { calls: Vec::new() };

    let execution = terminal_interpreter::interpret_terminal_artifact_measured(
        &module_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        inputs,
        &mut observer,
    )
    .expect("registration program interprets");

    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    assert_eq!(observer.calls.len(), 1, "one boundary call observed");
    let [(identity, arguments)] = observer.calls.as_slice() else {
        unreachable!()
    };
    assert_eq!(*identity, boundary.id);
    assert_eq!(arguments.as_slice(), &[registered]);
}
