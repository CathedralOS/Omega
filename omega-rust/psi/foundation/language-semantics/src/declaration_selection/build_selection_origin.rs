//! Authorship of an already executed Build choice across checked source worlds.
//!
//! An activation and its lexical helper are separate declarations. Neither must
//! be a runtime Product machine. Retaining their source anchors preserves the
//! original evaluation's provenance without transplanting arena handles or
//! giving the receiving program access to the helper's private names.

use source::{SourceSpan, Span};
use symbols::{RetainedDeclaration, SymbolHandle, SymbolKind, SymbolTable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSelectionOrigin {
    activation: RetainedDeclaration,
    author: RetainedDeclaration,
    occurrence: Span,
    event_ordinal: Option<u64>,
}

impl BuildSelectionOrigin {
    pub fn capture(
        symbols: &SymbolTable,
        activation: SymbolHandle,
        author: SymbolHandle,
        occurrence: SourceSpan,
        event_ordinal: Option<u64>,
    ) -> Result<Self, &'static str> {
        let activation = RetainedDeclaration::capture(symbols, activation)?;
        let author = RetainedDeclaration::capture(symbols, author)?;
        if author.source().source_id != occurrence.source_id {
            return Err("Build selection occurrence does not belong to its lexical author");
        }
        Self::from_retained_parts(activation, author, occurrence.span, event_ordinal)
    }

    /// Decode provenance, not an authorization. Source admission and the
    /// original activation's executed ledger remain separate obligations.
    pub fn from_retained_parts(
        activation: RetainedDeclaration,
        author: RetainedDeclaration,
        occurrence: Span,
        event_ordinal: Option<u64>,
    ) -> Result<Self, &'static str> {
        if activation.kind() != SymbolKind::Machine || author.kind() != SymbolKind::Machine {
            return Err("Build selection activation and author must be machine declarations");
        }
        if occurrence.start >= occurrence.end
            || author
                .source()
                .source
                .get(occurrence.start..occurrence.end)
                .is_none()
        {
            return Err("Build selection has an invalid authored occurrence");
        }
        let portable = |declaration: RetainedDeclaration| {
            RetainedDeclaration::from_retained_parts(
                declaration.source().clone(),
                declaration.span(),
                declaration.kind(),
            )
        };
        Ok(Self {
            activation: portable(activation)?,
            author: portable(author)?,
            occurrence,
            event_ordinal,
        })
    }

    pub fn activation(&self) -> &RetainedDeclaration {
        &self.activation
    }
    pub fn author(&self) -> &RetainedDeclaration {
        &self.author
    }
    pub const fn occurrence(&self) -> Span {
        self.occurrence
    }
    pub const fn event_ordinal(&self) -> Option<u64> {
        self.event_ordinal
    }
}
