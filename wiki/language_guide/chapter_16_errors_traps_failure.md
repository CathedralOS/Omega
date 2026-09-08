# Chapter 16: Errors, Traps, And Failure

Recoverable failure is data. A crash is an explicit no-successor outcome.
An unproved obligation is a compile error—not an implicit exception, error
result or permission to abort. [Operational contracts](../spec/language/effects.md)
keep failure, crashes, service reach, suspension and blocking separate.

## Recoverable Failure Is A Sum, Handled At A Transition Boundary

Use a sum whose cases name the actual outcomes:

```omega
data ParseResult {
    case Parsed(value: i32);
    case BadDigit(at: u32);
    case Empty;
}

machine App::run(&mut self, input: &[u8]) {
    let outcome = self.parser.parse(input);
    transition outcome {
        ParseResult::Parsed { value } -> self.use_value(value)
        ParseResult::BadDigit { at } -> self.report(at)
        ParseResult::Empty -> self.report_empty()
    }
}
```

The transition covers every possible case. `value` exists only in the success
arm. A library `Result<T, E>` uses these same rules; it has no privileged error
behavior.

A non-Unit call result cannot be silently ignored. `_ = call();` acknowledges
intentional discard, but still owes the result's legal ownership disposition.
It cannot discard a live linear protocol obligation. See
[strict result use](../spec/language/effects.md#recoverable-outcomes-and-strict-use).

## Success Cases Carry Proven Facts (The Fact Catalog)

Facts can travel with the successful payload. For example, a table with sixteen
entries can return a bounded index:

```omega
data Slot {
    case Found(index: u32 [0..16]);
    case Full;
}
```

Constructing `Found` must establish its index range. A `Found { index }` arm
then has that bound available for a sixteen-entry collection, subject to the
ordinary selected-index and borrow obligations. No unbound `Found.index`
expression is needed in a surrounding contract.

Guards and case partitions refine live facts. Across calls, the caller proves
`requires` and receives the callee's established `ensures`; writes invalidate
facts about changed storage. Private analysis may derive evidence, but cannot
invent caller-facing preconditions. See [state contracts](../spec/language/state_contracts.md).

## Propagation Is An Explicit Edge — There Is No `?`

Propagation is an arm returning or constructing the caller's failure outcome:

```omega
machine App::run2(&mut self, input: &[u8]) -> ParseResult {
    transition self.parser.parse(input) {
        ParseResult::Parsed { value } -> self.done(value)
        ParseResult::BadDigit { at } -> self.bubble_bad(at)
        ParseResult::Empty -> self.bubble_empty()
    }
    // The target states construct and return the corresponding ParseResult.
}
```

There is no `?` propagation operator or `fails` clause. The checked graph keeps
failure propagation and the required ownership transfers visible.

## There Is No "Trap" Category For Logic

A failed Exact arithmetic proof or contract obligation rejects. Prover
incompleteness does not produce a runtime handler automatically. An author may
choose a separately contracted failure-returning operation or explicit Trapping
arithmetic, then discharge that operation's obligations.

Trapping arithmetic is one runtime crash source, not the only possible platform
or operation fault. Proving a failure case impossible removes the need for an
executable handler; it does not insert an `expect` or `unwrap` check.

## Crashes Are Explicit, Guarded Control Ceilings

`crashes Trap` permits operation/platform faults under its stated routes;
`crashes Abort` permits deliberate execution-domain termination. For example,
these are alternative routes in a division contract:

```omega
crashes Trap
    denominator == 0
    numerator == i32::Minimum && denominator == -1
```

Any one route permits that cause. An empty route list is unconditional;
omitting the cause forbids it. The executable operation must still have the
selected behavior—writing a crash clause does not make Exact division trap.
`crash Trap;` and `crash Abort;` are explicit terminals.

Every derived crash site must be covered by the same-cause published routes.
At a call, actual arguments and live facts can disprove routes; only disproving
every route removes that cause. Route expressions must themselves be total,
so trapping arithmetic cannot execute inside a route to justify its own failure.
See [guarded crashes](../spec/language/effects.md#guarded-crashes).

Process-exit service reach is separate from an Abort outcome. Graceful shutdown
does ordinary cleanup before its selected exit operation.

## Compiler-owned stack storage and spill accesses

Register allocation may increase the artifact's stack demand, but does not add
`crashes Trap` to a source machine. Spill accesses use the activation's provisioned
frame storage:

1. Derive final physical frame demand, including spills and calling storage.
2. Compose it through call chains and external-entry contexts.
3. Establish sufficient aligned, accessible backing for the activation's lifetime.

A byte-count comparison is not a backing proof. Failed provisioning follows
admission, installation or activation failure, not a new source stack-overflow
outcome. Validated accesses are non-faulting under that established contract;
a wrong compiler-generated address remains a defect.

The [stack specification](../spec/resources/storage.md#compiler-owned-stack-accesses)
owns exact realization identity, slot reuse, probing, suspension lifetime and
external-root composition. Tail backedges add no recursive frames; termination
alone does not size a stack.

## A Crash Contract Does Not Prove Recovery

A crash frontier reports locally known live obligations, not all damaged state.
A device may be half-programmed or a peer may have observed part of a protocol.
Unlisted resources are not thereby safe survivors.

An uncontained crash terminates the execution domain. Continuing elsewhere needs
independent isolation, an explicit resource owner-death/recovery protocol, or an
external reset/reconciliation guarantee. Restart establishes a fresh activation;
it does not resume abandoned computation. See [recovery contracts](../spec/language/effects.md#recovery-and-execution-domains).

## Crash Terminals Do Not Unwind

Recoverable outcomes follow ordinary cleanup-bearing edges. Crashes have no
successor and do no cleanup; their explicit abandonment record is not an absent
cleanup list. A supervisor cannot turn that record into permission to resume
the abandoned activation. [Terminal outcomes](../spec/terminal-psi/calls_and_outcomes.md#crash)
retain this distinction through verification and execution.

## Host Failure

Host operations use the same outcome model:

```omega
data ReadOutcome {
    case Read(bytes: u64);
    case Closed;
    case Failed(error: IOError);
}
```

A host read requirement returns `ReadOutcome` and states which resources remain
valid for each case. Recoverable data, possible blocking and declared crashes
are independent contract choices. The selected provider/calling plan handles
any ABI out-parameters; they do not introduce a second source result mechanism.
[Foreign bindings](../spec/build/foreign_bindings.md) explains explicit supply.

## Cancellation

Cancellation is delivered through ordinary task protocol data and handled at a
checked safe point. It does not unwind or silently discharge the external task
claim. The task follows its own cleanup-and-exit states; callers complete its
ordinary lifecycle. See [tasks](../spec/build/task_runtime.md#suspension-and-cancellation).
