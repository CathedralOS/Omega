use crate::{ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes};

use crate::{LiteralFoldCustodyReceipt, SelectedLoweringOptimizationCustodyReceipt};

use super::model::{PostLiteralFoldHomeCustodyReceipt, PostSelectedLoweringHomeCustodyReceipt};

pub(super) fn literal_fold_home_custody_receipt(
    source: LiteralFoldCustodyReceipt,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
) -> PostLiteralFoldHomeCustodyReceipt {
    PostLiteralFoldHomeCustodyReceipt {
        source,
        homes: homes.receipt().identity(),
        post_allocation_manifest: manifest.record().identity,
        function_count: homes.receipt().function_count(),
        assignment_count: homes.receipt().assignment_count(),
    }
}

pub(super) fn selected_lowering_home_custody_receipt(
    source: SelectedLoweringOptimizationCustodyReceipt,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
) -> PostSelectedLoweringHomeCustodyReceipt {
    PostSelectedLoweringHomeCustodyReceipt {
        source,
        homes: homes.receipt().identity(),
        post_allocation_manifest: manifest.record().identity,
        function_count: homes.receipt().function_count(),
        assignment_count: homes.receipt().assignment_count(),
    }
}
