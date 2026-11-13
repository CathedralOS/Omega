/*
We resolve the root bindings a Build executed to the machines they name
(`collect_root_bindings`).
*/

/*
A Build binds a root slot, such as a target's program entry, either to a path
it wrote or to a product description it was handed. Each binding is resolved
in the package of the build source that wrote it: a bare name is a machine of
that package, and a path through a dependency alias is a public machine of
that dependency. Borrowing the Build grants no other package's names. A slot
is bound once. This reads declarations and names only; whether the bound
machine is a valid entry is judged where the entry is checked.
*/

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    ExecutedRootBinding, RootBinding, RootBindingSelection,
};
use typed_trees::TypedTrees;

pub(crate) fn collect_root_bindings(
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
                let selected = described
                    .declaration
                    .rejoin(
                        &typed.symbols,
                        typed.machines().iter().map(|machine| machine.symbol),
                    )
                    .map_err(|reason| {
                        vec![Diagnostic::error(reason).with_source_span(binding.source_span)]
                    })?;
                (described.machine_name.clone(), selected)
            }
            RootBindingSelection::AuthoredPath(implementation) => {
                let implementation = implementation.clone();
                let occurrence = typed.symbols.rejoin_product_occurrence(binding.source_span);
                let qualified = implementation.split_once("::");
                let dependency = qualified.and_then(|(alias, _)| {
                    typed.symbols.product_dependency_target(occurrence, alias)
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
                                        typed
                                            .symbols
                                            .same_product_package_instance(occurrence, span)
                                    }),
                            }
                    })
                    .filter(|machine| {
                        typed.symbols.source_reference_can_describe_product_symbol(
                            occurrence,
                            machine.symbol,
                        )
                    })
                    .collect::<Vec<_>>();
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
            source_span: Some(binding.source_span),
        });
    }
    if diagnostics.is_empty() {
        Ok(bindings)
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> (TypedTrees, source::SourceId) {
        let mut sources = source::SourceMap::from_files(Vec::new());
        let source_id = sources
            .add(
                std::path::PathBuf::from("program/main.omg"),
                text.to_owned(),
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text)
            .tokenize()
            .expect("source lexes");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
            .expect("source parses");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
                retained_base: None,
            },
        )
        .expect("source resolves");
        let base =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(
                resolved,
            )
            .expect("source types");
        (base.into_typed(), source_id)
    }

    fn binding(typed: &TypedTrees, slot: &str, path: &str) -> ExecutedRootBinding {
        // A Build's binding is written in the root package; the entry
        // machine's own declaration is such a place.
        let source_span = typed
            .symbols
            .symbol_source_span(typed.machines()[0].symbol)
            .expect("a declared machine has a span");
        ExecutedRootBinding {
            source_span,
            slot: slot.to_owned(),
            implementation: RootBindingSelection::AuthoredPath(path.to_owned()),
        }
    }

    #[test]
    fn a_written_entry_binds_the_machine_it_names_in_its_own_package() {
        let (typed, _) = typed("machine main() { }\nmachine helper() { }\n");
        let helper = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "helper")
            .unwrap()
            .symbol;
        let bound = collect_root_bindings(
            &typed,
            &[binding(&typed, "linux_x86_64::ProgramEntry", "helper")],
        )
        .expect("the package declares `helper`");
        assert_eq!(bound.len(), 1);
        assert_eq!(bound[0].slot, "linux_x86_64::ProgramEntry");
        assert_eq!(bound[0].implementation, "helper");
        assert_eq!(bound[0].implementation_symbol, helper);

        // The twin names a machine the package does not declare.
        let error = collect_root_bindings(
            &typed,
            &[binding(&typed, "linux_x86_64::ProgramEntry", "absent")],
        )
        .unwrap_err();
        assert!(
            error[0].message.contains("unknown entry machine `absent`"),
            "{error:?}"
        );
    }

    #[test]
    fn a_root_slot_is_bound_once() {
        let (typed, _) = typed("machine main() { }\nmachine helper() { }\n");
        let slot = "linux_x86_64::ProgramEntry";
        let error = collect_root_bindings(
            &typed,
            &[
                binding(&typed, slot, "main"),
                binding(&typed, slot, "helper"),
            ],
        )
        .unwrap_err();
        assert!(
            error[0].message.contains("is already bound to `main`"),
            "{error:?}"
        );
        // The twin binds two slots, one machine each.
        let bound = collect_root_bindings(
            &typed,
            &[
                binding(&typed, slot, "main"),
                binding(&typed, "windows_x86_64::ProgramEntry", "helper"),
            ],
        )
        .unwrap();
        assert_eq!(bound.len(), 2);
    }
}
