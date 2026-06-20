# Chapter 15: Errors, Traps, And Failure

Failure semantics must be explicit. Omega has no hidden exceptions, no second
control-flow system, and no ambient panic. Recoverable failure is **data**;
deliberate termination is an **opt-in effect**; everything else that other
languages call a "trap" is, in a proof-oriented language, a **compile error**.

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
the failure sum) or **deliberately abandoned** (the `abort` effect below). Prover
incompleteness produces a required handler or an explicit opt-in — never a hidden
death.

The one in-language failure that legitimately reaches runtime is **opt-in
`Trapping` arithmetic** (decision 17): `T in Trapping` emits a hardware trap on
overflow. It is explicit in the type, chosen deliberately, and already
implemented. That is the model — not a default.

There is no `expect`/`unwrap`. "I know this cannot fail here" is spelled by
*proving* the failure case impossible (the provably-dead arm discharges
exhaustiveness with no handler), not by asserting it at runtime.

## Deliberate Termination Is The `abort` Effect

A program that genuinely must die rather than recover opts into an `abort`
effect. `abort` is:

- **contagious** — `main` declares it, and it propagates to every caller; a
  boundary fronting something abortable must itself declare it;
- **visible** — it appears in signatures and effect sets, so a package policy can
  refuse any dependency that carries it;
- **nuclear** — it runs no cleanup and no unwinding; it lowers directly to an
  `exit`/`abort` boundary call. Giving up does not tidy up.

The contagion is the deterrent: aborting is annoying to opt into by design,
reserved for services whose owner restarts them. It is not for ordinary error
handling.

The contagion and visibility are **not new machinery** — they are the chapter 18
effect system. Aborting carries the `process_exit` effect, which is inferred
bodies-up through the call graph (no per-call-site pollution), forced to be
declared at boundaries (boundary traits, exported APIs, `main`), and recorded in
the executable manifest for a build/store/OS policy to audit or deny. So "a
caller of an abortable machine is itself abortable unless it proves the abort
unreachable" is just ordinary effect propagation; abort adds the *nuclear,
no-cleanup operation*, not a new propagation rule. (`exit(code)` below carries
the same `process_exit` effect — the difference is cleanup, not the effect.)

**Graceful shutdown is not `abort`.** Releasing resources and exiting cleanly is
ordinary control flow: transition to a cleanup state, run its effects, then call
`exit(code)` (a normal host boundary). Only the no-cleanup, give-up case is the
`abort` effect.

## No Hidden Unwind

Cleanup happens along known graph edges. A failure edge is an **ordinary
transition edge** with an ordinary per-edge drop set: the locals not moved into
the target and still owned are dropped crossing that edge, exactly as on a
success edge. There is no separate failure-cleanup mechanism and no invisible
unwinder. (The drop model itself is chapter 16; ch15 commits only to "the failure
edge is not special.")

If unwinding is ever added, it must be modelled as explicit graph edges with
cleanup and proof obligations, never as a second control-flow system.

## Host Failure

Host calls and syscalls declare how they fail, and they fail the same way
everything else does — by **returning a sum**. Result-by-out-parameter is
rejected as a surface form (it is at most an invisible ABI lowering detail).

```omega
machine HostFile::read(handle: HostHandle, out: &mut Buffer) -> ReadOutcome
boundary host
{ }

data ReadOutcome {
    case Read(bytes: usize);
    case Closed;
    case Failed(error: IOError);
}
```

The boundary contract decides whether a given failure is data (a case in the
returned sum), a blocking wait, or — at most — the `abort` effect. Host
boundaries must document whether resources remain valid after a failure case.

## Cancellation

Cancellation (chapter 17 / decision 16) rides this same channel: it is a
zero-case value delivered in a task's mailbox sum and handled by an ordinary
transition. There is no unwinding and no special cancellation control flow — a
cancelled task observes a case and transitions to its own cleanup-and-exit
states, like any other recoverable failure.
