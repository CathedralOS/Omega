//! External binding rows: how one host operation binds to a foreign symbol.

use crate::plans::BoundaryEntryPlan;
use target::NormalizedForeignLocator;

/// One selected bodyless external leaf retained for target realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalBindingRow {
    /// The leaf's target identifier, retained for exact target realization.
    pub target_name: String,
    pub trait_name: String,
    pub method: String,
    /// Exact canonical overload identity. The human method name remains only
    /// readable drift data, including for singleton requirements.
    pub requirement_identity: String,
    /// The selected provider's nominal identity, rejoining this row to its
    /// selected plan. Empty for free leaves. It never names a dispatch table:
    /// a table binding reads its table from the leaf's first parameter.
    pub provider_type: String,
    /// Complete admitted geometry and plan for this concrete service method,
    /// whether its policy is authored or the target's ordinary native default.
    pub calling_applications: Vec<crate::BoundaryCallingApplication>,
    pub binding: ExternalBindingKind,
}

impl ExternalBindingRow {
    /// Physical placement and semantic geometry have one immutable owner.
    /// Consumers cannot update one while accidentally retaining the other.
    pub fn calling_application(
        &self,
        abi: &semantic_vocabulary::CallingAbiIdentity,
    ) -> Option<&crate::BoundaryCallingApplication> {
        let mut applications = self
            .calling_applications
            .iter()
            .filter(|application| &application.abi == abi);
        let application = applications.next()?;
        applications.next().is_none().then_some(application)
    }

    pub fn boundary_entry_plan(
        &self,
        abi: &semantic_vocabulary::CallingAbiIdentity,
    ) -> Option<&BoundaryEntryPlan> {
        self.calling_application(abi)
            .map(|application| application.plan.plan())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalBindingKind {
    Syscall {
        number: i64,
    },
    /// One evaluated, target-normalized foreign locator. Raw foreign bytes are
    /// data; a string pair authored in source never reaches this row.
    Import {
        locator: NormalizedForeignLocator,
    },
    /// Select an existing compiler-known platform lowering. The retained
    /// string is the exact normalized realization-machine overload identity.
    CompilerIntrinsic {
        machine: String,
    },
    VtableSlot {
        index: i64,
    },
    /// Dispatch by a fn-ptr field of the record the leaf's first parameter
    /// borrows; the layout plan supplies its byte offset.
    VtableField {
        field: String,
        byte_offset: u32,
    },
    /// Dispatch by fn-ptr FIELD like `VtableField`, but the table pointer is
    /// DISPATCH-ONLY -- never a wire argument (EFI table services take no
    /// This; protocol/COM methods do).
    TableFunction {
        field: String,
        byte_offset: u32,
    },
}
