//! Ordered raw operator-token rows for one exact declaration storage view.
//! Visibility, ownership, default primitive meaning and operand applicability
//! remain questions of the use site after this index releases its borrow.
use super::HashMap;
use crate::TypedTrees;
use crate::domain::DomainDefinition;
use crate::operator::OperatorDefinition;
use crate::typed_trees::spelled_declarations::DeclarationStorageIdentity;
use arena::{Handle, HandleSpan};
use language_core::OperatorSpelling;
use std::sync::Arc;

pub(super) type DomainDeclarationScope = (DeclarationStorageIdentity, HandleSpan<DomainDefinition>);

#[derive(Clone, PartialEq, Eq)]
pub(super) struct OperatorSpellingScope {
    operators: DeclarationStorageIdentity,
    domains: DeclarationStorageIdentity,
    root_operators: HandleSpan<OperatorDefinition>,
    machine_bindings: HandleSpan<OperatorDefinition>,
    root_domains: HandleSpan<DomainDefinition>,
}

#[derive(Clone, Copy)]
pub(crate) enum OperatorSpellingRow {
    Root(Handle<OperatorDefinition>),
    Domain {
        operator: Handle<OperatorDefinition>,
        domain: Handle<DomainDefinition>,
    },
}

#[derive(Clone, Default)]
pub(super) struct OperatorSpellingIndex {
    rows: HashMap<OperatorSpelling, Arc<[OperatorSpellingRow]>>,
}

impl TypedTrees {
    pub(super) fn domain_declaration_scope(&self) -> DomainDeclarationScope {
        (
            self.tables.domain_definitions.storage_identity(),
            self.roots.domain_definitions,
        )
    }

    pub(super) fn with_operator_spelling_index<Result>(
        &self,
        read: impl FnOnce(&OperatorSpellingIndex) -> Result,
    ) -> Result {
        let scope = OperatorSpellingScope {
            operators: self.tables.operators.storage_identity(),
            domains: self.tables.domain_definitions.storage_identity(),
            root_operators: self.roots.operators,
            machine_bindings: self.roots.machine_token_bindings,
            root_domains: self.roots.domain_definitions,
        };
        self.declaration_indexes.operator_spellings.with(
            scope,
            || {
                let mut rows: HashMap<OperatorSpelling, Vec<OperatorSpellingRow>> =
                    HashMap::default();
                for span in [self.roots.operators, self.roots.machine_token_bindings] {
                    for (position, operator) in
                        self.tables.operators.span_or_empty(span).iter().enumerate()
                    {
                        if let Some(spelling) = operator.spelling {
                            rows.entry(spelling)
                                .or_default()
                                .push(OperatorSpellingRow::Root(span.handle_at(position as u32)));
                        }
                    }
                }
                for (position, domain) in self.domain_definitions().iter().enumerate() {
                    let domain_handle = self.roots.domain_definitions.handle_at(position as u32);
                    for (position, operator) in self.domain_operators(domain).iter().enumerate() {
                        if let Some(spelling) = operator.spelling {
                            rows.entry(spelling)
                                .or_default()
                                .push(OperatorSpellingRow::Domain {
                                    operator: domain.operators.handle_at(position as u32),
                                    domain: domain_handle,
                                });
                        }
                    }
                }
                OperatorSpellingIndex {
                    rows: rows
                        .into_iter()
                        .map(|(spelling, rows)| (spelling, Arc::from(rows)))
                        .collect(),
                }
            },
            read,
        )
    }

    pub(crate) fn operator_spelling_rows(
        &self,
        spelling: OperatorSpelling,
    ) -> Option<Arc<[OperatorSpellingRow]>> {
        self.with_operator_spelling_index(|index| index.rows.get(&spelling).cloned())
    }
}
