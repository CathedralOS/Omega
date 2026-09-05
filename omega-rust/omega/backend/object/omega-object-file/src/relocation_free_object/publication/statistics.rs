//! Counts of current object records; no source or publication authority.
use super::FunctionFragmentObjectContainerStatistics;
use crate::{RelocationFreeObjectContainer, RelocationFreeObjectError, RelocationFreeObjectPlan};

pub fn relocation_free_object_statistics(
    object: &RelocationFreeObjectPlan,
    container: &RelocationFreeObjectContainer,
) -> Result<FunctionFragmentObjectContainerStatistics, RelocationFreeObjectError> {
    let symbols = u64::try_from(object.symbols.len())
        .map_err(|_| RelocationFreeObjectError::LengthOverflow)?;
    let container_bytes = u64::try_from(container.bytes.len())
        .map_err(|_| RelocationFreeObjectError::LengthOverflow)?;
    Ok(FunctionFragmentObjectContainerStatistics {
        sections: 1,
        function_symbols: symbols,
        object_local_symbols: symbols,
        external_symbols: 0,
        text_bytes: object.text_section.byte_count,
        container_bytes,
        relocation_records: object.relocation_record_count,
    })
}
