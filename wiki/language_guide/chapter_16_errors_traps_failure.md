# Chapter 16: Errors, Traps, And Failure

Failure semantics must be explicit. Omega has no hidden exceptions, no second
control-flow system, and no ambient panic. Recoverable failure is **data**;
deliberate termination is an **opt-in control outcome**; everything else that
other languages call a "trap" is, in a proof-oriented language, a **compile
error**. Reaching the service that performs process exit is separately visible
in the reach row; the terminal outcome itself is not service reach.

## Recoverable Failure Is A Sum, Handled At A Transition Boundary

Recoverable failure is modelled as an ordinary sum and handled by an exhaustive
transition. There is no dedicated error type and no propagation operator —
because the type system already supplies every guarantee one would want from
them.

```omega
data ParseResult {
    case Parsed(value: i32);
    case BadDigit(at: u32);
    case Empty;
}

machine App::run(&mut self, input: &[u8]) {
    let r = self.parser.parse(input);   // cannot be silently ignored
    transition r {                       // every case must be handled
        ParseResult::Parsed  { value } -> self.use_value(value)   // payload in scope ONLY here
        ParseResult::BadDigit { at }   -> self.report(at)
        ParseResult::Empty             -> self.report_empty()
    }
}
```

The three guarantees that other languages bolt on for errors are corollaries of
mechanisms Omega already has:

| Guarantee | Omega mechanism (general, not error-specific) |
|---|---|
| The result cannot be ignored | **Strict use:** discarding a non-unit return is a compile error; intentional discard is `_ = call();`. |
| Every failure case must be handled | **Exhaustive transitions** over the sum's cases. |
| The success payload is unreachable on a failure path | **Case-payload binding:** `value` exists only inside the `Parsed` arm. |

So the error model is not a feature. It is what falls out of sums + exhaustive
transitions + strict-use. A library `Result<T, E>` is *permitted* but buys
nothing special; most code should use a domain-specific sum whose cases name the
actual failures.

## Success Cases Carry Proven Facts (The Fact Catalog)

A transition over a sum is a **partition**: each arm learns which case holds, and
therefore that case's facts. This is the same flow-sensitive narrowing the
arithmetic domains use for scalar guards (chapter 7)—one unified
**fact catalog** threaded through the control-flow graph, carrier-generic over
scalars (intervals), sums (which-case), slices (length, encoding), and
references (validity).

A machine's contract may attach a fact to a success payload, and the handling
arm inherits it:

```omega
data Slot {
    case Found(index: u32);
    case Full;
}

machine Table::find_free(&self) -> Slot
ensures Found.index in 0..16          // success guarantees index < capacity
{ /* returns Found(i) only for i < 16, else Full */ }

machine App::insert(&mut self, value: i32) {
    transition self.table.find_free() {
        Slot::Found { index } -> self.entries[index] = value   // proven in-bounds, no re-check
        Slot::Full            -> self.report_full()
    }
}
```

Inside the `Found` arm, `index in 0..16` is known, so the index is proven
in-bounds with no re-guard. This is the payoff over a plain sum: the success path
discharges downstream obligations (bounds, ranges, non-null, encoding) instead of
re-proving them.

Facts flow freely **within** a machine (flow-sensitive, narrowed by guards and by
case partitions, intersecting to the tightest bound). Across a **call** they are
mediated by the callee's `requires`/`ensures` contract: the caller proves the
`requires` from its local facts and assumes the `ensures` into its catalog. The
callee is verified once against its own contract — modular, separate-compilation
friendly. Contracts are **inferred** for non-exported machines within a
compilation unit (so most code carries none) and **written** only at boundaries
where inference cannot see: exported APIs, separate-compilation edges, recursion.

## Propagation Is An Explicit Edge — There Is No `?`

`?` desugars to early-return from the middle of a function body. A state has a
shared straight-line body and exactly one transition; there is no return from the
middle, and splitting a state at a mid-body `?` would be a within-state
transition, which the model forbids. So **failure handling lives at the
transition boundary**, and propagating a failure upward is just an arm that
targets a state returning the caller's own failure:

```omega
machine App::run2(&mut self, input: &[u8]) -> ParseResult {
    transition self.parser.parse(input) {
        ParseResult::Parsed  { value } -> self.done(value)
        ParseResult::BadDigit { at }   -> self.bubble_bad(at)    // bubble up, unchanged
        ParseResult::Empty             -> self.bubble_empty()
    }
    // states that re-wrap and return ParseResult ...
}
```

This is deliberately verbose. There is no propagation sugar (no `?`, no `fails`
keyword): a new keyword that exists to save typing on the happy path is exactly
the kind of hidden control flow this chapter rejects.

## There Is No "Trap" Category For Logic

In a proof-oriented language, the failures other languages trap at runtime are
discharged at compile time:

- a **proven-impossible state** is dead code (eliminated) or a contradiction (a
  compile error) — never a runtime trap;
- a **failed exact-arithmetic proof** is a compile error because exact
  arithmetic is a proof obligation;
- a **provable contract violation** is a compile error.

When the prover *cannot* discharge an obligation, that does not silently become a
runtime trap. The obligation must be **handled** (an exhaustive transition over
the failure sum) or **deliberately abandoned** (the opt-in abort control outcome below). Prover
incompleteness produces a required handler or an explicit opt-in — never a hidden
death.

The one in-language failure that legitimately reaches runtime is **opt-in
`Trapping` arithmetic**: `T::Trapping` emits a hardware trap on
overflow. It is explicit in the type, chosen deliberately, and already
implemented. That is the model — not a default.

There is no `expect`/`unwrap`. "I know this cannot fail here" is spelled by
*proving* the failure case impossible (the provably-dead arm discharges
exhaustiveness with no handler), not by asserting it at runtime.

## Crashes Are Explicit, Guarded Control Ceilings

`crashes` publishes the ways an invocation may leave without returning or
running cleanup. The initial causes are `Trap` and `Abort`. `Trap` covers an
operation- or platform-triggered fault; `Abort` is deliberate execution-domain
termination. One clause names one cause:

```omega
machine divide(numerator: i32, denominator: i32) -> i32
crashes Trap
    denominator == 0
    numerator == i32::Minimum && denominator == -1
{
    // ...
}
```

The indented facts are alternative **routes**: any one permits the named crash.
They are not the conjunctive list used by `requires`. Repeating a clause for the
same cause contributes more alternatives to the same canonical bucket. A
route predicate is an ordinary Boolean proof expression and may use
`&&`, `||`, or other ordinary operators internally.
Like every contract expression, it must be total. Direct Trapping arithmetic
therefore rejects and cannot itself create a crash route. Authors restate a
primitive's trap condition with explicit total denotations such as
`embed(value)` for fixed integers and addresses or `Float::meaning32(value)`
for `f32`; the executable body remains the sole source of the derived crash
site. See
[Total Specification Arithmetic](../design_briefs/total_specification_arithmetic.md).

One cause appears per clause. The formatter renders one route per line and
starts routes below the cause. A clause with no routes is unconditional.
Omitting a cause entirely is the negative guarantee that the machine cannot
crash for that cause. Private checked machines may infer crash ceilings;
exports, requirements, and boundaries publish them.

`crash Abort;` and `crash Trap;` are explicit no-return terminals. Operations
with intrinsic crash behavior, such as `Trapping` arithmetic, contribute crash
sites and guards without requiring a source terminal statement. Every checked
site must be covered by the published routes:

```text
derived_site_guard
    implies OR(published_route_guards for the same cause)
```

The derived guard includes the path condition. A trap-capable division inside
`if x > 0` therefore contributes `x > 0 && denominator == 0`, not merely the
primitive's local guard.

At a call, arguments and current facts refine the published routes. A cause is
removed only after every surviving route for that cause is disproved. This is
why `divide(10, 2)` is crash-free at that invocation even though `divide` is
published as trap-capable.

The explicit terminal outcome and service reach remain separate axes. An abort
lowering may also reach the `ProcessExit` boundary service; neither fact implies
the other. Graceful shutdown remains ordinary cleanup followed by `exit(code)`.

## Compiler-owned stack storage and spill accesses

Compiler-selected spill slots are part of an activation's frame storage, not
new source-authored memory operations or crash routes. Their final physical
extent contributes to the existing worst-case stack usage (WCSU) derivation.
Register allocation may change the resources required by a native artifact;
it does not add `crashes Trap` to the source machine or weaken the negative
guarantee given by an omitted cause.

This relies on the [runtime cycle contract](chapter_3_machines.md): tail
recursion lowers to iterative backedges with no accumulating frames, and
non-tail runtime recursion rejects. The admitted runtime call chains therefore
have a statically bounded stack demand. A termination measure does not size a
frame or introduce recursive stack growth. No new recursion-depth bound or
per-call exhaustion protocol is introduced by spill realization.

Use the existing stack contracts in this order:

1. Derive each final validated frame extent, including spill-slot reuse,
   alignment, saved registers, and calling-plan storage. Charge live physical
   storage, not the sum of all spill instructions or overlapping slot demands.
2. Compose those extents through the existing WCSU call-chain and external-root
   context/nesting rules, including checked or admitted same-stack providers.
3. Compare the composed demand with the admitted stack supply, and establish
   valid backing through the existing `StackPlan`/`StackLease` or external-entry
   provisioning contract before the checked activation executes. Maintain that
   backing for the activation's required lifetime, including suspension.

A byte-count comparison alone does not establish usable memory. The selected
target/provider must satisfy the access, alignment, lifetime, and backing
requirements as well. Insufficient supply or failure to establish it follows
the existing admission, installation, or activation-failure route; it is not a
new Omega machine `Trap`. Ordinary nested calls within that admitted activation
consume the already-composed bound, not a newly introduced runtime exhaustion
mechanism.

Within the established contract, validated spill loads and stores are
non-faulting in the language model. Target-required setup or probing is not
prohibited: it belongs to establishing and realizing the admitted stack
contract, and any live stack overhead contributes to the bound. This decision
requires no per-spill source crash guard. It does not remove physical
store/reload provenance, bounds, offset, alignment, lifetime, or frame replay.
A wrong generated address or invalid lifetime remains a compiler defect, not
a permitted resource-exhaustion outcome.

WCSU must be checked against the exact final physical realization whose stack
it provisions. The retained relation covers the selected optimization and
allocation result, final frames, target/calling rules, and installed artifact;
matching Terminal semantics alone is insufficient. Reusing demand from another
realization without revalidation rejects. Existing independently checked,
transitive identity relations may establish this connection; a parallel hash
field is not required merely to restate it.

This is an integration requirement, not a second stack planner or failure
model. Existing abstract spill requirements and WCSU composition machinery do
not by themselves establish complete physical realization or runtime backing.
The remaining work is tracked under spill realization and frame layout on
[the optimizer execution board](../../TASKS_OPTIMIZER.md).

## A Crash Contract Does Not Prove Recovery

Crash checking proves route coverage, propagation, and absence after successful
refinement. It does not infer a complete post-crash state. The ownership checker
can record the claims, guards, invariant windows, and other obligations known to
be live at a crash site. That record is a necessary lower bound on the damage
and valuable audit material; it is not proof that everything outside the record
remains valid.

The distinction matters for effects that escape the ownership model. A device
may be left halfway through a programming sequence, foreign storage may be
partially updated, and an external peer may have observed only part of a
protocol. No enumeration of locally held claims establishes that none of those
effects occurred.

The default consequence of an uncontained crash is termination of the complete
execution domain. Omega makes no ambient promise that another activation may
continue, that a lock is merely poisoned, or that the faulting activation can
resume.

Fault-tolerant continuation requires independent structure:

- a closed-custody component owns all mutable state that its failure can
  invalidate and shares none of it with survivors;
- a specific shared resource exposes an explicit owner-death outcome and a
  checked recovery protocol; or
- an external device or protocol supplies its own reset, reconciliation, or
  transactional guarantee.

The target must separately realize any component-isolation and restart plan.
Restart creates a fresh activation from established state; it never resumes the
abandoned computation. These are component, resource, and installation
properties, not meanings attached to an ordinary `crashes` clause.

## Crash Terminals Do Not Unwind

Cleanup happens along known graph edges. A recoverable-failure edge is an
**ordinary transition edge** with an ordinary per-edge drop set. A crash is a
distinct no-successor terminator: it performs no cleanup and carries an
explicit abandonment plan. Absence of a cleanup list does not encode
abandonment, because the verifier must distinguish deliberate abandonment from
compiler failure to compute an edge.

The statically known local frontier recorded at a crash site is only a lower
bound on what is abandoned. Caller frames, suspended continuations, external
effects, and other live activations need not be syntactically dominated by the
site, so the exact dynamic set is not claimed to be edge-enumerable. The record
supports auditing and diagnostics; it never licenses survivors.

If unwinding is ever added, it must be modelled as explicit graph edges with
cleanup and proof obligations, never as a second control-flow system.

An independently verified component supervisor may terminate the failed
component and begin a fresh frontier. It cannot resume the abandoned
activation; resumable faults require a different explicit protocol. Component
replacement likewise uses cooperative drain, coexistence, or migration rather
than asynchronous destruction hidden from the checked graph.

## Host Failure

Host calls and syscalls declare how they fail, and they fail the same way
everything else does — by **returning a sum**. Result-by-out-parameter is
rejected as a surface form (it is at most an invisible ABI lowering detail).

```omega
boundary machine HostFile::read(
    handle: HostHandle,
    out: &mut Buffer
) -> ReadOutcome;

data ReadOutcome {
    case Read(bytes: u64);
    case Closed;
    case Failed(error: IOError);
}
```

The boundary contract decides whether a given failure is data (a case in the
returned sum), a blocking wait, or a declared non-returning outcome. Host
boundaries must document whether resources remain valid after a failure case.
The selected provider and calling plan realize the ABI. The retired trailing
`boundary host` / `boundary Name` clause contributes no contract surface.

## Cancellation

Cancellation (chapter 18) rides this same channel: it is a
zero-case value delivered in a task's mailbox sum and handled by an ordinary
transition. There is no unwinding and no special cancellation control flow — a
cancelled task observes a case and transitions to its own cleanup-and-exit
states, like any other recoverable failure.
