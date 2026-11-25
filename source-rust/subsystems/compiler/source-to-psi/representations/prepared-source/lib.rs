//! Immutable typed source and the computation inputs that accompany it.
//! The constructor checks their shared source and exact declaration coordinates;
//! preparation algorithms remain in stage 04 and lowering reads only this product.
#![forbid(unsafe_code)]

mod values;
use diagnostics::Diagnostic;
use prepared_source_issuer::PreparationIssuer;
use symbols::SymbolHandle;
use typed_trees::source_state::static_applications::StaticMachineApplicationHandle;
use typed_trees::source_state::{self, FrozenTypedTrees, TypedTrees};
pub use values::{PreparedTarget, PreparedValue, TargetInputs, ValueInputs};

/// A static application selected as an entry in this typed source. Its arena
/// coordinates belong to this program, including when its trees are cloned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryApplication {
    source: u64,
    application: StaticMachineApplicationHandle,
}

impl EntryApplication {
    pub fn new(program: &TypedTrees, application: StaticMachineApplicationHandle) -> Self {
        Self {
            source: program.identity.get(),
            application,
        }
    }

    pub fn in_source(self, program: &TypedTrees) -> Option<StaticMachineApplicationHandle> {
        (self.source == program.identity.get()
            && program
                .expression_table
                .static_machine_application_is_valid(self.application))
        .then_some(self.application)
    }
}

/// A generic domain checked at its own declaration, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainSchema {
    pub declaration: SymbolHandle,
    pub application: Result<semantic_vocabulary::SemanticDomainId, String>,
}

/// A frozen source snapshot with checked computation-input correspondence.
/// Cloning retains the same source owner. Only stage 04 links the issuer
/// crate, so only stage 04 can build one:
///
/// ```
/// use prepared_source::PreparedTypedTrees;
/// use prepared_source_issuer::PREPARATION;
/// use typed_trees::source_state::TypedTrees;
/// let prepared = PreparedTypedTrees::new(PREPARATION, TypedTrees::default(), &[], Vec::new(), Vec::new()).unwrap();
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
    entries: std::sync::Arc<[(EntryApplication, SymbolHandle)]>,
    domain_schemas: std::sync::Arc<[DomainSchema]>,
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
        entries: Vec<(EntryApplication, SymbolHandle)>,
        domain_schemas: Vec<DomainSchema>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let constants = values::constant_inputs(&program, values)?;
        Ok(Self {
            typed: std::sync::Arc::new(program.into()),
            values: values.into(),
            constants: constants.into(),
            entries: entries.into(),
            domain_schemas: domain_schemas.into(),
        })
    }

    /// Read the exact source snapshot used to prepare this program.
    pub fn trees(&self) -> &std::sync::Arc<FrozenTypedTrees> {
        &self.typed
    }

    /// Declaration applications and individual formation failures, in domain declaration order.
    pub fn domain_schemas(&self) -> &[DomainSchema] {
        &self.domain_schemas
    }

    /// The exact executable specialization produced for this root request.
    pub fn entry_for_application(&self, application: EntryApplication) -> Option<SymbolHandle> {
        self.entries
            .iter()
            .find(|(selected, _)| *selected == application)
            .map(|(_, machine)| *machine)
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
        let prepared =
            PreparedTypedTrees::new(PREPARATION, program, &[], Vec::new(), Vec::new()).unwrap();
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
        let refusal = PreparedTypedTrees::new(PREPARATION, other, &[input], Vec::new(), Vec::new())
            .unwrap_err();
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
        let refusal =
            PreparedTypedTrees::new(PREPARATION, program, &[input], Vec::new(), Vec::new())
                .unwrap_err();
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
        let refusal =
            PreparedTypedTrees::new(PREPARATION, program, &[input], Vec::new(), Vec::new())
                .unwrap_err();
        assert_eq!(refusal.len(), 1);
        assert_eq!(
            refusal[0].message,
            "prepared static argument lost its declaration, expression or carrier"
        );
    }
}
