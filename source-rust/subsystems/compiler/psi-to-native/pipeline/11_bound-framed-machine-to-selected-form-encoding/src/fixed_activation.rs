//! Encode a cooperative transfer between the stepping caller and its fixed worker.
use crate::adapters::{AdapterEncodingError, encode_operation};
use calling_conventions::MachineRegister;
use physical_recipes::fixed_activation::{
    CALLER_CONTEXT, FixedTransferPlan, MXCSR, PENDING_PUBLICATION, PHASE, Phase, REPLY_DESTINATION,
    REPLY_SIZE, REPLY_TYPE, SAVED_REGISTERS, WORKER_CONTEXT, X87_CONTROL,
};
use post_allocation_machine::adapters::AdapterOperation;
use target::Architecture;

/// The link in the first step targets the selected worker's argument/result adapter.
/// No parked activation supplies executable code to the linker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFixedTransfer {
    pub step: Vec<u8>,
    pub worker_call: usize,
    pub prepare: Vec<u8>,
    pub park: Vec<u8>,
    pub resume: Vec<u8>,
}

pub fn encode<Origin>(
    plan: &FixedTransferPlan<Origin>,
) -> Result<EncodedFixedTransfer, AdapterEncodingError> {
    let transfer = plan.transfer();
    let mut code = encode_stack(transfer.stack_top)?;
    // The start path calls this measured adapter on the worker stack. Only
    // the adapter's own call remains to be linked to the published entry.
    let (adapter, worker_call) = crate::adapters::encode_instructions(&transfer.job)?;
    let adapter_offset = code.step.len();
    patch_forward(&mut code.step, code.worker_call, adapter_offset)?;
    code.worker_call = adapter_offset
        .checked_add(worker_call)
        .ok_or(AdapterEncodingError)?;
    code.step.extend_from_slice(&adapter);
    code.prepare = encode_prepare(
        transfer.stack_top,
        transfer.alignment,
        transfer.argument_offset,
        transfer.argument_bytes,
    )?;
    Ok(code)
}

fn encode_stack(stack_top: u32) -> Result<EncodedFixedTransfer, AdapterEncodingError> {
    let mut step = Vec::new();
    // RDI is the exclusively owned job. Its saved worker stack is empty until
    // park first records a real, unfinished invocation on that stack.
    expect_phase(&mut step, Phase::Ready);
    set_phase(&mut step, Phase::Running);
    context(&mut step, CALLER_CONTEXT, false)?;
    step.extend_from_slice(&[0x48, 0x83, 0xbf]); // cmpq $0, WORKER_CONTEXT(%rdi)
    step.extend_from_slice(&WORKER_CONTEXT.to_le_bytes());
    step.extend_from_slice(&[0, 0x0f, 0x85]); // jne resume
    let resume_offset = step.len();
    step.extend_from_slice(&[0; 4]);
    step.extend_from_slice(&[0x49, 0x89, 0xfc]); // mov %rdi,%r12 (execution owner)
    step.extend_from_slice(&[0x48, 0x8d, 0xa7]); // lea stack_top(%rdi),%rsp
    step.extend_from_slice(&stack_top.to_le_bytes());
    step.push(0xe8);
    let worker_call = step.len();
    step.extend_from_slice(&[0; 4]);
    // The worker adapter wrote its result by ordinary return. Restore the
    // caller with the Returned discriminator; its cleanup already ran.
    step.extend_from_slice(&[0x4c, 0x89, 0xe7, 0x31, 0xc0]); // mov %r12,%rdi; xor %eax,%eax
    set_phase(&mut step, Phase::Finished);
    context(&mut step, CALLER_CONTEXT, true)?;
    step.push(0xc3);
    let resume_entry = step.len();
    patch_forward(&mut step, resume_offset, resume_entry)?;
    context(&mut step, WORKER_CONTEXT, true)?;
    step.push(0xc3);

    Ok(EncodedFixedTransfer {
        step,
        worker_call,
        park: encode_park()?,
        prepare: Vec::new(),
        resume: encode_resume()?,
    })
}

fn encode_park() -> Result<Vec<u8>,AdapterEncodingError> {
    // A park takes the actual publication and the typed result destination.
    // The latter remains live on this worker's stack until a matching reply.
    let mut park = Vec::new();
    // RDI carries the issued publication; use volatile R9 to keep it while
    // authenticating the running owner before touching the pending record.
    park.extend_from_slice(&[0x49,0x89,0xf9,0x4c,0x89,0xe7]); // mov rdi,r9; mov r12,rdi
    expect_phase(&mut park, Phase::Running);
    pointer_word(
        &mut park,
        MachineRegister::X86R9,
        MachineRegister::X86R12,
        PENDING_PUBLICATION,
        false,
    )?;
    pointer_word(
        &mut park,
        MachineRegister::X86Rsi,
        MachineRegister::X86R12,
        REPLY_DESTINATION,
        false,
    )?;
    pointer_word(
        &mut park,
        MachineRegister::X86Rdx,
        MachineRegister::X86R12,
        REPLY_TYPE,
        false,
    )?;
    pointer_word(
        &mut park,
        MachineRegister::X86Rcx,
        MachineRegister::X86R12,
        REPLY_SIZE,
        false,
    )?;
    set_phase(&mut park, Phase::Parked);
    context(&mut park, WORKER_CONTEXT, false)?;
    park.extend_from_slice(&[0xb8, 1, 0, 0, 0]); // mov $1,%eax (Parked)
    context(&mut park, CALLER_CONTEXT, true)?;
    park.push(0xc3);

    Ok(park)
}

fn patch_forward(
    code: &mut [u8],
    offset: usize,
    destination: usize,
) -> Result<(), AdapterEncodingError> {
    let displacement = destination
        .checked_sub(offset + 4)
        .and_then(|n| i32::try_from(n).ok())
        .ok_or(AdapterEncodingError)?;
    code[offset..offset + 4].copy_from_slice(&displacement.to_le_bytes());
    Ok(())
}

// Arguments are the exclusive backing address, its original length and the job
// address. Rejected preparation must leave all three untouched.
fn encode_prepare(
    stack_top: u32,
    alignment: u32,
    argument_offset: u32,
    argument_bytes: u32,
) -> Result<Vec<u8>, AdapterEncodingError> {
    use MachineRegister as R;
    use physical_recipes::fixed_activation::{BACKING_BASE, BACKING_LENGTH};
    let mut bytes = Vec::new();
    let mut rejected = Vec::new();
    let mut refuse = |bytes: &mut Vec<u8>, condition: u8| {
        bytes.extend_from_slice(&[0x0f, condition]);
        rejected.push(bytes.len());
        bytes.extend_from_slice(&[0; 4]);
    };
    bytes.extend_from_slice(&[0x48, 0x85, 0xff]); // test %rdi,%rdi
    refuse(&mut bytes, 0x84);
    // The public input is a byte view and may begin at any address. Save its
    // full bounds, then align the private header inside that same view.
    bytes.extend_from_slice(&[0x49,0x89,0xf9]); // mov %rdi,%r9 (original base)
    bytes.extend_from_slice(&[0x48,0x89,0xf8,0x48,0x01,0xf0]); // end = base + length
    refuse(&mut bytes, 0x82);
    bytes.extend_from_slice(&[0x48,0x81,0xc7]); // add alignment-1,%rdi
    bytes.extend_from_slice(&(alignment-1).to_le_bytes());
    refuse(&mut bytes, 0x82);
    bytes.extend_from_slice(&[0x48,0x81,0xe7]); // and -alignment,%rdi
    bytes.extend_from_slice(&alignment.wrapping_neg().to_le_bytes());
    bytes.extend_from_slice(&[0x48,0x29,0xf8]); // remaining = end - aligned base
    refuse(&mut bytes, 0x82);
    bytes.extend_from_slice(&[0x48,0x3d]); // cmp stack_top,%rax
    bytes.extend_from_slice(&stack_top.to_le_bytes());
    refuse(&mut bytes, 0x82);
    bytes.extend_from_slice(&[0x49,0x89,0xf8]); // mov %rdi,%r8 (owner)
    pointer_word(&mut bytes, R::X86R9, R::X86Rdi, BACKING_BASE, false)?;
    pointer_word(&mut bytes, R::X86Rsi, R::X86Rdi, BACKING_LENGTH, false)?;
    set_phase(&mut bytes, Phase::Ready);
    bytes.extend_from_slice(&[0x31, 0xc0]); // xor %eax,%eax
    for offset in [
        WORKER_CONTEXT,
        PENDING_PUBLICATION,
        REPLY_DESTINATION,
        REPLY_TYPE,
        REPLY_SIZE,
    ] {
        pointer_word(&mut bytes, R::X86Rax, R::X86Rdi, offset, false)?;
    }
    if argument_bytes != 0 {
        bytes.extend_from_slice(&[0x48, 0x89, 0xd6, 0x48, 0x8d, 0xbf]); // mov %rdx,%rsi; lea argument(%rdi),%rdi
        bytes.extend_from_slice(&argument_offset.to_le_bytes());
        bytes.push(0xb9);
        bytes.extend_from_slice(&argument_bytes.to_le_bytes());
        bytes.extend_from_slice(&[0xf3, 0xa4]); // rep movsb
    }
    bytes.extend_from_slice(&[0x4c,0x89,0xc2, 0xb8, 1, 0, 0, 0, 0xc3]); // Ready, private owner in RDX
    let failure = bytes.len();
    for offset in rejected {
        patch_forward(&mut bytes, offset, failure)?;
    }
    bytes.extend_from_slice(&[0x31, 0xc0, 0xc3]); // Rejected
    Ok(bytes)
}

// RDI is the parked owner; RSI the answer's issued anchor; RDX its native
// value address; RCX the static byte width; R8 the closed reply type in this
// artifact. The caller materializes either result variant from the return bit.
fn encode_resume() -> Result<Vec<u8>, AdapterEncodingError> {
    use MachineRegister as R;
    let mut bytes = vec![0x48, 0x85, 0xf6, 0x0f, 0x84]; // test %rsi,%rsi; jz mismatch
    let null = bytes.len();
    bytes.extend_from_slice(&[0; 4]);
    compare_phase(&mut bytes, Phase::Parked);
    bytes.extend_from_slice(&[0x0f, 0x85]);
    let wrong_phase = bytes.len();
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&[0x48, 0x39, 0xb7]); // cmp %rsi,pending(%rdi)
    bytes.extend_from_slice(&PENDING_PUBLICATION.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x85]);
    let wrong_anchor = bytes.len();
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&[0x4c, 0x39, 0x87]); // cmp %r8,reply_type(%rdi)
    bytes.extend_from_slice(&REPLY_TYPE.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x85]);
    let wrong_type = bytes.len();
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&[0x48, 0x39, 0x8f]); // cmp %rcx,reply_size(%rdi)
    bytes.extend_from_slice(&REPLY_SIZE.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x85]);
    let wrong_size = bytes.len();
    bytes.extend_from_slice(&[0; 4]);
    set_phase(&mut bytes, Phase::Ready);
    pointer_word(&mut bytes, R::X86R9, R::X86Rdi, REPLY_DESTINATION, true)?;
    bytes.extend_from_slice(&[0x31, 0xc0]);
    pointer_word(&mut bytes, R::X86Rax, R::X86Rdi, PENDING_PUBLICATION, false)?;
    pointer_word(&mut bytes, R::X86Rax, R::X86Rdi, REPLY_DESTINATION, false)?;
    bytes.extend_from_slice(&[0x4c, 0x89, 0xcf, 0x48, 0x89, 0xd6, 0xf3, 0xa4]); // move reply to its live result home
    bytes.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xc3]);
    let failure = bytes.len();
    for offset in [null, wrong_phase, wrong_anchor, wrong_type, wrong_size] {
        patch_forward(&mut bytes, offset, failure)?;
    }
    bytes.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    Ok(bytes)
}

fn compare_phase(bytes: &mut Vec<u8>, phase: Phase) {
    bytes.extend_from_slice(&[0x80, 0xbf]); // cmpb phase, PHASE(%rdi)
    bytes.extend_from_slice(&PHASE.to_le_bytes());
    bytes.push(phase as u8);
}

fn expect_phase(bytes: &mut Vec<u8>, phase: Phase) {
    compare_phase(bytes, phase);
    // This internal call consumes linear custody of the named phase.
    bytes.extend_from_slice(&[0x74, 2, 0x0f, 0x0b]); // je 1f; ud2; 1:
}

fn set_phase(bytes: &mut Vec<u8>, phase: Phase) {
    bytes.extend_from_slice(&[0xc6, 0x87]); // movb phase, PHASE(%rdi)
    bytes.extend_from_slice(&PHASE.to_le_bytes());
    bytes.push(phase as u8);
}

fn context(bytes: &mut Vec<u8>, offset: u32, load: bool) -> Result<(), AdapterEncodingError> {
    for (index, register) in SAVED_REGISTERS.iter().copied().enumerate() {
        let member = offset + 8 * index as u32;
        pointer_word(bytes, register, MachineRegister::X86Rdi, member, load)?;
    }
    // Save or restore the two callee-preserved FP controls using the same job
    // base. The incoming FP environment is inherited on the first entry.
    bytes.extend_from_slice(&[0x0f, 0xae, if load { 0x97 } else { 0x9f }]);
    bytes.extend_from_slice(&(offset + MXCSR).to_le_bytes());
    bytes.extend_from_slice(&[0xd9, if load { 0xaf } else { 0xbf }]);
    bytes.extend_from_slice(&(offset + X87_CONTROL).to_le_bytes());
    Ok(())
}

fn pointer_word(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    base: MachineRegister,
    offset: u32,
    load: bool,
) -> Result<(), AdapterEncodingError> {
    encode_operation(
        Architecture::X86_64,
        isa_x86_64::validated_x86_64_physical_register_model(),
        &AdapterOperation::PointerMemory {
            register,
            base,
            offset,
            size: 8,
            load,
        },
        bytes,
    )
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    use super::*;
    use std::{fmt::Write, process::Command};

    #[test]
    fn two_parked_workers_resume_their_own_calls_and_run_cleanup_once() {
        // Each worker has a 64 KiB region below its own initial stack pointer.
        const TOP: u32 = 65_536;
        let code = encode_stack(TOP).unwrap();
        let prepare = encode_prepare(TOP, 16, 256, 4).unwrap();
        let mut assembly = String::from(".text\n.globl step\nstep:\n");
        let bytes = |out: &mut String, data: &[u8]| {
            for chunk in data.chunks(16) {
                let row = chunk
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                writeln!(out, ".byte {row}").unwrap();
            }
        };
        bytes(&mut assembly, &code.step[..code.worker_call - 1]);
        assembly.push_str("call run_worker\n");
        bytes(&mut assembly, &code.step[code.worker_call + 4..]);
        // C supplies the owner explicitly. The Omega call selector places it
        // in R12 at every suspending call, including when the caller has locals.
        assembly.push_str(".globl prepare\nprepare:\n");
        bytes(&mut assembly, &prepare);
        assembly.push_str(".globl answer\nanswer:\n");
        bytes(&mut assembly, &code.resume);
        assembly.push_str(
            ".globl park_with_owner\npark_with_owner:\n\
            push %r12\nmov %rdi,%r12\nmov %rsi,%rdi\nmov %rdx,%rsi\nmov $42,%edx\nmov $4,%ecx\n\
            call park\npop %r12\nret\npark:\n",
        );
        bytes(&mut assembly, &code.park);
        assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
        let work = std::env::temp_dir().join(format!("omega-fixed-workers-{}", std::process::id()));
        std::fs::create_dir_all(&work).unwrap();
        let source = work.join("transfer.s");
        let binary = work.join("fixed-workers");
        std::fs::write(&source, assembly).unwrap();
        let driver =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixed_activation.c");
        let built = Command::new("cc")
            .arg("-std=c11")
            .arg("-O1")
            .arg(format!("-DTOP={TOP}"))
            .arg(format!("-DPENDING={PENDING_PUBLICATION}"))
            .arg(format!("-DBASE={}",physical_recipes::fixed_activation::BACKING_BASE))
            .arg(format!("-DLENGTH={}",physical_recipes::fixed_activation::BACKING_LENGTH))
            .arg(format!("-DREPLY={REPLY_DESTINATION}"))
            .arg(&source)
            .arg(driver)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let ran = Command::new(binary).output().unwrap();
        assert_eq!(
            ran.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&ran.stderr)
        );
        assert!(ran.stdout.is_empty() && ran.stderr.is_empty());
        std::fs::remove_dir_all(work).unwrap();
    }
}

/// The native encoder alone can issue a new image with this origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedActivationEncoderOrigin(());

/// Emit the call issued by physical planning. Each public entry calls only its
/// own action and optional, statically selected worker in this text section.
pub fn encode_call<Origin>(plan:&bound_framed_machine::entries::BoundFixedActivationCall<Origin>)
    -> Result<machine_code::fixed_activation::EncodedFixedActivationCall<FixedActivationEncoderOrigin>,AdapterEncodingError> {
    use physical_recipes::fixed_activation::FixedActivationAction as Action;
    let call=plan.parts().call();
    let fixed=call.binding.fixed_activation().ok_or(AdapterEncodingError)?;
    let mut code=Vec::new();
    let emit=|code:&mut Vec<u8>,seq:&post_allocation_machine::adapters::AdapterInstructions| {
        let model=isa_x86_64::validated_x86_64_physical_register_model();
        for op in &seq.operations {encode_operation(Architecture::X86_64,model,op,code)?;}
        Ok::<_,AdapterEncodingError>(())
    };
    emit(&mut code,&call.before)?;
    code.push(0xe8);
    let helper_call=code.len();
    code.extend_from_slice(&[0;4]);
    if let Some(success)=&call.nonzero {
        code.extend_from_slice(&[0x85,0xc0,0x0f,0x85]); // test eax,eax; jne nonzero
        let nonzero=code.len(); code.extend_from_slice(&[0;4]);
        emit(&mut code,&call.zero)?;
        code.push(0xe9);
        let finish=code.len(); code.extend_from_slice(&[0;4]);
        let at=code.len(); patch_forward(&mut code,nonzero,at)?;
        emit(&mut code,success)?;
        let at=code.len(); patch_forward(&mut code,finish,at)?;
    } else {emit(&mut code,&call.zero)?;}
    emit(&mut code,&call.finish)?;
    let helper=code.len();
    patch_forward(&mut code,helper_call,helper)?;
    let worker_call=match fixed.action {
        Action::Prepare{..}=>{
            let t=call.transfer.as_ref().ok_or(AdapterEncodingError)?.transfer();
            code.extend(encode_prepare(t.stack_top,t.alignment,t.argument_offset,t.argument_bytes)?);
            None
        }
        Action::Step{worker,..}=>{
            let encoded=encode(call.transfer.as_ref().ok_or(AdapterEncodingError)?)?;
            let mut field=helper.checked_add(encoded.worker_call).ok_or(AdapterEncodingError)?;
            code.extend(encoded.step);
            if let Some(bridge)=&call.entry_bridge {
                let (bridge,callee)=crate::adapters::encode_instructions(bridge.instructions())?;
                let at=code.len();
                patch_forward(&mut code,field,at)?;
                field=at.checked_add(callee).ok_or(AdapterEncodingError)?;
                code.extend(bridge);
            }
            Some((field,worker))
        }
        Action::Park{..}=>{code.extend(encode_park()?);None}
        Action::Resume{..}=>{code.extend(encode_resume()?);None}
    };
    Ok(machine_code::fixed_activation::EncodedFixedActivationCall::from_encoder(
        FixedActivationEncoderOrigin(()),call.physical_identity,std::sync::Arc::clone(&call.frame),call.binding.clone(),code,worker_call,call.caller_stack_bytes))
}
