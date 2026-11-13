//! The retired source evaluator has no source-checker substitute. Empty
//! normalization requests preserve their actual input; other requests need a
//! producer through the lowered-form route.

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
fn empty_typed(typed: &TypedTrees) -> bool {
    // No retired semantic roster may be omitted from the no-work decision.
    typed == &TypedTrees::default()
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
    if !request.syntax_trees.root_item_handles().is_empty() {
        return Err(unavailable());
    }
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
        if !empty_typed(original) || !empty_typed(projection) {
            return Err(unavailable());
        }
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
        // No job ran: emptiness was established above, rather than inferred
        // from the absence of old facts or execution receipts.
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
        if wire_schema_frontier != 0 || !empty_typed(typed) {
            return Err(unavailable());
        }
        for pending in pre_checks {
            pending.evaluate(typed)?;
        }
        Ok(Vec::new())
    }
    pub fn evaluate(self, typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        if self.recorded_authority.is_some() || !empty_typed(typed) {
            return Err(unavailable());
        }
        Ok(())
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
