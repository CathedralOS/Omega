# Chapter 3: Machines

A machine takes explicit inputs and performs work. Its final expression supplies
the result:

```omega
machine double(value: u32) -> u32
requires value <= 1000
{
    value * 2
}

let answer: u32 = double(21); // 42
```

The precondition makes multiplication safe for every accepted input. Callers
must prove it before calling. Machines can also update borrowed data, transfer
control to another machine, or keep processing events indefinitely.

Put related machines in modules and pass their data explicitly. A qualified
machine name selects a module, trait requirement or named implementation;
data types do not supply an implicit receiver.

## Header punctuation

Machine clauses do not end with semicolons. The next clause or the body ends
the current clause:

```omega
machine identity(value: u64) -> u64
requires value <= 100
ensures result == value
terminates
{
    value
}
```

A bodyless declaration ends with one semicolon after the complete header, as in
`machine identity(value: u64) -> u64 terminates;`. The same applies to a nested
`requires machine`: its final semicolon closes that required callable, and the
next clause belongs to the enclosing machine. `terminates`, `suspends`, and
`blocks` have no special punctuation. See the
[header delimiter rule](../spec/language/state_contracts.md#machine-header-delimiters).

## Operational inference and public contracts

A machine is private unless marked `public`. Private checked bodies may infer
suspension and blocking from their calls. On public surfaces, omitting `suspends`
or `blocks` forbids the corresponding behavior; a public generic declares the
ceilings permitted by its originating callback requirements.

A checked generic also publishes the dependencies of suspension/blocking on its
static callbacks. Callers substitute the selected contracts into that summary.
The generic body acknowledges the callback's upper bound, while callers
acknowledge their specialized envelope. See the complete
[public callback example](chapter_19_capabilities_effects_boundaries.md#suspension-and-blocking-through-callbacks).

A wrapper can [inherit an exact forwarded callback requirement](chapter_13_generics.md#forwarding-a-callback-contract)
instead of repeating its clause, whether public or private. This publishes a
checked requirement; it does not guess a trait from a direct call's shape.

## Use machines in different contexts

The same machine declaration can serve a runtime call, permitted compile-time
evaluation, a proof citation, a task entry or a trait requirement. Its contract
determines which uses are legal.

An operator is a machine with a fixed token, such as
`machine + vector::add(...) { ... }`. [Expressions](chapter_5_expressions_evaluation.md#operators)
explains selection. [Concurrency](chapter_18_concurrency.md) shows task creation,
and [proofs](chapter_10_compile_time_proofs.md) shows theorem citations.

## Operations with explicit data

An operation names its data input explicitly, just like any other input.
Borrowing and ownership come from that parameter's type; the type does not own
the machine.

```omega
data Player {
    health: i32;
}

machine take_damage(
    player: &mut Player,
    amount: i32
)
requires amount >= 0
requires player.health >= amount
{
    player.health = player.health - amount;
}
```

Call it as `take_damage(&mut player, amount)`. Neither
`machine Player::take_damage` nor `player.take_damage(amount)` is a supported
code-to-data binding. Put related machines in an ordinary module when a
namespace is useful.

## Free-standing machines

Free-standing machines use an unqualified declaration name. Their parameter
rules are identical to qualified machines.

```omega
machine add_i32(
    left: i32,
    right: i32
) -> i32
requires left in 0..=1000
requires right in 0..=1000
{
    left + right
}
```

Free and module-scoped machines are equally suitable for math, proof helpers
and operations on one or several data types. No data type is a code owner.

## Program entry

`build.omg` binds a target-owned program-entry slot to one exact machine. The
binding selects what the target bridge will call; it is not itself a call and
does not pass arguments or allocate values. A free entry machine has no
implicit state and no machine name allocates data:

```omega
machine start() {
    Console::write_line("Hello, Omega.");
}
```

Create application state explicitly and pass it to other machines. Temporary
work belongs in ordinary local bindings, not fields added to the application's
persistent data merely to make them reachable:

```omega
data Application {
    total: i32;
}

machine run(application: &mut Application) {
    let increment: i32 = add_i32(3, 4);
    application.total = increment;
}

machine start() {
    let mut application: Application;
    run(&mut application);
}
```

The build binds the target's `ProgramEntry` slot to `start`.

`Application` remains an ordinary nominal type. This example uses its ordinary
zero initialization; a type whose invariant does not hold at zero needs an
explicit construction establishing that invariant. The local's storage and
cleanup follow ordinary machine rules. There is no implicit program-lifetime
instance created from a machine's qualified name.

The target's entry contract determines which parameters the launch adapter
supplies. Hosted programs usually have no physical arrival arguments;
freestanding targets may expose validated image or storage extents. The adapter
must establish backing and authority before the entry uses them.

See [program entries](../spec/build/entry_roots.md) for platform return values,
entry arguments and storage planning.

## Parameters and returns

Machine parameters are entry data. A machine return type is the value shape its
body or control-flow graph produces if it reaches a returned terminal
outcome. The type alone is not a termination guarantee.

```omega
machine resolve_command(
    parser: &Parser,
    line: &[u8]
) -> Command {
    Command::Invalid
}
```

Every reachable terminal path in a typed machine must produce a compatible
return value.

Machine code always uses a brace body. There is no `machine f(...) = expr;`
form; a one-expression machine simply returns its final expression:

```omega
machine in_span(g: Game) -> bool {
    g.turn in 1..=9
}
```

### Result-domain overloads

A named machine or requirement may reuse one path and parameter signature when
each declaration returns a different set of dispatch-bearing domains. This is
compile-time overload selection over erased qualification, not runtime return
type inspection:

```omega
boundary operator I32::from_f64(value: f64) -> i32
requires
    value == value && value > -2147483649.0 && value < 2147483648.0;

boundary operator I32::from_f64(value: f64) -> i32 in Trapping;
boundary operator I32::from_f64(value: f64) -> i32 in Saturating;
```

The expected result type supplies the requested dispatch set. With no usable
expected type, the requested set is empty, so the unqualified overload is the
default. Resolution requires set equality: neither weakening nor a partial
semantic match participates. A caller asking for `i32 in Saturating & Km`
therefore needs an overload returning both selections or must compose two
explicit operations.

A domain's theory determines whether it participates in dispatch. Predicate-only
refinements are checked after selection; they do not choose another overload.
Two declarations differing only by such predicates are therefore duplicates.

Fixed operators remain operand-directed: an expected return type cannot change
the meaning of `+` or `/`. This rule concerns explicit named calls. See
[domain selection](../spec/language/domains.md) for classification and identity.

## Requirements and external implementations

A machine with a body supplies checked Omega code. A bodyless machine inside
a trait declares a requirement for an implementation to satisfy:

```omega
public boundary trait InterruptCompletion {
    machine complete(acknowledgement: InterruptAcknowledgement)
    reaches PortIo
    requires acknowledgement in InterruptAcknowledgement::Pending
    crashes none;
}
```

This interface sketch assumes the acknowledgement and PortIo declarations.
A checked implementation proves the contract; a foreign implementation needs
explicit admission. Package Build binds irreducible native operations to their
target locators. Compiler primitives use sealed declarations.

Composite work, such as obtaining a console handle and writing a line, belongs
in a checked adapter. [Boundaries](chapter_19_capabilities_effects_boundaries.md)
shows selection, authority and foreign supply. The [machine reference](../spec/language/machines.md#supply)
also covers admission-bearing proof claims.

## Calls

Ordinary call syntax invokes a machine contract and resumes its continuation on
normal return. A machine may be nested for lexical visibility; neither its name
nor nesting prescribes a physical call frame.

```omega
let command: Command = resolve_command(&parser, &line);
let guard: Guard = block Mutex::lock(&mut mutex);
let event: Event = suspend Inbox::take(&mut inbox);
```

An ordinary call has a continuation. A transition arm instead returns a value
or invokes a machine and forwards its eventual result. Chapter 4
introduces these arm forms. `suspend` and `block` acknowledge possible waiting at a
direct call; they do not create another machine species or change the result
type. Chapter 5 defines their call-position rules, and chapter 18 explains the
concurrency consequences.

## Termination and ranked cycles

`ensures` describes a return if one occurs. `terminates` additionally promises
that a terminal outcome is eventually reached under the machine's progress
premises. A private checked acyclic body derives a local termination summary;
an exported or required promise is explicit or inherited from its requirement.

A terminating cycle needs an authored ranking witness:

```omega
machine countdown(n: u64)
terminates by n -> Nat::Descending
{
    transition n {
        0 -> {}
        _ -> countdown(n - 1)
    }
}
```

The recursive call is the arm's final operation. The rank decreases toward its
floor; arithmetic safety remains a separate proof or crash-contract obligation.

Two questions must stay separate:

- **Does it terminate?** A well-founded ranking proves that a cycle cannot
  continue forever. Productive transition loops without a termination promise
  may run forever and owe no ranking.
- **Can runtime code execute it without growing the stack?** Runtime recursive
  calls must be tail calls. They lower to iteration with no accumulating frames.
    `1 + recursive_call(...)` is not tail: multiplication remains after the return.
  Non-tail runtime recursion rejects; use explicit work storage when an
  algorithm needs pending work.

Proof and compile-time evaluation permit well-founded non-tail recursion.
Those contexts use the same machine and termination contract, not a second
language. A recursive proof citation still needs strict descent on that exact
edge before importing the callee's guarantee as an induction hypothesis.

### Ranking views

`terminates by subject -> View` selects how progress is measured. Descending
naturals, a bounded increasing cursor, proper subtrees, and lexicographic
products can all supply well-founded views. An optional `in lo..=hi` bounds the
rank; it allocates no storage.

Mutually recursive machines share a joint ranking. Every complete cycle must
decrease, and every runtime recursive edge must be tail. A valid private ranking
witness may change without changing the public termination promise.
The [termination specification](../spec/language/termination.md) owns exact
edge checks, progress-profile premises, and the remaining mutual-call syntax
question.

### Borrowed subplaces

For `scan_parser(&mut compiler.parser, ...)`, the ordinary call borrows the
parser field. Inside `scan`, the explicit parameter names the same referent
across backedges and return. Writes affect the original field; conflicting
parent access is unavailable while the loan is live. A changing traversal
cursor can instead be an ordinary loop-carried reference parameter.

### Stack demand

Once recursive cycles lower to backedges, ordinary runtime calls form an acyclic
graph. Build-time WCSU composition accounts for maximum live frames, final spill
storage, and admitted external/provider demands before activation. Register
allocation may change that resource bound, not the machine's `crashes` contract.
See [compiler-owned stack storage](chapter_16_errors_traps_failure.md#provision-stack-storage-before-execution).

## Return a guarantee

```omega
machine double(value: u32) -> u32
requires value <= 100
ensures result <= 200
{
    value * 2
}
```

The caller proves the requirement. The body proves the guarantee, which callers
can use after the return. [Chapter 7](chapter_7_types_constraints_invariants.md)
shows how branches and calls carry these facts; [Chapter 16](chapter_16_errors_traps_failure.md)
shows guarantees for particular result cases and failure paths.

## Continue with control flow

Use [nested machines and transitions](chapter_4_nested_machines_transitions.md)
to split work into explicit steps. Each receiver gets its declared parameters
and must receive the facts its contract requires.
