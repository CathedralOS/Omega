# Chapter 3: Machines

A machine is a **named, contracted transition system**. Given its inputs,
state, and authority, it produces a contract-observable trace and may produce
a terminal outcome. A productive machine may run forever, so an ordinary
function-like call is one important use of a machine, not its definition.

> **Machine taxonomy.** Runtime calls, compile-time
> evaluation, proof citation, concurrent activation, trait satisfaction, and
> boundary provision consume the same semantic construct. Checked bodies, requirements,
> external providers, and accepted trust declarations are supply modes, not
> separate machine species. See
> [machine_taxonomy.md](../design_briefs/machine_taxonomy.md).

Machines may be attached to data, or free-standing when there is no natural
owning data type.

`proposition P(...);` is the adjacent proof-formula declaration that machine
contracts may require or ensure. An ordinary proof machine carries the checked
work or derivation that establishes it. Chapter 10 defines primitive,
witness-bearing, and transparent proposition declarations.

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
    builder.target = windows_x86_64;
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

The schema keeps physical and semantic arrival separate. A UEFI physical entry,
for example, receives `ImageHandle` and `SystemTable` and returns `EfiStatus`;
the build-bound semantic continuation may instead receive only
`image: Extent in Granted` and `initial_storage: Extent in Granted`. A generated
ABI shell calls the exact target-authored bootstrap adapter, and the installed
semantic edge introduces those root occurrences after provider validation.
Neither firmware input is silently reinterpreted as an `Extent`, and no hidden
platform parameter is appended to the source machine.

> **Implementation gate:** explicit `Build` root binding, exact source-entry
> selection, target-owned `ProgramEntry` profile/schema metadata, hosted
> free/receiver source-shape checks, exact UEFI visible-root type/arity checks
> against `ProgramStorageEntry::enter`, receiver ZII checks, and the current
> semantic UEFI source-calling-plan retention, validation, and inbound lowering
> are live. The compile report also retains the exact target root slot, checked
> semantic arrival requirement, calling-plan fingerprint, and generated captures for
> both storage positions; the machine-readable program-storage artifact renders
> their semantic roles, normalized ABI placements, frame capture ranges, strict
> carry, and the pending two-grant installation rule. Physical bridge/grant
> installation is generically modeled with all-or-nothing predicate validation
> and a non-authoritative geometry/address-space/rights/provenance/era/lineage/root-origin
> record. Provider-issued roots also bind their admitted issuance, backing,
> provider, live-issuance, custody, alias, correspondence, and trust identities
> to one selected provider plan/invocation, establishment route, capacity
> account, and qualification through that record. A canonical
> non-authoritative completed-installation JSON renderer and atomic artifact
> writer cover both provider-issued and installed program-local origins. The
> installation handoff releases roots only after that record is emitted and
> seals them for retry across a write failure; ordinary compilation removes
> stale copies and never claims completion. Receiver-bound entries now retain
> their checked layout, reject insufficient or misaligned storage before grant
> consumption, conserve every reservation remainder, and audit the exact
> placement. The recorded installation handoff now rejects unchecked release
> of receiver-bound roots, validates the exact mapped backing, zeroes it, and
> retains its exclusive borrow through one activation before returning the
> conserved roots. Installed program-local roots additionally remain joined to
> their non-copyable account registry through activation failures, exact
> activation and finish, receiver-free continuation binding and recovery,
> emitted-wrapper checks, logical and operand realization, and outgoing-frame
> planning. The former passive-origin installer is gone; only checked
> installation establishment may create this custody carrier. Binding this
> handoff and portable evidence to the selected
> target-fixed physical requirement, authored bootstrap adapter and result map,
> physical provider, and generated native shell, plus corpus migration and
> removal of transitional entry-name discovery, remain under
> `ENTRY-CONTENT-ROOTS` in `TASKS.md`.

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

Dispatch-bearing status is derived from the normalized domain theory. A domain
that contributes a semantic role, authorizes an establishment route, or is an
empty explicit tag participates. A domain carrying only predicate obligations
does not; its predicates are proved after the machine has been selected. A
mixed domain participates once by identity and still contributes all of its
predicate obligations afterward. Aliases are expanded before this partition.

For one path and normalized parameter signature, result dispatch sets must be
pairwise distinct. Two declarations differing only by predicate refinements
are therefore a declaration-site duplicate; publish the stronger result once
or state the difference through `ensures`. The normalized result dispatch set
is part of requirement identity, artifact identity, and emitted-symbol
distinction. This makes result selection a lookup rather than a search. Other
ordinary parameter or generic ambiguities remain possible and reject normally.

Fixed operator spellings remain operand-directed. Their return type does not
select an operator meaning; this result-domain rule is for explicit named
machine and requirement calls.

## Supply Forms

The one machine construct has four explicit supply forms:

| Supply | Spelling |
|---|---|
| Checked Omega implementation | `{ ... }` body |
| Trait requirement | Bodyless declaration inside the trait |
| External requirement realization | `satisfies Trait::requirement via <Binding>;` |
| Accepted claim | Bodyless `boundary machine ... ensures ...;` |

An external realization binds an irreducible imported operation to a
requirement without pretending the binding is executable Omega code:

```omega
windows_x64 machine WindowsBindings::write_file() -> Binding<12, 9, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "WriteFile",
        },
    }
}

machine Kernel32::write_file(handle: WinHandle, bytes: &[u8]) -> WriteResult
    satisfies Kernel32Requirements::write_file
    via WindowsBindings::write_file();
```

The value after `via` must be compile-time evaluable to the closed `Binding`
vocabulary. The compiler normalizes and validates it, derives the provider
plan from explicit conformances, and assigns any trust expenditure only when
the provider is admitted. `satisfies` supplies the requirement contract and
public service/suspension/blocking and guarded-crash ceilings; the
binding/provider behavior must refine each one. A `via` machine does not repeat
those clauses.

Binding operands are ordinary typed compile-time values. A DLL locator is one
object-format-specific sum case, so its library/export, library/ordinal, or
object/symbol/version coordinates cannot be independently paired. Its textual
bytes are physical target data, not Omega names or provider-selection keys. The
satisfied requirement's `Calling<C, Policy>` relationship separately evaluates
the ABI `CallPlan`; the binding cannot select a second one. A compiler-intrinsic
binding has no payload: its exact realization-machine symbol, normalized
signature, and target select the sealed lowering catalog entry.

The compiler fingerprints the complete evaluated binding, its producer closure,
and selected target. Changing a raw foreign spelling therefore changes every
dependent final artifact and requires relink plus fresh admission. `build.omg`
may select a target/provider declaration but cannot rewrite a binding value.

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
  with explicit storage the machine declares and sizes, such as a fixed-capacity
  field or an allocator-backed collection. Activation frames are storage the
  author never sees or sizes; depth does not hide there.
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
  opaque providers remain responsible for their pinned stack domains. Their
  complete admissible arrival contexts, per-domain entry epochs, declared
  nesting, and checked or admitted demands compose through the external-root
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
Independent may-ceilings publish service reach, suspension, blocking, and
guarded crashes. In particular, `crashes Cause` lists alternative route
predicates; callers may disprove routes for a concrete invocation, while the
body must keep every derived crash site within the published guards. Chapter 16
defines the crash surface, its no-cleanup terminal semantics, and the separate
requirements for fault-tolerant continuation.

A contract fact may be named when the body must retain and project its exact
erased proof term:

```omega
requires proof: witness_bearing_proposition(input)
ensures result_proof: another_proposition(result)
```

Named requirements are positional erased proof inputs supplied after a call's
`;` separator, which marks the boundary between ordinary Type arguments and
Prop inhabitants. Named guarantees are public proof-output selectors. The
ordinary result stays in its declared Type lane; a caller that needs one exact
outgoing witness selects it after the same `;` separator in the binding pattern:

```omega
let (value; result_proof: proof) = call(...);
```

Unselected guarantees still enter the caller's fact catalog but mint no local
witness term. Outcome-guarded selectors exist only in the matching outcome arm.
Chapter 10 defines evidence projection, assignment, call passing, output-lane
binding, and the separate proposition, evidence-term, and derivation identities.

A result-case group makes postconditions conditional on one exact nominal case
of the declared result sum:

```omega
machine Search::find(items: &[Item], target: Item) -> SearchResult
ensures
    SearchResult::Found -> {
        in_bounds: result.index < items.len;
        items[result.index] == target;
    }
{
    ...
}
```

`->` is the existing case-directed token. The braces organize contract rows;
they construct no value, package, aggregate, or independently identified group.
The case path resolves against the declared result sum and normalizes to its
exact case symbol. A group may contain named evidence outputs and unnamed
fact-only guarantees. It is not a domain declaration or arbitrary Boolean
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

> **Implementation gate:** the current Rust trees do not yet carry the
> normalized complete machine contract or explicit supply mode. See
> [semantic_taxonomy_representation.md](../architecture/semantic_taxonomy_representation.md).
