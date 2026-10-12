//! Retained direct external execution custody, independent of provider selection.
//! The producer joins an admitted concrete declaration to its original evaluated
//! binding or compiler catalog origin and calling application. These records
//! retain those distinct supply categories; possession
//! does not grant selection or receiving authority.

use crate::provider_plan::{EvaluatedForeignImport, EvaluatedForeignSyscall, ServiceSchema};
use calling_conventions::BoundaryEntryPlan;
use sha2::{Digest, Sha256};

/// Original authority issued for compiler-native supply. A selected plan is
/// retained only when that genuine plan already supplied the catalog row;
/// standalone bundled supply retains the authenticated original source units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectCompilerIntrinsicOrigin {
    /// Ordered original trait, requirement member and concrete realization units.
    BundledDeclarations([[u8; 32]; 3]),
    SelectedProvider(crate::provider_plan::ProviderPlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectForeignBinding {
    Import(EvaluatedForeignImport),
    Syscall(EvaluatedForeignSyscall),
    /// Exact compiler-owned native supply, issued independently of provider selection.
    CompilerIntrinsic {
        execution: crate::CompilerIntrinsicExecutionIdentity,
        origin: DirectCompilerIntrinsicOrigin,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectForeignRealization {
    /// Exact physical declaration key consumed by Terminal demand.
    callable_identity: String,
    /// Original callable overload, independently retained across target siblings.
    overload_identity: String,
    requirement_identity: String,
    target: target::TargetProfile,
    schema: ServiceSchema,
    binding: DirectForeignBinding,
    boundary_entry_plan: Option<BoundaryEntryPlan>,
    binding_identities: std::sync::Arc<terminal_psi::TerminalIdentityTable>,
    identity: [u8; 32],
}

impl DirectForeignRealization {
    /// Retain an already evaluated binding under its genuine issuing table.
    /// Native admission must rejoin the concrete callable and checked contract;
    /// this constructor neither evaluates a source expression nor selects a map.
    pub fn from_evaluated(
        callable_identity: String,
        overload_identity: String,
        requirement_identity: String,
        target: target::TargetProfile,
        schema: ServiceSchema,
        binding: DirectForeignBinding,
        boundary_entry_plan: Option<BoundaryEntryPlan>,
        binding_identities: std::sync::Arc<terminal_psi::TerminalIdentityTable>,
    ) -> Result<Self, String> {
        if callable_identity.is_empty()
            || overload_identity.is_empty()
            || requirement_identity.is_empty()
        {
            return Err("direct foreign realization has no exact declaration identity".into());
        }
        let receipt = match &binding {
            DirectForeignBinding::Import(evaluated) if evaluated.locator().target() == target => {
                evaluated.receipt()
            }
            DirectForeignBinding::Syscall(evaluated) if evaluated.target() == target => {
                evaluated.receipt()
            }
            _ => return Err("direct foreign realization has no matching evaluated binding".into()),
        };
        if !binding_identities
            .text(receipt.producer_callable_identity())
            .is_some_and(|text| !text.is_empty())
        {
            return Err("direct foreign realization lost its evaluated binding issuer".into());
        }
        Self::retain(
            callable_identity,
            overload_identity,
            requirement_identity,
            target,
            schema,
            binding.clone(),
            boundary_entry_plan,
            binding_identities,
            receipt.identity_digest(),
        )
    }

    /// Transport the catalog issuer's complete concrete declaration relation.
    /// Source authentication belongs to that issuer; this record neither elects
    /// a provider nor grants receiving authority. Missing calling custody refuses.
    pub fn from_compiler_catalog(
        callable_identity: String,
        overload_identity: String,
        requirement_identity: String,
        target: target::TargetProfile,
        schema: ServiceSchema,
        execution: crate::CompilerIntrinsicExecutionIdentity,
        origin: DirectCompilerIntrinsicOrigin,
        boundary_entry_plan: BoundaryEntryPlan,
    ) -> Result<Self, String> {
        if callable_identity.is_empty()
            || overload_identity.is_empty()
            || requirement_identity.is_empty()
        {
            return Err("direct intrinsic lost its declaration, source or calling custody".into());
        }
        let mut digest = Sha256::new();
        digest.update(b"omega.direct-compiler-intrinsic-source.v1\0");
        match &origin {
            DirectCompilerIntrinsicOrigin::BundledDeclarations(sources) => {
                if sources.contains(&[0; 32]) {
                    return Err("direct intrinsic source custody is absent".into());
                }
                digest.update([0]);
                for source in sources {
                    digest.update(source);
                }
            }
            DirectCompilerIntrinsicOrigin::SelectedProvider(plan) => {
                let mut matching = plan.rows.iter().filter(|row| {
                    plan.identities.text(row.requirement_identity) == requirement_identity
                        && matches!(&row.binding, crate::provider_plan::ProviderBinding::CompilerIntrinsic { machine } if machine == &overload_identity)
                });
                if plan.target != target.target_name()
                    || plan.schema != schema
                    || matching.next().is_none()
                    || matching.next().is_some()
                {
                    return Err("direct intrinsic lost its original selected catalog row".into());
                }
                digest.update([1]);
                digest.update(plan.identity_digest().as_bytes());
            }
        }
        digest.update(crate::compiler_intrinsic_execution_identity_bytes(
            execution,
        ));
        let origin_digest = digest.finalize().into();
        Self::retain(
            callable_identity,
            overload_identity,
            requirement_identity,
            target,
            schema,
            DirectForeignBinding::CompilerIntrinsic { execution, origin },
            Some(boundary_entry_plan),
            std::sync::Arc::new(terminal_psi::TerminalIdentityTable::default()),
            origin_digest,
        )
    }

    fn retain(
        callable_identity: String,
        overload_identity: String,
        requirement_identity: String,
        target: target::TargetProfile,
        schema: ServiceSchema,
        binding: DirectForeignBinding,
        boundary_entry_plan: Option<BoundaryEntryPlan>,
        binding_identities: std::sync::Arc<terminal_psi::TerminalIdentityTable>,
        origin: [u8; 32],
    ) -> Result<Self, String> {
        let mut digest = Sha256::new();
        digest.update(b"omega.direct-foreign-realization.v2\0");
        for text in [
            &callable_identity,
            &overload_identity,
            &requirement_identity,
        ] {
            digest.update((text.len() as u64).to_be_bytes());
            digest.update(text.as_bytes());
        }
        digest.update(target.target_name().as_bytes());
        digest.update(schema.identity_digest().as_bytes());
        digest.update(origin);
        // Strong calling commitment retains all ABI coordinates.
        match &boundary_entry_plan {
            Some(plan) => {
                digest.update([1]);
                digest.update(plan.contract_commitment_digest());
            }
            None => digest.update([0]),
        }
        let identity = digest.finalize().into();
        Ok(Self {
            callable_identity,
            overload_identity,
            requirement_identity,
            target,
            schema,
            binding,
            boundary_entry_plan,
            binding_identities,
            identity,
        })
    }
    pub fn callable_identity(&self) -> &str {
        &self.callable_identity
    }
    pub fn overload_identity(&self) -> &str {
        &self.overload_identity
    }
    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }
    pub const fn target(&self) -> target::TargetProfile {
        self.target
    }
    pub const fn schema(&self) -> &ServiceSchema {
        &self.schema
    }
    pub const fn binding(&self) -> &DirectForeignBinding {
        &self.binding
    }
    pub fn boundary_entry_plan(&self) -> Option<&BoundaryEntryPlan> {
        self.boundary_entry_plan.as_ref()
    }
    pub fn binding_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.binding_identities
    }
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
}
