use psi_core::{IntegerValue, OperationId, ValueId};
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{EvidenceContractLaneKind, EvidenceTermDeclaration, OperationKind};
use psi_terminal_codec::{
    decode_module, decode_proof_bundle, encode_module, encode_proof_bundle,
    proof_bundle_fingerprint, render_verified_proof_synopsis, semantic_fingerprint,
};
use psi_terminal_fixed_fuel::derive_fixed_entry_fuel;
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralValue,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const FORWARDED_SOURCE: &str = r#"
    trait Evidence<T> {
        machine witness(value: T);
    }

    proposition ready<T>() evidence Evidence<T>;

    data Token { value: u64; }
    data Root {}
    machine Root::forward(first: Token, second: Token)
    requires incoming_first: ready<i32>()
    requires incoming_second: ready<i32>()
    ensures outgoing_first: ready<i32>()
    ensures outgoing_second: ready<i32>()
    {
        outgoing_first = incoming_first;
        outgoing_second = incoming_second;
    }
"#;

const PRODUCED_SOURCE: &str = r#"
    trait Evidence<T> {
        machine witness(value: T);
    }

    proposition ready<T>() evidence Evidence<T>;

    ConcreteEvidence: satisfies Evidence<i32> {
        machine witness(value: i32) {}
    }

    data Token { value: u64; }
    data Root {}
    machine Root::produce(first: Token, second: Token)
    ensures
        outgoing: ready<i32>()
    {
        outgoing = ConcreteEvidence;
    }
"#;

const EMPTY_PRODUCER_SOURCE: &str = r#"
    trait Evidence<T> {}

    proposition ready<T>() evidence Evidence<T>;

    ConcreteEvidence: satisfies Evidence<i32> {}

    data Root {}
    machine Root::produce()
    ensures outgoing: ready<i32>()
    {
        outgoing = ConcreteEvidence;
    }
"#;

const PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce()
    ensures outgoing: ready()
    {
        outgoing = ConcreteEvidence;
    }

    machine Root::relay()
    ensures relayed: ready()
    {
        let (; outgoing: local) = Root::produce();
        relayed = local;
    }
"#;

const ARGUMENTED_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition carries(value: i32) evidence Evidence;
    data Root {}

    machine Root::produce(value: i32)
    requires incoming: carries(value)
    ensures copied: carries(value)
    {
        copied = incoming;
    }

    machine Root::relay()
    requires source: carries(7)
    ensures relayed: carries(7)
    {
        let (; copied: local) = Root::produce(7; source);
        relayed = local;
    }
"#;

const STATIC_REQUIREMENT_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;

    trait Producer {
        machine Self::produce(&self)
        requires public_in: ready()
        ensures public_out: ready();
    }

    data Token {}

    TokenProducer: Token satisfies Producer {
        machine produce(&self)
        requires local_in: ready()
        ensures public_out: ready()
        ensures private_out: ready()
        {
            public_out = local_in;
            private_out = local_in;
        }
    }

    data Root {}

    machine Root::invoke<Element, Order: Element satisfies Producer>(
        &self,
        value: &Element
    )
    requires incoming: ready()
    {
        let (; public_out: result) = Order::produce(value; incoming);
    }

    machine Root::caller(&self, value: &Token)
    requires incoming: ready()
    {
        self.invoke<Token, TokenProducer>(value; incoming);
    }
"#;

const PLURAL_STATIC_REQUIREMENT_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;

    trait Producer {
        machine Self::produce(&self)
        requires public_first: ready()
        requires public_second: ready()
        ensures first_copy: ready()
        ensures second_copy: ready()
        ensures third_copy: ready();
    }

    data Token {}

    TokenProducer: Token satisfies Producer {
        machine produce(&self)
        requires local_first: ready()
        requires local_second: ready()
        ensures first_copy: ready()
        ensures second_copy: ready()
        ensures third_copy: ready()
        {
            first_copy = local_first;
            second_copy = local_first;
            third_copy = local_second;
        }
    }

    data Root {}

    machine Root::invoke<Element, Order: Element satisfies Producer>(
        &self,
        value: &Element
    )
    requires incoming_first: ready()
    requires incoming_second: ready()
    {
        let (; first_copy: first, second_copy: second, third_copy: third) =
            Order::produce(value; incoming_first, incoming_second);
    }

    machine Root::caller(&self, value: &Token)
    requires incoming_first: ready()
    requires incoming_second: ready()
    {
        self.invoke<Token, TokenProducer>(value; incoming_first, incoming_second);
    }
"#;

const STATIC_REQUIREMENT_TRAIT_DEFAULT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;

    trait Producer {
        machine Self::produce(&self)
        requires public_in: ready()
        ensures public_out: ready()
        {
            public_out = public_in;
        }
    }

    data Token {}
    TokenProducer: Token satisfies Producer {}

    data Root {}
    machine Root::invoke<Element, Order: Element satisfies Producer>(
        &self,
        value: &Element
    )
    requires incoming: ready()
    {
        let (; public_out: result) = Order::produce(value; incoming);
    }

    machine Root::caller(&self, value: &Token)
    requires incoming: ready()
    {
        self.invoke<Token, TokenProducer>(value; incoming);
    }
"#;

const STATIC_REQUIREMENT_RUNTIME_BASELINE_SOURCE: &str = r#"
    trait Producer {
        machine Self::produce(&self);
    }

    data Token {}

    TokenProducer: Token satisfies Producer {
        machine produce(&self) {}
    }

    data Root {}

    machine Root::invoke<Element, Order: Element satisfies Producer>(
        &self,
        value: &Element
    ) {
        Order::produce(value);
    }

    machine Root::caller(&self, value: &Token) {
        self.invoke<Token, TokenProducer>(value);
    }
"#;

const STATIC_REQUIREMENT_I32_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;

    trait Producer {
        machine Self::produce() -> i32
        requires public_in: ready()
        ensures public_out: ready();
    }

    data Token {}
    TokenProducer: Token satisfies Producer {
        machine produce() -> i32
        requires local_in: ready()
        requires 17i32 == 17i32
        ensures public_out: ready()
        ensures 17i32 == 17i32
        {
            public_out = local_in;
            17i32
        }
    }

    machine invoke<Element, Order: Element satisfies Producer>() -> i32
    requires incoming: ready()
    requires 17i32 == 17i32
    ensures 17i32 == 17i32
    {
        let (value; public_out: result) = Order::produce(; incoming);
        value
    }

    machine caller() -> i32
    requires incoming: ready()
    requires 17i32 == 17i32
    ensures 17i32 == 17i32
    {
        invoke<Token, TokenProducer>(; incoming)
    }
"#;

const STATIC_REQUIREMENT_I32_RUNTIME_BASELINE_SOURCE: &str = r#"
    machine produce() -> i32
    requires 17i32 == 17i32
    ensures 17i32 == 17i32
    {
        17i32
    }

    machine caller() -> i32
    requires 17i32 == 17i32
    ensures 17i32 == 17i32
    {
        let value: i32 = produce();
        value
    }
"#;

const ORDINARY_ATTACHED_SCALAR_SOURCE: &str = r#"
    data Root {}

    machine Root::f(value: i64) -> bool
    requires true == true
    ensures true == true
    {
        true
    }
"#;

const DUPLICATE_ARGUMENTED_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    data Root {}

    machine Root::produce()
    requires first: ready()
    requires second: ready()
    ensures copied: ready()
    {
        copied = second;
    }

    machine Root::relay()
    requires source: ready()
    ensures relayed: ready()
    {
        let (; copied: local) = Root::produce(; source, source);
        relayed = local;
    }
"#;

const RUNTIME_UNIT_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::touch() {}

    machine Root::produce()
    ensures outgoing: ready()
    {
        Root::touch();
        outgoing = ConcreteEvidence;
    }

    machine Root::relay()
    ensures relayed: ready()
    {
        let (; outgoing: local) = Root::produce();
        relayed = local;
    }
"#;

const COPY_AND_DISCARD_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce()
    ensures copied: ready()
    ensures discarded: ready()
    { copied = ConcreteEvidence; discarded = ConcreteEvidence; }

    machine Root::relay()
    ensures first: ready()
    ensures second: ready()
    {
        let (; copied: local, discarded: _) = Root::produce();
        first = local;
        second = local;
    }
"#;

const MULTI_FIELD_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce()
    ensures first: ready()
    ensures second: ready()
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
    }

    machine Root::relay()
    ensures relayed_first: ready()
    ensures relayed_second: ready()
    {
        let (; second: local_second, first: local_first) = Root::produce();
        relayed_first = local_first;
        relayed_second = local_second;
    }
"#;

const RUNTIME_VALUE_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    machine warmup() -> bool
    requires true == true
    ensures true == true
    { true }

    machine produce() -> bool
    requires true == true
    ensures true == true
    ensures first: ready()
    ensures second: ready()
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        true
    }

    machine relay() -> bool
    requires true == true
    ensures true == true
    ensures relayed_first: ready()
    ensures relayed_second: ready()
    {
        let warmed: bool = warmup();
        let (local_value; second: local_second, first: local_first) = produce();
        relayed_first = local_first;
        relayed_second = local_second;
        local_value
    }
"#;

const REPEATED_MULTI_FIELD_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce()
    ensures first: ready()
    ensures second: ready()
    { first = ConcreteEvidence; second = ConcreteEvidence; }

    machine Root::relay()
    ensures first_one: ready()
    ensures first_two: ready()
    ensures second_one: ready()
    ensures second_two: ready()
    {
        let (; first: local_first_one, second: local_first_two) = Root::produce();
        first_one = local_first_one;
        first_two = local_first_two;
        let (; first: local_second_one, second: local_second_two) = Root::produce();
        second_one = local_second_one;
        second_two = local_second_two;
    }
"#;

const REPEATED_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce()
    ensures outgoing: ready()
    {
        outgoing = ConcreteEvidence;
    }

    machine Root::relay()
    ensures first: ready()
    ensures second: ready()
    {
        let (; outgoing: local_first) = Root::produce();
        first = local_first;
        let (; outgoing: local_second) = Root::produce();
        second = local_second;
    }
"#;

const DISTINCT_PROOF_OUTPUT_PRODUCERS_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce_first()
    ensures outgoing: ready()
    { outgoing = ConcreteEvidence; }

    machine Root::produce_second()
    ensures outgoing: ready()
    { outgoing = ConcreteEvidence; }

    machine Root::relay()
    ensures first: ready()
    ensures second: ready()
    {
        let (; outgoing: local_first) = Root::produce_first();
        first = local_first;
        let (; outgoing: local_second) = Root::produce_second();
        second = local_second;
    }
"#;

const PROJECTED_SOURCE: &str = r#"
    trait Parent<T> {
        machine modulus(value: T) -> i32;
    }
    trait Evidence<T>: Parent<T> {}

    proposition ready<T>() evidence Evidence<T>;
    proposition selected<machine Witness>();
    proposition chosen<machine Witness>() = selected<Witness>();

    data Root {}
    machine Root::project()
    requires first: ready<i32>()
    requires second: ready<i32>()
    requires selected<first.modulus>()
    requires selected<first.modulus>()
    requires chosen<first.modulus>()
    requires selected<second.modulus>()
    {
    }

    machine Root::forward()
    requires incoming: ready<i32>()
    requires selected<incoming.modulus>()
    ensures outgoing: ready<i32>()
    {
        outgoing = incoming;
    }
"#;

#[test]
fn source_projection_uses_canonical_term_and_exact_requirement_identity() {
    let checked = check(PROJECTED_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::project")
        .expect("carrierless projection should cross terminal Psi");
    let projections = lowered
        .semantic_module
        .proposition_applications
        .iter()
        .flat_map(|application| &application.binder_arguments)
        .filter_map(|argument| argument.evidence_projection.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(projections.len(), 2, "the repeated projection deduplicates");
    assert_eq!(
        projections[0].requirement_identity,
        projections[1].requirement_identity
    );
    assert_eq!(projections[0].declaring_trait_identity, "Parent");
    assert_eq!(projections[0].declaring_trait_arguments.len(), 1);
    assert_ne!(
        projections[0].term, projections[1].term,
        "separate evidence introductions retain distinct opaque projections"
    );
    assert!(lowered.semantic_module.evidence_terms.iter().all(|term| {
        term.interface.requirements.iter().any(|requirement| {
            requirement.requirement_identity == projections[0].requirement_identity
        })
    }));

    let bytes = encode_module(&lowered.semantic_module).expect("projection module encodes");
    assert_eq!(
        decode_module(&bytes).expect("projection module decodes"),
        lowered.semantic_module
    );

    let mut wrong_term = lowered.semantic_module.clone();
    let projection = wrong_term
        .proposition_applications
        .iter_mut()
        .flat_map(|application| &mut application.binder_arguments)
        .find_map(|argument| argument.evidence_projection.as_mut())
        .expect("projection");
    projection.term = psi_core::EvidenceTermId::new(99).expect("test evidence term");
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&wrong_term),
        Err(psi_terminal_verifier::ModuleError::UnknownEvidenceProjectionTerm { .. })
    ));

    let mut wrong_requirement = lowered.semantic_module.clone();
    wrong_requirement
        .proposition_applications
        .iter_mut()
        .flat_map(|application| &mut application.binder_arguments)
        .find_map(|argument| argument.evidence_projection.as_mut())
        .expect("projection")
        .requirement_identity = "forged requirement".to_owned();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&wrong_requirement),
        Err(psi_terminal_verifier::ModuleError::EvidenceProjectionRequirementMismatch { .. })
    ));
}

#[test]
fn forwarded_projection_uses_the_shared_terminal_term_identity() {
    let checked = check(PROJECTED_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::forward")
        .expect("forwarded carrierless projection should cross terminal Psi");
    let projection = lowered
        .semantic_module
        .proposition_applications
        .iter()
        .flat_map(|application| &application.binder_arguments)
        .find_map(|argument| argument.evidence_projection.as_ref())
        .expect("projection");
    let requires = lowered
        .semantic_module
        .evidence_contract_lanes
        .iter()
        .find(|lane| lane.kind == EvidenceContractLaneKind::Requires)
        .expect("requires lane");
    let ensures = lowered
        .semantic_module
        .evidence_contract_lanes
        .iter()
        .find(|lane| lane.kind == EvidenceContractLaneKind::Ensures)
        .expect("ensures lane");
    assert_eq!(requires.term, ensures.term);
    assert_eq!(projection.term, requires.term);
}

#[test]
fn source_forwarding_preserves_exact_positional_terminal_evidence_identities() {
    let checked = check(FORWARDED_SOURCE);
    assert_eq!(checked.facts.proof.evidence_terms.len(), 4);
    let expected_argument = checked
        .facts
        .proof
        .evidence_terms
        .iter()
        .next()
        .and_then(|(_, term)| term.evidence_interface.as_ref())
        .and_then(|interface| interface.arguments.first())
        .expect("the checked endpoint has one exact instantiated type argument")
        .as_str()
        .to_owned();

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::forward")
        .expect("forwarded witness identity should cross terminal Psi");
    assert_eq!(lowered.semantic_module.evidence_terms.len(), 2);
    let term = &lowered.semantic_module.evidence_terms[0];
    assert_eq!(term.interface.trait_identity, "Evidence");
    assert_eq!(term.interface.arguments, vec![expected_argument]);
    assert_eq!(
        term.proposition,
        lowered.semantic_module.proposition_applications[0].id
    );
    assert_eq!(
        lowered.semantic_module.proposition_applications[0]
            .evidence_interface
            .as_ref(),
        Some(&term.interface)
    );
    let lanes = &lowered.semantic_module.evidence_contract_lanes;
    assert_eq!(lanes.len(), 4);
    assert_eq!(lanes[0].kind, EvidenceContractLaneKind::Requires);
    assert_eq!(lanes[0].position, 0);
    assert_eq!(lanes[0].output_field, None);
    assert_eq!(lanes[1].kind, EvidenceContractLaneKind::Requires);
    assert_eq!(lanes[1].position, 1);
    assert_eq!(lanes[1].output_field, None);
    assert_eq!(lanes[2].kind, EvidenceContractLaneKind::Ensures);
    assert_eq!(lanes[2].position, 0);
    assert_eq!(lanes[2].output_field.as_deref(), Some("outgoing_first"));
    assert_eq!(lanes[3].kind, EvidenceContractLaneKind::Ensures);
    assert_eq!(lanes[3].position, 1);
    assert_eq!(lanes[3].output_field.as_deref(), Some("outgoing_second"));
    assert_eq!(lanes[0].term, lanes[2].term);
    assert_eq!(lanes[1].term, lanes[3].term);
    assert_ne!(lanes[0].term, lanes[1].term);

    let bytes = encode_module(&lowered.semantic_module).expect("terminal evidence should encode");
    assert_eq!(
        decode_module(&bytes).expect("terminal evidence should decode"),
        lowered.semantic_module
    );
    let baseline = semantic_fingerprint(&lowered.semantic_module).expect("terminal identity");

    let mut drifted_term = lowered.semantic_module.clone();
    drifted_term.evidence_terms[0].interface.trait_identity = "OtherEvidence".to_owned();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&drifted_term),
        Err(psi_terminal_verifier::ModuleError::EvidenceTermInterfaceMismatch(_))
    ));

    let mut changed = lowered.semantic_module.clone();
    for term in &mut changed.evidence_terms {
        term.interface.trait_identity = "OtherEvidence".to_owned();
    }
    changed.proposition_applications[0]
        .evidence_interface
        .as_mut()
        .expect("witness application interface")
        .trait_identity = "OtherEvidence".to_owned();
    assert_ne!(
        semantic_fingerprint(&changed).expect("changed terminal identity"),
        baseline,
        "the structured exact interface, not its diagnostic spelling, enters identity"
    );

    let mut unresolved = lowered.semantic_module.clone();
    unresolved.evidence_terms[0]
        .interface
        .trait_identity
        .clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unresolved),
        Err(psi_terminal_verifier::ModuleError::InvalidEvidenceInterface(_))
    ));

    let mut malformed_application = lowered.semantic_module.clone();
    malformed_application.proposition_applications[0]
        .evidence_interface
        .as_mut()
        .expect("witness application interface")
        .trait_identity
        .clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&malformed_application),
        Err(psi_terminal_verifier::ModuleError::InvalidPropositionEvidenceInterface(_))
    ));

    let mut fact_only_application = lowered.semantic_module.clone();
    fact_only_application.proposition_declarations[0].evidence =
        psi_terminal::PropositionEvidence::FactOnly;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&fact_only_application),
        Err(psi_terminal_verifier::ModuleError::InvalidPropositionEvidenceInterface(_))
    ));

    fact_only_application.proposition_applications[0].evidence_interface = None;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&fact_only_application),
        Err(psi_terminal_verifier::ModuleError::FactOnlyEvidenceTerm(_))
    ));

    let mut unknown_proposition = lowered.semantic_module.clone();
    unknown_proposition.evidence_terms[0].proposition =
        psi_core::PropositionId::new(99).expect("test proposition identity");
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unknown_proposition),
        Err(psi_terminal_verifier::ModuleError::UnknownEvidenceTermProposition(_))
    ));

    let mut unknown_machine = lowered.semantic_module.clone();
    unknown_machine.evidence_contract_lanes[0].machine =
        psi_core::MachineId::new(99).expect("test machine identity");
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unknown_machine),
        Err(psi_terminal_verifier::ModuleError::UnknownEvidenceContractMachine(_))
    ));

    let mut unknown_term = lowered.semantic_module.clone();
    unknown_term.evidence_contract_lanes[0].term =
        psi_core::EvidenceTermId::new(99).expect("test evidence-term identity");
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unknown_term),
        Err(psi_terminal_verifier::ModuleError::UnknownEvidenceContractTerm(_))
    ));

    let mut non_dense = lowered.semantic_module.clone();
    non_dense.evidence_contract_lanes[1].position = 2;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&non_dense),
        Err(psi_terminal_verifier::ModuleError::NonDenseEvidenceContractLane { .. })
    ));

    let third = psi_core::EvidenceTermId::new(3).expect("third evidence-term identity");
    let mut unforwarded = lowered.semantic_module.clone();
    unforwarded.evidence_terms.push(EvidenceTermDeclaration {
        id: third,
        proposition: unforwarded.evidence_terms[0].proposition,
        interface: unforwarded.evidence_terms[0].interface.clone(),
    });
    unforwarded.evidence_contract_lanes[2].term = third;
    psi_terminal_verifier::validate_module_representation(&unforwarded)
        .expect("proof provenance, not semantic validation, owns fresh evidence");
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &unforwarded,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidenceProducer(term))
            if term == third
    ));

    let mut orphan = lowered.semantic_module.clone();
    orphan.evidence_terms.push(EvidenceTermDeclaration {
        id: third,
        proposition: orphan.evidence_terms[0].proposition,
        interface: orphan.evidence_terms[0].interface.clone(),
    });
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&orphan),
        Err(psi_terminal_verifier::ModuleError::OrphanEvidenceTerm(_))
    ));

    let mut reordered = lowered.semantic_module.clone();
    reordered.evidence_contract_lanes.swap(0, 1);
    assert!(matches!(
        encode_module(&reordered),
        Err(psi_terminal_codec::CodecError::NonCanonicalOrder(
            "evidence contract lanes by machine, kind, and position"
        ))
    ));

    let mut remapped = lowered.semantic_module.clone();
    remapped.evidence_contract_lanes.swap(0, 1);
    remapped.evidence_contract_lanes[0].position = 0;
    remapped.evidence_contract_lanes[1].position = 1;
    remapped.evidence_contract_lanes.swap(2, 3);
    remapped.evidence_contract_lanes[2].position = 0;
    remapped.evidence_contract_lanes[3].position = 1;
    assert_ne!(
        semantic_fingerprint(&remapped).expect("remapped lanes remain canonical"),
        baseline,
        "strict lane position-to-term identity enters the fingerprint"
    );

    let mut renamed = lowered.semantic_module.clone();
    renamed.evidence_contract_lanes[2].output_field = Some("renamed".to_owned());
    assert_ne!(
        semantic_fingerprint(&renamed).expect("renamed output remains valid"),
        baseline,
        "the public proof-output field enters semantic identity"
    );

    let mut missing_field = lowered.semantic_module.clone();
    missing_field.evidence_contract_lanes[2].output_field = None;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&missing_field),
        Err(psi_terminal_verifier::ModuleError::MissingEvidenceOutputField { .. })
    ));

    let mut input_field = lowered.semantic_module.clone();
    input_field.evidence_contract_lanes[0].output_field = Some("incoming_first".to_owned());
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&input_field),
        Err(psi_terminal_verifier::ModuleError::EvidenceRequiresHasOutputField { .. })
    ));

    let mut reserved = lowered.semantic_module.clone();
    reserved.evidence_contract_lanes[2].output_field = Some("value".to_owned());
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&reserved),
        Err(psi_terminal_verifier::ModuleError::ReservedEvidenceOutputField(_))
    ));

    let mut duplicate = lowered.semantic_module.clone();
    let field = duplicate.evidence_contract_lanes[2].output_field.clone();
    duplicate.evidence_contract_lanes[3].output_field = field;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&duplicate),
        Err(psi_terminal_verifier::ModuleError::DuplicateEvidenceOutputField(_))
    ));

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("forwarding-only erased lanes verify");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("erased evidence lanes admit fixed fuel");
    let mut stripped = lowered.semantic_module.clone();
    stripped.evidence_terms.clear();
    stripped.evidence_contract_lanes.clear();
    let stripped_verified = psi_terminal_verifier::verify_module(
        &stripped,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the executable graph is unchanged without erased rows");
    let stripped_fixed = derive_fixed_entry_fuel(&stripped_verified, stripped.entry)
        .expect("the executable graph keeps fixed fuel");
    assert_eq!(fixed.ceiling_units(), stripped_fixed.ceiling_units());

    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("terminal proof should encode");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let arguments = machine
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 0xe710 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("erased evidence lanes require no runtime arguments");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("execute forwarded lanes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}

#[test]
fn source_producer_provenance_is_separate_canonical_verified_proof_data() {
    let checked = check(PRODUCED_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::produce")
        .expect("selected producer provenance should cross terminal Psi");
    assert_eq!(lowered.semantic_module.evidence_terms.len(), 1);
    assert_eq!(lowered.semantic_module.evidence_contract_lanes.len(), 1);
    assert_eq!(
        lowered.semantic_module.evidence_contract_lanes[0].kind,
        EvidenceContractLaneKind::Ensures
    );
    assert_eq!(
        lowered.semantic_module.evidence_contract_lanes[0]
            .output_field
            .as_deref(),
        Some("outgoing")
    );
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 1);
    let producer = &lowered.proof_bundle.evidence_producers[0];
    assert_eq!(producer.id.get(), 1);
    assert_eq!(
        producer.term,
        lowered.semantic_module.evidence_contract_lanes[0].term
    );
    assert_eq!(producer.conformance_identity, "ConcreteEvidence");
    assert_eq!(producer.evidence_trait_identity, "Evidence");
    assert_eq!(producer.rows.len(), 1);
    assert!(!producer.rows[0].requirement_identity.is_empty());
    assert_eq!(
        producer.rows[0].declaring_trait_arguments,
        lowered.semantic_module.evidence_terms[0]
            .interface
            .requirements[0]
            .declaring_trait_arguments
    );

    let semantic = semantic_fingerprint(&lowered.semantic_module).expect("terminal identity");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("producer proof encodes");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let proof_fingerprint =
        proof_bundle_fingerprint(&lowered.proof_bundle).expect("proof identity");
    let mut changed_proof = lowered.proof_bundle.clone();
    changed_proof.evidence_producers[0].conformance_identity = "OtherEvidence".to_owned();
    assert_ne!(
        proof_bundle_fingerprint(&changed_proof).expect("changed proof remains canonical"),
        proof_fingerprint
    );
    assert_eq!(
        semantic_fingerprint(&lowered.semantic_module).expect("semantic identity is independent"),
        semantic
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("exact selected producer provenance verifies");

    let mut missing = lowered.proof_bundle.clone();
    missing.evidence_producers.clear();
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &missing,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidenceProducer(_))
    ));

    let mut wrong_trait = lowered.proof_bundle.clone();
    wrong_trait.evidence_producers[0].evidence_trait_identity = "OtherEvidence".to_owned();
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &wrong_trait,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::EvidenceProducerInterfaceMismatch(_))
    ));

    let mut non_dense = lowered.proof_bundle.clone();
    non_dense.evidence_producers[0].id =
        psi_core::EvidenceIdentity::new(2).expect("test proof identity");
    assert_eq!(
        encode_proof_bundle(&non_dense),
        Err(psi_terminal_codec::ProofCodecError::NonCanonicalEvidenceProducerOrder)
    );
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &non_dense,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::NonDenseEvidenceProducer { .. })
    ));

    let mut malformed_row = lowered.proof_bundle.clone();
    malformed_row.evidence_producers[0].rows[0]
        .requirement_identity
        .clear();
    assert_eq!(
        encode_proof_bundle(&malformed_row),
        Err(psi_terminal_codec::ProofCodecError::InvalidEvidenceProducer)
    );
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &malformed_row,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::InvalidEvidenceProducer(_))
    ));

    let mut duplicate_row = lowered.proof_bundle.clone();
    let row = duplicate_row.evidence_producers[0].rows[0].clone();
    duplicate_row.evidence_producers[0].rows.push(row);
    assert_eq!(
        encode_proof_bundle(&duplicate_row),
        Err(psi_terminal_codec::ProofCodecError::NonCanonicalEvidenceProducerRows)
    );
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &duplicate_row,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::NonCanonicalEvidenceProducerRows(_))
    ));

    let mut wrong_complete_map = lowered.proof_bundle.clone();
    wrong_complete_map.evidence_producers[0].rows[0].requirement_identity =
        "different complete row".to_owned();
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &wrong_complete_map,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::EvidenceProducerInterfaceMismatch(_))
    ));

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("producer proof verifies for fuel derivation");
    let synopsis = render_verified_proof_synopsis(&verified).expect("producer audit synopsis");
    assert!(
        synopsis.contains("evidence-producer 1 term 1 conformance ConcreteEvidence trait Evidence")
    );
    assert!(synopsis.contains("  row Evidence "));
    let fuel = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("producer provenance does not prevent fixed fuel");
    let mut stripped_module = lowered.semantic_module.clone();
    stripped_module.evidence_terms.clear();
    stripped_module.evidence_contract_lanes.clear();
    let stripped_bundle = psi_terminal_verifier::ProofBundle::default();
    let stripped = psi_terminal_verifier::verify_module(
        &stripped_module,
        &stripped_bundle,
        &AdmissionProfile::default(),
    )
    .expect("erased evidence does not change execution");
    assert_eq!(
        derive_fixed_entry_fuel(&stripped, stripped_module.entry)
            .expect("stripped machine has fixed fuel")
            .ceiling_units(),
        fuel.ceiling_units()
    );

    let bytes = encode_module(&lowered.semantic_module).expect("producer module encodes");
    let machine = &lowered.semantic_module.machines[0];
    let arguments = machine
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 0xe720 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("producer evidence remains erased at runtime");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution.resume(&mut meter).expect("execute producer"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}

#[test]
fn argumented_proof_output_retains_substitution_and_erased_input_identity() {
    let checked = check(ARGUMENTED_PROOF_OUTPUT_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::relay")
        .expect("argumented proof-output call should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one terminal proof-output invocation expected")
    };
    let [argument] = invocation.evidence_arguments.as_slice() else {
        panic!("one terminal erased input expected")
    };
    let [output] = invocation.outputs.as_slice() else {
        panic!("one terminal proof output expected")
    };
    let term = |id| {
        lowered
            .semantic_module
            .evidence_terms
            .iter()
            .find(|term| term.id == id)
            .expect("proof-output term identity")
    };
    assert_eq!(argument.input_position, 0);
    assert_eq!(
        term(argument.source).proposition,
        argument.instantiated_proposition
    );
    assert_ne!(
        argument.callee_proposition, argument.instantiated_proposition,
        "formal and call-substituted propositions remain distinct"
    );
    assert_eq!(
        term(output.output.expect("captured output")).proposition,
        output.instantiated_proposition
    );
    assert_eq!(output.callee_output, None);
    assert_ne!(output.callee_proposition, output.instantiated_proposition);

    let bytes = encode_module(&lowered.semantic_module).expect("argumented proof output encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("argument substitution and erased evidence input verify together");

    let mut wrong_input = lowered.semantic_module.clone();
    wrong_input.proof_output_calls[0].evidence_arguments[0].instantiated_proposition =
        argument.callee_proposition;
    assert!(psi_terminal_verifier::validate_module_representation(&wrong_input).is_err());

    let mut wrong_output = lowered.semantic_module.clone();
    wrong_output.proof_output_calls[0].outputs[0].instantiated_proposition =
        output.callee_proposition;
    assert!(psi_terminal_verifier::validate_module_representation(&wrong_output).is_err());
}

#[test]
fn generic_proof_output_target_identity_binds_the_closed_conformance_application() {
    fn source(selection: &str) -> String {
        format!(
            r#"
                trait Evidence {{}}
                trait Marker {{}}
                proposition ready() evidence Evidence;
                data Card {{}}
                data Root {{ card: Card; }}
                {selection}: Card satisfies Marker {{}}

                machine produce<Element, Selection: Element satisfies Marker>(value: &Element)
                requires incoming: ready()
                ensures copied: ready()
                {{ copied = incoming; }}

                machine Root::relay(&self)
                requires source: ready()
                ensures relayed: ready()
                {{
                    let (; copied: local) = produce<Card, {selection}>(&self.card; source);
                    relayed = local;
                }}
            "#
        )
    }
    fn selected_specialization(
        checked: &psi_checked_trees::CheckedTrees,
    ) -> &psi_typed_trees::typed_trees::MachineSpecialization {
        let target = checked
            .facts
            .proof
            .proof_output_calls
            .iter()
            .next()
            .map(|(_, invocation)| invocation.target_machine_symbol)
            .expect("one checked generic proof-output call");
        checked
            .machine_specializations
            .iter()
            .find(|specialization| specialization.instance == target)
            .expect("the target should retain its checked specialization")
    }

    fn commitment_hex(
        commitment: psi_typed_trees::typed_trees::MachineSpecializationCommitment,
    ) -> String {
        use std::fmt::Write;

        let mut rendered = String::with_capacity(64);
        for byte in commitment.as_bytes() {
            write!(&mut rendered, "{byte:02x}").expect("writing to String cannot fail");
        }
        rendered
    }

    let first = check(&source("FirstMarker"));
    let second = check(&source("SecondMarker"));
    let first_specialization = selected_specialization(&first);
    let second_specialization = selected_specialization(&second);
    assert_ne!(
        first_specialization.report_fingerprint,
        second_specialization.report_fingerprint,
    );
    assert_ne!(
        first_specialization.commitment,
        second_specialization.commitment,
    );
    let first_commitment = commitment_hex(first_specialization.commitment);
    let second_commitment = commitment_hex(second_specialization.commitment);
    let first_lowered = psi_checked_trees_to_terminal::lower_machine(&first, "Root::relay")
        .expect("first closed generic proof output should lower");
    let first_replay = psi_checked_trees_to_terminal::lower_machine(&first, "Root::relay")
        .expect("the same exact specialization should lower deterministically");
    let second_lowered = psi_checked_trees_to_terminal::lower_machine(&second, "Root::relay")
        .expect("second closed generic proof output should lower");
    let [first_call] = first_lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one first proof-output call expected")
    };
    let [first_replay_call] = first_replay.semantic_module.proof_output_calls.as_slice() else {
        panic!("one replayed proof-output call expected")
    };
    let [second_call] = second_lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one second proof-output call expected")
    };
    assert!(
        first_call
            .target_machine_identity
            .starts_with("specialized-machine|")
    );
    assert!(
        first_call
            .target_machine_identity
            .ends_with(&format!("application={first_commitment}"))
    );
    assert!(
        second_call
            .target_machine_identity
            .ends_with(&format!("application={second_commitment}"))
    );
    assert_eq!(
        first_call.target_machine_identity, first_replay_call.target_machine_identity,
        "the same exact specialization has one deterministic producer identity",
    );
    assert_ne!(
        first_call.target_machine_identity, second_call.target_machine_identity,
        "different closed conformance selections are different proof-output targets"
    );
    let baseline =
        semantic_fingerprint(&first_lowered.semantic_module).expect("generic target identity");
    let mut tampered = first_lowered.semantic_module.clone();
    let identity = &mut tampered.proof_output_calls[0].target_machine_identity;
    let last = identity.len() - 1;
    let replacement = if identity.ends_with('0') { "1" } else { "0" };
    identity.replace_range(last.., replacement);
    assert_ne!(
        semantic_fingerprint(&tampered).expect("tampered generic target remains canonical"),
        baseline,
        "the exact closed application enters terminal semantic identity"
    );

    let mut changed_exact_specialization = first.clone();
    let specialization = selected_specialization(&changed_exact_specialization);
    let target = specialization.instance;
    let retained_report = specialization.report_fingerprint;
    let specialization = changed_exact_specialization
        .typed
        .machine_specializations
        .iter_mut()
        .find(|specialization| specialization.instance == target)
        .expect("mutated specialization remains present");
    specialization.type_argument_identities[0].push_str("|substituted");
    assert_eq!(specialization.report_fingerprint, retained_report);
    assert_eq!(
        psi_checked_trees_to_terminal::lower_machine(&changed_exact_specialization, "Root::relay",),
        Err(psi_checked_trees_to_terminal::LoweringError::Unsupported(
            "evidence machine specialization commitment does not replay",
        )),
        "an exact specialization mutation cannot hide behind the same compact report coordinate",
    );

    let mut substituted_commitment = first.clone();
    let specialization = selected_specialization(&substituted_commitment);
    let target = specialization.instance;
    let retained_report = specialization.report_fingerprint;
    let specialization = substituted_commitment
        .typed
        .machine_specializations
        .iter_mut()
        .find(|specialization| specialization.instance == target)
        .expect("substituted specialization remains present");
    specialization.commitment = selected_specialization(&second).commitment;
    assert_eq!(specialization.report_fingerprint, retained_report);
    assert_eq!(
        psi_checked_trees_to_terminal::lower_machine(&substituted_commitment, "Root::relay"),
        Err(psi_checked_trees_to_terminal::LoweringError::Unsupported(
            "evidence machine specialization commitment does not replay",
        )),
        "a stored commitment from another exact specialization must not be admitted",
    );
}

#[test]
fn forwarded_proof_output_retains_the_exact_duplicate_input_lane() {
    let checked = check(DUPLICATE_ARGUMENTED_PROOF_OUTPUT_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::relay")
        .expect("duplicate proof inputs should retain exact lane identity");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one terminal proof-output invocation expected")
    };
    let [output] = invocation.outputs.as_slice() else {
        panic!("one terminal proof output expected")
    };
    assert_eq!(invocation.evidence_arguments.len(), 2);
    assert_eq!(output.forwarded_input_position, Some(1));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("forwarding the second duplicate witness lane should verify");
}

#[test]
fn proof_output_is_canonical_verified_and_runtime_erased() {
    let checked = check(PROOF_OUTPUT_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::relay")
        .expect("proof-output call should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one terminal proof-output invocation expected")
    };
    assert_eq!(invocation.ordinal, 0);
    let [output] = invocation.outputs.as_slice() else {
        panic!("one terminal proof-output output expected")
    };
    assert_eq!(output.output_position, 0);
    assert_eq!(output.output_field, "outgoing");
    assert_ne!(
        output.callee_output.expect("producer-backed callee output"),
        output.output.expect("bound output")
    );
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 1);

    let bytes = encode_module(&lowered.semantic_module).expect("proof-output module encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof-output proof encodes");
    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("exact proof-output call and callee producer verify");
    derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("proof-only invocation adds no runtime fuel obligation");

    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[],
    )
    .expect("proof-only proof output requires no runtime argument");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("execute erased proof output"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );

    let mut forged = lowered.semantic_module.clone();
    forged.proof_output_calls[0].outputs[0].output =
        forged.proof_output_calls[0].outputs[0].callee_output;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&forged),
        Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));

    let baseline = semantic_fingerprint(&lowered.semantic_module).expect("proof-output identity");
    let mut renamed_target = lowered.semantic_module.clone();
    renamed_target.proof_output_calls[0].target_machine_identity =
        "different canonical producer".to_owned();
    assert_ne!(
        semantic_fingerprint(&renamed_target).expect("renamed target is distinct semantics"),
        baseline
    );
    let mut renamed_field = lowered.semantic_module.clone();
    renamed_field.proof_output_calls[0].outputs[0].output_field = "renamed".to_owned();
    assert_ne!(
        semantic_fingerprint(&renamed_field).expect("renamed field is distinct semantics"),
        baseline
    );
    let mut non_dense = lowered.semantic_module.clone();
    non_dense.proof_output_calls[0].ordinal = 1;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&non_dense),
        Err(psi_terminal_verifier::ModuleError::NonCanonicalProofOutputCall { .. })
    ));
    let mut empty_target = lowered.semantic_module.clone();
    empty_target.proof_output_calls[0]
        .target_machine_identity
        .clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&empty_target),
        Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
    let mut reserved_field = lowered.semantic_module.clone();
    reserved_field.proof_output_calls[0].outputs[0].output_field = "value".to_owned();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&reserved_field),
        Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
}

#[test]
fn runtime_unit_proof_output_links_and_executes_its_ordinary_call() {
    let checked = check(RUNTIME_UNIT_PROOF_OUTPUT_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::relay")
        .expect("runtime Unit proof output should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one runtime Unit proof-output invocation expected")
    };
    assert_eq!(
        invocation.runtime_result,
        Some(psi_terminal::ProofOutputRuntimeResult::Unit)
    );
    let runtime_call = invocation
        .runtime_call
        .expect("the proof-output row links its ordinary Unit call");
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == invocation.caller)
        .expect("proof-output caller machine");
    let operation = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == runtime_call.operation)
        .expect("linked Unit call operation");
    assert!(matches!(
        operation.kind,
        OperationKind::CallUnit { callee, .. } if callee == runtime_call.callee
    ));

    let bytes = encode_module(&lowered.semantic_module).expect("runtime Unit module encodes");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("runtime Unit proof encodes");
    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the Unit operation and proof-output row verify together");
    assert!(
        derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("the retained Unit call has fixed fuel")
            .ceiling_units()
            > 0
    );
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[],
    )
    .expect("runtime Unit proof-output artifact starts");
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded())
            .expect("execute runtime Unit proof output"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );

    let mut missing_link = lowered.semantic_module.clone();
    missing_link.proof_output_calls[0].runtime_call = None;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&missing_link),
        Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
}

#[test]
fn static_requirement_proof_output_keeps_public_identity_and_private_dispatch_separate() {
    let checked = check(STATIC_REQUIREMENT_PROOF_OUTPUT_SOURCE);
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .find_map(|(_, invocation)| {
            invocation
                .static_requirement_dispatch
                .as_ref()
                .map(|_| invocation)
        })
        .expect("one checked static requirement proof-output call");
    let machine_name = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| {
            (selection.machine == invocation.caller_machine_symbol)
                .then_some(selection.name.clone())
        })
        .expect("the specialized requirement caller is terminal-selected");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, &machine_name)
        .expect("static requirement proof output should cross terminal Psi");
    let baseline_checked = check(STATIC_REQUIREMENT_RUNTIME_BASELINE_SOURCE);
    let baseline_machine_name = baseline_checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| {
            baseline_checked
                .machine_specializations
                .iter()
                .find(|specialization| {
                    specialization.instance == selection.machine
                        && !specialization.conformance_applications.is_empty()
                })
                .map(|_| selection.name.clone())
        })
        .expect("the baseline specialized requirement caller is terminal-selected");
    let baseline =
        psi_checked_trees_to_terminal::lower_machine(&baseline_checked, &baseline_machine_name)
            .expect("matching runtime-only static requirement call should lower");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one static requirement proof-output invocation expected")
    };
    let dispatch = invocation
        .static_requirement_dispatch
        .as_ref()
        .expect("the private static realization remains explicit");
    let [argument] = invocation.evidence_arguments.as_slice() else {
        panic!("one public requirement input expected")
    };
    let [output] = invocation.outputs.as_slice() else {
        panic!("one public requirement output expected")
    };
    assert_eq!(
        invocation.target_machine_identity,
        dispatch.public_requirement_identity
    );
    assert_ne!(
        invocation.target_machine_identity, dispatch.realization_identity,
        "the public requirement identity must not become its private realization"
    );
    assert_eq!(output.output_field, "public_out");
    assert!(
        invocation
            .outputs
            .iter()
            .all(|output| output.output_field != "private_out"),
        "the strengthening-only satisfier selector must stay private"
    );
    assert_eq!(output.forwarded_input_position, None);
    assert_eq!(
        output.callee_output, None,
        "no satisfier or requirement callee term identity crosses the static abstraction"
    );
    assert_ne!(output.output, Some(argument.source));
    assert_eq!(
        invocation.runtime_result,
        Some(psi_terminal::ProofOutputRuntimeResult::Unit)
    );
    assert_eq!(
        invocation.runtime_call.map(|call| call.callee),
        Some(dispatch.realization)
    );
    let application = lowered
        .semantic_module
        .closed_conformance_applications
        .iter()
        .find(|application| {
            application.owner == invocation.caller
                && application.report_fingerprint
                    == dispatch.conformance_application_report_fingerprint
                && application.commitment == dispatch.conformance_application_commitment
        })
        .expect("the dispatch rejoins its exact closed application");
    assert_eq!(
        application.trait_identity,
        dispatch.declaring_trait_identity
    );
    let bound_row = application
        .rows
        .iter()
        .find(|row| {
            row.declaring_trait_identity == dispatch.declaring_trait_identity
                && row.requirement_identity == dispatch.requirement_identity
                && row.realization_identity == dispatch.realization_identity
        })
        .expect("the exact conformance row remains present");
    let realization_callable_identity = bound_row
        .realization_callable_identity
        .as_ref()
        .expect("static dispatch references its standalone callable registry");
    assert_eq!(
        realization_callable_identity.as_str(),
        dispatch.realization_callable_identity.as_str()
    );
    let realization_callable = application
        .realization_callables
        .iter()
        .find(|callable| callable.source_callable_identity == *realization_callable_identity)
        .expect("the row reference resolves in its application registry");
    assert_eq!(realization_callable.machine, dispatch.realization);
    assert_eq!(
        lowered
            .semantic_module
            .closed_conformance_applications
            .iter()
            .flat_map(|application| &application.realization_callables)
            .count(),
        1,
        "the bounded static call publishes exactly one canonical registry entry"
    );
    assert_eq!(
        lowered
            .semantic_module
            .closed_conformance_applications
            .iter()
            .flat_map(|application| &application.rows)
            .filter(|row| row.realization_callable_identity.is_some())
            .count(),
        1,
        "exactly the selected row references the canonical registry"
    );
    assert!(
        baseline
            .semantic_module
            .closed_conformance_applications
            .iter()
            .all(|application| {
                application.realization_callables.is_empty()
                    && application
                        .rows
                        .iter()
                        .all(|row| row.realization_callable_identity.is_none())
            }),
        "the matching runtime-only conformance remains map-free"
    );
    let baseline_bytes =
        encode_module(&baseline.semantic_module).expect("map-free baseline module encodes");
    let baseline_decoded =
        decode_module(&baseline_bytes).expect("map-free baseline module decodes");
    assert!(
        baseline_decoded
            .closed_conformance_applications
            .iter()
            .all(|application| {
                application.realization_callables.is_empty()
                    && application
                        .rows
                        .iter()
                        .all(|row| row.realization_callable_identity.is_none())
            }),
        "codec roundtrip preserves absence rather than synthesizing a map"
    );
    let mut spurious_runtime_registry = baseline.semantic_module.clone();
    let spurious_application = spurious_runtime_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| !application.rows.is_empty())
        .expect("runtime-only fixture retains its closed application");
    let spurious_identity = "forged::runtime-only-callable".to_owned();
    spurious_application.realization_callables =
        vec![psi_terminal::ClosedConformanceRealizationCallable {
            source_callable_identity: spurious_identity.clone(),
            machine: spurious_application.owner,
        }];
    spurious_application.rows[0].realization_callable_identity = Some(spurious_identity);
    spurious_application.report_fingerprint =
        psi_terminal::closed_conformance_application_report_fingerprint(spurious_application);
    spurious_application.commitment =
        psi_terminal::closed_conformance_application_commitment(spurious_application);
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&spurious_runtime_registry),
        Err(psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));

    let mut widened_static_registry = lowered.semantic_module.clone();
    let widened_application = widened_static_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    let second_identity = format!(
        "{}::second",
        widened_application.realization_callables[0].source_callable_identity
    );
    widened_application.realization_callables.push(
        psi_terminal::ClosedConformanceRealizationCallable {
            source_callable_identity: second_identity.clone(),
            machine: widened_application.owner,
        },
    );
    widened_application.realization_callables.sort();
    let mut second_row = widened_application.rows[0].clone();
    second_row.requirement_identity.push_str("::second");
    second_row.realization_callable_identity = Some(second_identity);
    widened_application.rows.push(second_row);
    widened_application.report_fingerprint =
        psi_terminal::closed_conformance_application_report_fingerprint(widened_application);
    widened_application.commitment =
        psi_terminal::closed_conformance_application_commitment(widened_application);
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&widened_static_registry),
        Err(psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));
    let mut changed_binding = application.clone();
    changed_binding
        .realization_callables
        .first_mut()
        .expect("standalone callable binding")
        .source_callable_identity
        .push_str("::changed");
    assert_ne!(
        psi_terminal::closed_conformance_application_report_fingerprint(&changed_binding),
        application.report_fingerprint
    );
    assert_ne!(
        psi_terminal::closed_conformance_application_commitment(&changed_binding),
        application.commitment
    );
    assert!(
        lowered.proof_bundle.evidence_producers.is_empty(),
        "the satisfier's private forwarded producer must not escape"
    );
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("named static entry");
    let baseline_entry = baseline
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == baseline.semantic_module.entry)
        .expect("runtime baseline entry");
    assert_eq!(entry.parameters, baseline_entry.parameters);
    assert_eq!(
        entry.structural_parameters,
        baseline_entry.structural_parameters
    );
    assert_eq!(entry.result, baseline_entry.result);
    assert_eq!(entry.structural_places, baseline_entry.structural_places);
    let operation_kinds = |machine: &psi_terminal::TerminalMachine| {
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| std::mem::discriminant(&operation.kind))
            .collect::<Vec<_>>()
    };
    assert_eq!(operation_kinds(entry), operation_kinds(baseline_entry));
    assert_eq!(
        lowered.semantic_module.machines.len(),
        baseline.semantic_module.machines.len(),
        "named proof lanes do not change the executable closure"
    );
    for (machine, baseline_machine) in lowered
        .semantic_module
        .machines
        .iter()
        .zip(&baseline.semantic_module.machines)
    {
        assert_eq!(machine.parameters, baseline_machine.parameters);
        assert_eq!(
            machine.structural_parameters,
            baseline_machine.structural_parameters
        );
        assert_eq!(machine.result, baseline_machine.result);
        assert_eq!(
            machine.structural_places,
            baseline_machine.structural_places
        );
        assert_eq!(machine.blocks, baseline_machine.blocks);
    }

    let bytes = encode_module(&lowered.semantic_module).expect("static dispatch module encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the exact public/private dispatch split verifies");
    let baseline_verified = psi_terminal_verifier::verify_module(
        &baseline.semantic_module,
        &baseline.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the matching runtime-only static call verifies");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("named static call has fixed fuel")
            .ceiling_units(),
        derive_fixed_entry_fuel(&baseline_verified, baseline.semantic_module.entry)
            .expect("baseline static call has fixed fuel")
            .ceiling_units(),
        "erased named lanes add no runtime fuel"
    );

    let rejects = |mut module: psi_terminal::TerminalModule,
                   mutate: fn(&mut psi_terminal::StaticRequirementDispatch)| {
        mutate(
            module.proof_output_calls[0]
                .static_requirement_dispatch
                .as_mut()
                .expect("static dispatch"),
        );
        assert!(matches!(
            psi_terminal_verifier::validate_module_representation(&module),
            Err(
                psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. }
                    | psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. }
            )
        ));
    };
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.conformance_application_report_fingerprint ^= 1;
    });
    let original_application = lowered.semantic_module.closed_conformance_applications[0].clone();
    let mut compact_equal_substitute = original_application.clone();
    compact_equal_substitute
        .trait_arguments
        .push("forged-static-argument".to_owned());
    let substitute_commitment =
        psi_terminal::closed_conformance_application_commitment(&compact_equal_substitute);
    assert_ne!(substitute_commitment, original_application.commitment);
    let mut substituted_dispatch = lowered.semantic_module.clone();
    let dispatch = substituted_dispatch.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch");
    let compact_coordinate = dispatch.conformance_application_report_fingerprint;
    dispatch.conformance_application_commitment = substitute_commitment;
    assert_eq!(
        dispatch.conformance_application_report_fingerprint,
        compact_coordinate
    );
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&substituted_dispatch),
        Err(
            psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. }
                | psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. }
        )
    ));
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.public_requirement_identity.push_str("::forged");
    });
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.declaring_trait_identity.push_str("::forged");
    });
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.requirement_identity.push_str("::forged");
    });
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.realization_identity.push_str("::forged");
    });
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.realization_callable_identity.push_str("::forged");
    });
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.realization = psi_core::MachineId::new(999).unwrap();
    });
    let mut drifted_callable_binding = lowered.semantic_module.clone();
    let application = drifted_callable_binding
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    application
        .rows
        .iter_mut()
        .find_map(|row| row.realization_callable_identity.as_mut())
        .expect("standalone callable reference")
        .push_str("::forged");
    application.report_fingerprint =
        psi_terminal::closed_conformance_application_report_fingerprint(application);
    application.commitment = psi_terminal::closed_conformance_application_commitment(application);
    let drifted_dispatch = drifted_callable_binding.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch");
    drifted_dispatch.conformance_application_report_fingerprint = application.report_fingerprint;
    drifted_dispatch.conformance_application_commitment = application.commitment;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&drifted_callable_binding),
        Err(psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));

    let mut unused_callable_registry = lowered.semantic_module.clone();
    let application = unused_callable_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    application
        .rows
        .iter_mut()
        .find_map(|row| row.realization_callable_identity.take())
        .expect("selected row registry reference");
    application.report_fingerprint =
        psi_terminal::closed_conformance_application_report_fingerprint(application);
    application.commitment = psi_terminal::closed_conformance_application_commitment(application);
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unused_callable_registry),
        Err(psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));

    let mut duplicate_callable_registry = lowered.semantic_module.clone();
    let application = duplicate_callable_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    application
        .realization_callables
        .push(application.realization_callables[0].clone());
    application.report_fingerprint =
        psi_terminal::closed_conformance_application_report_fingerprint(application);
    application.commitment = psi_terminal::closed_conformance_application_commitment(application);
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&duplicate_callable_registry),
        Err(psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));

    let mut retargeted_callable_registry = lowered.semantic_module.clone();
    let application = retargeted_callable_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    application.realization_callables[0].machine = application.owner;
    application.report_fingerprint =
        psi_terminal::closed_conformance_application_report_fingerprint(application);
    application.commitment = psi_terminal::closed_conformance_application_commitment(application);
    let report_fingerprint = application.report_fingerprint;
    let commitment = application.commitment;
    let dispatch = retargeted_callable_registry.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch");
    dispatch.conformance_application_report_fingerprint = report_fingerprint;
    dispatch.conformance_application_commitment = commitment;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&retargeted_callable_registry),
        Err(
            psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. }
                | psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. }
        )
    ));

    let mut coordinated_dispatch_retarget = lowered.semantic_module.clone();
    let original_realization = invocation.runtime_call.expect("runtime call").callee;
    let alternate = psi_core::MachineId::new(999).expect("alternate machine identity");
    let mut alternate_machine = coordinated_dispatch_retarget
        .machines
        .iter()
        .find(|machine| machine.id == original_realization)
        .expect("static realization machine")
        .clone();
    alternate_machine.id = alternate;
    alternate_machine.contract.id = psi_core::ContractId::new(999).expect("alternate contract");
    for (ordinal, ensure) in alternate_machine.contract.ensures.iter_mut().enumerate() {
        ensure.obligation =
            psi_core::ObligationId::new(9_900 + ordinal as u64).expect("alternate obligation");
    }
    for (ordinal, ensure) in alternate_machine
        .contract
        .outcome_specific_ensures
        .iter_mut()
        .enumerate()
    {
        ensure.obligation = psi_core::ObligationId::new(9_950 + ordinal as u64)
            .expect("alternate guarded obligation");
    }
    coordinated_dispatch_retarget
        .machines
        .push(alternate_machine);
    let runtime_operation = coordinated_dispatch_retarget.proof_output_calls[0]
        .runtime_call
        .expect("runtime call")
        .operation;
    let original_realization_identity = coordinated_dispatch_retarget.proof_output_calls[0]
        .static_requirement_dispatch
        .as_ref()
        .expect("static dispatch")
        .realization_identity
        .clone();
    let application = coordinated_dispatch_retarget
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    let row = application
        .rows
        .iter_mut()
        .find(|row| row.realization_identity == original_realization_identity)
        .expect("selected static conformance row");
    row.realization_identity.push_str("::coordinated-retarget");
    let retargeted_realization_identity = row.realization_identity.clone();
    let callable_identity = row
        .realization_callable_identity
        .clone()
        .expect("selected row references the independent registry");
    assert_eq!(
        application
            .realization_callables
            .iter()
            .find(|callable| callable.source_callable_identity == callable_identity)
            .expect("standalone callable binding")
            .machine,
        original_realization,
        "the independent source-callable map remains rooted at the original machine"
    );
    application.report_fingerprint =
        psi_terminal::closed_conformance_application_report_fingerprint(application);
    application.commitment = psi_terminal::closed_conformance_application_commitment(application);
    let application_report_fingerprint = application.report_fingerprint;
    let application_commitment = application.commitment;
    coordinated_dispatch_retarget.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime call")
        .callee = alternate;
    let coordinated_dispatch = coordinated_dispatch_retarget.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch");
    coordinated_dispatch.conformance_application_report_fingerprint =
        application_report_fingerprint;
    coordinated_dispatch.conformance_application_commitment = application_commitment;
    coordinated_dispatch.realization_identity = retargeted_realization_identity;
    coordinated_dispatch.realization = alternate;
    let operation = coordinated_dispatch_retarget
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.id == runtime_operation)
        .expect("linked runtime operation");
    let OperationKind::CallUnit { callee, .. } = &mut operation.kind else {
        panic!("Unit fixture retains one ordinary Unit call")
    };
    *callee = alternate;
    let coordinated_result =
        psi_terminal_verifier::validate_module_representation(&coordinated_dispatch_retarget);
    assert!(
        matches!(
            coordinated_result,
            Err(
                psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. }
                    | psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. }
            )
        ),
        "coordinated dispatch retarget must reject through its unchanged callable binding, got {coordinated_result:?}"
    );
    let mut deleted_dispatch = lowered.semantic_module.clone();
    deleted_dispatch.proof_output_calls[0].static_requirement_dispatch = None;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&deleted_dispatch),
        Err(psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));
    let mut leaked_input = lowered.semantic_module.clone();
    let input_source = leaked_input.proof_output_calls[0].evidence_arguments[0].source;
    leaked_input.proof_output_calls[0].outputs[0].output = Some(input_source);
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&leaked_input),
        Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
    let mut exposed_callee = lowered.semantic_module.clone();
    exposed_callee.proof_output_calls[0].outputs[0].callee_output = Some(input_source);
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&exposed_callee),
        Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
    let mut renamed_row = lowered.semantic_module.clone();
    let renamed_entry = renamed_row.entry;
    let application = renamed_row
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == renamed_entry)
        .expect("entry closed application");
    application.rows[0]
        .public_requirement_identity
        .push_str("::forged");
    application.report_fingerprint =
        psi_terminal::closed_conformance_application_report_fingerprint(application);
    application.commitment = psi_terminal::closed_conformance_application_commitment(application);
    renamed_row.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_report_fingerprint = application.report_fingerprint;
    renamed_row.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_commitment = application.commitment;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&renamed_row),
        Err(psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));
    let mut deleted_row = lowered.semantic_module.clone();
    let deleted_entry = deleted_row.entry;
    let application = deleted_row
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == deleted_entry)
        .expect("entry closed application");
    application.rows.clear();
    application.report_fingerprint =
        psi_terminal::closed_conformance_application_report_fingerprint(application);
    application.commitment = psi_terminal::closed_conformance_application_commitment(application);
    deleted_row.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_report_fingerprint = application.report_fingerprint;
    deleted_row.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_commitment = application.commitment;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&deleted_row),
        Err(psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));
    let mut widened_application = lowered.semantic_module.clone();
    let widened_entry = widened_application.entry;
    let application = widened_application
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == widened_entry)
        .expect("entry closed application");
    application.trait_arguments.push("forged".to_owned());
    application.report_fingerprint =
        psi_terminal::closed_conformance_application_report_fingerprint(application);
    application.commitment = psi_terminal::closed_conformance_application_commitment(application);
    widened_application.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_report_fingerprint = application.report_fingerprint;
    widened_application.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_commitment = application.commitment;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&widened_application),
        Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
}

#[test]
fn static_requirement_i32_result_uses_one_ordinary_scalar_call_without_runtime_overhead() {
    let checked = check(STATIC_REQUIREMENT_I32_PROOF_OUTPUT_SOURCE);
    let checked_invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .find_map(|(_, invocation)| {
            invocation
                .static_requirement_dispatch
                .as_ref()
                .map(|_| invocation)
        })
        .expect("one checked i32 static requirement proof-output call");
    let machine_name = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| {
            (selection.machine == checked_invocation.caller_machine_symbol)
                .then_some(selection.name.clone())
        })
        .expect("the free specialized i32 caller is terminal-selected");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, &machine_name)
        .expect("the exact i32 static requirement call crosses Terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one terminal i32 static requirement proof-output call")
    };
    let dispatch = invocation
        .static_requirement_dispatch
        .as_ref()
        .expect("private i32 realization dispatch");
    let i32_type = psi_core::ScalarType::Integer(
        psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).expect("i32"),
    );
    assert_eq!(
        invocation.runtime_result,
        Some(psi_terminal::ProofOutputRuntimeResult::Scalar(i32_type))
    );
    let runtime_call = invocation
        .runtime_call
        .expect("static i32 proof output links its ordinary call");
    assert_eq!(runtime_call.callee, dispatch.realization);
    let application = lowered
        .semantic_module
        .closed_conformance_applications
        .iter()
        .find(|application| {
            application.owner == invocation.caller
                && application.commitment == dispatch.conformance_application_commitment
        })
        .expect("i32 dispatch retains its closed application");
    let row_identity = application
        .rows
        .iter()
        .find_map(|row| row.realization_callable_identity.as_ref())
        .expect("i32 row references its standalone callable registry");
    let callable = application
        .realization_callables
        .iter()
        .find(|callable| callable.source_callable_identity == *row_identity)
        .expect("i32 registry retains its standalone callable binding");
    assert_eq!(callable.machine, dispatch.realization);
    assert_eq!(
        callable.source_callable_identity,
        dispatch.realization_callable_identity
    );
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == invocation.caller)
        .expect("i32 proof-output caller");
    assert_eq!(caller.attachment, None, "the scalar caller stays free");
    let realization = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == dispatch.realization)
        .expect("i32 static realization");
    assert_eq!(
        realization.attachment, None,
        "no runtime receiver is emitted"
    );
    assert!(realization.parameters.is_empty());
    assert!(realization.structural_parameters.is_empty());
    assert!(matches!(
        realization.result,
        psi_terminal::TerminalMachineResult::Scalar(result)
            if result.scalar_type == i32_type
    ));
    let operation = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == runtime_call.operation)
        .expect("linked i32 call operation");
    assert!(matches!(
        (&operation.result, &operation.kind),
        (
            psi_terminal::OperationResult::Scalar(result),
            OperationKind::Call {
                callee,
                arguments,
                ..
            }
        ) if result.scalar_type == i32_type
            && *callee == dispatch.realization
            && arguments.is_empty()
    ));

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("static i32 module encodes");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("static i32 proof encodes");
    let decoded = decode_module(&semantic_bytes).expect("static i32 module decodes");
    assert_eq!(decoded, lowered.semantic_module);
    let decoded_proof = decode_proof_bundle(&proof_bytes).expect("static i32 proof decodes");
    assert_eq!(decoded_proof, lowered.proof_bundle);
    let verified = psi_terminal_verifier::verify_module(
        &decoded,
        &decoded_proof,
        &AdmissionProfile::default(),
    )
    .expect("the exact static i32 operation and dispatch verify together");

    let baseline_checked = check(STATIC_REQUIREMENT_I32_RUNTIME_BASELINE_SOURCE);
    let baseline_name = baseline_checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| (selection.name == "caller").then(|| selection.name.clone()))
        .expect("the scalar runtime baseline is terminal-selected");
    let baseline = psi_checked_trees_to_terminal::lower_machine(&baseline_checked, &baseline_name)
        .expect("the scalar runtime baseline lowers");
    let baseline_verified = psi_terminal_verifier::verify_module(
        &baseline.semantic_module,
        &baseline.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the scalar runtime baseline verifies");
    let baseline_entry = baseline
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == baseline.semantic_module.entry)
        .expect("scalar baseline entry");
    assert_eq!(caller.parameters, baseline_entry.parameters);
    assert_eq!(caller.result, baseline_entry.result);
    assert_eq!(
        caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| std::mem::discriminant(&operation.kind))
            .collect::<Vec<_>>(),
        baseline_entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| std::mem::discriminant(&operation.kind))
            .collect::<Vec<_>>(),
        "erased static proof rows add no runtime operations"
    );
    assert_eq!(
        derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("static i32 call has fixed fuel")
            .ceiling_units(),
        derive_fixed_entry_fuel(&baseline_verified, baseline.semantic_module.entry)
            .expect("baseline i32 call has fixed fuel")
            .ceiling_units(),
        "erased static proof rows add no runtime fuel"
    );

    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        &[],
    )
    .expect("static i32 artifact starts");
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded())
            .expect("execute static i32 requirement call"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                scalar_type: psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
                    .expect("i32"),
                value: IntegerValue::Signed(17),
            }
        ))
    );

    let invalid = |module: &psi_terminal::TerminalModule| {
        matches!(
            psi_terminal_verifier::validate_module_representation(module),
            Err(
                psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. }
                    | psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. }
            )
        )
    };
    let mut wrong_type = lowered.semantic_module.clone();
    wrong_type.proof_output_calls[0].runtime_result = Some(
        psi_terminal::ProofOutputRuntimeResult::Scalar(psi_core::ScalarType::Boolean),
    );
    assert!(invalid(&wrong_type));
    let mut unit_result = lowered.semantic_module.clone();
    unit_result.proof_output_calls[0].runtime_result =
        Some(psi_terminal::ProofOutputRuntimeResult::Unit);
    assert!(invalid(&unit_result));
    let mut unknown_operation = lowered.semantic_module.clone();
    unknown_operation.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime call")
        .operation = OperationId::new(u64::MAX).expect("nonzero operation ID");
    assert!(invalid(&unknown_operation));
    let mut wrong_callee = lowered.semantic_module.clone();
    wrong_callee.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime call")
        .callee = invocation.caller;
    assert!(invalid(&wrong_callee));
    let mut wrong_realization = lowered.semantic_module.clone();
    wrong_realization.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .realization = invocation.caller;
    assert!(invalid(&wrong_realization));
    let mut wrong_operation = lowered.semantic_module.clone();
    wrong_operation
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|candidate| candidate.id == runtime_call.operation)
        .expect("linked operation")
        .kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(17),
    };
    assert!(invalid(&wrong_operation));

    let mut forged_argument = lowered.semantic_module.clone();
    let argument = psi_terminal::ValueDeclaration {
        id: ValueId::new(u64::MAX).expect("nonzero forged argument ID"),
        scalar_type: i32_type,
    };
    forged_argument
        .machines
        .iter_mut()
        .find(|machine| machine.id == invocation.caller)
        .expect("forged caller")
        .parameters
        .push(argument);
    forged_argument
        .machines
        .iter_mut()
        .find(|machine| machine.id == dispatch.realization)
        .expect("forged realization")
        .parameters
        .push(psi_terminal::ValueDeclaration {
            id: ValueId::new(u64::MAX - 1).expect("nonzero forged parameter ID"),
            scalar_type: i32_type,
        });
    let OperationKind::Call { arguments, .. } = &mut forged_argument
        .machines
        .iter_mut()
        .find(|machine| machine.id == invocation.caller)
        .expect("forged caller")
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|candidate| candidate.id == runtime_call.operation)
        .expect("forged linked operation")
        .kind
    else {
        panic!("linked operation remains a scalar call")
    };
    arguments.push(argument.id);
    assert!(
        invalid(&forged_argument),
        "a coherent argument-bearing scalar dispatch remains outside the bounded rung"
    );
}

#[test]
fn ordinary_attached_scalar_machine_remains_outside_the_scalar_entry_lane() {
    let checked = check(ORDINARY_ATTACHED_SCALAR_SOURCE);
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|selection| selection.name == "Root::f")
        .expect("ordinary attached scalar machine selection");
    assert_eq!(
        selection.signature,
        psi_checked_trees::CheckedTerminalSignatureEligibility::Attached
    );
    assert!(matches!(
        psi_checked_trees_to_terminal::lower_machine(&checked, &selection.name),
        Err(psi_checked_trees_to_terminal::LoweringError::Unsupported(_))
    ));
}

#[test]
fn plural_static_requirement_proof_outputs_preserve_order_identity_and_freshness() {
    let checked = check(PLURAL_STATIC_REQUIREMENT_PROOF_OUTPUT_SOURCE);
    let checked_invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .find_map(|(_, invocation)| {
            invocation
                .static_requirement_dispatch
                .as_ref()
                .map(|_| invocation)
        })
        .expect("one checked plural static requirement proof-output call");
    let machine_name = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| {
            (selection.machine == checked_invocation.caller_machine_symbol)
                .then_some(selection.name.clone())
        })
        .expect("the plural specialized requirement caller is terminal-selected");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, &machine_name)
        .expect("plural static requirement proof outputs should cross Terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one plural static requirement proof-output invocation expected")
    };
    assert!(invocation.static_requirement_dispatch.is_some());
    assert_eq!(invocation.evidence_arguments.len(), 2);
    assert_eq!(invocation.outputs.len(), 3);
    assert_eq!(
        invocation
            .evidence_arguments
            .iter()
            .map(|argument| argument.input_position)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(
        invocation
            .outputs
            .iter()
            .map(|output| (output.output_position, output.output_field.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, "first_copy"), (1, "second_copy"), (2, "third_copy")]
    );
    assert!(
        invocation
            .outputs
            .iter()
            .all(|output| output.forwarded_input_position.is_none()
                && output.callee_output.is_none()
                && output.output.is_some()),
        "static dispatch must expose only fresh public output identities"
    );
    let output_terms = invocation
        .outputs
        .iter()
        .map(|output| output.output.expect("fresh static output term"))
        .collect::<Vec<_>>();
    assert!(output_terms.windows(2).all(|pair| pair[0] != pair[1]));
    assert!(
        invocation
            .evidence_arguments
            .iter()
            .all(|argument| { !output_terms.contains(&argument.source) })
    );

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("encode plural static semantics");
    let proof_bytes =
        encode_proof_bundle(&lowered.proof_bundle).expect("encode plural static proof bundle");
    let decoded_semantic = decode_module(&semantic_bytes).expect("decode plural static semantics");
    let decoded_proof =
        decode_proof_bundle(&proof_bytes).expect("decode plural static proof bundle");
    assert_eq!(decoded_semantic, lowered.semantic_module);
    assert_eq!(decoded_proof, lowered.proof_bundle);
    psi_terminal_verifier::verify_module(
        &decoded_semantic,
        &decoded_proof,
        &AdmissionProfile::default(),
    )
    .expect("plural subjectless static named-witness lanes verify");

    let rejects = |label: &str, module: &psi_terminal::TerminalModule| {
        let result = psi_terminal_verifier::validate_module_representation(module);
        assert!(
            matches!(
                result,
                Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
            ),
            "{label} mutation must reject as an invalid proof-output call, got {result:?}"
        );
    };

    let mut reordered_arguments = lowered.semantic_module.clone();
    reordered_arguments.proof_output_calls[0]
        .evidence_arguments
        .swap(0, 1);
    rejects("argument order", &reordered_arguments);

    let mut reordered_outputs = lowered.semantic_module.clone();
    reordered_outputs.proof_output_calls[0].outputs.swap(0, 1);
    rejects("output order", &reordered_outputs);

    let mut wrong_proposition = lowered.semantic_module.clone();
    wrong_proposition.proof_output_calls[0].outputs[2].instantiated_proposition =
        psi_core::PropositionId::new(999).expect("forged proposition identity");
    rejects("proposition", &wrong_proposition);

    let mut wrong_interface = lowered.semantic_module.clone();
    let third_output = wrong_interface.proof_output_calls[0].outputs[2]
        .output
        .expect("third output term");
    wrong_interface
        .evidence_terms
        .iter_mut()
        .find(|term| term.id == third_output)
        .expect("third output declaration")
        .interface
        .trait_identity
        .push_str("::forged");
    assert_eq!(
        psi_terminal_verifier::validate_module_representation(&wrong_interface),
        Err(psi_terminal_verifier::ModuleError::EvidenceTermInterfaceMismatch(third_output))
    );

    let mut reused_output = lowered.semantic_module.clone();
    let first_output = reused_output.proof_output_calls[0].outputs[0].output;
    reused_output.proof_output_calls[0].outputs[1].output = first_output;
    rejects("freshness", &reused_output);
}

#[test]
fn static_requirement_trait_default_rejoins_exact_application_and_runtime_callee() {
    let checked = check(STATIC_REQUIREMENT_TRAIT_DEFAULT_SOURCE);
    let checked_invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .find_map(|(_, invocation)| {
            invocation
                .static_requirement_dispatch
                .as_ref()
                .map(|dispatch| (invocation, dispatch))
        })
        .expect("one checked trait-default static dispatch");
    let expected_public_requirement = checked
        .symbols
        .display_path(checked_invocation.1.requirement, "::");
    let expected_realization = checked
        .symbols
        .display_path(checked_invocation.1.realization_state, "::");
    let machine_name = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| {
            (selection.machine == checked_invocation.0.caller_machine_symbol)
                .then_some(selection.name.clone())
        })
        .expect("the specialized default caller is terminal-selected");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, &machine_name)
        .expect("the exact trait-default dispatch should cross Terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one trait-default proof-output invocation")
    };
    let dispatch = invocation
        .static_requirement_dispatch
        .as_ref()
        .expect("the trait-default dispatch remains explicit");
    assert_eq!(dispatch.requirement_identity, expected_public_requirement);
    assert_eq!(dispatch.realization_identity, expected_realization);
    assert_ne!(dispatch.conformance_application_report_fingerprint, 0);
    assert!(!dispatch.conformance_application_commitment.is_zero());
    assert_eq!(
        invocation.runtime_call.map(|call| call.callee),
        Some(dispatch.realization)
    );
    let application_index = lowered
        .semantic_module
        .closed_conformance_applications
        .iter()
        .position(|application| {
            application.owner == invocation.caller
                && application.commitment == dispatch.conformance_application_commitment
        })
        .expect("the dispatch rejoins its owner-scoped closed application");
    let application = &lowered.semantic_module.closed_conformance_applications[application_index];
    assert!(application.rows.iter().any(|row| {
        row.public_requirement_identity == dispatch.public_requirement_identity
            && row.realization_identity == dispatch.realization_identity
    }));
    let bytes = encode_module(&lowered.semantic_module).expect("default dispatch module encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the exact trait-default dispatch verifies");

    let mut declaration_drift = lowered.semantic_module.clone();
    declaration_drift.closed_conformance_applications[application_index]
        .declaration_identity
        .push_str("::forged");
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&declaration_drift),
        Err(psi_terminal_verifier::ModuleError::ClosedConformanceFingerprintMismatch { .. })
    ));

    let mut row_drift = lowered.semantic_module.clone();
    row_drift.closed_conformance_applications[application_index].rows[0]
        .realization_identity
        .push_str("::forged");
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&row_drift),
        Err(psi_terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));

    let mut runtime_drift = lowered.semantic_module.clone();
    runtime_drift.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime call")
        .callee = runtime_drift.entry;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&runtime_drift),
        Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
}

#[test]
fn proof_output_retains_copy_and_explicit_discard() {
    let checked = check(COPY_AND_DISCARD_PROOF_OUTPUT_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::relay")
        .expect("copyable and discarded evidence should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one proof-output call expected")
    };
    let [copied, discarded] = invocation.outputs.as_slice() else {
        panic!("two complete proof selectors expected")
    };
    let copied_term = copied.output.expect("copied field binds a caller term");
    assert_eq!(discarded.output, None);
    let relayed = lowered
        .semantic_module
        .evidence_contract_lanes
        .iter()
        .filter(|lane| {
            lane.machine == lowered.semantic_module.entry
                && lane.kind == EvidenceContractLaneKind::Ensures
        })
        .collect::<Vec<_>>();
    assert_eq!(relayed.len(), 2);
    assert!(relayed.iter().all(|lane| lane.term == copied_term));

    let bytes = encode_module(&lowered.semantic_module).expect("discard disposition encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("copy and discard preserve exact evidence provenance");

    let mut omitted = lowered.semantic_module.clone();
    omitted.proof_output_calls[0].outputs.pop();
    assert!(psi_terminal_verifier::validate_module_representation(&omitted).is_err());
}

#[test]
fn runtime_value_proof_output_links_one_scalar_call_and_executes_once() {
    let checked = check(RUNTIME_VALUE_PROOF_OUTPUT_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "relay")
        .expect("runtime value proof output should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one terminal runtime-value proof output expected")
    };
    let Some(psi_terminal::ProofOutputRuntimeResult::Scalar(runtime_type)) =
        invocation.runtime_result
    else {
        panic!("runtime proof output retains its scalar payload type")
    };
    let runtime_call = invocation
        .runtime_call
        .expect("runtime proof output retains its exact ordinary call operation");
    assert_eq!(runtime_type, psi_core::ScalarType::Boolean);
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == invocation.caller)
        .expect("proof-output caller machine");
    let calls = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        2,
        "one unrelated call precedes the proof-output call"
    );
    let call = calls
        .iter()
        .copied()
        .find(|operation| operation.id == runtime_call.operation)
        .expect("the retained ID selects the proof-output call, not the earlier call");
    assert_eq!(call.id, runtime_call.operation);
    assert!(matches!(
        call.kind,
        OperationKind::Call { callee, .. } if callee == runtime_call.callee
    ));

    let bytes =
        encode_module(&lowered.semantic_module).expect("runtime proof-output module encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    let proof =
        encode_proof_bundle(&lowered.proof_bundle).expect("runtime proof-output proof encodes");
    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("runtime proof-output operation and proof group verify together");
    let fuel = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("the ordinary scalar call has fixed fuel");
    assert!(fuel.ceiling_units() > 0);
    let baseline = check(
        r#"
            machine warmup() -> bool
            requires true == true
            ensures true == true
            { true }
            machine produce() -> bool
            requires true == true
            ensures true == true
            { true }
            machine relay() -> bool
            requires true == true
            ensures true == true
            { let warmed: bool = warmup(); let local: bool = produce(); local }
        "#,
    );
    let baseline = psi_checked_trees_to_terminal::lower_machine(&baseline, "relay")
        .expect("ordinary scalar-call baseline lowers");
    let baseline_verified = psi_terminal_verifier::verify_module(
        &baseline.semantic_module,
        &baseline.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("ordinary scalar-call baseline verifies");
    let baseline_fuel = derive_fixed_entry_fuel(&baseline_verified, baseline.semantic_module.entry)
        .expect("ordinary scalar-call baseline has fixed fuel");
    assert_eq!(
        fuel.ceiling_units(),
        baseline_fuel.ceiling_units(),
        "erased proof-output proof metadata adds no runtime fuel"
    );

    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[],
    )
    .expect("runtime proof-output artifact starts");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("execute runtime proof output"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );

    let invalid_proof_output = |module: &psi_terminal::TerminalModule| {
        matches!(
            psi_terminal_verifier::validate_module_representation(module),
            Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
        )
    };
    let mut unknown_operation = lowered.semantic_module.clone();
    unknown_operation.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime link")
        .operation = OperationId::new(u64::MAX).unwrap();
    assert!(invalid_proof_output(&unknown_operation));

    let mut wrong_kind = lowered.semantic_module.clone();
    let linked = wrong_kind.proof_output_calls[0]
        .runtime_call
        .expect("runtime link")
        .operation;
    wrong_kind
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.id == linked)
        .expect("linked operation")
        .kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(7),
    };
    assert!(invalid_proof_output(&wrong_kind));

    let mut wrong_caller = lowered.semantic_module.clone();
    wrong_caller.proof_output_calls[0].caller = runtime_call.callee;
    assert!(invalid_proof_output(&wrong_caller));

    let mut missing_link = lowered.semantic_module.clone();
    missing_link.proof_output_calls[0].runtime_call = None;
    assert!(invalid_proof_output(&missing_link));

    let mut mismatched_callee = lowered.semantic_module.clone();
    mismatched_callee.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime link")
        .callee = invocation.caller;
    assert!(invalid_proof_output(&mismatched_callee));

    let proof_only =
        psi_checked_trees_to_terminal::lower_machine(&check(PROOF_OUTPUT_SOURCE), "Root::relay")
            .expect("proof-only proof output");
    let mut spurious_link = proof_only.semantic_module;
    spurious_link.proof_output_calls[0].runtime_call = Some(runtime_call);
    assert!(invalid_proof_output(&spurious_link));
}

#[test]
fn multi_field_proof_output_is_complete_canonical_and_runtime_erased() {
    let checked = check(MULTI_FIELD_PROOF_OUTPUT_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::relay")
        .expect("complete multi-field proof output should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one grouped terminal proof-output invocation expected")
    };
    assert_eq!(invocation.ordinal, 0);
    let [first, second] = invocation.outputs.as_slice() else {
        panic!("two terminal proof-output outputs expected")
    };
    assert_eq!((first.output_position, second.output_position), (0, 1));
    assert_eq!(
        (first.output_field.as_str(), second.output_field.as_str()),
        ("first", "second")
    );
    assert_ne!(
        first.callee_output.expect("first producer-backed output"),
        first.output.expect("first bound output")
    );
    assert_ne!(
        second.callee_output.expect("second producer-backed output"),
        second.output.expect("second bound output")
    );
    assert_ne!(first.output, second.output);
    assert_ne!(first.callee_output, second.callee_output);
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 2);

    let bytes = encode_module(&lowered.semantic_module).expect("multi-field proof output encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("multi-field proof encodes");
    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("complete multi-field proof output verifies");
    derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("multi-field proof-only invocation adds no runtime fuel");

    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[],
    )
    .expect("multi-field proof-only proof output requires no runtime argument");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("execute erased proof output"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );

    let mut non_dense = lowered.semantic_module.clone();
    non_dense.proof_output_calls[0].outputs[1].output_position = 2;
    assert!(encode_module(&non_dense).is_err());
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&non_dense),
        Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));

    let mut aliased = lowered.semantic_module.clone();
    aliased.proof_output_calls[0].outputs[1].output =
        aliased.proof_output_calls[0].outputs[0].output;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&aliased),
        Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));

    let mut duplicate_field = lowered.semantic_module.clone();
    duplicate_field.proof_output_calls[0].outputs[1].output_field =
        duplicate_field.proof_output_calls[0].outputs[0]
            .output_field
            .clone();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&duplicate_field),
        Err(psi_terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));

    let mut incomplete = lowered.semantic_module.clone();
    incomplete.proof_output_calls[0].outputs.pop();
    assert!(psi_terminal_verifier::validate_module_representation(&incomplete).is_err());
}

#[test]
fn repeated_multi_field_proof_outputs_group_calls_and_reuse_callee_producers() {
    let checked = check(REPEATED_MULTI_FIELD_PROOF_OUTPUT_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::relay")
        .expect("repeated multi-field proof outputs should lower");
    let [first, second] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("two grouped proof-output calls expected")
    };
    assert_eq!((first.ordinal, second.ordinal), (0, 1));
    assert_eq!(first.outputs.len(), 2);
    assert_eq!(second.outputs.len(), 2);
    for position in 0..2 {
        assert_eq!(
            first.outputs[position].callee_output,
            second.outputs[position].callee_output
        );
        assert_ne!(
            first.outputs[position].output,
            second.outputs[position].output
        );
    }
    let caller_outputs = first
        .outputs
        .iter()
        .chain(&second.outputs)
        .map(|output| output.output)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(caller_outputs.len(), 4);
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 2);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("two calls reuse each callee producer and mint four caller terms");
}

#[test]
fn repeated_proof_output_calls_have_dense_fresh_outputs_and_one_callee_producer() {
    let checked = check(REPEATED_PROOF_OUTPUT_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::relay")
        .expect("repeated proof-output calls should retain distinct invocation terms");
    let [first, second] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("two invocation rows expected")
    };
    assert_eq!((first.ordinal, second.ordinal), (0, 1));
    assert_eq!(
        first.outputs[0].callee_output,
        second.outputs[0].callee_output
    );
    assert_ne!(first.outputs[0].output, second.outputs[0].output);
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 1);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("one callee declaration needs one producer regardless of invocation count");
}

#[test]
fn same_shape_proof_outputs_retain_distinct_canonical_callee_identities() {
    let checked = check(DISTINCT_PROOF_OUTPUT_PRODUCERS_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::relay")
        .expect("same-shape producer proof outputs should lower");
    let [first, second] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("two invocation rows expected")
    };
    assert_ne!(
        first.target_machine_identity,
        second.target_machine_identity
    );
    assert!(!first.target_machine_identity.is_empty());
    assert!(!second.target_machine_identity.is_empty());
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("canonical callee identity remains verified semantic data");
}

#[test]
fn empty_complete_evidence_conformance_remains_valid_provenance() {
    let checked = check(EMPTY_PRODUCER_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::produce")
        .expect("an empty closed conformance is still complete");
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 1);
    assert!(lowered.proof_bundle.evidence_producers[0].rows.is_empty());
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("empty row set is canonical");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the exact empty conformance verifies");
}

fn check(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}
