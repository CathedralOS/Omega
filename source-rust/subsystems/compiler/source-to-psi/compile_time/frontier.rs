//! Carry unresolved evaluation requests on their typed declarations until a
//! consumer demands those declarations. Retained inputs require the same
//! declaration-selection authority when they are reopened.

use declaration_scope::DeclarationSelectionAuthority;
use diagnostics::Diagnostic;
use record_storage::{Record, RecordError, RecordReader, RecordWriter};
use std::sync::Arc;
use typed_trees::source_state::TypedTrees;
use typed_trees::source_state::declaration_authority::RecordedDeclarationAuthority;
use typed_trees::source_state::deferred_refusals::DeferredRequest;

/// One evaluation request written inside a declaration, and the declaration
/// that holds it.
type SourceJob = (symbols::SymbolHandle, DeferredRequest);

fn unsupported(job: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("no product route yet: {job}"))]
}

/// Records every evaluation request written inside a declaration on that
/// declaration (`typed_trees::source_state::deferred_refusals`). This stage sees the
/// program with every bundled library, so a type with a layout policy in a
/// library declaration is not an error of a program that never reaches it.
/// Work recorded beside the program (recipes, plans, schemas) still refuses.
fn defer_source_jobs(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let jobs = source_jobs(typed)?;
    record_jobs(typed, jobs);
    Ok(())
}

/// Recompute unresolved layout policy occurrences after a target's plans were
/// installed. Each declaration keeps its other independently issued obligations.
pub(super) fn refresh_layout_policy_requests(
    typed: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let jobs = declared_jobs(typed)?;
    typed
        .deferred_refusals
        .retain(|row| row.request != DeferredRequest::LayoutPolicyStorage);
    record_jobs(
        typed,
        jobs.into_iter()
            .filter(|(_, request)| *request == DeferredRequest::LayoutPolicyStorage),
    );
    Ok(())
}

fn record_jobs(typed: &mut TypedTrees, jobs: impl IntoIterator<Item = SourceJob>) {
    for (declaration, request) in jobs {
        let source_span = typed
            .symbols
            .symbol_source_span(declaration)
            .unwrap_or_default();
        typed.record_deferred_refusal(declaration, request, source_span);
    }
}

/// The evaluation requests of the program's declarations, after refusing
/// every request the program holds outside them.
fn source_jobs(typed: &TypedTrees) -> Result<Vec<SourceJob>, Vec<Diagnostic>> {
    require_no_recorded_work(typed)?;
    declared_jobs(typed)
}

/// Refuses work recorded beside the program rather than inside one of its
/// declarations: nothing could raise it later.
fn require_no_recorded_work(typed: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    require_no_unevaluated_constant_use(typed)?;
    if !typed.constant_recipes.is_empty()
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
    // A numbered record contributes a derived schema for later selection.
    // Evaluation is owed once an encoding or its executable plan is present.
    if typed
        .wire_schemas()
        .iter()
        .any(|schema| schema.encoding.is_some())
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
    Ok(())
}

/// The evaluation requests written inside declarations, each with the
/// declaration that holds it. One that no declaration owns refuses.
pub(super) fn declared_jobs(typed: &TypedTrees) -> Result<Vec<SourceJob>, Vec<Diagnostic>> {
    // Reuse the reference owner's complete type-position inventory rather
    // than searching selected statements or treating a type as only a name.
    let references =
        typed_trees::source_state::references::collect_source_evaluation_references(typed);
    let mut expressions = Vec::new();
    for expression in &references.expressions {
        if typed
            .expression_table
            .materialization_source(*expression)
            .is_valid()
        {
            expressions.push((*expression, DeferredRequest::MaterializedExpression));
        }
    }
    let mut types = Vec::new();
    for reference in references.type_references {
        use typed_trees::source_state::types::{FixedArrayLength, TypeReferenceNode};
        if typed.layout_storage_arguments(reference).is_some() {
            if typed.plan_laid_layout_for_type(reference).is_none() {
                types.push((reference, DeferredRequest::LayoutPolicyStorage));
            }
            continue;
        }
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::ConstExpression(_)
                if !typed
                    .constant_recipe_for_const_application(reference)
                    .is_valid() =>
            {
                types.push((reference, DeferredRequest::TypeExpressionEvaluation));
            }
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Expression(_) | FixedArrayLength::ConstCall { .. },
                ..
            } if typed.declared_array_extent(reference).is_none() => {
                types.push((reference, DeferredRequest::ArrayExtentEvaluation));
            }
            TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Constrained { .. }
            | TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Callable { .. }
            | TypeReferenceNode::Unit => {}
        }
    }
    let mut jobs = Vec::new();
    if expressions.is_empty() && types.is_empty() {
        return Ok(jobs);
    }
    // A request is its declaration's to carry. One that no declaration owns
    // has no reader that could raise it, so it still refuses the program.
    let owners =
        typed_trees::source_state::references::collect_source_evaluation_operand_owners(typed);
    for (expression, request) in expressions {
        let before = jobs.len();
        jobs.extend(
            owners
                .expression_owners(expression)
                .filter(|owner| owner.is_valid())
                .map(|owner| (owner, request)),
        );
        if jobs.len() == before {
            return Err(unsupported(&format!(
                "outside every declaration, one {}",
                request.construct()
            )));
        }
    }
    for (reference, request) in types {
        let before = jobs.len();
        jobs.extend(
            owners
                .type_owners(reference)
                .filter(|owner| owner.is_valid())
                .map(|owner| (owner, request)),
        );
        if jobs.len() == before {
            return Err(unsupported(&format!(
                "outside every declaration, one {}",
                request.construct()
            )));
        }
    }
    Ok(jobs)
}

/// Refuse the first use of a constant whose value no stage has recorded.
///
/// Declaring a constant asks for nothing: the program can carry it unread.
/// What needs a value is a reference to the constant from some other
/// declaration, such as a machine body or a type. Resolution records the
/// value of each initializer it owns the folding of; a referenced constant
/// without a recorded value is work for an evaluator that does not exist
/// yet, so the reference is refused by the constant's name. The initializer
/// is never read here as a substitute for the missing value.
fn require_no_unevaluated_constant_use(typed: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let unevaluated = typed
        .const_declarations()
        .iter()
        .filter(|declaration| {
            declaration.evaluated_value.is_none()
                && !typed.tables.machines.is_valid(declaration.evaluation_entry)
        })
        .map(|declaration| declaration.symbol)
        .collect::<Vec<_>>();
    if unevaluated.is_empty() {
        return Ok(());
    }
    // Constants and the derived machines that would compute them, sorted.
    // Their own bodies are declarations, not uses.
    let key = |symbol: symbols::SymbolHandle| (symbol.arena_index(), symbol.generation());
    let mut constant_owned = typed
        .const_declarations()
        .iter()
        .flat_map(|declaration| {
            let entry = typed
                .tables
                .machines
                .is_valid(declaration.evaluation_entry)
                .then(|| {
                    typed
                        .tables
                        .machines
                        .get(declaration.evaluation_entry)
                        .symbol
                });
            [Some(declaration.symbol), entry].into_iter().flatten()
        })
        .collect::<Vec<_>>();
    constant_owned.sort_unstable_by_key(|symbol| key(*symbol));
    let mut uses =
        typed_trees::source_state::references::ConstantUses::new(unevaluated, |declaration| {
            constant_owned
                .binary_search_by_key(&key(declaration), |symbol| key(*symbol))
                .is_err()
        });
    typed_trees::source_state::references::visit_program(typed, &mut uses);
    for argument in typed.designated_static_type_arguments() {
        typed_trees::source_state::references::visit_type_reference(
            typed,
            &mut uses,
            argument.reference,
        );
    }
    let Some(typed_trees::source_state::references::ConstantUse {
        constant,
        expression,
    }) = uses.into_uses().into_iter().next()
    else {
        return Ok(());
    };
    let name = if typed.symbols.symbols().nodes().is_valid(constant) {
        typed.symbols.name(constant)
    } else {
        "<unnamed>"
    };
    let mut refusal = Diagnostic::error(format!(
        "no product route yet: source constant evaluation: `{name}` is used but no stage has \
         evaluated it"
    ));
    if expression.is_valid() {
        refusal = refusal.with_source_span(typed.expression_table.source_span(expression));
    }
    Err(vec![refusal])
}

/// Declaration visibility accompanying a retained or activated typed source program.
#[derive(Clone)]
pub struct PreCheckEvaluation {
    authority: Option<Arc<dyn DeclarationSelectionAuthority>>,
    recorded_authority: Option<RecordedDeclarationAuthority>,
}
impl PreCheckEvaluation {
    pub fn new(authority: Option<Arc<dyn DeclarationSelectionAuthority>>) -> Self {
        Self {
            authority,
            recorded_authority: None,
        }
    }

    pub fn rejoin_source_authority(
        &mut self,
        program: &TypedTrees,
        authority: Option<Arc<dyn DeclarationSelectionAuthority>>,
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
    /// Records the evaluation requests written inside the declarations of a
    /// freshly typed program on those declarations. The owner of the typing
    /// calls this once it has the program, before any narrowing or
    /// comparison of it: those only read, and accept a request only where
    /// its declaration already carries the refusal.
    pub fn defer_declared_requests(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        defer_source_jobs(typed)
    }
    /// A request written inside a declaration is accepted here only when that
    /// declaration already carries its refusal (`defer_source_jobs` records
    /// it): the declaration is then refused wherever it is demanded, and this
    /// program, which may never reach it, goes on.
    pub fn require_recorded_requests(typed: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        for (declaration, request) in source_jobs(typed)? {
            if !typed
                .deferred_refusals_of(typed.refusal_declaration(declaration))
                .any(|row| row.request == request)
            {
                return Err(unsupported(&format!(
                    "declaration `{}` {}",
                    typed.symbols.name(declaration),
                    request.construct()
                )));
            }
        }
        Ok(())
    }

    /// The evaluation continuation of the program `activation` narrowed out
    /// of a retained base.
    ///
    /// Only the activated program's requests are this continuation's to
    /// account for. The base also holds sources the program did not select,
    /// and the requests of their declarations stay with the base's own
    /// continuation; they do not stop a program that holds none of them.
    pub fn activate_sources(&self, activation: &TypedTrees) -> Result<Self, Vec<Diagnostic>> {
        Self::require_recorded_requests(activation)?;
        Ok(self.clone())
    }
    /// All continuations refer to this typed program. Preserve the ordering
    /// of authority and inventory errors while recording its requests once.
    pub fn record_continuations(
        continuations: Vec<Self>,
        typed: &mut TypedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut continuations = continuations.into_iter();
        if let Some(first) = continuations.next() {
            first.require_rejoined_authority()?;
            defer_source_jobs(typed)?;
        }
        for pending in continuations {
            pending.require_rejoined_authority()?;
        }
        Ok(())
    }

    fn require_rejoined_authority(&self) -> Result<(), Vec<Diagnostic>> {
        if self.recorded_authority.is_some() {
            return Err(vec![Diagnostic::error(
                "source normalization authority has not been rejoined",
            )]);
        }
        Ok(())
    }
    pub fn record_extension(
        pre_checks: Vec<Self>,
        wire_schema_frontier: usize,
        typed: &mut TypedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        if wire_schema_frontier > typed.wire_schemas().len() {
            return Err(vec![Diagnostic::error(
                "source extension lost its original wire-schema frontier",
            )]);
        }
        defer_source_jobs(typed)?;
        for pending in pre_checks {
            pending.require_rejoined_authority()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_requests_require_the_current_source_authority() {
        let limits = record_storage::RecordLimits::default();
        let pending = PreCheckEvaluation::new(None);
        let bytes = record_storage::encode(&pending, limits).unwrap();
        let mut restored: PreCheckEvaluation = record_storage::decode(&bytes, limits).unwrap();
        let mut typed = TypedTrees::default();

        let error = PreCheckEvaluation::record_continuations(vec![restored.clone()], &mut typed)
            .unwrap_err();
        assert_eq!(
            error[0].message,
            "source normalization authority has not been rejoined"
        );

        let error = PreCheckEvaluation::record_extension(vec![restored.clone()], 1, &mut typed)
            .unwrap_err();
        assert_eq!(
            error[0].message,
            "source extension lost its original wire-schema frontier"
        );

        restored.rejoin_source_authority(&typed, None).unwrap();
        PreCheckEvaluation::record_continuations(vec![pending, restored], &mut typed).unwrap();
        assert!(typed.deferred_refusals.is_empty());

        let mut obsolete = bytes;
        let marker = b"BTE-source-input-7";
        let index = obsolete
            .windows(marker.len())
            .position(|bytes| bytes == marker)
            .unwrap();
        obsolete[index + marker.len() - 1] = b'6';
        assert!(record_storage::decode::<PreCheckEvaluation>(&obsolete, limits).is_err());
    }
}
