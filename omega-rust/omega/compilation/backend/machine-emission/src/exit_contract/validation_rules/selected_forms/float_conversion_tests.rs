//! A private conversion guard may crash without becoming a function exit edge.
use super::validate_non_return;
use selected_instructions::{
    IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatFormat,
    IeeeFloatIntegerCarrier, IeeeFloatToIntegerDomain, MachineAlternativeFamily,
    MachineAlternativeKey, MachineEncodedControlEffect, SelectedInstructionId,
    SelectedInstructionKind,
};

#[test]
fn float_conversion_exit_admission_retains_only_its_selected_policy_control() {
    let physical = register_model::validate_physical_register_model(
        isa_aarch64::aarch64_physical_register_model(),
    )
    .unwrap();
    let registers = ["x3", "x4"].map(|name| physical.model().view_named(name).unwrap().id);
    for domain in [
        IeeeFloatToIntegerDomain::Exact,
        IeeeFloatToIntegerDomain::Saturating,
        IeeeFloatToIntegerDomain::Trapping,
    ] {
        for target in IeeeFloatIntegerCarrier::ALL {
            for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                let form = IeeeFloatArithmeticForm {
                    operation: IeeeFloatArithmeticOperation::TruncateToInteger { target, domain },
                    format,
                };
                let kind = SelectedInstructionKind::NearestIeeeFloatArithmetic { form };
                let alternative = MachineAlternativeKey {
                    family: MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
                    variant: 0,
                };
                let encoded = isa_aarch64::encode_aarch64_selected_form(
                    &physical,
                    kind,
                    alternative,
                    &registers,
                )
                .unwrap();
                let instruction = SelectedInstructionId(1);
                let (store, encoding, layout) =
                    super::validated_aarch64_form_fixture(&encoded, instruction, alternative, kind);
                let mut changed = store.clone();
                let footprint = changed.footprint_mut(encoding.footprint).unwrap();
                footprint.encoded.control = if domain == IeeeFloatToIntegerDomain::Trapping {
                    MachineEncodedControlEffect::FallThroughV1
                } else {
                    MachineEncodedControlEffect::FallThroughOrTrapV1
                };
                assert!(
                    validate_non_return(
                        target::Architecture::Aarch64,
                        instruction,
                        kind,
                        &encoding,
                        &layout,
                        &changed
                    )
                    .is_err(),
                    "{form:?}"
                );
            }
        }
    }
}
