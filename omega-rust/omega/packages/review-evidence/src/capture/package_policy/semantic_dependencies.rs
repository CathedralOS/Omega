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
use typed_trees_to_checked_trees::derive_checked_semantic_dependencies;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    package: PackageKeyIdentity,
    callables: &PackagePolicyCallables,
) -> Result<Vec<PackagePolicySemanticDependency>, Vec<Diagnostic>> {
    let derived =
        derive_checked_semantic_dependencies(compilation.checking_input, &compilation.facts);
    if derived != compilation.facts.flow.semantic_dependencies {
        return Err(super::rejected(
            "semantic dependencies differ from exact checked rederivation",
        ));
    }
    let mut rows = Vec::new();
    for fact in &derived.rows {
        if !is_checked_source_instance(compilation, fact.consumer_machine)? {
            continue;
        }
        let owner = nominal_identity(compilation, fact.consumer_machine)?;
        if !reviewed_package_owns(&owner, package)? {
            continue;
        }
        let machine = exactly_one(
            compilation
                .machines()
                .iter()
                .filter(|machine| machine.symbol == fact.consumer_machine),
            "policy semantic dependency",
            "consumer machine",
        )?;
        let consumer = if machine.is_public
            || machine.supply_mode.is_boundary_declaration()
            || matches!(
                machine.supply_mode,
                language_semantics::MachineSupplyMode::ExternalRealization { .. }
            )
            || compilation.custody.build().selected_build_machine_symbol == Some(machine.symbol)
        {
            let identity = policy_callable_identity(compilation, machine.symbol)?;
            if !callables
                .callables()
                .iter()
                .any(|callable| *callable.identity() == identity)
            {
                return Err(super::rejected(
                    "public semantic dependency has no normalized callable owner",
                ));
            }
            PackagePolicySemanticDependencyConsumer::Callable(identity)
        } else {
            if fact.exposure != CheckedSemanticDependencyExposure::PrivateImplementation {
                return Err(super::rejected(
                    "private semantic consumer has public interface exposure",
                ));
            }
            PackagePolicySemanticDependencyConsumer::PackageImplementation
        };
        let dependencies =
            if fact.kind == checked_trees::CheckedSemanticDependencyKind::AutomaticCleanupMachine {
                vec![policy_callable_identity(compilation, fact.dependency)?]
            } else {
                nominal_declarations(compilation, fact.dependency)?
            };
        for dependency in dependencies {
            rows.push(PackagePolicySemanticDependency {
                consumer: consumer.clone(),
                dependency,
                exposure: match fact.exposure {
                    CheckedSemanticDependencyExposure::PrivateImplementation => {
                        PackageReviewSemanticDependencyExposure::PrivateImplementation
                    }
                    CheckedSemanticDependencyExposure::PublicInterface => {
                        PackageReviewSemanticDependencyExposure::PublicInterface
                    }
                },
                kind: match fact.kind {
                    checked_trees::CheckedSemanticDependencyKind::NominalIdentity => {
                        PackageReviewSemanticDependencyKind::NominalIdentity
                    }
                    checked_trees::CheckedSemanticDependencyKind::Layout => {
                        PackageReviewSemanticDependencyKind::Layout
                    }
                    checked_trees::CheckedSemanticDependencyKind::OwnershipBehavior => {
                        PackageReviewSemanticDependencyKind::OwnershipBehavior
                    }
                    checked_trees::CheckedSemanticDependencyKind::AutomaticCleanup => {
                        PackageReviewSemanticDependencyKind::AutomaticCleanup
                    }
                    checked_trees::CheckedSemanticDependencyKind::AutomaticCleanupMachine => {
                        PackageReviewSemanticDependencyKind::AutomaticCleanupMachine
                    }
                },
            });
        }
    }
    rows.sort();
    rows.dedup();
    Ok(rows)
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
                    TypeReferenceNode::Callable { contract } => {
                        compilation
                            .runtime_callable_contract_identity(*contract, &[])
                            .map_err(|reason| invalid(reason.reason()))?;
                        let inputs = typed_trees::references::type_reference_inputs(
                            &compilation.typed,
                            reference,
                        );
                        for symbol in inputs.symbols {
                            if compilation
                                .typed
                                .type_reference_mentions_any(reference, &[symbol])
                            {
                                // The shared inventory retains code selection
                                // separately from invocation; this is a nominal
                                // semantic dependency, never a call roster.
                                pending.push(Visit::Symbol(symbol));
                            }
                        }
                    }
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
