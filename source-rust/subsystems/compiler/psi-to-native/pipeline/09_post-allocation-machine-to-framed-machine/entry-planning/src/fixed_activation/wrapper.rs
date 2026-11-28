//! Translate the authenticated, ordinary caller ABI to the private transfer
//! protocol. Neither case numbers nor payload positions are inferred here.
use super::plan_transfer;
use crate::bridge_values::{Encoding, ValueStorage, WordLocation, value_storage};
use calling_conventions::{MachineRegister as R, ValueClass};
use physical_recipes::fixed_activation::{BACKING_BASE, BACKING_LENGTH, FixedCall, FixedCallPlan};
use post_allocation_machine::adapters::{AdapterInstructions as Ops, AdapterOperation as Op};
use target_vocabulary::{
    FixedActivationAction as Action, FixedCallFrame as Frame, FixedCase, FixedField,
    NormalizedForeignCallBinding,
};

type Result<T> = std::result::Result<T, &'static str>;
type Home = (u32, u32);
const TRANSPORT: &str = "fixed activation value has no complete System V transport";

pub fn plan_call(
    body: &crate::PhysicalFrameProgram,
    installations: installation_admission::EntryInstallationResources<'_>,
    binding: &NormalizedForeignCallBinding,
) -> Result<bound_framed_machine::entries::BoundFixedActivationCall<crate::PhysicalRecipeOrigin>> {
    if body.instructions.target != target::NativeTarget::linux_x64() {
        return Err("unsupported fixed activation target");
    }
    let fixed = binding
        .fixed_activation()
        .ok_or("expected a compiler-issued fixed activation")?;
    if matches!(fixed.action, Action::Park { .. }) {
        require_stepped_park(body, fixed.boundary)?;
    }
    let frame = Frame::for_binding(binding).ok_or("invalid fixed activation ABI frame")?;
    let worker = match fixed.action {
        action if action.prepares_split() => None,
        Action::Prepare { worker, .. } | Action::Step { worker, .. } => Some(worker),
        _ => None,
    };
    let mut transfer = None;
    let mut entry_bridge = None;
    if let Some(worker) = worker {
        let entry = crate::callable_entry::plan_fixed_worker_entry(body, worker, installations)
            .map_err(|_| "fixed worker has no callable physical entry")?;
        if matches!(fixed.action, Action::Step { .. }) {
            entry_bridge = entry.clone().into_parts().bridge;
        }
        transfer = Some(plan_transfer(body, entry)?.1);
    }
    let wrap = Wrapper {
        frame: &frame,
        encoding: Encoding {
            architecture: target::Architecture::X86_64,
            model: target_x86_64::validated_x86_64_physical_register_model(),
            scratch: R::X86R10,
            address_scratch: R::X86R11,
            aligned_homes: Vec::new(),
        },
    };
    let mut before = blank();
    let mut zero = blank();
    let mut nonzero = None;
    let mut finish = blank();
    let call = &binding.boundary_entry_plan.call;
    let inputs = call
        .parameters
        .iter()
        .zip(&frame.inputs)
        .map(|(p, (_, n))| value_storage(p, *n).map_err(|_| TRANSPORT))
        .collect::<Result<Vec<_>>>()?;
    let output = call
        .result
        .as_ref()
        .map(|p| value_storage(p, frame.result.1).map_err(|_| TRANSPORT))
        .transpose()?;
    let granule =
        frame_admission::frame_layout::stack_commit_granule_bytes(body.instructions.target)
            .ok_or("unclosed wrapper stack granule")?;
    let touches = if u64::from(frame.frame_bytes) > granule {
        u32::try_from(u64::from(frame.frame_bytes).div_ceil(granule)).map_err(|_| TRANSPORT)?
    } else {
        0
    };
    before.frame(frame.frame_bytes, granule, touches, true);
    let stack_bias = frame.frame_bytes + 8;
    let enc = &wrap.encoding;
    // First preserve all incoming registers; stack copying can use volatile scratch.
    if let Some(ValueStorage::Indirect { pointer, .. }) = &output {
        enc.capture_registers(&mut before, &[*pointer], Frame::OUTPUT_POINTER)
            .map_err(|_| TRANSPORT)?;
        enc.capture_stack(&mut before, &[*pointer], Frame::OUTPUT_POINTER, stack_bias)
            .map_err(|_| TRANSPORT)?;
    }
    for (input, (offset, _)) in inputs.iter().zip(&frame.inputs) {
        enc.capture_registers(&mut before, input.pieces(), *offset)
            .map_err(|_| TRANSPORT)?;
    }
    for (input, (offset, n)) in inputs.iter().zip(&frame.inputs) {
        enc.capture_stack(&mut before, input.pieces(), *offset, stack_bias)
            .map_err(|_| TRANSPORT)?;
        if input.is_indirect() {
            enc.copy_word(&mut before, *offset, Frame::INPUT_POINTER, 8)
                .map_err(|_| TRANSPORT)?;
            enc.copy_indirect(&mut before, *offset, Frame::INPUT_POINTER, *n, true)
                .map_err(|_| TRANSPORT)?;
        }
    }
    wrap.clear_result(&mut before)?;
    match fixed.action {
        Action::Prepare {
            tag,
            ready,
            rejected,
            ..
        } => {
            let [argument, backing] = frame.inputs.as_slice() else {
                return Err(TRANSPORT);
            };
            if transfer
                .as_ref()
                .is_some_and(|t| t.transfer().argument_bytes != argument.1)
            {
                return Err("fixed worker job and prepare input differ");
            }
            if matches!(
                call.parameters[1].shape.class,
                ValueClass::BorrowedReference
            ) {
                if backing.1 != 8 || call.parameters[1].shape.byte_size != 16 {
                    return Err(TRANSPORT);
                }
                enc.copy_indirect(&mut before, Frame::BACKING, backing.0, 16, true)
                    .map_err(|_| TRANSPORT)?;
            } else {
                wrap.copy(&mut before, *backing, (Frame::BACKING, 16))?;
            }
            wrap.load(&mut before, R::X86Rdi, Frame::BACKING)?;
            wrap.load(&mut before, R::X86Rsi, Frame::BACKING + 8)?;
            wrap.address(&mut before, R::X86Rdx, argument.0)?;
            let accepted = nonzero.insert(blank());
            wrap.tag(accepted, tag, ready)?;
            let owner = sub(frame.result, ready.fields[0], 8)?;
            enc.memory(accepted, R::X86Rdx, owner, 8, false)
                .map_err(|_| TRANSPORT)?;
            wrap.tag(&mut zero, tag, rejected)?;
            wrap.field(&mut zero, rejected.fields[0], *argument)?;
            wrap.field(&mut zero, rejected.fields[1], (Frame::BACKING, 16))?;
        }
        Action::Step {
            tag,
            returned,
            parked,
            ..
        } => {
            let [owner] = frame.inputs.as_slice() else {
                return Err(TRANSPORT);
            };
            if owner.1 != 8 {
                return Err(TRANSPORT);
            }
            wrap.load(&mut before, R::X86Rdi, owner.0)?;
            let suspended = nonzero.insert(blank());
            wrap.tag(suspended, tag, parked)?;
            wrap.field(suspended, parked.fields[0], *owner)?;
            wrap.tag(&mut zero, tag, returned)?;
            let data = transfer.as_ref().ok_or(TRANSPORT)?.transfer();
            let result = wrap.destination(returned.fields[0])?;
            if result.1 != data.result_bytes {
                return Err("worker return width differs from its checked outcome");
            }
            enc.copy_indirect_at(
                &mut zero,
                result.0,
                owner.0,
                data.result_offset,
                result.1,
                true,
            )
            .map_err(|_| TRANSPORT)?;
            let backing = wrap.destination(returned.fields[1])?;
            if backing.1 != 16 {
                return Err("returned backing is not a complete mutable slice");
            }
            enc.copy_indirect_at(&mut zero, backing.0, owner.0, BACKING_BASE, 8, true)
                .map_err(|_| TRANSPORT)?;
            enc.copy_indirect_at(&mut zero, backing.0 + 8, owner.0, BACKING_LENGTH, 8, true)
                .map_err(|_| TRANSPORT)?;
        }
        Action::Park {
            anchor,
            reply_type,
            reply_bytes,
        } => {
            let [publication] = frame.inputs.as_slice() else {
                return Err(TRANSPORT);
            };
            if reply_bytes != frame.result.1 {
                return Err(TRANSPORT);
            }
            wrap.load(&mut before, R::X86Rdi, sub(*publication, anchor, 8)?)?;
            wrap.address(&mut before, R::X86Rsi, frame.result.0)?;
            constant(&mut before, R::X86Rdx, u64::from(reply_type.0));
            constant(&mut before, R::X86Rcx, u64::from(reply_bytes));
        }
        Action::Resume {
            anchor,
            reply_type,
            reply_bytes,
            tag,
            ready,
            mismatch,
            ..
        } => {
            let [owner, permit, reply] = frame.inputs.as_slice() else {
                return Err(TRANSPORT);
            };
            if owner.1 != 8 || reply.1 != reply_bytes {
                return Err(TRANSPORT);
            }
            wrap.load(&mut before, R::X86Rdi, owner.0)?;
            wrap.load(&mut before, R::X86Rsi, sub(*permit, anchor, 8)?)?;
            wrap.address(&mut before, R::X86Rdx, reply.0)?;
            constant(&mut before, R::X86Rcx, u64::from(reply_bytes));
            constant(&mut before, R::X86R8, u64::from(reply_type.0));
            let matched = nonzero.insert(blank());
            wrap.tag(matched, tag, ready)?;
            wrap.field(matched, ready.fields[0], *owner)?;
            wrap.tag(&mut zero, tag, mismatch)?;
            for (field, source) in mismatch.fields.into_iter().zip([*owner, *permit, *reply]) {
                wrap.field(&mut zero, field, source)?;
            }
        }
    }
    match output {
        Some(ValueStorage::Indirect { .. }) => {
            enc.copy_indirect(
                &mut finish,
                frame.result.0,
                Frame::OUTPUT_POINTER,
                frame.result.1,
                false,
            )
            .map_err(|_| TRANSPORT)?;
            wrap.load(&mut finish, R::X86Rax, Frame::OUTPUT_POINTER)?;
        }
        Some(ValueStorage::Direct(pieces)) => {
            if pieces
                .iter()
                .any(|p| matches!(p.location, WordLocation::Stack(_)))
            {
                return Err(TRANSPORT);
            }
            enc.distribute(&mut finish, &pieces, frame.result.0)
                .map_err(|_| TRANSPORT)?;
        }
        None => {}
    }
    finish.frame(frame.frame_bytes, granule, touches, false);
    finish.push(Op::Return);
    let plan = FixedCallPlan::from_planner(
        crate::PhysicalRecipeOrigin(()),
        FixedCall {
            physical_identity: body.instructions.identity,
            frame: body.frame.shared_plan(),
            binding: binding.clone(),
            before,
            zero,
            nonzero,
            finish,
            transfer,
            entry_bridge,
            caller_stack_bytes: frame.caller_stack_bytes(),
        },
    );
    Ok(bound_framed_machine::entries::BoundEntry::from_binding(
        installation_binding_issuer::BINDER,
        plan,
    ))
}

/// An ordinary process or callback entry has no activation owner in R12.
/// Follow its solved machine calls. A Step helper transfers to the selected
/// worker on a separate stack, so it deliberately introduces no edge here.
fn require_stepped_park(
    body: &crate::PhysicalFrameProgram,
    boundary: semantic_vocabulary::BoundaryMachineId,
) -> Result<()> {
    let root = body.instructions.entry;
    let graph = frame_planning::stack_demand::plan_physical_stack_program_for_entry(body, root)?;
    let mut pending = vec![root];
    let mut visited = std::collections::BTreeSet::new();
    let calls = graph
        .functions
        .iter()
        .map(|f| (f.machine, f))
        .collect::<std::collections::BTreeMap<_, _>>();
    while let Some(machine) = pending.pop() {
        if !visited.insert(machine) {
            continue;
        }
        let function = body
            .instructions
            .function(machine)
            .ok_or("entry call has no compiled function")?;
        if function
            .normalized_foreign_calls
            .iter()
            .any(|r| r.call.boundary == boundary)
        {
            return Err("native park is reachable without stepping an owned activation");
        }
        let row = calls
            .get(&machine)
            .ok_or("entry call lacks a solved call graph")?;
        pending.extend(row.calls.iter().map(|call| call.target));
    }
    Ok(())
}

struct Wrapper<'a> {
    frame: &'a Frame,
    encoding: Encoding,
}
fn blank() -> Ops {
    Ops::new(target::Architecture::X86_64)
}
fn constant(code: &mut Ops, register: R, value: u64) {
    code.push(Op::Constant { register, value });
}
fn sub(home: Home, field: FixedField, n: u32) -> Result<u32> {
    if field.bytes != n || field.offset.checked_add(n).is_none_or(|end| end > home.1) {
        return Err("activation field exceeds its checked value");
    }
    home.0.checked_add(field.offset).ok_or(TRANSPORT)
}
impl Wrapper<'_> {
    fn destination(&self, field: FixedField) -> Result<Home> {
        Ok((sub(self.frame.result, field, field.bytes)?, field.bytes))
    }
    fn field(&self, code: &mut Ops, field: FixedField, source: Home) -> Result<()> {
        self.copy(code, source, self.destination(field)?)
    }
    fn copy(&self, code: &mut Ops, source: Home, destination: Home) -> Result<()> {
        if source.1 != destination.1 {
            return Err("fixed activation transport size differs from its case field");
        }
        for (offset, width) in words(source.1) {
            self.encoding
                .copy_word(code, source.0 + offset, destination.0 + offset, width)
                .map_err(|_| TRANSPORT)?;
        }
        Ok(())
    }
    fn clear_result(&self, code: &mut Ops) -> Result<()> {
        constant(code, R::X86R10, 0);
        for (offset, width) in words(self.frame.result.1) {
            self.encoding
                .memory(code, R::X86R10, self.frame.result.0 + offset, width, false)
                .map_err(|_| TRANSPORT)?;
        }
        Ok(())
    }
    fn load(&self, code: &mut Ops, reg: R, offset: u32) -> Result<()> {
        self.encoding
            .memory(code, reg, offset, 8, true)
            .map_err(|_| TRANSPORT)
    }
    fn address(&self, code: &mut Ops, reg: R, offset: u32) -> Result<()> {
        self.encoding
            .address_register(code, reg, offset)
            .map_err(|_| TRANSPORT)
    }
    fn tag<const N: usize>(
        &self,
        code: &mut Ops,
        tag: FixedField,
        case: FixedCase<N>,
    ) -> Result<()> {
        let slot = sub(self.frame.result, tag, 4)?;
        constant(code, R::X86R10, u64::from(case.tag));
        self.encoding
            .memory(code, R::X86R10, slot, 4, false)
            .map_err(|_| TRANSPORT)
    }
}
fn words(len: u32) -> impl Iterator<Item = (u32, u8)> {
    let mut offset = 0;
    std::iter::from_fn(move || {
        let width = [8u8, 4, 2, 1]
            .into_iter()
            .find(|&n| u32::from(n) <= len - offset)?;
        let start = offset;
        offset += u32::from(width);
        Some((start, width))
    })
}
