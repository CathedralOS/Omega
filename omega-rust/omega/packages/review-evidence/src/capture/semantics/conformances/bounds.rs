use crate::capture::PackageReviewInput;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::types::review_signature_type_identity_with_binders;
use crate::record::PackageReviewConformanceBound;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn project_conformance_bounds(
    compilation: &PackageReviewInput<'_>,
    bounds: &[typed_trees::machine::GenericConformanceBound],
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[typed_trees::name::Identifier],
    declaration_kind: &str,
    declaration_path: &str,
) -> Result<Vec<PackageReviewConformanceBound>, Vec<Diagnostic>> {
    let mut projected = Vec::with_capacity(bounds.len());
    for (binder_ordinal, bound) in bounds.iter().enumerate() {
        if !bound.binder.is_valid() {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` has an unresolved conformance evidence binder"
            ))]);
        }
        let binder_ordinal = u32::try_from(binder_ordinal).map_err(|_| {
            vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` has too many conformance binders for portable review evidence"
            ))]
        })?;
        let trait_symbol = bound.trait_symbol;
        let trait_lifetime_arguments = Vec::new();
        let trait_arguments = bound
            .arguments
            .iter()
            .map(|argument| {
                review_signature_type_identity_with_binders(
                    compilation,
                    *argument,
                    binders,
                    lifetime_binders,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let matching_traits = compilation
            .traits()
            .iter()
            .filter(|definition| definition.symbol == trait_symbol)
            .collect::<Vec<_>>();
        let [trait_definition] = matching_traits.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` conformance bound resolves to {} traits; expected exactly one",
                matching_traits.len()
            ))]);
        };
        if !trait_definition.is_public {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` exposes non-public conformance trait `{}`",
                trait_definition.name
            ))]);
        }
        if trait_lifetime_arguments.len() != trait_definition.lifetime_parameters.len() {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` uses conformance trait `{}` with {} target lifetime argument(s), expected {}",
                trait_definition.name,
                trait_lifetime_arguments.len(),
                trait_definition.lifetime_parameters.len(),
            ))]);
        }
        projected.push(PackageReviewConformanceBound {
            binder_ordinal,
            trait_identity: nominal_identity(compilation, trait_definition.symbol)?,
            trait_lifetime_arguments,
            arguments: trait_arguments,
        });
    }
    Ok(projected)
}
