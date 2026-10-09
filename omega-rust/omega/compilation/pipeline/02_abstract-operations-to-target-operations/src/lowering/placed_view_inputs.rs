use abstract_operations::{AbstractFunctionResult, AbstractOperationPlanWithPlacedViewInputs};
use calling_conventions::{CallPlan, CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use target::NativeTarget;
use target_operations::{TargetOperationPlanWithPlacedViewInputs, TargetPlacedViewInput};

use crate::{
    LoweringError, PlacedViewInputTranslationError, SelectedPlacedViewInputPlan,
    TargetLoweringRequest, lower_to_target_operations,
};

