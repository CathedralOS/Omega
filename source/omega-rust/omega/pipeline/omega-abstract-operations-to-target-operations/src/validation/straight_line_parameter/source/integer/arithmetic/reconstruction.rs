//! Exact arithmetic-family source envelope selection and replay join.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::super::model::{
    ExactIntegerAddParametersSource, ExactIntegerDivideParametersSource,
    ExactIntegerMultiplyParametersSource, ExactIntegerRemainderParametersSource,
    ExactIntegerSubtractParametersSource, IntegerArithmeticParametersSource,
    SaturatingIntegerDivideParametersSource, WrappingIntegerDivideParametersSource,
    WrappingIntegerRemainderParametersSource,
};
use crate::validation::model::{
    StraightLineExactIntegerAddParametersTranslationError,
    StraightLineExactIntegerDivideParametersTranslationError,
    StraightLineExactIntegerMultiplyParametersTranslationError,
    StraightLineExactIntegerRemainderParametersTranslationError,
    StraightLineExactIntegerSubtractParametersTranslationError,
    StraightLineSaturatingIntegerAddParametersTranslationError,
    StraightLineSaturatingIntegerDivideParametersTranslationError,
    StraightLineSaturatingIntegerMultiplyParametersTranslationError,
    StraightLineSaturatingIntegerSubtractParametersTranslationError,
    StraightLineWrappingIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerDivideParametersTranslationError,
    StraightLineWrappingIntegerMultiplyParametersTranslationError,
    StraightLineWrappingIntegerRemainderParametersTranslationError,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
};

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_add(
    function: &AbstractFunction,
) -> Result<ExactIntegerAddParametersSource, StraightLineExactIntegerAddParametersTranslationError>
{
    let Some(AbstractOperation::ExactIntegerAdd { scalar_type, .. }) = function.operations.first()
    else {
        return Err(StraightLineExactIntegerAddParametersTranslationError::SourceOperationRoster);
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    super::exact_add::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_subtract(
    function: &AbstractFunction,
) -> Result<
    ExactIntegerSubtractParametersSource,
    StraightLineExactIntegerSubtractParametersTranslationError,
> {
    let Some(AbstractOperation::ExactIntegerSubtract { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineExactIntegerSubtractParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    super::exact_subtract::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_multiply(
    function: &AbstractFunction,
) -> Result<
    ExactIntegerMultiplyParametersSource,
    StraightLineExactIntegerMultiplyParametersTranslationError,
> {
    let Some(AbstractOperation::ExactIntegerMultiply { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineExactIntegerMultiplyParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    super::exact_multiply::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_divide(
    function: &AbstractFunction,
) -> Result<
    ExactIntegerDivideParametersSource,
    StraightLineExactIntegerDivideParametersTranslationError,
> {
    let Some(AbstractOperation::ExactIntegerDivide { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineExactIntegerDivideParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    super::exact_divide::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_remainder(
    function: &AbstractFunction,
) -> Result<
    ExactIntegerRemainderParametersSource,
    StraightLineExactIntegerRemainderParametersTranslationError,
> {
    let Some(AbstractOperation::ExactIntegerRemainder { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineExactIntegerRemainderParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    super::exact_remainder::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_divide(
    function: &AbstractFunction,
) -> Result<
    WrappingIntegerDivideParametersSource,
    StraightLineWrappingIntegerDivideParametersTranslationError,
> {
    let Some(AbstractOperation::WrappingIntegerDivide { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineWrappingIntegerDivideParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    super::wrapping_divide::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_remainder(
    function: &AbstractFunction,
) -> Result<
    WrappingIntegerRemainderParametersSource,
    StraightLineWrappingIntegerRemainderParametersTranslationError,
> {
    let Some(AbstractOperation::WrappingIntegerRemainder { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineWrappingIntegerRemainderParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    super::wrapping_remainder::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_saturating_divide(
    function: &AbstractFunction,
) -> Result<
    SaturatingIntegerDivideParametersSource,
    StraightLineSaturatingIntegerDivideParametersTranslationError,
> {
    let Some(AbstractOperation::SaturatingIntegerDivide { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineSaturatingIntegerDivideParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    super::saturating_divide::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_saturating_add(
    function: &AbstractFunction,
) -> Result<
    IntegerArithmeticParametersSource,
    StraightLineSaturatingIntegerAddParametersTranslationError,
> {
    let Some(AbstractOperation::SaturatingIntegerAdd { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineSaturatingIntegerAddParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    super::saturating_add::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_saturating_subtract(
    function: &AbstractFunction,
) -> Result<
    IntegerArithmeticParametersSource,
    StraightLineSaturatingIntegerSubtractParametersTranslationError,
> {
    let Some(AbstractOperation::SaturatingIntegerSubtract { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    super::saturating_subtract::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_saturating_multiply(
    function: &AbstractFunction,
) -> Result<
    IntegerArithmeticParametersSource,
    StraightLineSaturatingIntegerMultiplyParametersTranslationError,
> {
    let Some(AbstractOperation::SaturatingIntegerMultiply { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineSaturatingIntegerMultiplyParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    super::saturating_multiply::reconstruct(function, &envelope)
}

macro_rules! reconstruct_wrapping {
    ($name:ident, $variant:ident, $leaf:ident, $error:ty) => {
        pub(in crate::validation::straight_line_parameter) fn $name(
            function: &AbstractFunction,
        ) -> Result<IntegerArithmeticParametersSource, $error> {
            let Some(AbstractOperation::$variant { scalar_type, .. }) = function.operations.first()
            else {
                return Err(<$error>::SourceOperationRoster);
            };
            let envelope = super::super::super::envelope::reconstruct(
                function,
                ScalarType::Integer(*scalar_type),
            )?;
            super::$leaf::reconstruct(function, &envelope)
        }
    };
}

reconstruct_wrapping!(
    reconstruct_wrapping_add,
    WrappingIntegerAdd,
    wrapping_add,
    StraightLineWrappingIntegerAddParametersTranslationError
);
reconstruct_wrapping!(
    reconstruct_wrapping_subtract,
    WrappingIntegerSubtract,
    wrapping_subtract,
    StraightLineWrappingIntegerSubtractParametersTranslationError
);
reconstruct_wrapping!(
    reconstruct_wrapping_multiply,
    WrappingIntegerMultiply,
    wrapping_multiply,
    StraightLineWrappingIntegerMultiplyParametersTranslationError
);
