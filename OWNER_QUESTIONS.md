# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`.

Last pruned: 2026-07-20.

## 1. Control-flow integrity and protected returns

Executable installation is settled separately and prevents code injection.
Control-flow integrity is an independent gate over every executable artifact,
including the boot-admitted installer itself.

The forward edge is substantially shaped: direct branches are fixed and
final-artifact validated; indirect calls and tail calls use sealed,
requirement-compatible entry references rather than numeric addresses; dynamic
descriptors retain satisfier/contract identity; checked assembly cannot add an
unmodeled control exit; interrupt/exception exits are deriver-owned under
`CallPlan + StatePlan`.

The remaining owner decision is the backward edge and enforcement contract:

- What normalized fact proves a return corresponds to its legitimate call or
  continuation state?
- Which guarantees come from software proof, protected control storage,
  shadow stacks/CET, PAC, or another target mechanism?
- How do suspension, cancellation, exceptions, interrupts, tail calls, and
  component/provider crossings preserve the return discipline?
- What final-artifact certificate lets an independent validator check every
  indirect call, return, entry stub, veneer, and thunk after placement?
- Must admitted foreign providers supply accepted CFI claims, run behind
  hardware isolation, or both according to policy?

Recommendation: one normalized CFI plan/certificate consumed by the final
validator, with checked Omega producing evidence and opaque leaves either
receipt-gated or isolated. Keep target mechanisms as realizations of the plan,
not source attributes or a new `unsafe` escape.

Detailed surrounding context and engineering residue are in
[`wiki/design_briefs/os_memory_and_hardware_foundation.md`](wiki/design_briefs/os_memory_and_hardware_foundation.md).

## 2. What is Cathedral's first x86 interrupt state policy?

The normalized `CallPlan + StatePlan` vocabulary and validator are ready, but
the first timer profile cannot be derived until Cathedral chooses the hardware
entry policy that both the stub and installed-root/WCSU analysis must enforce.
Cathedral's own open-question ledger still leaves its initial x86 stack classes,
masking/preemption graph, and WCSU composition unresolved.

Decide, for the first timer root:

- whether it uses the interrupted stack or a dedicated IST stack, and which
  exception/root classes may share or preempt that stack;
- whether the gate masks interrupts for the whole handler, where nesting may be
  re-enabled, and whether the timer root may re-enter itself;
- the exact interrupted machine-state set the stub saves/restores and the
  transitive state ceiling exposed to checked handler code; and
- whether the first acknowledgement token represents legacy PIC EOI, LAPIC EOI,
  or a target-selected protocol with distinct concrete providers.

Recommendation: start with a non-reentrant interrupt gate on a dedicated IST
stack, interrupts masked until deriver-owned exit, a full integer/control-state
save with no SIMD use permitted transitively, and a protocol-neutral linear
acknowledgement requirement refined by PIC/LAPIC providers. This is deliberately
conservative and can later admit nesting or a broader state ceiling, but it is
still an OS policy choice because it fixes stack demand and preemption edges.

## 3. What is the generated post-handoff writer boundary?

The normalized materializer can derive an atomic writer plan, and exact
`InstalledCode` can privately resolve only entry identities admitted with that
artifact. What remains is not an opcode-selection question: no platform
boundary yet specifies how generated target code receives resolver authority,
destination authority, staging storage, or publication/failure obligations.

Decide:

- whether the writer is a compiler-generated checked Omega machine, a
  provider-private admitted entry, or code inlined into each platform provider;
- what sealed capability resolves `DataSymbolId` and `EntryStubId` without
  exposing a general numeric-address operation;
- who owns the full-plan staging buffer required for all-or-nothing publication,
  how its maximum size/alignment is admitted, and what happens on allocation or
  resolution failure;
- which memory-order/cache/device-visibility fact constitutes publication for
  ordinary RAM versus hardware-consumed tables; and
- how the writer's call/state plan, footprint evidence, destination extent,
  installation scope, and installed target lifetimes are bound into one receipt.

Recommendation: generate a provider-private checked Omega machine from the
normalized writer plan. It should consume an exact destination extent plus a
sealed resolver capability, use provider-owned bounded staging storage admitted
with the plan, and return a receipt establishing one target-specific publication
fact. The machine may lower normally under its evaluated call/state plan; do not
standardize a public callback ABI or let ordinary Omega code observe resolved
addresses.

## 4. What is the native boundary ABI for fixed arrays and text descriptors?

Primitive scalars and declared `data` records/cases now have normalized entry
result shapes across Microsoft x64, SysV AMD64, and AAPCS64. Fixed arrays and
the current builtin `String` descriptor do not: neither has a declared-data
layout symbol, and C-family ABIs do not provide one uniform source-level rule
for returning arrays by value. Treating both as anonymous integer aggregates
would be mechanically possible, but would silently establish a public ABI and
would interact with the planned retirement of builtin `String` in favor of
domain-qualified `[u8]` values.

Decide:

- whether fixed arrays are legal ordinary-boundary parameters/results by value,
  and if so whether their ABI class is structural (including float HFA/SSE
  classification) or always opaque/in-memory;
- whether `{ptr, len}` text/slice descriptors are stable public ABI values or
  must cross only through explicit admitted record types;
- whether process-entry `main` may declare any native result shape, or must be
  restricted to the platform's exit-status scalar even though callable/firmware
  entries may return aggregates; and
- whether the answer belongs in `Calling<C>` policy evaluation so custom
  policies can reject or classify these shapes explicitly.

Recommendation: keep process `main` restricted to an exit-status integer, make
fixed-array/text boundary legality explicit in the evaluated calling policy,
and classify admitted fixed arrays structurally (including HFA/SSE rules) while
requiring text to use an explicit public descriptor record after String
retirement. Do not infer either ABI from byte size alone.

## 5. What is the runtime and object-safety contract for `dyn Trait`?

Closed-world call-site specialization currently makes `&dyn Trait` parameters
execute correctly when every concrete receiver is known at its call site. It
cannot represent a runtime-varying trait value stored in data, passed across a
component boundary, or rebound to one of several satisfiers. The language guide
explicitly leaves the runtime representation and boundary legality open, while
the remaining task requires descriptors that preserve satisfier identity.

Decide:

- whether the stable value is a two-word `{instance, table}` pair whose table
  identity names the satisfier, or carries a separate sealed satisfier/contract
  identity (or component/endpoint handle);
- which trait signatures are object-safe, especially `Self` outside the
  receiver, unbound trait parameters, value returns, generic requirements,
  effects, capabilities, and boundary machines;
- whether `dyn Trait` may be owned/stored directly or only borrowed, and how
  lifetime, mutability, drop, migration, and hot-swap pinning travel with it;
- who emits, owns, versions, validates, and updates machine tables, including
  the ABI identity used across separately built components; and
- how named satisfier selection and third-party named-only conformances are
  encoded and checked at coercion.

Recommendation: use a sealed descriptor whose logical identity is
`{instance, satisfier_contract}` and let a validated target-specific table be a
private realization of that contract. Initially admit only borrowed receivers,
fully bound trait parameters, and requirements whose nonreceiver
parameters/results do not mention `Self`; require declared effect/capability
ceilings at every dynamic slot. This keeps the public model independent of raw
table addresses and leaves room for loader-controlled table replacement.
