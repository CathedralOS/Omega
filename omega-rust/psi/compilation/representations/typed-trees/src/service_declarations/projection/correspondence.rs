//! Check the source-emitted correspondence without selecting rows by rendered text.
//!
//! Catalog admission historically follows trait requirement edges, not refinement
//! edges, and rejects missing/ambiguous declarations even for an empty surface.
//! Keep that narrower custody check distinct from ordinary surface projection;
//! the latter may include refinement methods and remains usable independently.

use super::{ProjectedServiceSurface, ProviderSchemaDeclaration};
use crate::TypedTrees;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(super) fn validate(
    program: &TypedTrees,
    projection: &ProjectedServiceSurface,
) -> Result<Vec<SymbolHandle>, Diagnostic> {
    if projection.declaration.methods.len() != projection.requirements.len() {
        return Err(Diagnostic::error(
            "service row lost its ordered source correspondence",
        ));
    }
    if !matches!(
        projection.schema,
        ProviderSchemaDeclaration::BoundaryTrait(_)
    ) {
        return Ok(projection
            .requirements
            .iter()
            .map(|source| source.symbol)
            .collect());
    }
    let mut pending = vec![projection.schema.symbol()];
    let mut visited = Vec::new();
    let mut declarations = Vec::new();
    while let Some(symbol) = pending.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let mut matches = program
            .traits()
            .iter()
            .filter(|declaration| declaration.symbol == symbol);
        let Some(declaration) = matches.next() else {
            return Err(Diagnostic::error(
                "service inheritance has no exact declaration",
            ));
        };
        if matches.next().is_some() {
            return Err(Diagnostic::error(
                "service inheritance has ambiguous declarations",
            ));
        }
        declarations.push(declaration);
        pending.extend(
            program
                .trait_requirements(declaration)
                .iter()
                .map(|parent| parent.symbol),
        );
    }
    let mut requirements = Vec::with_capacity(projection.requirements.len());
    for (method, source) in projection
        .declaration
        .methods
        .iter()
        .zip(&projection.requirements)
    {
        let owner = program
            .trait_definition_by_symbol(source.declaring_trait)
            .ok_or_else(|| Diagnostic::error("service row lost its exact declaring trait"))?;
        let signature = program
            .trait_machine_signature_by_symbol(owner, source.symbol)
            .ok_or_else(|| Diagnostic::error("service row lost its exact declared requirement"))?;
        let identity = program.trait_requirement_callable_identity(owner, signature);
        let mut matches = declarations.iter().flat_map(|declaration| {
            program
                .trait_machine_signatures(declaration)
                .iter()
                .filter_map(move |candidate| {
                    (program.symbols.symbol_package_identity(declaration.symbol)
                        == method.requirement_owner_package_identity
                        && program.symbols.symbol_package_identity(candidate.symbol)
                            == method.requirement_owner_package_identity
                        && program.trait_requirement_callable_identity(declaration, candidate)
                            == identity)
                        .then_some(candidate.symbol)
                })
        });
        if matches.next() != Some(source.symbol) || matches.next().is_some() {
            return Err(Diagnostic::error(
                "service row has no unique exact inherited requirement",
            ));
        }
        requirements.push(source.symbol);
    }
    Ok(requirements)
}
