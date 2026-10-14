# Chapter 3: Machines

A machine is a **contracted transition system**. Given its
inputs, state, and authority, it produces a contract-observable trace and may produce
a terminal outcome. A productive machine may run forever, so an ordinary
function-like call is one important use of a machine, not its definition.

> **Machine taxonomy.** Runtime calls, compile-time
> evaluation, proof citation, concurrent activation, trait satisfaction, and
> boundary provision, and operator syntax consume the same semantic construct.
> Checked bodies, requirements, exact compiler primitives, external providers,
> and accepted trust declarations are supply modes, not
> separate machine species. See
> [machines and contract refinement](../spec/language/machines.md).

All machines are static: every input is an ordinary declared parameter. A
qualified path such as `Player::take_damage` gives the operation a namespace
and semantic owner; it does not supply a receiver. There is no special `self`
parameter or receiver-style dot invocation.

Contracts state logical conditions; ordinary machines establish their
conclusions. Traits and named conformances bundle mathematical operations,
witnesses, and laws. General logical expressions do not add an executable
machine supply mode; see [chapter 10](chapter_10_compile_time_proofs.md).

## One Construct, Several Uses

The same machine can be called at runtime and evaluated by the compiler when
its contract, reach, and totality make that evaluation legal. It can also be
cited as proof, started through a task runtime, or used to satisfy a
trait requirement, including a requirement in a boundary trait.

An optional fixed token binds operator notation to a named machine:
`machine + Vec2::add(...) { ... }`. Its body and contracts remain ordinary
machine code; [chapter 5](chapter_5_expressions_evaluation.md#operators) covers
selection, ownership, and the bodyless requirement/primitive cases.
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

## Data-Owned Operations

A data-owned operation names its data input explicitly, just like any other
input. Borrowing and ownership come from that parameter's type.

```omega
data Player {
    health: i32;
}

machine Player::take_damage(
    player: &mut Player,
    amount: i32
) {
    player.health = player.health - amount;
}
```

Call it as `Player::take_damage(&mut player, amount)`. The qualified name
does not insert an argument or permit `player.take_damage(amount)`.

## Free-Standing Machines

Free-standing machines use an unqualified declaration name. Their parameter
rules are identical to qualified machines.

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

`build.omg` binds a target-owned program-entry slot to one exact machine. The
binding selects what the target bridge will call; it is not itself a call and
does not pass arguments or allocate values. A free entry machine has no
implicit state, and naming a data-owned operation does not allocate that data:

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

machine Application::run(application: &mut Application) {
    let increment: i32 = add_i32(3, 4);
    application.total = increment;
}

machine start() {
    let mut application: Application;
    Application::run(&mut application);
}
```

The build binds the target's `ProgramEntry` slot to `start`.

`Application` remains an ordinary nominal type. This example uses its ordinary
zero initialization; a type whose invariant does not hold at zero needs an
explicit construction establishing that invariant. The local's storage and
cleanup follow ordinary machine rules. There is no implicit program-lifetime
instance created from a machine's qualified name.

The target schema controls ordinary entry parameters: hosted entry normally
exposes no physical arrival arguments, while freestanding entry may intentionally
expose raw image or initial-storage extents. An entry shape may separately permit
one explicit mutable context parameter and provision its valid fields and selected
bindings. That permission belongs to the exact entry occurrence, not an arbitrary
parameter or a qualified machine name. See the entry rules below.

The machine name is not special. The build binding chooses the machine, its
source signature states its explicit arguments, and
the target schema states how the launch environment supplies those needs.

Physical arrival and the source signature are separate. For example, a UEFI
bridge can receive firmware handles while the source machine receives only
validated image and initial-storage extents. Target-authored checked code
establishes those values; a raw platform handle is not secretly an Omega extent.
The bridge also defines platform return mapping and accounts for its storage.

See [program-entry rules](../spec/build/entry_roots.md) and
[UEFI arrival](../spec/build/uefi_entry.md) for the full contract.
End-to-end physical entry support remains incomplete under
[ENTRY-CONTENT-ROOTS](../../TASKS.md); selecting and checking an entry does not
claim that its native bridge has been installed.
The static-only source contract also does not claim that receiver removal is
complete in the compiler, bundled libraries, or remaining samples.

## Parameters And Returns

Machine parameters are entry data. A machine return type is the value shape its
body or internal state graph produces if it reaches a returned terminal
outcome. The type alone is not a termination guarantee.

```omega
machine Parser::resolve(
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

## Supply Forms

The callable machine model has five explicit supply forms:

| Supply | Spelling |
|---|---|
| Checked Omega implementation | `{ ... }` body |
| Trait requirement | Bodyless declaration inside the trait |
| External requirement realization | Bodyless `boundary machine ... satisfies Trait<...>::requirement;`, with `via <Binding>` only for an explicit payload |
| Admission-bearing claim | Bodyless `boundary machine ... ensures ...;` |

A boundary operation is a requirement inside a boundary trait. When one
operation needs independent provider selection, use a one-member boundary trait
rather than a carrier-owned operation declaration. The standalone `boundary
requirement Package::operation(...);` spelling is retired: a data path is not a
code owner, and ordinary trait arguments already carry every data, authority and
token type explicitly. The trait application, requirement name, static
telescope, signature and contract form requirement identity.

```omega
pub boundary trait InterruptCompletion {
    machine complete(acknowledgement: InterruptAcknowledgement)
        reaches PortIo
        requires
            acknowledgement in InterruptAcknowledgement::Pending;
}
```

A named implementation maps that row to a checked or external machine;
`build.omg` selects the implementation. The qualified acknowledgement supplies
the call's authority premise but neither chooses nor realizes the machine.

An external realization binds an irreducible imported operation to a
requirement without pretending the binding is executable Omega code:

```omega
windows_x86_64 machine WindowsBindings::write_file() -> ForeignBinding<12, 9, 0> {
    ForeignBinding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "WriteFile",
        },
    }
}

boundary machine Kernel32::write_file(handle: WinHandle, bytes: &[u8]) -> WriteResult
    satisfies Kernel32Requirements::write_file
    via WindowsBindings::write_file();
```

The expression after `via` is a typed compile-time locator value. It names the
physical import, while `satisfies` selects the exact requirement and inherits
its full contract. The provider must refine that contract; the import bytes
and a matching value signature are not permission to execute it.

A target prefix (`windows_x86_64 machine ...`) declares one body of a family
keyed by target under a single path. Every body of every family is resolved,
typed, and checked in every compilation, whatever host or target set is being
realized: a compile on Windows reports an error in a `macos_arm64` body.
Callers name the family; realizing a target selects that target's body, and a
realized target with no body for a called family rejects that realization only.

If the trait has lifetime parameters, the satisfaction path supplies them
explicitly. [Foreign bindings](../spec/build/foreign_bindings.md) defines exact
application identity, target-specific locators, calling plans, and admission.
A compiler intrinsic has no binding payload: its declaration, normalized
signature, and target identify the sealed implementation.

Composite adaptation belongs in checked code. For example, a
`Console::write_line` implementation may call bound `get_stdout` and
`write_file` operations, cache a handle, or merge writes. Those choices belong
in its body, not in authored provider-plan rows.

Proof-position terms such as `embed(value)` and `old(&place)` are not extra
machine supply modes. Nor does an arbitrary bodyless declaration create a
proof symbol. [Machine supply](../spec/language/machines.md#supply) defines the
closed alternatives.

## Calls

Ordinary call syntax invokes a machine contract in the current activation. Its
realization may need a call frame; a jump to a local state adds none.

```omega
let command: Command = Parser::resolve(&parser, &line);
let guard: Guard = block Mutex::lock(&mut mutex);
let event: Event = suspend Inbox::take(&mut inbox);
```

Calls and state jumps are different. A transition arm may jump to a local state,
return a value, or invoke a machine and forward its eventual result. Chapter 4
introduces these arm forms. `suspend` and `block` acknowledge possible waiting at a
direct call; they do not create another machine species or change the result
type. Chapter 5 defines their call-position rules, and chapter 18 explains the
concurrency consequences.

## Termination And Ranked Cycles

`ensures` describes a return if one occurs. `terminates` additionally promises
that a terminal outcome is eventually reached under the machine's progress
premises. A private checked acyclic body derives a local termination summary;
an exported or required promise is explicit or inherited from its requirement.

A terminating cycle needs an authored ranking witness:

```omega
machine Gauss::sum(n: u64, acc: u64) -> u64
terminates by n -> Nat::Descending;
{
    transition n {
        0 -> acc
        _ -> Gauss::sum(n - 1, acc + n)
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
  `3 * Gauss::sum(...)` is not tail: multiplication remains after the return.
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

For `Parser::scan(&mut compiler.parser, ...)`, the ordinary call borrows the
parser field. Inside `scan`, the explicit parameter names the same referent
across backedges and return. Writes affect the original field; conflicting
parent access is unavailable while the loan is live. A changing traversal
cursor can instead be an ordinary loop-carried reference parameter.

This needs no special rebinding syntax. General ranked-callee native composition
remains an [implementation gap](../../omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/README.md#ranked-native-admission),
not permission to copy the referent or omit the callee's ranking check.

### Stack demand

Once recursive cycles lower to backedges, ordinary runtime calls form an acyclic
graph. Build-time WCSU composition accounts for maximum live frames, final spill
storage, and admitted external/provider demands before activation. Register
allocation may change that resource bound, not the machine's `crashes` contract.
See [compiler-owned stack storage](chapter_16_errors_traps_failure.md#compiler-owned-stack-storage-and-spill-accesses).

## Contracts

Machines may declare requirements and guarantees.

```omega
machine Player::enter_combat(player: &mut Player)
requires
    player in Player::Alive
ensures
    player in Player::InCombat
{
}
```

The caller must satisfy `requires`. The machine body must establish `ensures`.
Independent may-ceilings publish service reach, suspension, blocking, and
guarded crashes. In particular, `crashes Cause` lists alternative route
predicates; callers may disprove routes for a concrete invocation, while the
body must keep every derived crash site within the published guards. Chapter 16
defines the crash surface, its no-cleanup terminal semantics, and the separate
requirements for fault-tolerant continuation.

A proof machine may consume mathematical witnesses and their laws through
ordinary named trait/conformance bundles. Contract facts still follow from
checked bodies and instantiated assumptions, not from a bundle's name.
Projection and forwarding preserve the witness's identity and validity.
See [chapter 10](chapter_10_compile_time_proofs.md#contracts-and-evidence-bundles)
for the evidence model and its implementation limits.

A result-case group makes postconditions conditional on one exact nominal case
of the declared result sum:

```omega
machine Search::find(items: &[Item], target: Item) -> SearchResult
ensures
    SearchResult::Found -> {
        result.index < items.len;
        items[result.index] == target;
    }
{
    ...
}
```

`->` is the existing case-directed token. The braces organize contract rows;
they construct no value, package, aggregate, or independently identified group.
The case path resolves against the declared result sum and normalizes to its
exact case symbol. A group contains ordinary guarantees checked on that result
path. It is not a domain declaration or arbitrary Boolean
guard: the returned sum tag establishes the exclusive case fact, and that fact
activates the rows. Use a qualified payload type such as `T in D` when domain
membership belongs to the returned value itself; use a guarded guarantee for a
relation specific to this invocation or outcome.

## Machine Graph Compatibility

Internal states participate in the machine's graph, but they are not public
machine entries. Values needed by a destination state are passed through its
ordinary state parameters; a state has no implicit receiver. Use local `let`
bindings for scratch work and explicit transition arguments for loop-carried
work. None of this introduces another temporary-storage scope or lifetime rule.

Working rules:

- State-transition arguments must match the target state's parameters.
- Terminal values must satisfy the active machine's return type.
- Every reachable terminal path in a typed machine graph must produce the
  declared return type.
- Transition dispatch arms add proof assumptions for the target edge.

The [machine specification](../spec/language/machines.md) defines the complete
contract. [Semantic representation notes](../../omega-rust/psi/compilation/representations/README.md)
describe its compiler owners; this guide is not an implementation coverage report.
