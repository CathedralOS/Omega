// Retained semantic dependency edge projection with package-grouped private
// implementation consumers. Automatic cleanup machine edges (`dropped by`) project
// the exact selected cleanup hook rather than a declaration family, while
// materialized generic carriers follow their checked nominal application metadata
// (including nominal type arguments) rather than publishing generated carrier names.

use crate::capture::PackageReviewInput;
use crate::capture::semantics::conformances::policy_callable_identity;
use crate::capture::semantics::declarations::{
    is_checked_source_instance, nominal_identity, nominal_identity_from_symbols,
    reviewed_package_owns,
};
use crate::capture::semantics::facts::exactly_one;
use crate::record::{
    PackagePolicyCallables, PackagePolicySemanticDependency,
    PackagePolicySemanticDependencyConsumer, PackageReviewSemanticDependencyExposure,
    PackageReviewSemanticDependencyKind,
};
use checked_trees::CheckedSemanticDependencyExposure;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn project(
    _compilation: &PackageReviewInput<'_>,
    _package: PackageKeyIdentity,
    _callables: &PackagePolicyCallables,
) -> Result<Vec<PackagePolicySemanticDependency>, Vec<Diagnostic>> {
    Err(vec![diagnostics::Diagnostic::error(
        "no product route yet: package review requires lowered-form evidence",
    )])
}

fn nominal_declarations(
    compilation: &PackageReviewInput<'_>,
    root: SymbolHandle,
) -> Result<Vec<crate::record::PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    enum Visit {
        Symbol(SymbolHandle),
        Type(TypeReferenceHandle),
        EndSymbol(SymbolHandle),
        EndType(TypeReferenceHandle),
    }
    let invalid = |reason| super::rejected(reason);
    let mut pending = vec![Visit::Symbol(root)];
    let (mut active_symbols, mut complete_symbols) = (Vec::new(), Vec::new());
    let (mut active_types, mut complete_types) = (Vec::new(), Vec::new());
    let mut output = Vec::new();
    while let Some(visit) = pending.pop() {
        match visit {
            Visit::Symbol(symbol) => {
                if !symbol.is_valid() {
                    continue;
                }
                if !compilation.typed.symbols.symbols().nodes().is_valid(symbol)
                    || !compilation
                        .custody
                        .typed
                        .symbols
                        .symbols()
                        .nodes()
                        .is_valid(symbol)
                {
                    return Err(invalid("semantic dependency has no checked nominal symbol"));
                }
                if matches!(
                    compilation.typed.symbols.get(symbol).kind,
                    SymbolKind::BuiltinType
                        | SymbolKind::TypeParameter
                        | SymbolKind::ConformanceParameter
                        | SymbolKind::MachineParameter
                        | SymbolKind::PropositionParameter
                        | SymbolKind::PropositionMachineParameter
                ) || complete_symbols.contains(&symbol)
                {
                    continue;
                }
                if active_symbols.contains(&symbol) {
                    return Err(invalid(
                        "semantic dependency has a cyclic nominal application",
                    ));
                }
                let candidate = validation::unique_data_definition_by_symbol(
                    compilation.checking_input,
                    symbol,
                );
                let custody = validation::unique_data_definition_by_symbol(
                    compilation.original_checking_input,
                    symbol,
                );
                let application = match (
                    candidate.map(|data| data.generic_instance),
                    custody.map(|data| data.generic_instance),
                ) {
                    (Some(Some(candidate)), Some(Some(original))) => {
                        if !compilation
                            .type_reference_table
                            .contains_type_reference(candidate)
                            || !compilation
                                .custody
                                .typed
                                .type_reference_table
                                .contains_type_reference(original)
                            || compilation.typed.normalized_type_identity(candidate)
                                != compilation.custody.typed.normalized_type_identity(original)
                        {
                            return Err(invalid(
                                "semantic dependency application differs from checked custody",
                            ));
                        }
                        Some(candidate)
                    }
                    (Some(None), Some(None)) | (None, None) => None,
                    _ => {
                        return Err(invalid(
                            "semantic dependency lost its checked nominal application",
                        ));
                    }
                };
                let identity = nominal_identity(compilation, symbol)?;
                let original = nominal_identity_from_symbols(
                    &compilation.custody.typed.symbols,
                    symbol,
                    compilation.custody.sources().exact_toolchain_sources(),
                )?;
                if identity.owner != original.owner
                    || (application.is_none() && identity != original)
                {
                    return Err(invalid(
                        "semantic dependency declaration differs from checked custody",
                    ));
                }
                if let Some(application) = application {
                    active_symbols.push(symbol);
                    pending.push(Visit::EndSymbol(symbol));
                    pending.push(Visit::Type(application));
                } else {
                    complete_symbols.push(symbol);
                    output.push(identity);
                }
            }
            Visit::Type(reference) => {
                if !compilation
                    .type_reference_table
                    .contains_type_reference(reference)
                {
                    return Err(invalid(
                        "semantic dependency application has no checked type",
                    ));
                }
                if complete_types.contains(&reference) {
                    continue;
                }
                if active_types.contains(&reference) {
                    return Err(invalid("semantic dependency has a cyclic type application"));
                }
                active_types.push(reference);
                pending.push(Visit::EndType(reference));
                match compilation.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Reference { referee, .. }
                    | TypeReferenceNode::Constrained {
                        base_type: referee, ..
                    } => pending.push(Visit::Type(*referee)),
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => {
                        pending.push(Visit::Type(*element_type))
                    }
                    TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    }
                    | TypeReferenceNode::DynamicTrait {
                        symbol: base_symbol,
                        arguments,
                        ..
                    } => {
                        pending.push(Visit::Symbol(*base_symbol));
                        for argument in compilation
                            .type_reference_table
                            .type_reference_handles(*arguments)
                        {
                            pending.push(Visit::Type(*argument));
                        }
                    }
                    TypeReferenceNode::Named { symbol, .. } => pending.push(Visit::Symbol(*symbol)),
                    TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {}
                }
            }
            Visit::EndSymbol(symbol) => {
                if active_symbols.pop() != Some(symbol) {
                    return Err(invalid("semantic dependency nominal walk lost its owner"));
                }
                complete_symbols.push(symbol);
            }
            Visit::EndType(reference) => {
                if active_types.pop() != Some(reference) {
                    return Err(invalid("semantic dependency type walk lost its owner"));
                }
                complete_types.push(reference);
            }
        }
    }
    Ok(output)
}
