//! The compact FNV-1a identity every plan reports.

use crate::calling_conventions::callback_materializations::NativePlace;
use crate::calling_conventions::plans::vocabulary::{register_code, system_v_eightbyte_class_code};
use crate::calling_conventions::plans::{
    CallPlan, CallingPolicy, EntryControl, EntryStack, IndirectPointerLocation, MachineRegime,
    MachineRegister, Preemption, RegisterSet, StatePlan, ValueClass, ValueLocation, ValuePlacement,
    ValueShape,
};
use sha2::Digest;
use sha2::Sha256;

pub(crate) struct Fnv1a {
    compact: u64,
    strong: Option<Sha256>,
}

impl Fnv1a {
    pub(crate) const fn new() -> Self {
        Self {
            compact: 0xcbf29ce484222325,
            strong: None,
        }
    }

    pub(crate) fn with_strong_domain(domain: &[u8]) -> Self {
        let mut strong = Sha256::new();
        strong.update((domain.len() as u64).to_le_bytes());
        strong.update(domain);
        Self {
            compact: 0xcbf29ce484222325,
            strong: Some(strong),
        }
    }

    pub(crate) const fn finish(self) -> u64 {
        self.compact
    }

    pub(crate) fn finish_strong(self) -> [u8; 32] {
        self.strong.expect("strong plan hasher").finalize().into()
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.compact ^= u64::from(*byte);
            self.compact = self.compact.wrapping_mul(0x100000001b3);
        }
        if let Some(strong) = &mut self.strong {
            strong.update(bytes);
        }
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn call_plan(&mut self, plan: &CallPlan) {
        self.u8(match plan.policy {
            CallingPolicy::MicrosoftX64 => 0,
            CallingPolicy::SystemVAMD64 => 1,
            CallingPolicy::Aapcs64 => 2,
            CallingPolicy::LinuxSyscallX86_64 => 3,
            CallingPolicy::LinuxSyscallAarch64 => 4,
        });
        self.u16(plan.stack_alignment);
        self.u16(plan.shadow_bytes);
        self.entry_control(plan.entry_control);
        self.u32(plan.parameters.len() as u32);
        for parameter in &plan.parameters {
            self.value_placement(parameter);
        }
        match &plan.result {
            Some(result) => {
                self.u8(1);
                self.value_placement(result);
            }
            None => self.u8(0),
        }
        self.register_set(&plan.ordinary_clobbers);
        // Empty catalogs preserve the identity of ordinary plans created
        // before callback materialization existed. A nonempty catalog is a
        // new observable ABI commitment and receives its own domain tag.
        if !plan.callback_materializations.is_empty() {
            self.bytes(b"omega.callback-materializations.v1");
            self.u32(plan.callback_materializations.len() as u32);
            for row in &plan.callback_materializations {
                self.u64(row.binder.get());
                self.native_place(&row.destination);
            }
        }
    }

    fn native_place(&mut self, place: &NativePlace) {
        match place {
            NativePlace::Parameter(parameter) => {
                self.u8(0);
                self.u64(parameter.get());
            }
            NativePlace::Field {
                parameter,
                layout,
                field_path,
            } => {
                self.u8(1);
                self.u64(parameter.get());
                self.u64(layout.get());
                self.u32(field_path.len() as u32);
                for slot in field_path {
                    self.u64(slot.get());
                }
            }
        }
    }

    pub(crate) fn state_plan(&mut self, plan: &StatePlan) {
        match plan.initial_regime {
            MachineRegime::X86Long64 => self.u8(0),
            MachineRegime::Aarch64A64 { exception_level } => {
                self.u8(1);
                self.u8(exception_level);
            }
        }
        self.u16(plan.interrupted_state.bits());
        self.u16(plan.saved_state.bits());
        self.u16(plan.restored_state.bits());
        self.u16(plan.permitted_transitive_use.bits());
        match plan.stack {
            EntryStack::Interrupted => self.u8(0),
            EntryStack::Dedicated { class } => {
                self.u8(1);
                self.u16(class);
            }
            EntryStack::ProviderSelected => self.u8(2),
        }
        match plan.preemption {
            Preemption::NotApplicable => self.u8(0),
            Preemption::Masked => self.u8(1),
            Preemption::Nestable { maximum_depth } => {
                self.u8(2);
                self.u16(maximum_depth);
            }
            Preemption::ProviderDefined => self.u8(3),
        }
    }

    fn entry_control(&mut self, control: EntryControl) {
        match control {
            EntryControl::CallReturn => self.u8(0),
            EntryControl::SupervisorCall {
                number_register,
                immediate,
            } => {
                self.u8(1);
                self.register(number_register);
                self.u16(immediate);
            }
            EntryControl::InterruptReturn => self.u8(2),
        }
    }

    fn value_placement(&mut self, placement: &ValuePlacement) {
        self.value_shape(placement.shape);
        self.u32(placement.locations.len() as u32);
        for location in &placement.locations {
            match *location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => {
                    self.u8(0);
                    self.register(register);
                    self.u16(value_byte_offset);
                    self.u16(byte_size);
                }
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    alignment,
                } => {
                    self.u8(1);
                    self.u32(stack_byte_offset);
                    self.u16(value_byte_offset);
                    self.u16(byte_size);
                    self.u16(alignment);
                }
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    alignment,
                } => {
                    self.u8(2);
                    match pointer {
                        IndirectPointerLocation::Register(register) => {
                            self.u8(0);
                            self.register(register);
                        }
                        IndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        } => {
                            self.u8(1);
                            self.u32(stack_byte_offset);
                            self.u16(alignment);
                        }
                    }
                    match copy_stack_byte_offset {
                        Some(offset) => {
                            self.u8(1);
                            self.u32(offset);
                        }
                        None => self.u8(0),
                    }
                    self.u16(byte_size);
                    self.u16(alignment);
                }
            }
        }
    }

    fn value_shape(&mut self, shape: ValueShape) {
        match shape.class {
            ValueClass::Integer => self.u8(0),
            ValueClass::Float => self.u8(1),
            ValueClass::BorrowedReference => self.u8(4),
            ValueClass::HomogeneousFloatAggregate { members } => {
                self.u8(2);
                self.u8(members);
            }
            ValueClass::SystemVAggregate { first, second } => {
                self.u8(3);
                self.u8(system_v_eightbyte_class_code(first));
                self.u8(system_v_eightbyte_class_code(second));
            }
        }
        self.u16(shape.byte_size);
        self.u16(shape.alignment);
    }

    pub(crate) fn register_set(&mut self, registers: &RegisterSet) {
        self.u32(registers.as_slice().len() as u32);
        for register in registers.as_slice() {
            self.register(*register);
        }
    }

    fn register(&mut self, register: MachineRegister) {
        let code = register_code(register);
        self.u8((code >> 8) as u8);
        self.u8(code as u8);
    }
}
