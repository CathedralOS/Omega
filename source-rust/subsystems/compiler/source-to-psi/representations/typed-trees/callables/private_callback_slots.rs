//! A private callback slot belongs to one applied layout and boundary requirement.
//! Keep the identities used by native layout together with their source commitment.

use crate::source_state::expression::StaticMachineArgument;
use crate::source_state::trait_definition::AppliedTraitScope;
use crate::source_state::type_identity::{
    ExactOwnerTypeIdentityRequest, IndexIdentityArgument, portable_applied_index_identity,
};
use crate::source_state::typed_trees::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
};
use crate::source_state::types::TypeReferenceNode;
use crate::source_state::{PrivateCallbackSlotApplication, TypedTrees};
use application_identity::callback_declarations::PrivateCallbackSlotSelection;
use application_identity::conformance_identity::{
    ConformanceIdentityDescription, source_identity_commitments,
};
use language_core::core_vocabulary::CoreVocabulary;
use symbols::SymbolHandle;

impl TypedTrees {
    pub fn private_callback_slot_application(
        &mut self,
        selected: &StaticMachineArgument,
        owner: SymbolHandle,
        sources: &[(source_text::SourceId, [u8; 32])],
    ) -> Result<PrivateCallbackSlotApplication, String> {
        let (mut slot, scope) = self.close_private_callback_slot(selected, owner, None, sources)?;
        let environment = scope.environment();
        slot.application.trait_scope = environment.retain(self)?;
        Ok(slot)
    }

    pub fn check_private_callback_slot_declaration(
        &self,
        selected: &StaticMachineArgument,
        owner: SymbolHandle,
        sources: &[(source_text::SourceId, [u8; 32])],
    ) -> Result<(), String> {
        self.private_callback_slot_selection(selected, owner, sources)
            .map(|_| ())
    }

    /// An installation cites the closed slot without running the layout policy.
    pub fn private_callback_slot_selection(
        &self,
        selected: &StaticMachineArgument,
        owner: SymbolHandle,
        sources: &[(source_text::SourceId, [u8; 32])],
    ) -> Result<PrivateCallbackSlotSelection, String> {
        let (slot, _) = self.close_private_callback_slot(selected, owner, None, sources)?;
        Ok(slot.selection)
    }

    /// A policy's ordinary checked body returns its public LayoutPlan. Only
    /// compiler evaluation can collect the private placements made while it runs.
    pub fn check_private_callback_placement_owner(
        &self,
        selected: &StaticMachineArgument,
        caller: SymbolHandle,
        sources: &[(source_text::SourceId, [u8; 32])],
    ) -> Result<(), String> {
        let (policy, _) = self
            .owned_conformance_realization(caller)
            .filter(|(_, row)| {
                self.core_vocabulary(row.requirement) == Some(CoreVocabulary::FixedLayoutPlan)
            })
            .ok_or("private callback placement outside its owning fixed layout policy")?;
        self.close_private_callback_slot(selected, caller, Some(policy.symbol), sources)?;
        Ok(())
    }

    fn close_private_callback_slot(
        &self,
        selected: &StaticMachineArgument,
        owner: SymbolHandle,
        expected_layout: Option<SymbolHandle>,
        sources: &[(source_text::SourceId, [u8; 32])],
    ) -> Result<(PrivateCallbackSlotApplication, AppliedTraitScope<'_>), String> {
        if selected.const_literal.is_some()
            || selected.evidence_projection.is_some()
            || selected.application.is_some()
            || selected.authored_machine_application.is_valid()
        {
            return Err("private callback slot needs a closed named application".into());
        }
        let implementation = self
            .conformance_by_symbol(selected.symbol)
            .ok_or("private callback slot has no named implementation")?;
        if self.core_vocabulary(implementation.trait_symbol)
            != Some(CoreVocabulary::PrivateCallbackSlot)
            || !implementation.type_parameters.is_empty()
            || !implementation.lifetime_parameters.is_empty()
            || !implementation.trait_lifetime_arguments.is_empty()
            || !implementation.rows.is_empty()
        {
            return Err("private callback slot needs a closed memberless core PrivateCallbackSlot implementation".into());
        }
        let [semantic, layout, requirement] = self
            .type_reference_table
            .type_reference_handles(implementation.arguments)
        else {
            return Err(
                "private callback slot needs semantic data, its layout and a boundary requirement"
                    .into(),
            );
        };
        let slot_scope = AppliedTraitScope::from_conformance(self, implementation)
            .ok_or("private callback slot has no closed trait application")?;
        let Some([_, layout_binder, _]) = self
            .trait_type_parameters(slot_scope.declaration)
            .as_array()
        else {
            return Err("private callback slot lost its layout binder".into());
        };
        let required = slot_scope
            .bound_scope(self, layout_binder.symbol)
            .ok_or("private callback slot has no applied layout bound")?;
        let supplied = self
            .applied_requirement_scope(owner, *layout)
            .and_then(|scope| scope.declaring_scope(self, required.declaration.symbol))
            .ok_or("private callback slot layout does not implement the required trait")?;
        if !required.same_application(self, &supplied) {
            return Err("private callback slot layout belongs to another semantic type".into());
        }
        let TypeReferenceNode::Named { symbol, .. } =
            self.type_reference_table.type_reference(*requirement)
        else {
            return Err("private callback slot needs one exact boundary requirement".into());
        };
        let symbol = *symbol;
        if !self.symbols.symbols().nodes().is_valid(symbol)
            || self.symbols.get(symbol).kind != symbols::SymbolKind::State
        {
            return Err("private callback slot requires a boundary requirement declaration".into());
        }
        let boundary = self
            .trait_definition_by_symbol(self.symbols.get(symbol).parent)
            .filter(|boundary| boundary.is_boundary)
            .ok_or("private callback slot requirement has no boundary trait owner")?;
        let row = self
            .trait_machine_signature_by_symbol(boundary, symbol)
            .ok_or("private callback slot requirement is absent from its boundary trait")?;
        let requirement_identity = self.trait_requirement_callable_identity_text(boundary, row);
        let nominal = |symbol| {
            self.exact_owner_nominal_identity(symbol, sources)
                .and_then(|id| self.type_identity_text(id))
                .ok_or("private callback slot declaration has no exact source identity")
        };
        let type_identity = |type_reference| {
            self.exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
                type_reference,
                lifetime_scope: None,
                binders: &[],
                substitutions: &[],
                exact_toolchain_sources: sources,
            })
            .and_then(|id| self.type_identity_text(id))
            .ok_or("private callback slot has an unidentified trait argument")
        };
        // The machine binder selects the exact overload. A nominal type path
        // alone would give different signatures of Callback::enter the same hash.
        let trait_arguments = vec![
            type_identity(*semantic)?,
            type_identity(*layout)?,
            application_identity::type_identity::encoding::callable_declaration_path(
                &nominal(symbol)?,
                &requirement_identity,
            ),
        ];
        let bytes = ConformanceIdentityDescription {
            declaration: &nominal(implementation.symbol)?,
            trait_name: &nominal(implementation.trait_symbol)?,
            trait_arguments: &trait_arguments,
            ..Default::default()
        }
        .encoded_bytes();
        let (report_fingerprint, digest) = source_identity_commitments(&bytes);
        let portable = |argument| {
            portable_applied_index_identity(self, argument, owner, &[])
                .and_then(|id| self.type_identity_text(id))
                .ok_or("private callback slot lost its portable applied identity")
        };
        let layout_selection = self.static_operand_from_type_reference(*layout)?;
        if expected_layout.is_some_and(|owner| owner != layout_selection.symbol) {
            return Err("private callback placement belongs to another fixed layout policy".into());
        }
        let application = ClosedConformanceApplication {
            declaration: implementation.symbol,
            trait_definition: implementation.trait_symbol,
            trait_arguments,
            report_fingerprint,
            commitment: ClosedConformanceApplicationCommitment::from_digest(digest),
            identity_encoding: bytes.into(),
            ..Default::default()
        };
        let selection = PrivateCallbackSlotSelection {
            declaration_identity: self
                .normalized_hermetic_symbol_identity(implementation.symbol)?,
            trait_identity: self
                .normalized_hermetic_symbol_identity(implementation.trait_symbol)?,
            semantic_identity: portable(IndexIdentityArgument::TypeArgument(*semantic))?,
            layout_identity: portable(IndexIdentityArgument::StaticArgument(&layout_selection))?,
            requirement_identity,
            application_report_fingerprint: report_fingerprint,
            application_commitment: digest,
            application_encoding: application.identity_encoding.clone(),
        };
        selection.validate_identity_encoding()?;
        Ok((
            PrivateCallbackSlotApplication {
                application,
                selection,
            },
            slot_scope,
        ))
    }
}
