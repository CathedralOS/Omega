// Source entry nomination belongs before the frozen source frontier, rather
// than in canonical provider planning. This reads declarations and proves no body.
use crate::source_state::TypedTrees;
use crate::source_state::callable_signature_shapes::{BoundCallableShape, ParameterOccurrence};
use crate::source_state::product_operand::core_arrival_requirement;
use crate::RootBinding;
use diagnostics::Diagnostic;

/// Resolve every bound slot's owner and name. Only deployed entries project
/// their calling shape and address model onto the semantic arrival requirement.
pub fn bound_program_entry_shapes(
    typed: &TypedTrees,
    bindings: &[RootBinding],
    targets: &[crate::compile_product::source_targets::SourceTarget],
    calling_targets: &[&str],
) -> Result<Vec<BoundCallableShape>, Vec<Diagnostic>> {
    let mut shapes = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let resolved = (|| -> Result<_, Diagnostic> {
            let (owner, name) = binding
                .slot
                .rsplit_once("::")
                .ok_or_else(|| Diagnostic::error("root binding has no target-qualified slot"))?;
            let profile = crate::compile_product::source_targets::target_owner(targets, owner)
                .map_err(|_| Diagnostic::error(format!("unknown root slot owner `{owner}`")))?;
            let slot = &profile.entry;
            if slot.slot_name != name {
                return Err(Diagnostic::error(format!(
                    "unsupported program root slot `{}`",
                    binding.slot
                )));
            }
            if !calling_targets.contains(&profile.name) {
                return Ok(None);
            }
            let (requirement_owner, requirement) =
                core_arrival_requirement(typed, slot.semantic_arrival_requirement)
                    .map_err(Diagnostic::error)?;
            let parameters = typed.state_signature_parameters(requirement);
            let mut visible_parameters = Vec::with_capacity(slot.visible_parameters.len());
            for argument in slot.visible_parameters {
                let ordinal = argument.requirement_parameter;
                let parameter = parameters.get(ordinal).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "installed entry `{}` names missing parameter {ordinal} on `{}`",
                        binding.slot, slot.semantic_arrival_requirement
                    ))
                })?;
                visible_parameters.push(ParameterOccurrence {
                    ordinal,
                    symbol: parameter.symbol,
                });
            }
            let implementation = binding.implementation_symbol;
            Ok(Some(BoundCallableShape {
                slot: binding.slot.clone(),
                arrival_description: if slot.visible_parameters.is_empty() {
                    "hosted `ProgramEntry` exposes no arrival parameters"
                } else {
                    "the installed entry supplies its declared arrival parameters"
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
                address_model: profile.address_model.clone(),
            }))
        })();
        match resolved {
            Ok(Some(shape)) => shapes.push(shape),
            Ok(None) => {}
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
