#![forbid(unsafe_code)]

//! Canonical target-legal operation custody for the clean Terminal lane.
//!
//! This representation is deliberately below instruction selection and above
//! physical-home assignment. It records which bounded target operation shapes
//! have passed target legalization, with complete semantic provenance, proof,
//! and fuel custody. It assigns no virtual or physical register, stack slot,
//! instruction encoding, or frame location.

use omega_optimization_core::{AcceptedObligationFactIdentity, OptimizationUnitIdentity};
use omega_optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::TerminalValueBinding;
use omega_terminal_target_operations::{MachineRegister, TerminalPsiProvenance};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, ValueId,
};
use psi_terminal::TerminalPsiIdentity;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalLegalizedOperationPlanIdentity([u8; 32]);

impl TerminalLegalizedOperationPlanIdentity {
    pub fn from_canonical_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Dense function-local identity for a value introduced by target
/// legalization rather than Terminal Psi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalLegalizedTemporaryId(pub u32);

/// Closed semantic theorem that authorizes a non-identity legalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLegalizationTheorem {
    /// For unsigned exact addition with a discharged narrow overflow
    /// obligation, zero-extension commutes with addition.
    UnsignedExactAddCommutesWithWidenV1,
    /// For unsigned exact subtraction with a discharged narrow underflow
    /// obligation, zero-extension commutes with subtraction while preserving
    /// the authored operand order.
    UnsignedExactSubtractCommutesWithWidenV1,
}

/// The closed V3 legality recipe admitted for one function.
///
/// The original recipes are identity legalizations. The widened-u8 recipes
/// are closed non-identity transformations with explicit theorem, temporary,
/// source-operation, proof, and fuel custody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLegalizationRecipe {
    ReturnU64ImmediateConditionalV1,
    ReturnU64EntryParameterConditionalV1,
    ReturnU64ExactAddImmediateConditionalV1,
    ReturnU64ExactSubtractImmediateConditionalV1,
    ReturnU64WidenedU8ExactAddImmediateConditionalV1,
    ReturnU64WidenedU8ExactSubtractImmediateConditionalV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedOperationPlan {
    pub terminal_psi: TerminalPsiIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<TerminalLegalizedFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub recipe: TerminalLegalizationRecipe,
    pub condition_source: ValueId,
    pub condition_parameter_index: usize,
    pub condition_register: MachineRegister,
    pub condition_definition_site: ValueDefinitionSite,
    pub entry_block: BlockId,
    pub true_block: BlockId,
    pub false_block: BlockId,
    pub branch_true_edge: EdgeId,
    pub branch_false_edge: EdgeId,
    pub branch_true_fuel: Vec<FuelSettlement>,
    pub branch_false_fuel: Vec<FuelSettlement>,
    pub branch_true_bindings: Vec<TerminalValueBinding>,
    pub branch_false_bindings: Vec<TerminalValueBinding>,
    pub when_true: TerminalLegalizedLeaf,
    pub when_false: TerminalLegalizedLeaf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedLeaf {
    pub return_edge: EdgeId,
    pub source_value: ValueId,
    pub return_fuel: Vec<FuelSettlement>,
    pub value: TerminalLegalizedLeafValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLegalizedLeafValue {
    Immediate {
        value: IntegerValue,
        constant_operation: OperationId,
        definition_site: ValueDefinitionSite,
        constant_fuel: Vec<FuelSettlement>,
    },
    EntryParameter {
        parameter_index: usize,
        register: MachineRegister,
        definition_site: ValueDefinitionSite,
    },
    ExactAdd {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        add_operation: OperationId,
        definition_site: ValueDefinitionSite,
        add_fuel: Vec<FuelSettlement>,
        left: TerminalLegalizedImmediate,
        right: TerminalLegalizedImmediate,
    },
    ExactSubtract {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        subtract_operation: OperationId,
        definition_site: ValueDefinitionSite,
        subtract_fuel: Vec<FuelSettlement>,
        left: TerminalLegalizedImmediate,
        right: TerminalLegalizedImmediate,
    },
    WidenedExactAdd {
        source_type: IntegerType,
        target_type: IntegerType,
        theorem: TerminalLegalizationTheorem,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        add_operation: OperationId,
        narrow_result: ValueId,
        add_definition_site: ValueDefinitionSite,
        add_fuel: Vec<FuelSettlement>,
        widen_operation: OperationId,
        widen_definition_site: ValueDefinitionSite,
        widen_fuel: Vec<FuelSettlement>,
        left_temporary: TerminalLegalizedTemporaryId,
        right_temporary: TerminalLegalizedTemporaryId,
        left: TerminalLegalizedImmediate,
        right: TerminalLegalizedImmediate,
    },
    WidenedExactSubtract {
        source_type: IntegerType,
        target_type: IntegerType,
        theorem: TerminalLegalizationTheorem,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        subtract_operation: OperationId,
        narrow_result: ValueId,
        subtract_definition_site: ValueDefinitionSite,
        subtract_fuel: Vec<FuelSettlement>,
        widen_operation: OperationId,
        widen_definition_site: ValueDefinitionSite,
        widen_fuel: Vec<FuelSettlement>,
        left_temporary: TerminalLegalizedTemporaryId,
        right_temporary: TerminalLegalizedTemporaryId,
        left: TerminalLegalizedImmediate,
        right: TerminalLegalizedImmediate,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedImmediate {
    pub source_value: ValueId,
    pub value: IntegerValue,
    pub constant_operation: OperationId,
    pub definition_site: ValueDefinitionSite,
    pub fuel: Vec<FuelSettlement>,
}

pub fn terminal_legalized_operation_plan_identity(
    plan: &TerminalLegalizedOperationPlan,
) -> TerminalLegalizedOperationPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-legalized-operations.v3\0");
    bytes.extend_from_slice(plan.terminal_psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&plan.terminal_psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.entry.get().to_le_bytes());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_option_id(
            &mut bytes,
            function.attachment.map(|attachment| attachment.get()),
        );
        encode_ids(
            &mut bytes,
            function
                .provenance
                .operations
                .iter()
                .map(|operation| operation.get()),
        );
        encode_ids(
            &mut bytes,
            function.provenance.edges.iter().map(|edge| edge.get()),
        );
        bytes.push(match function.recipe {
            TerminalLegalizationRecipe::ReturnU64ImmediateConditionalV1 => 0,
            TerminalLegalizationRecipe::ReturnU64EntryParameterConditionalV1 => 1,
            TerminalLegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1 => 2,
            TerminalLegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1 => 3,
            TerminalLegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1 => 4,
            TerminalLegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1 => 5,
        });
        bytes.extend_from_slice(&function.condition_source.get().to_le_bytes());
        bytes.extend_from_slice(&(function.condition_parameter_index as u64).to_le_bytes());
        encode_register(&mut bytes, function.condition_register);
        encode_definition_site(&mut bytes, function.condition_definition_site);
        bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
        bytes.extend_from_slice(&function.true_block.get().to_le_bytes());
        bytes.extend_from_slice(&function.false_block.get().to_le_bytes());
        bytes.extend_from_slice(&function.branch_true_edge.get().to_le_bytes());
        bytes.extend_from_slice(&function.branch_false_edge.get().to_le_bytes());
        encode_fuel(&mut bytes, &function.branch_true_fuel);
        encode_fuel(&mut bytes, &function.branch_false_fuel);
        encode_bindings(&mut bytes, &function.branch_true_bindings);
        encode_bindings(&mut bytes, &function.branch_false_bindings);
        encode_leaf(&mut bytes, &function.when_true);
        encode_leaf(&mut bytes, &function.when_false);
    }
    TerminalLegalizedOperationPlanIdentity::from_canonical_bytes(&bytes)
}

fn encode_leaf(bytes: &mut Vec<u8>, leaf: &TerminalLegalizedLeaf) {
    bytes.extend_from_slice(&leaf.return_edge.get().to_le_bytes());
    bytes.extend_from_slice(&leaf.source_value.get().to_le_bytes());
    encode_fuel(bytes, &leaf.return_fuel);
    match &leaf.value {
        TerminalLegalizedLeafValue::Immediate {
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
        TerminalLegalizedLeafValue::EntryParameter {
            parameter_index,
            register,
            definition_site,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&(*parameter_index as u64).to_le_bytes());
            encode_register(bytes, *register);
            encode_definition_site(bytes, *definition_site);
        }
        TerminalLegalizedLeafValue::ExactAdd {
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
        TerminalLegalizedLeafValue::ExactSubtract {
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
        TerminalLegalizedLeafValue::WidenedExactAdd {
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
                TerminalLegalizationTheorem::UnsignedExactAddCommutesWithWidenV1 => 0,
                TerminalLegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1 => 1,
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
        TerminalLegalizedLeafValue::WidenedExactSubtract {
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
                TerminalLegalizationTheorem::UnsignedExactAddCommutesWithWidenV1 => 0,
                TerminalLegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1 => 1,
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
    }
}

fn encode_immediate(bytes: &mut Vec<u8>, immediate: &TerminalLegalizedImmediate) {
    bytes.extend_from_slice(&immediate.source_value.get().to_le_bytes());
    encode_integer(bytes, immediate.value);
    bytes.extend_from_slice(&immediate.constant_operation.get().to_le_bytes());
    encode_definition_site(bytes, immediate.definition_site);
    encode_fuel(bytes, &immediate.fuel);
}

fn encode_integer(bytes: &mut Vec<u8>, value: IntegerValue) {
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

fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
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

fn encode_register(bytes: &mut Vec<u8>, register: MachineRegister) {
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

fn encode_bindings(bytes: &mut Vec<u8>, bindings: &[TerminalValueBinding]) {
    encode_len(bytes, bindings.len());
    for binding in bindings {
        bytes.extend_from_slice(&binding.parameter.get().to_le_bytes());
        bytes.extend_from_slice(&binding.argument.get().to_le_bytes());
        encode_scalar_type(bytes, binding.scalar_type);
    }
}

fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: psi_core::ScalarType) {
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
    }
}

fn encode_integer_type(bytes: &mut Vec<u8>, integer: IntegerType) {
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

fn encode_fuel(bytes: &mut Vec<u8>, fuel: &[FuelSettlement]) {
    encode_len(bytes, fuel.len());
    for settlement in fuel {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                bytes.extend_from_slice(&edge.get().to_le_bytes());
            }
        }
        bytes.extend_from_slice(&settlement.units.to_le_bytes());
    }
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        omega_target::Architecture::X86_64 => 0,
        omega_target::Architecture::Aarch64 => 1,
    });
    bytes.push(match target.object_format {
        omega_target::ObjectFormat::Elf => 0,
        omega_target::ObjectFormat::MachO => 1,
        omega_target::ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}

fn encode_option_id(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_ids(bytes: &mut Vec<u8>, values: impl IntoIterator<Item = u64>) {
    let values = values.into_iter().collect::<Vec<_>>();
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(&(len as u64).to_le_bytes());
}
