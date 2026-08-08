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
| The result cannot be ignored | **Strict-use (decision 9):** discarding a non-unit return is a compile error; intentional discard is `_ = call();`. |
| Every failure case must be handled | **Exhaustive transitions** over the sum's cases. |
| The success payload is unreachable on a failure path | **Case-payload binding:** `value` exists only inside the `Parsed` arm. |

So the error model is not a feature. It is what falls out of sums + exhaustive
transitions + strict-use. A library `Result<T, E>` is *permitted* but buys
nothing special; most code should use a domain-specific sum whose cases name the
actual failures.

## Success Cases Carry Proven Facts (The Fact Catalog)

A transition over a sum is a **partition**: each arm learns which case holds, and
therefore that case's facts. This is the same flow-sensitive narrowing the
arithmetic domains use for scalar guards (chapter 7 / decision 17) — one unified
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
- a **failed exact-arithmetic proof** is a compile error (decision 17: exact
  arithmetic is a proof obligation);
- a **provable contract violation** is a compile error.

When the prover *cannot* discharge an obligation, that does not silently become a
runtime trap. The obligation must be **handled** (an exhaustive transition over
the failure sum) or **deliberately abandoned** (the opt-in abort control outcome below). Prover
incompleteness produces a required handler or an explicit opt-in — never a hidden
death.

The one in-language failure that legitimately reaches runtime is **opt-in
`Trapping` arithmetic** (decision 17): `T::Trapping` emits a hardware trap on
overflow. It is explicit in the type, chosen deliberately, and already
implemented. That is the model — not a default.

There is no `expect`/`unwrap`. "I know this cannot fail here" is spelled by
*proving* the failure case impossible (the provably-dead arm discharges
exhaustiveness with no handler), not by asserting it at runtime.

## Crashes Are Explicit, Guarded Control Ceilings

`crashes` publishes the ways an invocation may leave without returning or
running cleanup. The initial causes are `Trap` and `Abort`. `Trap` covers an
operation- or platform-triggered fault; `Abort` is deliberate execution-domain
termination. One clause names one cause and one abstract containment demand:

```omega
machine divide(numerator: i32, denominator: i32) -> i32
crashes Trap Activation
    denominator == 0
    numerator == i32::Minimum && denominator == -1
{
    // ...
}
```

The indented facts are alternative **routes**: any one permits the named crash.
They are not the conjunctive list used by `requires`. Repeating a clause for the
same cause and scope contributes more alternatives to the same canonical
bucket. A route predicate is an ordinary Boolean proof expression and may use
`&&`, `||`, or other ordinary operators internally.

One cause appears per clause. The formatter renders one route per line and
starts routes below the cause and scope. A clause with no routes is
unconditional. Omitting the scope uses the stable portable top:

```omega
crashes Trap
```

is equivalent to an unconditional `crashes Trap ExecutionDomain`. Omitting a
cause entirely is the negative guarantee that the machine cannot crash for
that cause. Private checked machines may infer crash ceilings; exports,
requirements, and boundaries publish them.

`crash Abort;` and `crash Trap;` are explicit no-return terminals. Operations
with intrinsic crash behavior, such as `Trapping` arithmetic, contribute crash
sites and guards without requiring a source terminal statement. Every checked
site must be covered by the published routes:

```text
derived_site_guard
    implies OR(published_route_guards for the same cause that cover its scope)
```

The derived guard includes the path condition. A trap-capable division inside
`if x > 0` therefore contributes `x > 0 && denominator == 0`, not merely the
primitive's local guard.

At a call, arguments and current facts refine the published routes. A cause is
removed only after every surviving route for that cause is disproved. This is
why `divide(10, 2)` is crash-free at that invocation even though `divide` is
published as trap-capable. Refinement may also remove only the broadly damaging
routes, allowing a call to fit a narrower containment context without proving
all crashes impossible.

The explicit terminal outcome and service reach remain separate axes. An abort
lowering may also reach the `ProcessExit` boundary service; neither fact implies
the other. Graceful shutdown remains ordinary cleanup followed by `exit(code)`.

## Containment Is A Two-Sided Contract

Crash scopes are portable nominal tokens ordered by the breadth of execution
they terminate. `ExecutionDomain` is the permanent portable top: the root of
the execution owned by this artifact. Its physical realization is
target-relative—a hosted process, a Cathedral Matrix, or a bare-metal image and
its grants. New stable scopes may be inserted below that top without changing
the meaning of existing artifacts. The declaration and cross-package ordering
form for those intermediate scopes is not yet settled; see
`OWNER_QUESTIONS.md` Q2.

Each route publishes how much containment it may demand. Each enclosing
execution context publishes, separately for each cause, the widest scope it
tolerates. The map is owned by the activation, task, supervisor, or root that
expects state to survive; ordinary leaf machines inherit it rather than
repeating it. Absence from that sparse per-cause map means forbidden. After
call-site refinement, Psi compares every surviving bucket independently; it
does not require a join of incomparable scopes:

```text
derived damage minimum
    <= published route demand
    <= context maximum[cause]
```

The lower bound matters. If a crash occurs while a shared invariant is open or
while the activation owns custody needed by survivors, killing only that
activation may expose broken state. Such a site derives a wider minimum. In a
context that expects activation-level survival, the offending wide-scope route
must be disproved. A resource-specific owner-death protocol can reduce that
minimum only when reacquisition returns a checked outcome such as
`OwnerDied(recovery_custody)` and successful recovery re-establishes the
resource invariant; there is no ambient poisoning mechanism.

Psi proves this internal demand-versus-tolerance relation using only nominal
scopes. Omega installation chooses a fault-containment plan and checks the
second side:

```text
published route demand
    <= realized target scope
    <= context maximum[cause]
```

A realized scope that is too narrow leaves corrupted state visible to
survivors. A realized scope that is too broad destroys state the context
promised would survive. The portable fingerprint contains the authored routes,
scope demands, and context maxima; the installation record contains the
selected plan, realized scopes, and supporting evidence.

## Crash Terminals Do Not Unwind

Cleanup happens along known graph edges. A recoverable-failure edge is an
**ordinary transition edge** with an ordinary per-edge drop set. A crash is a
distinct no-successor terminator: it performs no cleanup and carries an
explicit abandonment plan. Absence of a cleanup list does not encode
abandonment, because the verifier must distinguish deliberate abandonment from
compiler failure to compute an edge.

The statically known local frontier recorded at a crash site is only a lower
bound on what is abandoned. A trap abandons at least the faulting activation,
including caller frames; an abort abandons the execution domain. Suspended
continuations and other live activations need not be syntactically dominated by
the crash site, so the exact dynamic set is not claimed to be edge-enumerable.

If unwinding is ever added, it must be modelled as explicit graph edges with
cleanup and proof obligations, never as a second control-flow system.

A fault handler may terminate the faulting activation or begin a fresh
frontier. It cannot resume the abandoned activation; resumable faults require a
different explicit protocol. Component replacement likewise uses cooperative
drain, coexistence, or migration rather than asynchronous destruction hidden
from the checked graph.

## Host Failure

Host calls and syscalls declare how they fail, and they fail the same way
everything else does — by **returning a sum**. Result-by-out-parameter is
rejected as a surface form (it is at most an invisible ABI lowering detail).

```omega
machine HostFile::read(handle: HostHandle, out: &mut Buffer) -> ReadOutcome
boundary host
{ }

data ReadOutcome {
    case Read(bytes: u64);
    case Closed;
    case Failed(error: IOError);
}
```

The boundary contract decides whether a given failure is data (a case in the
returned sum), a blocking wait, or a declared non-returning outcome. Host
boundaries must document whether resources remain valid after a failure case.

## Cancellation

Cancellation (chapter 18 / decision 16) rides this same channel: it is a
zero-case value delivered in a task's mailbox sum and handled by an ordinary
transition. There is no unwinding and no special cancellation control flow — a
cancelled task observes a case and transitions to its own cleanup-and-exit
states, like any other recoverable failure.
