//! Nominal native-layout provenance beside canonical semantic storage.
//!
//! Structural storage owns field placement used by execution. This catalog
//! retains the complete authored report and selected private slots needed for
//! native ABI identity; those cannot be reconstructed from size/offsets alone.
//! Natural records retain nominal identities but need no authored report.

use crate::{TargetLayoutInput, TerminalIdentityIndex};
use arena::{Arena, HandleSpan};
use language_semantics::callback_declarations::PrivateCallbackSlotSelection;
use layout_plans::NativeLayoutPlanReport;
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeLayoutCatalog {
    records: Arena<NativeRecordLayout>,
    fields: Arena<NativeLayoutFieldIdentity>,
    realizations: Arena<NativeLayoutRealization>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRecordLayout {
    pub record: StructuralTypeId,
    pub data_identity: TerminalIdentityIndex,
    pub policy_identity: TerminalIdentityIndex,
    fields: HandleSpan<NativeLayoutFieldIdentity>,
    realizations: HandleSpan<NativeLayoutRealization>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLayoutFieldIdentity {
    pub field: StructuralFieldId,
    pub declaration_identity: TerminalIdentityIndex,
}

impl Default for NativeRecordLayout {
    fn default() -> Self {
        Self {
            record: StructuralTypeId::new(1).expect("arena sentinel"),
            data_identity: Default::default(),
            policy_identity: Default::default(),
            fields: HandleSpan::empty(),
            realizations: HandleSpan::empty(),
        }
    }
}

impl Default for NativeLayoutFieldIdentity {
    fn default() -> Self {
        Self {
            field: StructuralFieldId::new(1).expect("arena sentinel"),
            declaration_identity: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLayoutRealization {
    /// None is one invariant result, not an arbitrary target representative.
    pub target: Option<TargetLayoutInput>,
    pub report: NativeLayoutPlanReport<PrivateCallbackSlotSelection>,
}

impl Default for NativeLayoutRealization {
    fn default() -> Self {
        Self {
            target: None,
            report: NativeLayoutPlanReport {
                layout: layout_plans::LayoutPlanReport {
                    schema_report_fingerprint: 0,
                    entries: Vec::new(),
                    offsets: None,
                    size: None,
                    align: 1,
                },
                private_callback_demands: Vec::new(),
            },
        }
    }
}

impl NativeLayoutCatalog {
    pub fn records(&self) -> impl Iterator<Item = &NativeRecordLayout> {
        self.records.iter().map(|(_, record)| record)
    }

    pub fn record(&self, identity: StructuralTypeId) -> Option<&NativeRecordLayout> {
        self.records().find(|record| record.record == identity)
    }

    pub fn fields(
        &self,
        record: &NativeRecordLayout,
    ) -> Result<&[NativeLayoutFieldIdentity], String> {
        self.fields
            .span(record.fields)
            .ok_or_else(|| "native layout has stale nominal field storage".into())
    }

    pub fn realizations(
        &self,
        record: &NativeRecordLayout,
    ) -> Result<&[NativeLayoutRealization], String> {
        self.realizations
            .span(record.realizations)
            .ok_or_else(|| "native layout has stale realization storage".into())
    }

    pub fn insert(
        &mut self,
        record: StructuralTypeId,
        data_identity: TerminalIdentityIndex,
        policy_identity: TerminalIdentityIndex,
        fields: impl IntoIterator<Item = NativeLayoutFieldIdentity>,
        realizations: impl IntoIterator<Item = NativeLayoutRealization>,
    ) {
        let fields = self.fields.insert_many(fields);
        let realizations = self.realizations.insert_many(realizations);
        self.records.insert(NativeRecordLayout {
            record,
            data_identity,
            policy_identity,
            fields,
            realizations,
        });
    }

    /// Close exactly one retained alternative; missing or repeated members do
    /// not authorize Source fallback or ordinary packing.
    pub fn for_target(
        &self,
        record: &NativeRecordLayout,
        target: &str,
    ) -> Result<Option<&NativeLayoutRealization>, String> {
        let realizations = self.realizations(record)?;
        if record.policy_identity.is_absent() {
            if !realizations.is_empty() {
                return Err("natural native layout carries authored realizations".into());
            }
            return Ok(None);
        }
        let mut selected = realizations.iter().filter(|realization| {
            realization
                .target
                .as_ref()
                .is_none_or(|facts| facts.target == target)
        });
        let result = selected
            .next()
            .ok_or("native layout has no realization for the selected target")?;
        if selected.next().is_some() {
            return Err("native layout has multiple realizations for the selected target".into());
        }
        Ok(Some(result))
    }
}
