//! Typed descriptor construction, descended by translation semantics.

mod immediate;
mod parameter;
mod terminal;

use super::super::{
    straight_line_boolean_immediate, straight_line_integer_immediate, straight_line_parameter,
    straight_line_scalar_crash,
};
use super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(super) const STRAIGHT_LINE_INTEGER_IMMEDIATE: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
        straight_line_integer_immediate::is_candidate,
        immediate::straight_line_integer_immediate,
    );

pub(super) const STRAIGHT_LINE_BOOLEAN_IMMEDIATE: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanImmediate,
        straight_line_boolean_immediate::is_candidate,
        immediate::straight_line_boolean_immediate,
    );

pub(super) const STRAIGHT_LINE_SCALAR_CRASH: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineScalarCrash,
        straight_line_scalar_crash::is_candidate,
        terminal::straight_line_scalar_crash,
    );

pub(super) const STRAIGHT_LINE_INTEGER_PARAMETER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerParameter,
        straight_line_parameter::integer::direct::is_candidate,
        parameter::direct::integer,
    );

pub(super) const STRAIGHT_LINE_BOOLEAN_PARAMETER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanParameter,
        straight_line_parameter::boolean::direct::is_candidate,
        parameter::direct::boolean,
    );

pub(super) const STRAIGHT_LINE_BOOLEAN_NOT_PARAMETER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanNotParameter,
        straight_line_parameter::boolean::not::is_candidate,
        parameter::unary::boolean_not,
    );

pub(super) const STRAIGHT_LINE_BOOLEAN_EQUAL_PARAMETERS: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanEqualParameters,
        straight_line_parameter::boolean::equal::is_candidate,
        parameter::comparison::boolean_equal,
    );

pub(super) const STRAIGHT_LINE_INTEGER_EQUAL_PARAMETERS: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerEqualParameters,
        straight_line_parameter::integer::comparison::equal::is_candidate,
        parameter::comparison::integer_equal,
    );

pub(super) const STRAIGHT_LINE_INTEGER_LESS_THAN_PARAMETERS: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerLessThanParameters,
        straight_line_parameter::integer::comparison::less_than::is_candidate,
        parameter::comparison::integer_less_than,
    );

pub(super) const STRAIGHT_LINE_INTEGER_LESS_OR_EQUAL_PARAMETERS: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualParameters,
        straight_line_parameter::integer::comparison::less_or_equal::is_candidate,
        parameter::comparison::integer_less_or_equal,
    );

pub(super) const STRAIGHT_LINE_INTEGER_BITWISE_NOT_PARAMETER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotParameter,
        straight_line_parameter::integer::unary::bitwise_not::is_candidate,
        parameter::unary::integer_bitwise_not,
    );

pub(super) const STRAIGHT_LINE_INTEGER_WIDEN_PARAMETER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerWidenParameter,
        straight_line_parameter::integer::unary::widen::is_candidate,
        parameter::unary::integer_widen,
    );
