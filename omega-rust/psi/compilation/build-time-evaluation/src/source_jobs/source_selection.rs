//! Join captured-source selection consent to the actual invocation's retained
//! source correspondence. Nothing here gives unowned declarations an owner.
//! One shared capture consent is borrowed; the sorted lookup is admission
//! scratch over this module's real declarations, not a saved authority record.

use std::sync::Arc;

use source::{SourceMap, SourceSpan};
use terminal_psi::{ProofCallableAssociation, ProofDeclarationId, TargetMachineCallable};

use super::PreparedBuildMachineEntry;

pub(super) struct InvocationSelection<'a> {
    entry: &'a PreparedBuildMachineEntry,
    identity: terminal_codec::TerminalExecutionIdentity,
    sources: Arc<SourceMap>,
    requester: SourceSpan,
    declarations: Vec<(ProofDeclarationId, SourceSpan)>,
}

impl<'a> InvocationSelection<'a> {
    pub(super) fn new(
        entry: &'a PreparedBuildMachineEntry,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Option<Self> {
        entry.source_consent.as_ref()?;
        let invocation = artifact.module().identities.invocation_scope()?;
        if entry.accepted.module().identities.invocation_scope() != Some(invocation) {
            return None;
        }
        let source = entry.accepted.source();
        let sources = source.symbols.source_map()?;
        let requester = entry
            .source
            .symbols
            .symbol_provenance_source_span(entry.requester)?;
        let elaboration = &entry.accepted.closure().elaboration;
        let scope = &artifact.module().scalar_applications.proof_scope;
        let mut declarations = scope
            .callables
            .iter()
            .filter_map(|(_, association)| {
                let ProofCallableAssociation::Callable {
                    declaration,
                    callable,
                } = association
                else {
                    return None;
                };
                let symbol = match callable {
                    TargetMachineCallable::Machine(machine) => elaboration.machines.get(*machine),
                    TargetMachineCallable::Boundary(boundary) => {
                        elaboration.boundaries.get(*boundary)
                    }
                }?;
                source
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(*symbol)
                    .then(|| source.symbols.symbol_provenance_source_span(*symbol))
                    .flatten()
                    .map(|span| (*declaration, span))
            })
            .collect::<Vec<_>>();
        declarations.sort_unstable_by_key(|(declaration, _)| declaration.0);
        if declarations
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1)
        {
            return None;
        }
        declarations.dedup_by_key(|(declaration, _)| *declaration);
        Some(Self {
            entry,
            identity: artifact.custody_identity(),
            sources,
            requester,
            declarations,
        })
    }

    fn source(&self, declaration: ProofDeclarationId) -> Option<SourceSpan> {
        self.declarations
            .binary_search_by_key(&declaration.0, |(declaration, _)| declaration.0)
            .ok()
            .map(|position| self.declarations[position].1)
    }
}

impl terminal_installation::InvocationSourceSelection for InvocationSelection<'_> {
    fn allows_selection(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
        requester: Option<ProofDeclarationId>,
        declaration: ProofDeclarationId,
    ) -> bool {
        if artifact.custody_identity() != self.identity {
            return false;
        }
        let requester = match requester {
            Some(requester) => self.source(requester),
            None => Some(self.requester),
        };
        let Some((requester, declaration)) = requester.zip(self.source(declaration)) else {
            return false;
        };
        self.entry.source_consent.as_ref().is_some_and(|consent| {
            consent.allows_selection(&self.sources, requester, declaration, self.entry.purpose)
        })
    }
}
