//! Compact payload vocabulary for arithmetic-family translation errors.

pub(super) use super::super::parameter::{
    StraightLineExactIntegerAddParametersTranslationError as ExactAddError,
    StraightLineExactIntegerDivideParametersTranslationError as ExactDivideError,
    StraightLineExactIntegerMultiplyParametersTranslationError as ExactMultiplyError,
    StraightLineExactIntegerRemainderParametersTranslationError as ExactRemainderError,
    StraightLineExactIntegerSubtractParametersTranslationError as ExactSubtractError,
    StraightLineSaturatingIntegerAddParametersTranslationError as SaturatingAddError,
    StraightLineSaturatingIntegerDivideParametersTranslationError as SaturatingDivideError,
    StraightLineSaturatingIntegerMultiplyParametersTranslationError as SaturatingMultiplyError,
    StraightLineSaturatingIntegerRemainderParametersTranslationError as SaturatingRemainderError,
    StraightLineSaturatingIntegerSubtractParametersTranslationError as SaturatingSubtractError,
    StraightLineWrappingIntegerAddParametersTranslationError as WrappingAddError,
    StraightLineWrappingIntegerDivideParametersTranslationError as WrappingDivideError,
    StraightLineWrappingIntegerMultiplyParametersTranslationError as WrappingMultiplyError,
    StraightLineWrappingIntegerRemainderParametersTranslationError as WrappingRemainderError,
    StraightLineWrappingIntegerSubtractParametersTranslationError as WrappingSubtractError,
};
