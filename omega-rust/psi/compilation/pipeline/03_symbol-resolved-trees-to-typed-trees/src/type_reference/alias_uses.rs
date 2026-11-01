//! Erase source alias syntax while retaining its own argument formation.

use diagnostics::Diagnostic;
use source::SourceSpan;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(super) fn retain_application(
    source: &resolved::SymbolResolvedTrees,
    target: &mut typed::TypedTrees,
    expansion: typed::types::TypeReferenceHandle,
    application: typed::types::TypeReferenceHandle,
    declaration_source: SourceSpan,
    reference: SourceSpan,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    use typed::types::TypeReferenceNode;
    let (declaration, arguments, lifetime_arguments) =
        match target.type_reference_table.type_reference(application) {
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } => (*base_symbol, *arguments, lifetime_arguments.clone()),
            TypeReferenceNode::Named { symbol, .. } => {
                (*symbol, arena::HandleSpan::empty(), Vec::new())
            }
            _ => {
                return Err(Diagnostic::error(
                    "type alias application lost its declaration head",
                ));
            }
        };
    if source.symbols.get(declaration).kind != symbols::SymbolKind::TypeAlias
        || source.symbols.symbol_source_span(declaration) != Some(declaration_source)
    {
        return Err(Diagnostic::error(
            "type alias application does not rejoin its selected source declaration",
        )
        .with_source_span(reference));
    }
    let supplied_operand_owner = target
        .type_reference_table
        .operand_lifetime_owner(application);
    let instance = target.type_reference_table.fresh_occurrence(expansion);
    target.type_reference_table.retain_type_alias_application(
        typed::type_aliases::TypeAliasApplication {
            instance,
            application,
            declaration,
            declaration_source,
            reference,
            arguments,
            lifetime_arguments,
            supplied_operand_owner,
        },
    );
    Ok(instance)
}
