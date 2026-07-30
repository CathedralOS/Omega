# Chapter 3: Machines

A machine is a **named, contracted transition system**. Given its inputs,
state, and authority, it produces a contract-observable trace and may produce
a terminal outcome. A productive machine may run forever, so an ordinary
function-like call is one important use of a machine, not its definition.

> **Machine taxonomy settled 2026-07-18.** Runtime calls, compile-time
> evaluation, proof citation, concurrent activation, trait satisfaction, and
> boundary provision consume the same semantic construct. Checked bodies, requirements,
> external providers, and accepted trust declarations are supply modes, not
> separate machine species. See
> [machine_taxonomy.md](../design_briefs/machine_taxonomy.md).

Machines may be attached to data, or free-standing when there is no natural
owning data type.

## One Construct, Several Uses

The same machine can be called at runtime and evaluated by the compiler when
its contract, reach, and totality make that evaluation legal. It can also be
cited as proof, started through a task runtime, or used to satisfy a
trait/boundary requirement.
Those contexts change eligibility and lowering; they do not create parallel
`async`, `proof`, or `const` machine identities.

A machine's substitutable contract is wider than its input/output relation. It
also includes failure and cancellation; service reach; possible suspension and
blocking; required authority; positive progress guarantees;
atomicity and reentrancy; context-visible resource bounds; and a boundary
calling plan where applicable. Provider substitution must refine the whole
contract.

Internal compiler/runtime transitions may be hidden only after projection
through the declared observation surface and only above the floor imposed by
the caller. A machine cannot hide blocking, authority, failure, or another
context-forbidden behavior merely by calling it unobservable.

## Attached Machines

Attached machines operate on a named data type.

```omega
data Player {
    health: i32;
}

machine Player::take_damage(
    &mut self,
    amount: i32
) {
    self.health = self.health - amount;
}
```

`self` is explicit. If the machine mutates the receiver, it takes `&mut self`.

## Free-Standing Machines

Free-standing machines are ordinary machines without a data receiver.

```omega
machine add_i32(
    left: i32,
    right: i32
) -> i32 {
    left + right
}
```

Use a free-standing machine for math helpers, proof helpers, and operations
that are not naturally owned by one data type.

## Program Entry

Executable programs should use an explicit root data type.

```omega
data Main {
    total: i32;
}

machine Main::main(&mut self) -> i32 {
    self.total = add_i32(3, 4);
    self.total
}
```

The process entry is the `main` machine on the `Main` root data object. Startup
allocates the root object, then enters `Main::main(&mut root)`.

This keeps process-owned state under one explicit owner.

## Parameters And Returns

Machine parameters are entry data. A machine return type is the value shape its
body or internal state graph produces if it reaches a returned terminal
outcome. The type alone is not a termination guarantee.

```omega
machine Parser::resolve(
    &self,
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

## Supply Forms

The one machine construct has four explicit supply forms:

| Supply | Spelling |
|---|---|
| Checked Omega implementation | `{ ... }` body |
| Trait requirement | Bodyless declaration inside the trait |
| External realization | `satisfies Requirement via <Binding>;` |
| Accepted claim | Bodyless `boundary machine ... ensures ...;` |

An external realization binds an irreducible imported operation to a
requirement without pretending the binding is executable Omega code:

```omega
machine Kernel32::write_file(handle: WinHandle, bytes: &[u8]) -> WriteResult
    satisfies Kernel32Requirements::write_file
    via Binding::DllImport {
        library: kernel32_lib,
        symbol: write_file_symbol,
        plan: MsX64,
    };
```

The value after `via` must be compile-time evaluable to the closed `Binding`
vocabulary. The compiler normalizes and validates it, derives the provider
plan from explicit conformances, and assigns any trust expenditure only when
the provider is admitted. `satisfies` supplies the requirement contract and
public service/suspension/blocking ceilings; the binding/provider behavior must
refine each one. A `via` machine does not repeat those clauses.

Composite adaptation is ordinary checked code. For example, an implementation
of `Console::write_line` may call separately bound `get_stdout` and
`write_file` machines, cache a handle, or merge writes. Those decisions belong
in its brace body rather than a call-shape DSL or authored plan row.

## Calls

Ordinary call syntax enters a machine and creates a call frame.

```omega
let command: Command = self.parser.resolve(&self.line);
let guard: Guard = block mutex.lock();
let event: Event = suspend inbox.take();
```

Calls and transitions are different. A call enters another machine. A transition
jumps to a state inside the current machine. Chapter 4 introduces states and
transitions directly. `suspend` and `block` acknowledge possible waiting at a
direct call; they do not create another machine species or change the result
type. Chapter 5 defines their call-position rules, and chapter 18 explains the
concurrency consequences.

## Termination And Ranked Cycles

A machine may promise termination with `terminates`: every invocation reaches
a terminal outcome under its declared progress premises. Checked acyclic bodies
derive that guarantee without annotation. Every cycle in a terminating machine
instead needs an authored, checker-verified ranking witness written with
`terminates by` (chapter 9):

A machine may call itself, directly or through a mutual cycle, when every
cycle through the call graph strictly decreases a well-founded rank and
each recursive call is the last thing its arm does:

```omega
machine Gauss::sum(n: u64, acc: u64) -> u64
terminates by n -> Nat::Descending;
{
    transition n {
        0 -> acc
        _ -> Gauss::sum(n - 1, acc + n)   // tail: the call IS the arm's result
    }
}
```

The same rule covers explicit state/transition loops and recursive call
cycles. A productive machine may deliberately run forever; a loop that makes
no termination promise owes no ranking.

Working rules:

- **Legality is the ranking, not the position.** Every terminating runtime
  cycle needs both: `terminates by` proves progress; tail position gives a
  recursive call cycle a lowering.
  Spelling encodes intent: a transition arrow says "process — may run
  forever, constant space, no proof owed"; a recursive call says
  "terminating walk — measured, or it does not compile."
- **Every runtime cycle lowers to the loop machinery.** A tail recursive
  call compiles to the same back-edge a transition loop-back uses: zero
  stack growth, no frame accumulation, ever. Classification is strict and
  never silent: `-> 3 * Gauss::sum(...)` is not tail (the multiply runs
  after the call returns), and the error names why.
- **Non-tail recursion does not compile in runtime code.** A measured cycle
  whose call returns into more work (`1 + max(depth(l), depth(r))`) is
  rejected with the classification error. Depth belongs in data: iterate
  with explicit storage the machine declares and sizes — a fixed-capacity
  field today, an Arena-backed Allocation when the allocator arc lands. Activation frames are
  storage the author never sees or sizes; depth does not hide there.
- **The range is a termination fact, never a size.** `terminates by cursor ->
  Cursor::TowardStart in lo..=hi` constrains the rank produced by the view;
  the floor is the well-foundedness
  bound (any start, not only zero, so a cursor walking `hi` down to `lo`
  needs no re-zeroed distance measure). Dependent endpoints are legal —
  pinned witnesses, re-proven at every back-edge, the same fact the loop
  spelling declares as a parameter range. Nothing is ever allocated from a
  range.
- **Lexicographic measures compose freely.** The measure gates legality and
  sizes nothing, so dictionary orders need no special case; bounded
  components may still flatten to a single linear measure (`m*B + n`).
- **Mutual cycles share a joint ranking** (lexicographic when needed); every
  cycle through the call graph must decrease it, and at runtime every call
  along the cycle must be tail.
- **The admitted artifact's worst-case stack is a static constant.** After
  lowering, its runtime call graph is acyclic, so the maximum live activation
  storage along any call chain is computable at build time. External roots and
  opaque providers remain responsible for their pinned stack domains; their
  declared nesting and same-stack demands compose through the external-root
  ledger. The resulting bound appears in the layout report.

Proof-stratum machines (chapter 10) use the same clause and legality rule with
no tail restriction: non-tail shapes — `1 + max(Tree::depth(node.left),
Tree::depth(node.right))`, induction over a tree — are legal there, because
fact-only machines evaluate in the compiler's hermetic semantic evaluator and
never lower. Their ordinary termination proof is mandatory; deterministic work
metering supports progress, warnings, and optional root policy without creating
a second notion of termination. No runtime frame ever materializes.

The ranking witness is implementation evidence, not public contract identity.
Changing a valid witness revalidates the implementation without changing what
callers or external requirement bindings see. See
[termination_ranking_and_progress.md](../design_briefs/termination_ranking_and_progress.md).

## Contracts

Machines may declare requirements and guarantees.

```omega
machine Player::enter_combat(&mut self)
requires
    self in Player::Alive
ensures
    self in Player::InCombat
{
}
```

The caller must satisfy `requires`. The machine body must establish `ensures`.

## Machine Graph Compatibility

Internal states participate in the machine's graph, but they are not public
machine entries.

Working rules:

- State-transition arguments must match the target state's parameters.
- Terminal values must satisfy the active machine's return type.
- Every reachable terminal path in a typed machine graph must produce the
  declared return type.
- Transition dispatch arms add proof assumptions for the target edge.

> **Implementation gate:** the current Rust trees do not yet carry the
> normalized complete machine contract or explicit supply mode. See
> [semantic_taxonomy_representation.md](../architecture/semantic_taxonomy_representation.md).
