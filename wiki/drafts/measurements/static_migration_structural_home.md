# The static migration needs a realizable home for structural values

**STATIC-MACHINE-MIGRATION** moves every program off machine/state `self`
receivers. Ten samples have migrated and run. The remaining sample work is
mostly blocked behind one missing capability, and this file records exactly
what it is so the next slice is not spent rediscovering it.

## What was measured, 2026-09-29 at `5685b2942c5`

Under a receiver, a structural value lives in a field of the entry's data and
the call reads `self.line`. That realizes today. Under the static contract the
data is gone, so the same value must live somewhere else — and **none of the
alternatives realize.**

A three-line probe, identical except for where the carrier lives:

| where the carrier lives | `--check` | native |
| --- | --- | --- |
| field of the entry data (`self.line`), i.e. today's form | passes | **realizes** (`text/text_padding` runs) |
| ordinary local (`let mut line: [u8; 2] in Utf8 = "ab";`) | passes | `InvalidUnitMachinePlan` |
| state parameter (`state show(line: [u8; 2] in Utf8)`) | passes | `InvalidUnitMachinePlan` |

Every failing case stops at the same phase, `call operation: structural
arguments: caller structural result`. So the refusal is not about locals
specifically: it is that a structural value has no realizable home other than
a data field once it is handed to a machine.

## The same wall, from three directions

| customer | argument | result |
| --- | --- | --- |
| `dutch_flag` (board note) | `&mut pause`, a local `[u8; 256]` to `Console::read_line` | `UnknownStructuralArgumentPlace` |
| `generic_counters` | `&mut score`, a whole local record | stops at `structural arguments: caller structural result` |
| `nested_diagnostics` | `&dash.engine`, a **shared** borrow of a nested member of a local | the same stop |
| this probe | a carrier passed by value to `Console::write_line` | the same stop |

The board's note describes the mutable case. `nested_diagnostics` shows the
gap is **not specific to mutable arguments, nor to whole locals** — a shared
borrow of a projection hits it too, and so does an ordinary by-value carrier.
Whoever completes the claimed `SCALAR-CALL-ELEMENT-WINDOW` transport should
cover all four shapes, not only `&mut` whole-local views.

## How much rides on it

Of the **136** samples still carrying a receiver, **119** hand a structural
value to a machine — a printed carrier, a `read_line` buffer, or an explicit
`&`/`&mut` argument. Those cannot complete the migration until a structural
value has a home outside a data field.

**17** remain that do not, and two of those (`generic_counters`,
`nested_diagnostics`) are already proven blocked at the deeper level above.

## Sequencing

Migrating a blocked sample anyway would trade a program that runs today for
one that only checks. The board says not to demote coverage to avoid exposing
a gap, and this does not: `dutch_flag` already carries this gap as a tracked
customer, so a second red sample adds no coverage and costs a working program.

The order that keeps programs running: land the structural-argument transport,
then migrate the 119 in bulk. Until then the migration advances on canaries
and on samples whose state is entirely scalar.

## Three distinct blockers, not one

Measured 2026-09-29 while migrating canaries. Each has its own signature, and
conflating them would send a repair at the wrong layer.

| # | trigger | signature | evidence |
| --- | --- | --- | --- |
| 1 | a structural value handed to a machine from anywhere but a data field | `InvalidUnitMachinePlan`, stopping at `call operation: structural arguments: caller structural result` | the carrier probe above; `generic_counters`; `nested_diagnostics` |
| 2 | a static entry that applies a boundary operator | `BoundaryRealizationMismatch(BoundaryMachineId(1))` | `pass/providers/external_boundary_requirement_via_exit` checks with a receiver, rejects as a free machine |
| 3 | a checked canary whose data held only the Console binding | native `Lowering(Unsupported("composed Unit control has unsupported graph custody"))` | `pass/arithmetic/runtime_integer_casts_exit` |

Blocker 3 is worth stating carefully, because the obvious reading is wrong.
The migration is not a checking failure: after it, `--check` passes on
`macos_arm64`, `linux_x86_64` **and** `windows_x86_64`. With the receiver the
fixture reports `native exit 70 / interp exit: 70`; migrated, native
compilation fails outright. Thirteen of eighteen fixtures in that class
regressed the same way and none was landed.

Nor is it "static entries cannot call Console": `samples/cli/collections/slice_maximum`
already uses `machine main()` with `Console::write_line` and
`Console::exit_process` and realizes natively today. Something narrower about
this shape is unsupported, and it was not isolated further here.

Nor is it the entry shape. `entry_roots.md`'s `NoneOrProvisionedZii` "admits
an entry without a context or with one ordinary `context: &mut Context`
parameter", so a parameterless `machine main()` is admitted by contract.

## A hosted entry cannot take a context parameter, so entry state has nowhere to go

This is the reason only entries with an EMPTY data have migrated, and it is a
fourth blocker distinct from the three above.

`entry_roots.md` describes `NoneOrProvisionedZii` as admitting "an entry
without a context or with one ordinary `context: &mut Context` parameter".
Taking the second option on a hosted entry is refused:

```
machine main(context: &mut Main) reaches Console { ... }
  error: hosted `ProgramEntry` exposes no arrival parameters, but `main` declares `context`
```

So for a hosted `ProgramEntry` the entry must be parameterless. An entry that
holds state therefore has only one place left to put it: ordinary locals.

**And that is not a mechanical rewrite.** Measured over 35 canaries with one
to four plain fields, a single body and no console binding: 3 refused the
rewrite, and of the 32 that applied, **24 moved** — 15 regressed outright and
9 moved their diagnostic. One stayed checked. All were reverted.

A field and a local are not interchangeable to the checker. A field is
ZII-established and owned by the data; a local is not. Ownership, loan and
establishment facts differ, and those facts are exactly what many fixtures
pin.

### What this leaves

| migration | mechanical? | landed |
| --- | --- | --- |
| empty `data Main` + entry receiver | yes | 98 canaries |
| operation that never reads its receiver | yes | 25 canaries |
| operation that does read it, named as a parameter | yes | 11 canaries and packages |
| entry holding state | **no** | blocked above |

Either the hosted entry shape gains the provisioned context parameter its own
spec describes, or every stateful entry is a per-fixture rewrite whose subject
must be read first. The first would unblock the bulk mechanically; nothing
below it will.

## Receiver-shaped contracts: a fifth category, and why the seam ends

Some declarations are not merely written with a receiver; the receiver is part
of the contract the fixture pins. Migrating them produces a correct-looking
diff that rewrites the question:

- **drop hooks** — `Resource::drop(&mut self)`; the cleanup contract selects on
  that shape (`fail/drops/authored_cleanup_static_argument_rejected`,
  `fail/drops/drop_ensures_unlocked_predicate`)
- **callback slots** — `Spread::plan(&mut self, ..)` behind a private callback
  layout (`fail/layouts/private_callback_slot_wrong_layout`)
- **the entry receiver itself** — `fail/build/program_entry_receiver_not_zii`,
  both `fail/build` program-entry binding fixtures,
  `fail/calls/free_machine_named_transition_rejected`
- **shared-receiver and dependent-window subjects** —
  `fail/atomics/shared_receiver_plain_field_write`,
  `fail/dependent/data_where_*_during_window_*`

Measured: of 21 compile-only, binding-free fixtures whose attached operations
have no call sites — the most favourable remaining shape — **every one that
applied regressed**, first 7 of 7, then 2 of 2 after excluding the obvious
contract shapes. The diffs are right; `Application::launch(application: &mut
Application)` is exactly the contract's form. The fixtures still move, because
what they assert is the receiver.

These migrate when the receiver syntax is removed compiler-side, which the
board lists as the step after customer migration. They are not per-fixture
work that can be finished first.

## The corpus, classified

944 files still carry a receiver. Their composition, by the properties that
decide migratability:

| count | tier | leg | routed binding | receivers |
| --- | --- | --- | --- | --- |
| 344 | pass | runs | yes | multiple |
| 94 | pass | compile | yes | multiple |
| 86 | fail | compile | no | single |
| 81 | pass | compile | no | multiple |
| 70 | fail | compile | no | multiple |
| 64 | fail | compile | yes | single |
| 54 | fail | compile | yes | multiple |
| 41 | pass | compile | yes | single |
| 37 | pass | compile | no | single |
| 31 | pass | runs | yes | single |

The 375 that **run** need the run leg of a static console entry, which fails
with `composed Unit control has unsupported graph custody`. The 253 carrying a
**routed binding** need either that binding to live somewhere a static entry
can reach, or the hosted entry to accept the context parameter its spec
describes. The binding-free compile-only remainder is where the receiver is
usually the subject.

## What unblocking the hosted context parameter actually takes

The single highest-leverage unblock is letting a hosted entry declare
`machine main(context: &mut Main)` instead of a receiver: it converts the 253
routed-binding files from blocked into the mechanical rewrite already proven
on smaller shapes. This section records what that change is, measured by
making it and walking the failures, so the next attempt starts from the map
rather than the first error.

### Site 1 — `provider-planning/src/program_entry.rs`

Two validators filter `visible` parameters as "everything that is not
`is_self` and not a placed view", and `ProgramEntryVisibleParameters::None`
then rejects whatever is left. A provisioned context parameter has to be
excluded there too, and `validate_selected_program_entry_shape` has to fold it
into the receiver signature:

```rust
let receiver = self_parameters.first().copied().or(context_parameter).map_or(
    ProgramEntrySourceReceiverSignature::Free,
    |provisioned| ProgramEntrySourceReceiverSignature::ProvisionedMutable {
        normalized_type_identity: typed
            .normalized_type_identity(provisioned.type_reference).into_string(),
    },
);
```

Recognizing the parameter is unambiguous **only** on a slot whose
`visible_parameters` is `None`: with nothing arriving from the loader, a single
mutable non-placed parameter can only be the context. Slots exposing arrival
roots must keep their arity rules.

This part was implemented and compiles, and it does remove the rejection:
`hosted ProgramEntry exposes no arrival parameters` no longer fires.

### Site 2 — `selected-dispatch/src/service_custody/root.rs`

Establishment then fails with `selected provisioned ProgramEntry rejoins 0
self receivers; expected one`. That function needs the same recognition, and
one thing more: it derives the owning data from
`machine.attached_data_symbol`, which a free entry does not have. The owner
has to come from the parameter's referent instead. Receiver logic appears at
five points in that file (lines 73, 105, 129, 237, 352), and it carries a
separate `attachment_type_identity` beside the receiver identity, so the
attachment concept needs settling too.

### Site 3 — the attachment identity is a checked fact, in Psi

With both sites above implemented and compiling, establishment gets further
and then fails with:

```
selected ProgramEntry establishment rejoins 0 Terminal attachment identities; expected one
```

`selected-dispatch` reads `attachment_type_identity` off
`checked.facts.flow.terminal_unit_effects`, for both `machines` and
`composed_machines`. That identity is produced in
`04_typed-trees-to-checked-trees/src/execution/terminal_unit/`, keyed on the
machine's attachment, and threaded through `providers.rs` at three call sites.
A free entry has no attachment, so the fact is absent and no amount of work in
Omega can supply it.

So the change spans three stages: provider planning and selected dispatch in
Omega, and unit-effect fact production in Psi's checking pipeline, inside the
`execution/terminal_unit/` cluster.

### Why this was implemented and then reverted

Sites 1 and 2 are written and compile, and each moves the failure one stage
later -- `hosted ProgramEntry exposes no arrival parameters` becomes `rejoins
0 self receivers` becomes `rejoins 0 Terminal attachment identities`. None of
that helps a customer: no program compiles that did not before, and the
diagnostic an author sees gets worse at every step, moving from a clear
statement about the entry shape into dispatch and then into unit-effect facts.

The work is genuinely the board's compiler half -- "provision the exact
ordinary mutable context parameter named by the entry shape ... remove
attachment/`is_self` as authority inputs" -- and it wants all three sites in
one change, with the Psi fact first. Landing one or two of them leaves the
compiler half-migrated with no customer able to use it and a worse error for
anyone who tries.


## `source/library` is blocked on the same syntax removal

The bundled library is the third corpus the migration names, and all 22 of its
files carrying a receiver fall into two kinds, neither of which is a
customer-side rewrite.

**Boundary requirements.** `core/interrupt.omg`'s
`machine save_and_mask(&mut self) -> InterruptMaskGuard in Active`,
`core/task.omg`'s `pub boundary requirement Task::request_cancel<T>(&self)`,
`core/layout.omg`'s `PlacedField` surface. Changing these changes the boundary
contract every implementor and provider satisfies, not one file.

**The method-call surface itself.** `core/vec.omg` declares

```
// Method-call surface used by borrow/lifetime analysis. Runtime lowering may
// still choose the boundary operator below; this state-shaped declaration lets
// `items.push(value)` participate in ordinary receiver mutation checks.
machine Vec::as_slice<T>(&self) -> &[T] { }
machine Vec::push<T>(&mut self, value: T) { }
```

These exist so the receiver call spelling participates in borrow analysis.
They are removed **with** that spelling, not migrated ahead of it: rewriting
them now breaks 82 call sites (25 `.push(`, 57 `.as_slice(`) and removes the
surface the borrow checker reads, with nothing to replace it until the
receiver call syntax itself goes.

Two files' matches are prose, not code: `core/fixed_vec.omg` and
`core/drop.omg` mention `Cell::touch_count(&self)` and `T::drop(&mut self)` in
comments describing the hooks.

So the library waits on the same step as the receiver-as-subject fixtures --
the removal of the accepted receiver syntax, which the board sequences after
customer migration.

## Why operation migration stops where it does

An operation can be migrated even when its entry cannot: the entry keeps its
receiver and the operation takes its data as a parameter. That is how 25
receiver-ignoring and 11 receiver-reading operations landed. The limit is the
call site, and it is blocker 1 again.

`pass/calls/nested_machine_continuation` is the clean demonstration. Migrating
`Banner::entry(&mut self)` to `Banner::entry(banner: &mut Banner)` requires its
caller to spell the argument:

```
machine Main::main(&mut self) reaches Console {
    Banner::entry(&mut self.banner);
    ...
}
```

`&mut self.banner` is a structural projection handed to a machine, so it fails
exactly as `&dash.engine` and `&mut score` do:

```
selected ProgramEntry establishment rejoins 0 Terminal attachment identities
```

The entry still has its receiver here, so this is not the entry-shape blocker.
It is the same missing transport.

So the operation seam is bounded precisely: an operation migrates when its
call sites pass **no structural argument** -- no call site at all, or scalar
arguments only. Every operation whose caller must hand it a record, a
projection or a carrier waits on the structural-argument transport, alongside
the 119 samples.

## Scratch-to-locals is a separate, smaller seam -- and it has its own limit

Hoisting a scratch field to a local is independent of receiver removal: the
machine keeps its receiver and only the value moves. That half of the work can
proceed while every migration blocker above stands, and it is how `bit_shift`
and `vending_machine` landed.

The population is small, though. Filtering every sample for a **scalar** field
that is used in exactly one state and assigned there -- so a local fully
replaces it -- returns 2. Most sample fields genuinely cross a state edge, or
are a `Binding<R>` carrier or an input buffer, neither of which can be a local.

An earlier count of 85 was wrong: it counted fields appearing in one state
without requiring an assignment, and included carriers and buffers.

### A local sum-typed transition subject does not check

`brightness_control` assigns `command: Command` and immediately uses it as its
transition subject, so it reads as pure scratch. It is not:

```
let command: Command = Command::SetLevel { level: 65 };
transition command { ... }
```

checks as a **field** and fails as a **local**, with `selected ProgramEntry
establishment rejoins 0 Terminal attachment identities`. The sample checks
today, so this was reverted. Hoisting a scalar is safe; hoisting a sum-typed
value that a transition dispatches on is not, and the two need separating
before any bulk pass.

## An unread state receiver is usually still load-bearing

208 states across 122 canaries declare `&mut self` and never mention it. That
reads as pure ceremony, and for a few it is. For most it is not.

Dropping the receiver from every such state moves **73 of 115** fixtures --
32 regressed, 36 moved a diagnostic, and only 24 stayed checked. A first batch
of 20 files was clean, which is exactly how this looks safe before it is
measured at scale.

The receiver carries the entry's storage across the state edge. A body that
names nothing from it still arrives with it, so removing the declaration
changes what the edge transports even when the source text does not read it.

Six files were landed after gating individually. The rest wait for the same
step every other receiver does.

## The structural-argument transport has partly landed

Re-measured 2026-09-29 after `psi: admit dynamic-index record-element field
writes through element views`. The blocker is no longer all-or-nothing.

| argument | before | now |
| --- | --- | --- |
| `&self.dash.engine` — SHARED projection of the ENTRY's receiver | refused | **realizes** |
| `&mut self.banner` — MUTABLE projection of the entry's receiver | refused | refused |
| `&self.list` inside another OPERATION's body | refused | refused |
| a local carrier by value to `Console::write_line` | refused | refused |
| `&mut score`, a whole local record | refused | refused |

`samples/cli/basics/nested_diagnostics` migrated on the strength of the first
row and runs with its documented exit 85. It had been deferred for exactly
this refusal.

The remaining rows are what the transport still owes. The third is worth
separating from the second: it is not about mutability. A shared projection
handed on from inside an operation body still fails where the same spelling
from the entry now succeeds.

Six canaries matching that third shape were migrated and reverted. Their
diffs are correct -- `machine ItemList::get_count(item_list: &ItemList)` with
`ItemList::get_count(&self.list)` at the call -- and several are
receiver-as-subject besides, `nested_call_tail_roots_exit` pinning "receiver-
form tail on a member-path field" by name.

## Why a state receiver cannot be dropped piecemeal

Measured 2026-09-29. A state that READS `self` must declare the receiver:

```
machine Main::main(&mut self) reaches Console {
    transition { _ -> done() }
    state done() { self.console.exit_process(self.n); }
}
  error: machine `Main::main` state `done` uses `self`, which is not a declared
         local, parameter, field, or type in this scope
```

The consequence is the one that matters for a bulk pass: a receiverless state
cannot TRANSFER the receiver onward. Dropping it from a state whose own body
names nothing still breaks a later state that needs it, which is the
`state graph: terminator: conditional successors: receiver transfer` failure
that reverted 31 of 40 samples in the combined sweep.

So the rule is not per state. A state's receiver may be dropped only when no
state reachable from it needs one -- in practice, only when the whole machine
is free of `self`.

**For samples that is never true today.** Applying the safe rule to the entire
sample corpus qualifies **zero** files: every sample carrying states has at
least one state reading `self.console`, and a routed `Binding<R>` cannot be a
local. Samples are therefore gated exactly on the binding question, not on
anything per-state.

The count to watch is receiver OCCURRENCES, not files. A file leaves the
"still has a receiver" list only when every receiver in it goes, so the
partial migrations that are possible today are invisible to a file count:
`samples/cli/simulation/elevator` is migrated and still holds 16. At this
writing the corpus holds 3,078 occurrences in samples and 4,327 in canaries.

## Rewrite rules the gate taught, 2026-09-29

Four rules separate a receiver that can go from one that cannot. Each was
learned from a rejection, and each was wrong in an earlier attempt.

**A `pub machine` is still a machine.** A declaration scanner anchored on
`machine` or `state` at a line start misses every `pub`-qualified machine, and
with it the enclosing machine of any state nested inside one. This is silent:
the states are still found, so they are rewritten without their owner. Fixing
the anchor took one threading pass over the same corpus from 39 fixtures to
138.

**An empty body is a declaration stub, not dead spelling.** `core::Vec`'s
`as_slice` and `push` have no body; the receiver is the method-call surface
that lets `items.push(value)` participate in receiver mutation checks.

**A conditional transition transfers the receiver to all successors at once.**
A state may drop its receiver only when every state of the enclosing machine
can. `Vec::open4` reaches `verify`, `opened` and `closed`; `opened` writes
three fields, so none of the three may drop. Violating this reports `state
graph: terminator: conditional successors: receiver transfer`.

**An operation called method-style from another file keeps its receiver.** A
call site cannot be requalified by name: the same name may belong to a
boundary the package never declares. Rewriting `self.console.write_byte(...)`
to `ParserHarness::write_byte(&mut self.console, ...)` type-checked into an
implicit i32-to-u8 store.

Threading adds two more. A state needs the data when it *reads* it, declared
receiver or not — an undeclared reader used to see the machine's implicit
receiver. And a state that only forwards still has to carry it, so the
receiving set closes over the state graph.

## What each seam yields

| Seam | Attempted | Held | Where the rest goes |
| --- | --- | --- | --- |
| Name the data, machine-only files | 73 | 20 | diagnostics quote the receiver; hosted entry |
| Drop unread receivers, canaries | 42+200 | 118 | same |
| Drop unread receivers, samples | 54+26 | 18 | same |
| Drop unread receivers, library | 12 | 7 | filesystem fixtures move |
| Thread data through states, canaries | 486+734+532 | 164 | hosted entry; golden text |
| Thread data through states, samples | 150 ×2 | 0 | the entry's routed `Binding` |
| Thread data through states, psi lexer | 8 | 1 file, 317 decls | five cross-file operations |

Samples are the flat case and the reason is single: threading state data
through all 150 sample files with machine receivers takes `sample_gate` from
137/149 to 19/149, and skipping program entries recovers it only to 105/149.
The `Binding` a sample's entry routes is the obstacle, which is what
`STATIC-ENTRY-CONTEXT-*` owns.

## Census after this pass

`tests/omega` 698 files / 3571 declarations, `samples` 150 / 2201,
`source/psi` 8 / 334, `source/omega` 1 / 9, `source/library` 18 / 1697.

## Three scanner blindnesses, each shrinking every measurement above

The rules in the previous section were necessary but not sufficient, because
the tool could not see all the declarations it was reasoning about. Each of
these was silent: states nested inside an invisible machine were still
rewritten, just without their owner.

**`pub machine`.** Anchoring on `machine` at a line start misses every
`pub`-qualified declaration. Fixing it took one threading pass from 39 held
fixtures to 138 over the same corpus.

**Clause-bearing bodies.** A declaration may carry `reaches`, `invokes` or
`requires` clauses before its body, and those end in `;`. Stopping the body
scan at the first semicolon reports the machine as bodyless, which skipped its
whole file: 76 canary files and 5 samples, 112 declarations, never attempted.

**Target-scoped machines.** `macos_arm64 machine Filesystem::open_with(&mut
self, ...)` carries a target qualifier before `machine`. Allowing it raised the
true census from 6320 declarations to 6548 and made `source/library/std`'s five
`filesystem_impl.omg` files visible for the first time.

## Requalification cannot be done by name

A call site moves onto the owner path only when the receiver expression
resolves to that owner's type. Matching the operation name conflates
`Lexer::reject` with the `TokenStream::reject` one line away in
`lexer.output.reject(...)`, and rewriting `self.console.write_byte(...)` into
`ParserHarness::write_byte(&mut self.console, ...)` type-checks into an
implicit i32-to-u8 store. `self` is typed by the machine that lexically
encloses the call, not by the callee's owner.

Three further exclusions, all learned from rejections:

- An **empty body** is a declaration stub whose receiver is the method-call
  surface. `core::Vec`'s `as_slice` and `push` exist so `items.push(value)`
  participates in receiver mutation checks.
- A **build-time policy machine** (`StatLayout::plan`, `Spread::plan`) is
  invoked by the compiler at a fixed arity, from no call site the rewrite can
  see.
- An operation with **one declaration per target** must have all of its
  variants in the same pass. Threading the macOS `Filesystem::rda_drain` and
  requalifying the shared call site leaves the Windows variant taking one
  argument fewer.

`source/library/core` has no `build.omg`, so its package root resolves to the
repository root and the sibling set becomes the whole tree. That both empties
the outside-call-site guard and rewrites unrelated canaries.

## A stale baseline invents improvements

Comparing a run against a corpus baseline taken several landings earlier
reported nine fixtures newly checking and six returning to the golden. Taking
the baseline immediately before the run, in the same command, showed the true
figure: zero of either. `main` had moved underneath the comparison. Any
attribution here must pair its two runs back to back; none of the landed work
in this series rests on a separated pair.

## Position drift in a substituting rewrite

A call-site rewrite that resolves each match against a captured copy of the
file silently corrupts itself: `re.sub` scans the accumulating string, so once
an earlier replacement changes its length, every later `m.start()` indexes the
wrong place in the captured copy and resolves to the wrong enclosing machine.
The symptom is a scattering of call sites left unrewritten with no pattern to
them. Resolve against `m.string`, the text actually being scanned.

## `source/library/std` after four more defects

The earlier reading of this package was wrong twice over: the improvements it
reported came from a stale baseline, and the reasons it gave for stopping were
tool defects, not language limits. Gating one fixture at a time -- 81 seconds
against `pass/filesystem/cross_linux_value_syscalls`, not 40 minutes against
the corpus -- found four more:

- A machine's **own tail recursion** is spelled unqualified in a transition
  arm (`true -> mkall_walk(path, i + 1)`) and needs the data exactly as a
  receiving state does.
- `self.step(..)` is a **transition to a sibling state** spelled through the
  receiver, not a call on the data; renaming `self` turns it into a value call
  that resolves to nothing.
- An operation the rewrite **skips keeps its receiver**, so its call sites must
  keep the attached spelling. Requalifying them anyway draws "explicit self
  argument for attached call `read_dir_nth` does not belong to the receiver's
  exact data type".
- **Position drift**: resolving each match against a captured copy of the file
  while `re.sub` scans the accumulating string. Once a replacement changes the
  length, every later lookup lands in the wrong machine. The symptom is call
  sites missed in no discernible pattern.

One genuine compiler gap remains and is excluded rather than worked around:
`Filesystem::rda_step` and `rda_drain` index `self.rda_stack[self.rda_depth -
1]`, which the compiler accepts rooted in a receiver and refuses rooted in a
parameter -- "index `filesystem.rda_depth - 1` is a computed expression, not
yet supported as an indexed operand".

Landed with 0 new regressions against a baseline captured immediately before,
in the same command. It costs diagnostic quality on five already-red programs,
which now fail at a unit-plan omission rather than at provider selection --
and that is itself a finding: these `Filesystem` machines do not yet lower in
static form, which the provider failure had been hiding.

## What now bounds the remainder

`source/library` holds 791 declarations that the rewrite deliberately will not
touch, because the operation is called method-style from a program outside its
package. Those callers are the canaries and samples, and they are waiting on
the hosted entry. The library is therefore no longer independently blocked --
it is downstream of the same dependency.

`source/library/core` is the extreme case: every one of its operations is a
method-call surface, so it migrates last, after its callers, not first.
