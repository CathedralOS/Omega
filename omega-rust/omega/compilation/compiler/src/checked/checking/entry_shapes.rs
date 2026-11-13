// Source entry nomination belongs before the frozen source frontier, rather
// than in canonical provider planning. This reads declarations and proves no body.
use diagnostics::Diagnostic;
use provider_planning::program_entry::RootBinding;
use typed_trees::TypedTrees;
use typed_trees::callable_signature_shapes::{BoundCallableShape, ParameterOccurrence};
use typed_trees::product_operand::core_arrival_requirement;

/// Retain every executed binding's visible calling shape, including rows for
/// targets this invocation does not realize. Source custody names the core
/// semantic arrival requirement; physical admission stays with the selected
/// target, so an inactive row does not demand its package acceptance. The physical schema,
/// accepted package digest, backing and effect/resource checks still belong to
/// their existing admission paths; a shape receipt grants none of those.
pub(super) fn bound_program_entry_shapes(
    typed: &TypedTrees,
    bindings: &[RootBinding],
) -> Result<Vec<BoundCallableShape>, Vec<Diagnostic>> {
    let mut shapes = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let resolved = (|| -> Result<_, Diagnostic> {
            let (owner, name) = binding
                .slot
                .rsplit_once("::")
                .ok_or_else(|| Diagnostic::error("root binding has no target-qualified slot"))?;
            let profile = target::TargetProfile::from_root_slot_owner(owner)
                .map_err(|_| Diagnostic::error(format!("unknown root slot owner `{owner}`")))?;
            let slot = profile
                .required_root_slot(name)
                .and_then(|slot| slot.program_entry())
                .ok_or_else(|| {
                    Diagnostic::error(format!("unsupported program root slot `{}`", binding.slot))
                })?;
            let (requirement_owner, requirement) =
                core_arrival_requirement(typed, slot.semantic_arrival_requirement)
                    .map_err(Diagnostic::error)?;
            let visible_parameters = match slot.source_contract().visible_parameters {
                semantic_vocabulary::ProgramEntryVisibleParameters::None => Vec::new(),
                semantic_vocabulary::ProgramEntryVisibleParameters::ImageAndInitialStorage => typed
                    .state_signature_parameters(requirement)
                    .iter()
                    .enumerate()
                    .map(|(ordinal, parameter)| ParameterOccurrence {
                        ordinal,
                        symbol: parameter.symbol,
                    })
                    .collect(),
            };
            let mut implementation = binding.implementation_symbol;
            if let Some(family) = typed.target_machine_families.iter().find(|family| {
                family.canonical == implementation
                    || family
                        .members
                        .iter()
                        .any(|member| member.machine == implementation)
            }) {
                implementation = family
                    .members
                    .iter()
                    .find(|member| member.target.as_str() == profile.target_name())
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "entry family `{}` has no body for target `{}`",
                            family.name,
                            profile.target_name()
                        ))
                    })?
                    .machine;
            }
            Ok(BoundCallableShape {
                slot: binding.slot.clone(),
                arrival_description: match slot.source_contract().visible_parameters {
                    semantic_vocabulary::ProgramEntryVisibleParameters::None => {
                        "hosted `ProgramEntry` exposes no arrival parameters"
                    }
                    semantic_vocabulary::ProgramEntryVisibleParameters::ImageAndInitialStorage => {
                        "target schema exposes exactly image and initial-storage roots"
                    }
                },
                source_span: binding.source_span,
                implementation,
                semantic_schema_symbol: requirement_owner.symbol,
                requirement_symbol: requirement.symbol,
                semantic_schema: symbols::RetainedDeclaration::capture(
                    &typed.symbols,
                    requirement_owner.symbol,
                )
                .map_err(Diagnostic::error)?,
                requirement: symbols::RetainedDeclaration::capture(
                    &typed.symbols,
                    requirement.symbol,
                )
                .map_err(Diagnostic::error)?,
                visible_parameters,
            })
        })();
        match resolved {
            Ok(shape) => shapes.push(shape),
            Err(mut diagnostic) => {
                if let Some(span) = binding.source_span {
                    diagnostic = diagnostic.with_source_span(span);
                }
                return Err(vec![diagnostic]);
            }
        }
    }
    Ok(shapes)
}
