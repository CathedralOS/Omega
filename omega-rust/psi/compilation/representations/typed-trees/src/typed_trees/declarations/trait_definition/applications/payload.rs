//! Owned invocation-local scope storage for a later immutable cohort snapshot.
//! Handles remain in the original program's clone lineage; rebinding never
//! relocates operands or rebuilds the scope from three flattened fields.

use super::{AppliedDeclarationRegionScope, AppliedTraitScope, TraitDefinition};
use crate::TypedTrees;
use crate::typed_trees::product_applications::CapturedStaticLexicalScope;
use crate::types::TypeReferenceHandle;
use symbols::SymbolHandle;

// Arena absence carries program zero and cannot bind to an issued program.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedTraitScopePayload {
    original_program: u64,
    declaration: TraitDefinition,
    substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    substitution_scope_starts: Vec<usize>,
    argument_owner: SymbolHandle,
    captured_argument_owner: Option<CapturedStaticLexicalScope>,
    region_scopes: Vec<AppliedDeclarationRegionScope>,
}

impl AppliedTraitScope<'_> {
    pub fn retain_payload(&self, program: &TypedTrees) -> AppliedTraitScopePayload {
        AppliedTraitScopePayload {
            original_program: program.identity.get(),
            declaration: self.declaration.clone(),
            substitutions: self.substitutions.clone(),
            substitution_scope_starts: self.substitution_scope_starts.clone(),
            argument_owner: self.argument_owner,
            captured_argument_owner: self.captured_argument_owner.clone(),
            region_scopes: self.region_scopes.clone(),
        }
    }
}

impl AppliedTraitScopePayload {
    /// The producing pass freezes this program after preparing the cohort.
    /// Cloning preserves its process-unique namespace; a foreign compiler
    /// object cannot rehydrate these same-numbered handles.
    pub fn bind<'program>(
        &self,
        program: &'program TypedTrees,
    ) -> Option<AppliedTraitScope<'program>> {
        if program.identity.get() != self.original_program {
            return None;
        }
        let declaration = program.trait_definition_by_symbol(self.declaration.symbol)?;
        if declaration != &self.declaration {
            return None;
        }
        Some(AppliedTraitScope {
            declaration,
            substitutions: self.substitutions.clone(),
            substitution_scope_starts: self.substitution_scope_starts.clone(),
            argument_owner: self.argument_owner,
            captured_argument_owner: self.captured_argument_owner.clone(),
            region_scopes: self.region_scopes.clone(),
        })
    }
}
