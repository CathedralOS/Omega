//! Review dependency edges come from retained declarations and actual accepted
//! invocations. Descriptor construction is a dependency without a call edge.

use super::nominal::nominal_identity_from_symbols;
use super::published_input::{PublishedPackageReviewInput, rejected};
use crate::record::*;
use diagnostics::Diagnostic;
use semantic_vocabulary::{IdTable, MachineId, PackageKeyIdentity};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;

type Review<T> = Result<T, Vec<Diagnostic>>;

pub(super) fn project(
    input: PublishedPackageReviewInput<'_>,
    package: PackageKeyIdentity,
    callables: &PackagePolicyCallables,
) -> Review<Vec<PackagePolicySemanticDependency>> {
    let source = input.custody.source_inspection().program();
    let mut rows = packet(
        input.candidate,
        source.module(),
        &source.correspondence().calls,
        &source.correspondence().machines,
        &source.correspondence().boundaries,
        package,
        callables,
        input.custody.sources().exact_toolchain_sources(),
    )?;
    if let Some(build) = &input.custody.build().accepted_build_source {
        let publication = build.publication();
        rows.extend(packet(
            &***build.typed(),
            publication.artifact.semantic_module(),
            &publication.calls,
            &publication.machines,
            &publication.boundaries,
            package,
            callables,
            input.custody.sources().exact_toolchain_sources(),
        )?);
    }
    rows.sort();
    rows.dedup();
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
fn packet(
    typed: &TypedTrees,
    module: &terminal_psi::TerminalModule,
    calls: &[typed_trees::SourceCallCorrespondence],
    machines: &IdTable<MachineId, SymbolHandle>,
    boundaries: &IdTable<semantic_vocabulary::BoundaryMachineId, SymbolHandle>,
    package: PackageKeyIdentity,
    callables: &PackagePolicyCallables,
    exact_sources: &[(source::SourceId, [u8; 32])],
) -> Review<Vec<PackagePolicySemanticDependency>> {
    let mut declarations = typed_trees::references::collect_program_declarations(typed);
    declarations.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    declarations.dedup();
    let nominal = |symbol| nominal_identity_from_symbols(&typed.symbols, symbol, exact_sources);
    let mut rows = Vec::new();
    let consumer = |owner: SymbolHandle| -> Review<PackagePolicySemanticDependencyConsumer> {
        let identity = nominal(owner)?;
        Ok(
            if callables
                .callables
                .iter()
                .any(|row| row.identity == identity)
            {
                PackagePolicySemanticDependencyConsumer::Callable(identity)
            } else {
                PackagePolicySemanticDependencyConsumer::PackageImplementation
            },
        )
    };
    // The general declaration walk includes exact static applications, type
    // operands, mathematical definitions and selected code dependencies.
    for (owner, dependencies) in
        typed_trees::references::collect_declaration_symbol_dependencies(typed)
    {
        if typed.symbols.symbol_package_identity(owner) != Some(package) {
            continue;
        }
        let consumed = consumer(owner)?;
        for symbol in dependencies {
            let Some(symbol) = declaration_owner(typed, &declarations, symbol)? else {
                continue;
            };
            if symbol == owner {
                continue;
            }
            let dependency = nominal(symbol)?;
            rows.push(PackagePolicySemanticDependency {
                consumer: consumed.clone(),
                dependency: dependency.clone(),
                exposure: PackageReviewSemanticDependencyExposure::PrivateImplementation,
                kind: PackageReviewSemanticDependencyKind::NominalIdentity,
            });
            if let Some(data) = typed.unique_data_definition_by_symbol(symbol) {
                for kind in [
                    PackageReviewSemanticDependencyKind::Layout,
                    PackageReviewSemanticDependencyKind::OwnershipBehavior,
                ] {
                    rows.push(PackagePolicySemanticDependency {
                        consumer: consumed.clone(),
                        dependency: dependency.clone(),
                        exposure: PackageReviewSemanticDependencyExposure::PrivateImplementation,
                        kind,
                    });
                }
                if let Some(cleanup) = data.cleanup_selection {
                    rows.push(PackagePolicySemanticDependency {
                        consumer: consumed.clone(),
                        dependency: dependency.clone(),
                        exposure: PackageReviewSemanticDependencyExposure::PrivateImplementation,
                        kind: PackageReviewSemanticDependencyKind::AutomaticCleanup,
                    });
                    rows.push(PackagePolicySemanticDependency {
                        consumer: consumed.clone(),
                        dependency: nominal(cleanup.executable)?,
                        exposure: PackageReviewSemanticDependencyExposure::PrivateImplementation,
                        kind: PackageReviewSemanticDependencyKind::AutomaticCleanupMachine,
                    });
                }
            }
        }
    }
    // Source owners survive SCC folding separately from execution owners.
    // Use the private checked packet's occurrence relation, never a span join.
    for machine in &module.machines {
        let invocation_owner = |site| -> Review<SymbolHandle> {
            match calls.binary_search_by_key(&(machine.id, site), |row| (row.machine, row.site)) {
                Ok(index) => Ok(calls[index].owner),
                Err(_) => machines.get(machine.id).copied().ok_or_else(|| {
                    rejected("source-free invocation has no retained execution declaration")
                }),
            }
        };
        for call in machine.module_invocations() {
            let owner = invocation_owner(call.site)?;
            if typed.symbols.symbol_package_identity(owner) != Some(package) {
                continue;
            }
            let target = machines.get(call.inputs.callee).copied().ok_or_else(|| {
                rejected("accepted invocation has no exact target declaration correspondence")
            })?;
            rows.push(PackagePolicySemanticDependency {
                consumer: consumer(owner)?,
                dependency: nominal(target)?,
                exposure: PackageReviewSemanticDependencyExposure::PrivateImplementation,
                kind: PackageReviewSemanticDependencyKind::NominalIdentity,
            });
        }
        for call in machine.boundary_invocations() {
            let owner = invocation_owner(call.site)?;
            if typed.symbols.symbol_package_identity(owner) != Some(package) {
                continue;
            }
            let target = boundaries.get(call.boundary).copied().ok_or_else(|| {
                rejected(
                    "accepted boundary invocation has no exact source declaration correspondence",
                )
            })?;
            rows.push(PackagePolicySemanticDependency {
                consumer: consumer(owner)?,
                dependency: nominal(target)?,
                exposure: PackageReviewSemanticDependencyExposure::PrivateImplementation,
                kind: PackageReviewSemanticDependencyKind::NominalIdentity,
            });
        }
        let owner = machines
            .get(machine.id)
            .copied()
            .ok_or_else(|| rejected("accepted machine has no source declaration correspondence"))?;
        if typed.symbols.symbol_package_identity(owner) != Some(package) {
            continue;
        }
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            if let terminal_psi::OperationKind::SelectMachine { descriptor } = operation.kind {
                let descriptor = module
                    .callables
                    .descriptor(descriptor)
                    .ok_or_else(|| rejected("constructed callable has no admitted descriptor"))?;
                let target = machines.get(descriptor.machine).copied().ok_or_else(|| {
                    rejected("constructed callable has no exact selected source declaration")
                })?;
                rows.push(PackagePolicySemanticDependency {
                    consumer: consumer(owner)?,
                    dependency: nominal(target)?,
                    exposure: PackageReviewSemanticDependencyExposure::PrivateImplementation,
                    kind: PackageReviewSemanticDependencyKind::NominalIdentity,
                });
            }
        }
    }
    Ok(rows)
}

fn declaration_owner(
    typed: &TypedTrees,
    declarations: &[SymbolHandle],
    mut symbol: SymbolHandle,
) -> Review<Option<SymbolHandle>> {
    let mut remaining = typed.symbols.symbols().nodes().len();
    while symbol.is_valid() {
        if remaining == 0 || !typed.symbols.symbols().nodes().is_valid(symbol) {
            return Err(rejected(
                "semantic dependency has a cyclic, stale or foreign declaration owner",
            ));
        }
        remaining -= 1;
        let node = typed.symbols.get(symbol);
        if matches!(
            node.kind,
            SymbolKind::BuiltinType
                | SymbolKind::BuiltinFunction
                | SymbolKind::Parameter
                | SymbolKind::TypeParameter
                | SymbolKind::MachineParameter
                | SymbolKind::ConformanceParameter
                | SymbolKind::CallableContract
                | SymbolKind::PropositionParameter
                | SymbolKind::PropositionMachineParameter
        ) {
            return Ok(None);
        }
        if declarations
            .binary_search_by_key(&(symbol.arena_index(), symbol.generation()), |symbol| {
                (symbol.arena_index(), symbol.generation())
            })
            .is_ok()
        {
            return Ok(Some(symbol));
        }
        symbol = typed.declaration_scope_parent(symbol);
    }
    Ok(None)
}
