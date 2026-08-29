//! Root-level checked-to-terminal producer regressions.

use super::*;
use psi_language_semantics::{
    PermissionEventSource, SemanticDomainId,
    content::{
        ContentCaseSegment, ContentConservationEquation, ContentConservationOwnerKind,
        ContentFieldSegment,
    },
};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_symbols::SymbolHandle;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::BindingRelevance;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

mod attached_unit_cases;
mod content_conservation;
mod quotient_correspondence;
mod scalar_graph;
mod service_reach_contracts;
mod structural_control_cases;
mod structural_return_cases;
mod unit_cleanup;

fn checked_source(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn program_entry_receipt_binds_checked_source_to_canonical_terminal_entry() {
    let checked = checked_source(
        r#"
            data Main {}
            machine Main::launch() {}
        "#,
    );
    let source_signature_identity = [0x5a; 32];
    let produced = produce_program_entry_terminal_artifact(
        &checked,
        "Main::launch",
        source_signature_identity,
    )
    .expect("produce checked Unit ProgramEntry artifact");
    let receipt = produced.receipt();
    let decoded = psi_terminal_codec::decode_module(produced.artifact().semantic_bytes())
        .expect("decode canonical semantic module");

    assert_eq!(
        receipt.source_signature_identity(),
        source_signature_identity
    );
    assert_eq!(receipt.source_machine_name(), "Main::launch");
    assert_eq!(receipt.terminal_entry(), decoded.entry);
    assert_eq!(
        receipt.terminal_psi_identity(),
        produced.artifact().manifest().semantic()
    );
    assert!(
        decoded
            .machines
            .iter()
            .any(|machine| machine.id == receipt.terminal_entry()
                && machine.result == TerminalMachineResult::Unit)
    );
    produced
        .artifact()
        .validate()
        .expect("receipt-coupled artifact replays");
}

#[test]
fn program_entry_receipt_retains_two_granted_extent_roots_and_their_boundary_handoff() {
    let checked = checked_source(
        r#"
            data Extent [linear] {
                base: addr;
                length: u64;
            }

            boundary machine no_wrap(base: addr, length: u64) -> bool;

            domain Extent::Granted
            requires
                no_wrap(self.base, self.length)
            established by
                ProgramStorageEntry::enter;

            boundary trait ProgramStorageEntry {
                machine enter(
                    image: Extent in Granted,
                    initial_storage: Extent in Granted
                );
            }

            data ProgramLocalProducer {}
            machine ProgramLocalProducer::handoff<machine Enter>(
                image: Extent in Granted,
                initial_storage: Extent in Granted
            )
            where machine Enter satisfies ProgramStorageEntry::enter;
            {
                Enter(image, initial_storage);
            }
        "#,
    );
    let source_signature_identity = [0xa5; 32];
    let produced = produce_program_entry_terminal_artifact(
        &checked,
        "ProgramLocalProducer::handoff",
        source_signature_identity,
    )
    .expect("produce exact two-root Unit ProgramEntry artifact");
    let receipt = produced.receipt();
    let decoded = psi_terminal_codec::decode_module(produced.artifact().semantic_bytes())
        .expect("decode canonical two-root semantic module");

    assert_eq!(
        receipt.source_signature_identity(),
        source_signature_identity
    );
    assert_eq!(
        receipt.source_machine_name(),
        "ProgramLocalProducer::handoff"
    );
    assert_eq!(receipt.terminal_entry(), decoded.entry);
    assert_eq!(
        receipt.terminal_psi_identity(),
        produced.artifact().manifest().semantic()
    );

    let entry = decoded
        .machines
        .iter()
        .find(|machine| machine.id == receipt.terminal_entry())
        .expect("receipt names one retained Terminal entry");
    let [image, initial_storage] = entry.structural_parameters.as_slice() else {
        panic!("ProgramStorage handoff must retain two structural inputs")
    };
    assert_eq!((image.position, initial_storage.position), (0, 1));
    assert!(!image.is_self && !initial_storage.is_self);
    assert_ne!(image.place, initial_storage.place);
    assert_eq!(image.structural_type, initial_storage.structural_type);
    assert_eq!(image.multiplicity, StructuralMultiplicity::Linear);
    assert_eq!(initial_storage.multiplicity, StructuralMultiplicity::Linear);
    assert_eq!(image.access, StructuralAccess::Owned);
    assert_eq!(initial_storage.access, StructuralAccess::Owned);
    let [image_domain] = image.qualifications.as_slice() else {
        panic!("Image must retain exactly one qualification")
    };
    let [storage_domain] = initial_storage.qualifications.as_slice() else {
        panic!("InitialStorage must retain exactly one qualification")
    };
    assert_eq!(image_domain, storage_domain);
    let domain = decoded
        .structural_domains
        .iter()
        .find(|domain| domain.id == *image_domain)
        .expect("Granted domain declaration remains in the canonical artifact");
    assert_eq!(domain.identity, "Extent::Granted");
    assert_eq!(domain.carrier, image.structural_type);
    let carrier = decoded
        .structural_types
        .iter()
        .find(|declaration| declaration.id == image.structural_type)
        .expect("Extent carrier declaration remains in the canonical artifact");
    assert_eq!(carrier.identity, "named(name(Extent))");
    let StructuralTypeShape::Record { fields } = &carrier.shape else {
        panic!("Extent carrier must remain a record")
    };
    assert!(matches!(fields.as_slice(), [base, length]
        if base.identity == "base"
            && base.relevance == BindingRelevance::Relevant
            && matches!(base.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.is_address())
            && length.identity == "length"
            && length.relevance == BindingRelevance::Relevant
            && matches!(length.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.sign() == psi_core::IntegerSign::Unsigned && integer.bits() == 64)));

    assert!(
        matches!(entry.structural_places.as_slice(), [image_place, storage_place]
        if image_place.id == image.place
            && image_place.kind == StructuralPlaceKind::Parameter { position: 0, is_self: false }
            && storage_place.id == initial_storage.place
            && storage_place.kind == StructuralPlaceKind::Parameter { position: 1, is_self: false })
    );
    let [image_claim, storage_claim] = entry.entry_claims.as_slice() else {
        panic!("ProgramStorage handoff must retain two entry claims")
    };
    assert_eq!(image_claim.input, image.place);
    assert_eq!(storage_claim.input, initial_storage.place);
    assert!(image_claim.path.is_empty() && storage_claim.path.is_empty());
    let [block] = entry.blocks.as_slice() else {
        panic!("ProgramStorage handoff must remain straight-line")
    };
    let [call] = block.operations.as_slice() else {
        panic!("ProgramStorage handoff must retain one Unit call")
    };
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        ..
    } = &call.kind
    else {
        panic!("ProgramStorage handoff operation must remain BoundaryCall")
    };
    let boundary = decoded
        .boundary_machines
        .iter()
        .find(|candidate| candidate.id == *boundary)
        .expect("generic ProgramStorage requirement remains a bodyless boundary");
    assert_eq!(boundary.structural_parameters.len(), 2);
    assert_eq!(boundary.structural_parameters[0].position, 0);
    assert_eq!(boundary.structural_parameters[1].position, 1);
    assert!(
        matches!(structural_arguments.as_slice(), [image_argument, storage_argument]
        if image_argument.place == image.place
            && image_argument.access == StructuralAccess::Owned
            && image_argument.path.is_empty()
            && storage_argument.place == initial_storage.place
            && storage_argument.access == StructuralAccess::Owned
            && storage_argument.path.is_empty())
    );
    assert!(
        matches!(completion_receipts.as_slice(), [image_receipt, storage_receipt]
        if image_receipt.claim == image_claim.claim
            && image_receipt.argument_index == 0
            && storage_receipt.claim == storage_claim.claim
            && storage_receipt.argument_index == 1)
    );
    assert!(matches!(
        block.terminator,
        Terminator::ReturnUnit {
            ref trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ));
    produced
        .artifact()
        .validate()
        .expect("two-root receipt-coupled artifact replays");
}

#[test]
fn program_entry_receipt_rejects_a_scalar_result_machine() {
    let checked = checked_source(
        r#"
            data Helper {}
            machine Helper::touch() {}
            data Token { value: u64; }
            machine Token::drop(&mut self) { Helper::touch(); }
            data Main {}
            machine Main::launch(token: Token) -> u64 { 7u64 }
        "#,
    );
    let error = produce_program_entry_terminal_artifact(&checked, "Main::launch", [0x11; 32])
        .expect_err("ProgramEntry receipt requires a Unit result");
    assert!(
        matches!(
            &error,
            TerminalArtifactProductionError::EntryReceipt(
                ProgramEntryTerminalReceiptError::NonUnitEntry
            )
        ),
        "unexpected receipt rejection: {error:?}"
    );
}

fn checked_write_line_literal() -> psi_checked_trees::CheckedTrees {
    let source = r#"
        boundary trait Console {
            machine write_line(text: &[u8])
            reaches Console;
        }

        data Root {}
        machine Root::enter()
        reaches Console
        {
            Console::write_line("\x80A");
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn lowers_exact_raw_bytes_into_borrowed_boundary_argument() {
    let checked = checked_write_line_literal();
    let lowered = lower_machine(&checked, "Root::enter").expect("lower write_line literal");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one source machine")
    };
    let literal_place = machine
        .structural_places
        .iter()
        .find_map(|place| {
            matches!(
                place.kind,
                StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: 0,
                    ..
                }
            )
            .then_some(place.id)
        })
        .expect("canonical byte-sequence literal place");
    let [establish, call] = machine.blocks[0].operations.as_slice() else {
        panic!("literal establishment then boundary call")
    };
    assert!(matches!(
        &establish.kind,
        OperationKind::EstablishByteSequenceLiteral { destination, bytes }
            if *destination == literal_place && bytes == &[0x80, b'A']
    ));
    assert!(matches!(
        &call.kind,
        OperationKind::BoundaryCall { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.place == literal_place && argument.path.is_empty())
    ));
    let literal_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
            )
        })
        .expect("borrowed-view declaration");
    assert!(machine.structural_places.iter().any(|place| matches!(
        place.kind,
        StructuralPlaceKind::ByteSequenceLiteral { structural_type, .. }
            if structural_type == literal_type.id
    )));
}

#[test]
fn mutable_to_write_only_access_crosses_source_codec_and_verification() {
    let source = r#"
        data Sink {}
        machine Sink::fill(destination: &write [u8]) {}

        data Root {}
        machine Root::enter(bytes: &mut [u8]) {
            Sink::fill(&write bytes);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Root::enter").expect("lower write-only forwarding");
    let module = &lowered.semantic_module;

    assert_eq!(
        module.machines[0].structural_parameters[0].access,
        StructuralAccess::MutableBorrow
    );
    assert_eq!(
        module.machines[1].structural_parameters[0].access,
        StructuralAccess::WriteOnlyBorrow
    );
    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("root emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow)
    ));

    let encoded = psi_terminal_codec::encode_module(module).expect("encode access-bearing module");
    let decoded =
        psi_terminal_codec::decode_module(&encoded).expect("decode access-bearing module");
    assert_eq!(&decoded, module);
    psi_terminal_verifier::validate_module(&decoded).expect("verify write-only attenuation");
}

#[test]
fn write_only_common_field_subloan_crosses_source_codec_and_verification() {
    let source = r#"
        data Leaf [copy] { value: u16; }
        data Inner [copy] { leaf: Leaf; sibling: u16; }
        data Outer [copy] { inner: Inner; other: Inner; }

        data Sink {}
        machine Sink::fill(destination: &write Leaf) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.inner.leaf);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Root::forward").expect("lower projected forwarding");
    let module = &lowered.semantic_module;

    assert_eq!(
        module.machines[0].structural_parameters[0].access,
        StructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(
        module.machines[1].structural_parameters[0].access,
        StructuralAccess::WriteOnlyBorrow
    );
    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("projected caller emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow
                    && argument.path.len() == 2
                    && argument.path.iter().all(|segment| matches!(
                        segment,
                        StructuralPathSegment::Field(_)
                    )))
    ));

    let encoded = psi_terminal_codec::encode_module(module).expect("encode projected module");
    let decoded = psi_terminal_codec::decode_module(&encoded).expect("decode projected module");
    assert_eq!(&decoded, module);
    psi_terminal_verifier::validate_module(&decoded).expect("verify projected write-only subloan");

    let mut path_drifted = decoded.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut path_drifted.machines[0].blocks[0].operations[0].kind
    else {
        panic!("projected caller call")
    };
    structural_arguments[0].path[1] = structural_arguments[0].path[0].clone();
    psi_terminal_verifier::validate_module(&path_drifted)
        .expect_err("a redirected common-field identity must reject");

    let mut target_type_drifted = decoded.clone();
    target_type_drifted.machines[1].structural_parameters[0].structural_type =
        target_type_drifted.machines[0].structural_parameters[0].structural_type;
    psi_terminal_verifier::validate_module(&target_type_drifted)
        .expect_err("the projected leaf must match the callee's exact structural type");

    let mut target_access_drifted = decoded.clone();
    target_access_drifted.machines[1].structural_parameters[0].access =
        StructuralAccess::MutableBorrow;
    psi_terminal_verifier::validate_module(&target_access_drifted)
        .expect_err("the projected leaf must match the callee's exact access");

    let mut access_drifted = decoded;
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut access_drifted.machines[0].blocks[0].operations[0].kind
    else {
        panic!("projected caller call")
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    psi_terminal_verifier::validate_module(&access_drifted)
        .expect_err("a projected write-only argument cannot widen to shared access");
}

#[test]
fn rejects_tampered_owned_carrier_for_source_literal() {
    let mut checked = checked_write_line_literal();
    let literal_type = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter_mut()
        .find(|plan| {
            matches!(
                plan.shape,
                psi_checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(_)
            )
        })
        .expect("literal type");
    literal_type.shape = psi_checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
        psi_checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: 2 },
    );
    let error = lower_machine(&checked, "Root::enter")
        .expect_err("an owned carrier must not establish a borrowed source literal");
    assert!(
        error.to_string().contains("requires a borrowed-view type"),
        "{error}"
    );
}

#[test]
fn bounded_installation_reach_lowers_source_free_terminal_dependency() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        boundary trait InterruptCompletion {
            machine complete()
            reaches <= MachineControl + PortIo;
        }

        machine pic_complete()
        satisfies InterruptCompletion::complete
        reaches PortIo
        { }

        machine invoke<machine Completion>()
        where machine Completion satisfies InterruptCompletion::complete;
        { Completion(); }

    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke")
        .expect("generic invocation machine");
    let service_ids = ["MachineControl", "PortIo"]
        .iter()
        .enumerate()
        .map(|(index, name)| {
            (
                checked
                    .facts
                    .service_reaches
                    .services
                    .id_for_name(name)
                    .expect("service exists"),
                service_id(u64::try_from(index).expect("service index") + 1),
            )
        })
        .collect::<Vec<_>>();
    let closure = lower_root_service_reach(&checked, root.symbol, &service_ids)
        .expect("lower root service reach");
    assert!(closure.concrete.is_empty());
    let [dependency] = closure.installation_dependencies.as_slice() else {
        panic!("terminal root must retain one installation reach dependency");
    };
    assert!(
        dependency
            .requirement_identity
            .contains("InterruptCompletion::complete")
    );
    let bound_names = dependency
        .upper_bound
        .iter()
        .map(|id| {
            service_ids
                .iter()
                .find(|(_, terminal)| terminal == id)
                .and_then(|(source, _)| checked.facts.service_reaches.services.definition(*source))
                .expect("bound service is declared")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(bound_names, ["MachineControl", "PortIo"]);
}

#[test]
fn top_level_bounded_reach_lowers_normalized_machine_identity() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        boundary machine InterruptAcknowledgement::complete()
        reaches <= MachineControl + PortIo;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let requirement = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("top-level completion requirement");
    let expected_identity = typed
        .normalized_machine_overload_identity(requirement)
        .expect("normalized top-level requirement")
        .identity();
    let requirement_symbol = requirement.symbol;
    let checked = lower_typed_trees(typed).expect("check");
    let service_ids = ["MachineControl", "PortIo"]
        .iter()
        .enumerate()
        .map(|(index, name)| {
            (
                checked
                    .facts
                    .service_reaches
                    .services
                    .id_for_name(name)
                    .expect("service exists"),
                service_id(u64::try_from(index).expect("service index") + 1),
            )
        })
        .collect::<Vec<_>>();
    let closure = lower_root_service_reach(&checked, requirement_symbol, &service_ids)
        .expect("lower top-level requirement reach");
    assert!(closure.concrete.is_empty());
    let [dependency] = closure.installation_dependencies.as_slice() else {
        panic!("top-level requirement must retain one installation dependency");
    };
    assert_eq!(dependency.requirement_identity, expected_identity);
    let bound_names = dependency
        .upper_bound
        .iter()
        .map(|id| {
            service_ids
                .iter()
                .find(|(_, terminal)| terminal == id)
                .and_then(|(source, _)| checked.facts.service_reaches.services.definition(*source))
                .expect("bound service is declared")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(bound_names, ["MachineControl", "PortIo"]);
}

#[test]
fn actual_float_meaning_calls_emit_source_free_module_rows() {
    let source = r#"
        data FloatMeaning { }
        operator Float::meaning32(value: f32) -> FloatMeaning;
        operator Float::meaning64(value: f64) -> FloatMeaning;

        machine prove_projection(value32: f32, value64: f64)
        requires
            Float::meaning32(value32) == Float::meaning32(value32);
            Float::meaning64(value64) == Float::meaning64(value64);
        { }

        machine terminal_root(value: bool) -> bool
        requires
            true == true;
        ensures
            true == true;
        { value }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "terminal_root").expect("lower");
    let projections = &lowered.semantic_module.float_meaning_projections;
    assert_eq!(projections.len(), 4);
    assert_eq!(projections[0].result.id, psi_terminal::ProofValueId(0));
    assert_eq!(
        projections[0].operation,
        psi_terminal::FloatMeaningProjectionOperation::Meaning32
    );
    assert_eq!(
        projections[2].operation,
        psi_terminal::FloatMeaningProjectionOperation::Meaning64
    );
    assert_eq!(
        lowered.semantic_module.float_meaning_equalities,
        vec![
            psi_terminal::FloatMeaningEqualityProposition {
                id: psi_terminal::ProofPropositionId(0),
                left: psi_terminal::ProofValueId(0),
                right: psi_terminal::ProofValueId(1),
            },
            psi_terminal::FloatMeaningEqualityProposition {
                id: psi_terminal::ProofPropositionId(1),
                left: psi_terminal::ProofValueId(2),
                right: psi_terminal::ProofValueId(3),
            },
        ]
    );
    psi_terminal_verifier::validate_module(&lowered.semantic_module).expect("verify");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );
}

#[test]
fn integer_operation_obligations_follow_the_shared_policy_catalog() {
    let operation = operation_id(10);
    let obligation_kinds = [
        LoweredIntegerBinaryKind::ExactShiftLeft,
        LoweredIntegerBinaryKind::ExactShiftRight,
        LoweredIntegerBinaryKind::ExactAdd,
        LoweredIntegerBinaryKind::ExactSubtract,
        LoweredIntegerBinaryKind::ExactMultiply,
        LoweredIntegerBinaryKind::ExactDivide,
        LoweredIntegerBinaryKind::ExactRemainder,
        LoweredIntegerBinaryKind::WrappingDivide,
        LoweredIntegerBinaryKind::WrappingRemainder,
        LoweredIntegerBinaryKind::SaturatingDivide,
        LoweredIntegerBinaryKind::SaturatingRemainder,
    ];
    for kind in obligation_kinds {
        assert!(kind.formation_obligation(operation).is_some(), "{kind:?}");
    }
    for kind in [
        LoweredIntegerBinaryKind::BitwiseAnd,
        LoweredIntegerBinaryKind::BitwiseOr,
        LoweredIntegerBinaryKind::BitwiseXor,
        LoweredIntegerBinaryKind::WrappingShiftLeft,
        LoweredIntegerBinaryKind::WrappingShiftRight,
        LoweredIntegerBinaryKind::WrappingAdd,
        LoweredIntegerBinaryKind::SaturatingAdd,
        LoweredIntegerBinaryKind::WrappingSubtract,
        LoweredIntegerBinaryKind::SaturatingSubtract,
        LoweredIntegerBinaryKind::WrappingMultiply,
        LoweredIntegerBinaryKind::SaturatingMultiply,
    ] {
        assert!(kind.formation_obligation(operation).is_none(), "{kind:?}");
    }
    assert_eq!(
        LoweredIntegerBinaryKind::ExactSubtract.integer_policy_binding(),
        Some((IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Exact,)),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::SaturatingDivide.integer_policy_binding(),
        Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Saturating,)),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::ExactRemainder.integer_policy_binding(),
        Some((IntegerPolicyPrimitive::Remainder, ArithmeticDomain::Exact,)),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::WrappingRemainder.integer_policy_binding(),
        Some((
            IntegerPolicyPrimitive::Remainder,
            ArithmeticDomain::Wrapping,
        )),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::SaturatingRemainder.integer_policy_binding(),
        Some((
            IntegerPolicyPrimitive::Remainder,
            ArithmeticDomain::Saturating,
        )),
    );
}

#[test]
fn shared_boolean_comparison_normalization_rejects_two_runtime_sides() {
    let comparison = LoweredBooleanReturnExpression::Equal {
        left: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
        right: Box::new(LoweredBooleanReturnExpression::Parameter { position: 1 }),
    };
    assert!(normalize_shared_boolean_comparison_leaves(&comparison).is_none());

    let local_comparison = LoweredBooleanReturnExpression::Equal {
        left: Box::new(LoweredBooleanReturnExpression::Local { position: 1 }),
        right: Box::new(LoweredBooleanReturnExpression::Constant { value: false }),
    };
    assert!(normalize_shared_boolean_comparison_leaves(&local_comparison).is_none());
}

#[test]
fn generic_conformance_application_crosses_terminal_scalar_closure() {
    let source = r#"
        trait Ranked<'rank, Context> {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}
        data Borrow<'scope, Element> { value: &'scope Element }

        FieldOrder<'scope, Element>:
            Element satisfies Ranked<'scope, Borrow<'scope, Element>>
        {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<
            'call,
            Element,
            Order: Element satisfies Ranked<Borrow<'call, Element>>
        >(
            left: &'call Element,
            right: &'call Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller<'view>(left: &'view Card, right: &'view Card) -> bool {
            choose<Card, FieldOrder<Card>>(left, right)
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let owner = checked
        .machine_specializations
        .iter()
        .find(|specialization| !specialization.conformance_applications.is_empty())
        .expect("conformance specialization")
        .instance;

    let terminal_source = r#"
        machine terminal_root(value: bool) -> bool
        requires true == true
        ensures true == true
        { value }
    "#;
    let tokens = Lexer::new(terminal_source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let terminal_checked = lower_typed_trees(typed).expect("check");
    let mut lowered = lower_machine(&terminal_checked, "terminal_root").expect("lower terminal");
    lower_closed_conformance_applications(&checked, &[owner], &mut lowered.semantic_module)
        .expect("lower closed application");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("verify closed application");
    let [application] = lowered
        .semantic_module
        .closed_conformance_applications
        .as_slice()
    else {
        panic!("one closed application should cross terminal lowering")
    };
    assert!(application.telescope.iter().any(|binding| {
        binding.kind == psi_terminal::ClosedConformanceParameterKind::Type
            && binding.parameter == "Element"
            && binding.argument == "Card"
    }));
    assert_eq!(application.subject_identity.as_deref(), Some("Card"));
    assert_eq!(application.trait_lifetime_arguments, ["view"]);
    assert_eq!(application.trait_arguments, ["Borrow<'view,Card>"]);
    assert_eq!(application.rows.len(), 1);
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .any(|machine| machine.id == application.owner)
    );
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode closed application");
    let decoded = psi_terminal_codec::decode_module(&bytes).expect("decode closed application");
    assert_eq!(decoded, lowered.semantic_module);

    let mut redirected_lifetime = decoded.clone();
    redirected_lifetime.closed_conformance_applications[0].trait_lifetime_arguments[0]
        .push_str("::redirected");
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected_lifetime),
        Err(psi_terminal_verifier::ModuleError::ClosedConformanceFingerprintMismatch { .. })
    ));

    let mut redirected = decoded;
    redirected.closed_conformance_applications[0].rows[0]
        .realization_identity
        .push_str("::redirected");
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::ClosedConformanceFingerprintMismatch { .. })
    ));
}

#[test]
fn scalar_crash_disjunction_lowers_to_canonical_terminal_propositions() {
    let values = vec![
        ValueDeclaration {
            id: value_id(2),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            id: value_id(1),
            scalar_type: ScalarType::Boolean,
        },
    ];
    let proposition = checked_boolean_proposition(
        &CheckedBooleanExpression::Or {
            left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
            right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
        },
        &values,
    )
    .expect("scalar disjunction lowers");
    let Proposition::Disjunction(disjuncts) = &proposition else {
        panic!("scalar disjunction retains proposition structure")
    };
    assert_eq!(disjuncts.len(), 2);
    let keys = disjuncts
        .iter()
        .map(|disjunct| psi_terminal_codec::canonical_proposition_order_key(disjunct).unwrap())
        .collect::<Vec<_>>();
    assert!(keys[0] < keys[1]);
    PropositionContext::from_value_types(values.iter().map(|value| (value.id, value.scalar_type)))
        .unwrap()
        .validate(&proposition)
        .expect("scalar disjunction is well typed");
}

#[test]
fn payloadless_sum_equality_lowers_to_case_membership_equivalence() {
    let source = r#"
        data Mode {
            case Off;
            case On;
        }

        data Root {}
        machine Root::enter(left: Mode, right: Mode)
        crashes Abort
            left == right
        {}

        machine Root::different(left: Mode, right: Mode)
        crashes Abort
            left != right
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Root::enter").expect("lower terminal");
    let cases = lowered
        .semantic_module
        .structural_types
        .iter()
        .find_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Sum { cases } => Some(cases),
            _ => None,
        })
        .expect("payload-less sum retains a sum shape");
    assert_eq!(
        cases
            .iter()
            .map(|case| case.identity.as_str())
            .collect::<Vec<_>>(),
        ["Off", "On"]
    );
    let [bucket] = lowered.semantic_module.machines[0]
        .contract
        .crash_routes
        .as_slice()
    else {
        panic!("one crash bucket")
    };
    let [psi_terminal::CrashRouteGuard::Predicate(predicate)] = bucket.alternatives.as_slice()
    else {
        panic!("one predicate")
    };
    let Proposition::Conjunction(equivalences) = predicate.proposition() else {
        panic!("sum equality is one canonical conjunction")
    };
    assert_eq!(equivalences.len(), 4);
    assert!(equivalences.iter().all(|equivalence| matches!(
        equivalence,
        Proposition::Implication { premise, conclusion }
            if matches!(premise.as_ref(), Proposition::StructuralCaseMembership { .. })
                && matches!(conclusion.as_ref(), Proposition::StructuralCaseMembership { .. })
    )));
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("case-membership equality validates");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("case-membership module encodes");
    assert_eq!(&bytes[8..10], &35_u16.to_le_bytes());
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module.clone())
    );
    let different = lower_machine(&checked, "Root::different").expect("lower inequality");
    let [psi_terminal::CrashRouteGuard::Predicate(predicate)] =
        different.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        panic!("one inequality predicate")
    };
    assert!(matches!(
        predicate.proposition(),
        Proposition::Implication { premise, conclusion }
            if matches!(premise.as_ref(), Proposition::Conjunction(_))
                && matches!(conclusion.as_ref(), Proposition::Falsehood)
    ));
}

#[test]
fn payload_bearing_sum_equality_uses_exact_case_payload_paths() {
    let source = r#"
        trait Equatable {
            machine equals(&self, rhs: &Self) -> bool;
        }

        data Message {
            case Empty;
            case Data(value: i32);
        }
        MessageEquatable: Message satisfies Equatable;

        data Root {}
        machine Root::enter(left: Message, right: Message)
        crashes Abort
            left == right
        {}

        machine Root::different(left: Message, right: Message)
        crashes Abort
            left != right
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("payload-bearing equality has exact case-payload paths");
    let cases = lowered
        .semantic_module
        .structural_types
        .iter()
        .find_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Sum { cases } => Some(cases),
            _ => None,
        })
        .expect("payload-bearing sum shape");
    assert_eq!(cases.len(), 2);
    assert!(cases[0].fields.is_empty());
    assert_eq!(cases[1].fields.len(), 1);
    let [psi_terminal::CrashRouteGuard::Predicate(predicate)] =
        lowered.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        panic!("one equality predicate")
    };
    let Proposition::Disjunction(arms) = predicate.proposition() else {
        panic!("payload-bearing equality is a per-case disjunction")
    };
    assert_eq!(arms.len(), 2);
    assert!(format!("{arms:?}").contains("Case(StructuralCaseId(2))"));
    assert!(format!("{arms:?}").contains("Field(StructuralFieldId(1))"));
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("exact case-payload paths validate");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("payload-bearing sum module encodes");
    assert_eq!(&bytes[8..10], &35_u16.to_le_bytes());
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module.clone())
    );
    let mut redirected = lowered.semantic_module.clone();
    let payload_field = redirected
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Sum { cases } => {
                cases.iter_mut().find_map(|case| case.fields.first_mut())
            }
            _ => None,
        })
        .expect("payload field");
    payload_field.id = psi_core::StructuralFieldId::new(99).expect("redirected field");
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::InvalidIntegerFieldTerm { .. })
    ));

    let different = lower_machine(&checked, "Root::different").expect("lower inequality");
    let [psi_terminal::CrashRouteGuard::Predicate(predicate)] =
        different.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        panic!("one inequality predicate")
    };
    assert!(matches!(
        predicate.proposition(),
        Proposition::Implication { premise, conclusion }
            if matches!(premise.as_ref(), Proposition::Disjunction(_))
                && matches!(conclusion.as_ref(), Proposition::Falsehood)
    ));
}

fn unit_claim_at(
    machine: SymbolHandle,
    state: SymbolHandle,
    ordinal: u32,
) -> PermissionClaimIdentity {
    PermissionClaimIdentity::Established {
        machine_symbol: machine,
        state_symbol: state,
        source: PermissionEventSource::StateEntry,
        ordinal,
    }
}

fn unit_claim(machine: SymbolHandle, state: SymbolHandle) -> PermissionClaimIdentity {
    unit_claim_at(machine, state, 0)
}

fn hard_root_checked_fixture() -> CheckedTrees {
    let root = SymbolHandle::from_arena_index(1);
    let helper = SymbolHandle::from_arena_index(2);
    let boundary = SymbolHandle::from_arena_index(3);
    let root_state = SymbolHandle::from_arena_index(11);
    let helper_state = SymbolHandle::from_arena_index(12);
    let boundary_state = SymbolHandle::from_arena_index(13);
    let port_service_symbol = SymbolHandle::from_arena_index(20);
    let domain = SemanticDomainId(9);

    let mut checked = CheckedTrees::default();
    let port_service = checked
        .facts
        .service_reaches
        .services
        .intern(port_service_symbol, "PortIo");
    let empty_reach = checked.facts.service_reaches.rows.intern(Vec::new());
    assert_eq!(
        empty_reach,
        psi_language_semantics::ServiceReachRowTable::EMPTY_ROW
    );
    let port_reach = checked
        .facts
        .service_reaches
        .rows
        .intern(vec![port_service]);
    let reach = ServiceReachSummary {
        direct: port_reach,
        transitive: port_reach,
    };
    let contract_reach = ServiceReachPlan {
        interface: ServiceReachInterface::PublishedCeiling(port_reach),
        checked_inferred: port_reach,
    };
    checked.facts.service_reaches.machines.append_to_span(
        &mut checked.facts.service_reaches.root_machines,
        psi_checked_trees::MachineServiceReachRows {
            machine: root,
            interface: ServiceReachInterface::PublishedCeiling(port_reach),
            published_ceiling: port_reach,
            inferred_direct: port_reach,
            inferred_transitive: port_reach,
            effective: port_reach,
            concrete_effective: port_reach,
            ..Default::default()
        },
    );
    checked.facts.flow.terminal_machines = psi_checked_trees::CheckedTerminalMachineSelections {
        machines: vec![
            CheckedTerminalMachineSelection {
                machine: root,
                name: "example::Root::enter".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
            CheckedTerminalMachineSelection {
                machine: helper,
                name: "example::Helper::run".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
            CheckedTerminalMachineSelection {
                machine: boundary,
                name: "example::Acknowledgement::settle".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
        ],
    };
    let structural_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
        position,
        is_self: false,
        type_identity: "example::Acknowledgement".to_owned(),
        multiplicity: Multiplicity::Linear,
        access: psi_checked_trees::CheckedStructuralAccess::Owned,
        qualifications: vec![domain],
    };
    let entry_claim = |machine, state| psi_checked_trees::CheckedUnitEntryClaimPlan {
        claim_identity: unit_claim(machine, state),
        parameter_index: 0,
        path: Vec::new(),
        carry: CarryPolicy::STRICT,
    };
    checked.facts.flow.terminal_unit_effects = psi_checked_trees::CheckedUnitEffectPlans {
        structural_types: vec![
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Acknowledgement".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record {
                    fields: vec![
                        psi_checked_trees::CheckedUnitStructuralFieldPlan {
                            identity: "sequence".to_owned(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64),
                        },
                        psi_checked_trees::CheckedUnitStructuralFieldPlan {
                            identity: "proof".to_owned(),
                            relevance: psi_terminal::BindingRelevance::Erased,
                            field_type: CheckedUnitStructuralFieldType::Erased {
                                type_identity: "named(name(example::Evidence))".to_owned(),
                            },
                        },
                    ],
                },
            },
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Helper".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
            },
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Root".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: vec![psi_checked_trees::CheckedUnitStructuralDomainPlan {
            domain,
            identity: "example::Acknowledgement::Pending".to_owned(),
            carrier_type_identity: "example::Acknowledgement".to_owned(),
        }],
        boundary_machines: vec![CheckedBoundaryMachinePlan {
            machine: boundary,
            state: boundary_state,
            contract_owner: boundary,
            attachment_type_identity: Some("example::Acknowledgement".to_owned()),
            structural_parameters: vec![psi_checked_trees::CheckedUnitStructuralParameterPlan {
                is_self: true,
                ..structural_parameter(0)
            }],
            scalar_parameters: Vec::new(),
            result_type: None,
            domain_requirements: vec![
                psi_checked_trees::CheckedUnitStructuralDomainRequirementPlan {
                    argument_index: 0,
                    domain,
                },
            ],
            contract_report_fingerprint: 0x303,
            contract_commitment: psi_checked_trees::MachineContractCommitment::from_digest(
                [0x03; 32],
            ),
            contract_service_reach: contract_reach,
            service_reach: reach,
        }],
        machines: vec![
            CheckedUnitEffectMachinePlan {
                machine: root,
                state: root_state,
                attachment_type_identity: "example::Root".to_owned(),
                structural_parameters: vec![structural_parameter(7)],
                provider_attachment_requirements: Vec::new(),
                trivial_affine_locals: Vec::new(),
                entry_claims: vec![entry_claim(root, root_state)],
                body_qualifications: vec![domain],
                contract_report_fingerprint: 0x101,
                contract_commitment: psi_checked_trees::MachineContractCommitment::from_digest(
                    [0x01; 32],
                ),
                contract_service_reach: contract_reach,
                service_reach: reach,
                operations: vec![
                    CheckedUnitEffectOperationPlan::CallUnit {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 0,
                            call_ordinal: 0,
                        },
                        target_machine: helper,
                        target_state: helper_state,
                        target_contract_report_fingerprint: 0x202,
                        service_reach: reach,
                        structural_arguments: vec![
                            psi_checked_trees::CheckedUnitStructuralArgumentPlan {
                                source_parameter_index: 0,
                                type_identity: "example::Acknowledgement".to_owned(),
                                access: psi_checked_trees::CheckedStructuralAccess::Owned,
                                path: Vec::new(),
                                byte_sequence_literal: None,
                            },
                        ],
                        claim_transfers: vec![psi_checked_trees::CheckedUnitClaimTransferPlan {
                            claim_identity: unit_claim(root, root_state),
                            argument_index: 0,
                        }],
                    },
                    CheckedUnitEffectOperationPlan::ReturnUnit {
                        statement_index: 1,
                        trivial_affine_local_discard_ordinals: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            },
            CheckedUnitEffectMachinePlan {
                machine: helper,
                state: helper_state,
                attachment_type_identity: "example::Helper".to_owned(),
                structural_parameters: vec![structural_parameter(3)],
                provider_attachment_requirements: Vec::new(),
                trivial_affine_locals: Vec::new(),
                entry_claims: vec![entry_claim(helper, helper_state)],
                body_qualifications: vec![domain],
                contract_report_fingerprint: 0x202,
                contract_commitment: psi_checked_trees::MachineContractCommitment::from_digest(
                    [0x02; 32],
                ),
                contract_service_reach: contract_reach,
                service_reach: reach,
                operations: vec![
                    CheckedUnitEffectOperationPlan::PortWrite {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 0,
                            call_ordinal: 0,
                        },
                        port: 0x3f8,
                        value: 0x5a,
                        service_reach: reach,
                    },
                    CheckedUnitEffectOperationPlan::BoundaryCall {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 1,
                            call_ordinal: 0,
                        },
                        target_machine: boundary,
                        target_state: boundary_state,
                        target_contract_report_fingerprint: 0x303,
                        service_reach: reach,
                        scalar_arguments: Vec::new(),
                        structural_arguments: vec![
                            psi_checked_trees::CheckedUnitStructuralArgumentPlan {
                                source_parameter_index: 0,
                                type_identity: "example::Acknowledgement".to_owned(),
                                access: psi_checked_trees::CheckedStructuralAccess::Owned,
                                path: Vec::new(),
                                byte_sequence_literal: None,
                            },
                        ],
                        completion_receipts: vec![
                            psi_checked_trees::CheckedUnitClaimTransferPlan {
                                claim_identity: unit_claim(helper, helper_state),
                                argument_index: 0,
                            },
                        ],
                    },
                    CheckedUnitEffectOperationPlan::ReturnUnit {
                        statement_index: 2,
                        trivial_affine_local_discard_ordinals: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            },
        ],
    };
    checked.facts.contract_plans.machines = vec![
        psi_checked_trees::MachineContractPlan {
            machine: root,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0x101,
            commitment: psi_checked_trees::MachineContractCommitment::from_digest([0x01; 32]),
        },
        psi_checked_trees::MachineContractPlan {
            machine: helper,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0x202,
            commitment: psi_checked_trees::MachineContractCommitment::from_digest([0x02; 32]),
        },
        psi_checked_trees::MachineContractPlan {
            machine: boundary,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0x303,
            commitment: psi_checked_trees::MachineContractCommitment::from_digest([0x03; 32]),
        },
    ];
    checked
}

fn source_projection(
    version: CheckedContentPlaceVersion,
    root: CheckedContentPlaceRoot,
    fields: &[(&str, u32)],
    semantic_domain: SemanticDomainId,
) -> CheckedContentConservationTerm {
    CheckedContentConservationTerm::Projection {
        domain: SymbolHandle::from_arena_index(70),
        semantic_domain,
        projection_machine: SymbolHandle::from_arena_index(71),
        projection_report_fingerprint: 0xfeed,
        subject: CheckedContentStructuralPlace {
            version,
            root,
            segments: fields
                .iter()
                .map(|(name, symbol)| {
                    CheckedContentPlaceSegment::Field(ContentFieldSegment {
                        symbol: SymbolHandle::from_arena_index(*symbol),
                        name: (*name).to_owned(),
                    })
                })
                .collect(),
        },
    }
}
