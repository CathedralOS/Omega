//! Keep ordinary source intact until resolution identifies its actual
//! evaluation requests. A declaration is not itself an execution request;
//! unresolved work needs a real lowered-form producer and is never erased.

use compiler_storage::{Record, RecordError, RecordReader, RecordWriter};
use diagnostics::Diagnostic;
pub use semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority as BuildTimeSelectionAuthority;
use std::sync::Arc;
use typed_trees::TypedTrees;
use typed_trees::declaration_authority::RecordedDeclarationAuthority;

fn unavailable() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "no product route yet: source evaluation and normalization",
    )]
}
/// This stage consumes no evaluation job. Reject actual unresolved work,
/// including work in type positions, before emitting a zero-work receipt.
/// Ordinary bodies and declarations remain for the source checker to judge.
fn require_no_source_jobs(typed: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let unsupported = |job| vec![Diagnostic::error(format!("no product route yet: {job}"))];
    if !typed.const_declarations().is_empty()
        || !typed.constant_recipes.is_empty()
        || !typed.constant_expression_contexts.is_empty()
        || !typed.constant_recipe_closure.values.is_empty()
        || !typed.constant_recipe_closure.receipts.is_empty()
        || !typed.constant_recipe_closure.target_bodies.is_empty()
    {
        return Err(unsupported("source constant evaluation"));
    }
    if !typed.pending_const_range_endpoints.is_empty() {
        return Err(unsupported("source constant range-endpoint evaluation"));
    }
    if typed.static_type_contexts != Default::default() {
        return Err(unsupported(
            "retained source type judgments need their actual producer correspondence",
        ));
    }
    if typed.product_declaration_frontier.is_some() {
        return Err(unsupported(
            "retained product source normalization correspondence",
        ));
    }
    if !typed.wire_schemas().is_empty()
        || !typed.wire_schema_plans.is_empty()
        || !typed.wire_placements.is_empty()
        || !typed.wire_encode_obligations.is_empty()
    {
        return Err(unsupported("source wire-schema evaluation"));
    }
    if !typed.access_policy_invocations.is_empty()
        || !typed.access_view_plans.is_empty()
        || !typed.access_lifecycle_plans.is_empty()
    {
        return Err(unsupported("source access-policy evaluation"));
    }
    if !typed.plan_laid_layouts.is_empty() || !typed.plan_laid_target_layouts.is_empty() {
        return Err(unsupported("source layout-policy evaluation"));
    }
    for argument in typed.designated_static_type_arguments() {
        if !typed
            .type_reference_table
            .contains_type_reference(argument.reference)
            || !typed.symbols.symbols().nodes().is_valid(argument.owner)
        {
            return Err(unsupported(
                "source designated type argument has no exact declaration/type custody",
            ));
        }
    }
    // Reuse the reference owner's complete type-position inventory rather
    // than searching selected statements or treating a type as only a name.
    let references = typed_trees::references::collect_program_references(typed);
    if references.expressions.iter().any(|expression| {
        typed
            .expression_table
            .materialization_source(*expression)
            .is_valid()
    }) {
        return Err(unsupported(
            "source materialized-expression evaluation correspondence",
        ));
    }
    for reference in references.type_references {
        use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
        if typed.layout_storage_arguments(reference).is_some() {
            return Err(unsupported("source layout-policy evaluation"));
        }
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::ConstExpression(_) => {
                return Err(unsupported("source type-expression evaluation"));
            }
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Expression(_) | FixedArrayLength::ConstCall { .. },
                ..
            } => return Err(unsupported("source array-extent evaluation")),
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Constrained { .. }
            | TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Unit => {}
        }
    }
    Ok(())
}

#[derive(Clone, Default)]
pub struct BuildTimeSources<'a> {
    pub declaration_syntax: Option<&'a syntax_trees::SyntaxTrees>,
    pub retained_declaration_syntax: Option<&'a syntax_trees::SyntaxTrees>,
    pub sources: Option<Arc<source::SourceMap>>,
    pub source_scoped_top_level_bindings: &'a [symbols::SourceScopedTopLevelBinding],
    pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    pub predecessor: Option<symbol_resolved_trees_to_typed_trees::SeededTypingBase>,
}

pub struct BuildTimeEvaluationRequest<'a> {
    pub syntax_trees: syntax_trees::SyntaxTrees,
    pub source_context: BuildTimeSources<'a>,
}

#[derive(Clone)]
pub struct AuthorizedSelection {
    pub authority: Arc<dyn BuildTimeSelectionAuthority>,
    pub custody: BuildTimeInvocationCustody,
}
#[derive(Clone, Copy, Debug)]
pub enum BuildTimeInvocationCustody {
    Source(source::SourceSpan),
    Symbol(symbols::SymbolHandle),
}

pub struct PreResolutionEvaluation {
    syntax_trees: syntax_trees::SyntaxTrees,
    pre_check: PreCheckEvaluation,
}

pub fn evaluate_pre_resolution(
    request: BuildTimeEvaluationRequest<'_>,
) -> Result<PreResolutionEvaluation, Vec<Diagnostic>> {
    // Resolution retains the exact source and exposes typed evaluation
    // requests. Do not execute or discard them based on untyped root count;
    // the continuation must account for them before it can finish.
    Ok(PreResolutionEvaluation {
        syntax_trees: request.syntax_trees,
        pre_check: PreCheckEvaluation {
            authority: request.source_context.selection_authority,
            recorded_authority: None,
        },
    })
}
impl PreResolutionEvaluation {
    pub fn into_syntax_and_pre_check(self) -> (syntax_trees::SyntaxTrees, PreCheckEvaluation) {
        (self.syntax_trees, self.pre_check)
    }
    pub fn evaluate_extension(
        units: Vec<syntax_trees::SyntaxTrees>,
        first_source: source::SourceId,
        context: BuildTimeSources<'_>,
    ) -> Result<(syntax_trees::SyntaxTrees, Vec<PreCheckEvaluation>), Vec<Diagnostic>> {
        let mut syntax: Option<syntax_trees::SyntaxTrees> = None;
        let mut continuations = Vec::with_capacity(units.len());
        for unit in units {
            let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
                syntax_trees: unit,
                source_context: context.clone(),
            })?;
            let (unit, continuation) = evaluated.into_syntax_and_pre_check();
            if let Some(syntax) = &mut syntax {
                syntax.adopt_page(unit);
            } else {
                syntax = Some(unit);
            }
            continuations.push(continuation);
        }
        let mut syntax = syntax.unwrap_or_else(|| syntax_trees::SyntaxTrees::new(first_source));
        let generation = u32::try_from(syntax.page_count())
            .ok()
            .and_then(|v| v.checked_add(1))
            .ok_or_else(unavailable)?;
        syntax.begin_page(generation);
        Ok((syntax, continuations))
    }
}

#[derive(Clone)]
pub struct PreCheckEvaluation {
    authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    recorded_authority: Option<RecordedDeclarationAuthority>,
}
impl PreCheckEvaluation {
    pub fn rejoin_source_authority(
        &mut self,
        program: &TypedTrees,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<(), String> {
        if let Some(recorded) = &self.recorded_authority {
            if matches!(recorded, RecordedDeclarationAuthority::Absent) {
                if RecordedDeclarationAuthority::capture(authority.as_deref())?
                    != RecordedDeclarationAuthority::Absent
                {
                    return Err("presence of declaration selection authority changed".into());
                }
            } else {
                let sources = program
                    .symbols
                    .source_map()
                    .ok_or("restored source input has no complete source context")?;
                recorded.require_current(authority.as_deref(), &sources)?;
            }
            self.authority = authority;
            self.recorded_authority = None;
        }
        Ok(())
    }
    pub fn same_pending_work(&self, other: &Self) -> bool {
        self.recorded_authority.is_none()
            && other.recorded_authority.is_none()
            && match (&self.authority, &other.authority) {
                (None, None) => true,
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                _ => false,
            }
    }
    pub fn project(
        &self,
        original: &TypedTrees,
        projection: &TypedTrees,
    ) -> Result<Self, Vec<Diagnostic>> {
        require_no_source_jobs(original)?;
        require_no_source_jobs(projection)?;
        Ok(self.clone())
    }
    pub fn activate_sources(
        &self,
        original: &TypedTrees,
        activation: &TypedTrees,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.project(original, activation)
    }
    pub fn exclude_completed_work(
        &self,
        original: &TypedTrees,
        completed: &TypedTrees,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.project(original, completed)
    }
    pub fn evaluate_or_defer_recorded(
        self,
        typed: &mut TypedTrees,
    ) -> Result<crate::RecordedPreCheck, Vec<Diagnostic>> {
        self.evaluate(typed)?;
        // The complete source-job inventory was empty. Reading declarations
        // produced no execution receipt and discharged no source contract.
        Ok(crate::RecordedPreCheck {
            pending: None,
            accounting: crate::SourceCompletionAccounting::default(),
        })
    }
    pub fn evaluate_or_defer(
        self,
        typed: &mut TypedTrees,
    ) -> Result<Option<Self>, Vec<Diagnostic>> {
        self.evaluate(typed)?;
        Ok(None)
    }
    pub fn evaluate_extension_or_defer(
        pre_checks: Vec<Self>,
        wire_schema_frontier: usize,
        typed: &mut TypedTrees,
    ) -> Result<Vec<Self>, Vec<Diagnostic>> {
        if wire_schema_frontier > typed.wire_schemas().len() {
            return Err(vec![Diagnostic::error(
                "source extension lost its original wire-schema frontier",
            )]);
        }
        if wire_schema_frontier < typed.wire_schemas().len() {
            return Err(vec![Diagnostic::error(
                "no product route yet: source wire-schema evaluation",
            )]);
        }
        require_no_source_jobs(typed)?;
        for pending in pre_checks {
            pending.evaluate(typed)?;
        }
        Ok(Vec::new())
    }
    pub fn evaluate(self, typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        if self.recorded_authority.is_some() {
            return Err(vec![Diagnostic::error(
                "source normalization authority has not been rejoined",
            )]);
        }
        require_no_source_jobs(typed)
    }
}
impl Record for PreCheckEvaluation {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| {
            "BTE-source-input-7".to_owned().write(output)?;
            let scope = match &self.recorded_authority {
                Some(scope) => scope.clone(),
                None => RecordedDeclarationAuthority::capture(self.authority.as_deref())
                    .map_err(|e| output.error(e))?,
            };
            scope.write(output)
        })
    }
    fn read<C>(input: &mut RecordReader<'_, C>) -> Result<Self, RecordError> {
        input.record(|input| {
            if String::read(input)? != "BTE-source-input-7" {
                return Err(
                    input.error("no product route yet: retired source evaluation continuation")
                );
            }
            Ok(Self {
                authority: None,
                recorded_authority: Some(Record::read(input)?),
            })
        })
    }
}
