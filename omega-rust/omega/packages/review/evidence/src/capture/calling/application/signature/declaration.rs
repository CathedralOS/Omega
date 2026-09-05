//! An uninstantiated service telescope is not a closed calling application.
//!
//! Root arguments retain declaration-order symbolic coordinates in a separate
//! namespace from each requirement's own static parameters. Checked expression
//! handles and declarations remain unchanged; only temporary type references
//! and scope-normalized signature copies are allocated.

use super::*;
use symbols::SymbolHandle;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::TypeReferenceNode;

fn root(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<&TraitDefinition, Vec<Diagnostic>> {
    let mut roots = compilation
        .traits()
        .iter()
        .filter(|owner| owner.symbol == symbol && owner.is_boundary);
    let root = roots
        .next()
        .ok_or_else(|| rejected("service declaration has no exact boundary trait"))?;
    if roots.next().is_some() {
        return Err(rejected("service declaration repeats its boundary trait"));
    }
    Ok(root)
}

pub(crate) fn declaration_parameters(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<(Vec<PackagePolicyTypeParameter>, u32), Vec<Diagnostic>> {
    let owner = root(compilation, symbol)?;
    let mut projected = compilation.clone();
    let mut parameters = compilation.trait_type_parameters(owner).to_vec();
    let lifetimes = owner
        .lifetime_parameters
        .iter()
        .cloned()
        .map(|name| (name.clone(), name))
        .collect::<Vec<_>>();
    let mut scopes = Vec::new();
    parameters::instantiate(
        &mut projected,
        &mut parameters,
        &[],
        &lifetimes,
        &owner.lifetime_parameters,
        &mut scopes,
        0,
    )?;
    let (_, parameters) = project_type_parameters(
        &projected,
        compilation,
        &parameters,
        compilation.trait_type_parameters(owner),
        owner.name.as_str(),
        &[],
        0,
        &owner.lifetime_parameters,
        &[],
        &scopes,
        false,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
    )?;
    Ok((parameters, count(owner.lifetime_parameters.len())?))
}

pub(crate) fn project_declaration(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
    requirement: SymbolHandle,
) -> Result<CallingSignatureProjection, Vec<Diagnostic>> {
    let owner = root(compilation, symbol)?;
    let mut projected = compilation.clone();
    let mut arguments = Vec::new();
    let mut binders = Vec::new();
    for (ordinal, parameter) in compilation.trait_type_parameters(owner).iter().enumerate() {
        if !parameter.symbol.is_valid()
            || binders
                .iter()
                .any(|(symbol, _)| *symbol == parameter.symbol)
        {
            return Err(rejected(
                "service declaration has missing or duplicate static parameter identity",
            ));
        }
        arguments.push(
            projected
                .typed
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: parameter.symbol,
                    name: parameter.name.clone(),
                }),
        );
        binders.push((parameter.symbol, format!("service-parameter:{ordinal}")));
    }
    project_with_binders(&projected, symbol, &arguments, requirement, &binders)
}
