//! Ordered, explicit inventory of independently replayed translation families.
//!
//! Adding or disabling a family happens in `ENABLED_TRANSLATION_FAMILIES`;
//! each row then descends into one source classifier and one replay leaf.

use omega_abstract_operations::AbstractFunction;
use omega_target_operations::TargetFunction;

use super::{
    AbstractToTargetTranslationValidationError, StraightLineBooleanImmediateTranslationReceipt,
    StraightLineIntegerImmediateTranslationReceipt, straight_line_boolean_immediate,
    straight_line_integer_immediate,
};

const ENABLED_TRANSLATION_FAMILIES: &[TranslationFamily] = &[
    TranslationFamily::StraightLineIntegerImmediate,
    TranslationFamily::StraightLineBooleanImmediate,
];

#[derive(Clone, Copy)]
enum TranslationFamily {
    StraightLineIntegerImmediate,
    StraightLineBooleanImmediate,
}

#[derive(Default)]
pub(super) struct ValidatedTranslationFamilies {
    straight_line_integer_immediates: Vec<StraightLineIntegerImmediateTranslationReceipt>,
    straight_line_boolean_immediates: Vec<StraightLineBooleanImmediateTranslationReceipt>,
}

impl ValidatedTranslationFamilies {
    pub(super) fn validate_function(
        &mut self,
        source: &AbstractFunction,
        target: &TargetFunction,
    ) -> Result<(), AbstractToTargetTranslationValidationError> {
        for family in ENABLED_TRANSLATION_FAMILIES {
            match family {
                TranslationFamily::StraightLineIntegerImmediate
                    if straight_line_integer_immediate::is_candidate(source) =>
                {
                    self.straight_line_integer_immediates.push(
                        straight_line_integer_immediate::validate(source, target).map_err(
                            |error| {
                                AbstractToTargetTranslationValidationError::StraightLineIntegerImmediate {
                                    machine: source.machine,
                                    error,
                                }
                            },
                        )?,
                    );
                }
                TranslationFamily::StraightLineBooleanImmediate
                    if straight_line_boolean_immediate::is_candidate(source) =>
                {
                    self.straight_line_boolean_immediates.push(
                        straight_line_boolean_immediate::validate(source, target).map_err(
                            |error| {
                                AbstractToTargetTranslationValidationError::StraightLineBooleanImmediate {
                                    machine: source.machine,
                                    error,
                                }
                            },
                        )?,
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn into_receipts(
        self,
    ) -> (
        Vec<StraightLineIntegerImmediateTranslationReceipt>,
        Vec<StraightLineBooleanImmediateTranslationReceipt>,
    ) {
        (
            self.straight_line_integer_immediates,
            self.straight_line_boolean_immediates,
        )
    }
}
