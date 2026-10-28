use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::type_identity::CallableIdentityId;
use typed_trees::type_identity::TypeIdentityId;

/// The checked implementation behind one `Impl as dyn Trait<Args>`
/// conversion: the selector value the conversion produces names exactly this
/// implementation's table. Lowering consumes these rows; it never rediscovers
/// an implementation from names or data types.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct DynamicSelectionFact {
    pub occurrence: ExpressionHandle,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    /// The selected named implementation.
    pub implementation: SymbolHandle,
    pub target_trait: SymbolHandle,
    /// Normalized identity of the selector's `dyn Trait<Args>` type: every
    /// selection with this identity fills the same dynamic interface.
    pub interface_identity: TypeIdentityId,
    /// The implementation's complete closed row map in canonical order.
    pub rows: Vec<DynamicConformanceRowFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct DynamicConformanceRowFact {
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    /// Complete normalized overload identity of the declaring-trait slot.
    /// Physical descriptor lowering consumes this identity directly rather
    /// than reconstructing a slot from an unqualified requirement spelling.
    pub requirement_identity: CallableIdentityId,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    /// Complete normalized callable identity of the selected realization.
    /// This remains logical descriptor identity; it is never a table address.
    pub realization_identity: CallableIdentityId,
    /// The finite-family value tuple this row realizes, keyed by the canonical
    /// const identities a specialization carries for the same arguments (for
    /// example `named(integer-const(16))`), in the requirement's const/value
    /// binder declaration order. Empty on an exact nongeneric row: it matches
    /// every call to its requirement without consulting family actuals.
    pub family_tuple: Box<[String]>,
    pub source: DynamicConformanceRowSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum DynamicConformanceRowSource {
    Inline,
    Reference,
    TraitDefault,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct DynamicConformanceFacts {
    pub selections: Vec<DynamicSelectionFact>,
}
