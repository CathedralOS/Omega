//! Original declaration selections and typed Product application custody.
//!
//! Build specialization and Product source completion grow separate payload
//! arenas. A declaration symbol survives their shared source prefix, but a type
//! or application handle does not. Preparation relocates operand payloads into
//! one retained declaration owner; the original authored Build application and
//! operand ordinal identify the edge. These rows describe selections, never
//! provider, trust or execution authority. Immutable checking still admits the
//! complete typed tuple before an executed registration can export it.

use crate::TypedTrees;
use crate::static_applications::StaticMachineApplicationHandle;
use std::sync::Arc;
use symbols::SymbolHandle;

/// The original non-executing role of one designated Build operand. A whole
/// provider interface and a callable requirement have different telescopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductDeclarationKind {
    ProviderSubject,
    ProviderType,
    BoundaryRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDeclarationOperand {
    pub source_application: StaticMachineApplicationHandle,
    pub argument_index: usize,
    pub source_span: source::SourceSpan,
    pub declaration: SymbolHandle,
    pub kind: ProductDeclarationKind,
    pub owner: Arc<crate::FrozenTypedTrees>,
    /// Zero for declaration-family descriptions which form no applied tuple.
    /// A valid handle belongs to `owner` and is admitted by static tuple checking.
    pub application: StaticMachineApplicationHandle,
    /// Applied interface geometry issued beside the same checked outer tuple.
    /// Zero on declaration families and callable requirement descriptions.
    pub receiver_type: crate::types::TypeReferenceHandle,
    /// Original descendant coordinates survive normalization into type payloads.
    /// These handles belong to `owner`, never the executable Build arenas.
    pub declaration_selections: arena::HandleSpan<ProductStaticArgumentSelection>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductStaticArgumentSelection {
    pub descendant_ordinal: usize,
    pub declaration: SymbolHandle,
}

/// The arena's absent slot carries no declaration or frozen owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductDeclarationOperandSlot {
    operand: Option<ProductDeclarationOperand>,
}

impl From<ProductDeclarationOperand> for ProductDeclarationOperandSlot {
    fn from(operand: ProductDeclarationOperand) -> Self {
        Self {
            operand: Some(operand),
        }
    }
}

impl TypedTrees {
    /// Applied tuple projection only. Declaration-family selection custody
    /// does not bypass its ordinary provider/category/conformance admission.
    pub fn product_declaration_application(
        &self,
        source_application: StaticMachineApplicationHandle,
        argument_index: usize,
    ) -> Option<&ProductDeclarationOperand> {
        self.product_declaration_operand(source_application, argument_index)
            .filter(|operand| operand.application.is_valid())
    }

    pub fn product_declaration_operand(
        &self,
        source_application: StaticMachineApplicationHandle,
        argument_index: usize,
    ) -> Option<&ProductDeclarationOperand> {
        if !source_application.is_valid() {
            return None;
        }
        self.product_declaration_operands
            .iter()
            .filter_map(|(_, slot)| slot.operand.as_ref())
            .find(|application| {
                application.source_application == source_application
                    && application.argument_index == argument_index
            })
    }
}

/// An original lexical declaration retained for a non-executing application.
/// Its parameter handles belong to the frozen capture, independently of the
/// relocated operand payloads. It is never inserted into executable roots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedStaticLexicalScope {
    owner: Arc<crate::FrozenTypedTrees>,
    declaration: SymbolHandle,
    identity: u64,
    retained_handle: arena::Handle<CapturedStaticLexicalScopeSlot>,
}

impl CapturedStaticLexicalScope {
    pub fn capture(owner: Arc<crate::FrozenTypedTrees>, declaration: SymbolHandle) -> Option<Self> {
        owner.machine(declaration)?;
        // A local capture namespace survives arena relocation. It is never
        // encoded in a portable declaration/application/trust commitment.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Some(Self {
            owner,
            declaration,
            identity: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            retained_handle: arena::Handle::invalid(),
        })
    }

    fn declaration(&self) -> &crate::machine::Machine {
        self.owner
            .machine(self.declaration)
            .expect("captured lexical declaration remains in its frozen owner")
    }

    pub fn identity(&self) -> u64 {
        self.identity
    }

    pub fn symbol(&self) -> SymbolHandle {
        self.declaration
    }

    pub fn name(&self) -> &crate::name::Identifier {
        &self.declaration().name
    }

    pub fn lifetime_parameters(&self) -> &[crate::name::Identifier] {
        &self.declaration().lifetime_parameters
    }

    /// Original custody for any payload handle in the retained binder roster.
    pub fn owner(&self) -> &TypedTrees {
        &self.owner
    }

    /// The original binder roster. Its payload handles belong to `owner()`,
    /// independently of the relocated Product operand owner.
    pub fn type_parameters(&self) -> &[crate::data::TypeParameter] {
        self.owner.machine_type_parameters(self.declaration())
    }
}

/// Arena absence carries no invented declaration or frozen payload owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapturedStaticLexicalScopeSlot {
    scope: Option<CapturedStaticLexicalScope>,
    value_carriers: arena::HandleSpan<CapturedStaticValueCarrier>,
    value_carrier_state: CapturedValueCarrierState,
}

/// Actual construction state, including recursive capture references while
/// the owning producer completes appended semantic payloads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum CapturedValueCarrierState {
    #[default]
    Uninitialized,
    Copying,
    Complete,
}

/// Exact original binder and carrier coordinates beside their copied payload.
/// `original_carrier` belongs to the frozen capture; `carrier` belongs to the
/// TypedTrees arena which owns this row. Neither coordinate is portable identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapturedStaticValueCarrier {
    pub parameter: SymbolHandle,
    pub original_carrier: crate::types::TypeReferenceHandle,
    pub carrier: crate::types::TypeReferenceHandle,
}

impl From<CapturedStaticLexicalScope> for CapturedStaticLexicalScopeSlot {
    fn from(scope: CapturedStaticLexicalScope) -> Self {
        Self {
            scope: Some(scope),
            ..Self::default()
        }
    }
}

impl TypedTrees {
    /// Authenticate a source declaration coordinate retained while its type
    /// and expression payloads move into this Product. Equal handles or paths
    /// alone are insufficient; independently generated tuples are not shared.
    pub fn retains_source_declaration_symbol(
        &self,
        source: &TypedTrees,
        symbol: SymbolHandle,
    ) -> bool {
        if !symbol.is_valid() {
            return false;
        }
        let original_symbols = &source.symbols;
        let receiving_symbols = &self.symbols;
        if let Some(atom) = original_symbols.builtin_type_atom(symbol) {
            return receiving_symbols.builtin_type_atom(symbol) == Some(atom);
        }
        let same_sources = match (
            original_symbols.retained_source_map(),
            receiving_symbols.retained_source_map(),
        ) {
            (Some(original), Some(receiving)) => Arc::ptr_eq(original, receiving),
            _ => false,
        };
        let original = original_symbols.get(symbol);
        let receiving = receiving_symbols.get(symbol);
        same_sources
            && original.kind != symbols::SymbolKind::Unknown
            && original.kind == receiving.kind
            && !original.generated_from.is_valid()
            && original.parent == receiving.parent
            && original.generated_from == receiving.generated_from
            && original_symbols
                .symbol_provenance_source_span(symbol)
                .is_some()
            && original_symbols.symbol_provenance_source_span(symbol)
                == receiving_symbols.symbol_provenance_source_span(symbol)
    }

    pub fn captured_static_lexical_scope(
        &self,
        handle: arena::Handle<CapturedStaticLexicalScopeSlot>,
    ) -> Option<&CapturedStaticLexicalScope> {
        self.captured_static_lexical_scopes
            .get(handle)
            .scope
            .as_ref()
    }

    pub fn capture_static_lexical_scope(
        &mut self,
        owner: Arc<crate::FrozenTypedTrees>,
        declaration: SymbolHandle,
    ) -> arena::Handle<CapturedStaticLexicalScopeSlot> {
        if let Some((handle, _)) = self
            .captured_static_lexical_scopes
            .iter()
            .find(|(_, slot)| {
                slot.scope.as_ref().is_some_and(|scope| {
                    scope.declaration == declaration && Arc::ptr_eq(&scope.owner, &owner)
                })
            })
        {
            return handle;
        }
        let Some(scope) = CapturedStaticLexicalScope::capture(owner, declaration) else {
            return arena::Handle::invalid();
        };
        self.retain_captured_static_lexical_scope(&scope)
    }

    /// Copy the original capture association, independently of destination
    /// arena numbers and generated declaration symbols.
    pub fn retain_captured_static_lexical_scope(
        &mut self,
        scope: &CapturedStaticLexicalScope,
    ) -> arena::Handle<CapturedStaticLexicalScopeSlot> {
        if let Some((handle, _)) = self
            .captured_static_lexical_scopes
            .iter()
            .find(|(_, slot)| {
                slot.scope
                    .as_ref()
                    .is_some_and(|previous| previous.identity == scope.identity)
            })
        {
            return handle;
        }
        let handle = self
            .captured_static_lexical_scopes
            .insert(scope.clone().into());
        self.captured_static_lexical_scopes
            .get_mut(handle)
            .scope
            .as_mut()
            .expect("retained capture has its original scope")
            .retained_handle = handle;
        handle
    }
}

impl TypedTrees {
    /// Construction starts once per retained scope. Nested copied applications
    /// may refer to that scope while its carrier graphs are being completed.
    pub fn begin_captured_static_value_carriers(
        &mut self,
        handle: arena::Handle<CapturedStaticLexicalScopeSlot>,
    ) -> bool {
        let slot = self.captured_static_lexical_scopes.get_mut(handle);
        if slot.scope.is_none()
            || slot.value_carrier_state != CapturedValueCarrierState::Uninitialized
        {
            return false;
        }
        slot.value_carrier_state = CapturedValueCarrierState::Copying;
        true
    }

    /// Publication follows complete payload copying under the receiving issuer.
    pub fn retain_captured_static_value_carriers(
        &mut self,
        handle: arena::Handle<CapturedStaticLexicalScopeSlot>,
        carriers: impl IntoIterator<Item = CapturedStaticValueCarrier>,
    ) {
        let carriers = self.captured_static_value_carriers.insert_many(carriers);
        let slot = self.captured_static_lexical_scopes.get_mut(handle);
        slot.value_carriers = carriers;
        slot.value_carrier_state = CapturedValueCarrierState::Complete;
    }

    /// The borrowed scope itself must belong to this retained arena. An equal
    /// local scope/parameter number in another issuer cannot select its span.
    pub fn captured_static_value_carrier(
        &self,
        scope: &CapturedStaticLexicalScope,
        parameter: SymbolHandle,
    ) -> Option<crate::types::TypeReferenceHandle> {
        let slot = self
            .captured_static_lexical_scopes
            .get(scope.retained_handle);
        if slot.value_carrier_state != CapturedValueCarrierState::Complete
            || !std::ptr::eq(slot.scope.as_ref()?, scope)
        {
            return None;
        }
        let original = scope
            .type_parameters()
            .iter()
            .find(|row| row.symbol == parameter)?;
        let original_carrier = match original.kind {
            crate::data::TypeParameterKind::Const { type_reference }
            | crate::data::TypeParameterKind::Value { type_reference } => type_reference,
            _ => return None,
        };
        let mut carriers = self
            .captured_static_value_carriers
            .span_or_empty(slot.value_carriers)
            .iter()
            .filter(|row| row.parameter == parameter);
        let carrier = carriers.next()?;
        (carriers.next().is_none()
            && carrier.original_carrier == original_carrier
            && self
                .type_reference_table
                .contains_type_reference(carrier.carrier))
        .then_some(carrier.carrier)
    }
}
