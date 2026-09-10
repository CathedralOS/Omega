//! Rejoin declaration-owned folds before any use copies their materialized value.
//!
//! The semantic evaluator establishes the result and builtin meanings. Resolution
//! independently checks the retained scalar payload, source roots, and complete
//! selected dependency/operator roster. Original roots still undergo ordinary
//! declaration-source resolution; initializer implementation exposure never
//! becomes public index exposure just because its constant is public.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure as Exposure,
    AuthoredDeclarationSelectionIntrinsic as Intrinsic, AuthoredDeclarationSelectionKind as Kind,
    AuthoredDeclarationSelectionLateBinding as LateBinding,
    AuthoredDeclarationSelectionTarget as Target,
};
use source::SourceSpan;
use symbol_resolved_trees::{SymbolResolvedTrees, expression::ExpressionHandle};
use symbols::SymbolKind;
use syntax_trees::{SyntaxTrees, item::ConstDefinition, types::ConstArgumentOrigin};

pub(crate) struct PendingInitializer {
    declaration: SourceSpan,
    materialized: ExpressionHandle,
    references: Vec<(SourceSpan, String)>,
    operators: Vec<SourceSpan>,
    receipt: syntax_trees::item::ConstInitializerNormalization,
}

fn error(reference: SourceSpan, reason: &str) -> Diagnostic {
    Diagnostic::error(format!("constant initializer normalization: {reason}"))
        .with_source_span(reference)
}

pub(crate) fn retain(
    lowerer: &mut crate::lowerer::Lowerer,
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    materialized: ExpressionHandle,
) -> Result<(), Diagnostic> {
    let Some(receipt) = &definition.normalization else {
        return Ok(());
    };
    let reference = syntax.expressions.source_span(definition.value);
    if !syntax
        .expressions
        .contains_expression(receipt.authored_expression)
        || receipt.authored_expression == definition.value
        || syntax.expressions.source_span(receipt.authored_expression) != reference
    {
        return Err(error(reference, "lost its exact authored expression"));
    }
    let value = crate::generic_data::canonicalize_selected_declared_const_definition(
        syntax,
        definition,
        lowerer.constant_selection.as_ref(),
    )
    .map_err(|_| error(reference, "materialized value is not canonical"))?;
    if value.encoding != receipt.canonical_result_encoding {
        return Err(error(
            reference,
            "materialized value drifted from its canonical result",
        ));
    }
    let mut references = Vec::new();
    let mut operators = Vec::new();
    let mut pending = vec![receipt.authored_expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !syntax.expressions.contains_expression(expression) {
            return Err(error(reference, "authored expression has an invalid child"));
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        use syntax_trees::expression::{ExpressionNode, MatchPattern};
        match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_) => {}
            ExpressionNode::Name(path) => {
                let members = syntax.expressions.identifier_path_members(*path);
                let (Some(first), Some(last)) = (members.first(), members.last()) else {
                    return Err(error(reference, "authored dependency lost its name"));
                };
                let reference = SourceSpan::new(
                    first.source_span().source_id,
                    source::Span::new(first.source_span().span.start, last.source_span().span.end),
                );
                references.push((
                    reference,
                    members
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::"),
                ));
            }
            ExpressionNode::Binary(binary) => {
                operators.push(syntax.expressions.source_span(expression));
                pending.push(binary.right);
                pending.push(binary.left);
            }
            ExpressionNode::Match(dispatch) => {
                for arm in syntax.expressions.match_arms(dispatch.arms).iter().rev() {
                    pending.push(arm.value);
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                }
                pending.push(dispatch.subject);
            }
            _ => {
                return Err(error(
                    reference,
                    "authored expression is outside scalar evaluation",
                ));
            }
        }
    }
    let original =
        lowerer.with_authored_expression_exposure(Exposure::PrivateImplementation, |lowerer| {
            crate::expression::lower_expression_into_table(
                lowerer,
                syntax,
                receipt.authored_expression,
            )
        })?;
    lowerer.pending_const_values.push(original);
    lowerer.pending_const_initializers.push(PendingInitializer {
        declaration: definition.name.source_span(),
        materialized,
        references,
        operators,
        receipt: receipt.clone(),
    });
    Ok(())
}

/// Reconstruct the transitive roster from original references, not receipt
/// membership. Even omitted or unused dependencies must rejoin current values.
fn reconstruct(
    program: &SymbolResolvedTrees,
    records: &[PendingInitializer],
    root: usize,
) -> Result<(Vec<ConstArgumentOrigin>, Vec<SourceSpan>), Diagnostic> {
    let mut origins = Vec::new();
    let mut operators = Vec::new();
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(ordinal) = pending.pop() {
        if visited.contains(&ordinal) {
            continue;
        }
        visited.push(ordinal);
        let record = &records[ordinal];
        for operator in &record.operators {
            if !operators.contains(operator) {
                operators.push(*operator);
            }
        }
        for (reference, name) in &record.references {
            let selected = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    name,
                    &[SymbolKind::Const],
                    *reference,
                )
                .ok_or_else(|| {
                    error(*reference, "dependency no longer selects an exact constant")
                })?;
            let declaration = program
                .const_declarations
                .iter()
                .find(|declaration| declaration.symbol == selected)
                .ok_or_else(|| error(*reference, "selected dependency lost its declaration"))?;
            let source = program
                .symbols
                .symbol_source_span(selected)
                .ok_or_else(|| error(*reference, "selected dependency lost its source"))?;
            if !declaration.is_public && !program.symbols.same_source_package(*reference, source) {
                return Err(error(
                    *reference,
                    "dependency selects a private foreign declaration",
                ));
            }
            let origin = ConstArgumentOrigin {
                reference: *reference,
                declaration: source,
                initializer: declaration.initializer_source_span,
                canonical_value_encoding: declaration.canonical_value_encoding.clone().ok_or_else(
                    || error(*reference, "dependency has no completed canonical value"),
                )?,
            };
            if !origins.contains(&origin) {
                origins.push(origin);
            }
            if let Some(dependency) = records
                .iter()
                .position(|record| record.declaration == source)
            {
                if dependency == root {
                    return Err(error(*reference, "normalized initializer dependency cycle"));
                }
                pending.push(dependency);
            } else {
                append_retained_custody(
                    program,
                    declaration,
                    records[root].declaration,
                    &mut origins,
                    &mut operators,
                )?;
            }
        }
    }
    Ok((origins, operators))
}

/// A seeded base already owns the complete transitive roster on its resolved
/// initializer. Rejoin those exact selections rather than resolving its source
/// again under an extension's namespace.
fn append_retained_custody(
    program: &SymbolResolvedTrees,
    owner: &symbol_resolved_trees::constant::ConstDeclaration,
    root: SourceSpan,
    origins: &mut Vec<ConstArgumentOrigin>,
    operators: &mut Vec<SourceSpan>,
) -> Result<(), Diagnostic> {
    let owner_source = program.symbols.symbol_source_span(owner.symbol);
    for occurrence in program
        .tables
        .bodies
        .expressions
        .authored_selection_occurrences(owner.initializer)
    {
        let selection = program
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or_else(|| error(root, "retained initializer lost its exact selection"))?;
        let reference = selection.source_span();
        match (selection.kind(), selection.target()) {
            (Kind::Operator, Target::Intrinsic(Intrinsic::BuiltinOperator)) => {
                if !operators.contains(&reference) {
                    operators.push(reference);
                }
            }
            (Kind::StaticPathSegment, Target::Resolved(selected)) => {
                let declaration = program
                    .const_declarations
                    .iter()
                    .find(|declaration| declaration.symbol == selected.selected_symbol())
                    .ok_or_else(|| {
                        error(
                            reference,
                            "retained initializer dependency is not a constant",
                        )
                    })?;
                let source = program
                    .symbols
                    .symbol_source_span(declaration.symbol)
                    .ok_or_else(|| {
                        error(reference, "retained initializer dependency lost its source")
                    })?;
                if source == root || Some(source) == owner_source {
                    return Err(error(reference, "normalized initializer dependency cycle"));
                }
                if !declaration.is_public && !program.symbols.same_source_package(reference, source)
                {
                    return Err(error(
                        reference,
                        "retained initializer selects a private foreign declaration",
                    ));
                }
                let origin = ConstArgumentOrigin {
                    reference,
                    declaration: source,
                    initializer: declaration.initializer_source_span,
                    canonical_value_encoding: declaration
                        .canonical_value_encoding
                        .clone()
                        .ok_or_else(|| {
                            error(
                                reference,
                                "retained dependency has no completed canonical value",
                            )
                        })?,
                };
                if !origins.contains(&origin) {
                    origins.push(origin);
                }
            }
            _ => {
                return Err(error(
                    reference,
                    "retained initializer selection has no checked scalar meaning",
                ));
            }
        }
    }
    Ok(())
}

/// After ordinary authored selections have been recorded, close duplicate
/// declaration-side operator obligations using the receipt's evaluated meaning.
/// Instantiated partitions and unrelated source occurrences retain their own
/// selection obligations.
pub(crate) fn finalize_operator_obligations(
    program: &mut SymbolResolvedTrees,
    records: &[PendingInitializer],
) -> Result<(), Diagnostic> {
    let occurrences = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == Kind::Operator
                && selection.exposure() == Exposure::PrivateImplementation
                && selection.compiler_partition().is_none()
                && matches!(
                    selection.target(),
                    Target::LateBound(LateBinding::CheckedOperator)
                )
                && records.iter().any(|record| {
                    record
                        .receipt
                        .builtin_operators
                        .contains(&selection.source_span())
                })
        })
        .map(|selection| (selection.occurrence_id(), selection.source_span()))
        .collect::<Vec<_>>();
    for (occurrence, reference) in occurrences {
        program
            .finalize_intrinsic_authored_declaration_selection(
                occurrence,
                LateBinding::CheckedOperator,
                Intrinsic::BuiltinOperator,
            )
            .map_err(|reason| {
                error(
                    reference,
                    &format!("retained builtin selection finalization failed: {reason:?}"),
                )
            })?;
    }
    Ok(())
}

pub(crate) fn finalize(
    program: &mut SymbolResolvedTrees,
    records: &[PendingInitializer],
) -> Result<(), Diagnostic> {
    for (ordinal, record) in records.iter().enumerate() {
        let (origins, operators) = reconstruct(program, records, ordinal)?;
        if origins.len() != record.receipt.selections.len()
            || origins
                .iter()
                .any(|origin| !record.receipt.selections.contains(origin))
            || operators.len() != record.receipt.builtin_operators.len()
            || operators
                .iter()
                .any(|operator| !record.receipt.builtin_operators.contains(operator))
        {
            return Err(error(
                record.declaration,
                "authored declaration, value, or operator custody drifted",
            ));
        }
        let mut occurrences = Vec::new();
        for origin in origins {
            let selected = program
                .const_declarations
                .iter()
                .find(|declaration| {
                    program.symbols.symbol_source_span(declaration.symbol)
                        == Some(origin.declaration)
                })
                .ok_or_else(|| error(origin.reference, "selected declaration disappeared"))?
                .symbol;
            occurrences.push(
                program
                    .record_resolved_authored_declaration_selection(
                        origin.reference,
                        Exposure::PrivateImplementation,
                        Kind::StaticPathSegment,
                        selected,
                    )
                    .map_err(super::const_selection_record_diagnostic)?,
            );
        }
        for operator in operators {
            let occurrence = program
                .record_late_bound_authored_declaration_selection(
                    operator,
                    Exposure::PrivateImplementation,
                    Kind::Operator,
                    LateBinding::CheckedOperator,
                )
                .map_err(super::const_selection_record_diagnostic)?;
            program
                .finalize_intrinsic_authored_declaration_selection(
                    occurrence,
                    LateBinding::CheckedOperator,
                    Intrinsic::BuiltinOperator,
                )
                .map_err(|reason| {
                    error(
                        operator,
                        &format!("builtin selection finalization failed: {reason:?}"),
                    )
                })?;
            occurrences.push(occurrence);
        }
        program
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(record.materialized, occurrences);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use syntax_trees::{
        expression::ExpressionNode,
        item::{ConstInitializerNormalization, Item},
    };

    fn normalized() -> (SyntaxTrees, syntax_trees::item::ItemHandle) {
        let tokens = source_files_to_tokens::Lexer::new(
            "const BASE: u64 = 2; pub const COUNT: u64 = BASE + 1; machine main() -> u64 { COUNT }",
        )
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
        let ExpressionNode::Binary(binary) = syntax.expressions.expression(original) else {
            panic!("binary initializer")
        };
        let ExpressionNode::Name(path) = syntax.expressions.expression(binary.left) else {
            panic!("named dependency")
        };
        let members = syntax.expressions.identifier_path_members(*path);
        let reference = members[0].source_span();
        let encoding = crate::generic_data::canonicalize_declared_const_definition(&syntax, base)
            .expect("canonical BASE")
            .encoding;
        let value = syntax.expressions.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(3),
        ));
        let original_span = syntax.expressions.source_span(original);
        syntax.expressions.set_source_span(value, original_span);
        definition.value = value;
        definition.normalization = Some(ConstInitializerNormalization {
            authored_expression: original,
            canonical_result_encoding: crate::generic_data::canonicalize_declared_const_definition(
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
        });
        syntax.items.replace_item(item, Item::Const(definition));
        (syntax, item)
    }

    #[test]
    fn initializer_receipt_retains_dependency_and_operator_on_materialized_root() {
        let (syntax, _) = normalized();
        let resolved =
            crate::lower_syntax_trees(&syntax).expect("resolve retained initializer receipt");
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
                crate::lower_syntax_trees(&syntax).is_err(),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn retained_initializer_custody_rejects_unchecked_operators_and_cycles() {
        for mutation in 0..3 {
            let (syntax, _) = normalized();
            let mut program = crate::lower_syntax_trees(&syntax).expect("retained base");
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
        let mut program = crate::lower_syntax_trees(&syntax).expect("retained base");
        let materialized = program
            .const_declarations
            .iter()
            .find(|declaration| program.symbols.name(declaration.symbol) == "COUNT")
            .expect("COUNT")
            .initializer;
        let record = PendingInitializer {
            declaration: definition.name.source_span(),
            materialized,
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
        let normalize =
            |syntax: &mut SyntaxTrees,
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
        let one_shot = crate::lower_syntax_trees_with_sources(&combined, sources.clone())
            .expect("one-shot normalized declarations");
        let base = crate::lower_syntax_trees_with_sources(&base_syntax, sources.clone())
            .expect("normalized retained base");
        let retained_selections = base.authored_declaration_selections().as_slice().to_vec();
        let seeded = crate::lower_syntax_extension_against_resolved_base(
            base,
            &extension_syntax,
            sources,
            Vec::new(),
        )
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
}
