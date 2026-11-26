#![forbid(unsafe_code)]

/*
We finish native stage 04 by preparing the execution plan that instruction
selection consumes. `prepare_execution_plan` joins the lowered target
operations with the optimized program's accepted obligations and ownership
facts, the admitted boundary origins and interrupt installations, and the
sealed module. Native stage 05 receives that `PreparedExecutionPlan`.

The same crate holds the plan validator. `verify_execution_plan` checks the
published plan's seal, header, callable facts, publication, closed types,
ABIs, installations, boundary origins, and target-to-source operation
correspondence, then re-derives the legalized functions from the target
operations and compares them row for row. Ordinary compilation never calls it;
it runs on an explicit validator request and in tests.
*/

/*
- `legalization.rs` coordinates preparation and validation.
- `legalization/instruction.rs` and `legalization/control.rs` prepare the plan from retained target rows.
- `legalization/source.rs` and `legalization/scalar_graph.rs` hold retained row projectors and boundary checks.
- `installed_interrupts.rs` and `interrupt_installation.rs` join the target program to its admitted interrupt installations.
*/

pub(crate) mod native_geometry {
    pub(crate) use legalized_operations::native_geometry::{NativeGeometries, NativeGeometry};

    /// The integer types the accepted module retains.
    ///
    /// Preparation closes every scalar it writes into the plan here, so the
    /// plan carries only closed types and selection never asks which types
    /// exist. Only the exact module origin admission used can supply these:
    /// see [`accepted_types`].
    #[derive(Debug, Clone)]
    pub struct ModuleTypes(terminal_codec::ClosedTypeCatalog);

    impl ModuleTypes {
        pub(crate) fn integer(
            &self,
            integer: semantic_vocabulary::IntegerType,
        ) -> Option<semantic_vocabulary::ClosedIntegerType> {
            self.0.integer(integer).ok()
        }

        pub(crate) fn scalar(
            &self,
            scalar: semantic_vocabulary::ScalarType,
        ) -> Option<semantic_vocabulary::ClosedScalarType> {
            self.0.scalar(scalar).ok()
        }

        /// Whether a closed type the plan states is one the module retains.
        pub(crate) fn retains(&self, scalar: semantic_vocabulary::ClosedScalarType) -> bool {
            self.scalar(scalar.as_checking()) == Some(scalar)
        }

        /// The types a unit fixture's own closed module retains.
        #[cfg(test)]
        pub(crate) fn of(closed: &terminal_codec::ClosedTerminalModule) -> Self {
            Self(closed.closed_types().clone())
        }
    }

    /// The accepted module's retained types. The module must be the exact one
    /// origin admission used; clones of the retained owner share that
    /// immutable module, and a module with equal bytes but another owner is
    /// refused.
    pub fn accepted_types(
        closed: &terminal_codec::ClosedTerminalModule,
        psi: terminal_psi::TerminalPsiIdentity,
        module: &terminal_psi::TerminalModule,
    ) -> Option<ModuleTypes> {
        (closed.identity() == psi && std::ptr::eq(closed.module(), module))
            .then(|| ModuleTypes(closed.closed_types().clone()))
    }

    #[cfg(test)]
    mod tests {
        use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};
        use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

        #[test]
        fn only_the_exact_accepted_module_supplies_types() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 13).unwrap();
            let mut module = terminal_psi::TerminalModule::declarations();
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
            let identity = identities.intern("native_closed_input");
            module.identities = identities.build();
            module.types.structural_types.push(StructuralTypeDeclaration {
                declaration_identity: Default::default(),
                id: StructuralTypeId::new(1).unwrap(),
                identity,
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                descriptor_cleanup: false,
                nominal_cleanup: None,
                scalar_restriction: None,
            });
            let published = terminal_codec::ClosedTerminalModule::new(module).unwrap();
            let accepted = |closed: &terminal_codec::ClosedTerminalModule, psi| {
                super::accepted_types(closed, psi, published.module())
            };
            let types = accepted(&published, published.identity()).unwrap();
            assert_eq!(types.integer(integer).unwrap().bits(), 13);
            let closed = types.scalar(ScalarType::Integer(integer)).unwrap();
            assert!(types.retains(closed));
            let foreign = IntegerType::new(IntegerSign::Unsigned, 17).unwrap();
            assert!(types.integer(foreign).is_none());
            let foreign = semantic_vocabulary::ClosedScalarType::Integer(
                semantic_vocabulary::ClosedIntegerType::new(IntegerSign::Unsigned, 17).unwrap(),
            );
            assert!(!types.retains(foreign));
            let symbolic = IntegerType::parameterized(
                IntegerSign::Unsigned,
                semantic_vocabulary::WidthParameter {
                    owner: semantic_vocabulary::MachineId::new(1).unwrap(),
                    value: semantic_vocabulary::ValueId::new(1).unwrap(),
                },
            );
            assert!(types.integer(symbolic).is_none());
            // Another program's identity, or an owner with equal bytes that
            // origin admission never saw, supplies nothing.
            let other = terminal_codec::ClosedTerminalModule::new(
                terminal_psi::TerminalModule::declarations(),
            )
            .unwrap();
            assert!(accepted(&published, other.identity()).is_none());
            assert!(accepted(&published.clone(), published.identity()).is_some());
            let foreign_owner =
                terminal_codec::ClosedTerminalModule::new(published.module().clone()).unwrap();
            assert_eq!(foreign_owner.identity(), published.identity());
            assert!(accepted(&foreign_owner, published.identity()).is_none());
        }
    }
}
pub(crate) use legalized_operations::structural_inputs;

mod callable_invocations;
pub use callable_invocations::PreparedCallableInvocations;
mod publication;
pub use publication::capture_source_publication;

pub use verified_optimization_input::publication::target_replay::BoundaryOriginFacts;

pub(crate) mod installed_interrupts;
pub(crate) mod interrupt_installation;
mod legalization;
#[path = "legalization/control.rs"]
mod legalization_control;
#[path = "legalization/instruction.rs"]
mod legalization_instruction;
#[path = "legalization/scalar_graph.rs"]
mod legalization_scalar_graph;
#[path = "legalization/source.rs"]
mod legalization_source;

pub(crate) use legalization::LegalizationError;
pub use legalization::{
    LegalizationError as TargetOperationAuditError, PreparationEvidence, prepare_execution_plan,
    verify_execution_plan, verify_target_operation_plan_with_closed_module,
};
pub use native_geometry::accepted_types;

#[cfg(test)]
mod sum_layout_selection;
