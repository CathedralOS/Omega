# Chapter 3: Machines

A machine is a **contracted transition system**, named or anonymous. Given its
inputs, state, and authority, it produces a contract-observable trace and may produce
a terminal outcome. A productive machine may run forever, so an ordinary
function-like call is one important use of a machine, not its definition.

> **Machine taxonomy.** Runtime calls, compile-time
> evaluation, proof citation, concurrent activation, trait satisfaction, and
> boundary provision consume the same semantic construct. Checked bodies, requirements,
> external providers, and accepted trust declarations are supply modes, not
> separate machine species. See
> [machines and contract refinement](../spec/language/machines.md).

Machines may be attached to data, or free-standing when there is no natural
owning data type.

Contracts state logical conditions; ordinary machines establish their
conclusions. Traits and named conformances bundle mathematical operations,
witnesses, and laws. General logical expressions do not add an executable
machine supply mode; see [chapter 10](chapter_10_compile_time_proofs.md).

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

## Anonymous Machines

Write a small machine at its use site without naming a declaration:

```omega
let above_threshold = [threshold = threshold](value: u32) -> bool {
    value > threshold
};

let selected = above_threshold(sample);
```

The brackets explicitly construct its captured data. Here threshold is copied
once; the body runs only when called. Use ordinary borrow or move expressions
to capture references or owned resources instead. A capture-free form is
`(value: u32) -> u32 { value }`.

These are ordinary machines: their bodies may have states, transitions,
preconditions, guarantees, and termination measures. Calls do not allocate a
closure box, spawn a task, or produce a future. A statically selected body
operates on its ordinary environment; generic libraries check it against an
inline callable contract or an exact named trait requirement.

An explicit `&self`, `&mut self`, or owned `self` parameter declares access to
the environment. If omitted, a local body's capture uses determine the required
receiver access. Moving out a capture consumes the environment; simply owning
a capture does not. Captured resources retain their obligations even if the
machine is never called. See [anonymous machines](../spec/language/anonymous_machines.md)
for construction failure, fresh invocation loans, erased captures, and task use.

## Program Entry

`build.omg` binds a target-owned program-entry slot to one exact machine. The
binding selects what the target bridge will call; it is not itself a call and
does not pass arguments or allocate values. A free entry machine has no
implicit state:

```omega
machine start() {
    Console::write_line("Hello, Omega.");
}
```

When the selected entry is attached and has one `&mut self` receiver, the
receiver declaration requests exactly one program-lifetime receiver instance:

```omega
data Application {
    total: i32;
}

machine Application::start(&mut self) {
    self.total = add_i32(3, 4);
}

machine build(builder: &mut Build) {
    builder.roots.bind(
        windows_x86_64::ProgramEntry,
        Application::start
    );
}
```

The selected machine's receiver is the entire request: the generated target
bridge provisions one ZII-valid `Application` beneath an entry-supplied storage
root and lends the only reference as `&mut self`. No separate declaration says
that the value is static. The value is not globally nameable and no `static`
declaration exists. Its physical placement is target lowering: a hosted image
may reserve it in writable image storage, while a freestanding target may
partition initial storage. Either way, the artifact records the derived
subextent and root lineage rather than minting a new storage root.

This provisions one value occurrence. `Application` remains an ordinary
nominal type; the bridge's admitted storage root and derived subextent carry
the authority and qualification for this receiver. Other `Application` values
follow the ordinary construction and ownership rules.

If the receiver cannot be validly constructed through ZII, the binding rejects.
Use a free entry machine and explicitly construct the required state from the
resources that target schema exposes. The target schema also controls ordinary
entry parameters: hosted entry normally exposes none, while freestanding entry
may intentionally expose raw image or initial-storage extents.

The machine name is not special. The build binding chooses the machine, its
source signature states whether it needs a receiver or visible arguments, and
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

### Result-domain overloads

A named machine or requirement may reuse one path and parameter signature when
each declaration returns a different set of dispatch-bearing domains. This is
compile-time overload selection over erased qualification, not runtime return
type inspection:

```omega
boundary machine I32::from_f64(value: f64) -> i32
    requires finite_in_i32_interval(value);

boundary machine I32::from_f64(value: f64) -> i32 in Trapping;
boundary machine I32::from_f64(value: f64) -> i32 in Saturating;
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
| Top-level provider requirement | `boundary requirement Package::operation(...);` |
| External requirement realization | Bodyless `boundary machine ... satisfies Trait<...>::requirement;`, with `via <Binding>` only for an explicit payload |
| Admission-bearing claim | Bodyless `boundary machine ... ensures ...;` |

A top-level boundary requirement is a nominal operation slot whose natural
source owner is a carrier rather than a trait. It is explicit: neither a
bounded reach clause, absence of a body, catalog lookup, `via`, nor a later
build selection can manufacture this declaration kind. Its exact package,
path, static telescope, signature, and contract form requirement identity.
Checked and external machines may target it with the ordinary `satisfies`
clause. A carrier-qualified requirement remains callable through its attached
operation syntax; the declaration contributes interface identity, not runtime
authority. Any authority comes from the receiver or arguments—for example, a
linear acknowledgement consumed by `complete`.

An external realization binds an irreducible imported operation to a
requirement without pretending the binding is executable Omega code:

```omega
windows_x86_64 machine WindowsBindings::write_file() -> Binding<12, 9, 0> {
    Binding::DllImport {
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

The expression after `via` is a typed compile-time binding value. It names the
physical import, while `satisfies` selects the exact requirement and inherits
its full contract. The provider must refine that contract; the import bytes
and a matching value signature are not permission to execute it.

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

Ordinary call syntax enters another machine contract. Its realization may need a
call frame; a transition stays within the current activation.

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

### Receiver subplaces

For `compiler.parser.scan(...)`, the ordinary call borrows the parser field.
Inside `scan`, that field is the whole receiver and remains the same referent
across backedges and return. Writes affect the original field; conflicting
parent access is unavailable while the loan is live. A changing traversal
cursor can instead be an ordinary loop-carried reference parameter.

This needs no receiver-rebinding syntax. General ranked-callee native composition
remains an [implementation gap](../../omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/README.md#ranked-native-admission),
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
machine Player::enter_combat(&mut self)
requires
    self in Player::Alive
ensures
    self in Player::InCombat
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
machine entries.

Working rules:

- State-transition arguments must match the target state's parameters.
- Terminal values must satisfy the active machine's return type.
- Every reachable terminal path in a typed machine graph must produce the
  declared return type.
- Transition dispatch arms add proof assumptions for the target edge.

The [machine specification](../spec/language/machines.md) defines the complete
contract. [Semantic representation notes](../../omega-rust/psi/representations/README.md)
describe its compiler owners; this guide is not an implementation coverage report.
