//! Exact compiler-catalog declarations for generated placed scalar accessors.
//! This describes implementation availability, never a backing or loan grant.

use crate::ProviderPlanningContext;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub use checked_trees::typed_trees::PlacedScalarAccessorOperation as SealedPlacedScalarOperation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedPlacedAccessor {
    declaration: checked_trees::typed_trees::PlacedScalarAccessorDeclaration,
}

impl SealedPlacedAccessor {
    pub const fn machine(&self) -> SymbolHandle {
        self.declaration.machine()
    }
    pub const fn state(&self) -> SymbolHandle {
        self.declaration.state()
    }
    pub const fn access_slot(&self) -> u32 {
        self.declaration.access_slot()
    }
    pub const fn placement_commitment(&self) -> [u8; 32] {
        self.declaration.placement_commitment()
    }
    pub const fn operation(&self) -> SealedPlacedScalarOperation {
        self.declaration.operation()
    }

    pub fn binding_identity(
        &self,
        context: ProviderPlanningContext<'_>,
    ) -> Result<String, Diagnostic> {
        use std::fmt::Write;
        let mut identity = String::from("compiler-catalog::placed-scalar.v1::");
        for byte in self.execution_contract(context)? {
            write!(&mut identity, "{byte:02x}").expect("String formatting");
        }
        Ok(identity)
    }

    /// Full nominal/signature/placement identity for receiving mechanism joins.
    /// Hashing this description grants no storage, lifetime or authority.
    pub fn execution_contract(
        &self,
        context: ProviderPlanningContext<'_>,
    ) -> Result<[u8; 32], Diagnostic> {
        let schema = crate::service_schema::from_sealed_placed_accessor(context, self)?;
        let operation = match self.operation() {
            SealedPlacedScalarOperation::Read => effects::CompilerPlacedScalarOperation::Read,
            SealedPlacedScalarOperation::Take => effects::CompilerPlacedScalarOperation::Take,
            SealedPlacedScalarOperation::Write => effects::CompilerPlacedScalarOperation::Write,
        };
        Ok(effects::compiler_placed_scalar_contract_identity(
            schema.identity_digest(),
            self.placement_commitment(),
            self.access_slot(),
            operation,
        ))
    }

    /// Independently rejoin one retained plan at a receiving declaration gate.
    /// The catalog role is not inferred from an equal callable signature.
    pub fn rejoin_plan(
        context: ProviderPlanningContext<'_>,
        plan: &effects::provider_plan::ProviderPlan,
    ) -> Result<Option<Self>, Diagnostic> {
        let claimed = plan.rows.iter().any(|row| {
            matches!(&row.binding,
            effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine }
                if machine.starts_with("compiler-catalog::placed-scalar.v1::"))
        });
        if !claimed {
            return Ok(None);
        }
        let [row] = plan.rows.as_slice() else {
            return Err(Diagnostic::error(
                "placed scalar catalog plan must retain one exact row",
            ));
        };
        let identity = plan.identities.text(row.requirement_identity);
        let mut candidates = Vec::new();
        for machine in context.typed.machines() {
            if context
                .typed
                .machine_callable_identity_text(machine)
                .as_deref()
                != Some(identity)
            {
                continue;
            }
            if let Some(accessor) = Self::authenticate(context, machine.symbol)? {
                candidates.push(accessor);
            }
        }
        let [accessor] = candidates.as_slice() else {
            return Err(Diagnostic::error(
                "placed scalar catalog plan lost its unique declaration",
            ));
        };
        let schema = crate::service_schema::from_sealed_placed_accessor(context, accessor)?;
        let binding = effects::provider_plan::ProviderBinding::CompilerIntrinsic {
            machine: accessor.binding_identity(context)?,
        };
        let provider = context
            .typed
            .normalized_hermetic_symbol_identity(accessor.declaration.accessor())
            .map_err(Diagnostic::error)?;
        if schema != plan.schema
            || plan.provider_type != provider
            || plan.provider_type_package_identity
                != context
                    .typed
                    .symbols
                    .symbol_package_identity(accessor.declaration.accessor())
            || plan.origin_package_identity
                != context
                    .typed
                    .symbols
                    .symbol_package_identity(accessor.machine())
            || binding != row.binding
            || !row.requirement_lifetime_partition.is_empty()
        {
            return Err(Diagnostic::error(
                "placed scalar catalog plan substituted its sealed schema or row",
            ));
        }
        Ok(Some(accessor.clone()))
    }

    pub fn authenticate(
        context: ProviderPlanningContext<'_>,
        symbol: SymbolHandle,
    ) -> Result<Option<Self>, Diagnostic> {
        let declaration =
            checked_trees::typed_trees::PlacedScalarAccessorDeclaration::authenticate(
                context.typed,
                symbol,
            )?;
        Ok(declaration
            .filter(|declaration| context.contains(declaration.machine()))
            .map(|declaration| Self { declaration }))
    }
}
