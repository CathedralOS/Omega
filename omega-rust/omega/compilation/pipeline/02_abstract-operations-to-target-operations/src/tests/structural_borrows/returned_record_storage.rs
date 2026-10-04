//! Returned record loans retain their checked origins whether their backing
//! storage entered as a parameter or was constructed in this activation.

use super::source_plan;
use semantic_vocabulary::PlaceId;
use target::NativeTarget;
use target_operations::{TargetCallResult, TargetUnitOperation};

