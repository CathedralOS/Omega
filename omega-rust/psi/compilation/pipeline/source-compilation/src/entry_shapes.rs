// Source entry nomination belongs before the frozen source frontier, rather
// than in canonical provider planning. This reads declarations and proves no body.
use crate::source_state::TypedTrees;
use crate::source_state::callable_signature_shapes::{BoundCallableShape, ParameterOccurrence};
use crate::source_state::product_operand::core_arrival_requirement;
use declaration_contracts::selection::RootBinding;
use diagnostics::Diagnostic;

/// Retain every executed binding's visible calling shape, including rows for
/// targets this invocation does not realize. Source custody names the core
/// semantic arrival requirement; physical admission stays with the selected
/// target, so an inactive row does not demand its package acceptance. The physical schema,
/// accepted package digest, backing and effect/resource checks still belong to
/// their existing admission paths; a shape receipt grants none of those.
pub fn bound_program_entry_shapes(
    typed: &TypedTrees,
    bindings: &[RootBinding],
    targets: &[crate::source_targets::SourceTarget],
) -> Result<Vec<BoundCallableShape>, Vec<Diagnostic>> {
    let mut shapes = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let resolved = (|| -> Result<_, Diagnostic> {
            let (owner, name) = binding
                .slot
                .rsplit_once("::")
                .ok_or_else(|| Diagnostic::error("root binding has no target-qualified slot"))?;
            let profile = crate::source_targets::target_owner(targets, owner)
                .map_err(|_| Diagnostic::error(format!("unknown root slot owner `{owner}`")))?;
            let slot = &profile.entry;
            if slot.slot_name != name {
                return Err(Diagnostic::error(format!(
                    "unsupported program root slot `{}`",
                    binding.slot
                )));
            }
            let (requirement_owner, requirement) =
                core_arrival_requirement(typed, slot.semantic_arrival_requirement)
                    .map_err(Diagnostic::error)?;
            let visible_parameters = match slot.visible_parameters {
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
            let implementation = binding.implementation_symbol;
            Ok(BoundCallableShape {
                slot: binding.slot.clone(),
                arrival_description: match slot.visible_parameters {
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
                semantic_schema: crate::source_symbols::retain_declaration(
                    &typed.symbols,
                    requirement_owner.symbol,
                )
                .map_err(Diagnostic::error)?,
                requirement: crate::source_symbols::retain_declaration(
                    &typed.symbols,
                    requirement.symbol,
                )
                .map_err(Diagnostic::error)?,
                visible_parameters,
                has_entry_bridge: profile.entry_contract.is_some(),
                address_model: profile.address_model.clone(),
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
