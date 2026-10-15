//! Owned block storage has the same address authority for every scalar store.
//! Its exact arrival declaration, rather than a predecessor producer, owns it.

use semantic_vocabulary::{BlockId, PlaceId};
use std::collections::BTreeMap;
use target_operations::{TargetStructuralHomeOrigin, TargetStructuralHomeRequirement};
use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};

pub(super) fn block_home<'homes>(
    homes: &'homes BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
    block: BlockId,
    parameter: &StructuralParameterDeclaration,
) -> Option<&'homes TargetStructuralHomeRequirement> {
    let home = homes.get(&parameter.place)?;
    (parameter.access == StructuralAccess::Owned
        && matches!(&home.origin,
            TargetStructuralHomeOrigin::BlockParameter { block: owner, declaration }
                if *owner == block && declaration == parameter))
    .then_some(home)
}
