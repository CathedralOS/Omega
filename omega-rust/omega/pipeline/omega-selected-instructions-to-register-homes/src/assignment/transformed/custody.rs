use crate::{ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes};

use crate::{
    StagedOptimizedLiteralFoldCustodyReceipt, StagedSelectedLoweringOptimizationCustodyReceipt,
};

use super::model::{
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
};

pub(super) fn literal_fold_home_custody_receipt(
    source: StagedOptimizedLiteralFoldCustodyReceipt,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
) -> StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
        source,
        homes: homes.receipt().identity(),
        post_allocation_manifest: manifest.record().identity,
        function_count: homes.receipt().function_count(),
        assignment_count: homes.receipt().assignment_count(),
    }
}

pub(super) fn selected_lowering_home_custody_receipt(
    source: StagedSelectedLoweringOptimizationCustodyReceipt,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
) -> StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
        source,
        homes: homes.receipt().identity(),
        post_allocation_manifest: manifest.record().identity,
        function_count: homes.receipt().function_count(),
        assignment_count: homes.receipt().assignment_count(),
    }
}
