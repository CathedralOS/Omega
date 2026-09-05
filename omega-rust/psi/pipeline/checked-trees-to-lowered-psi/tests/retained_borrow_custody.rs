use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

const SOURCE: &str = r#"
data ByteUnit {}
data CountedQuantity<Unit> { magnitude: u64; }
trait Content<A> { machine project(subject: &Self) -> A; }

data Buffer [linear] {}
domain Buffer::Owned;
machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
satisfies Content<CountedQuantity<ByteUnit>>::project
{ CountedQuantity { magnitude: 1 } }

data PendingRead<'storage> [linear] {}
domain PendingRead::Retained
established by Reader::submit;
machine Retained::content(pending: &PendingRead) -> CountedQuantity<ByteUnit>
satisfies Content<CountedQuantity<ByteUnit>>::project
{ CountedQuantity { magnitude: 1 } }

boundary trait Reader {
    machine submit<'storage>(
        buffer: &'storage Buffer in Buffer::Owned
    ) -> PendingRead<'storage>
    ensures result in PendingRead::Retained;
}

data Main {}
machine Main::main(&mut self) {}
"#;

fn checked() -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

fn lowered() -> lowered_psi::LoweredPsi {
    checked_trees_to_lowered_psi::lower_machine(&checked(), "Main::main")
        .expect("retained shared-borrow custody should lower")
}

fn retained_mut(
    module: &mut terminal_psi::TerminalModule,
) -> &mut terminal_psi::RetainedBorrowCustody {
    module
        .boundary_machines
        .iter_mut()
        .flat_map(|boundary| &mut boundary.content_guarantees)
        .find_map(|guarantee| match guarantee {
            terminal_psi::BoundaryContentGuarantee::RetainedBorrow(custody) => Some(custody),
            terminal_psi::BoundaryContentGuarantee::Conservation(_) => None,
        })
        .expect("retained-borrow custody")
}

#[test]
fn checked_retained_shared_borrow_reaches_canonical_terminal_custody() {
    let lowered = lowered();
    let [boundary] = lowered.semantic_module.boundary_machines.as_slice() else {
        panic!("one declaration-only retained-borrow boundary")
    };
    let [terminal_psi::BoundaryContentGuarantee::RetainedBorrow(custody)] =
        boundary.content_guarantees.as_slice()
    else {
        panic!("one retained-borrow guarantee")
    };
    assert_eq!(boundary.identity, custody.callable_identity);
    assert!(boundary.scalar_parameters.is_empty());
    assert!(boundary.structural_parameters.is_empty());
    assert!(boundary.result.is_unit());
    assert_eq!(custody.access, terminal_psi::StructuralAccess::SharedBorrow);
    assert_eq!(custody.callable_lifetime_parameter_count, 1);
    assert_eq!(custody.callable_lifetime_parameter_ordinal, 0);
    assert_eq!(custody.result_lifetime_argument_count, 1);
    assert_eq!(custody.result_lifetime_argument_ordinal, 0);
    assert!(custody.result_lifetime_slot_is_erased);
    assert_eq!(
        custody.result_multiplicity,
        terminal_psi::StructuralMultiplicity::Linear
    );
    assert_eq!(
        custody.result_nominal_identity,
        custody.result_projection.carrier_identity
    );
    assert_eq!(
        custody.retained_semantic_domain,
        custody.result_projection.semantic_domain
    );
    assert_eq!(
        custody.source_projection.projection.algebra,
        custody.result_projection.projection.algebra
    );
    assert!(matches!(
        &custody.source,
        terminal_psi::RetainedBorrowPlace {
            version: semantic_vocabulary::ContentPlaceVersion::Entry,
            root: terminal_psi::RetainedBorrowPlaceRoot::Parameter {
                position: 0,
                is_self: false,
                ..
            },
            segments,
        } if segments.is_empty()
    ));
    assert!(matches!(
        &custody.result,
        terminal_psi::RetainedBorrowPlace {
            version: semantic_vocabulary::ContentPlaceVersion::Current,
            root: terminal_psi::RetainedBorrowPlaceRoot::Result,
            segments,
        } if segments.is_empty()
    ));

    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    let decoded = terminal_codec::decode_module(&bytes).expect("decode");
    assert_eq!(decoded, lowered.semantic_module);
    terminal_verifier::validate_module(&decoded).expect("independent verification");
}

#[test]
fn terminal_verifier_rejects_retained_borrow_fence_drift() {
    let baseline = lowered().semantic_module;
    let rejects = |mutate: fn(&mut terminal_psi::RetainedBorrowCustody)| {
        let mut module = baseline.clone();
        mutate(retained_mut(&mut module));
        assert!(matches!(
            terminal_verifier::validate_module(&module),
            Err(terminal_verifier::ModuleError::InvalidBoundaryContentGuarantee(_))
        ));
    };

    rejects(|custody| custody.access = terminal_psi::StructuralAccess::MutableBorrow);
    rejects(|custody| custody.callable_lifetime_parameter_ordinal = 1);
    rejects(|custody| custody.result_lifetime_argument_count = 2);
    rejects(|custody| custody.result_lifetime_argument_ordinal = 1);
    rejects(|custody| custody.result_lifetime_slot_is_erased = false);
    rejects(|custody| custody.result_multiplicity = terminal_psi::StructuralMultiplicity::Affine);
    rejects(|custody| custody.result_nominal_identity.push_str("-drift"));
    rejects(|custody| {
        custody
            .source
            .segments
            .push(semantic_vocabulary::ContentPlaceSegment::FixedIndex(0))
    });
    rejects(|custody| {
        custody.source.root = terminal_psi::RetainedBorrowPlaceRoot::Parameter {
            position: 0,
            identity: "self".to_owned(),
            is_self: true,
        };
    });
    rejects(|custody| custody.source_projection.carrier_identity.clear());
    rejects(|custody| {
        custody
            .result_projection
            .projection
            .identity
            .projection_report_fingerprint += 1
    });
}

#[test]
fn lowering_replays_checked_source_and_projection_identity() {
    let mut checked = checked();
    checked
        .facts
        .qualifications
        .content
        .retained_borrow_custodies[0]
        .source_projection
        .carrier_identity
        .push_str("-drift");
    assert!(matches!(
        checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main"),
        Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(_))
    ));
}
