use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_byte_sequence_literal_unit_return,
    straight_line_ieee_float_literal_sequence_unit_return,
    straight_line_ieee_float_literal_unit_return,
    straight_line_integer_literal_sequence_unit_return, straight_line_integer_literal_unit_return,
    straight_line_nearest_ieee_float_fused_multiply_add_unit_return,
    straight_line_port_write_unit_return, straight_line_scalar_crash,
    straight_line_trivial_affine_local_unit_return, straight_line_unit_call_return,
    straight_line_unit_return,
};
use super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const UNIT_RETURN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineUnitReturn,
        straight_line_unit_return::is_candidate,
        straight_line_unit_return,
    );

pub(in crate::validation::catalog) const PORT_WRITE_UNIT_RETURN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLinePortWriteUnitReturn,
        straight_line_port_write_unit_return::is_candidate,
        straight_line_port_write_unit_return,
    );

pub(in crate::validation::catalog) const UNIT_CALL_RETURN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineUnitCallReturn,
        straight_line_unit_call_return::is_candidate,
        straight_line_unit_call_return,
    );

pub(in crate::validation::catalog) const BYTE_SEQUENCE_LITERAL_UNIT_RETURN:
    TranslationFamilyDescriptor = TranslationFamilyDescriptor::new(
    AbstractToTargetTranslationFamily::StraightLineByteSequenceLiteralUnitReturn,
    straight_line_byte_sequence_literal_unit_return::is_candidate,
    straight_line_byte_sequence_literal_unit_return,
);

pub(in crate::validation::catalog) const INTEGER_LITERAL_UNIT_RETURN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerLiteralUnitReturn,
        straight_line_integer_literal_unit_return::is_candidate,
        straight_line_integer_literal_unit_return,
    );

pub(in crate::validation::catalog) const INTEGER_LITERAL_SEQUENCE_UNIT_RETURN:
    TranslationFamilyDescriptor = TranslationFamilyDescriptor::new(
    AbstractToTargetTranslationFamily::StraightLineIntegerLiteralSequenceUnitReturn,
    straight_line_integer_literal_sequence_unit_return::is_candidate,
    straight_line_integer_literal_sequence_unit_return,
);

pub(in crate::validation::catalog) const IEEE_FLOAT_LITERAL_UNIT_RETURN:
    TranslationFamilyDescriptor = TranslationFamilyDescriptor::new(
    AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralUnitReturn,
    straight_line_ieee_float_literal_unit_return::is_candidate,
    straight_line_ieee_float_literal_unit_return,
);

pub(in crate::validation::catalog) const IEEE_FLOAT_LITERAL_SEQUENCE_UNIT_RETURN:
    TranslationFamilyDescriptor = TranslationFamilyDescriptor::new(
    AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralSequenceUnitReturn,
    straight_line_ieee_float_literal_sequence_unit_return::is_candidate,
    straight_line_ieee_float_literal_sequence_unit_return,
);

pub(in crate::validation::catalog) const NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD_UNIT_RETURN:
    TranslationFamilyDescriptor = TranslationFamilyDescriptor::with_ieee_float_fma(
    AbstractToTargetTranslationFamily::StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn,
    straight_line_nearest_ieee_float_fused_multiply_add_unit_return::is_candidate,
    straight_line_nearest_ieee_float_fused_multiply_add_unit_return,
);

pub(in crate::validation::catalog) const TRIVIAL_AFFINE_LOCAL_UNIT_RETURN:
    TranslationFamilyDescriptor = TranslationFamilyDescriptor::new(
    AbstractToTargetTranslationFamily::StraightLineTrivialAffineLocalUnitReturn,
    straight_line_trivial_affine_local_unit_return::is_candidate,
    straight_line_trivial_affine_local_unit_return,
);

pub(in crate::validation::catalog) const SCALAR_CRASH: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineScalarCrash,
        straight_line_scalar_crash::is_candidate,
        straight_line_scalar_crash,
    );

pub(super) fn straight_line_scalar_crash(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_scalar_crash::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineScalarCrash)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineScalarCrash)
}

pub(super) fn straight_line_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_unit_return::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineUnitReturn)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineUnitReturn)
}

pub(super) fn straight_line_port_write_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_port_write_unit_return::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLinePortWriteUnitReturn)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLinePortWriteUnitReturn)
}

pub(super) fn straight_line_unit_call_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_unit_call_return::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineUnitCallReturn)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineUnitCallReturn)
}

pub(super) fn straight_line_byte_sequence_literal_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_byte_sequence_literal_unit_return::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineByteSequenceLiteralUnitReturn)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineByteSequenceLiteralUnitReturn)
}

pub(super) fn straight_line_integer_literal_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_integer_literal_unit_return::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLiteralUnitReturn)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerLiteralUnitReturn)
}

pub(super) fn straight_line_integer_literal_sequence_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_integer_literal_sequence_unit_return::validate(source, expected_target, target)
        .map(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLiteralSequenceUnitReturn,
        )
        .map_err(
            AbstractToTargetTranslationFamilyError::StraightLineIntegerLiteralSequenceUnitReturn,
        )
}

pub(super) fn straight_line_ieee_float_literal_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_ieee_float_literal_unit_return::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIeeeFloatLiteralUnitReturn)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIeeeFloatLiteralUnitReturn)
}

pub(super) fn straight_line_ieee_float_literal_sequence_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_ieee_float_literal_sequence_unit_return::validate(
        source,
        expected_target,
        target,
    )
    .map(
        AbstractToTargetFunctionTranslationReceipt::StraightLineIeeeFloatLiteralSequenceUnitReturn,
    )
    .map_err(
        AbstractToTargetTranslationFamilyError::StraightLineIeeeFloatLiteralSequenceUnitReturn,
    )
}

pub(super) fn straight_line_nearest_ieee_float_fused_multiply_add_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    settlements: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_nearest_ieee_float_fused_multiply_add_unit_return::validate(
        source,
        expected_target,
        target,
        settlements,
    )
    .map(
        AbstractToTargetFunctionTranslationReceipt::StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn,
    )
    .map_err(
        AbstractToTargetTranslationFamilyError::StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn,
    )
}

pub(super) fn straight_line_trivial_affine_local_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_trivial_affine_local_unit_return::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineTrivialAffineLocalUnitReturn)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineTrivialAffineLocalUnitReturn)
}
