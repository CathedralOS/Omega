//! `size_of<T>()`/`align_of<T>()` inputs from the ordinary layout producer.
//!
//! Psi recognizes the compiler-owned spellings and retains each occurrence as
//! an explicit `DataLayoutQueryRequest` on the call (see
//! `language_core::data_layout_queries`). Policy inputs the layout producer
//! answers without target geometry can settle before source checking. The
//! remaining inputs wait for exact target custody. Once execution settlement
//! has selected the native target, this module folds each retained query into an
//! ordinary `u64` integer literal in place: every parent expression and every
//! `const` materialized initializer that shared the node sees the literal.
//!
//! The queried layout is never recomputed here. Ordinary type references go
//! through `layout::layout_type_reference` — the backend's layout authority —
//! which reports the carrier's own storage (padding included, erased fields
//! omitted, references measured as the reference, not the referent). A type
//! whose `data` definition carries a validated layout plan consults the
//! installed `PlanLaidLayout` inside that same path: its size and alignment
//! are the `LayoutPlanReport::{size, align}` that the plan-laid policy
//! evaluation in `build-time-evaluation` produced and the layout
//! installer validated, so policy, schema, and field custody stay exactly
//! where they were judged. Machine declarations and proof-only carriers were
//! already rejected at lowering, where the operand was admitted as a data
//! type.

use build_time_evaluation::LayoutContextInputs;
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::expression::ExpressionNode;
use checked_trees::typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use diagnostics::Diagnostic;
use language_core::data_layout_queries::DataLayoutQueryKind;
use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};

/// Post-Build representation choices are inputs to the layout authority,
/// never a host target selection. Only answers the layout producer computes
/// without reading target geometry may enter the portable source judgment.
pub struct SourceLayoutQueries<'a> {
    pub opaque_representation_selections:
        &'a [representation_planning::OpaqueRepresentationSelection],
    pub root_bindings: &'a [crate::program_entry::RootBinding],
    pub provider_selections: &'a [crate::ProviderSelection],
}

impl build_time_evaluation::LayoutQueryPreparation for SourceLayoutQueries<'_> {
    fn prepare(
        &self,
        program: &mut TypedTrees,
        selection: &typed_trees::references::DeclarationSelection,
    ) -> Result<(), Vec<Diagnostic>> {
        fold_queries(
            program,
            self.opaque_representation_selections,
            selection,
            QueryScope::Invariant,
        )
        .map(|_| ())
    }

    fn context_roots(
        &self,
    ) -> Result<Vec<(typed_trees::name::Identifier, symbols::SymbolHandle)>, Vec<Diagnostic>> {
        let mut roots = Vec::new();
        for binding in self.root_bindings {
            let (owner, _) = binding.slot.rsplit_once("::").ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "root slot `{}` lost its target owner",
                    binding.slot
                ))]
            })?;
            let profile = target::TargetProfile::from_root_slot_owner(owner)
                .map_err(|diagnostic| vec![diagnostic])?;
            roots.push((
                typed_trees::name::Identifier::from(profile.target_name()),
                binding.implementation_symbol,
            ));
            // An executed whole-provider choice retains its complete nominal
            // implementation map as Product surface, including uncalled rows.
            // Psi's ordinary declaration traversal follows that exact map's
            // inline members and type dependencies. This supplies a context
            // for formation, never provider admission or private guarantees.
            for selection in self.provider_selections {
                roots.push((
                    typed_trees::name::Identifier::from(profile.target_name()),
                    selection.provider_type.symbol,
                ));
            }
        }
        // A library with no entry has no implicit deployment. Its authored
        // target bodies still supply contexts through Psi's dependency owner.
        Ok(roots)
    }

    fn library_contexts(&self) -> Vec<typed_trees::name::Identifier> {
        if !self.root_bindings.is_empty() {
            return Vec::new();
        }
        target::PhysicalLayoutContext::catalog()
            .map(|context| typed_trees::name::Identifier::from(context.profile().target_name()))
            .collect()
    }

    fn prepare_context(
        &self,
        program: &mut TypedTrees,
        selection: &typed_trees::references::DeclarationSelection,
        target: &str,
    ) -> Result<LayoutContextInputs, Vec<Diagnostic>> {
        if !typed_trees::references::collect_selected_program_references(program, selection)
            .expressions.into_iter().any(|expression| matches!(program.expression_table.expression(expression), ExpressionNode::Call(call) if call.layout_query.is_some())) {
            return Ok(LayoutContextInputs::Ready);
        }
        let profile = target::TargetProfile::from_canonical_target_name(target)
            .map_err(|diagnostic| vec![diagnostic])?;
        let context = target::PhysicalLayoutContext::from_profile(profile).ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "layout context `{target}` requires an explicit physical deployment target"
            ))]
        })?;
        fold_queries(
            program,
            self.opaque_representation_selections,
            selection,
            QueryScope::Selected {
                target: context.profile(),
                native: context.native_target(),
                allow_pending: true,
            },
        )
    }
}

#[derive(Clone, Copy)]
enum QueryScope {
    Invariant,
    Selected {
        target: target::TargetProfile,
        native: target::NativeTarget,
        allow_pending: bool,
    },
}

/// Fold every selected declaration's retained `size_of`/`align_of` query in the typed program to
/// its `u64` literal under `target`. Psi then completes each retained scalar
/// initializer through ordinary semantic evaluation, including derived expressions.
///
/// A plan-laid operand's layout exists only once the deferred evaluation
/// installs the validated plan (`compute_plan_laid_layouts_in`). With
/// `defer_uninstalled_plans`, the actual layout traversal retains queries that
/// still need array extents or policy geometry, including nested dependencies.
/// The post-evaluation pass folds them after those producers finish. With `false` every query
/// resolves or reports the layout authority's diagnostic. `selection` owns
/// evaluation work, so canonical Product descriptions remain untouched while
/// Build is evaluated on the same arenas.
pub fn fold_data_layout_queries(
    program: &mut TypedTrees,
    target: target::TargetProfile,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    defer_uninstalled_plans: bool,
    selection: &checked_trees::typed_trees::references::DeclarationSelection,
) -> Result<(), Vec<Diagnostic>> {
    fold_queries(
        program,
        opaque_representation_selections,
        selection,
        QueryScope::Selected {
            target,
            native: target.native_target(),
            allow_pending: defer_uninstalled_plans,
        },
    )
    .and_then(|inputs| match inputs {
        LayoutContextInputs::Ready | LayoutContextInputs::Pending => Ok(()),
        LayoutContextInputs::Unavailable { application } => Err(vec![Diagnostic::error(format!(
            "layout application `{}` has no input geometry in context `{}`",
            program.symbols.name(application),
            target.target_name(),
        ))]),
    })
}

fn fold_queries(
    program: &mut TypedTrees,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    selection: &checked_trees::typed_trees::references::DeclarationSelection,
    scope: QueryScope,
) -> Result<LayoutContextInputs, Vec<Diagnostic>> {
    // The proof-only classification is computed over the typed program;
    // a `size_of<Nat>` over recursive data must name the
    // classification instead of reporting the layout builder's zero default.
    let proof_only = checked_trees::typed_trees::proof_only::classify(program);
    let owned_expressions =
        checked_trees::typed_trees::references::collect_selected_program_references(
            program, selection,
        )
        .expressions;
    let queries: Vec<(
        checked_trees::typed_trees::expression::ExpressionHandle,
        checked_trees::typed_trees::expression::DataLayoutQueryRequest,
        source::SourceSpan,
    )> = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| owned_expressions.contains(handle))
        .filter_map(|(handle, node)| match node {
            ExpressionNode::Call(call) => call.layout_query.as_ref().map(|query| {
                (
                    handle,
                    query.clone(),
                    program.expression_table.source_span(handle),
                )
            }),
            _ => None,
        })
        .collect();
    // The physical backend consumes invariant geometry. A family needs one
    // private layout view closed under this exact profile; source alternatives
    // remain intact for Terminal production. All queries borrow that same view.
    let selected_target = match scope {
        QueryScope::Selected { target, .. } => Some(target),
        QueryScope::Invariant => None,
    };
    let layout_program = selected_target
        .filter(|_| {
            program.plan_laid_layouts.iter().any(|plan| {
                matches!(
                    plan.realizations,
                    checked_trees::typed_trees::PlanLaidRealizations::TargetFamily(_)
                        | checked_trees::typed_trees::PlanLaidRealizations::TargetContexts { .. }
                )
            })
        })
        .map(|target| {
            let mut closed = program.clone();
            for (plan, authored) in closed
                .plan_laid_layouts
                .iter_mut()
                .zip(&program.plan_laid_layouts)
            {
                if let Some(geometry) =
                    authored.geometry_for_target(program, Some(target.target_name()))
                {
                    plan.realizations = checked_trees::typed_trees::PlanLaidRealizations::Invariant(
                        geometry.clone(),
                    );
                }
            }
            closed
        });
    let mut inputs = LayoutContextInputs::Ready;
    for (handle, query, reference) in queries {
        let operand = query.operand.type_reference;
        let spelling = query.kind.target_name();
        if let Some(held) = proof_only.proof_only_mention(program, operand) {
            return Err(vec![
                Diagnostic::error(format!(
                    "`{spelling}` cannot measure a proof-only carrier: `{held}` has no runtime layout"
                ))
                .with_source_span(reference),
            ]);
        }
        if matches!(
            scope,
            QueryScope::Selected {
                allow_pending: false,
                ..
            }
        ) {
            reject_unresolved_layout_operands(program, operand)
                .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(reference)])?;
        }
        let measured = layout::query_type_reference_layout(
            layout_program.as_ref().unwrap_or(program),
            match scope {
                QueryScope::Selected { native, .. } => Some(native),
                QueryScope::Invariant => None,
            },
            opaque_representation_selections,
            operand,
        )
        .map_err(|error| vec![error])?;
        let geometry = match measured {
            layout::TypeLayoutQuery::Ready(geometry) => geometry,
            layout::TypeLayoutQuery::Pending(prerequisite) => {
                // Availability is an input roster fact, not an evaluation
                // failure: a completed guarded dependency has no such member.
                // Unfinished plans/extents remain pending and cannot disappear
                // from the library domain merely because they are not ready.
                let unavailable = match (selected_target, prerequisite) {
                    (Some(target), layout::LayoutQueryPrerequisite::PlanGeometry(application)) => {
                        program
                            .plan_laid_layout(application)
                            .filter(|plan| {
                                plan.is_materialized()
                                    && plan
                                        .geometry_for_target(program, Some(target.target_name()))
                                        .is_none()
                            })
                            .map(|_| application)
                    }
                    _ => None,
                };
                if let Some(application) = unavailable {
                    inputs = LayoutContextInputs::Unavailable { application };
                    continue;
                }
                if inputs == LayoutContextInputs::Ready {
                    inputs = LayoutContextInputs::Pending;
                }
                if !matches!(
                    scope,
                    QueryScope::Selected {
                        allow_pending: false,
                        ..
                    }
                ) {
                    continue;
                }
                let reason = match prerequisite {
                    layout::LayoutQueryPrerequisite::ArrayExtent(_) => format!(
                        "`{spelling}` cannot measure a carrier with an unresolved array extent"
                    ),
                    layout::LayoutQueryPrerequisite::PlanGeometry(application) => format!(
                        "`{spelling}` awaits validated layout geometry for `{}`",
                        program.symbols.name(application),
                    ),
                    layout::LayoutQueryPrerequisite::TargetGeometry => {
                        format!("`{spelling}` requires exact target layout geometry")
                    }
                };
                return Err(vec![Diagnostic::error(reason).with_source_span(reference)]);
            }
        };
        let extent = match query.kind {
            DataLayoutQueryKind::Size => geometry.size,
            DataLayoutQueryKind::Alignment => geometry.alignment,
        };
        let value = u64::try_from(extent).map_err(|_| {
            vec![
                Diagnostic::error(format!(
                    "`{spelling}` layout extent exceeds the `u64` result carrier"
                ))
                .with_source_span(reference),
            ]
        })?;
        let literal = IntegerLiteral::from_parts(false, IntegerRadix::Decimal, &value.to_string())
            .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(reference)])?
            .with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U64,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            });
        *program.expression_table.expression_mut(handle) = ExpressionNode::Integer(literal);
    }
    Ok(inputs)
}

/// A `layout_type_reference` of an unresolved fixed array returns the
/// builder's `(0, 1)` default — bit-identical to a valid zero-sized layout.
/// Walk the operand's type-reference tree first so `size_of<[u8; N]>()` with
/// an unbound `N` rejects as "no fixed size" instead of silently answering
/// `size_of` on a phantom zero-extent carrier.
fn reject_unresolved_layout_operands(
    typed: &checked_trees::typed_trees::TypedTrees,
    root: TypeReferenceHandle,
) -> Result<(), String> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(reference) = pending.pop() {
        if !reference.is_valid() || visited.contains(&reference) {
            continue;
        }
        visited.push(reference);
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                match length {
                    FixedArrayLength::Literal(_) => {}
                    FixedArrayLength::Expression(_) => return Err("`size_of`/`align_of` cannot measure an array with an unresolved extent expression".into()),
                    // `ConstCall` lengths must already have folded to
                    // `Literal` before settlement; a surviving non-literal
                    // length means the layout was never resolved.
                    FixedArrayLength::ConstParameter { name, .. } => {
                        return Err(format!(
                            "`size_of`/`align_of` cannot measure a fixed array: \
                             the length `{name}` is not resolved to a constant"
                        ));
                    }
                    FixedArrayLength::ConstCall { .. }
                        if typed.closed_fixed_array_length(reference).is_some() => {}
                    FixedArrayLength::ConstCall { name, .. } => {
                        return Err(format!(
                            "`size_of`/`align_of` cannot measure a fixed array whose \
                             length `{name}` never evaluated to a constant"
                        ));
                    }
                }
                pending.push(*element_type);
            }
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
            TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            TypeReferenceNode::Generic { arguments, .. } => {
                pending.extend(
                    typed
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .copied(),
                );
            }
            TypeReferenceNode::ConstExpression(_) => {
                return Err(
                    "`size_of`/`align_of` takes a data carrier, not a proof-static \
                     const expression"
                        .to_owned(),
                );
            }
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => {}
        }
    }
    Ok(())
}
