# Machines as runtime values

Named machines can be stored in ordinary data and selected at runtime. A callable
field publishes the complete contract its caller may rely on. Operations records
replace runtime trait selectors; state remains ordinary explicit arguments.

Ratified 2026-11-06 together with [some packages](some_packages.md). All `dyn`
source forms, including captured `dyn Trait<_>`, are retired. Static traits,
named conformances, proof laws and `machine F` generic binders remain. Anonymous bodies and capture syntax remain a separate proposal.

## Callable types and construction

A runtime callable type uses the structural signature and clauses already used
by a `requires machine` requirement, without a declaration name:

```omega
data Counter { value: u64; }
data CounterOps {
    clear: machine(counter: &mut Counter) terminates;
    value: machine(counter: &Counter) -> u64 terminates;
}

machine clear(counter: &mut Counter) terminates { counter.value = 0; }
machine value(counter: &Counter) -> u64 terminates { counter.value }

let operations = CounterOps { clear: clear, value: value };
let current: u64 = (operations.value)(&counter);
```

The same type form is usable for locals, parameters, results, fields, arrays and
other ordinary containers. Parameter names in its contract bind that callable's
formal arguments; `result` denotes its result. Lifetime binders and ordinary
`requires`, `ensures`, reach, invocation, suspension, blocking, crash, progress,
resource and custody contracts retain their meanings. Runtime callable service
reach has a fixed finite ceiling: omission and `reaches none` both mean empty.
The deferred omission rule for static requirements does not apply to callable
types. Other axes retain their existing omission rules.

In a callable-valued context, a named machine reference selects an exact closed
declaration application. A qualified named implementation member is eligible;
the conformance must be explicitly named, and the exact overload and all static
arguments resolved by ordinary selection. No search for a unique visible
implementation or implicit whole-trait conversion is introduced. Unrestricted
type-, machine- or evidence-polymorphic runtime callables are outside V1; close
their static arguments before constructing a value. Existing explicitly finite
value families retain their declared coverage, as described in
[Terminal dispatch](../terminal-psi/dynamic_dispatch.md#finite-generic-method-families).

Construction proves ordinary [contract refinement](machines.md#substitution):
the selected body must accept every input permitted by the callable requirement,
provide its guarantees, and stay within its operational/resource/custody bounds.
Substitute any static callback or conformance-member reach dependencies before
checking the closed selection against the destination callable's finite ceiling.
It does not run the body. Machine values contain code selection and retained
contract evidence, not an implicit state capture, receiver, allocation, raw
address or new source of authority. Ordinary initialization must establish a
callable; no null/default callable or unchecked integer-to-callable cast exists.

## Invocation, storage and evidence

Calling `f(args)` or `(record.operation)(args)` uses the value's current callable
contract and exact argument/result mapping. It proves current preconditions and
access obligations, retains permitted effects and progress, and supplies checked
postconditions and returned-borrow origins. Calls can take zero, one or multiple
state arguments. They are not method calls on the record containing the field.

Forwarding, reassignment, joins, fields and containers preserve the selected code
and its contract evidence. Known selection may narrow the operational envelope
only by [retained checked evidence](effects.md#known-dynamic-selections).
Unknown selections use the declared ceiling. Possible demands union at joins;
guarantees intersect. A reusable body cannot adopt one caller's stronger facts
or an optimizer's target prediction as an unconditional promise.

A code-only value is copyable where its complete qualifications and custody
allow. Copying it does not borrow or extend the lifetime of any state argument.
Code lifetime and replacement dependencies still apply. A record containing
mutable state references is governed by those references' ordinary multiplicity,
not by the copyability of its callable fields.

Operations records are ordinary records. Their fields are checked independently;
mixing compatible machines from different named implementations is permitted
unless a published record predicate or explicit evidence relates them. Field
completeness is not a proof of cross-operation laws, matching allocators, or a
shared implementation identity. Such relationships must be stated and proved.
Static trait laws do not appear automatically on construction of a record.

Callable addresses do not automatically supply executable equality, ordering,
hashing, or proof identity. Ordinary conformance synthesis needs its published
derivation; it cannot compare descriptors or hidden witness tags as a shortcut.

In proposition contexts, a call must independently satisfy the ordinary proof
admission rules. A stored callable's signature or absence of effects does not
establish totality, termination, mathematical identity or a rewrite law.

## Static selection remains distinct

`machine F` in a generic list selects a declaration statically under its required
contract. It remains eligible for specialization and direct calls. It is not a
runtime parameter and is not removed by this feature. Conversely, a runtime
callable does not become a static generic argument merely because the optimizer
predicts its target. Explicit generic instantiations can produce callable values
once their static arguments close. Their callable contracts use the checked
application's substituted [operational dependencies](effects.md#static-callback-operational-dependencies);
construction still proves refinement against the destination callable type.

Machine values can share consumer code across runtime choices. Generics can
specialize that consumer, and a program can combine both. Neither spelling
promises inlining, exact body counts or a specific indirect-call cost.

## Representation and component boundaries

The baseline is an indirect callable descriptor with a fixed contract and
calling shape. It has no semantic global roster of every assigned machine.
Adding an admissible target does not change the callable type or every holder's
layout. Native descriptor encoding is target-owned; it is not promised to be a
raw pointer, a particular word count, or immune to indirect-call mitigations.

Closed-world reachability, resource checking and devirtualization still apply.
An optimizer may prove a target set and use direct calls or finite dispatch, but
that set is optimization evidence, not part of source type identity. Retain the
same contract, argument correspondence, costs and code dependencies.

Local callable descriptors use within-artifact calling conventions. They do not
cross an independently replaceable component boundary as unchecked pointers.
Use boundary requirements with their `CallPlan`/`StatePlan`, and a local proxy
machine when a local callable interface is wanted. Code retention, outstanding
borrows and era custody follow [component publication](../build/component_publication.md).
An indirect representation removes roster-layout coupling; it does not make old
code or state safe to unload while values still depend on it.
