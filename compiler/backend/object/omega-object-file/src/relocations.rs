use crate::ObjectSymbolHandle;
use omega_core::arena::{Arena, Handle};
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationPlan {
    pub target: NativeTarget,
    pub record_set: RelocationRecordSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationRecordSet {
    pub records: Arena<RelocationRecord>,
}

impl RelocationRecordSet {
    pub fn with_roots(records: Arena<RelocationRecord>) -> Self {
        Self { records }
    }

    pub fn with_capacity(record_capacity: usize) -> Self {
        Self::with_roots(Arena::with_capacity(record_capacity))
    }
}

impl Default for RelocationPlan {
    fn default() -> Self {
        Self::with_target(NativeTarget::host())
    }
}

impl RelocationPlan {
    pub fn with_target(target: NativeTarget) -> Self {
        Self::with_record_capacity(target, 0)
    }

    pub fn with_roots(target: NativeTarget, record_set: RelocationRecordSet) -> Self {
        Self { target, record_set }
    }

    pub fn with_record_capacity(target: NativeTarget, record_capacity: usize) -> Self {
        Self::with_roots(target, RelocationRecordSet::with_capacity(record_capacity))
    }

    pub fn push_record(&mut self, record: RelocationRecord) -> Handle<RelocationRecord> {
        self.record_set.records.insert(record)
    }

    pub fn record_count(&self) -> usize {
        self.record_set.records.len()
    }

    pub fn records(&self) -> impl Iterator<Item = (Handle<RelocationRecord>, &RelocationRecord)> {
        self.record_set.records.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationRecord {
    pub function_symbol_handle: ObjectSymbolHandle,
    pub selected_instruction_index: u32,
    pub text_offset: usize,
    pub byte_width: usize,
    pub symbol_handle: ObjectSymbolHandle,
    pub kind: RelocationKind,
}

impl Default for RelocationRecord {
    fn default() -> Self {
        Self {
            function_symbol_handle: Handle::invalid(),
            selected_instruction_index: 0,
            text_offset: 0,
            byte_width: 0,
            symbol_handle: Handle::invalid(),
            kind: RelocationKind::Aarch64Branch26,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationKind {
    Aarch64Page21,
    Aarch64PageOffset12,
    Aarch64Branch26,
    X86_64Absolute64,
    X86_64Relative32,
}

#[cfg(test)]
mod tests {
    use crate::{RelocationPlan, RelocationRecord, RelocationRecordSet};
    use omega_core::arena::Arena;
    use omega_target::NativeTarget;

    #[test]
    fn relocation_record_set_constructor_keeps_record_root_explicit() {
        let records = Arena::<RelocationRecord>::with_capacity(3);

        let record_set = RelocationRecordSet::with_roots(records.clone());

        assert_eq!(record_set.records, records);
    }

    #[test]
    fn relocation_plan_constructor_keeps_target_and_record_roots_explicit() {
        let target = NativeTarget::linux_arm64();
        let record_set = RelocationRecordSet::with_capacity(2);

        let plan = RelocationPlan::with_roots(target, record_set.clone());

        assert_eq!(plan.target, target);
        assert_eq!(plan.record_set, record_set);
    }
}
