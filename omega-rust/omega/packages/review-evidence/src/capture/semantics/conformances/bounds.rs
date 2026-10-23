use crate::capture::PackageReviewInput;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::types::signature_type_identity;
use crate::record::PackageReviewConformanceBound;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use validation::CheckingProjection;

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
        // The trait owns each argument's category. A const value is not a
        // nominal structural type: reify exact checked named constants through
        // the same canonical-value projector used by selected conformance
        // policy. Borrow source custody from the original checked input.
        let parameters = compilation.trait_type_parameters(trait_definition);
        if parameters.len() != bound.arguments.len() {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` conformance bound has {} static argument(s), expected {}",
                bound.arguments.len(),
                parameters.len(),
            ))]);
        }
        let mut instantiated = CheckingProjection::new(compilation.checking_input);
        let mut trait_arguments = Vec::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(&bound.arguments) {
            let const_argument = parameter.kind.is_const_or_value();
            let reference = if const_argument
                && matches!(
                    compilation.type_reference_table.type_reference(*argument),
                    typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                        if symbol.is_valid() && compilation.symbols.get(*symbol).kind == symbols::SymbolKind::Const
                ) {
                let selected = compilation
                    .checking_input
                    .typed_program()
                    .static_operand_from_type_reference(*argument)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                super::argument_type_reference(
                    instantiated.program_mut(),
                    &selected,
                    &parameter.kind,
                    0,
                )?
            } else {
                *argument
            };
            trait_arguments.push(signature_type_identity(
                compilation, &instantiated,
                compilation.custody.sources().exact_toolchain_sources(),
                reference, binders, lifetime_binders, &[], &[], const_argument,
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
            )?);
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
