use omega_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_calling_conventions::ValueShape;
use omega_image_emission::{
    build_installation_record, build_object_artifact, decode_installation_record,
    derive_stack_demand, emit_executable_image, encode_installation_record,
    validate_installation_record, InstallationError,
};
use omega_machine_emission::emit_machine_code;
use omega_optimization_unit::reconstruct_psi_optimization_unit_seed;
use omega_optimization_validation::validate_psi_optimization_unit;
use omega_psi_to_abstract_operations::lower_artifact_sections;
use omega_target::NativeTarget;
use omega_target_operations::CallSiteOwner;
use omega_target_operations::TargetOperation;
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_checked_trees_to_terminal::lower_machine;
use psi_core::{ClaimId, OperationId, PlaceId, ProfileDecisionId, StructuralPlaceKind};
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{
    StructuralFieldType, StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction, Terminator,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_fuel::TerminalFuelSchedule;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    boundary trait PortIo {}
    data Receipt [linear] { value: u64; }

    boundary machine Receipt::settle(self)
    reaches PortIo
    ensures true;

    data Helper {}
    machine Helper::run(receipt: Receipt)
    reaches PortIo
    {
        Receipt::settle(receipt);
    }

    data Root {}
    machine Root::enter(receipts: [Receipt; 2])
    reaches PortIo
    {
        Helper::run(receipts[0]);
        Helper::run(receipts[1]);
    }
"#;

const PARTIAL_AFFINE_SOURCE: &str = r#"
    data LeftToken { value: u32; }
    data RightToken { value: u64; }
    data Pair { left: LeftToken; right: RightToken; }
    data Helper {}
    machine Helper::take(token: RightToken) {}
    data Root {}
    machine Root::enter(pair: Pair) {
        Helper::take(pair.right);
    }
"#;

const PARTIAL_AFFINE_PAIR_SOURCE: &str = r#"
    data Token { value: u64; }
    data Helper {}
    machine Helper::take(token: Token) {}
    data Root {}
    machine Root::enter(values: [Token; 2]) {
        Helper::take(values[1]);
    }
"#;

const FULLY_CONSUMED_AFFINE_PAIR_SOURCE: &str = r#"
    data Token { value: u64; }
    data Helper {}
    machine Helper::take(token: Token) {}
    data Root {}
    machine Root::enter(values: [Token; 2]) {
        Helper::take(values[1]);
        Helper::take(values[0]);
    }
"#;

const FORWARD_FULLY_CONSUMED_AFFINE_PAIR_SOURCE: &str = r#"
    data Token { value: u64; }
    data Helper {}
    machine Helper::take(token: Token) {}
    data Root {}
    machine Root::enter(values: [Token; 2]) {
        Helper::take(values[0]);
        Helper::take(values[1]);
    }
"#;

const PARTIAL_AFFINE_TRIPLE_SOURCE: &str = r#"
    data Token { value: u64; }
    data Helper {}
    machine Helper::take(token: Token) {}
    data Root {}
    machine Root::enter(values: [Token; 3]) {
        Helper::take(values[2]);
        Helper::take(values[0]);
    }
"#;

const PARTIAL_AFFINE_QUARTET_SOURCE: &str = r#"
    data Token { value: u64; }
    data Helper {}
    machine Helper::take(token: Token) {}
    data Root {}
    machine Root::enter(values: [Token; 4]) {
        Helper::take(values[1]);
        Helper::take(values[3]);
    }
"#;

const NESTED_AFFINE_ARRAY_SOURCE: &str = r#"
    data Token { value: u64; }
    data Helper {}
    machine Helper::take(token: Token) {}
    data Root {}
    machine Root::enter(values: [[Token; 3]; 2]) {
        Helper::take(values[1][0]);
        Helper::take(values[0][1]);
    }
"#;

const MIXED_SCALAR_PARTIAL_AFFINE_SOURCE: &str = r#"
    domain [u8; 3]::Utf8
    requires
        valid_utf8(self);
    domain [u8; 8]::Utf8
    requires
        valid_utf8(self);
    data LeftToken { value: u32; }
    data RightToken { value: u64; }
    data Mixed {
        before: u8;
        before_bytes: [u8; 3] in Utf8;
        before_float: f32;
        left: LeftToken;
        between: bool;
        between_bytes: [u8; 8] in Utf8;
        between_float: f64;
        right: RightToken;
        after: u16;
    }
    data Helper {}
    machine Helper::take(token: RightToken) {}
    data Root {}
    machine Root::enter(mixed: Mixed) {
        Helper::take(mixed.right);
    }
"#;

const WIDE_PARTIAL_AFFINE_SOURCE: &str = r#"
    data LeftToken { value: u32; }
    data MiddleToken { value: u64; }
    data RightToken { value: u64; }
    data Triple { left: LeftToken; middle: MiddleToken; right: RightToken; }
    data Helper {}
    machine Helper::take(token: RightToken) {}
    data Root {}
    machine Root::enter(triple: Triple) {
        Helper::take(triple.right);
    }
"#;

const MULTIPLE_MOVE_PARTIAL_AFFINE_SOURCE: &str = r#"
    data FirstToken { value: u32; }
    data SecondToken { value: u64; }
    data ThirdToken { value: u16; }
    data FourthToken { value: u64; }
    data Quartet {
        first: FirstToken;
        second: SecondToken;
        third: ThirdToken;
        fourth: FourthToken;
    }
    data FirstHelper {}
    machine FirstHelper::take(token: SecondToken) {}
    data SecondHelper {}
    machine SecondHelper::take(token: FourthToken) {}
    data Root {}
    machine Root::enter(quartet: Quartet) {
        FirstHelper::take(quartet.second);
        SecondHelper::take(quartet.fourth);
    }
"#;

const NESTED_PARTIAL_AFFINE_SOURCE: &str = r#"
    data FirstToken { value: u32; }
    data LeftToken { value: u16; }
    data MiddleToken { value: u64; }
    data RightToken { value: u32; }
    data LastToken { value: u64; }
    data Inner { left: LeftToken; middle: MiddleToken; right: RightToken; }
    data Outer { first: FirstToken; inner: Inner; last: LastToken; }
    data Helper {}
    machine Helper::take(token: MiddleToken) {}
    data Root {}
    machine Root::enter(outer: Outer) {
        Helper::take(outer.inner.middle);
    }
"#;

const MIXED_DEPTH_PARTIAL_AFFINE_SOURCE: &str = r#"
    data Token { value: u64; }
    data Deep { x: Token; y: Token; z: Token; }
    data Inner { a: Token; deep: Deep; c: Token; }
    data Outer { pre: Token; direct: Token; inner: Inner; post: Token; }
    data DirectHelper {}
    machine DirectHelper::take(token: Token) {}
    data InnerHelper {}
    machine InnerHelper::take(token: Token) {}
    data DeepHelper {}
    machine DeepHelper::take(token: Token) {}
    data Root {}
    machine Root::enter(outer: Outer) {
        DirectHelper::take(outer.direct);
        InnerHelper::take(outer.inner.a);
        DeepHelper::take(outer.inner.deep.y);
    }
"#;

const NOMINAL_AFFINE_SOURCE: &str = r#"
    data Token { first: u64; second: u64; third: u64; fourth: u64; fifth: u64; }
    data FirstCleanupHelper {}
    machine FirstCleanupHelper::run() {}
    data SecondCleanupHelper {}
    machine SecondCleanupHelper::run() {}
    data ThirdCleanupHelper {}
    machine ThirdCleanupHelper::run() {}
    machine Token::drop(&mut self) {
        FirstCleanupHelper::run();
        SecondCleanupHelper::run();
        ThirdCleanupHelper::run();
    }
    data Root {}
    machine Root::enter(token: Token) {}
"#;

const CONTEXTUAL_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Token { first: bool; second: bool; caller_only: bool; padding: u8; }
    machine Token::drop(&mut self)
    requires self.second, self.first
    {}

    data Root {}
    machine Root::enter(token: Token)
    requires token.caller_only, token.first, token.second
    {}
"#;

const ORDERED_CONTEXTUAL_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Shared { ready: bool; audited: bool; padding: u16; }
    machine Shared::drop(&mut self)
    requires self.ready
    { Helper::touch(); }

    data Distinct { ready: bool; padding: u8; }
    machine Distinct::drop(&mut self)
    requires self.ready == false
    { Helper::touch(); }

    data Root {}
    machine Root::enter(first: Shared, second: Distinct, third: Shared)
    requires third.ready, first.audited, second.ready == false, first.ready
    {}
"#;

const TWO_EMPTY_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Token {}
    machine Token::drop(&mut self) {}
    data Root {}
    machine Root::enter(first: Token, second: Token) {}
"#;

const FIRST_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data First { value: u32; }
    machine First::drop(&mut self) { Helper::touch(); }
    data Second { value: u64; }
    machine Second::drop(&mut self) {}
    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const SECOND_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data First { value: u32; }
    machine First::drop(&mut self) {}
    data Second { value: u64; }
    machine Second::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const TWO_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data First { value: u32; }
    machine First::drop(&mut self) { Helper::touch(); }
    data Second { value: u64; }
    machine Second::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token { value: u32; }
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(first: Token, second: Token) {}
"#;

const THREE_SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token { value: u32; }
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(first: Token, second: Token, third: Token) {}
"#;

const FIVE_SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data FirstHelper {}
    machine FirstHelper::touch() {}
    data SecondHelper {}
    machine SecondHelper::touch() {}
    data ThirdHelper {}
    machine ThirdHelper::touch() {}
    data FourthHelper {}
    machine FourthHelper::touch() {}
    data FifthHelper {}
    machine FifthHelper::touch() {}
    data Token { value: u32; }
    machine Token::drop(&mut self) {
        FirstHelper::touch();
        SecondHelper::touch();
        ThirdHelper::touch();
        FourthHelper::touch();
        FifthHelper::touch();
    }
    data Root {}
    machine Root::enter(
        first: Token,
        second: Token,
        third: Token,
        fourth: Token,
        fifth: Token
    ) {}
"#;

fn verified_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower terminal Psi");
    let semantics = encode_module(&terminal.semantic_module).expect("encode terminal semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode terminal proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("canonical terminal artifact verifies and lowers into Omega")
}

fn backend_projection_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let mut plan = verified_plan();
    // Provider settlement is covered by the admitted-effect suite and its
    // current native realization is deliberately x86-only. This test isolates
    // the portable internal-call carrier after the canonical Psi-to-Omega seam.
    for function in &mut plan.functions {
        function.operations.retain(|operation| {
            !matches!(
                operation,
                omega_abstract_operations::AbstractOperation::BoundaryCall { .. }
            )
        });
    }
    plan.boundary_machines.clear();
    plan
}

fn partial_affine_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(PARTIAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize partial affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse partial affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve partial affine source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type partial affine source");
    let checked = lower_typed_trees(typed).expect("check partial affine source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower partial affine Psi");
    let semantics = encode_module(&terminal.semantic_module).expect("encode partial affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode partial affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified partial affine artifact enters Omega")
}

fn partial_affine_pair_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(PARTIAL_AFFINE_PAIR_SOURCE)
        .tokenize()
        .expect("tokenize partial affine pair source");
    let syntax = parse_syntax_trees(&tokens).expect("parse partial affine pair source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve partial affine pair source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type partial affine pair source");
    let checked = lower_typed_trees(typed).expect("check partial affine pair source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower partial affine pair Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode partial affine pair Psi");
    let proof =
        encode_proof_bundle(&terminal.proof_bundle).expect("encode partial affine pair proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified partial affine pair artifact enters Omega")
}

fn fully_consumed_affine_pair_plan(
    source: &str,
) -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize fully consumed affine pair source");
    let syntax = parse_syntax_trees(&tokens).expect("parse fully consumed affine pair source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve fully consumed affine pair source");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type fully consumed affine pair source");
    let checked = lower_typed_trees(typed).expect("check fully consumed affine pair source");
    let terminal =
        lower_machine(&checked, "Root::enter").expect("lower fully consumed affine pair Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode fully consumed affine pair Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle)
        .expect("encode fully consumed affine pair proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified fully consumed affine pair artifact enters Omega")
}

fn partial_affine_triple_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(PARTIAL_AFFINE_TRIPLE_SOURCE)
        .tokenize()
        .expect("tokenize partial affine triple source");
    let syntax = parse_syntax_trees(&tokens).expect("parse partial affine triple source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve partial affine triple source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type partial affine triple source");
    let checked = lower_typed_trees(typed).expect("check partial affine triple source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower partial affine triple Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode partial affine triple Psi");
    let proof =
        encode_proof_bundle(&terminal.proof_bundle).expect("encode partial affine triple proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified partial affine triple artifact enters Omega")
}

fn partial_affine_quartet_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(PARTIAL_AFFINE_QUARTET_SOURCE)
        .tokenize()
        .expect("tokenize partial affine quartet source");
    let syntax = parse_syntax_trees(&tokens).expect("parse partial affine quartet source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve partial affine quartet source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type partial affine quartet source");
    let checked = lower_typed_trees(typed).expect("check partial affine quartet source");
    let terminal =
        lower_machine(&checked, "Root::enter").expect("lower partial affine quartet Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode partial affine quartet Psi");
    let proof =
        encode_proof_bundle(&terminal.proof_bundle).expect("encode partial affine quartet proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified partial affine quartet artifact enters Omega")
}

fn nested_affine_array_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(NESTED_AFFINE_ARRAY_SOURCE)
        .tokenize()
        .expect("tokenize nested affine array source");
    let syntax = parse_syntax_trees(&tokens).expect("parse nested affine array source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve nested affine array source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type nested affine array source");
    let checked = lower_typed_trees(typed).expect("check nested affine array source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower nested affine array Psi");
    let semantics = encode_module(&terminal.semantic_module).expect("encode nested affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode nested affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified nested affine artifact enters Omega")
}

fn mixed_scalar_partial_affine_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(MIXED_SCALAR_PARTIAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize mixed scalar partial affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse mixed scalar partial affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve mixed scalar partial affine source");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type mixed scalar partial affine source");
    let checked = lower_typed_trees(typed).expect("check mixed scalar partial affine source");
    let terminal =
        lower_machine(&checked, "Root::enter").expect("lower mixed scalar partial affine Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode mixed scalar partial affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle)
        .expect("encode mixed scalar partial affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified mixed scalar partial affine artifact enters Omega")
}

fn wide_partial_affine_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(WIDE_PARTIAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize wide partial affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse wide partial affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve wide partial affine source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type wide partial affine source");
    let checked = lower_typed_trees(typed).expect("check wide partial affine source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower wide partial affine Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode wide partial affine Psi");
    let proof =
        encode_proof_bundle(&terminal.proof_bundle).expect("encode wide partial affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified wide partial affine artifact enters Omega")
}

fn multiple_move_partial_affine_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(MULTIPLE_MOVE_PARTIAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize multiple-move partial affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse multiple-move partial affine source");
    let resolved =
        lower_syntax_trees(&syntax).expect("resolve multiple-move partial affine source");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type multiple-move partial affine source");
    let checked = lower_typed_trees(typed).expect("check multiple-move partial affine source");
    let terminal =
        lower_machine(&checked, "Root::enter").expect("lower multiple-move partial affine Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode multiple-move partial affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle)
        .expect("encode multiple-move partial affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified multiple-move partial affine artifact enters Omega")
}

fn nested_partial_affine_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(NESTED_PARTIAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize nested partial affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse nested partial affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve nested partial affine source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type nested partial affine source");
    let checked = lower_typed_trees(typed).expect("check nested partial affine source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower nested partial affine Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode nested partial affine Psi");
    let proof =
        encode_proof_bundle(&terminal.proof_bundle).expect("encode nested partial affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified nested partial affine artifact enters Omega")
}

fn mixed_depth_partial_affine_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(MIXED_DEPTH_PARTIAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize mixed-depth partial affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse mixed-depth partial affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve mixed-depth partial affine source");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type mixed-depth partial affine source");
    let checked = lower_typed_trees(typed).expect("check mixed-depth partial affine source");
    let terminal =
        lower_machine(&checked, "Root::enter").expect("lower mixed-depth partial affine Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode mixed-depth partial affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle)
        .expect("encode mixed-depth partial affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified mixed-depth partial affine artifact enters Omega")
}

fn nominal_affine_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(NOMINAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize nominal affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse nominal affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve nominal affine source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type nominal affine source");
    let checked = lower_typed_trees(typed).expect("check nominal affine source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower nominal affine Psi");
    let semantics = encode_module(&terminal.semantic_module).expect("encode nominal affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode nominal affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified nominal affine artifact enters Omega")
}

fn contextual_nominal_affine_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(CONTEXTUAL_NOMINAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize contextual nominal affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse contextual nominal affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve contextual nominal affine source");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type contextual nominal affine source");
    let checked = lower_typed_trees(typed).expect("check contextual nominal affine source");
    let terminal =
        lower_machine(&checked, "Root::enter").expect("lower contextual nominal affine Psi");
    let entry = terminal
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == terminal.semantic_module.entry)
        .expect("contextual cleanup entry");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("contextual cleanup must use the nominal return carrier")
    };
    let [cleanup] = cleanups.as_slice() else {
        panic!("one contextual cleanup action")
    };
    assert!(cleanup.cleanup_receiver.is_some());
    assert_eq!(cleanup.requirement_obligations.len(), 2);
    assert_eq!(terminal.proof_bundle.evidence.len(), 2);

    let semantics =
        encode_module(&terminal.semantic_module).expect("encode contextual nominal affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle)
        .expect("encode contextual nominal affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified contextual nominal artifact enters Omega")
}

fn ordered_contextual_nominal_affine_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(ORDERED_CONTEXTUAL_NOMINAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize ordered contextual nominal affine source");
    let syntax =
        parse_syntax_trees(&tokens).expect("parse ordered contextual nominal affine source");
    let resolved =
        lower_syntax_trees(&syntax).expect("resolve ordered contextual nominal affine source");
    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("type ordered contextual nominal affine source");
    let checked = lower_typed_trees(typed).expect("check ordered contextual nominal affine source");
    let terminal = lower_machine(&checked, "Root::enter")
        .expect("lower ordered contextual nominal affine Psi");
    let entry = terminal
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == terminal.semantic_module.entry)
        .expect("ordered contextual cleanup entry");
    let [first, second, third] = entry.structural_parameters.as_slice() else {
        panic!("ordered contextual caller has three roots")
    };
    assert_eq!(entry.contract.requires.len(), 4);
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered contextual cleanup uses the nominal return carrier")
    };
    let [third_cleanup, second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("ordered contextual cleanup has three actions")
    };
    assert_eq!(
        [
            third_cleanup.place,
            second_cleanup.place,
            first_cleanup.place
        ],
        [third.place, second.place, first.place]
    );
    assert_eq!(third_cleanup.cleanup_machine, first_cleanup.cleanup_machine);
    assert_ne!(
        third_cleanup.cleanup_machine,
        second_cleanup.cleanup_machine
    );
    assert_eq!(
        third_cleanup.cleanup_receiver,
        first_cleanup.cleanup_receiver
    );
    assert_ne!(
        third_cleanup.cleanup_receiver,
        second_cleanup.cleanup_receiver
    );
    assert!(cleanups
        .iter()
        .all(|cleanup| cleanup.cleanup_receiver.is_some()));
    assert!(cleanups
        .iter()
        .all(|cleanup| cleanup.requirement_obligations.len() == 1));
    assert_eq!(terminal.proof_bundle.evidence.len(), 3);
    let distinct_targets = [
        third_cleanup.cleanup_machine,
        second_cleanup.cleanup_machine,
    ];
    for target_id in distinct_targets {
        let target = terminal
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == target_id)
            .expect("ordered contextual cleanup target remains in the closure");
        assert_eq!(target.blocks[0].operations.len(), 1);
    }

    let semantics = encode_module(&terminal.semantic_module)
        .expect("encode ordered contextual nominal affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle)
        .expect("encode ordered contextual nominal affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified ordered contextual nominal artifact enters Omega")
}

fn two_empty_nominal_affine_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(TWO_EMPTY_NOMINAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize two nominal affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse two nominal affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve two nominal affine source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type two nominal affine source");
    let checked = lower_typed_trees(typed).expect("check two nominal affine source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower two nominal affine Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode two nominal affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode two nominal proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified two nominal artifact enters Omega")
}

fn two_nominal_one_executable_plan(
    source: &str,
) -> omega_abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize executable nominal-list source");
    let syntax = parse_syntax_trees(&tokens).expect("parse executable nominal-list source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve executable nominal-list source");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type executable nominal-list source");
    let checked = lower_typed_trees(typed).expect("check executable nominal-list source");
    let terminal =
        lower_machine(&checked, "Root::enter").expect("lower executable nominal-list Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode executable nominal-list semantics");
    let proof =
        encode_proof_bundle(&terminal.proof_bundle).expect("encode executable nominal-list proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified executable nominal-list artifact enters Omega")
}

#[test]
fn canonical_verified_artifact_delivers_projected_calls_to_omega() {
    let plan = verified_plan();
    let caller = &plan.functions[0];
    let calls = caller
        .operations
        .iter()
        .filter_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::CallUnit {
                structural_arguments,
                claim_transfers,
                ..
            } => Some((structural_arguments, claim_transfers)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    for (index, (arguments, transfers)) in calls.into_iter().enumerate() {
        assert_eq!(
            arguments[0].path,
            [StructuralPathSegment::FixedIndex(index as u64)]
        );
        assert_eq!(transfers[0].claim, ClaimId::new(index as u64 + 1).unwrap());
    }
}

#[test]
fn literal_element_calls_retain_native_and_installed_custody_on_all_targets() {
    let plan = backend_projection_plan();
    let caller_parameter = plan.functions[0].structural_parameters[0].structural_type;
    let root_declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == caller_parameter)
        .expect("caller array type remains declared");
    let StructuralTypeShape::FixedArray {
        element: element_type,
        length: 2,
    } = &root_declaration.shape
    else {
        panic!("caller parameter must remain a two-element fixed array")
    };
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let TargetOperation::UnitBody(caller) = &target_plan.functions[0].operation else {
            panic!("caller must remain Unit")
        };
        assert_eq!(caller.parameters[0].shape, ValueShape::integer(16, 8));
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let custody = &machine.functions[0].internal_unit_calls;
        assert_eq!(custody.len(), 2);
        assert_eq!(custody[0].arguments[0].source_byte_offset, 0);
        assert_eq!(custody[1].arguments[0].source_byte_offset, 8);
        for (index, call) in custody.iter().enumerate() {
            let argument = &call.arguments[0];
            assert_eq!(
                argument.path,
                [StructuralPathSegment::FixedIndex(index as u64)]
            );
            assert_eq!(argument.root_structural_type, caller_parameter);
            assert_eq!(argument.structural_type, *element_type);
            assert_eq!(argument.shape, ValueShape::integer(8, 8));
            assert_eq!(argument.source.shape, ValueShape::integer(16, 8));
            assert_eq!(argument.fixed_array_length, Some(2));
            assert_eq!(argument.element_stride, Some(8));
            assert!(argument.byte_count != 0);
            if target == NativeTarget::windows_x64() {
                assert!(matches!(
                    argument.source.locations.as_slice(),
                    [omega_calling_conventions::ValueLocation::Indirect { .. }]
                ));
            } else {
                assert!(argument.source.locations.iter().all(|location| !matches!(
                    location,
                    omega_calling_conventions::ValueLocation::Indirect { .. }
                )));
            }
            assert_eq!(
                call.claim_transfers[0].claim,
                ClaimId::new(index as u64 + 1).unwrap()
            );
        }

        let mut changed_offset = machine.clone();
        changed_offset.functions[0].internal_unit_calls[1].arguments[0].source_byte_offset = 0;
        assert!(build_object_artifact(&changed_offset).is_err());
        let mut changed_path = machine.clone();
        changed_path.functions[0].internal_unit_calls[1].arguments[0].path =
            vec![StructuralPathSegment::FixedIndex(0)];
        assert!(build_object_artifact(&changed_path).is_err());
        let mut dropped_claim = machine.clone();
        dropped_claim.functions[0].internal_unit_calls[1]
            .claim_transfers
            .clear();
        assert!(build_object_artifact(&dropped_claim).is_err());
        let mut duplicated_custody = machine.clone();
        duplicated_custody.functions[0].internal_unit_calls[1] =
            duplicated_custody.functions[0].internal_unit_calls[0].clone();
        assert!(build_object_artifact(&duplicated_custody).is_err());
        let mut changed_copy_byte = machine.clone();
        let copy_offset =
            changed_copy_byte.functions[0].internal_unit_calls[1].arguments[0].code_offset;
        changed_copy_byte.functions[0].bytes[copy_offset] ^= 1;
        assert!(build_object_artifact(&changed_copy_byte).is_err());
        let mut paired_copy_mutation = machine.clone();
        let copy_offset =
            paired_copy_mutation.functions[0].internal_unit_calls[1].arguments[0].code_offset;
        paired_copy_mutation.functions[0].internal_unit_calls[1].arguments[0].bytes[0] ^= 1;
        paired_copy_mutation.functions[0].bytes[copy_offset] ^= 1;
        assert!(build_object_artifact(&paired_copy_mutation).is_err());

        let first_copy_offset =
            machine.functions[0].internal_unit_calls[0].arguments[0].code_offset;
        let mut forged_home = machine.clone();
        forged_home.functions[0].internal_unit_calls[0].arguments[0].source_home_byte_offset = 8;
        forged_home.functions[0].internal_unit_calls[0].arguments[0].bytes[0] ^= 1;
        forged_home.functions[0].bytes[first_copy_offset] ^= 1;
        assert!(build_object_artifact(&forged_home).is_err());
        let mut forged_call_stack = machine.clone();
        forged_call_stack.functions[0].internal_unit_calls[0].arguments[0].call_stack_bytes += 8;
        forged_call_stack.functions[0].internal_unit_calls[0].arguments[0].bytes[0] ^= 1;
        forged_call_stack.functions[0].bytes[first_copy_offset] ^= 1;
        assert!(build_object_artifact(&forged_call_stack).is_err());

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        assert_eq!(installation.internal_unit_calls().len(), 2);
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        let installed_argument = installation.internal_unit_calls()[1].custody.arguments[0].clone();
        assert_eq!(decode_installation_record(&bytes), Ok(installation.clone()));
        let mut projection = Vec::new();
        projection.extend_from_slice(&installed_argument.place.get().to_le_bytes());
        projection.extend_from_slice(&1_u32.to_le_bytes());
        projection.extend_from_slice(&1_u32.to_le_bytes());
        projection.extend_from_slice(&[2, 0, 0, 0]);
        projection.extend_from_slice(&1_u64.to_le_bytes());
        projection.extend_from_slice(&installed_argument.root_structural_type.get().to_le_bytes());
        projection.extend_from_slice(&installed_argument.structural_type.get().to_le_bytes());
        projection.extend_from_slice(&[1, 0, 8, 0, 8, 0, 0, 0]);
        projection.extend_from_slice(&installed_argument.source_byte_offset.to_le_bytes());
        let offset = bytes
            .windows(projection.len())
            .position(|window| window == projection)
            .expect("format-36 bytes retain the second resolved projection");
        let mut changed_installation = bytes.clone();
        let source_offset = offset + 52;
        changed_installation[source_offset..source_offset + 4]
            .copy_from_slice(&0_u32.to_le_bytes());
        assert!(decode_installation_record(&changed_installation).is_err());
    }
}

#[test]
fn source_partial_and_nominal_affine_plans_reach_current_optimizer_ownership_replay() {
    for plan in [
        partial_affine_plan(),
        partial_affine_pair_plan(),
        fully_consumed_affine_pair_plan(FULLY_CONSUMED_AFFINE_PAIR_SOURCE),
        fully_consumed_affine_pair_plan(FORWARD_FULLY_CONSUMED_AFFINE_PAIR_SOURCE),
        partial_affine_triple_plan(),
        nested_affine_array_plan(),
        wide_partial_affine_plan(),
        multiple_move_partial_affine_plan(),
        nested_partial_affine_plan(),
        mixed_depth_partial_affine_plan(),
    ] {
        let unit =
            reconstruct_psi_optimization_unit_seed(&plan, TerminalFuelSchedule::CURRENT.identity())
                .expect("verified partial-affine plan reconstructs an optimization unit");
        validate_psi_optimization_unit(&unit)
            .expect("current ownership replay accepts verified partial-affine source custody");
    }

    for plan in [
        nominal_affine_plan(),
        contextual_nominal_affine_plan(),
        ordered_contextual_nominal_affine_plan(),
        two_empty_nominal_affine_plan(),
    ] {
        let unit =
            reconstruct_psi_optimization_unit_seed(&plan, TerminalFuelSchedule::CURRENT.identity())
                .expect("verified nominal-affine plan reconstructs an optimization unit");
        validate_psi_optimization_unit(&unit)
            .expect("current ownership replay accepts verified nominal-affine source custody");
    }
}

#[test]
fn partial_affine_field_cleanup_is_zero_code_and_installed_on_all_targets() {
    let plan = partial_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let pair_type = caller.structural_parameters[0].structural_type;
    let residual = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => match cleanup_actions.as_slice() {
                [TerminalAffineCleanupAction::DiscardResidual(residual)] => Some(residual.clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("partial return retains one residual cleanup");
    assert_eq!(residual.path, [StructuralPathSegment::Field("left".into())]);

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("caller machine code exists");
        let [call] = emitted.internal_unit_calls.as_slice() else {
            panic!("caller has one projected internal call")
        };
        let [argument] = call.arguments.as_slice() else {
            panic!("call has one argument")
        };
        assert_eq!(argument.root_structural_type, pair_type);
        assert_eq!(
            argument.path,
            [StructuralPathSegment::Field("right".into())]
        );
        assert_eq!(argument.source_byte_offset, 8);
        assert_ne!(argument.structural_type, residual.structural_type);

        let cleanup = emitted
            .unit_affine_cleanup
            .as_ref()
            .expect("caller retains cleanup ledger");
        assert_eq!(
            cleanup.actions,
            [TerminalAffineCleanupAction::DiscardResidual(
                residual.clone()
            )]
        );
        let mut root_cleanup_assigned = assigned.clone();
        let root_cleanup_caller = root_cleanup_assigned
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut root_cleanup_caller.operation
        else {
            panic!("caller remains a Unit body")
        };
        let omega_assigned_target_operations::AssignedUnitOperation::Return {
            cleanup_actions, ..
        } = body.operations.last_mut().unwrap()
        else {
            panic!("caller ends in a Unit return")
        };
        *cleanup_actions = vec![TerminalAffineCleanupAction::DiscardRoot(residual.place)];
        let root_cleanup_machine = emit_machine_code(&root_cleanup_assigned).unwrap();
        let root_cleanup_bytes = &root_cleanup_machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .bytes;
        assert_eq!(
            &emitted.bytes, root_cleanup_bytes,
            "path-sensitive cleanup adds no runtime instruction bytes"
        );

        let mut forged_path = machine.clone();
        forged_path
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions[0] =
            TerminalAffineCleanupAction::DiscardResidual(psi_terminal::StructuralAffineDiscard {
                path: vec![StructuralPathSegment::Field("right".into())],
                ..residual.clone()
            });
        assert!(build_object_artifact(&forged_path).is_err());
        let mut forged_type = machine.clone();
        forged_type
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions[0] =
            TerminalAffineCleanupAction::DiscardResidual(psi_terminal::StructuralAffineDiscard {
                structural_type: pair_type,
                ..residual.clone()
            });
        assert!(build_object_artifact(&forged_type).is_err());

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed_cleanup = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_ref()
            .unwrap();
        assert_eq!(
            installed_cleanup.actions,
            [TerminalAffineCleanupAction::DiscardResidual(
                residual.clone()
            )]
        );
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation.clone()));
    }
}

#[test]
fn partial_affine_pair_cleanup_retains_exact_native_and_installed_projection_on_all_targets() {
    let plan = partial_affine_pair_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let pair_type = caller.structural_parameters[0].structural_type;
    let declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == pair_type)
        .expect("array declaration remains present");
    let StructuralTypeShape::FixedArray {
        element: element_type,
        length: 2,
    } = declaration.shape
    else {
        panic!("caller root remains an exact two-element array")
    };
    let residual = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => match cleanup_actions.as_slice() {
                [TerminalAffineCleanupAction::DiscardResidual(residual)] => Some(residual.clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("opposite array element remains one residual cleanup");
    assert_eq!(residual.path, [StructuralPathSegment::FixedIndex(0)]);
    assert_eq!(residual.structural_type, element_type);

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let target_caller = target_plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("target caller remains present");
        let TargetOperation::UnitBody(body) = &target_caller.operation else {
            panic!("caller remains Unit")
        };
        assert_eq!(body.parameters[0].shape, ValueShape::integer(16, 8));

        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("caller machine code exists");
        let [call] = emitted.internal_unit_calls.as_slice() else {
            panic!("caller has one projected internal call")
        };
        let [argument] = call.arguments.as_slice() else {
            panic!("call has one projected argument")
        };
        assert_eq!(argument.path, [StructuralPathSegment::FixedIndex(1)]);
        assert_eq!(argument.root_structural_type, pair_type);
        assert_eq!(argument.structural_type, element_type);
        assert_eq!(argument.shape, ValueShape::integer(8, 8));
        assert_eq!(argument.source.shape, ValueShape::integer(16, 8));
        assert_eq!(argument.source_byte_offset, 8);
        assert_eq!(argument.fixed_array_length, Some(2));
        assert_eq!(argument.element_stride, Some(8));
        assert!(call.claim_transfers.is_empty());
        assert_eq!(
            emitted
                .unit_affine_cleanup
                .as_ref()
                .expect("caller cleanup ledger")
                .actions,
            [TerminalAffineCleanupAction::DiscardResidual(
                residual.clone()
            )]
        );

        for forged in [
            {
                let mut forged = machine.clone();
                let caller = forged
                    .functions
                    .iter_mut()
                    .find(|function| function.machine == caller_machine)
                    .unwrap();
                caller.internal_unit_calls[0].arguments[0].source_byte_offset = 0;
                forged
            },
            {
                let mut forged = machine.clone();
                let caller = forged
                    .functions
                    .iter_mut()
                    .find(|function| function.machine == caller_machine)
                    .unwrap();
                caller.internal_unit_calls[0].arguments[0].fixed_array_length = Some(3);
                forged
            },
            {
                let mut forged = machine.clone();
                let caller = forged
                    .functions
                    .iter_mut()
                    .find(|function| function.machine == caller_machine)
                    .unwrap();
                caller.internal_unit_calls[0].arguments[0].element_stride = Some(16);
                forged
            },
            {
                let mut forged = machine.clone();
                let caller = forged
                    .functions
                    .iter_mut()
                    .find(|function| function.machine == caller_machine)
                    .unwrap();
                caller.unit_affine_cleanup.as_mut().unwrap().actions[0] =
                    TerminalAffineCleanupAction::DiscardResidual(
                        psi_terminal::StructuralAffineDiscard {
                            path: vec![StructuralPathSegment::FixedIndex(1)],
                            ..residual.clone()
                        },
                    );
                forged
            },
        ] {
            assert!(build_object_artifact(&forged).is_err());
        }

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed_call = installation
            .internal_unit_calls()
            .iter()
            .find(|call| call.machine == caller_machine)
            .expect("installed caller call");
        assert_eq!(installed_call.custody.arguments[0].path, argument.path);
        assert_eq!(
            installed_call.custody.arguments[0].source_byte_offset,
            argument.source_byte_offset
        );
        let installed_cleanup = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_ref()
            .expect("installed cleanup ledger");
        assert_eq!(
            installed_cleanup.actions,
            [TerminalAffineCleanupAction::DiscardResidual(
                residual.clone()
            )]
        );
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}

#[test]
fn partial_affine_triple_retains_two_calls_and_one_installed_residual_on_all_targets() {
    let plan = partial_affine_triple_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let root_place = caller.structural_parameters[0].place;
    let root_type = caller.structural_parameters[0].structural_type;
    let declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root_type)
        .expect("array declaration remains present");
    let StructuralTypeShape::FixedArray {
        element: element_type,
        length: 3,
    } = declaration.shape
    else {
        panic!("caller root remains an exact three-element array")
    };
    let residual = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => match cleanup_actions.as_slice() {
                [TerminalAffineCleanupAction::DiscardResidual(residual)] => Some(residual.clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("affine triple return retains one residual");
    assert_eq!(residual.place, root_place);
    assert_eq!(residual.path, [StructuralPathSegment::FixedIndex(1)]);
    assert_eq!(residual.structural_type, element_type);

    let mut missing_target_call = plan.clone();
    missing_target_call
        .functions
        .iter_mut()
        .find(|function| function.machine == caller_machine)
        .unwrap()
        .operations
        .remove(1);
    assert!(lower_to_target_operations(&missing_target_call, NativeTarget::linux_x64()).is_err());

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let target_caller = target_plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let TargetOperation::UnitBody(body) = &target_caller.operation else {
            panic!("caller remains Unit")
        };
        assert_eq!(body.parameters[0].shape, ValueShape::integer(24, 8));
        let assigned = assign_registers(&target_plan).unwrap();
        let mut extra_assigned_call = assigned.clone();
        let assigned_caller = extra_assigned_call
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(assigned_body) =
            &mut assigned_caller.operation
        else {
            panic!("assigned caller remains Unit")
        };
        assigned_body
            .operations
            .insert(1, assigned_body.operations[0].clone());
        assert!(emit_machine_code(&extra_assigned_call).is_err());
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("caller machine code exists");
        let [first, second] = emitted.internal_unit_calls.as_slice() else {
            panic!("affine triple retains exactly two calls")
        };
        let cleanup = emitted
            .unit_affine_cleanup
            .as_ref()
            .expect("partial affine return retains its edge record");
        assert_eq!(
            emitted
                .semantic_code_attribution
                .iter()
                .map(|attribution| attribution.operation_ordinal)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert!(matches!(
            emitted.semantic_code_attribution.as_slice(),
            [
                omega_machine_code::SemanticCodeAttribution {
                    site: omega_machine_code::SemanticCodeSite::Operation(first_site),
                    ..
                },
                omega_machine_code::SemanticCodeAttribution {
                    site: omega_machine_code::SemanticCodeSite::Operation(second_site),
                    ..
                },
                omega_machine_code::SemanticCodeAttribution {
                    site: omega_machine_code::SemanticCodeSite::Edge(return_edge),
                    ..
                },
            ] if first.owner.operation() == Some(*first_site)
                && second.owner.operation() == Some(*second_site)
                && cleanup.psi_edge == *return_edge
        ));
        assert_eq!(
            [
                first.arguments[0].path.clone(),
                second.arguments[0].path.clone()
            ],
            [
                vec![StructuralPathSegment::FixedIndex(2)],
                vec![StructuralPathSegment::FixedIndex(0)],
            ]
        );
        assert_eq!(
            [
                first.arguments[0].source_byte_offset,
                second.arguments[0].source_byte_offset,
            ],
            [16, 0]
        );
        for call in [first, second] {
            let [argument] = call.arguments.as_slice() else {
                panic!("each triple call retains one argument")
            };
            assert_eq!(argument.root_structural_type, root_type);
            assert_eq!(argument.structural_type, element_type);
            assert_eq!(argument.fixed_array_length, Some(3));
            assert_eq!(argument.element_stride, Some(8));
            assert!(call.claim_transfers.is_empty());
        }
        assert_eq!(
            cleanup.actions,
            [TerminalAffineCleanupAction::DiscardResidual(
                residual.clone()
            )]
        );

        let mut missing = machine.clone();
        missing
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .internal_unit_calls
            .remove(1);
        assert!(build_object_artifact(&missing).is_err());

        let mut duplicate = machine.clone();
        let caller = duplicate
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        caller.internal_unit_calls[1].arguments[0].path =
            caller.internal_unit_calls[0].arguments[0].path.clone();
        assert!(build_object_artifact(&duplicate).is_err());

        let mut wrong_path = machine.clone();
        wrong_path
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .internal_unit_calls[1]
            .arguments[0]
            .path = vec![StructuralPathSegment::FixedIndex(3)];
        assert!(build_object_artifact(&wrong_path).is_err());

        let mut wrong_length = machine.clone();
        wrong_length
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .internal_unit_calls[1]
            .arguments[0]
            .fixed_array_length = Some(2);
        assert!(build_object_artifact(&wrong_length).is_err());

        let mut wrong_order = machine.clone();
        wrong_order
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .internal_unit_calls
            .swap(0, 1);
        assert!(build_object_artifact(&wrong_order).is_err());

        let mut wrong_residual = machine.clone();
        wrong_residual
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions[0] =
            TerminalAffineCleanupAction::DiscardResidual(psi_terminal::StructuralAffineDiscard {
                path: vec![StructuralPathSegment::FixedIndex(0)],
                ..residual.clone()
            });
        assert!(build_object_artifact(&wrong_residual).is_err());

        let mut no_cleanup = machine.clone();
        no_cleanup
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions
            .clear();
        assert!(build_object_artifact(&no_cleanup).is_err());

        let mut third_call = machine.clone();
        let caller = third_call
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        caller
            .internal_unit_calls
            .push(caller.internal_unit_calls[0].clone());
        assert!(build_object_artifact(&third_call).is_err());

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        assert_eq!(
            installation
                .internal_unit_calls()
                .iter()
                .filter(|call| call.machine == caller_machine)
                .count(),
            2
        );
        assert_eq!(
            installation
                .functions()
                .iter()
                .find(|function| function.machine == caller_machine)
                .unwrap()
                .unit_affine_cleanup
                .as_ref()
                .unwrap()
                .actions,
            [TerminalAffineCleanupAction::DiscardResidual(
                residual.clone()
            )]
        );
        validate_installation_record(&installation, &image).unwrap();
        let encoded = encode_installation_record(&installation).unwrap();
        assert_eq!(
            decode_installation_record(&encoded),
            Ok(installation.clone())
        );
        let installed_second = &installation
            .internal_unit_calls()
            .iter()
            .filter(|call| call.machine == caller_machine)
            .nth(1)
            .unwrap()
            .custody
            .arguments[0];
        let mut projection = Vec::new();
        projection.extend_from_slice(&installed_second.place.get().to_le_bytes());
        projection.extend_from_slice(&1_u32.to_le_bytes());
        projection.extend_from_slice(&1_u32.to_le_bytes());
        projection.extend_from_slice(&[2, 0, 0, 0]);
        projection.extend_from_slice(&0_u64.to_le_bytes());
        projection.extend_from_slice(&installed_second.root_structural_type.get().to_le_bytes());
        projection.extend_from_slice(&installed_second.structural_type.get().to_le_bytes());
        projection.extend_from_slice(&[1, 0, 8, 0, 8, 0, 0, 0]);
        projection.extend_from_slice(&installed_second.source_byte_offset.to_le_bytes());
        let projection_offset = encoded
            .windows(projection.len())
            .position(|window| window == projection)
            .expect("installation retains the second triple projection");
        let mut duplicate_installed_path = encoded.clone();
        duplicate_installed_path[projection_offset + 20..projection_offset + 28]
            .copy_from_slice(&2_u64.to_le_bytes());
        assert!(decode_installation_record(&duplicate_installed_path).is_err());
    }
}

#[test]
fn partial_affine_quartet_retains_two_calls_and_decreasing_cleanup_on_all_targets() {
    let plan = partial_affine_quartet_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let root_place = caller.structural_parameters[0].place;
    let root_type = caller.structural_parameters[0].structural_type;
    let declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root_type)
        .expect("quartet declaration remains present");
    let StructuralTypeShape::FixedArray {
        element: element_type,
        length: 4,
    } = declaration.shape
    else {
        panic!("caller root remains an exact four-element array")
    };
    let residuals = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(
                cleanup_actions
                    .iter()
                    .filter_map(|action| match action {
                        TerminalAffineCleanupAction::DiscardResidual(residual) => {
                            Some(residual.clone())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .expect("quartet return retains its residuals");
    assert_eq!(
        residuals
            .iter()
            .map(|residual| residual.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![StructuralPathSegment::FixedIndex(2)],
            vec![StructuralPathSegment::FixedIndex(0)],
        ],
    );
    assert!(residuals.iter().all(|residual| {
        residual.place == root_place && residual.structural_type == element_type
    }));

    let mut increasing_cleanup = plan.clone();
    let return_actions = increasing_cleanup
        .functions
        .iter_mut()
        .find(|function| function.machine == caller_machine)
        .unwrap()
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions),
            _ => None,
        })
        .unwrap();
    return_actions.reverse();
    assert!(
        lower_to_target_operations(&increasing_cleanup, NativeTarget::linux_x64()).is_err(),
        "target lowering independently rejects increasing residual order",
    );

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let target_caller = target_plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let TargetOperation::UnitBody(body) = &target_caller.operation else {
            panic!("quartet caller remains Unit")
        };
        assert_eq!(body.parameters[0].shape, ValueShape::integer(32, 8));

        let assigned = assign_registers(&target_plan).unwrap();
        let mut duplicate_assigned = assigned.clone();
        let assigned_caller = duplicate_assigned
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(assigned_body) =
            &mut assigned_caller.operation
        else {
            panic!("assigned quartet caller remains Unit")
        };
        let duplicate = assigned_body.operations[0].clone();
        assigned_body.operations[1] = duplicate;
        assert!(
            emit_machine_code(&duplicate_assigned).is_err(),
            "assigned duplicate projection rejects",
        );

        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("quartet machine code exists");
        let [first, second] = emitted.internal_unit_calls.as_slice() else {
            panic!("quartet retains exactly two calls")
        };
        assert_eq!(
            [
                first.arguments[0].path.clone(),
                second.arguments[0].path.clone(),
            ],
            [
                vec![StructuralPathSegment::FixedIndex(1)],
                vec![StructuralPathSegment::FixedIndex(3)],
            ],
        );
        assert_eq!(
            [
                first.arguments[0].source_byte_offset,
                second.arguments[0].source_byte_offset,
            ],
            [8, 24],
        );
        for call in [first, second] {
            let argument = &call.arguments[0];
            assert_eq!(argument.root_structural_type, root_type);
            assert_eq!(argument.structural_type, element_type);
            assert_eq!(argument.fixed_array_length, Some(4));
            assert_eq!(argument.element_stride, Some(8));
        }
        assert_eq!(
            emitted
                .semantic_code_attribution
                .iter()
                .map(|attribution| attribution.operation_ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1, 2],
            "two calls and one return retain canonical fuel ordinals",
        );
        let cleanup = emitted
            .unit_affine_cleanup
            .as_ref()
            .expect("quartet return retains cleanup custody");
        assert_eq!(
            cleanup.actions,
            residuals
                .iter()
                .cloned()
                .map(TerminalAffineCleanupAction::DiscardResidual)
                .collect::<Vec<_>>(),
        );

        let mut wrong_machine_order = machine.clone();
        wrong_machine_order
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions
            .reverse();
        assert!(
            build_object_artifact(&wrong_machine_order).is_err(),
            "object replay rejects reordered cleanup custody",
        );

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            installed.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup.actions
        );
        assert_eq!(
            installation
                .internal_unit_calls()
                .iter()
                .filter(|call| call.machine == caller_machine)
                .count(),
            2,
        );
        validate_installation_record(&installation, &image).unwrap();
        let encoded = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&encoded), Ok(installation));
    }
}

#[test]
fn nested_affine_arrays_retain_exact_offsets_and_decreasing_cleanup_on_all_targets() {
    let plan = nested_affine_array_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("nested caller remains present");
    let root_place = caller.structural_parameters[0].place;
    let root_type = caller.structural_parameters[0].structural_type;
    let StructuralTypeShape::FixedArray {
        element: inner_type,
        length: 2,
    } = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root_type)
        .expect("outer declaration")
        .shape
    else {
        panic!("root remains an exact outer pair")
    };
    let StructuralTypeShape::FixedArray {
        element: leaf_type,
        length: 3,
    } = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == inner_type)
        .expect("inner declaration")
        .shape
    else {
        panic!("element remains an exact inner triple")
    };
    let residuals = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(
                cleanup_actions
                    .iter()
                    .filter_map(|action| match action {
                        TerminalAffineCleanupAction::DiscardResidual(residual) => {
                            Some(residual.clone())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .expect("nested return retains residuals");
    assert_eq!(
        residuals
            .iter()
            .map(|residual| residual.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![
                StructuralPathSegment::FixedIndex(1),
                StructuralPathSegment::FixedIndex(2),
            ],
            vec![
                StructuralPathSegment::FixedIndex(1),
                StructuralPathSegment::FixedIndex(1),
            ],
            vec![
                StructuralPathSegment::FixedIndex(0),
                StructuralPathSegment::FixedIndex(2),
            ],
            vec![
                StructuralPathSegment::FixedIndex(0),
                StructuralPathSegment::FixedIndex(0),
            ],
        ],
    );
    assert!(residuals
        .iter()
        .all(|residual| { residual.place == root_place && residual.structural_type == leaf_type }));

    let mut wrong_order = plan.clone();
    let actions = wrong_order
        .functions
        .iter_mut()
        .find(|function| function.machine == caller_machine)
        .unwrap()
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions),
            _ => None,
        })
        .unwrap();
    actions.reverse();
    let wrong_unit = reconstruct_psi_optimization_unit_seed(
        &wrong_order,
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("tampered abstract plan remains representable as optimizer input");
    assert!(
        validate_psi_optimization_unit(&wrong_unit).is_err(),
        "optimizer ownership replay rejects reversed nested cleanup",
    );
    assert!(
        lower_to_target_operations(&wrong_order, NativeTarget::linux_x64()).is_err(),
        "target lowering rejects reversed nested cleanup",
    );

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let TargetOperation::UnitBody(body) = &target_plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .operation
        else {
            panic!("nested caller remains Unit")
        };
        assert_eq!(body.parameters[0].shape, ValueShape::integer(48, 8));
        let calls = body
            .operations
            .iter()
            .filter_map(|operation| match operation {
                omega_target_operations::TargetUnitOperation::Call { arguments, .. } => {
                    Some(&arguments[0])
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls
                .iter()
                .map(|call| call.path.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![
                    StructuralPathSegment::FixedIndex(1),
                    StructuralPathSegment::FixedIndex(0),
                ],
                vec![
                    StructuralPathSegment::FixedIndex(0),
                    StructuralPathSegment::FixedIndex(1),
                ],
            ],
        );
        assert_eq!(
            calls
                .iter()
                .map(|call| call.source_byte_offset)
                .collect::<Vec<_>>(),
            vec![24, 8],
        );
        assert!(calls.iter().all(|call| {
            call.root_structural_type == root_type
                && call.structural_type == leaf_type
                && call.fixed_array_length == Some(2)
                && call.element_stride == Some(24)
        }));

        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            emitted
                .semantic_code_attribution
                .iter()
                .map(|attribution| attribution.operation_ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1, 2],
        );
        assert_eq!(
            emitted.unit_affine_cleanup.as_ref().unwrap().actions,
            residuals
                .iter()
                .cloned()
                .map(TerminalAffineCleanupAction::DiscardResidual)
                .collect::<Vec<_>>(),
        );

        let mut tampered = machine.clone();
        tampered
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions
            .reverse();
        assert!(
            build_object_artifact(&tampered).is_err(),
            "object replay rejects reversed nested cleanup",
        );

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        validate_installation_record(&installation, &image).unwrap();
        let encoded = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&encoded), Ok(installation));
    }
}

#[test]
fn fully_consumed_affine_pair_retains_two_native_calls_and_empty_installed_cleanup() {
    let plan = fully_consumed_affine_pair_plan(FULLY_CONSUMED_AFFINE_PAIR_SOURCE);
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let pair_type = caller.structural_parameters[0].structural_type;
    let declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == pair_type)
        .expect("array declaration remains present");
    let StructuralTypeShape::FixedArray {
        element: element_type,
        length: 2,
    } = declaration.shape
    else {
        panic!("caller root remains an exact two-element array")
    };
    assert!(matches!(
        caller.operations.last(),
        Some(omega_abstract_operations::AbstractOperation::ReturnUnit {
            cleanup_actions,
            ..
        }) if cleanup_actions.is_empty()
    ));

    let mut missing = plan.clone();
    let missing_caller = missing
        .functions
        .iter_mut()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    missing_caller.operations.remove(1);
    assert!(lower_to_target_operations(&missing, NativeTarget::linux_x64()).is_err());

    let mut duplicate = plan.clone();
    let duplicate_caller = duplicate
        .functions
        .iter_mut()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let first_path = match &duplicate_caller.operations[0] {
        omega_abstract_operations::AbstractOperation::CallUnit {
            structural_arguments,
            ..
        } => structural_arguments[0].path.clone(),
        _ => panic!("first operation remains a call"),
    };
    let omega_abstract_operations::AbstractOperation::CallUnit {
        structural_arguments,
        ..
    } = &mut duplicate_caller.operations[1]
    else {
        panic!("second operation remains a call")
    };
    structural_arguments[0].path = first_path;
    assert!(lower_to_target_operations(&duplicate, NativeTarget::linux_x64()).is_err());

    let mut wrong_length = plan.clone();
    let wrong_root = wrong_length
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == pair_type)
        .unwrap();
    wrong_root.shape = StructuralTypeShape::FixedArray {
        element: element_type,
        length: 3,
    };
    assert!(lower_to_target_operations(&wrong_length, NativeTarget::linux_x64()).is_err());

    let mut added_cleanup = plan.clone();
    let cleanup_caller = added_cleanup
        .functions
        .iter_mut()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let Some(omega_abstract_operations::AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    }) = cleanup_caller.operations.last_mut()
    else {
        panic!("caller ends in Unit return")
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
        caller.structural_parameters[0].place,
    ));
    assert!(lower_to_target_operations(&added_cleanup, NativeTarget::linux_x64()).is_err());

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("caller machine code exists");
        let [first, second] = emitted.internal_unit_calls.as_slice() else {
            panic!("caller retains exactly two internal calls")
        };
        let cleanup = emitted
            .unit_affine_cleanup
            .as_ref()
            .expect("ordinary return retains its edge record");
        assert_eq!(
            emitted
                .semantic_code_attribution
                .iter()
                .map(|attribution| attribution.operation_ordinal)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert!(matches!(
            emitted.semantic_code_attribution.as_slice(),
            [
                omega_machine_code::SemanticCodeAttribution {
                    site: omega_machine_code::SemanticCodeSite::Operation(first_site),
                    ..
                },
                omega_machine_code::SemanticCodeAttribution {
                    site: omega_machine_code::SemanticCodeSite::Operation(second_site),
                    ..
                },
                omega_machine_code::SemanticCodeAttribution {
                    site: omega_machine_code::SemanticCodeSite::Edge(return_edge),
                    ..
                },
            ] if first.owner.operation() == Some(*first_site)
                && second.owner.operation() == Some(*second_site)
                && cleanup.psi_edge == *return_edge
        ));
        assert_eq!(
            [
                first.arguments[0].path.clone(),
                second.arguments[0].path.clone()
            ],
            [
                vec![StructuralPathSegment::FixedIndex(1)],
                vec![StructuralPathSegment::FixedIndex(0)],
            ]
        );
        assert_eq!(
            [
                first.arguments[0].source_byte_offset,
                second.arguments[0].source_byte_offset,
            ],
            [8, 0]
        );
        for call in [first, second] {
            let [argument] = call.arguments.as_slice() else {
                panic!("each call retains one projected argument")
            };
            assert_eq!(argument.root_structural_type, pair_type);
            assert_eq!(argument.structural_type, element_type);
            assert_eq!(argument.fixed_array_length, Some(2));
            assert_eq!(argument.element_stride, Some(8));
            assert!(call.claim_transfers.is_empty());
        }
        assert!(cleanup.actions.is_empty());

        let mut missing_call = machine.clone();
        missing_call
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .internal_unit_calls
            .remove(1);
        assert!(build_object_artifact(&missing_call).is_err());

        let mut duplicate_path = machine.clone();
        let duplicate_machine_caller = duplicate_path
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        duplicate_machine_caller.internal_unit_calls[1].arguments[0].path =
            duplicate_machine_caller.internal_unit_calls[0].arguments[0]
                .path
                .clone();
        assert!(build_object_artifact(&duplicate_path).is_err());

        let mut wrong_path = machine.clone();
        wrong_path
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .internal_unit_calls[1]
            .arguments[0]
            .path = vec![StructuralPathSegment::Field("value".into())];
        assert!(build_object_artifact(&wrong_path).is_err());

        let mut wrong_native_length = machine.clone();
        wrong_native_length
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .internal_unit_calls[1]
            .arguments[0]
            .fixed_array_length = Some(3);
        assert!(build_object_artifact(&wrong_native_length).is_err());

        let mut wrong_order = machine.clone();
        wrong_order
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .internal_unit_calls
            .swap(0, 1);
        assert!(build_object_artifact(&wrong_order).is_err());

        let mut native_cleanup = machine.clone();
        native_cleanup
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions
            .push(TerminalAffineCleanupAction::DiscardRoot(
                caller.structural_parameters[0].place,
            ));
        assert!(build_object_artifact(&native_cleanup).is_err());

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed_calls = installation
            .internal_unit_calls()
            .iter()
            .filter(|call| call.machine == caller_machine)
            .collect::<Vec<_>>();
        assert_eq!(installed_calls.len(), 2);
        assert!(installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_ref()
            .unwrap()
            .actions
            .is_empty());
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation.clone()));

        let installed_second = &installed_calls[1].custody.arguments[0];
        let mut projection = Vec::new();
        projection.extend_from_slice(&installed_second.place.get().to_le_bytes());
        projection.extend_from_slice(&1_u32.to_le_bytes());
        projection.extend_from_slice(&1_u32.to_le_bytes());
        projection.extend_from_slice(&[2, 0, 0, 0]);
        projection.extend_from_slice(&0_u64.to_le_bytes());
        projection.extend_from_slice(&installed_second.root_structural_type.get().to_le_bytes());
        projection.extend_from_slice(&installed_second.structural_type.get().to_le_bytes());
        projection.extend_from_slice(&[1, 0, 8, 0, 8, 0, 0, 0]);
        projection.extend_from_slice(&installed_second.source_byte_offset.to_le_bytes());
        let offset = bytes
            .windows(projection.len())
            .position(|window| window == projection)
            .expect("installation retains the second exact array projection");
        let mut duplicate_installed_path = bytes.clone();
        duplicate_installed_path[offset + 20..offset + 28].copy_from_slice(&1_u64.to_le_bytes());
        assert!(decode_installation_record(&duplicate_installed_path).is_err());
    }

    let forward = fully_consumed_affine_pair_plan(FORWARD_FULLY_CONSUMED_AFFINE_PAIR_SOURCE);
    let forward_caller = forward.entry;
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&forward, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == forward_caller)
            .unwrap();
        assert_eq!(
            emitted
                .internal_unit_calls
                .iter()
                .map(|call| call.arguments[0].path.clone())
                .collect::<Vec<_>>(),
            [
                vec![StructuralPathSegment::FixedIndex(0)],
                vec![StructuralPathSegment::FixedIndex(1)],
            ]
        );
        assert!(emitted
            .unit_affine_cleanup
            .as_ref()
            .unwrap()
            .actions
            .is_empty());
        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        validate_installation_record(&installation, &image).unwrap();
        assert_eq!(
            decode_installation_record(&encode_installation_record(&installation).unwrap()),
            Ok(installation)
        );
    }
}

#[test]
fn mixed_scalar_partial_affine_cleanup_preserves_identity_on_all_targets() {
    let plan = mixed_scalar_partial_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("mixed-scalar caller remains present");
    let root_type = caller.structural_parameters[0].structural_type;
    let root_declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root_type)
        .expect("mixed-scalar root type remains declared");
    let StructuralTypeShape::Record { fields } = &root_declaration.shape else {
        panic!("mixed-scalar root remains a record")
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| (field.identity.as_str(), &field.field_type))
            .collect::<Vec<_>>(),
        vec![
            (
                "before",
                &StructuralFieldType::Scalar(psi_core::ScalarType::Integer(
                    psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
                ))
            ),
            (
                "before_bytes",
                &StructuralFieldType::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity: 3 }
                )
            ),
            (
                "before_float",
                &StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32)
            ),
            ("left", &fields[3].field_type),
            (
                "between",
                &StructuralFieldType::Scalar(psi_core::ScalarType::Boolean)
            ),
            (
                "between_bytes",
                &StructuralFieldType::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity: 8 }
                )
            ),
            (
                "between_float",
                &StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64)
            ),
            ("right", &fields[7].field_type),
            (
                "after",
                &StructuralFieldType::Scalar(psi_core::ScalarType::Integer(
                    psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 16).unwrap(),
                ))
            ),
        ]
    );
    let left_type = match fields[3].field_type {
        StructuralFieldType::Structural(structural_type) => structural_type,
        _ => panic!("left field remains structural"),
    };
    assert!(matches!(
        fields[7].field_type,
        StructuralFieldType::Structural(_)
    ));
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .expect("mixed-scalar return retains cleanup actions");
    let [TerminalAffineCleanupAction::DiscardResidual(residual)] = cleanup_actions.as_slice()
    else {
        panic!("only the live affine left field requires cleanup")
    };
    assert_eq!(residual.path, [StructuralPathSegment::Field("left".into())]);

    let mut moved_as_float = plan.clone();
    let moved_root = moved_as_float
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == root_type)
        .unwrap();
    let StructuralTypeShape::Record { fields } = &mut moved_root.shape else {
        unreachable!()
    };
    fields
        .iter_mut()
        .find(|field| field.identity == "right")
        .unwrap()
        .field_type = StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32);
    assert!(
        lower_to_target_operations(&moved_as_float, NativeTarget::linux_x64()).is_err(),
        "a retained projected move cannot be rebound to a float leaf"
    );

    let mut moved_as_bytes = plan.clone();
    let moved_root = moved_as_bytes
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == root_type)
        .unwrap();
    let StructuralTypeShape::Record { fields } = &mut moved_root.shape else {
        unreachable!()
    };
    fields
        .iter_mut()
        .find(|field| field.identity == "right")
        .unwrap()
        .field_type =
        StructuralFieldType::ByteSequence(psi_terminal::ByteSequenceCarrier::BoundedOwned {
            capacity: 3,
        });
    assert!(
        lower_to_target_operations(&moved_as_bytes, NativeTarget::linux_x64()).is_err(),
        "a retained projected move cannot be rebound to bounded byte storage"
    );

    let mut borrowed_view = plan.clone();
    let moved_root = borrowed_view
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == root_type)
        .unwrap();
    let StructuralTypeShape::Record { fields } = &mut moved_root.shape else {
        unreachable!()
    };
    fields
        .iter_mut()
        .find(|field| field.identity == "before_bytes")
        .unwrap()
        .field_type =
        StructuralFieldType::ByteSequence(psi_terminal::ByteSequenceCarrier::BorrowedView);
    assert!(
        lower_to_target_operations(&borrowed_view, NativeTarget::linux_x64()).is_err(),
        "a borrowed view cannot enter bounded-storage no-code cleanup"
    );

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let target_caller = target_plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let TargetOperation::UnitBody(target_body) = &target_caller.operation else {
            panic!("mixed-byte caller remains a Unit body")
        };
        assert_eq!(
            target_body.parameters[0].shape,
            ValueShape::integer(72, 8),
            "bounded carriers contribute their exact N+8 layout on {target:?}"
        );
        let assigned = assign_registers(&target_plan).unwrap();

        let mut forged_assigned = assigned.clone();
        let forged_caller = forged_assigned
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut forged_caller.operation
        else {
            panic!("mixed-scalar caller remains a Unit body")
        };
        let forged_root = body
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.id == root_type)
            .unwrap();
        let StructuralTypeShape::Record { fields } = &mut forged_root.shape else {
            panic!("mixed-scalar root remains a record")
        };
        fields
            .iter_mut()
            .find(|field| field.identity == "before_float")
            .unwrap()
            .field_type = StructuralFieldType::Structural(left_type);
        assert!(emit_machine_code(&forged_assigned).is_err());

        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            emitted.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );

        let mut forged_machine = machine.clone();
        let forged_cleanup = forged_machine
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap();
        let forged_root = forged_cleanup
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.id == root_type)
            .unwrap();
        let StructuralTypeShape::Record { fields } = &mut forged_root.shape else {
            panic!("mixed-scalar cleanup root remains a record")
        };
        fields
            .iter_mut()
            .find(|field| field.identity == "before_float")
            .unwrap()
            .field_type = StructuralFieldType::Structural(left_type);
        assert!(build_object_artifact(&forged_machine).is_err());

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        assert_eq!(
            installation
                .functions()
                .iter()
                .find(|function| function.machine == caller_machine)
                .unwrap()
                .unit_affine_cleanup
                .as_ref()
                .unwrap()
                .actions,
            cleanup_actions
        );
        validate_installation_record(&installation, &image).unwrap();

        let mut changed_capacity = encode_installation_record(&installation).unwrap();
        let mut encoded_field = Vec::new();
        encoded_field.extend_from_slice(&12_u32.to_le_bytes());
        encoded_field.extend_from_slice(b"before_bytes");
        encoded_field.extend_from_slice(&[0, 6, 2, 0]);
        encoded_field.extend_from_slice(&3_u64.to_le_bytes());
        let capacity_offset = changed_capacity
            .windows(encoded_field.len())
            .position(|window| window == encoded_field)
            .expect("installation retains exact bounded-byte field row")
            + encoded_field.len()
            - 8;
        changed_capacity[capacity_offset..capacity_offset + 8]
            .copy_from_slice(&4_u64.to_le_bytes());
        let changed_capacity = decode_installation_record(&changed_capacity)
            .expect("changed nonzero capacity remains structurally decodable");
        assert!(
            validate_installation_record(&changed_capacity, &image).is_err(),
            "installation replay rejects bounded-byte capacity drift on {target:?}"
        );
    }
}

#[test]
fn wide_partial_affine_cleanup_preserves_reverse_field_order_without_code() {
    let plan = wide_partial_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .expect("wide partial return retains cleanup actions");
    let [TerminalAffineCleanupAction::DiscardResidual(middle), TerminalAffineCleanupAction::DiscardResidual(left)] =
        cleanup_actions.as_slice()
    else {
        panic!("wide partial return retains two residual fields")
    };
    assert_eq!(middle.path, [StructuralPathSegment::Field("middle".into())]);
    assert_eq!(left.path, [StructuralPathSegment::Field("left".into())]);
    assert_eq!(middle.place, left.place);
    assert_ne!(middle.structural_type, left.structural_type);

    let mut reordered = plan.clone();
    let reordered_caller = reordered
        .functions
        .iter_mut()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let omega_abstract_operations::AbstractOperation::ReturnUnit {
        cleanup_actions: reordered_actions,
        ..
    } = reordered_caller.operations.last_mut().unwrap()
    else {
        panic!("caller ends in a Unit return")
    };
    reordered_actions.swap(0, 1);
    assert!(lower_to_target_operations(&reordered, NativeTarget::linux_x64()).is_err());

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            emitted.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );
        let [call] = emitted.internal_unit_calls.as_slice() else {
            panic!("wide partial caller retains one projected call")
        };
        let [argument] = call.arguments.as_slice() else {
            panic!("wide partial call retains one projected argument")
        };
        assert_eq!(
            argument.path,
            [StructuralPathSegment::Field("right".into())]
        );
        assert_eq!(argument.source_byte_offset, 16);

        let mut root_cleanup_assigned = assigned.clone();
        let root_cleanup_caller = root_cleanup_assigned
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut root_cleanup_caller.operation
        else {
            panic!("caller remains a Unit body")
        };
        let omega_assigned_target_operations::AssignedUnitOperation::Return {
            cleanup_actions: root_actions,
            ..
        } = body.operations.last_mut().unwrap()
        else {
            panic!("caller ends in a Unit return")
        };
        *root_actions = vec![TerminalAffineCleanupAction::DiscardRoot(middle.place)];
        let root_cleanup_machine = emit_machine_code(&root_cleanup_assigned).unwrap();
        let root_cleanup_bytes = &root_cleanup_machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .bytes;
        assert_eq!(
            &emitted.bytes, root_cleanup_bytes,
            "two residual field actions add no runtime instruction bytes"
        );

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed_actions = &installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_ref()
            .unwrap()
            .actions;
        assert_eq!(installed_actions, &cleanup_actions);
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}

#[test]
fn multiple_direct_moves_preserve_exact_residual_complement_on_all_targets() {
    let plan = multiple_move_partial_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("multiple-move caller remains present");
    let root_type = caller.structural_parameters[0].structural_type;
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .expect("multiple-move return retains residual cleanup");
    let [TerminalAffineCleanupAction::DiscardResidual(third), TerminalAffineCleanupAction::DiscardResidual(first)] =
        cleanup_actions.as_slice()
    else {
        panic!("multiple-move cleanup retains the reverse residual complement")
    };
    assert_eq!(third.path, [StructuralPathSegment::Field("third".into())]);
    assert_eq!(first.path, [StructuralPathSegment::Field("first".into())]);
    assert_eq!(third.place, first.place);

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("multiple-move caller machine code exists");
        let [first_call, second_call] = emitted.internal_unit_calls.as_slice() else {
            panic!("multiple-move caller retains two projected calls")
        };
        let [second] = first_call.arguments.as_slice() else {
            panic!("first call retains the second-field projection")
        };
        let [fourth] = second_call.arguments.as_slice() else {
            panic!("second call retains the fourth-field projection")
        };
        assert_eq!(second.root_structural_type, root_type);
        assert_eq!(fourth.root_structural_type, root_type);
        assert_eq!(second.path, [StructuralPathSegment::Field("second".into())]);
        assert_eq!(fourth.path, [StructuralPathSegment::Field("fourth".into())]);
        assert_eq!(second.source_byte_offset, 8);
        assert_eq!(fourth.source_byte_offset, 24);
        assert_eq!(
            emitted.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );

        let mut root_cleanup_assigned = assigned.clone();
        let root_cleanup_caller = root_cleanup_assigned
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut root_cleanup_caller.operation
        else {
            panic!("multiple-move caller remains a Unit body")
        };
        let omega_assigned_target_operations::AssignedUnitOperation::Return {
            cleanup_actions: root_actions,
            ..
        } = body.operations.last_mut().unwrap()
        else {
            panic!("multiple-move caller ends in a Unit return")
        };
        *root_actions = vec![TerminalAffineCleanupAction::DiscardRoot(third.place)];
        let root_cleanup_machine = emit_machine_code(&root_cleanup_assigned).unwrap();
        let root_cleanup_bytes = &root_cleanup_machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .bytes;
        assert_eq!(
            &emitted.bytes, root_cleanup_bytes,
            "multiple residual actions add no runtime instruction bytes"
        );

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            installed.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}

#[test]
fn nested_move_preserves_maximal_residual_subtrees_on_all_targets() {
    let plan = nested_partial_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("nested partial caller remains present");
    let root_type = caller.structural_parameters[0].structural_type;
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .expect("nested partial return retains cleanup actions");
    let [TerminalAffineCleanupAction::DiscardResidual(last), TerminalAffineCleanupAction::DiscardResidual(right), TerminalAffineCleanupAction::DiscardResidual(left), TerminalAffineCleanupAction::DiscardResidual(first)] =
        cleanup_actions.as_slice()
    else {
        panic!("nested cleanup retains four maximal sibling subtrees")
    };
    assert_eq!(last.path, [StructuralPathSegment::Field("last".into())]);
    assert_eq!(
        right.path,
        [
            StructuralPathSegment::Field("inner".into()),
            StructuralPathSegment::Field("right".into()),
        ]
    );
    assert_eq!(
        left.path,
        [
            StructuralPathSegment::Field("inner".into()),
            StructuralPathSegment::Field("left".into()),
        ]
    );
    assert_eq!(first.path, [StructuralPathSegment::Field("first".into())]);
    assert!(cleanup_actions.iter().all(|action| matches!(action,
        TerminalAffineCleanupAction::DiscardResidual(residual) if residual.place == last.place)));

    let mut reordered = plan.clone();
    let reordered_caller = reordered
        .functions
        .iter_mut()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let omega_abstract_operations::AbstractOperation::ReturnUnit {
        cleanup_actions: reordered_actions,
        ..
    } = reordered_caller.operations.last_mut().unwrap()
    else {
        panic!("nested caller ends in a Unit return")
    };
    reordered_actions.swap(1, 2);
    assert!(lower_to_target_operations(&reordered, NativeTarget::linux_x64()).is_err());

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("nested partial caller machine code exists");
        let [call] = emitted.internal_unit_calls.as_slice() else {
            panic!("nested partial caller retains one projected call")
        };
        let [argument] = call.arguments.as_slice() else {
            panic!("nested call retains one projection")
        };
        assert_eq!(argument.root_structural_type, root_type);
        assert_eq!(
            argument.path,
            [
                StructuralPathSegment::Field("inner".into()),
                StructuralPathSegment::Field("middle".into()),
            ]
        );
        assert_eq!(argument.source_byte_offset, 16);
        assert_eq!(
            emitted.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );

        let mut root_cleanup_assigned = assigned.clone();
        let root_cleanup_caller = root_cleanup_assigned
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut root_cleanup_caller.operation
        else {
            panic!("nested caller remains a Unit body")
        };
        let omega_assigned_target_operations::AssignedUnitOperation::Return {
            cleanup_actions: root_actions,
            ..
        } = body.operations.last_mut().unwrap()
        else {
            panic!("nested caller ends in a Unit return")
        };
        *root_actions = vec![TerminalAffineCleanupAction::DiscardRoot(last.place)];
        let root_cleanup_machine = emit_machine_code(&root_cleanup_assigned).unwrap();
        let root_cleanup_bytes = &root_cleanup_machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .bytes;
        assert_eq!(
            &emitted.bytes, root_cleanup_bytes,
            "nested residual cleanup adds no runtime instruction bytes"
        );

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            installed.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}

#[test]
fn mixed_depth_moves_preserve_exact_partition_and_artifact_type_graph() {
    let plan = mixed_depth_partial_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .unwrap();
    let expected_paths = [
        vec![StructuralPathSegment::Field("post".into())],
        vec![
            StructuralPathSegment::Field("inner".into()),
            StructuralPathSegment::Field("c".into()),
        ],
        vec![
            StructuralPathSegment::Field("inner".into()),
            StructuralPathSegment::Field("deep".into()),
            StructuralPathSegment::Field("z".into()),
        ],
        vec![
            StructuralPathSegment::Field("inner".into()),
            StructuralPathSegment::Field("deep".into()),
            StructuralPathSegment::Field("x".into()),
        ],
        vec![StructuralPathSegment::Field("pre".into())],
    ];
    assert_eq!(cleanup_actions.len(), expected_paths.len());
    for (action, path) in cleanup_actions.iter().zip(&expected_paths) {
        assert!(matches!(action,
            TerminalAffineCleanupAction::DiscardResidual(residual) if residual.path == *path));
    }

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let moved = emitted
            .internal_unit_calls
            .iter()
            .map(|call| {
                let [argument] = call.arguments.as_slice() else {
                    panic!("each mixed-depth call has one argument")
                };
                (argument.path.clone(), argument.source_byte_offset)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            moved,
            [
                (vec![StructuralPathSegment::Field("direct".into())], 8),
                (
                    vec![
                        StructuralPathSegment::Field("inner".into()),
                        StructuralPathSegment::Field("a".into()),
                    ],
                    16,
                ),
                (
                    vec![
                        StructuralPathSegment::Field("inner".into()),
                        StructuralPathSegment::Field("deep".into()),
                        StructuralPathSegment::Field("y".into()),
                    ],
                    32,
                ),
            ]
        );
        let cleanup = emitted.unit_affine_cleanup.as_ref().unwrap();
        assert_eq!(cleanup.actions, cleanup_actions);
        assert!(!cleanup.structural_types.is_empty());

        let mut omitted = machine.clone();
        omitted
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions
            .remove(2);
        assert!(build_object_artifact(&omitted).is_err());

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed_cleanup = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_ref()
            .unwrap();
        assert_eq!(installed_cleanup.actions, cleanup_actions);
        assert_eq!(installed_cleanup.structural_types, cleanup.structural_types);
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}

#[test]
fn partial_affine_cleanup_rejects_a_residual_before_its_local_cleanup() {
    let mut plan = partial_affine_plan();
    let empty_type = plan
        .structural_types
        .iter()
        .find(|declaration| {
            matches!(&declaration.shape, StructuralTypeShape::Record { fields } if fields.is_empty())
        })
        .cloned()
        .expect("partial-cleanup closure retains an empty record type");
    let local_place = PlaceId::new(10_000).unwrap();
    let local_operation = OperationId::new(10_000).unwrap();
    let entry = plan.entry;
    let return_index = {
        let caller = plan
            .functions
            .iter_mut()
            .find(|function| function.machine == entry)
            .expect("entry caller remains present");
        let return_index = caller
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation,
                    omega_abstract_operations::AbstractOperation::ReturnUnit { .. }
                )
            })
            .expect("partial-cleanup caller returns Unit");
        caller.operations.insert(
            return_index,
            omega_abstract_operations::AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation: local_operation,
                place: StructuralPlaceDeclaration {
                    id: local_place,
                    kind: StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal: 0,
                        structural_type: empty_type.id,
                        construction: None,
                    },
                },
                structural_type: empty_type,
            },
        );
        let omega_abstract_operations::AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } = &mut caller.operations[return_index + 1]
        else {
            unreachable!("located Unit return remains at the next operation")
        };
        let [residual] = cleanup_actions.as_slice() else {
            panic!("partial-cleanup return retains one residual action")
        };
        let residual = residual.clone();
        *cleanup_actions = vec![
            TerminalAffineCleanupAction::DiscardRoot(local_place),
            residual,
        ];
        return_index
    };

    lower_to_target_operations(&plan, NativeTarget::linux_x64())
        .expect("reverse-local cleanup followed by the residual is canonical");
    let caller = plan
        .functions
        .iter_mut()
        .find(|function| function.machine == entry)
        .expect("entry caller remains present");
    let omega_abstract_operations::AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut caller.operations[return_index + 1]
    else {
        unreachable!("located Unit return remains at the next operation")
    };
    cleanup_actions.swap(0, 1);
    assert!(lower_to_target_operations(&plan, NativeTarget::linux_x64()).is_err());
}

#[test]
fn two_empty_nominal_cleanups_are_reverse_ordered_and_call_free_on_all_targets() {
    let plan = two_empty_nominal_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let parameter_places = caller
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<Vec<_>>();
    assert_eq!(parameter_places.len(), 2);
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .expect("entry return retains cleanup actions");
    let [TerminalAffineCleanupAction::InvokeNominal(first), TerminalAffineCleanupAction::InvokeNominal(second)] =
        cleanup_actions.as_slice()
    else {
        panic!("entry return must invoke exactly two nominal cleanups")
    };
    assert_eq!(
        [first.place, second.place],
        [parameter_places[1], parameter_places[0]]
    );
    assert_eq!(first.cleanup_machine, second.cleanup_machine);

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let emitted_cleanup = emitted.unit_affine_cleanup.as_ref().unwrap();
        assert_eq!(emitted_cleanup.actions, cleanup_actions);
        assert!(
            emitted.internal_unit_calls.is_empty(),
            "two empty cleanups emit no calls for {target:?}"
        );

        let mut swapped = machine.clone();
        swapped
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions
            .swap(0, 1);
        assert!(build_object_artifact(&swapped).is_err());

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            installed.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );
        assert!(installation.internal_unit_calls().is_empty());
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}

#[test]
fn contextual_nominal_cleanup_is_verified_then_projected_on_all_targets() {
    let plan = contextual_nominal_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("contextual cleanup caller remains present");
    let cleanup = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => match cleanup_actions.as_slice() {
                [TerminalAffineCleanupAction::InvokeNominal(cleanup)] => Some(cleanup.clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("verified contextual cleanup reaches the Omega action stream");
    assert!(cleanup.cleanup_receiver.is_none());
    assert!(cleanup.requirement_obligations.is_empty());

    let mut forged = plan.clone();
    let forged_caller = forged
        .functions
        .iter_mut()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let omega_abstract_operations::AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = forged_caller.operations.last_mut().unwrap()
    else {
        panic!("contextual caller ends at Unit return")
    };
    let TerminalAffineCleanupAction::InvokeNominal(forged_cleanup) = &mut cleanup_actions[0] else {
        unreachable!()
    };
    forged_cleanup.cleanup_receiver = Some(PlaceId::new(99).unwrap());
    assert!(lower_to_target_operations(&forged, NativeTarget::linux_x64()).is_err());

    let mut forged = plan.clone();
    let forged_caller = forged
        .functions
        .iter_mut()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let omega_abstract_operations::AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = forged_caller.operations.last_mut().unwrap()
    else {
        panic!("contextual caller ends at Unit return")
    };
    let TerminalAffineCleanupAction::InvokeNominal(forged_cleanup) = &mut cleanup_actions[0] else {
        unreachable!()
    };
    forged_cleanup
        .requirement_obligations
        .push(psi_core::ObligationId::new(1).unwrap());
    assert!(lower_to_target_operations(&forged, NativeTarget::linux_x64()).is_err());

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let target_caller = target_plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let TargetOperation::UnitBody(target_body) = &target_caller.operation else {
            panic!("contextual caller remains Unit")
        };
        let omega_target_operations::TargetUnitOperation::Return {
            cleanup_actions, ..
        } = target_body.operations.last().unwrap()
        else {
            panic!("contextual caller retains its return cleanup")
        };
        assert_eq!(
            cleanup_actions,
            &[TerminalAffineCleanupAction::InvokeNominal(cleanup.clone())]
        );

        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            emitted.unit_affine_cleanup.as_ref().unwrap().actions,
            [TerminalAffineCleanupAction::InvokeNominal(cleanup.clone())]
        );
        assert!(
            emitted.internal_unit_calls.is_empty(),
            "the contextual premise remains proof-only and the empty drop emits no call for {target:?}"
        );

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            installed.unit_affine_cleanup.as_ref().unwrap().actions,
            [TerminalAffineCleanupAction::InvokeNominal(cleanup.clone())]
        );
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}

#[test]
fn ordered_contextual_nominal_cleanups_are_verified_then_projected_on_all_targets() {
    let plan = ordered_contextual_nominal_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("ordered contextual cleanup caller remains present");
    let parameter_places = caller
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<Vec<_>>();
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .expect("ordered contextual cleanup reaches Omega actions");
    let [TerminalAffineCleanupAction::InvokeNominal(third), TerminalAffineCleanupAction::InvokeNominal(second), TerminalAffineCleanupAction::InvokeNominal(first)] =
        cleanup_actions.as_slice()
    else {
        panic!("ordered contextual cleanup retains three nominal actions")
    };
    assert_eq!(
        [third.place, second.place, first.place],
        [
            parameter_places[2],
            parameter_places[1],
            parameter_places[0]
        ]
    );
    assert_eq!(third.cleanup_machine, first.cleanup_machine);
    assert_ne!(third.cleanup_machine, second.cleanup_machine);
    for cleanup in [third, second, first] {
        assert!(cleanup.cleanup_receiver.is_none());
        assert!(cleanup.requirement_obligations.is_empty());
    }

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            emitted.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );
        assert_eq!(emitted.internal_unit_calls.len(), 3);
        for (ordinal, call) in emitted.internal_unit_calls.iter().enumerate() {
            let TerminalAffineCleanupAction::InvokeNominal(cleanup) = &cleanup_actions[ordinal]
            else {
                unreachable!()
            };
            assert_eq!(call.target, cleanup.cleanup_machine);
            assert_eq!(
                call.owner,
                CallSiteOwner::CleanupAction {
                    edge: emitted.unit_affine_cleanup.as_ref().unwrap().psi_edge,
                    action_ordinal: u32::try_from(ordinal).unwrap(),
                }
            );
        }
        assert!(emitted
            .internal_unit_calls
            .windows(2)
            .all(|pair| pair[0].code_offset + pair[0].byte_count <= pair[1].code_offset));

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            installed.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );
        let installed_caller_calls = installation
            .internal_unit_calls()
            .iter()
            .filter(|call| call.machine == caller_machine)
            .collect::<Vec<_>>();
        assert_eq!(installed_caller_calls.len(), 3);
        assert_eq!(installation.internal_unit_calls().len(), 5);
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}

#[test]
fn one_executable_nominal_cleanup_action_retains_its_exact_ordinal_on_all_targets() {
    for (source, executable_action_ordinal) in [
        (SECOND_EXECUTABLE_NOMINAL_AFFINE_SOURCE, 0_u32),
        (FIRST_EXECUTABLE_NOMINAL_AFFINE_SOURCE, 1_u32),
    ] {
        let plan = two_nominal_one_executable_plan(source);
        let caller_machine = plan.entry;
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("entry caller remains present");
        let cleanup_actions = caller
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_abstract_operations::AbstractOperation::ReturnUnit {
                    cleanup_actions,
                    ..
                } => Some(cleanup_actions.clone()),
                _ => None,
            })
            .expect("entry return retains cleanup actions");
        assert_eq!(cleanup_actions.len(), 2);
        let TerminalAffineCleanupAction::InvokeNominal(executable_cleanup) =
            &cleanup_actions[usize::try_from(executable_action_ordinal).unwrap()]
        else {
            unreachable!("both ordered actions remain nominal")
        };

        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::uefi_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let target_plan = lower_to_target_operations(&plan, target).unwrap();
            let assigned = assign_registers(&target_plan).unwrap();
            let machine = emit_machine_code(&assigned).unwrap();
            let emitted = machine
                .functions
                .iter()
                .find(|function| function.machine == caller_machine)
                .unwrap();
            let emitted_cleanup = emitted.unit_affine_cleanup.as_ref().unwrap();
            assert_eq!(emitted_cleanup.actions, cleanup_actions);
            let [cleanup_call] = emitted.internal_unit_calls.as_slice() else {
                panic!("exactly one ordered cleanup action emits a call for {target:?}")
            };
            let expected_owner = CallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: executable_action_ordinal,
            };
            assert_eq!(cleanup_call.owner, expected_owner);
            assert_eq!(cleanup_call.target, executable_cleanup.cleanup_machine);
            assert!(emitted.internal_calls.iter().any(|call| {
                call.owner == expected_owner && call.target == executable_cleanup.cleanup_machine
            }));

            let mut forged = machine.clone();
            let forged_caller = forged
                .functions
                .iter_mut()
                .find(|function| function.machine == caller_machine)
                .unwrap();
            let forged_owner = CallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: 1 - executable_action_ordinal,
            };
            forged_caller.internal_calls[0].owner = forged_owner;
            forged_caller.internal_unit_calls[0].owner = forged_owner;
            assert!(build_object_artifact(&forged).is_err());

            let object = build_object_artifact(&machine).unwrap();
            let image = emit_executable_image(&object, 3).unwrap();
            let installation =
                build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
            let installed_call = installation
                .internal_unit_calls()
                .iter()
                .find(|call| call.machine == caller_machine)
                .expect("installed caller cleanup call");
            assert_eq!(installed_call.custody.owner, expected_owner);
            validate_installation_record(&installation, &image).unwrap();
            let bytes = encode_installation_record(&installation).unwrap();
            let mut owner_encoding = vec![2, 0, 0, 0];
            owner_encoding.extend_from_slice(&emitted_cleanup.psi_edge.get().to_le_bytes());
            owner_encoding.extend_from_slice(&executable_action_ordinal.to_le_bytes());
            owner_encoding.extend_from_slice(&0_u32.to_le_bytes());
            let mut installed_call_header = Vec::new();
            installed_call_header.extend_from_slice(&installed_call.machine.get().to_le_bytes());
            installed_call_header.extend_from_slice(
                &u64::try_from(installed_call.text_offset)
                    .expect("installed call offset remains encodable")
                    .to_le_bytes(),
            );
            installed_call_header.extend_from_slice(&owner_encoding);
            installed_call_header
                .extend_from_slice(&installed_call.custody.target.get().to_le_bytes());
            let installed_call_offsets = bytes
                .windows(installed_call_header.len())
                .enumerate()
                .filter_map(|(offset, window)| (window == installed_call_header).then_some(offset))
                .collect::<Vec<_>>();
            let [installed_call_offset] = installed_call_offsets.as_slice() else {
                panic!(
                    "installation encodes one exact installed cleanup-call header, found {}",
                    installed_call_offsets.len()
                )
            };
            assert!(
                bytes[..*installed_call_offset]
                    .windows(owner_encoding.len())
                    .any(|window| window == owner_encoding),
                "the earlier stack-evidence duplicate must not select the installed call"
            );
            let mut forged_ordinal = bytes.clone();
            let ordinal_offset = installed_call_offset + 16 + 12;
            forged_ordinal[ordinal_offset..ordinal_offset + 4]
                .copy_from_slice(&(1 - executable_action_ordinal).to_le_bytes());
            assert!(decode_installation_record(&forged_ordinal).is_err());
            assert_eq!(decode_installation_record(&bytes), Ok(installation));
        }
    }
}

#[test]
fn two_executable_nominal_cleanup_actions_retain_order_and_custody_on_all_targets() {
    for (source, shared_target) in [
        (TWO_EXECUTABLE_NOMINAL_AFFINE_SOURCE, false),
        (SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE, true),
    ] {
        let plan = two_nominal_one_executable_plan(source);
        let caller_machine = plan.entry;
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("entry caller remains present");
        let cleanup_actions = caller
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_abstract_operations::AbstractOperation::ReturnUnit {
                    cleanup_actions,
                    ..
                } => Some(cleanup_actions.clone()),
                _ => None,
            })
            .expect("entry return retains cleanup actions");
        let [TerminalAffineCleanupAction::InvokeNominal(first), TerminalAffineCleanupAction::InvokeNominal(second)] =
            cleanup_actions.as_slice()
        else {
            panic!("entry return invokes two nominal cleanups")
        };
        assert_eq!(first.place, caller.structural_parameters[1].place);
        assert_eq!(second.place, caller.structural_parameters[0].place);
        assert_eq!(
            first.cleanup_machine == second.cleanup_machine,
            shared_target
        );
        assert_eq!(plan.functions.len(), if shared_target { 3 } else { 4 });

        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::uefi_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let target_plan = lower_to_target_operations(&plan, target).unwrap();
            let assigned = assign_registers(&target_plan).unwrap();
            let machine = emit_machine_code(&assigned).unwrap();
            let emitted = machine
                .functions
                .iter()
                .find(|function| function.machine == caller_machine)
                .unwrap();
            let emitted_cleanup = emitted.unit_affine_cleanup.as_ref().unwrap();
            assert_eq!(emitted_cleanup.actions, cleanup_actions);
            assert_eq!(emitted.internal_unit_calls.len(), 2);
            assert_eq!(emitted.internal_calls.len(), 2);
            for (ordinal, (call, cleanup)) in emitted
                .internal_unit_calls
                .iter()
                .zip([first, second])
                .enumerate()
            {
                let expected_owner = CallSiteOwner::CleanupAction {
                    edge: emitted_cleanup.psi_edge,
                    action_ordinal: u32::try_from(ordinal).unwrap(),
                };
                assert_eq!(call.owner, expected_owner);
                assert_eq!(call.target, cleanup.cleanup_machine);
                assert!(call.arguments.is_empty());
                assert!(call.claim_transfers.is_empty());
                assert_eq!(emitted.internal_calls[ordinal].owner, expected_owner);
                assert_eq!(
                    emitted.internal_calls[ordinal].target,
                    cleanup.cleanup_machine
                );
            }
            assert!(
                emitted.internal_unit_calls[0].code_offset
                    + emitted.internal_unit_calls[0].byte_count
                    <= emitted.internal_unit_calls[1].code_offset
            );

            let mut swapped_owners = machine.clone();
            let forged_caller = swapped_owners
                .functions
                .iter_mut()
                .find(|function| function.machine == caller_machine)
                .unwrap();
            forged_caller.internal_calls[0].owner = CallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: 1,
            };
            forged_caller.internal_calls[1].owner = CallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: 0,
            };
            forged_caller.internal_unit_calls[0].owner = CallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: 1,
            };
            forged_caller.internal_unit_calls[1].owner = CallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: 0,
            };
            assert!(build_object_artifact(&swapped_owners).is_err());

            let object = build_object_artifact(&machine).unwrap();
            let image = emit_executable_image(&object, 3).unwrap();
            let installation =
                build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
            let installed_calls = installation
                .internal_unit_calls()
                .iter()
                .filter(|call| call.machine == caller_machine)
                .collect::<Vec<_>>();
            assert_eq!(installed_calls.len(), 2);
            for (ordinal, (call, cleanup)) in
                installed_calls.iter().zip([first, second]).enumerate()
            {
                assert_eq!(
                    call.custody.owner,
                    CallSiteOwner::CleanupAction {
                        edge: emitted_cleanup.psi_edge,
                        action_ordinal: u32::try_from(ordinal).unwrap(),
                    }
                );
                assert_eq!(call.custody.target, cleanup.cleanup_machine);
            }
            assert_eq!(
                installation.internal_unit_calls().len(),
                if shared_target { 3 } else { 4 }
            );
            validate_installation_record(&installation, &image).unwrap();
            let bytes = encode_installation_record(&installation).unwrap();
            assert_eq!(decode_installation_record(&bytes), Ok(installation));
        }
    }
}

#[test]
fn three_shared_executable_cleanup_actions_retain_exact_order_on_all_targets() {
    let plan = two_nominal_one_executable_plan(THREE_SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE);
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .unwrap();
    let cleanup_targets = cleanup_actions
        .iter()
        .map(|action| match action {
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                (cleanup.place, cleanup.cleanup_machine)
            }
            _ => panic!("all three actions remain nominal"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cleanup_targets
            .iter()
            .map(|(place, _)| *place)
            .collect::<Vec<_>>(),
        caller
            .structural_parameters
            .iter()
            .rev()
            .map(|parameter| parameter.place)
            .collect::<Vec<_>>()
    );
    assert!(cleanup_targets
        .windows(2)
        .all(|pair| pair[0].1 == pair[1].1));
    assert_eq!(plan.functions.len(), 3);

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            emitted.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );
        assert_eq!(emitted.internal_unit_calls.len(), 3);
        for (ordinal, call) in emitted.internal_unit_calls.iter().enumerate() {
            assert_eq!(
                call.owner,
                CallSiteOwner::CleanupAction {
                    edge: emitted.unit_affine_cleanup.as_ref().unwrap().psi_edge,
                    action_ordinal: u32::try_from(ordinal).unwrap(),
                }
            );
            assert_eq!(call.target, cleanup_targets[ordinal].1);
        }
        assert!(emitted
            .internal_unit_calls
            .windows(2)
            .all(|pair| { pair[0].code_offset + pair[0].byte_count <= pair[1].code_offset }));

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        assert_eq!(installation.internal_unit_calls().len(), 4);
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}

#[test]
fn finite_cleanup_lists_and_helper_bodies_retain_exact_order_on_all_targets() {
    let plan = two_nominal_one_executable_plan(FIVE_SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE);
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(cleanup_actions.len(), 5);
    let TerminalAffineCleanupAction::InvokeNominal(first_cleanup) = &cleanup_actions[0] else {
        unreachable!()
    };
    let cleanup_function = plan
        .functions
        .iter()
        .find(|function| function.machine == first_cleanup.cleanup_machine)
        .unwrap();
    assert_eq!(cleanup_function.operations.len(), 6);
    assert_eq!(plan.functions.len(), 7);

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(emitted.internal_unit_calls.len(), 5);
        for (ordinal, call) in emitted.internal_unit_calls.iter().enumerate() {
            assert_eq!(
                call.owner,
                CallSiteOwner::CleanupAction {
                    edge: emitted.unit_affine_cleanup.as_ref().unwrap().psi_edge,
                    action_ordinal: u32::try_from(ordinal).unwrap(),
                }
            );
            assert_eq!(call.target, first_cleanup.cleanup_machine);
        }
        let drop = machine
            .functions
            .iter()
            .find(|function| function.machine == first_cleanup.cleanup_machine)
            .unwrap();
        assert_eq!(drop.internal_unit_calls.len(), 5);
        assert!(drop
            .internal_unit_calls
            .iter()
            .enumerate()
            .all(|(ordinal, call)| {
                call.operation_ordinal == ordinal
                    && matches!(call.owner, CallSiteOwner::Operation(_))
            }));

        let object = build_object_artifact(&machine).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        assert_eq!(installation.internal_unit_calls().len(), 10);
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}

#[test]
fn wide_flat_nominal_affine_cleanup_executes_and_is_installed_on_all_targets() {
    let plan = nominal_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let cleanup = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => match cleanup_actions.as_slice() {
                [TerminalAffineCleanupAction::InvokeNominal(cleanup)] => Some(cleanup.clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("entry return retains exact nominal cleanup");
    assert_eq!(caller.structural_parameters.len(), 1);
    assert_eq!(cleanup.place, caller.structural_parameters[0].place);
    assert_eq!(
        cleanup.structural_type,
        caller.structural_parameters[0].structural_type
    );
    let cleanup_function = plan
        .functions
        .iter()
        .find(|function| function.machine == cleanup.cleanup_machine)
        .expect("cleanup closure remains in the Omega plan");
    assert_eq!(cleanup_function.attachment, Some(cleanup.structural_type));
    let helper_calls = cleanup_function
        .operations
        .iter()
        .filter_map(|operation| match operation {
            omega_abstract_operations::AbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                claim_transfers,
            } => {
                assert!(structural_arguments.is_empty());
                assert!(claim_transfers.is_empty());
                Some((*psi_operation, *callee))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(helper_calls.len(), 3);
    assert_ne!(helper_calls[0].0, helper_calls[1].0);
    assert_ne!(helper_calls[0].1, helper_calls[1].1);
    assert_ne!(helper_calls[1].0, helper_calls[2].0);
    assert_ne!(helper_calls[1].1, helper_calls[2].1);

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let target_caller = target_plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let TargetOperation::UnitBody(target_body) = &target_caller.operation else {
            panic!("caller remains Unit")
        };
        assert_eq!(target_body.parameters[0].shape, ValueShape::integer(40, 8));
        assert!(!target_body.parameters[0].placement.locations.is_empty());
        let omega_target_operations::TargetUnitOperation::Return {
            cleanup_actions, ..
        } = target_body.operations.last().unwrap()
        else {
            panic!("caller ends in a Unit return")
        };
        assert_eq!(
            cleanup_actions,
            &[TerminalAffineCleanupAction::InvokeNominal(cleanup.clone())]
        );

        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let emitted_cleanup = emitted.unit_affine_cleanup.as_ref().unwrap();
        assert!(emitted_cleanup.locals.is_empty());
        assert_eq!(
            emitted_cleanup.actions,
            [TerminalAffineCleanupAction::InvokeNominal(cleanup.clone())]
        );
        let cleanup_call = emitted
            .internal_unit_calls
            .iter()
            .find(|call| {
                call.owner
                    == CallSiteOwner::CleanupAction {
                        edge: emitted_cleanup.psi_edge,
                        action_ordinal: 0,
                    }
            })
            .expect("cleanup edge owns one native Unit call");
        assert_eq!(cleanup_call.target, cleanup.cleanup_machine);
        assert!(cleanup_call.arguments.is_empty());
        assert!(cleanup_call.claim_transfers.is_empty());
        let relocation = emitted
            .internal_calls
            .iter()
            .find(|call| call.owner == cleanup_call.owner)
            .expect("cleanup call retains a relocation");
        assert_eq!(relocation.target, cleanup.cleanup_machine);
        assert!(relocation.unit_stack.is_some());
        assert!(emitted_cleanup.code_offset <= cleanup_call.code_offset);
        assert!(
            cleanup_call.code_offset + cleanup_call.byte_count
                <= emitted_cleanup.code_offset + emitted_cleanup.byte_count
        );
        assert_eq!(
            machine
                .functions
                .iter()
                .find(|function| function.machine == cleanup.cleanup_machine)
                .unwrap()
                .attachment,
            Some(cleanup.structural_type)
        );
        let emitted_drop = machine
            .functions
            .iter()
            .find(|function| function.machine == cleanup.cleanup_machine)
            .unwrap();
        assert_eq!(
            emitted_drop
                .internal_unit_calls
                .iter()
                .map(|call| (call.owner, call.target))
                .collect::<Vec<_>>(),
            helper_calls
                .iter()
                .map(|(operation, target)| { (CallSiteOwner::Operation(*operation), *target) })
                .collect::<Vec<_>>(),
            "drop helper calls retain source order"
        );
        for (ordinal, call) in emitted_drop.internal_unit_calls.iter().enumerate() {
            assert_eq!(call.operation_ordinal, ordinal);
        }
        assert!(emitted_drop
            .internal_unit_calls
            .windows(2)
            .all(|pair| { pair[0].code_offset + pair[0].byte_count <= pair[1].code_offset }));

        let mut forged_helper_order = machine.clone();
        let forged_drop = forged_helper_order
            .functions
            .iter_mut()
            .find(|function| function.machine == cleanup.cleanup_machine)
            .unwrap();
        let first_owner = forged_drop.internal_calls[0].owner;
        forged_drop.internal_calls[0].owner = forged_drop.internal_calls[2].owner;
        forged_drop.internal_calls[2].owner = first_owner;
        let first_owner = forged_drop.internal_unit_calls[0].owner;
        forged_drop.internal_unit_calls[0].owner = forged_drop.internal_unit_calls[2].owner;
        forged_drop.internal_unit_calls[2].owner = first_owner;
        assert!(build_object_artifact(&forged_helper_order).is_err());

        let mut forged_place = machine.clone();
        forged_place
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions[0] =
            TerminalAffineCleanupAction::InvokeNominal(psi_terminal::NominalAffineCleanup {
                place: psi_core::PlaceId::new(cleanup.place.get() + 1).unwrap(),
                ..cleanup.clone()
            });
        assert!(build_object_artifact(&forged_place).is_err());
        let mut forged_target = machine.clone();
        forged_target
            .functions
            .iter_mut()
            .find(|function| function.machine == cleanup.cleanup_machine)
            .unwrap()
            .attachment = None;
        assert!(build_object_artifact(&forged_target).is_err());

        let object = build_object_artifact(&machine).unwrap();
        let expected_stack_bytes = match target {
            target
                if target == NativeTarget::windows_x64() || target == NativeTarget::uefi_x64() =>
            {
                112
            }
            target if target == NativeTarget::linux_x64() => 80,
            _ => 48,
        };
        assert_eq!(
            derive_stack_demand(&object, caller_machine)
                .expect("executable cleanup stack closure")
                .ceiling_bytes(),
            expected_stack_bytes,
            "the wide receiver and both nested calls compose exactly for {target:?}"
        );
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        assert_eq!(installation.internal_unit_calls().len(), 4);
        let installed = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            installed.unit_affine_cleanup.as_ref().unwrap().actions,
            [TerminalAffineCleanupAction::InvokeNominal(cleanup.clone())]
        );
        assert_eq!(
            installation
                .functions()
                .iter()
                .find(|function| function.machine == cleanup.cleanup_machine)
                .unwrap()
                .attachment,
            Some(cleanup.structural_type)
        );
        validate_installation_record(&installation, &image).unwrap();
        let bytes = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&bytes), Ok(installation.clone()));
        if target == NativeTarget::linux_x64() {
            let native_cleanup = installed.unit_affine_cleanup.as_ref().unwrap();
            let mut encoded_cleanup = vec![3, 0, 0, 0];
            encoded_cleanup.extend_from_slice(&cleanup.place.get().to_le_bytes());
            encoded_cleanup.extend_from_slice(&cleanup.structural_type.get().to_le_bytes());
            encoded_cleanup.extend_from_slice(&cleanup.cleanup_machine.get().to_le_bytes());
            encoded_cleanup.extend_from_slice(&(native_cleanup.code_offset as u64).to_le_bytes());
            encoded_cleanup.extend_from_slice(&(native_cleanup.byte_count as u64).to_le_bytes());
            let matches = bytes
                .windows(encoded_cleanup.len())
                .enumerate()
                .filter(|(_, window)| *window == encoded_cleanup)
                .map(|(offset, _)| offset)
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "nominal cleanup encoding is unique");
            let offset = matches[0];
            let mut invalid_presence = bytes.clone();
            invalid_presence[offset] = 4;
            assert_eq!(
                decode_installation_record(&invalid_presence),
                Err(InstallationError::InvalidCleanupActionTag(4))
            );
            let mut zero_place = bytes.clone();
            zero_place[offset + 4..offset + 12].fill(0);
            assert_eq!(
                decode_installation_record(&zero_place),
                Err(InstallationError::ZeroStructuralReturnIdentity(
                    "nominal Unit cleanup place"
                ))
            );
        }
    }
}
