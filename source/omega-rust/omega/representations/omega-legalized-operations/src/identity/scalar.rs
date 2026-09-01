use super::shared::*;

pub(super) fn encode_leaf(bytes: &mut Vec<u8>, leaf: &LegalizedLeaf) {
    bytes.extend_from_slice(&leaf.return_edge.get().to_le_bytes());
    bytes.extend_from_slice(&leaf.source_value.get().to_le_bytes());
    encode_fuel(bytes, &leaf.return_fuel);
    match &leaf.value {
        LegalizedLeafValue::Immediate {
            value,
            constant_operation,
            definition_site,
            constant_fuel,
        } => {
            bytes.push(0);
            encode_integer(bytes, *value);
            bytes.extend_from_slice(&constant_operation.get().to_le_bytes());
            encode_definition_site(bytes, *definition_site);
            encode_fuel(bytes, constant_fuel);
        }
        LegalizedLeafValue::EntryParameter {
            parameter_index,
            register,
            definition_site,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&(*parameter_index as u64).to_le_bytes());
            encode_register(bytes, *register);
            encode_definition_site(bytes, *definition_site);
        }
        LegalizedLeafValue::ExactAdd {
            obligation,
            accepted_fact,
            add_operation,
            definition_site,
            add_fuel,
            left,
            right,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
            bytes.extend_from_slice(&add_operation.get().to_le_bytes());
            encode_definition_site(bytes, *definition_site);
            encode_fuel(bytes, add_fuel);
            encode_immediate(bytes, left);
            encode_immediate(bytes, right);
        }
        LegalizedLeafValue::ExactSubtract {
            obligation,
            accepted_fact,
            subtract_operation,
            definition_site,
            subtract_fuel,
            left,
            right,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
            bytes.extend_from_slice(&subtract_operation.get().to_le_bytes());
            encode_definition_site(bytes, *definition_site);
            encode_fuel(bytes, subtract_fuel);
            encode_immediate(bytes, left);
            encode_immediate(bytes, right);
        }
        LegalizedLeafValue::WidenedExactAdd {
            source_type,
            target_type,
            theorem,
            obligation,
            accepted_fact,
            add_operation,
            narrow_result,
            add_definition_site,
            add_fuel,
            widen_operation,
            widen_definition_site,
            widen_fuel,
            left_temporary,
            right_temporary,
            left,
            right,
        } => {
            bytes.push(4);
            encode_integer_type(bytes, *source_type);
            encode_integer_type(bytes, *target_type);
            bytes.push(match theorem {
                LegalizationTheorem::UnsignedExactAddCommutesWithWidenV1 => 0,
                LegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1 => 1,
            });
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
            bytes.extend_from_slice(&add_operation.get().to_le_bytes());
            bytes.extend_from_slice(&narrow_result.get().to_le_bytes());
            encode_definition_site(bytes, *add_definition_site);
            encode_fuel(bytes, add_fuel);
            bytes.extend_from_slice(&widen_operation.get().to_le_bytes());
            encode_definition_site(bytes, *widen_definition_site);
            encode_fuel(bytes, widen_fuel);
            bytes.extend_from_slice(&left_temporary.0.to_le_bytes());
            bytes.extend_from_slice(&right_temporary.0.to_le_bytes());
            encode_immediate(bytes, left);
            encode_immediate(bytes, right);
        }
        LegalizedLeafValue::WidenedExactSubtract {
            source_type,
            target_type,
            theorem,
            obligation,
            accepted_fact,
            subtract_operation,
            narrow_result,
            subtract_definition_site,
            subtract_fuel,
            widen_operation,
            widen_definition_site,
            widen_fuel,
            left_temporary,
            right_temporary,
            left,
            right,
        } => {
            bytes.push(5);
            encode_integer_type(bytes, *source_type);
            encode_integer_type(bytes, *target_type);
            bytes.push(match theorem {
                LegalizationTheorem::UnsignedExactAddCommutesWithWidenV1 => 0,
                LegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1 => 1,
            });
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
            bytes.extend_from_slice(&subtract_operation.get().to_le_bytes());
            bytes.extend_from_slice(&narrow_result.get().to_le_bytes());
            encode_definition_site(bytes, *subtract_definition_site);
            encode_fuel(bytes, subtract_fuel);
            bytes.extend_from_slice(&widen_operation.get().to_le_bytes());
            encode_definition_site(bytes, *widen_definition_site);
            encode_fuel(bytes, widen_fuel);
            bytes.extend_from_slice(&left_temporary.0.to_le_bytes());
            bytes.extend_from_slice(&right_temporary.0.to_le_bytes());
            encode_immediate(bytes, left);
            encode_immediate(bytes, right);
        }
        LegalizedLeafValue::ActiveResidentExactAddChain(chain) => {
            bytes.push(6);
            encode_immediate(bytes, &chain.resident);
            encode_immediate(bytes, &chain.left);
            encode_immediate(bytes, &chain.right);
            encode_exact_add(bytes, &chain.inner);
            encode_exact_add(bytes, &chain.middle);
            encode_exact_add(bytes, &chain.result);
        }
    }
}

pub(super) fn encode_exact_add(bytes: &mut Vec<u8>, add: &LegalizedExactAdd) {
    bytes.extend_from_slice(&add.source_value.get().to_le_bytes());
    bytes.extend_from_slice(&add.obligation.get().to_le_bytes());
    bytes.extend_from_slice(&add.accepted_fact.bytes());
    bytes.extend_from_slice(&add.operation.get().to_le_bytes());
    encode_definition_site(bytes, add.definition_site);
    encode_fuel(bytes, &add.fuel);
}

pub(super) fn encode_immediate(bytes: &mut Vec<u8>, immediate: &LegalizedImmediate) {
    bytes.extend_from_slice(&immediate.source_value.get().to_le_bytes());
    encode_integer(bytes, immediate.value);
    bytes.extend_from_slice(&immediate.constant_operation.get().to_le_bytes());
    encode_definition_site(bytes, immediate.definition_site);
    encode_fuel(bytes, &immediate.fuel);
}

pub(super) fn encode_integer(bytes: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            bytes.push(0);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}

pub(super) fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

pub(super) fn encode_register(bytes: &mut Vec<u8>, register: MachineRegister) {
    let (tag, payload) = match register {
        MachineRegister::X86Rax => (0, 0),
        MachineRegister::X86Rcx => (1, 0),
        MachineRegister::X86Rdx => (2, 0),
        MachineRegister::X86Rbx => (3, 0),
        MachineRegister::X86Rsp => (4, 0),
        MachineRegister::X86Rbp => (5, 0),
        MachineRegister::X86Rsi => (6, 0),
        MachineRegister::X86Rdi => (7, 0),
        MachineRegister::X86R8 => (8, 0),
        MachineRegister::X86R9 => (9, 0),
        MachineRegister::X86R10 => (10, 0),
        MachineRegister::X86R11 => (11, 0),
        MachineRegister::X86R12 => (12, 0),
        MachineRegister::X86R13 => (13, 0),
        MachineRegister::X86R14 => (14, 0),
        MachineRegister::X86R15 => (15, 0),
        MachineRegister::X86Xmm(index) => (16, index),
        MachineRegister::Aarch64X(index) => (17, index),
        MachineRegister::Aarch64V(index) => (18, index),
    };
    bytes.push(tag);
    bytes.push(payload);
}

pub(super) fn encode_bindings(bytes: &mut Vec<u8>, bindings: &[ValueBinding]) {
    encode_len(bytes, bindings.len());
    for binding in bindings {
        bytes.extend_from_slice(&binding.parameter.get().to_le_bytes());
        bytes.extend_from_slice(&binding.argument.get().to_le_bytes());
        encode_scalar_type(bytes, binding.scalar_type);
    }
}

pub(super) fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: psi_core::ScalarType) {
    match scalar_type {
        psi_core::ScalarType::Boolean => bytes.push(0),
        psi_core::ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                psi_core::IntegerCarrier::Fixed => 0,
                psi_core::IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                psi_core::IntegerSign::Signed => 0,
                psi_core::IntegerSign::Unsigned => 1,
            });
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
        psi_core::ScalarType::IeeeFloat(format) => {
            bytes.push(2);
            bytes.push(match format {
                psi_core::IeeeFloatFormat::Binary32 => 0,
                psi_core::IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}

pub(super) fn encode_integer_type(bytes: &mut Vec<u8>, integer: IntegerType) {
    bytes.push(match integer.carrier() {
        psi_core::IntegerCarrier::Fixed => 0,
        psi_core::IntegerCarrier::Address => 1,
    });
    bytes.push(match integer.sign() {
        psi_core::IntegerSign::Signed => 0,
        psi_core::IntegerSign::Unsigned => 1,
    });
    bytes.extend_from_slice(&integer.bits().to_le_bytes());
}
