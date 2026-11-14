use crate::{
    AuthoredDeclarationSelections, AuthoredSelectionExtensionFrontier,
    AuthoredSelectionExtensionRebaseError, SymbolResolvedTrees,
};
use std::sync::Arc;
use symbols::SymbolHandle;

/// A completed body batch carries its exact predecessor, not the resolver's
/// pending syntax. Typing validates this pairing before accepting the batch.
#[derive(Debug)]
pub struct RebasedResolvedBodyCompletion {
    pub successor: Arc<SymbolResolvedTrees>,
    pub predecessor: Arc<SymbolResolvedTrees>,
    pub selected: Vec<SymbolHandle>,
}

/// Source activation changes lookup membership without changing declarations.
/// The private pairing lets the typed owner adopt precisely this activation.
pub struct ResolvedSourceActivation {
    predecessor: Arc<SymbolResolvedTrees>,
    successor: Arc<SymbolResolvedTrees>,
}

/// Generated typing allocations reserve IDs before deferred resolution resumes.
/// This transition changes no declarations and validates the retained prefix once.
pub struct ResolvedAllocationExtension {
    predecessor: Arc<SymbolResolvedTrees>,
    successor: Arc<SymbolResolvedTrees>,
}

impl ResolvedAllocationExtension {
    pub fn new(
        predecessor: Arc<SymbolResolvedTrees>,
        symbols: &symbols::SymbolTable,
        domains: &language_semantics::SemanticDomainTable,
        service_rows: &language_semantics::ServiceReachRowTable,
    ) -> Result<Self, String> {
        if !symbols.is_generated_extension_of(&predecessor.symbols) {
            return Err("prepared allocation join changed retained source custody or appended authored symbols".into());
        }
        if domains.len() < predecessor.semantic_domains.len() {
            return Err("prepared allocation join truncated retained semantic domains".into());
        }
        for ordinal in 1..=predecessor.semantic_domains.len() {
            let identity = language_semantics::SemanticDomainId(ordinal as u32);
            if domains.name(identity) != predecessor.semantic_domains.name(identity) {
                return Err(format!(
                    "prepared allocation join changed retained semantic domain {ordinal}"
                ));
            }
        }
        if !service_rows.starts_with(&predecessor.service_reach_rows) {
            return Err("prepared allocation join changed retained service-row storage".into());
        }
        let mut successor = Arc::clone(&predecessor);
        if predecessor.symbols != *symbols
            || predecessor.semantic_domains != *domains
            || predecessor.service_reach_rows != *service_rows
        {
            let prepared = Arc::make_mut(&mut successor);
            prepared.symbols = symbols.clone();
            prepared.semantic_domains = domains.clone();
            prepared.service_reach_rows = service_rows.clone();
        }
        Ok(Self {
            predecessor,
            successor,
        })
    }

    pub fn successor(&self) -> Arc<SymbolResolvedTrees> {
        Arc::clone(&self.successor)
    }

    pub fn into_parts(self) -> (Arc<SymbolResolvedTrees>, Arc<SymbolResolvedTrees>) {
        (self.predecessor, self.successor)
    }
}

impl ResolvedSourceActivation {
    pub fn new(
        predecessor: Arc<SymbolResolvedTrees>,
        sources: Arc<source::SourceMap>,
        bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    ) -> Result<Self, String> {
        let symbols = predecessor
            .symbols
            .clone()
            .begin_source_activation(sources, bindings)?
            .finish();
        let mut successor = predecessor.as_ref().clone();
        successor.symbols = symbols;
        successor.resolution_indexes = Default::default();
        Ok(Self {
            predecessor,
            successor: Arc::new(successor),
        })
    }

    pub fn successor(&self) -> Arc<SymbolResolvedTrees> {
        Arc::clone(&self.successor)
    }

    pub fn into_parts(self) -> (Arc<SymbolResolvedTrees>, Arc<SymbolResolvedTrees>) {
        (self.predecessor, self.successor)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SeededSymbolResolvedTrees {
    pub trees: SymbolResolvedTrees,
    pub authored_selection_frontier: AuthoredSelectionExtensionFrontier,
    pub retained_base: Arc<SymbolResolvedTrees>,
}

/*
    A seeded resolved extension whose authored-selection suffix has been
    joined to the destination typed base ledger. The exact resolved base stays
    inside the carrier so the next phase can reject cross-paired continuations.
*/
#[derive(Debug, PartialEq, Eq)]
pub struct RebasedSeededSymbolResolvedTrees {
    trees: SymbolResolvedTrees,
    retained_base: Arc<SymbolResolvedTrees>,
}

impl SeededSymbolResolvedTrees {
    pub fn trees(&self) -> &SymbolResolvedTrees {
        &self.trees
    }

    pub fn rebase_authored_selections_for_typed_continuation(
        self,
        destination_base: &AuthoredDeclarationSelections,
    ) -> Result<RebasedSeededSymbolResolvedTrees, (Self, AuthoredSelectionExtensionRebaseError)>
    {
        match self
            .trees
            .rebase_authored_selection_extension(self.authored_selection_frontier, destination_base)
        {
            Ok(trees) => Ok(RebasedSeededSymbolResolvedTrees {
                trees,
                retained_base: self.retained_base,
            }),
            Err((trees, error)) => Err((Self { trees, ..self }, error)),
        }
    }
}

impl RebasedSeededSymbolResolvedTrees {
    pub fn trees(&self) -> &SymbolResolvedTrees {
        &self.trees
    }

    pub fn into_typing_continuation_parts(self) -> (SymbolResolvedTrees, Arc<SymbolResolvedTrees>) {
        (self.trees, self.retained_base)
    }
}
