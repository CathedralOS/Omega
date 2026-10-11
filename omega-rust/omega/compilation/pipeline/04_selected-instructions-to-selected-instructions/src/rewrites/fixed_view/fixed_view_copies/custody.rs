use crate::FixedViewCopyValidationReceipt;

use crate::StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt;

use super::StagedOptimizedFixedViewCopyCustodyReceipt;

pub(super) fn fixed_view_copy_custody_receipt(
    segment_homes: StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
    copies: FixedViewCopyValidationReceipt,
) -> StagedOptimizedFixedViewCopyCustodyReceipt {
    StagedOptimizedFixedViewCopyCustodyReceipt {
        core: segment_homes.upstream().core(),
        allocator_availability: segment_homes.upstream().allocator_availability(),
        source_selected: segment_homes.upstream().selected(),
        source_liveness: segment_homes.upstream().liveness(),
        source_ranges: segment_homes.upstream().ranges(),
        source_legality: segment_homes.upstream().legality(),
        fixed_intervals: segment_homes.fixed().identity(),
        split_requirements: segment_homes.requirements().identity(),
        segment_homes: segment_homes.homes().identity(),
        transformation: copies.identity(),
        transformed_selected: copies.transformed_selected(),
        policy: copies.policy(),
        usage: copies.usage(),
        function_count: copies.function_count(),
        copy_count: copies.copy_count(),
    }
}
