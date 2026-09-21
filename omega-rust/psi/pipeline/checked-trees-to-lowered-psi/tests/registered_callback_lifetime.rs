use checked_trees_to_lowered_psi::lower_machine;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, IntegerSign, IntegerType,
    IntegerValue, MachineId, PlaceId,
};
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentProjectionExpression, ContentProjectionScalar,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{VerifiedProgramLocalRootProducerCatalog, decode_module, encode_module};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralInputs, TerminalStructuralScalarFieldValue, TerminalStructuralValue,
};
use terminal_psi::program_local_root_introduction_compatibility_report_identity;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
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
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
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

/// The completed round trip the ledger is built for: `register` hands the
/// caller a live registration rooted under the routed domain, and
/// `unregister` settles that exact occurrence — two boundary calls driven by
/// one real caller rather than sequenced by hand. The result type carries
/// the grant (`-> Registration in Registration::Live`): an `ensures` clause
/// on a boundary signature is rejected by exact parameter qualification.
const REGISTER_SOURCE: &str = r#"
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
        machine register(registration: Registration) -> Registration in Registration::Live;

        machine unregister(registration: Registration in Live);
    }

    data Customer {}
    machine Customer::run(&mut self, registration: Registration)
    reaches Registrar invokes Registrar;
    {
        let registered: Registration in Registration::Live = Registrar::register(registration);
        Registrar::unregister(registered);
    }
"#;

struct DriveRegistration {
    register: BoundaryMachineId,
    calls: Vec<(BoundaryMachineId, Vec<TerminalStructuralValue>)>,
    registered: TerminalStructuralValue,
}

impl TerminalEffectHandler for DriveRegistration {
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

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if matches!(
            effect,
            TerminalEffect::BoundaryCall { boundary, .. } if *boundary == self.register
        ) {
            self.handle_effect(effect)?;
            return Ok(TerminalEffectResult::Structural(self.registered.clone()));
        }
        self.handle_effect(effect)?;
        Ok(TerminalEffectResult::Unit)
    }
}

/// `register`'s claimed linear result lands on the caller's claim frontier
/// under its own place, so `unregister` can settle it — the ledger observes
/// both boundary calls in order and forwards the live registration.
#[test]
fn interpreted_register_unregister_round_trip_drives_the_ledger() {
    let tokens = Lexer::new(REGISTER_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let lowered = lower_machine(&checked, "Customer::run").expect("lower registration program");
    let module = &lowered.semantic_module;
    let register_boundary = module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.identity.contains("Registrar::register"))
        .expect("register boundary retained");
    let unregister = module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.identity.contains("Registrar::unregister"))
        .expect("unregister boundary retained");
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

    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("registration round trip verifies");
    let module_bytes = encode_module(module).expect("encode module");
    let proof_bytes =
        terminal_codec::encode_proof_section(module, &lowered.proof_bundle).expect("encode proof");

    let registration_input = TerminalStructuralValue {
        opaque_identity: 7,
        structural_type: registration.id,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let registered = TerminalStructuralValue {
        opaque_identity: 41,
        structural_type: registration.id,
        qualifications: vec![domain.id],
        path: Vec::new(),
    };
    let inputs = TerminalStructuralInputs {
        arguments: std::slice::from_ref(&registration_input),
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
    let mut ledger = DriveRegistration {
        register: register_boundary.id,
        calls: Vec::new(),
        registered: registered.clone(),
    };

    let execution = terminal_interpreter::interpret_terminal_artifact_measured(
        &module_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        inputs,
        &mut ledger,
    )
    .expect("registration round trip interprets");

    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    let [
        (register_call, register_arguments),
        (unregister_call, unregister_arguments),
    ] = ledger.calls.as_slice()
    else {
        panic!("register and unregister boundary calls observed in order")
    };
    assert_eq!(*register_call, register_boundary.id);
    assert_eq!(register_arguments.as_slice(), &[registration_input]);
    assert_eq!(*unregister_call, unregister.id);
    assert_eq!(unregister_arguments.as_slice(), &[registered]);
}

/// The installed-provider form of the round trip. No authored provider
/// machine can mint a domain today — a `satisfies` body's return still
/// needs ordinary `in Live` evidence — so the provider half is spliced
/// into the lowered module as a terminal machine plus one
/// `ProviderCandidateConformance` row, exactly what a later authored leg
/// must emit.
const INSTALLED_PROVIDER_SOURCE: &str = r#"
    data RegistrationSlot {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> {
        machine project(subject: &Self) -> A;
    }

    pub data Registration [linear] { slot: u64; }

    pub domain Registration::Live
    established by Registrar::register, Registrar::unregister;

    machine Live::content(registration: &Registration) -> CountedQuantity<RegistrationSlot>
    satisfies Content<CountedQuantity<RegistrationSlot>>::project
    {
        CountedQuantity { magnitude: 1 }
    }

    pub boundary trait Registrar {
        machine register(registration: Registration) -> Registration in Registration::Live;
        machine unregister(registration: Registration in Live);
    }

    data Customer {}
    machine Customer::run(&mut self, registration: Registration)
    reaches Registrar invokes Registrar;
    {
        let registered: Registration in Registration::Live = Registrar::register(registration);
        Registrar::unregister(registered);
    }
"#;

/// The `register` boundary resolves to the installed provider: its claim is
/// minted on the caller's frontier at resume and its `Live` qualification is
/// introduced by the declared result, so `unregister` — still a real
/// boundary settlement — observes the claim live and receipts it.
#[test]
fn installed_registered_provider_mints_and_settles_the_live_claim() {
    let tokens = Lexer::new(INSTALLED_PROVIDER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let mut lowered =
        lower_machine(&checked, "Customer::run").expect("installed registration program lowers");
    let module = &mut lowered.semantic_module;
    let unregister = module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.identity.contains("unregister"))
        .expect("unregister boundary retained")
        .id;
    let register = module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.identity.contains("register"))
        .expect("register boundary retained")
        .clone();
    let registration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.identity.contains("Registration"))
        .expect("Registration declaration")
        .id;
    let live = module
        .structural_domains
        .iter()
        .find(|domain| domain.identity.contains("Registration::Live"))
        .expect("Live domain")
        .id;
    let [register_parameter] = register.structural_parameters.as_slice() else {
        panic!("register takes the one registration")
    };
    let terminal_psi::BoundaryMachineResult::Structural(register_result) = &register.result else {
        panic!("register returns a structural result")
    };

    // The checked route authorizes a `RegistrarProvider` machine whose body
    // forwards its argument: `register`'s declared `in Live` result
    // introduces the qualification at return, and the call's `result.claims`
    // mint the registration's claim on the caller at resume.
    let provider = MachineId::new(
        module
            .machines
            .iter()
            .map(|machine| machine.id.get())
            .max()
            .expect("entry machine")
            + 1,
    )
    .expect("provider machine id");
    let argument = PlaceId::new(1).expect("argument place");
    let result_place = PlaceId::new(2).expect("result place");
    let argument_claim = ClaimId::new(1).expect("argument claim");
    let next_block = BlockId::new(
        module
            .machines
            .iter()
            .flat_map(|machine| machine.blocks.iter().map(|block| block.id.get()))
            .max()
            .expect("entry block")
            + 1,
    )
    .expect("provider entry block");
    let next_edge = EdgeId::new(
        module
            .machines
            .iter()
            .flat_map(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| block.terminator.edges().collect::<Vec<_>>())
            })
            .map(|edge| edge.get())
            .max()
            .expect("entry edge")
            + 1,
    )
    .expect("provider return edge");
    let contract = ContractId::new(
        module
            .machines
            .iter()
            .map(|machine| machine.contract.id.get())
            .max()
            .expect("entry machine contract")
            + 1,
    )
    .expect("provider contract id");
    let provider_parameter = terminal_psi::StructuralParameterDeclaration {
        place: argument,
        position: 0,
        is_self: false,
        structural_type: registration,
        multiplicity: register_parameter.multiplicity,
        access: register_parameter.access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    module.machines.push(terminal_psi::TerminalMachine {
        id: provider,
        attachment: Some(registration),
        parameters: Vec::new(),
        structural_parameters: vec![provider_parameter.clone()],
        ranked_scc: None,
        result: terminal_psi::TerminalMachineResult::Structural(
            terminal_psi::StructuralResultDeclaration {
                place: result_place,
                structural_type: registration,
                multiplicity: register_result.multiplicity,
                qualifications: vec![live],
                projected_qualifications: Vec::new(),
                reference_sources: Vec::new(),
            },
        ),
        structural_places: vec![
            terminal_psi::StructuralPlaceDeclaration {
                id: argument,
                kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            terminal_psi::StructuralPlaceDeclaration {
                id: result_place,
                kind: semantic_vocabulary::StructuralPlaceKind::Result,
            },
        ],
        entry_claims: vec![terminal_psi::EntryClaim {
            claim: argument_claim,
            input: argument,
            path: Vec::new(),
        }],
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: next_block,
        // Hand-built terminal_psi fixture: every new `Block`/`MachineContract`
        // field must be added here or the whole c2l `suite` target stops
        // compiling (last drift: `erased_proof_formals`, e272856962).
        blocks: vec![terminal_psi::Block {
            erased_proof_formals: Vec::new(),
            id: next_block,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: terminal_psi::Terminator::ReturnStructural {
                edge: next_edge,
                source: argument,
                returned_claims: vec![argument_claim],
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: terminal_psi::MachineContract {
            erased_proof_formals: Vec::new(),
            id: contract,
            crash_routes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });
    module
        .provider_candidates
        .push(terminal_psi::ProviderCandidateConformance {
            boundary: register.id,
            requirement_identity: register.identity.clone(),
            provider_identity: "RegistrarProvider".to_string(),
            candidate_identity: "RegistrarProvider::register".to_string(),
            candidate: provider,
            signature: terminal_psi::ProviderSignature {
                parameters: vec![terminal_psi::ProviderSignatureParameter {
                    position: provider_parameter.position,
                    is_self: provider_parameter.is_self,
                    structural_type: provider_parameter.structural_type,
                    multiplicity: provider_parameter.multiplicity,
                    access: provider_parameter.access,
                    qualifications: provider_parameter.qualifications.clone(),
                    projected_qualifications: provider_parameter.projected_qualifications.clone(),
                }],
            },
            refinement: terminal_psi::ProviderRefinement {
                positional_parameters: vec![terminal_psi::ProviderParameterRefinement {
                    boundary_index: 0,
                    candidate_index: 0,
                }],
                required_domains: register.requires.clone(),
                realized_service_ceiling: Vec::new(),
            },
        });

    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("installed registration program verifies");
    let module_bytes = encode_module(module).expect("encode module");
    let proof_bytes =
        terminal_codec::encode_proof_section(module, &lowered.proof_bundle).expect("encode proof");

    let terminal_psi::StructuralTypeShape::Record { fields } = &module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == registration)
        .expect("Registration declaration")
        .shape
    else {
        panic!("Registration is a record")
    };
    let slot = fields
        .iter()
        .find(|field| field.identity.contains("slot"))
        .expect("slot field");
    let registration_input = TerminalStructuralValue {
        opaque_identity: 7,
        structural_type: registration,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let inputs = TerminalStructuralInputs {
        arguments: std::slice::from_ref(&registration_input),
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

    let selections = module
        .provider_candidates
        .iter()
        .map(
            |candidate| terminal_interpreter::ProviderInstallationSelection {
                boundary: candidate.boundary,
                provider_identity: candidate.provider_identity.clone(),
                candidate: candidate.candidate,
            },
        )
        .collect::<Vec<_>>();
    assert_eq!(selections.len(), 1, "the register provider is installed");
    let installation = terminal_interpreter::admit_provider_installation_from_artifact(
        &module_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &selections,
    )
    .expect("registrar provider installation is admitted");
    let mut execution = terminal_interpreter::TerminalExecution::start_installed_artifact(
        &module_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        inputs,
        &installation,
    )
    .expect("installed registration execution starts");
    let status = execution
        .resume(
            &mut terminal_fuel::TerminalFuelMeter::default(),
            &mut terminal_interpreter::AcceptTerminalEffects,
        )
        .expect("installed registration round trip resumes");
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    let [unregister_effect] = execution.effects() else {
        panic!("unregister is the one observed boundary settlement")
    };
    let TerminalEffect::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        ..
    } = unregister_effect
    else {
        panic!("the unregister settlement is a boundary call")
    };
    assert_eq!(*boundary, unregister);
    let [released] = structural_arguments.as_slice() else {
        panic!("unregister settles exactly the registered value")
    };
    assert_eq!(released.qualifications.as_slice(), [live]);
    assert_eq!(completion_receipts.len(), 1);
}

/// The authored-customer form of the ledger: `register` returns a sum whose
/// `Registered` case carries the live registration (`registration in
/// Registration::Live` inside `Reply`). The routed domain authorizes that
/// case payload — `established by` reaches the domain through `Reply`'s
/// members, and the checker grants the case-scoped claim under the same
/// issuance authority as a bare `-> Registration in Registration::Live`
/// result. The `Rejected` arm carries no qualification, so the domain is
/// minted per-case rather than per-carrier.
const REPLY_SUM_SOURCE: &str = r#"
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

    data Reply {
        case Registered(registration: Registration in Live);
        case Rejected;
    }

    boundary trait Registrar {
        machine register(registration: Registration) -> Reply;
        machine unregister(registration: Registration in Live);
    }

    data Customer {}
    machine Customer::run(&mut self, registration: Registration)
    reaches Registrar invokes Registrar;
    {
        let reply: Reply = Registrar::register(registration);
        transition reply {
            Reply::Registered { registration } -> ok(registration)
            Reply::Rejected -> again()
        }
        state ok(&mut self, registration: Registration in Live) {
            Registrar::unregister(registration);
        }
        state again(&mut self) {}
    }
"#;

/// The sum customer checks: the routed domain authorizes the `Registered`
/// case payload, the arm binding carries `Registration in Live` into `ok`,
/// and `unregister` discharges that exact occurrence. Checked-tree admission
/// is where the item's semantics live; the remaining legs (unit-machine plan
/// admission for the affine-classified sum result, installed-provider
/// `supported_result`, native callback entry) are owned by sibling items.
#[test]
fn sum_reply_case_payload_authorizes_the_routed_domain() {
    let tokens = Lexer::new(REPLY_SUM_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed, &CheckingRequest::settled()).expect("sum reply customer checks");
}

/// A boundary machine that returns the same `Reply` but is not named by the
/// domain's `established by` routes cannot mint the case payload's
/// qualification — the route binds issuance to the exact requirement.
#[test]
fn non_route_requirement_cannot_mint_the_case_payload_domain() {
    let source = REPLY_SUM_SOURCE.replace(
        "machine register(registration: Registration) -> Reply;",
        "machine register(registration: Registration) -> Reply;\n        machine mint(registration: Registration) -> Reply;",
    );
    let source = source.replace(
        "Registrar::register(registration)",
        "Registrar::mint(registration)",
    );
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics =
        lower_typed_trees(typed, &CheckingRequest::settled()).expect_err("mint is not a route");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot establish call-result qualification `Registration::Live`")),
        "expected a call-result qualification rejection, got {diagnostics:?}",
    );
}

/// The payload annotation is what mints the membership: with `Registered`'s
/// payload declared as plain `Registration`, the `ok` arm's
/// `Registration in Live` contract has no evidence and the customer is
/// rejected — an unqualified case never borrows the route's authority.
#[test]
fn unqualified_case_payload_cannot_serve_the_qualified_state() {
    // `Vouched` keeps `Live` named on `Reply` so `register` remains a valid
    // route; `Registered`'s own payload is unqualified.
    let source = REPLY_SUM_SOURCE.replace(
        "case Registered(registration: Registration in Live);\n        case Rejected;",
        "case Registered(registration: Registration);\n        case Vouched(voucher: Registration in Live);\n        case Rejected;",
    );
    let source = source.replace(
        "Reply::Rejected -> again()",
        "Reply::Rejected -> again()\n            Reply::Vouched { voucher } -> ok(voucher)",
    );
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("unqualified payload must fail");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Registration::Live")),
        "expected a `Registration::Live` qualification rejection, got {diagnostics:?}",
    );
}
