use omega_calling_conventions::MachineRegister;
use omega_target::{Architecture, NativeTarget, ObjectFormat, TargetProfile};

use super::evidence::{NativeFuelRuntimeTextEvidence, NativeFuelTransferRuntimeEvidence};
use super::plan::{
    NativeFuelRuntimeEntryIdentity, NativeFuelSavedValue, NativeFuelTransferRuntimePlanProjection,
};
use super::{NativeFuelContextLayout, SponsorContextTransport};

pub(super) fn fingerprint_transfer_plan(plan: &NativeFuelTransferRuntimePlanProjection) -> u64 {
    let mut hash = Fnv1a::new(b"omega.native-fuel-transfer-plan.v1");
    hash.u8(profile_tag(plan.profile));
    hash.target(plan.target);
    hash.transport(plan.transport);
    hash.context(plan.context);
    hash.u64(plan.activation_state_slots.len() as u64);
    for slot in &plan.activation_state_slots {
        hash.saved_value(slot.value);
        hash.u32(slot.context_offset);
        hash.u32(slot.byte_count);
    }
    hash.u32(plan.sponsor_stack.alignment);
    hash.u64(plan.sponsor_stack.byte_ceiling);
    hash.u16(plan.interrupted_state.bits());
    hash.u16(plan.saved_state.bits());
    hash.u16(plan.restored_state.bits());
    hash.entry(plan.transfer_entry);
    hash.entry(plan.resume_entry);
    hash.finish_nonzero()
}

pub(super) fn non_authoritative_transfer_evidence_report_fingerprint(
    evidence: &NativeFuelTransferRuntimeEvidence,
) -> u64 {
    let mut hash = Fnv1a::new(b"omega.native-fuel-transfer-evidence.v1");
    hash.u64(evidence.plan.normalized_identity());
    hash.text_evidence(&evidence.transfer_text);
    hash.text_evidence(&evidence.resume_text);
    hash.u64(evidence.physical_state_footprint.evidence_fingerprint());
    hash.u64(evidence.sponsor_stack_peak_bytes);
    hash.finish_nonzero()
}

fn profile_tag(profile: TargetProfile) -> u8 {
    match profile {
        TargetProfile::LinuxArm64 => 0,
        TargetProfile::LinuxX64 => 1,
        TargetProfile::MacosArm64 => 2,
        TargetProfile::WindowsX64 => 3,
        TargetProfile::UefiX64 => 4,
        TargetProfile::CrossPlatformCli => 5,
        TargetProfile::LocalUnchecked => 6,
    }
}

struct Fnv1a(u64);

impl Fnv1a {
    fn new(domain: &[u8]) -> Self {
        let mut hash = Self(0xcbf2_9ce4_8422_2325);
        hash.bytes(domain);
        hash
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn target(&mut self, target: NativeTarget) {
        self.u8(match target.architecture {
            Architecture::Aarch64 => 0,
            Architecture::X86_64 => 1,
        });
        self.u8(match target.object_format {
            ObjectFormat::Elf => 0,
            ObjectFormat::MachO => 1,
            ObjectFormat::Coff => 2,
        });
        self.u64(target.pointer_size as u64);
        self.u64(target.pointer_alignment as u64);
    }

    fn transport(&mut self, transport: SponsorContextTransport) {
        match transport {
            SponsorContextTransport::ReservedNonvolatileRegister { register } => {
                self.u8(0);
                self.register(register);
            }
        }
    }

    fn context(&mut self, context: NativeFuelContextLayout) {
        self.u32(context.byte_size);
        self.u32(context.alignment);
        self.u32(context.remaining_units_offset);
        self.u32(context.unpaid_site_kind_offset);
        self.u32(context.unpaid_site_identity_offset);
        self.u32(context.required_units_offset);
        self.u32(context.transfer_entry_offset);
        self.u32(context.retry_code_offset_offset);
        self.u32(context.sponsor_stack_top_offset);
        self.u32(context.activation_state_offset);
        self.u32(context.activation_state_byte_count);
    }

    fn saved_value(&mut self, value: NativeFuelSavedValue) {
        match value {
            NativeFuelSavedValue::Register(register) => {
                self.u8(0);
                self.register(register);
            }
            NativeFuelSavedValue::Flags => self.u8(1),
            NativeFuelSavedValue::StackPointer => self.u8(2),
        }
    }

    fn register(&mut self, register: MachineRegister) {
        let (class, index) = match register {
            MachineRegister::X86Rax => (0, 0),
            MachineRegister::X86Rcx => (0, 1),
            MachineRegister::X86Rdx => (0, 2),
            MachineRegister::X86Rbx => (0, 3),
            MachineRegister::X86Rsp => (0, 4),
            MachineRegister::X86Rbp => (0, 5),
            MachineRegister::X86Rsi => (0, 6),
            MachineRegister::X86Rdi => (0, 7),
            MachineRegister::X86R8 => (0, 8),
            MachineRegister::X86R9 => (0, 9),
            MachineRegister::X86R10 => (0, 10),
            MachineRegister::X86R11 => (0, 11),
            MachineRegister::X86R12 => (0, 12),
            MachineRegister::X86R13 => (0, 13),
            MachineRegister::X86R14 => (0, 14),
            MachineRegister::X86R15 => (0, 15),
            MachineRegister::X86Xmm(index) => (1, u16::from(index)),
            MachineRegister::Aarch64X(index) => (2, u16::from(index)),
            MachineRegister::Aarch64V(index) => (3, u16::from(index)),
        };
        self.u8(class);
        self.u16(index);
    }

    fn entry(&mut self, entry: NativeFuelRuntimeEntryIdentity) {
        self.u64(entry.section_identity);
        self.u64(entry.symbol_identity);
    }

    fn text_evidence(&mut self, evidence: &NativeFuelRuntimeTextEvidence) {
        self.entry(evidence.entry);
        self.u64(evidence.span.text_offset as u64);
        self.u64(evidence.span.byte_count as u64);
        self.u64(evidence.unrelocated_bytes.len() as u64);
        self.bytes(&evidence.unrelocated_bytes);
        self.u64(evidence.final_bytes.len() as u64);
        self.bytes(&evidence.final_bytes);
    }

    fn finish_nonzero(self) -> u64 {
        if self.0 == 0 {
            0xcbf2_9ce4_8422_2325
        } else {
            self.0
        }
    }
}
