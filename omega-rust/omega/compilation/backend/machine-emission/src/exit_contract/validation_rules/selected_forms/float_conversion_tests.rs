//! A private conversion guard may crash without becoming a function exit edge.
use super::validate_non_return;
use machine_code::{
    ResolvedSelectedFormRow, SelectedFormDecodedFootprint, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormMachineDisposition,
};
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
                let effects = &encoded.footprint().encoded;
                let mut encoding = SelectedFormEncodingRow {
                    instruction,
                    alternative,
                    machine_disposition: SelectedFormMachineDisposition::RetainedV1,
                    address: None,
                    state: SelectedFormEncodingState::Encoded {
                        bytes: encoded.bytes().to_vec(),
                        footprint: Box::new(SelectedFormDecodedFootprint {
                            register_reads: encoded.footprint().register_reads.clone(),
                            register_writes: encoded.footprint().register_writes.clone(),
                            implicit_defs: effects.implicit_unit_defs.clone(),
                            implicit_clobbers: effects.implicit_unit_clobbers.clone(),
                            encoded: effects.clone(),
                        }),
                    },
                };
                let layout = ResolvedSelectedFormRow {
                    instruction,
                    alternative,
                    offset: 0,
                    bytes: encoded.bytes().to_vec(),
                    branch: None,
                    internal_machine_fixup: None,
                    normalized_foreign_call_fixup: None,
                    byte_literal_address_fixup: None,
                };
                validate_non_return(instruction, kind, &encoding, &layout).unwrap();
                let SelectedFormEncodingState::Encoded { footprint, .. } = &mut encoding.state
                else {
                    unreachable!()
                };
                footprint.encoded.control = if domain == IeeeFloatToIntegerDomain::Trapping {
                    MachineEncodedControlEffect::FallThroughV1
                } else {
                    MachineEncodedControlEffect::FallThroughOrTrapV1
                };
                assert!(
                    validate_non_return(instruction, kind, &encoding, &layout).is_err(),
                    "{form:?}"
                );
            }
        }
    }
}
