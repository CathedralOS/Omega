//! Original source content published by Psi after source checking.
//!
//! Signature bytes use the original positional binder grammar. Conservation
//! rows retain the authored expression beside its produced plan. Omega may
//! commit this content, but does not rebuild source plans or infer source
//! authority from an empty lookup. All handles resolve in the enclosing
//! CheckedTrees owner; default arena rows carry no content.

use arena::{Arena, HandleSpan};
use language_semantics::content::ContentConservationPlan;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckedPublicSignatureContent {
    signature: SymbolHandle,
    bytes: HandleSpan<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedContentConservationSource {
    source_expression: ExpressionHandle,
    plan: Option<ContentConservationPlan>,
}

impl CheckedContentConservationSource {
    pub const fn source_expression(&self) -> ExpressionHandle {
        self.source_expression
    }

    pub fn plan(&self) -> Option<&ContentConservationPlan> {
        self.plan.as_ref()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedPublicContractSources {
    signatures: Arena<CheckedPublicSignatureContent>,
    bytes: Arena<u8>,
    conservation: Arena<CheckedContentConservationSource>,
}

impl CheckedPublicContractSources {
    /// Publish content from the source producer, before CheckedTrees freezes.
    pub fn retain_signature(&mut self, signature: SymbolHandle, bytes: Vec<u8>) {
        let bytes = self.bytes.insert_many(bytes);
        self.signatures
            .append(CheckedPublicSignatureContent { signature, bytes });
    }

    pub fn retain_conservation(
        &mut self,
        source_expression: ExpressionHandle,
        plan: ContentConservationPlan,
    ) {
        self.conservation.append(CheckedContentConservationSource {
            source_expression,
            plan: Some(plan),
        });
    }

    /// Missing or duplicate source coordinates supply no commitment content.
    pub fn signature_bytes(&self, signature: SymbolHandle) -> Option<&[u8]> {
        if !signature.is_valid() {
            return None;
        }
        let mut rows = self
            .signatures
            .iter()
            .filter_map(|(_, row)| (row.signature == signature).then_some(row));
        let row = rows.next()?;
        if rows.next().is_some() {
            return None;
        }
        Some(self.bytes.span_or_empty(row.bytes))
    }

    pub const fn conservation_sources(&self) -> &Arena<CheckedContentConservationSource> {
        &self.conservation
    }
}
