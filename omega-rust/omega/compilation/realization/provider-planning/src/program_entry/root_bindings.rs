//! Rejoin executed root-binding requests to the admitted build source.

use checked_trees::typed_trees::TypedTrees;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{ExecutedRootBinding, RootBindingSelection};

use super::RootBinding;

/// Admission is lexical per occurrence package: the `implementation` operand
/// resolves against the product machines declared in the package containing
/// the `Build::bind` statement, or a public declaration selected through that
/// package's explicit product dependency alias, never the caller's. A borrowed
/// root Build carries operational authority, not the caller's product namespace,
/// so a foreign helper can bind its own package's entry while wrong-scope spellings
/// reject. Identity leaves here as an exact symbol so later admission never
/// reselects by spelling.
///
/// A delegated `ProductEntryRef` operand instead rejoins the description the
/// compiler issued at the `product.entry` query: selection authority and the
/// package check belonged to the query occurrence, so the bind consumes the
/// exact retained symbol and requires the authored slot to match the queried
/// slot. The description names a product declaration only -- its machine was
/// looked up, never executed.
pub fn collect_root_bindings(
    typed: &TypedTrees,
    executed: &[ExecutedRootBinding],
) -> Result<Vec<RootBinding>, Vec<Diagnostic>> {
    let typed = typed.product_declarations();
    let mut bindings: Vec<RootBinding> = Vec::with_capacity(executed.len());
    let mut diagnostics = Vec::new();
    for binding in executed {
        let slot = binding.slot.clone();
        let (implementation, implementation_symbol) = match &binding.implementation {
            RootBindingSelection::Described(described) => {
                if slot != described.slot {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "root binding slot `{slot}` does not match the product description's slot `{}`",
                            described.slot
                        ))
                        .with_source_span(binding.source_span),
                    );
                    continue;
                }
                if typed.machine(described.machine_symbol).is_none() {
                    return Err(vec![
                        Diagnostic::error(
                            "product description names a machine outside the admitted program",
                        )
                        .with_source_span(binding.source_span),
                    ]);
                }
                (described.machine_name.clone(), described.machine_symbol)
            }
            RootBindingSelection::AuthoredPath(implementation) => {
                let implementation = implementation.clone();
                // Static root operands share the product query's lexical scope,
                // not the build helper's import scope. An authorized alias selects
                // the dependency's product checked instance even when the same
                // source also has an independently checked build instance.
                let qualified = implementation.split_once("::");
                let dependency = qualified.and_then(|(alias, _)| {
                    typed
                        .symbols
                        .product_dependency_target(binding.source_span, alias)
                });
                let declaration_path = match (qualified, dependency) {
                    (Some((_, path)), Some(_)) => path,
                    _ => implementation.as_str(),
                };
                let mut candidates = typed
                    .machines()
                    .iter()
                    .filter(|machine| {
                        ((dependency.is_none() && machine.name.as_str() == declaration_path)
                            || typed.symbols.display_path(machine.symbol, "::") == declaration_path)
                            && match dependency {
                                Some(package) => {
                                    machine.is_public
                                        && typed
                                            .symbols
                                            .symbol_product_package_identity(machine.symbol)
                                            == Some(package)
                                }
                                None => typed
                                    .symbols
                                    .symbol_source_span(machine.symbol)
                                    .is_some_and(|span| {
                                        typed.symbols.same_product_package_instance(
                                            binding.source_span,
                                            span,
                                        )
                                    }),
                            }
                    })
                    .filter(|machine| {
                        typed.symbols.source_reference_can_describe_product_symbol(
                            binding.source_span,
                            machine.symbol,
                        )
                    })
                    .collect::<Vec<_>>();
                // Prefer complete declaration paths over the existing same-package
                // relative spelling. Otherwise another module's attached machine
                // `Entry::launch` can make an exact `Entry::launch` path ambiguous.
                // Keep the local fallback for roots in a named product module;
                // dependency operands never inherit these relative spellings.
                if candidates.iter().any(|machine| {
                    typed.symbols.display_path(machine.symbol, "::") == declaration_path
                }) {
                    candidates.retain(|machine| {
                        typed.symbols.display_path(machine.symbol, "::") == declaration_path
                    });
                }
                match candidates.as_slice() {
                    [machine] => (implementation, machine.symbol),
                    [] => {
                        let probe = qualified.map_or(implementation.as_str(), |(_, path)| path);
                        let message = if typed.machines().iter().any(|machine| {
                            machine.name.as_str() == probe
                                || typed.symbols.display_path(machine.symbol, "::") == probe
                        }) {
                            format!(
                                "root binding names `{implementation}`, which is not a product declaration visible from this build source's package; borrowing Build does not grant the caller's product namespace"
                            )
                        } else {
                            format!(
                                "root binding names unknown entry machine `{implementation}` in this build source's package"
                            )
                        };
                        diagnostics
                            .push(Diagnostic::error(message).with_source_span(binding.source_span));
                        continue;
                    }
                    _ => {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "root binding `{implementation}` is ambiguous within its package"
                            ))
                            .with_source_span(binding.source_span),
                        );
                        continue;
                    }
                }
            }
        };
        if let Some(existing) = bindings.iter().find(|binding| binding.slot == slot) {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{slot}` is already bound to `{}`; it cannot also bind `{implementation}`", existing.implementation
            )).with_source_span(binding.source_span));
            continue;
        }
        bindings.push(RootBinding {
            slot,
            implementation,
            implementation_symbol,
        });
    }
    if diagnostics.is_empty() {
        Ok(bindings)
    } else {
        Err(diagnostics)
    }
}
