//! Match a Build occurrence to its own file's import for diagnostics.

use super::PendingBuildImport;
use crate::source_state::TypedTrees;
use source_text::SourceSpan;
use std::collections::BTreeSet;

impl PendingBuildImport {
    pub(super) fn files_without_product_uses<'a>(
        imports: &'a [Self],
        typed: &TypedTrees,
    ) -> BTreeSet<(usize, &'a str)> {
        let mut unused: BTreeSet<_> = imports
            .iter()
            .filter_map(|import| match &import.target {
                crate::SourceImportTarget::File { alias, .. } => {
                    Some((import.source.0, alias.as_str()))
                }
                crate::SourceImportTarget::Namespace(_) => None,
            })
            .collect();
        if unused.is_empty() {
            return unused;
        }
        let mut sources: Vec<_> = unused.iter().map(|(source, _)| *source).collect();
        sources.dedup();
        let machines = typed.machines();
        let authors = machines.iter().filter(|machine| {
            typed
                .symbols
                .symbol_source_span(typed.symbols.authored_origin(machine.symbol))
                .is_some_and(|span| {
                    let source = typed.symbols.rejoin_product_occurrence(span).source_id.0;
                    sources.binary_search(&source).is_ok()
                })
        });

        // Product operands can use an import that never becomes available to host calls.
        let mut mark = |span: SourceSpan, path: &str| {
            if let Some((alias, rest)) =
                path.strip_prefix('|').and_then(|path| path.split_once('|'))
                && (rest.is_empty() || rest.starts_with("::"))
            {
                unused.remove(&(
                    typed.symbols.rejoin_product_occurrence(span).source_id.0,
                    alias,
                ));
            }
        };
        typed_trees::product_operand::visit_build_selection_occurrences(
            typed,
            authors,
            |_, _, _, request| {
                if let Some(entry) = request.entry() {
                    mark(entry.source_span, &entry.slot);
                    if let crate::declaration_selection::RootBindingOperand::AuthoredPath(path) =
                        &entry.implementation
                    {
                        mark(entry.source_span, path);
                    }
                }
                for operand in request.designated_operands() {
                    mark(operand.source_span, &operand.path);
                    if let Some(application) = &operand.application {
                        for (_, name) in application.names.iter() {
                            mark(name.source_span, &name.text);
                        }
                    }
                }
            },
        );
        unused
    }

    pub fn alias_at_refused_call<'a>(
        imports: &'a [Self],
        typed: &TypedTrees,
        span: SourceSpan,
    ) -> Option<&'a str> {
        let path = typed.deferred_refusals.iter().find_map(|refusal| {
            (refusal.source_span == span)
                .then_some(refusal.unresolved_call_path.as_deref())
                .flatten()
        })?;
        imports.iter().find_map(|import| {
            let mut occurrence = path.split("::");
            if import.source != span.source_id {
                return None;
            }
            match &import.target {
                crate::SourceImportTarget::File { root, alias, .. } => {
                    let name = occurrence.next()?.strip_prefix('|')?.strip_suffix('|')?;
                    (name == alias && occurrence.next().is_some())
                        .then_some(root.as_deref())
                        .flatten()
                }
                crate::SourceImportTarget::Namespace(members) => members
                    .iter()
                    .all(|member| occurrence.next() == Some(member.as_str()))
                    .then(|| members.first().map(String::as_str))
                    .flatten(),
            }
        })
    }
}
