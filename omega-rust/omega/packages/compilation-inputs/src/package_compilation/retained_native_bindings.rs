//! Immutable native intent exported only by a completed checked package.
//!
//! These rows retain source custody and the original evaluation, not execution
//! authority. No receiving symbol, source id or generic application handle is
//! serialized. An applied tuple that is not formed in the receiving world stays
//! applied and unavailable; it never becomes a universal requirement binding.

use effects::provider_plan::EvaluatedBindingReceipt;
use language_semantics::declaration_selection::{
    BuildSelectionOrigin, NativeBindingValue, ProductTarget,
};
use symbols::RetainedDeclaration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetainedNativeApplication {
    Universal,
    Applied {
        outer: Option<String>,
        local: Option<RetainedNativeLocalApplication>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedNativeLocalApplication {
    pub arguments: Vec<String>,
    pub lifetimes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedNativeTable {
    pub table: RetainedDeclaration,
    pub table_identity: String,
    pub field: RetainedDeclaration,
    pub field_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedNativeSameStackDemand {
    pub bytes: u64,
    pub origin: BuildSelectionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedNativeBinding {
    pub requirement: RetainedDeclaration,
    pub requirement_identity: String,
    pub application: RetainedNativeApplication,
    pub origin: BuildSelectionOrigin,
    /// Exact original lexical author's nominal path, independent of whether
    /// its private declaration is part of the receiving Product.
    pub author_path: String,
    pub target: ProductTarget,
    pub raw_locator: NativeBindingValue,
    pub widths: [u64; 3],
    pub table: Option<RetainedNativeTable>,
    pub same_stack_demand: Option<RetainedNativeSameStackDemand>,
    pub unavailable: Option<String>,
    /// Complete sponsoring invocation accounting, not per-locator execution.
    pub usage: crate::BuildEvaluationUsage,
    pub observation: [u8; 32],
    /// Receipt namespace is reconstructed from this exact original text. Its
    /// digest must remain identical; no receiving Build callable is substituted.
    pub producer_callable: String,
    pub receipt: EvaluatedBindingReceipt,
}

impl RetainedNativeBinding {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !matches!(
            self.requirement.kind(),
            symbols::SymbolKind::State | symbols::SymbolKind::Machine
        ) || self.requirement_identity.is_empty()
            || self.author_path.is_empty()
            || self.origin.event_ordinal().is_none()
            || self.producer_callable.is_empty()
            || self.observation == [0; 32]
            || self.usage.validate().is_err()
            || self.receipt.producer_package() != self.origin.author().source().package_identity
        {
            return Err(
                "retained native intent has incomplete declaration, invocation or supplier custody",
            );
        }
        if let RetainedNativeApplication::Applied { outer, local } = &self.application {
            if (outer.is_none() && local.is_none())
                || outer.as_ref().is_some_and(String::is_empty)
                || local.as_ref().is_some_and(|local| {
                    local
                        .arguments
                        .iter()
                        .chain(&local.lifetimes)
                        .any(String::is_empty)
                })
            {
                return Err("retained native application has an incomplete portable tuple");
            }
        }
        let table_locator = matches!(
            self.raw_locator,
            NativeBindingValue::TableFunction | NativeBindingValue::VtableField
        );
        if table_locator != self.table.is_some()
            || (table_locator && self.widths != [0; 3])
            || matches!(self.raw_locator, NativeBindingValue::SameStackDemand { .. })
        {
            return Err("retained native locator and designated field disagree");
        }
        if let Some(table) = &self.table {
            if table.table.kind() != symbols::SymbolKind::Data
                || table.field.kind() != symbols::SymbolKind::Field
                || table.table_identity.is_empty()
                || table.field_identity.is_empty()
            {
                return Err("retained native table lost its exact data and field designation");
            }
        }
        if let Some(demand) = &self.same_stack_demand {
            if demand.bytes == 0
                || demand.origin.event_ordinal().is_none()
                || demand.origin.activation() != self.origin.activation()
                || demand.origin.author().source().package_identity
                    != self.origin.author().source().package_identity
            {
                return Err("retained native footprint has a different activation or supplier");
            }
        }
        let usage = self.receipt.evaluation_usage();
        if usage.usage_schema_version() != self.usage.usage_schema_version
            || usage.step_schedule_marker() != self.usage.step_schedule_marker
            || usage.fuel_units() != self.usage.fuel_units
            || usage.fuel_ceiling() != self.usage.invocation_fuel_ceiling
            || usage.build_log_bytes() != self.usage.build_log_bytes
            || usage.filesystem_operation_attempts() != self.usage.filesystem_operation_attempts
            || usage.peak_live_cells() != self.usage.peak_live_storage_cells
            || usage.peak_live_text_bytes() != self.usage.peak_live_payload_bytes
            || usage.result_cells() != self.usage.result_storage_cells
            || usage.result_text_bytes() != self.usage.result_payload_bytes
        {
            return Err("retained native receipt and sponsoring Build usage disagree");
        }
        Ok(())
    }

    pub fn anchors(&self) -> impl Iterator<Item = &RetainedDeclaration> {
        std::iter::once(&self.requirement).chain(
            self.table
                .iter()
                .flat_map(|table| [&table.table, &table.field]),
        )
    }
}
