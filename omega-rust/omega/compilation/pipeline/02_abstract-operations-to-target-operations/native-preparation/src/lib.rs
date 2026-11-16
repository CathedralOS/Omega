#![forbid(unsafe_code)]

/*
We finish `Omega 02`: turn the target operations it lowered into the execution
plan instruction selection consumes.

Stage 02 already holds everything this needs: the target operations, the
accepted obligation and ownership-frontier facts of the optimized program, the
admitted boundary origins and interrupt installations, and the sealed module.
`prepare_execution_plan` joins them once into a `PreparedExecutionPlan`
(`legalized-operations`), so `Omega 03` reads that plan and none of its sources.

The same crate holds stage 02's validator. `verify_execution_plan`
derives the plan a second way, from the abstract program and the optimization
unit, and compares it with the plan preparation published. Ordinary
compilation never calls it; it runs on an explicit validator request and in
tests.
*/

/*
- `legalization.rs`, `legalization_instruction.rs`, `legalization_control.rs`: preparation of the plan from retained target rows.
- `legalization_scalar_graph.rs`, `legalization_reference_custody.rs`, `legalization_target.rs`, `legalization_source.rs`, `legalization_descriptor_inputs.rs`, `legalization_function_inputs.rs`: shared derivation and the independent validator.
- `installed_interrupts.rs`, `interrupt_installation.rs`: the join of the target program to its admitted interrupt installations.
*/

pub(crate) mod native_geometry {
    pub(crate) use legalized_operations::native_geometry::{
        NativeGeometries, NativeGeometry, exact_cast_has_native_carriers,
    };

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

    /// A stage 02 input: its prepared layouts and the module's types.
    pub(crate) struct NativeInput<'a, T> {
        input: legalized_operations::native_geometry::NativeInput<'a, T>,
        types: &'a ModuleTypes,
    }

    impl<'a, T> NativeInput<'a, T> {
        pub(crate) fn with_types(
            source: &'a T,
            geometry: &'a NativeGeometry<'a>,
            types: &'a ModuleTypes,
        ) -> Self {
            Self {
                input: legalized_operations::native_geometry::NativeInput::new(source, geometry),
                types,
            }
        }

        pub(crate) fn closed_scalar(
            &self,
            scalar: semantic_vocabulary::ScalarType,
        ) -> Option<semantic_vocabulary::ClosedScalarType> {
            self.types.scalar(scalar)
        }

        pub(crate) fn closed_integer(
            &self,
            integer: semantic_vocabulary::IntegerType,
        ) -> Option<semantic_vocabulary::ClosedIntegerType> {
            self.types.integer(integer)
        }

        pub(crate) const fn closed_types(&self) -> &'a ModuleTypes {
            self.types
        }

        pub(crate) fn geometry(&self) -> &'a NativeGeometry<'a> {
            self.input.geometry()
        }
    }

    impl<'a, T> std::ops::Deref for NativeInput<'a, T> {
        type Target = legalized_operations::native_geometry::NativeInput<'a, T>;

        fn deref(&self) -> &Self::Target {
            &self.input
        }
    }

    /// The validator reads the abstract program and the optimization unit
    /// with the same prepared layouts and module types as the plan.
    pub(crate) type NativePlan<'a> = NativeInput<'a, abstract_operations::AbstractOperationPlan>;
    pub(crate) type NativeUnit<'a> = NativeInput<'a, optimization_unit::PsiOptimizationUnit>;

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
            module.structural_types.push(StructuralTypeDeclaration {
                declaration_identity: Default::default(),
                id: StructuralTypeId::new(1).unwrap(),
                identity,
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: true,
                descriptor_cleanup: false,
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

pub(crate) mod installed_interrupts;
pub(crate) mod interrupt_installation;
mod legalization;
mod legalization_control;
mod legalization_descriptor_inputs;
mod legalization_function_inputs;
mod legalization_instruction;
mod legalization_reference_custody;
mod legalization_scalar_graph;
mod legalization_source;
mod legalization_target;

pub(crate) use legalization::LegalizationError;
pub use legalization::{
    LegalizationError as TargetOperationAuditError, PreparationEvidence, prepare_execution_plan,
    verify_execution_plan, verify_target_operation_plan_with_closed_module,
};
pub use native_geometry::{ModuleTypes, accepted_types};

#[cfg(test)]
mod sum_layout_selection;
