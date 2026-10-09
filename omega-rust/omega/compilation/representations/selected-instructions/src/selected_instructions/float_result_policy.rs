//! Post-call policy checks on already-completed IEEE payloads. These forms do
//! not repeat arithmetic or select a provider. Trapping reads the raw result;
//! saturation reads the reduced overflow eligibility, then that raw result.
//! The output follows all inputs and may alias an input only after it is read.
//! Source operand classification belongs to scalar selection, not ISA state.

use semantic_vocabulary::IeeeFloatFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatResultPolicyKind {
    Trapping,
    Saturating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatResultPolicyForm {
    pub policy: FloatResultPolicyKind,
    pub format: IeeeFloatFormat,
}

impl FloatResultPolicyForm {
    pub const COUNT: usize = 4;
    pub const ALL: [Self; Self::COUNT] = {
        let policies = [
            FloatResultPolicyKind::Trapping,
            FloatResultPolicyKind::Saturating,
        ];
        let formats = [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64];
        let mut forms = [Self {
            policy: FloatResultPolicyKind::Trapping,
            format: IeeeFloatFormat::Binary32,
        }; Self::COUNT];
        let mut policy_index = 0;
        while policy_index < policies.len() {
            let mut format_index = 0;
            while format_index < formats.len() {
                forms[policy_index * formats.len() + format_index] = Self {
                    policy: policies[policy_index],
                    format: formats[format_index],
                };
                format_index += 1;
            }
            policy_index += 1;
        }
        forms
    };

    /// Append-only family tag; one closed payload byte follows in every codec.
    pub const FAMILY_TAG: u8 = 231;

    pub const fn machine_tag(self) -> u8 {
        Self::FAMILY_TAG
    }

    /// Policy-major order, binary32 before binary64. Existing ordinals never move.
    pub const fn ordinal(self) -> u8 {
        let policy = match self.policy {
            FloatResultPolicyKind::Trapping => 0,
            FloatResultPolicyKind::Saturating => 1,
        };
        policy * 2
            + match self.format {
                IeeeFloatFormat::Binary32 => 0,
                IeeeFloatFormat::Binary64 => 1,
            }
    }

    pub const fn machine_payload(self) -> u8 {
        self.ordinal()
    }

    pub const fn from_machine_payload(payload: u8) -> Option<Self> {
        if (payload as usize) < Self::COUNT {
            Some(Self::ALL[payload as usize])
        } else {
            None
        }
    }

    pub const fn is_binary32(self) -> bool {
        matches!(self.format, IeeeFloatFormat::Binary32)
    }

    pub const fn can_trap(self) -> bool {
        matches!(self.policy, FloatResultPolicyKind::Trapping)
    }

    /// Inputs are `[raw]` or `[eligible, raw]`; output follows all inputs.
    pub const fn operand_count(self) -> usize {
        match self.policy {
            FloatResultPolicyKind::Trapping => 1,
            FloatResultPolicyKind::Saturating => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_result_policy_forms_have_closed_stable_payloads() {
        assert!(FloatResultPolicyForm::ALL.is_sorted());
        for (ordinal, form) in FloatResultPolicyForm::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(form.machine_payload()), ordinal);
            assert_eq!(form.machine_tag(), 231);
            assert_eq!(
                FloatResultPolicyForm::from_machine_payload(ordinal as u8),
                Some(form)
            );
            assert_eq!(form.is_binary32(), ordinal % 2 == 0);
            assert_eq!(form.can_trap(), ordinal < 2);
            assert_eq!(form.operand_count(), if ordinal < 2 { 1 } else { 2 });
        }
        for payload in 4..=u8::MAX {
            assert_eq!(FloatResultPolicyForm::from_machine_payload(payload), None);
        }
    }
}
