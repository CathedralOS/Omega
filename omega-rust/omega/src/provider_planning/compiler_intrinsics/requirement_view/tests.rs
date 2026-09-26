use super::{IntrinsicRequirement, IntrinsicRequirementKind};
use language_core::operator_spelling::OperatorSpelling;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;

fn typed_requirement() -> TypedTrees {
    let source = "data Float {} pub boundary requirement Float::add(left: f32, right: f32) -> f32;";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize boundary requirement");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse boundary requirement");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve boundary requirement");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type boundary requirement")
}

#[test]
fn token_signature_preserves_machine_requirement_identity_and_intrinsic_meaning() {
    let mut typed = typed_requirement();
    let requirement = typed.machines()[0].clone();
    let entry = typed.machine_states(&requirement)[0].clone();
    let parameters = typed.state_parameters(&entry).to_vec();
    let mut token =
        symbol_resolved_trees_to_typed_trees::typed_trees::operator::OperatorDefinition {
            is_public: requirement.is_public,
            is_boundary: true,
            symbol: requirement.symbol,
            return_type: entry.return_type,
            spelling: Some(OperatorSpelling::Add),
            ..Default::default()
        };
    typed.push_operator_path_member(&mut token, "Float".into());
    typed.push_operator_path_member(&mut token, "add".into());
    for parameter in parameters {
        typed.push_operator_parameter(&mut token, parameter);
    }
    typed.push_machine_token_binding(token);
    let token = &typed.machine_token_bindings()[0];
    let by_declaration = IntrinsicRequirement::by_symbol(&typed, requirement.symbol)
        .expect("declaration requirement view");
    let by_entry =
        IntrinsicRequirement::by_symbol(&typed, entry.symbol).expect("entry requirement view");
    let by_token =
        IntrinsicRequirement::from_operator(&typed, token).expect("token requirement view");
    assert_eq!(by_declaration, by_entry);
    assert_eq!(by_declaration, by_token);
    assert_eq!(by_token.kind, IntrinsicRequirementKind::TopLevelRequirement);
    assert_eq!(by_token.call_target, entry.symbol);
    assert_eq!(
        by_token.requirement_identity,
        typed
            .normalized_machine_overload_identity(&requirement)
            .expect("machine overload")
            .identity()
    );
    assert_eq!(by_token.as_operator(), Some(token));
    assert_eq!(
        crate::primitive_float_binary_intrinsic_execution_identity_for(&typed, &by_token),
        Some(
            crate::CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
                operation: crate::CompilerPrimitiveFloatBinaryOperation::Add,
                format: numerics::literals::FloatFormat::F32,
            }
        )
    );
}

#[test]
fn an_untokened_requirement_does_not_gain_primitive_operator_meaning() {
    let typed = typed_requirement();
    let requirement = IntrinsicRequirement::from_requirement(&typed, &typed.machines()[0])
        .expect("ordinary requirement view");
    assert_eq!(
        requirement.kind,
        IntrinsicRequirementKind::TopLevelRequirement
    );
    assert!(requirement.as_operator().is_none());
    assert!(
        crate::primitive_float_binary_intrinsic_execution_identity_for(&typed, &requirement)
            .is_none()
    );
}
