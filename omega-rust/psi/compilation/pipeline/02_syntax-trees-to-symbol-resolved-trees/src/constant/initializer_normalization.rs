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
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::{SyntaxTrees, item::ConstDefinition, types::ConstArgumentOrigin};

/// The three existing initializer inventories: original value use, complete
/// declaration preparation, and the materialized value's realized selections.
#[derive(Clone, Copy)]
pub enum InitializerCustodyPurpose {
    Authored,
    Declaration,
    Value,
}

/// Read a completed predecessor's original graph, never its source syntax or
/// a guessed summary. The receiving evaluator still checks every new use.
pub(crate) fn retained_constant_custody(
    program: &SymbolResolvedTrees,
    owner: &symbol_resolved_trees::constant::ConstDeclaration,
    purpose: InitializerCustodyPurpose,
) -> Result<syntax_trees::item::ConstInitializerCustody, Diagnostic> {
    use super::initializer_dependencies::{DependencyPurpose, collect_declaration};
    let source = program
        .symbols
        .symbol_source_span(owner.symbol)
        .ok_or_else(|| {
            error(
                owner.initializer_source_span,
                "retained constant lost its declaration source",
            )
        })?;
    let mut custody = syntax_trees::item::ConstInitializerCustody::default();
    if matches!(purpose, InitializerCustodyPurpose::Value) {
        append_retained_custody(
            program,
            owner,
            source,
            &mut custody,
            DependencyPurpose::DeclarationReadiness,
        )?;
        return Ok(custody);
    }
    let dependencies = collect_declaration(
        program,
        if owner.authored_initializer.is_valid() {
            owner.authored_initializer
        } else {
            owner.initializer
        },
        &owner.declared_type,
        if matches!(purpose, InitializerCustodyPurpose::Authored) {
            DependencyPurpose::Authored
        } else {
            DependencyPurpose::DeclarationReadiness
        },
        None,
    )
    .map_err(|reason| error(source, &reason))?;
    custody.builtin_operators = dependencies.initializer_operators;
    custody.call_selections = dependencies.calls;
    for (reference, declaration_source) in dependencies.constants {
        let declaration = program
            .const_declaration_at_source_span(declaration_source)
            .ok_or_else(|| {
                error(
                    reference,
                    "retained constant dependency lost its declaration",
                )
            })?;
        if !declaration.is_public
            && !program
                .symbols
                .same_source_package(reference, declaration_source)
        {
            return Err(error(
                reference,
                "retained constant selects a private foreign declaration",
            ));
        }
        let origin = ConstArgumentOrigin {
            reference,
            declaration: declaration_source,
            initializer: declaration.initializer_source_span,
            evaluated_value: declaration
                .evaluated_value
                .clone()
                .ok_or_else(|| error(reference, "retained dependency has no completed value"))?,
        };
        if !custody.selections.contains(&origin) {
            custody.selections.push(origin);
        }
    }
    Ok(custody)
}

pub(crate) struct PendingInitializer {
    declaration: SourceSpan,
    materialized: ExpressionHandle,
    pub(crate) authored: ExpressionHandle,
    references: Vec<(SourceSpan, String)>,
    operators: Vec<SourceSpan>,
    receipt: syntax_trees::item::ConstInitializerNormalization,
}

/// The exact inventory validated before constant substitution. Substitution may
/// inherit another declaration's realized rows; this owner rejoins the same
/// root afterward without re-resolving or manufacturing occurrence identities.
pub(crate) struct InitializerRootCustody {
    root: ExpressionHandle,
    reference: SourceSpan,
    declaration: SourceSpan,
    occurrences: Vec<symbol_resolved_trees::AuthoredDeclarationSelectionOccurrenceId>,
}

pub(crate) fn rejoin_after_substitution(
    program: &mut SymbolResolvedTrees,
    custody: &[InitializerRootCustody],
) -> Result<(), Diagnostic> {
    for owner in custody {
        replace_initializer_lanes(program, owner)?;
    }
    Ok(())
}

fn error(reference: SourceSpan, reason: &str) -> Diagnostic {
    Diagnostic::error(format!("constant initializer normalization: {reason}"))
        .with_source_span(reference)
}

pub(crate) fn retain(
    lowerer: &mut crate::resolution::lowerer::Lowerer,
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
    let value = super::declaration_values::evaluated_declaration_value(
        syntax,
        definition,
        lowerer.constant_selection.as_ref(),
    )
    .map_err(|_| error(reference, "materialized value is not canonical"))?;
    if value != receipt.result_value {
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
            ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_) => {}
            ExpressionNode::Name(path) => {
                let members = syntax.expressions.identifier_path_members(*path);
                let (Some(first), Some(last)) = (members.first(), members.last()) else {
                    return Err(error(reference, "authored dependency lost its name"));
                };
                let reference = SourceSpan::new(
                    first.source_span().source_id,
                    source::Span::new(first.source_span().span.start, last.source_span().span.end),
                );
                let name = members.joined_path_in(syntax);
                let identifier = syntax_trees::identifier::Identifier::new(name.clone(), reference);
                if lowerer
                    .constant_selection
                    .as_ref()
                    .is_some_and(|selection| {
                        selection
                            .qualified_payload_free_case(syntax, &identifier)
                            .is_ok_and(|selected| selected.is_some())
                    })
                {
                    // A literal case retains ordinary constructor selection;
                    // it is not an evaluated named-constant dependency.
                    continue;
                }
                references.push((reference, name));
            }
            ExpressionNode::Binary(binary) => {
                operators.push(syntax.expressions.source_span(expression));
                pending.push(binary.right);
                pending.push(binary.left);
            }
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Unary(unary) => {
                operators.push(syntax.expressions.source_span(expression));
                pending.push(unary.operand);
            }
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Call(call) => {
                let arguments = syntax.expressions.expression_handles(call.arguments);
                if arguments.len() != call.arguments.len() {
                    return Err(error(reference, "authored call has a stale argument span"));
                }
                pending.extend(arguments.iter().rev().copied());
            }
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(
                    syntax
                        .expressions
                        .expression_handles(*elements)
                        .iter()
                        .rev()
                        .copied(),
                );
            }
            ExpressionNode::StructLiteral(literal) => {
                let fields = syntax.expressions.struct_fields(literal.fields);
                if fields.len() != literal.fields.len() {
                    return Err(error(
                        reference,
                        "authored constructor has a stale field span",
                    ));
                }
                pending.extend(fields.iter().rev().map(|field| field.value));
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
                    "authored expression is outside constant leaf evaluation",
                ));
            }
        }
    }
    let original =
        lowerer.with_authored_expression_exposure(Exposure::PrivateImplementation, |lowerer| {
            crate::lowering::expression::lower_expression_into_table(
                lowerer,
                syntax,
                receipt.authored_expression,
            )
        })?;
    lowerer.pending_const_values.push(original);
    lowerer.pending_const_initializers.push(PendingInitializer {
        declaration: definition.name.source_span(),
        materialized,
        authored: original,
        references,
        operators,
        receipt: receipt.clone(),
    });
    Ok(())
}

/// Reconstruct the transitive roster from original references, not receipt
/// membership. Even omitted or unused dependencies must rejoin current values.
/// Rejoin the receipt to its exact retained owner before traversing geometry.
/// A same-spelled declaration or a sibling instance cannot supply this type.
fn declaration_custody(
    program: &SymbolResolvedTrees,
    record: &PendingInitializer,
    purpose: super::initializer_dependencies::DependencyPurpose,
) -> Result<super::initializer_dependencies::ConstInitializerDependencies, Diagnostic> {
    let mut owners = program.const_declarations.iter().filter(|declaration| {
        program.symbols.symbol_source_span(declaration.symbol) == Some(record.declaration)
            && declaration.initializer == record.materialized
    });
    let owner = owners.next().ok_or_else(|| {
        error(
            record.declaration,
            "normalization lost its exact declaration owner",
        )
    })?;
    if owners.next().is_some() {
        return Err(error(
            record.declaration,
            "normalization has ambiguous declaration custody",
        ));
    }
    super::initializer_dependencies::collect_declaration(
        program,
        record.authored,
        &owner.declared_type,
        purpose,
        None,
    )
    .map_err(|reason| error(record.declaration, &reason))
}

fn reconstruct(
    program: &SymbolResolvedTrees,
    records: &[PendingInitializer],
    root: usize,
    purpose: super::initializer_dependencies::DependencyPurpose,
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
        let mut dependencies = Vec::new();
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
            dependencies.push((*reference, selected));
        }
        let closure = declaration_custody(program, record, purpose)?;
        for operator in &closure.initializer_operators {
            if !operators.contains(operator) {
                operators.push(*operator);
            }
        }

        for (reference, declaration) in closure.constants {
            let selected = program
                .const_declarations
                .iter()
                .find(|candidate| {
                    program.symbols.symbol_source_span(candidate.symbol) == Some(declaration)
                })
                .ok_or_else(|| error(reference, "closure dependency lost its exact declaration"))?
                .symbol;
            if !dependencies.contains(&(reference, selected)) {
                dependencies.push((reference, selected));
            }
        }
        for (reference, selected) in &dependencies {
            let declaration = program
                .const_declaration(*selected)
                .ok_or_else(|| error(*reference, "selected dependency lost its declaration"))?;
            let source = program
                .symbols
                .symbol_source_span(*selected)
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
                evaluated_value: declaration.evaluated_value.clone().ok_or_else(|| {
                    error(*reference, "dependency has no completed canonical value")
                })?,
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
                let mut retained = syntax_trees::item::ConstInitializerCustody::default();
                append_retained_custody(
                    program,
                    declaration,
                    records[root].declaration,
                    &mut retained,
                    purpose,
                )?;
                for origin in retained.selections {
                    if !origins.contains(&origin) {
                        origins.push(origin);
                    }
                }
                for operator in retained.builtin_operators {
                    if !operators.contains(&operator) {
                        operators.push(operator);
                    }
                }
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
    custody: &mut syntax_trees::item::ConstInitializerCustody,
    purpose: super::initializer_dependencies::DependencyPurpose,
) -> Result<(), Diagnostic> {
    let owner_source = program.symbols.symbol_source_span(owner.symbol);
    for occurrence in program
        .tables
        .bodies
        .expressions
        .authored_selection_occurrences(
            if purpose == super::initializer_dependencies::DependencyPurpose::Authored
                && owner.authored_initializer.is_valid()
            {
                owner.authored_initializer
            } else {
                owner.initializer
            },
        )
    {
        let selection = program
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or_else(|| error(root, "retained initializer lost its exact selection"))?;
        let reference = selection.source_span();
        match (selection.kind(), selection.target()) {
            (Kind::Call, Target::Resolved(selected))
                if matches!(
                    program.symbols.get(selected.selected_symbol()).kind,
                    SymbolKind::Machine | SymbolKind::State | SymbolKind::Operator
                ) =>
            {
                // The detached original rederives call custody. A copied call
                // selection is neither a constant origin nor a builtin premise;
                // a named boundary operator keeps that same custody-only role.
                let declaration = super::initializer_dependencies::call_declaration_source(
                    program,
                    selected.selected_symbol(),
                )
                .map_err(|reason| error(reference, &reason))?;
                if !custody.call_selections.contains(&(reference, declaration)) {
                    custody.call_selections.push((reference, declaration));
                }
            }
            (Kind::Operator, Target::Intrinsic(Intrinsic::BuiltinOperator)) => {
                if !custody.builtin_operators.contains(&reference) {
                    custody.builtin_operators.push(reference);
                }
            }
            (Kind::StaticPathSegment, Target::Resolved(selected))
                if matches!(
                    program.symbols.get(selected.selected_symbol()).kind,
                    SymbolKind::Module | SymbolKind::Data | SymbolKind::Variant
                ) =>
            {
                // Bare literal cases retain their ordinary resolved namespace,
                // carrier and case path rows. These are constructor custody,
                // not evaluated constant dependencies. Other static selections
                // must still select an exact retained constant below.
            }
            (Kind::StaticPathSegment, Target::Resolved(selected)) => {
                let declaration = program
                    .const_declaration(selected.selected_symbol())
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
                    evaluated_value: declaration.evaluated_value.clone().ok_or_else(|| {
                        error(
                            reference,
                            "retained dependency has no completed canonical value",
                        )
                    })?,
                };
                if !custody.selections.contains(&origin) {
                    custody.selections.push(origin);
                }
            }
            (
                Kind::StructLiteralType
                | Kind::StructLiteralCase
                | Kind::StructLiteralField
                | Kind::CaseReference,
                Target::Resolved(_),
            ) => {
                // The resolved initializer retains these exact declaration
                // selections through every copy. They are not scalar-evaluation
                // premises and therefore do not occupy normalization receipt slots.
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

fn attach_custody(
    program: &mut SymbolResolvedTrees,
    root: ExpressionHandle,
    custody: &syntax_trees::item::ConstInitializerCustody,
    available: &super::initializer_dependencies::ConstInitializerDependencies,
    declaration: SourceSpan,
) -> Result<InitializerRootCustody, Diagnostic> {
    let mut occurrences = Vec::new();
    for (reference, selected_source) in &custody.call_selections {
        let mut candidates = available.call_targets.iter().filter(|candidate| {
            candidate.reference == *reference && candidate.declaration == *selected_source
        });
        let candidate = candidates.next().ok_or_else(|| {
            error(
                *reference,
                "realized call has no exact source declaration dependency",
            )
        })?;
        if candidates.next().is_some() {
            return Err(error(
                *reference,
                "realized call has ambiguous source declaration custody",
            ));
        }
        occurrences.push(
            program
                .record_resolved_authored_declaration_selection(
                    *reference,
                    Exposure::PrivateImplementation,
                    Kind::Call,
                    candidate.target,
                )
                .map_err(crate::constant::selections::const_selection_record_diagnostic)?,
        );
    }
    for origin in &custody.selections {
        let mut candidates = program.const_declarations.iter().filter(|candidate| {
            program.symbols.symbol_source_span(candidate.symbol) == Some(origin.declaration)
        });
        let selected = candidates
            .next()
            .ok_or_else(|| error(origin.reference, "selected declaration disappeared"))?
            .symbol;
        if candidates.next().is_some() {
            return Err(error(
                origin.reference,
                "selected declaration has ambiguous source custody",
            ));
        }
        occurrences.push(
            program
                .record_resolved_authored_declaration_selection(
                    origin.reference,
                    Exposure::PrivateImplementation,
                    Kind::StaticPathSegment,
                    selected,
                )
                .map_err(crate::constant::selections::const_selection_record_diagnostic)?,
        );
    }
    for operator in &custody.builtin_operators {
        let occurrence = program
            .record_late_bound_authored_declaration_selection(
                *operator,
                Exposure::PrivateImplementation,
                Kind::Operator,
                LateBinding::CheckedOperator,
            )
            .map_err(crate::constant::selections::const_selection_record_diagnostic)?;
        program
            .finalize_intrinsic_authored_declaration_selection(
                occurrence,
                LateBinding::CheckedOperator,
                Intrinsic::BuiltinOperator,
            )
            .map_err(|reason| {
                error(
                    declaration,
                    &format!("builtin selection finalization failed: {reason:?}"),
                )
            })?;
        occurrences.push(occurrence);
    }
    let owner = InitializerRootCustody {
        root,
        reference: program.tables.bodies.expressions.source_span(root),
        declaration,
        occurrences,
    };
    replace_initializer_lanes(program, &owner)?;
    Ok(owner)
}

fn replace_initializer_lanes(
    program: &mut SymbolResolvedTrees,
    owner: &InitializerRootCustody,
) -> Result<(), Diagnostic> {
    if !program
        .tables
        .bodies
        .expressions
        .expression_is_valid(owner.root)
        || program.tables.bodies.expressions.source_span(owner.root) != owner.reference
    {
        return Err(error(
            owner.declaration,
            "initializer substitution changed its exact custody root",
        ));
    }
    // Substituting another constant may have copied its realized inventory
    // onto this root. The declaration owner has independently reconstructed the
    // complete inventory for this boundary; retain unrelated type/case custody,
    // then replace only the initializer's call, constant and operator lanes.
    let mut retained = Vec::new();
    for occurrence in program
        .tables
        .bodies
        .expressions
        .authored_selection_occurrences(owner.root)
    {
        let selection = program
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or_else(|| {
                error(
                    owner.declaration,
                    "initializer root lost an attached selection",
                )
            })?;
        let initializer_lane = matches!(selection.kind(), Kind::Call | Kind::Operator)
            || (selection.kind() == Kind::StaticPathSegment
                && matches!(selection.target(), Target::Resolved(selected)
                    if program.symbols.get(selected.selected_symbol()).kind == SymbolKind::Const));
        if !initializer_lane {
            retained.push(occurrence);
        }
    }
    retained.extend(owner.occurrences.iter().copied());
    program
        .tables
        .bodies
        .expressions
        .replace_authored_selection_occurrences(owner.root, retained);
    Ok(())
}

fn same_custody(
    expected: &syntax_trees::item::ConstInitializerCustody,
    actual: &syntax_trees::item::ConstInitializerCustody,
) -> bool {
    expected.selections.len() == actual.selections.len()
        && expected
            .selections
            .iter()
            .all(|value| actual.selections.contains(value))
        && expected.builtin_operators.len() == actual.builtin_operators.len()
        && expected
            .builtin_operators
            .iter()
            .all(|value| actual.builtin_operators.contains(value))
        && expected.call_selections.len() == actual.call_selections.len()
        && expected
            .call_selections
            .iter()
            .all(|value| actual.call_selections.contains(value))
}

pub(crate) fn finalize(
    program: &mut SymbolResolvedTrees,
    records: &[PendingInitializer],
) -> Result<Vec<InitializerRootCustody>, Diagnostic> {
    use super::initializer_dependencies::DependencyPurpose;
    use syntax_trees::item::ConstInitializerCustody;
    let mut roots = Vec::new();
    for (ordinal, record) in records.iter().enumerate() {
        let (authored_origins, authored_operators) =
            reconstruct(program, records, ordinal, DependencyPurpose::Authored)?;
        let authored_calls = declaration_custody(program, record, DependencyPurpose::Authored)?;
        let authored = ConstInitializerCustody {
            selections: authored_origins,
            builtin_operators: authored_operators,
            call_selections: authored_calls.calls.clone(),
        };
        let (origins, operators) = reconstruct(
            program,
            records,
            ordinal,
            DependencyPurpose::DeclarationReadiness,
        )?;
        let declaration_calls =
            declaration_custody(program, record, DependencyPurpose::DeclarationReadiness)?;
        let preparation = ConstInitializerCustody {
            selections: origins,
            builtin_operators: operators,
            call_selections: declaration_calls.calls.clone(),
        };
        if !same_custody(&authored, &record.receipt.authored_custody)
            || !same_custody(&preparation, &record.receipt.declaration_custody)
        {
            return Err(error(
                record.declaration,
                "authored or declaration readiness custody drifted",
            ));
        }
        let realized = ConstInitializerCustody {
            selections: record.receipt.selections.clone(),
            builtin_operators: record.receipt.builtin_operators.clone(),
            call_selections: record.receipt.call_selections.clone(),
        };
        if realized
            .selections
            .iter()
            .enumerate()
            .any(|(index, value)| realized.selections[..index].contains(value))
            || realized
                .builtin_operators
                .iter()
                .enumerate()
                .any(|(index, value)| realized.builtin_operators[..index].contains(value))
            || realized
                .call_selections
                .iter()
                .enumerate()
                .any(|(index, value)| realized.call_selections[..index].contains(value))
        {
            return Err(error(
                record.declaration,
                "realized custody contains duplicate occurrences",
            ));
        }
        // Source preparation establishes which declarations exist, never which
        // member was invoked. The checked receiver independently reconstructs
        // this realized roster and requires exact equality before replay.
        if realized
            .selections
            .iter()
            .any(|value| !preparation.selections.contains(value))
            || realized
                .builtin_operators
                .iter()
                .any(|value| !preparation.builtin_operators.contains(value))
            || realized
                .call_selections
                .iter()
                .any(|value| !preparation.call_selections.contains(value))
        {
            return Err(error(
                record.declaration,
                "realized custody has no exact source preparation dependency",
            ));
        }
        roots.push(attach_custody(
            program,
            record.authored,
            &authored,
            &authored_calls,
            record.declaration,
        )?);
        roots.push(attach_custody(
            program,
            record.materialized,
            &realized,
            &declaration_calls,
            record.declaration,
        )?);
    }
    Ok(roots)
}
