# Design Brief — Calling Plans (conventions as stated layouts over registers)

> **For:** Omega maintainer · **Status:** DIRECTION SETTLED (chat 2026-07-02,
> Zach) — vocabulary spellings provisional; the mechanism decisions are the
> settled part. · **Driver:** the UEFI/QEMU first-boot ladder needs a
> runtime-pointer call *now* and entry stubs *soon*; Linux syscalls and
> kernel32 calls already exist as hardcoded conventions in the backend; COM,
> AAPCS, and the interrupt frame are queued behind them. · **Sibling of:**
> [`programmable_layouts.md`](programmable_layouts.md) — same mechanism, a
> different carrier. · **Sequencing:** explicitly NOT a first-boot blocker
> (see §6).

## 1. Bottom line up front

**A calling convention is a layout over the register file + stack frame**
instead of a byte buffer: per-parameter placements, a return placement,
alignment, padding (shadow space *is* padding), and a clobber set. So it gets
the layout treatment: a **policy** (per-ABI, stated or computed, audited
against the psABI doc) produces a **validated plan** from a **signature**, and
compiler-owned derivers walk the plan. Omega's *internal* convention is never
stated and never expressible — conventions exist **only at boundaries**,
defaulted by the `Binding` kind so the common case names nothing.

## 2. The vocabulary

```omega
data Placement {
    case InReg(reg: Reg);            // rcx, xmm0, x0… — the placement vocabulary
    case OnStack(offset: count);     //   is PER-ARCHITECTURE, like byte offsets
    case ByPointer(reg: Reg);        // large aggregates pass by hidden pointer —
}                                    //   MS and SysV do this DIFFERENTLY (why this is data)

data CallPlan {
    params: [Placement];
    ret: Placement;
    clobbers: [Reg];                 // the volatile set — what a callee may destroy
    shadow: count;                   // MS-x64: 32; SysV: 0
    stack_align: count;              // 16 at the call site on both
}

trait CallingConvention {
    machine plan(signature: Signature) -> CallPlan;   // effect-free; build-time
}
```

`Signature` reflection (param/return types + sizes) is the same reflection
surface the layout work already needs (L0 `Schema` machinery, one more
consumer). Policies are ordinary data satisfying the trait — `MsX64`, `SysV`,
`LinuxSyscall` (≈ SysV with args in `R10` not `RCX` — a one-line diff between
two stated plans, versus a subtle hardcoded special case), `Aapcs64` when ARM
arrives, and `InterruptFrame` (the "convention" the CPU itself uses pushing a
trap frame — the boot brief's sample-6 stub is this plan's inbound direction).
SysV's recursive argument-classification algorithm is the *strongest* case for
policy-as-code: ~80 legible lines audited against the psABI beat the same
logic buried in instruction selection.

## 3. One plan, two derivers, agreement by construction

The same theorem shape as encode/materialize:

```text
convention.plan(signature) ──► validated CallPlan ──┬──► call encoder   (outbound: arrange
                                                    │     the world their way, `call rax`)
                                                    └──► entry stub     (inbound: unmarshal
                                                          their registers into machine args)
```

A caller and a callee derived from one stated plan cannot disagree about where
argument 2 lives. Since entry stubs are needed regardless (interrupt entry,
the UEFI export table, outbound callbacks — one foreign-initiated-activation
design, extern brief §12), the plan is the artifact that keeps both directions
honest.

**Validation** (before any deriver trusts a plan): no register
double-assignment, every parameter placed, placements type-compatible
(float→vector reg class), clobber set sane, stack offsets non-overlapping.
Same discipline as byte-plan validation, different invariants.

## 4. The sovereignty split — the exact mirror of data layouts

| | compiler-sovereign (never stated) | stated at edges |
|---|---|---|
| **data** | in-memory layout | serialized layout (policies) |
| **calls** | Omega↔Omega convention | boundary conventions (calling plans) |

Omega's internal convention may change any release; no user code can state,
observe, or depend on it. Calling plans attach **only** to boundary machines —
and are **defaulted by the `Binding` kind** (`Syscall(n)` implies the target's
syscall plan; `DllImport`/`VtableSlot` imply its C plan; an entry/export table
names its plan once), so in practice nobody writes a convention name at all.
This replaces ch19's provisional `abi "aarch64-darwin"` string-attribute
sketch — a string names nothing checkable; a policy is auditable data.

## 5. What stays compiler-owned

The derivers. Walking a plan means emitting per-arch instructions (moves into
registers, stack adjustment, the `call`/`ret`/`iretq`), exactly as byte-plan
lowering owns what `At`/`Bits` mean in loads and stores. Policies choose
placements; derivers own the machinery. Closed-vocabulary discipline
throughout.

## 6. Sequencing — explicitly not a first-boot blocker

Milestone 1 of the UEFI ladder ships on the **existing hardcoded MS-x64
encoder** (the five kernel32 ops) plus its `call rax` variant — no plan
mechanism required. The refactor target is the natural moment the mechanism
starts paying: when **entry stubs** land (second direction — agreement starts
mattering) or the **second convention** is touched (Linux syscalls are already
implicitly hardcoded; making both plans stated kills the special-casing).
Differential validation comes free at that point: call known-good libc/kernel
functions from plan-derived adapters and compare against clang-compiled
callers — the house oracle style, applied to ABI conformance.
