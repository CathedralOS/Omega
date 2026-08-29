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
        straight_line_parameter::integer::is_candidate,
        parameter::straight_line_integer_parameter,
    );

pub(super) const STRAIGHT_LINE_BOOLEAN_PARAMETER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanParameter,
        straight_line_parameter::boolean::is_candidate,
        parameter::straight_line_boolean_parameter,
    );

pub(super) const STRAIGHT_LINE_BOOLEAN_NOT_PARAMETER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanNotParameter,
        straight_line_parameter::boolean_not::is_candidate,
        parameter::straight_line_boolean_not_parameter,
    );

pub(super) const STRAIGHT_LINE_BOOLEAN_EQUAL_PARAMETERS: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanEqualParameters,
        straight_line_parameter::boolean_equal::is_candidate,
        parameter::straight_line_boolean_equal_parameters,
    );

pub(super) const STRAIGHT_LINE_INTEGER_EQUAL_PARAMETERS: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerEqualParameters,
        straight_line_parameter::integer_equal::is_candidate,
        parameter::straight_line_integer_equal_parameters,
    );

pub(super) const STRAIGHT_LINE_INTEGER_LESS_THAN_PARAMETERS: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerLessThanParameters,
        straight_line_parameter::integer_less_than::is_candidate,
        parameter::straight_line_integer_less_than_parameters,
    );
