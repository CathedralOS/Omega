//! Initializer normalization tests.

use super::{ConstArgumentOrigin, SourceSpan, finalize_operator_obligations};
use crate::constant::initializer_normalization::Exposure;
use crate::constant::initializer_normalization::Intrinsic;
use crate::constant::initializer_normalization::Kind;
use crate::constant::initializer_normalization::LateBinding;
use crate::constant::initializer_normalization::PendingInitializer;
use crate::constant::initializer_normalization::Target;
use crate::constant::initializer_normalization::append_retained_custody;
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::ConstDefinition;
use syntax_trees::{
    expression::ExpressionNode,
    item::{ConstInitializerNormalization, Item},
};

#[test]
fn normalized_calls_rejoin_literal_and_computed_helper_dependencies_before_table_rebuild() {
    for base_value in ["7", "7 / 2 * 2"] {
        let text = format!(
            "const BASE: u64 = {base_value}; machine size() -> u64 {{ BASE }} const SIZE: u64 = size();"
        );
        let mut sources = source::SourceMap::default();
        let source = sources
            .add(
                std::path::PathBuf::from("helper-dependency.omg"),
                text.clone(),
            )
            .source_id;
        let sources = std::sync::Arc::new(sources);
        let tokens = source_files_to_tokens::Lexer::new(&text)
            .tokenize()
            .expect("tokens");
        let mut syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).expect("syntax");
        let definitions = syntax
            .root_item_handles()
            .iter()
            .filter_map(|item| match syntax.root_item(*item) {
                Item::Const(definition) => Some((*item, definition.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        let preparation =
            crate::resolution::prepare_const_initializer_selection(crate::ResolutionRequest {
                syntax: &syntax,
                sources: Some(sources.clone()),
                top_level_bindings: Vec::new(),
            })
            .expect("preparation");
        let dependencies = preparation
            .initializer_dependencies(&syntax, &definitions[1].1)
            .expect("helper dependencies");
        assert_eq!(dependencies.constants.len(), 1);
        let encoding =
            language_semantics::const_value::CanonicalConstIdentity::integer("u64", 7).encoding;
        let mut operators = Vec::new();
        let mut pending = vec![definitions[0].1.value];
        while let Some(expression) = pending.pop() {
            if let ExpressionNode::Binary(binary) = syntax.expressions.expression(expression) {
                operators.push(syntax.expressions.source_span(expression));
                pending.extend([binary.left, binary.right]);
            }
        }
        for (ordinal, (item, definition)) in definitions.iter().enumerate() {
            if ordinal == 0 && operators.is_empty() {
                continue;
            }
            let mut definition = definition.clone();
            let original = definition.value;
            let materialized = syntax.expressions.insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(7),
            ));
            let original_source = syntax.expressions.source_span(original);
            syntax
                .expressions
                .set_source_span(materialized, original_source);
            definition.value = materialized;
            definition.normalization = Some(ConstInitializerNormalization {
                authored_expression: original,
                canonical_result_encoding: encoding.clone(),
                selections: if ordinal == 0 {
                    Vec::new()
                } else {
                    vec![ConstArgumentOrigin {
                        reference: dependencies.constants[0].0,
                        declaration: definitions[0].1.name.source_span(),
                        initializer: syntax.expressions.source_span(definitions[0].1.value),
                        canonical_value_encoding: encoding.clone(),
                    }]
                },
                builtin_operators: operators.clone(),
                call_selections: if ordinal == 0 {
                    Vec::new()
                } else {
                    dependencies.calls.clone()
                },
            });
            syntax.items.replace_item(*item, Item::Const(definition));
        }
        crate::resolve(crate::ResolutionRequest {
            syntax: &syntax,
            sources: Some(sources.clone()),
            top_level_bindings: Vec::new(),
        })
        .expect("complete resolution must visit authoritative helper statements");
        let mut changed = syntax.clone();
        let Item::Const(mut declaration) = changed.root_item(definitions[1].0).clone() else {
            panic!("SIZE")
        };
        declaration
            .normalization
            .as_mut()
            .expect("receipt")
            .selections
            .clear();
        changed
            .items
            .replace_item(definitions[1].0, Item::Const(declaration));
        assert!(
            crate::resolve(crate::ResolutionRequest {
                syntax: &changed,
                sources: Some(sources),
                top_level_bindings: Vec::new()
            })
            .is_err(),
            "omitted helper dependency must not validate"
        );
    }
}

fn normalized() -> (SyntaxTrees, syntax_trees::item::ItemHandle) {
    normalized_expression(false)
}

fn normalized_expression(unary: bool) -> (SyntaxTrees, syntax_trees::item::ItemHandle) {
    let text = if unary {
        "const BASE: bool = false; pub const COUNT: bool = !BASE; machine main() -> bool { COUNT }"
    } else {
        "const BASE: u64 = 2; pub const COUNT: u64 = BASE + 1; machine main() -> u64 { COUNT }"
    };
    let tokens = source_files_to_tokens::Lexer::new(text)
        .tokenize()
        .expect("tokenize initializer receipt");
    let mut syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse initializer receipt");
    let definitions = syntax
        .root_item_handles()
        .iter()
        .filter_map(|handle| match syntax.root_item(*handle) {
            Item::Const(definition) => Some((*handle, definition.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let (_, base) = &definitions[0];
    let (item, mut definition) = definitions[1].clone();
    let original = definition.value;
    let dependency = match syntax.expressions.expression(original) {
        ExpressionNode::Binary(binary) => binary.left,
        ExpressionNode::Unary(unary) => unary.operand,
        _ => panic!("operator initializer"),
    };
    let ExpressionNode::Name(path) = syntax.expressions.expression(dependency) else {
        panic!("named dependency")
    };
    let members = syntax.expressions.identifier_path_members(*path);
    let reference = members[0].source_span();
    let encoding =
        crate::preparation::generic_data::canonicalize_declared_const_definition(&syntax, base)
            .expect("canonical BASE")
            .encoding;
    let value = syntax.expressions.insert(if unary {
        ExpressionNode::Boolean(true)
    } else {
        ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(3))
    });
    let original_span = syntax.expressions.source_span(original);
    syntax.expressions.set_source_span(value, original_span);
    definition.value = value;
    definition.normalization = Some(ConstInitializerNormalization {
        authored_expression: original,
        canonical_result_encoding:
            crate::preparation::generic_data::canonicalize_declared_const_definition(
                &syntax,
                &definition,
            )
            .expect("canonical COUNT")
            .encoding,
        selections: vec![ConstArgumentOrigin {
            reference,
            declaration: base.name.source_span(),
            initializer: syntax.expressions.source_span(base.value),
            canonical_value_encoding: encoding,
        }],
        builtin_operators: vec![syntax.expressions.source_span(original)],
        call_selections: Vec::new(),
    });
    syntax.items.replace_item(item, Item::Const(definition));
    (syntax, item)
}

#[test]
fn initializer_receipt_retains_dependency_and_operator_on_materialized_root() {
    let (syntax, _) = normalized();
    let resolved = crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("resolve retained initializer receipt");
    let count = resolved
        .const_declarations
        .iter()
        .find(|declaration| resolved.symbols.name(declaration.symbol) == "COUNT")
        .expect("COUNT declaration");
    let selections = resolved
        .tables
        .bodies
        .expressions
        .authored_selection_occurrences(count.initializer)
        .map(|occurrence| {
            resolved
                .authored_declaration_selections()
                .get(occurrence)
                .expect("retained occurrence")
        })
        .collect::<Vec<_>>();
    assert!(
        selections
            .iter()
            .any(|selection| selection.kind() == Kind::Operator)
    );
    assert!(
        selections
            .iter()
            .any(|selection| selection.kind() == Kind::StaticPathSegment)
    );
}

#[test]
fn initializer_receipt_rejects_changed_or_missing_custody() {
    for mutation in 0..6 {
        let (mut syntax, item) = normalized();
        let Item::Const(mut definition) = syntax.root_item(item).clone() else {
            panic!("constant")
        };
        let receipt = definition.normalization.as_mut().expect("receipt");
        match mutation {
            0 => receipt.authored_expression = Default::default(),
            1 => receipt.selections.clear(),
            2 => receipt.builtin_operators.clear(),
            3 => receipt.canonical_result_encoding.push('0'),
            4 => receipt.selections[0].canonical_value_encoding.push('0'),
            5 => receipt.selections[0].initializer = Default::default(),
            _ => unreachable!(),
        }
        syntax.items.replace_item(item, Item::Const(definition));
        assert!(
            crate::resolve(crate::ResolutionRequest::new(&syntax)).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn unary_initializer_receipt_rejoins_operand_and_operator_custody() {
    let (syntax, item) = normalized_expression(true);
    crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("a normalized unary initializer retains its dependency and operator");
    for mutation in 0..3 {
        let mut changed = syntax.clone();
        let Item::Const(mut definition) = changed.root_item(item).clone() else {
            panic!("constant")
        };
        let receipt = definition.normalization.as_mut().expect("receipt");
        match mutation {
            0 => receipt.selections.clear(),
            1 => receipt.builtin_operators.clear(),
            2 => receipt.selections[0].canonical_value_encoding.push('0'),
            _ => unreachable!(),
        }
        changed.items.replace_item(item, Item::Const(definition));
        assert!(
            crate::resolve(crate::ResolutionRequest::new(&changed)).is_err(),
            "unary receipt mutation {mutation}"
        );
    }
}

#[test]
fn retained_initializer_custody_rejects_unchecked_operators_and_cycles() {
    for mutation in 0..3 {
        let (syntax, _) = normalized();
        let mut program =
            crate::resolve(crate::ResolutionRequest::new(&syntax)).expect("retained base");
        let owner = program
            .const_declarations
            .iter()
            .find(|declaration| program.symbols.name(declaration.symbol) == "COUNT")
            .expect("COUNT")
            .clone();
        let reference = owner.initializer_source_span;
        let occurrence = match mutation {
            0 => program.record_late_bound_authored_declaration_selection(
                reference,
                Exposure::PrivateImplementation,
                Kind::Operator,
                LateBinding::CheckedOperator,
            ),
            1 => program.record_resolved_authored_declaration_selection(
                reference,
                Exposure::PrivateImplementation,
                Kind::Operator,
                owner.symbol,
            ),
            _ => program.record_resolved_authored_declaration_selection(
                reference,
                Exposure::PrivateImplementation,
                Kind::StaticPathSegment,
                owner.symbol,
            ),
        }
        .expect("adversarial retained occurrence");
        program
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(owner.initializer, [occurrence]);
        assert!(
            append_retained_custody(
                &program,
                &owner,
                SourceSpan::default(),
                &mut Vec::new(),
                &mut Vec::new()
            )
            .is_err(),
            "mutation {mutation} must not become checked retained evidence"
        );
    }
}

#[test]
fn initializer_operator_finalization_preserves_unrelated_partitions_and_exposure() {
    use language_semantics::declaration_selection::CompilerDerivedSelectionPartition;
    let (syntax, item) = normalized();
    let Item::Const(definition) = syntax.root_item(item) else {
        panic!("constant")
    };
    let receipt = definition.normalization.clone().expect("receipt");
    let operator = receipt.builtin_operators[0];
    let mut program =
        crate::resolve(crate::ResolutionRequest::new(&syntax)).expect("retained base");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.name(declaration.symbol) == "COUNT")
        .expect("COUNT");
    let materialized = declaration.initializer;
    let authored = declaration.authored_initializer;
    let record = PendingInitializer {
        declaration: definition.name.source_span(),
        materialized,
        authored,
        references: Vec::new(),
        operators: vec![operator],
        receipt,
    };
    let mut occurrences = Vec::new();
    for (exposure, partition, reference) in [
        (Exposure::PrivateImplementation, None, operator),
        (
            Exposure::PrivateImplementation,
            Some(CompilerDerivedSelectionPartition::from_compiler_ordinal(1)),
            operator,
        ),
        (Exposure::PublicInterface, None, operator),
        (Exposure::PrivateImplementation, None, SourceSpan::default()),
    ] {
        occurrences.push(
            program
                .record_late_bound_authored_declaration_selection_in_partition(
                    reference,
                    exposure,
                    Kind::Operator,
                    partition,
                    LateBinding::CheckedOperator,
                )
                .expect("operator occurrence"),
        );
    }
    finalize_operator_obligations(&mut program, &[record]).expect("exact receipt finalization");
    for (ordinal, occurrence) in occurrences.into_iter().enumerate() {
        let target = program
            .authored_declaration_selections()
            .get(occurrence)
            .expect("selection")
            .target();
        if ordinal == 0 {
            assert!(matches!(
                target,
                Target::Intrinsic(Intrinsic::BuiltinOperator)
            ));
        } else {
            assert!(matches!(
                target,
                Target::LateBound(LateBinding::CheckedOperator)
            ));
        }
    }
}

#[test]
fn seeded_initializer_rejoins_retained_transitive_custody() {
    use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
    use std::{path::PathBuf, sync::Arc};

    let base_text = "const SEED: u64 = 1; pub const BASE: u64 = SEED + 1;";
    let extension_text = "const NEXT: u64 = BASE + 1;";
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base_text.into())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("extension.omg"),
            extension_text.into(),
            PathBuf::from(""),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let parse = |source_id, text: &str| {
        tokens_to_syntax_trees::parse_syntax_trees_with_id(
            source_id,
            &source_files_to_tokens::Lexer::new(text)
                .tokenize()
                .expect("tokens"),
        )
        .expect("syntax")
    };
    let mut base_syntax = parse(base_id, base_text);
    let mut extension_syntax = parse(extension_id, extension_text);
    let definition = |syntax: &SyntaxTrees, name: &str| {
        syntax
            .root_items()
            .find_map(|item| match item {
                Item::Const(definition) if definition.name.as_str() == name => {
                    Some(definition.clone())
                }
                _ => None,
            })
            .expect("declaration")
    };
    let normalize = |syntax: &mut SyntaxTrees,
                     name: &str,
                     value: i64,
                     dependency: &ConstDefinition,
                     dependency_encoding: String,
                     dependency_initializer: SourceSpan,
                     inherited: Option<&ConstInitializerNormalization>| {
        let item = *syntax.root_item_handles().iter().find(|item| {
            matches!(syntax.root_item(**item), Item::Const(definition) if definition.name.as_str() == name)
        }).expect("item");
        let Item::Const(mut definition) = syntax.root_item(item).clone() else {
            panic!("constant")
        };
        let original = definition.value;
        let ExpressionNode::Binary(binary) = syntax.expressions.expression(original) else {
            panic!("binary")
        };
        let ExpressionNode::Name(path) = syntax.expressions.expression(binary.left) else {
            panic!("dependency")
        };
        let reference = syntax.expressions.identifier_path_members(*path)[0].source_span();
        let mut selections = vec![ConstArgumentOrigin {
            reference,
            declaration: dependency.name.source_span(),
            initializer: dependency_initializer,
            canonical_value_encoding: dependency_encoding,
        }];
        let mut operators = vec![syntax.expressions.source_span(original)];
        if let Some(inherited) = inherited {
            selections.extend(inherited.selections.iter().cloned());
            operators.extend(inherited.builtin_operators.iter().copied());
        }
        let materialized = syntax.expressions.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(value),
        ));
        let original_span = syntax.expressions.source_span(original);
        syntax
            .expressions
            .set_source_span(materialized, original_span);
        definition.value = materialized;
        definition.normalization = Some(ConstInitializerNormalization {
            authored_expression: original,
            canonical_result_encoding:
                language_semantics::const_value::CanonicalConstIdentity::integer(
                    "u64",
                    i128::from(value),
                )
                .encoding,
            selections,
            builtin_operators: operators,
            call_selections: Vec::new(),
        });
        syntax.items.replace_item(item, Item::Const(definition));
    };
    let seed = definition(&base_syntax, "SEED");
    let seed_initializer = base_syntax.expressions.source_span(seed.value);
    normalize(
        &mut base_syntax,
        "BASE",
        2,
        &seed,
        language_semantics::const_value::CanonicalConstIdentity::integer("u64", 1).encoding,
        seed_initializer,
        None,
    );
    let base_definition = definition(&base_syntax, "BASE");
    normalize(
        &mut extension_syntax,
        "NEXT",
        3,
        &base_definition,
        language_semantics::const_value::CanonicalConstIdentity::integer("u64", 2).encoding,
        base_syntax.expressions.source_span(base_definition.value),
        base_definition.normalization.as_ref(),
    );

    let mut combined = base_syntax.clone();
    combined.extend_from(&extension_syntax);
    let sources = Arc::new(sources);
    let one_shot = crate::resolve(crate::ResolutionRequest {
        syntax: &combined,
        sources: Some(sources.clone()),
        top_level_bindings: Vec::new(),
    })
    .expect("one-shot normalized declarations");
    let base = crate::resolve(crate::ResolutionRequest {
        syntax: &base_syntax,
        sources: Some(sources.clone()),
        top_level_bindings: Vec::new(),
    })
    .expect("normalized retained base");
    let retained_selections = base.authored_declaration_selections().as_slice().to_vec();
    let seeded = crate::resolve_extension(crate::ExtensionRequest {
        base,
        syntax: &extension_syntax,
        sources,
        top_level_bindings: Vec::new(),
    })
    .map(|seeded| seeded.into_unrebased_trees())
    .expect("seeded initializer retains transitive dependencies and operators");
    assert_eq!(
        &seeded.authored_declaration_selections().as_slice()[..retained_selections.len()],
        retained_selections
    );
    let custody = |program: &SymbolResolvedTrees| {
        let next = program
            .const_declarations
            .iter()
            .find(|declaration| program.symbols.name(declaration.symbol) == "NEXT")
            .expect("NEXT");
        program.tables.bodies.expressions.authored_selection_occurrences(next.initializer)
            .map(|occurrence| {
                let selection = program.authored_declaration_selections().get(occurrence).expect("selection");
                let declaration = match selection.target() {
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target) =>
                        program.symbols.symbol_source_span(target.selected_symbol()),
                    _ => None,
                };
                (selection.source_span(), selection.kind(), declaration)
            }).collect::<Vec<_>>()
    };
    let expected = custody(&one_shot);
    let actual = custody(&seeded);
    assert_eq!(actual.len(), expected.len());
    assert!(expected.iter().all(|selection| actual.contains(selection)));
}
