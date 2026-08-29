use omega_calling_conventions::MachineRegister;
use omega_target::{Architecture, NativeTarget, ObjectFormat, TargetProfile};
use sha2::{Digest, Sha256};

use super::evidence::{NativeFuelRuntimeTextEvidence, NativeFuelTransferRuntimeEvidence};
use super::plan::{
    NativeFuelRuntimeEntryIdentity, NativeFuelSavedValue, NativeFuelTransferRuntimePlanProjection,
};
use super::{NativeFuelContextLayout, NativeFuelTransferPlanCommitment, SponsorContextTransport};

pub(super) fn non_authoritative_transfer_plan_report_fingerprint(
    plan: &NativeFuelTransferRuntimePlanProjection,
) -> u64 {
    let mut hash = Fnv1a::new(b"omega.native-fuel-transfer-plan.v1");
    hash.bytes(&canonical_transfer_plan_bytes(plan));
    hash.finish_nonzero()
}

pub(super) fn transfer_plan_commitment(
    plan: &NativeFuelTransferRuntimePlanProjection,
) -> NativeFuelTransferPlanCommitment {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-fuel-transfer-plan.commitment.v1");
    digest.update(canonical_transfer_plan_bytes(plan));
    NativeFuelTransferPlanCommitment::from_bytes(digest.finalize().into())
}

fn canonical_transfer_plan_bytes(plan: &NativeFuelTransferRuntimePlanProjection) -> Vec<u8> {
    let mut bytes = CanonicalPlanBytes::default();
    bytes.u8(profile_tag(plan.profile));
    bytes.target(plan.target);
    bytes.transport(plan.transport);
    bytes.context(plan.context);
    bytes.u64(plan.activation_state_slots.len() as u64);
    for slot in &plan.activation_state_slots {
        bytes.saved_value(slot.value);
        bytes.u32(slot.context_offset);
        bytes.u32(slot.byte_count);
    }
    bytes.u32(plan.sponsor_stack.alignment);
    bytes.u64(plan.sponsor_stack.byte_ceiling);
    bytes.u16(plan.interrupted_state.bits());
    bytes.u16(plan.saved_state.bits());
    bytes.u16(plan.restored_state.bits());
    bytes.entry(plan.transfer_entry);
    bytes.entry(plan.resume_entry);
    bytes.0
}

pub(super) fn non_authoritative_transfer_evidence_report_fingerprint(
    evidence: &NativeFuelTransferRuntimeEvidence,
) -> u64 {
    let mut hash = Fnv1a::new(b"omega.native-fuel-transfer-evidence.v1");
    hash.u64(evidence.plan.report_identity());
    hash.bytes(&evidence.plan.commitment().as_bytes());
    hash.text_evidence(&evidence.transfer_text);
    hash.text_evidence(&evidence.resume_text);
    hash.u64(
        evidence
            .physical_state_footprint
            .evidence_report_fingerprint(),
    );
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

#[derive(Default)]
struct CanonicalPlanBytes(Vec<u8>);

impl CanonicalPlanBytes {
    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
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
        let (class, index) = register_encoding(register);
        self.u8(class);
        self.u16(index);
    }

    fn entry(&mut self, entry: NativeFuelRuntimeEntryIdentity) {
        self.u64(entry.section_identity);
        self.u64(entry.symbol_identity);
    }
}

fn register_encoding(register: MachineRegister) -> (u8, u16) {
    match register {
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

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
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
