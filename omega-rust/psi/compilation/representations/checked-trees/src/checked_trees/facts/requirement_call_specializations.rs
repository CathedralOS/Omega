//! Checked specializations retained for generic requirement calls.
//!
//! MP2b admission derives the callee's generic substitutions while judging
//! each static machine argument. A nominal contract replays that judgment
//! into a `CheckedNominalMachineUse` row; a structural contract emits no
//! satisfaction row, so the derived specialization was discarded at
//! validation and `build_call_operation` could never recover what the call
//! instantiated. These facts carry the substitution across the checked
//! boundary, keyed by the same call-site identity nominal uses retain.

use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

use super::nominal_machine_uses::NominalMachineUseSite;

/// One `Type` generic-parameter substitution a generic call's admitted
/// static machine selections derived: the selected callable's shape pinned
/// the callee's `Type` parameter to this actual type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedRequirementCallTypeBinding {
    /// The callee's generic `Type` parameter.
    pub parameter: SymbolHandle,
    /// The actual type the selected callable's shape bound to it.
    pub actual: TypeReferenceHandle,
}

/// One admitted `machine`-binder selection at a generic call edge, keyed by
/// the same telescope ordinal nominal use rows retain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedRequirementCallMachineSelection {
    /// Telescope ordinal of the callee's `machine` parameter the argument
    /// was admitted against.
    pub static_machine_ordinal: u32,
    /// The `machine` parameter's symbol; its retained contract view names
    /// the requirement this selection satisfies.
    pub parameter: SymbolHandle,
    /// The concrete machine owning the selected entry state. Invalid when
    /// `selected` forwards an in-scope `machine` binder.
    pub selected_machine: SymbolHandle,
    /// The admitted selection: the selected concrete machine's resolved
    /// entry state, or the forwarded binder's own parameter symbol.
    pub selected: SymbolHandle,
}

/// The specialization one generic call's admitted static machine arguments
/// derived over the callee's generic parameters: every `Type` substitution
/// plus each `machine` binder's exact provider selection. A call that named
/// no static machine argument retains no row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedRequirementCallSpecialization {
    /// Exact authored call site, keyed identically to nominal use rows.
    pub site: NominalMachineUseSite,
    /// The generic callee the call named: a requirement signature, a machine
    /// parameter contract's call target, or a generic machine's entry state.
    pub registration_operation: SymbolHandle,
    /// Derived `Type` substitutions, in derivation order.
    pub type_bindings: Vec<CheckedRequirementCallTypeBinding>,
    /// One row per admitted static machine argument, in telescope order.
    pub machine_selections: Vec<CheckedRequirementCallMachineSelection>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequirementCallSpecializationFacts {
    pub specializations: Vec<CheckedRequirementCallSpecialization>,
}

impl RequirementCallSpecializationFacts {
    pub fn try_with_specializations(
        specializations: impl IntoIterator<Item = CheckedRequirementCallSpecialization>,
    ) -> Result<Self, String> {
        let mut retained: Vec<CheckedRequirementCallSpecialization> = Vec::new();
        for specialization in specializations {
            if specialization.machine_selections.is_empty() {
                return Err(
                    "requirement call specialization retained no machine selection".to_owned(),
                );
            }
            if let Some(existing) = retained.iter().find(|existing| {
                existing.site == specialization.site
                    && existing.registration_operation == specialization.registration_operation
            }) {
                if *existing != specialization {
                    return Err(format!(
                        "requirement call specialization site {:?} has conflicting admitted specializations",
                        specialization.site
                    ));
                }
                continue;
            }
            retained.push(specialization);
        }
        Ok(Self {
            specializations: retained,
        })
    }

    /// The specialization retained for one authored call site and callee,
    /// when the call named static machine arguments at all.
    pub fn for_site(
        &self,
        site: NominalMachineUseSite,
        registration_operation: SymbolHandle,
    ) -> Option<&CheckedRequirementCallSpecialization> {
        self.specializations.iter().find(|specialization| {
            specialization.site == site
                && specialization.registration_operation == registration_operation
        })
    }
}
