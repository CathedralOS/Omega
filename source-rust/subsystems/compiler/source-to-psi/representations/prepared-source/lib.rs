//! Immutable typed source and the computation inputs that accompany it.
//! The constructor checks their shared source and exact declaration coordinates;
//! preparation algorithms remain in stage 04 and lowering reads only this product.
#![forbid(unsafe_code)]

mod values;
use diagnostics::Diagnostic;
use prepared_source_issuer::PreparationIssuer;
use typed_trees::source_state::{self, FrozenTypedTrees, TypedTrees};
pub use values::{PreparedTarget, PreparedValue, TargetInputs, ValueInputs};

/// A frozen source snapshot with checked computation-input correspondence.
/// Cloning retains the same source owner. Only stage 04 links the issuer
/// crate, so only stage 04 can build one:
///
/// ```
/// use prepared_source::PreparedTypedTrees;
/// use prepared_source_issuer::PREPARATION;
/// use typed_trees::source_state::TypedTrees;
/// let prepared = PreparedTypedTrees::new(PREPARATION, TypedTrees::default(), &[]).unwrap();
/// assert!(std::sync::Arc::ptr_eq(prepared.trees(), prepared.clone().trees()));
/// ```
///
/// A snapshot cannot bypass the input checks with an unchecked conversion.
///
/// ```compile_fail,E0277
/// use prepared_source::PreparedTypedTrees;
/// use typed_trees::source_state::FrozenTypedTrees;
/// let _: PreparedTypedTrees = FrozenTypedTrees::default().into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTypedTrees {
    typed: std::sync::Arc<FrozenTypedTrees>,
    values: std::sync::Arc<[ValueInputs]>,
    constants: std::sync::Arc<[arena::Handle<source_state::constant::ConstDeclaration>]>,
}

impl PreparedTypedTrees {
    /// Check that demanded computations belong to this source and retain its
    /// exact constant handles before freezing the snapshot. Preparation passes
    /// remain the producer's responsibility; this constructor does not run them.
    /// The issuer argument is why no other crate can call this: only stage 04
    /// links the crate that supplies one, and `mbx xtask check-boundary`
    /// refuses any other link.
    pub fn new(
        _issuer: PreparationIssuer,
        program: TypedTrees,
        values: &[ValueInputs],
    ) -> Result<Self, Vec<Diagnostic>> {
        let constants = values::constant_inputs(&program, values)?;
        Ok(Self {
            typed: std::sync::Arc::new(program.into()),
            values: values.into(),
            constants: constants.into(),
        })
    }

    /// Read the exact source snapshot used to prepare this program.
    pub fn trees(&self) -> &std::sync::Arc<FrozenTypedTrees> {
        &self.typed
    }

    /// The source computations demanded during this preparation.
    pub fn values(&self) -> &[ValueInputs] {
        &self.values
    }

    /// Constants needed by the request or by its computed type arguments.
    pub fn constants(&self) -> impl Iterator<Item = &source_state::constant::ConstDeclaration> {
        self.constants
            .iter()
            .map(|&handle| self.typed.tables.const_declarations.get(handle))
    }

    /// Return to mutable source. A new preparation establishes its own inputs.
    pub fn into_inner(self) -> TypedTrees {
        std::sync::Arc::unwrap_or_clone(self.typed).into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prepared_source_issuer::PREPARATION;
    use symbols::SymbolHandle;

    #[test]
    fn cloning_keeps_the_same_snapshot_and_source_identity() {
        let program = TypedTrees::default();
        let identity = program.identity.get();
        let expected = program.clone();
        let prepared = PreparedTypedTrees::new(PREPARATION, program, &[]).unwrap();
        let cloned = prepared.clone();
        assert!(std::sync::Arc::ptr_eq(prepared.trees(), cloned.trees()));
        assert_eq!(prepared.trees().identity.get(), identity);
        assert_eq!(cloned.into_inner(), expected);
    }

    #[test]
    fn computation_inputs_cannot_be_attached_to_another_source() {
        let original = TypedTrees::default();
        let other = TypedTrees::default();
        assert_ne!(original.identity.get(), other.identity.get());
        let input = ValueInputs::new(
            &original,
            PreparedValue::Constant(SymbolHandle::invalid()),
            Vec::new(),
            TargetInputs::None,
        );
        let refusal = PreparedTypedTrees::new(PREPARATION, other, &[input]).unwrap_err();
        assert_eq!(refusal.len(), 1);
        assert_eq!(
            refusal[0].message,
            "prepared value inputs belong to another source program"
        );
    }

    #[test]
    fn a_requested_constant_must_have_its_declaration() {
        let program = TypedTrees::default();
        let input = ValueInputs::new(
            &program,
            PreparedValue::Constant(SymbolHandle::invalid()),
            Vec::new(),
            TargetInputs::None,
        );
        let refusal = PreparedTypedTrees::new(PREPARATION, program, &[input]).unwrap_err();
        assert_eq!(refusal.len(), 1);
        assert_eq!(
            refusal[0].message,
            "source preparation lost a demanded constant declaration"
        );
    }

    #[test]
    fn a_static_input_must_keep_its_owner_expression_and_carrier() {
        let program = TypedTrees::default();
        let input = ValueInputs::new(
            &program,
            PreparedValue::StaticArgument {
                owner: SymbolHandle::invalid(),
                site: Default::default(),
                expression: Default::default(),
                carrier: Default::default(),
            },
            Vec::new(),
            TargetInputs::None,
        );
        let refusal = PreparedTypedTrees::new(PREPARATION, program, &[input]).unwrap_err();
        assert_eq!(refusal.len(), 1);
        assert_eq!(
            refusal[0].message,
            "prepared static argument lost its declaration, expression or carrier"
        );
    }
}
