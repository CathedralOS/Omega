# Code Repair Queue

Agents fix obvious code-quality and architecture defects without waiting for
the owner. This queue lists concrete structural problems found outside an
agent's current repair, so another agent can fix, validate and land them. An
entry is neither a new language rule nor a reason to stop the current work.

Use [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md) only for truly unsettled language or
high-level architecture choices. Apply the existing
[architecture rules](AGENTS.md#conventions-and-workflow) directly; wide scope,
conflicting edits and difficulty need no decision. Plain missing functionality belongs in its existing outcome in
[TASKS.md](TASKS.md), not here. Refusing with "not implemented yet" fails
closed; it is not an unsound acceptance.

## How to add one

Add an entry when something would mislead the next agent, or when fixing it
properly reaches beyond the crate you are in. An ordinary bug, a style
preference, or anything you can repair inside your own task does not belong
here: just fix it.

The best time is your first hour in an area, before you have your own change to
defend. Name the broken mechanism and what it costs, not a speculative
framework or a list of unimplemented features.

- Add the entry at the top of the list under "Entries"; the list is flat, newest first.
- Name one category from the list below on its `Category` line.
- Keep it to the lines of the template. Link the file and line.
- Add it in the pull request you are already landing; never open one for it.
- Merge overlapping reports into one entry for the defect that remains. If an
  active `TASKS.md` item already owns the repair and its acceptance checks,
  add the structural evidence there.
- Whoever fixes it removes the entry when the fix lands. Remove a stale report
  after checking that its code or premise is gone; keep no log of finished
  work, and do not mistake deleting a report for implementing its feature.

```markdown
### <one line naming the concern>
- Category: <one of the ten below>
- Where: <path:line, crate, or tool>
- Saw: <what is there, concretely>
- Cost: <who it misleads or what it makes expensive>
- Reported: <date>, <agent>, while <what you were doing>
```

## Categories

**Structure**

1. **One idea, several implementations.** Your fix had to be applied in more
   than one place to work.
2. **Knowledge crossing a boundary.** A stage imports something other than its
   input representation, its output representation or a foundation crate.
3. **Branch per shape.** Logic keyed on where a value sits (local, field,
   parameter, match arm) instead of what it is.
4. **Re-deriving.** A later stage recomputes what an earlier one knew, or a
   stage checks its own output again.
5. **Too big to read.** You had to search the file because you could not read
   it.

**Honesty**

6. **A verdict with nothing behind it.** Something is accepted by default, by
   a fallback, or because a lookup returned nothing.
7. **Tests that pin the bug.** A fixture that would still pass if the feature
   were broken, or a check that quietly stopped running.
8. **Zombie machinery.** Code that only tests call, a retired path that still
   receives input, a fallback nobody can remove.

**Direction**

9. **The documents disagree.** You can cite a rule for either answer, or a
   document approves code that is decaying.
10. **Verifying costs more than writing.** You waited longer than you worked.

## Entries

### A callee's write through a `&mut` held inside a lent record leaves the caller's facts about the referent standing
- Category: One idea, several implementations
- Where: [call logs only argument places](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places/log.rs#L582), [Referent frame falls back to the argument](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/storage.rs#L2248), [cleanup follows carried references](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/owned_cleanup.rs#L393), [same-machine test only](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/stored_references.rs#L296)
- Saw: [mutation frames](wiki/spec/language/dependent_values.md) says a caller's fact survives a call only "when its storage dependencies are provably disjoint from the frame, aliases included", and [store vocabulary](wiki/spec/terminal-psi/structural_access.md#L700) says referent backing survives callee frames "so a completed write remains visible to the caller". `Storage::stored_by` logs a call's write only on each `&mut` argument's own place, and `call_write_regions` returns `None` when the frame crosses a `Referent`, falling back to that place. With `machine update(carrier: &mut Carrier, incoming: u64) { carrier.cell.value = incoming; }`, `let mut carrier = Carrier { cell: &mut cell }; update(&mut carrier, 73); need(cell.value);` proves `need`'s `requires value == 5` from the literal, while `cell.value` is 73 at run time. `owned_cleanup::writes` already walks carried references, but only for cleanup calls. Traced, not run.
- Cost: The checker accepts a call whose precondition is false at run time. Log a write row for each referent backing a call can reach through lent or moved values carrying `&mut`, reusing the cleanup walk, and pin it with the in-callee twin of `a_store_through_a_stored_reference_ends_facts_about_the_original_cell`.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/dependent_values.md

### Matching on storage lent to a live `&mut` is not checked against the loan, though copying it is refused
- Category: One idea, several implementations
- Where: [loan check walks operations](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places/loans.rs#L1076), [then cleanup only](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places/loans.rs#L1195), [match subject checked for availability only](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/plain_affine.rs#L1919)
- Saw: [borrows and aliases](wiki/spec/language/ownership.md) says exclusive children "suspend the corresponding parent branch". `uses_retained_loan` visits operations and cleanup actions but no block-ending instruction (`StructuralCase` dispatch, `EliminateEmpty`, `ReturnStructural`, owned `Jump`/`InvokeNoNormal` arguments). With `holder` holding `&mut engine`, `let moved: Mode = engine.mode;` is refused with `ObserveWhileExclusive`, but `transition engine.mode { ... }` reading the same storage while `holder` is still used is not judged. Whether lowering keeps the loan live at the dispatch was not confirmed.
- Cost: The same observation is refused as a copy and passes as a `match`. Visit each block-ending instruction in `uses_retained_loan`, counting a dispatch subject as a shared read and owned edge or return arguments as changes.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/ownership.md

### A read through a runtime index gets a field equation the spec says it never supplies
- Category: The documents disagree
- Where: [read leaf](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs#L2251), [read_equation](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs#L3514), [runtime-indexed store logged as value](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places/log.rs#L479)
- Saw: [store vocabulary](wiki/spec/terminal-psi/structural_access.md#L556) says a runtime-element read "supplies no field equation", and such a store "publishes no field equation" (L639). The write log records a store at `[RuntimeIndex i]` as `Stored::Value(v)`, and `read_equation` gives a later read of the same path `r == v`, so `a[i] = 7; need7(a[i]);` is accepted. This is sound (same selector, overlap-based retirement), but contradicts the page.
- Cost: A change made to match the spec would silently refuse programs that pass today. Either allow the equation for an identical selector in the spec, or drop it for runtime-index paths.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/terminal-psi/structural_access.md

### Inside a `match` arm, `x.p` returns the payload from before the arm even after the arm changes `x`
- Category: Re-deriving
- Where: [read_payload uses the dispatch-time parameter](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower/places.rs#L3624), [select creates the parameters](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower/places.rs#L3477), [selected stack push and pop](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower/control.rs#L370)
- Saw: [store vocabulary](wiki/spec/terminal-psi/structural_access.md) says "A read is not equality with the initializer or a previous read: intervening stores and calls can change storage", and [a sum held in a field](wiki/spec/language/patterns.md) keeps ordinary invalidation for payload mutation and case replacement. When a dispatch selects a case, lowering copies each scalar payload field into the arm block's parameters, and `read_payload` returns that parameter for every `x.p` in the arm; nothing clears `selected` on a write or call. In `match s { Slot::Full { .. } -> refill(&mut s) + s.n, ... }` where `refill` stores `Full { n: 9 }`, the program computes 1 instead of 9; if `refill` stores `Empty`, the inactive-case read the checker should refuse never reaches it. Read end to end; not compiled.
- Cost: Psi and native both compute the stale value, so `omega run --both` cannot catch it. Use the dispatch-time value only while no write or exclusive loan has touched the subject, and otherwise emit an ordinary read with a `Case` step for the checker to judge.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/terminal-psi/structural_access.md

### `c = R { .. }` on a record local binds a fresh root, so live loans and cleanup keep the old value
- Category: Branch per shape
- Where: [whole assignment read as Bind](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/statements.rs#L164), [new root bound](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower/control.rs#L314), [bind_record](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower/places.rs#L2795), [scalar locals already handled](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower/control.rs#L301), [pinning test](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/tests.rs#L3230)
- Saw: [store vocabulary](wiki/spec/terminal-psi/structural_access.md) says a whole record assignment replaces the destination "through the ordinary structural replacement operation" and that "Root, field, and indexed destinations do not change construction semantics"; [loans.md](wiki/spec/terminal-psi/loans.md) says reassignment "cannot preserve stale openings". A field destination lowers to `StoreStructuralField` on the existing root, but a whole owned record local becomes `Statement::Bind`, and `bind_record` creates a new root; nothing writes the old one. The scalar branch already avoids this ("replacing only its SSA binding would hide the write from outstanding loans"); the record branch does not. With `let view: &Cell = &c; c = Cell { value: 9 }; observe(view)`, the checker sees no write under the live loan and accepts, and `view` reads 7. A non-copyable displaced value keeps its old home until exit. The test `a_whole_assignment_constructs_its_value_then_names_the_fresh_root` pins the rebinding.
- Cost: A write under a live shared loan is accepted and the reference sees the old value; displaced-value cleanup moves to exit. Lower whole assignment as an empty-path store on the existing root with displaced-value disposal, delete the rebinding route and update the test.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/terminal-psi/structural_access.md

### Build operand lookup falls back to the first same-named declaration when lookup is ambiguous or unimported
- Category: A verdict with nothing behind it
- Where: [descendant fallback](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_targets.rs#L799), [provider-selection fallback](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_targets.rs#L870), [assumed-binding fallback](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_targets.rs#L897), [first child wins](omega-rust/psi/foundation/symbols/src/table.rs#L3265)
- Saw: [resolution](wiki/spec/language/modules.md) says "Ambiguous imported declarations reject. Compiler traversal order cannot select between them." `assign_provider_selection_argument_symbol` and `assign_assumed_binding_argument_symbol` call a lookup that returns `None` for both "not found" and "ambiguous", then fall back to `find_descendant_by_path`, which takes the first root child with that spelling and checks only dependency scope, not import exposure. In a Build file with `use a; use b;` where both declare `boundary trait Clock`, `Build::await_provider<Clock, …>` gets whichever comes first, and a `Clock` the file never imported is also found. Whether later stages re-resolve deferred operations (`select_provider`, `exclude_service`) was not traced.
- Cost: Root-roster order decides a Build provider operand. Delete the fallback, or make it report ambiguity and check import exposure, and refuse unresolved operands.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/modules.md

### Specialization never checks a type parameter's `[copy]`, `[linear]` or `carry` bound against the supplied type
- Category: One idea, several implementations
- Where: [Operand::read](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs#L1095), [named_type](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs#L1537), [generate](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs#L2257), [callable applications refuse bounds](omega-rust/psi/compilation/representations/typed-trees/src/callable_applications.rs#L419)
- Saw: [applications and specialization](wiki/spec/language/generics.md) says "Every application validates the complete argument tuple, including kinds, ranges, and constraints of unused parameters", and [property declarations](wiki/spec/language/ownership.md) says every use of `T` receives the property fixed by the declaration and application. `Operand::read` binds a type argument through `named_type` without reading `TypeParameter::bounds`, `generate` checks only callable binders, and the checker re-checks the generated copy as a closed machine. So `machine keep<T [copy]>(x: T) -> T { x }` called as `keep<Token>(t)` with a non-`[copy]` `Token` is accepted. Three other application paths refuse any non-default bound as unjudged; machine specialization is the one that skips it.
- Cost: A `[copy]` bound in a generic API is not enforced at its callers. Compare the argument's multiplicity and carry with the binder's bounds in `Operand::read`, and refuse bounds it cannot judge, as the other paths do.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/generics.md

### A callback binder declared `crashes none` admits a private callback with inferred crash routes
- Category: A verdict with nothing behind it
- Where: [satisfies_binder crash test](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization/callable_binder.rs#L174), [checker's binder comparison](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/calls.rs#L2359), [binding record](omega-rust/psi/compilation/representations/terminal-psi/src/boundary.rs#L1862)
- Saw: [guarded crashes](wiki/spec/language/effects.md) says "a requirement stating `crashes none` admits only crash-free selections", and private bodies may omit `crashes` to request inference. `satisfies_binder` refuses only when the callback has an authored `crashes` clause, so a private `machine bump(x: u32) -> u32 { x +! 1u32 }`, which publishes an inferred `Trap` route, passes as `const F: machine(x: u32) -> u32 crashes none`. The checker later compares the callee's reach, suspension and blocking with the binder, but `ClosedReachMachineBinding` carries no crash bound, so nothing compares crashes. Read end to end; acceptance through a private wrapper was inferred, not run.
- Cost: `crashes none` on a callback type guarantees nothing. Carry the binder's crash bound into the binding record and compare it with the callee's published routes in the checker, as reach is compared.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/effects.md

### Domain lookup silently prefers the same-module or same-package domain over visible competitors
- Category: One idea, several implementations
- Where: [Psi 02 prefer_module_local_domain](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols.rs#L1805), [Psi 02 operator homes](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs#L2826), [Psi 02 preparation](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs#L649), [Psi 03 prefer_local_domain](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs#L459), [pinning test](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection_domain_exposure_tests.rs#L209)
- Saw: [resolution](wiki/spec/language/modules.md) says distinct visible declarations competing for the same carrier-qualified domain "reject. Neither carrier ownership nor import order selects a winner", and [visibility and foreign declarations](wiki/spec/language/domains.md) repeats it. Psi 02's `prefer_module_local_domain` (used by membership, type constraints, operator homes and preparation) keeps only the referencing module's candidates whenever any exist, and Psi 03's `prefer_local_domain` keeps the module's, then the package's. In `module one; use two; machine probe(value: u64 in Mark)` with `public domain u64::Mark` in both `one` and `two`, the test `domain_exposure_operator_home_preserves_ambiguity_and_local_precedence` asserts that `one`'s domain is selected; the spec requires a refusal.
- Cost: Adding a domain to a module silently changes what an imported domain spelling means in its sibling files, with different predicates and authority. Delete both local tiers, refuse two visible candidates naming both owners, and flip the test.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/modules.md

### `terminates by (a, b)` without a view becomes `Nat::BoundedDistance` because it has two subjects
- Category: Tests that pin the bug
- Where: [pending_ranking_default](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L208), [test](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/ranking_defaults_tests.rs#L48), [Psi 05 README](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/README.md#L575)
- Saw: [ranking](wiki/spec/language/termination.md) says the short form without `-> View` "requires a carrier-declared stable canonical default", writes the pair example with an explicit `-> Nat::BoundedDistance`, and lists lexicographic product as another view. `pending_ranking_default` returns `NAT_BOUNDED_DISTANCE` for any two-subject witness before looking at either type, and `ranking_defaults_tests.rs` pins `terminates by (index, limit)` to it.
- Cost: An author's `terminates by (i, j)` silently becomes a bounded-distance ranking, and adding lexicographic products later would change what existing code means. Refuse multi-subject witnesses without a view and pin that refusal.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/termination.md

### Three kernel rules accept more forms than their audited rows say
- Category: The documents disagree
- Where: [open-term fallback](omega-rust/psi/compilation/proof-admission/src/integer_rules.rs#L224), [exact-op lift](omega-rust/psi/compilation/proof-admission/src/kernel.rs#L252), [strict carrier bound](omega-rust/psi/compilation/proof-admission/src/kernel.rs#L45), [open discreteness](omega-rust/psi/compilation/proof-admission/src/proof.rs#L1634), [inward discreteness](omega-rust/psi/compilation/proof-admission/src/proof.rs#L1666)
- Saw: [classicality.md](wiki/spec/proofs/classicality.md) and [integer_certificates.md](wiki/spec/terminal-psi/integer_certificates.md) describe narrower rules than the code accepts:
  - `Primitive(ClosedIntegerRelation)` is documented as re-deciding "closed integer relations ... over closed data", but `compare_integer_math_terms` falls back to ring normalization of open terms, so it accepts `x < x + 1` and open distributivity with no premises, and it lifts `ExactIntegerAdd`, multiply and shift-left into total mathematical operations, so u8 `x < x +! 1` is accepted although `x +! 1` traps at 255.
  - `IntegerCarrierBound` is documented as "Exactly `MIN <= value` or `value <= MAX` ... No strict, compound, or narrower bound", but it also accepts an address `a < R` where `R` evaluates to `MAX + 1`, including compound and open terms.
  - `IntegerOrderDiscreteness` is documented as an outward literal-adjacency step with no address carrier, but it also accepts the inward `a < c ⊢ a <= c - 1` and open `a < b ⊢ a + 1 <= b` with no literal or carrier check.
- Cost: Reviewers checking the rows approve a smaller trusted surface than the kernel has; the extra forms appear sound but are unaudited, and the discreteness ones also skip the core. Document every accepted form with its core status, or restrict the code to the rows; decide explicitly whether exact operations are read as total.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/proofs/classicality.md

### The default macOS arm64 calling plan uses 8-byte stack slots where Apple packs arguments
- Category: One idea, several implementations
- Where: [Mach-O arm64 maps to Aapcs64](omega-rust/omega/compilation/backend/calling-conventions/src/plans/vocabulary.rs#L524), [scalar stack placement](omega-rust/omega/compilation/backend/calling-conventions/src/plans/call_plan_evaluation.rs#L754), [import binding](omega-rust/omega/compilation/realization/provider-planning/src/calling_policy_plans/selected_application.rs#L273), [std's Darwin policy](source/library/std/darwin_calling.omg#L182)
- Saw: [authorship and evaluation](wiki/spec/build/calling_plans.md#L14) says the selected plan "must explicitly support the argument/result's representation, fit and placement; otherwise reject the binding". Apple arm64 places stack arguments at natural alignment with no 8-byte minimum, and `std/darwin_calling.omg` implements that. A default-ABI boundary on macos_arm64 instead gets the generic `Aapcs64` evaluator, which advances by `byte_size.max(8)`: for `f(a0..a7: u64, b: u32, c: u32)` Omega writes `c` at sp+8 while an Apple callee reads sp+4.
- Cost: Default-ABI imports and callbacks with more than eight integer arguments, any narrower than 8 bytes, pass wrong values on macOS, and the two Darwin implementations disagree. Give Mach-O arm64 its own packed policy row (or refuse such signatures) and test the 9th and 10th `u32`.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/calling_plans.md

### The Psi interpreter compares and offsets host-returned addresses that the spec says it must refuse
- Category: A verdict with nothing behind it
- Where: [integer equality](omega-rust/psi/interpreter/src/engine_execution.rs#L3598), [integer order](omega-rust/psi/interpreter/src/engine_execution.rs#L3612), [compare_with](omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs#L627), [address subtraction](omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs#L385), [address cast](omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs#L419), [host address result](omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/native_provider/original_homes.rs#L205), [host address ledger](omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/allocation.rs#L115)
- Saw: [host result validation](wiki/spec/terminal-psi/boundary_calls.md#L230) says the interpreter "never dereferences or offsets" a host-returned `addr` and "refuses any comparison or arithmetic that combines it with an interpreter-created address". Host provenance lives only in the terminal-runtime ledger, read only when an address goes back to a native import. `host_address_result` stores the value as a plain integer, and `IntegerEqual`, `IntegerLessThan`, `ExactIntegerAdd`, `ExactIntegerSubtract` and `IntegerExactCast` compute results on it, refusing only on a carrier mismatch. With `p` from a native import and `q = address_of(&local)`, `p < q` runs in Psi, ordering a stand-in identity against a real pointer, and `p == address_of(&x)` can be true in Psi alone.
- Cost: `omega run --both` can report agreement or disagreement on a comparison with no defined meaning across the two runs. Carry host provenance on the scalar and refuse mixed comparison, arithmetic and offsets in the core operations.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/terminal-psi/boundary_calls.md

### A shared byte view of an array gives its elements different addresses in the Psi interpreter
- Category: Re-deriving
- Where: [shared view is a snapshot](omega-rust/psi/interpreter/src/boundary_inputs.rs#L1403), [shared reborrow snapshot](omega-rust/psi/interpreter/src/storage_bytes_and_views.rs#L1017), [fresh namespace](omega-rust/psi/interpreter/src/storage_bytes_and_views.rs#L3003), [view element address](omega-rust/psi/interpreter/src/storage_bytes_and_views.rs#L3077), [array element address](omega-rust/psi/interpreter/src/storage_primitives.rs#L6016)
- Saw: [address exposure](wiki/spec/language/counts_and_addresses.md#L35) says compatible views of the same location "expose the same address", and [interpreted address identities](wiki/spec/language/counts_and_addresses.md#L107) says changing a view's type "cannot change the referent's address". A shared view `&bytes[..]` of a `[u8; N]` array, and a shared reborrow of a mutable view, copy the bytes into `ByteSequenceView::new`, which draws a new exposure namespace; `address_of(&view[i])` encodes that namespace while `address_of(&bytes[i])` encodes the array's identity. `var bytes: [u8; 4]; let view = &bytes[..]; exit_process(if address_of(&bytes[0]) == address_of(&view[0]) { 0 } else { 1 });` exits 0 natively and 1 under Psi. Traced in the interpreter; the lowering of `address_of` on a view place was not confirmed.
- Cost: Psi gives the wrong answer on a well-formed program. Compute view-element addresses from the `OriginalByteArrayWindow` the view already carries. TASKS.md ADDRESS-EXPOSURE owns completing `address_of` and may absorb this; this is a wrong answer from the part already in use, not a refusal.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/counts_and_addresses.md

### `Clock::sleep` promises it never blocks a worker, but every provider blocks for the whole wait
- Category: A verdict with nothing behind it
- Where: [Clock::sleep](source/library/std/clock.omg#L10), [LinuxClockWait::poll](source/library/std/targets/linux_x86_64/clock_impl.omg#L15), [LinuxX64TimeRead::poll](source/library/std/targets/linux_x86_64/time_host_impl.omg#L39), [Windows Clock binding](source/library/std/build.omg#L104), [WindowsTimeRead::sleep](source/library/std/targets/windows_x86_64/time_host_impl.omg#L51)
- Saw: [implementation boundaries](wiki/spec/language/effects.md) says that on published surfaces "omitted `blocks` means never blocks a worker". `Clock::sleep` omits `blocks` although its comment says "Block the caller for `milliseconds`", and so do its raw waits `LinuxClockWait::poll`, `LinuxArmClockWait::nanosleep` and `DarwinClockWait::poll`. The same native calls are bound elsewhere with `blocks`: syscall 7 is also `LinuxX64TimeRead::poll` (entered as `block ...`), and kernel32 `Sleep` is also `WindowsTimeRead::sleep`.
- Cost: The task runtime and synchronization checks read `blocks` to decide whether a call can hold a worker; `Clock::sleep(60000)` holds one for a minute with no evidence. Declare `blocks` on `Clock::sleep` and its three raw waits, as `TimeHost::sleep` does, and mark the call sites with `block`.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/effects.md

### On Windows, sleeping for `u32::MAX` milliseconds never returns
- Category: One idea, several implementations
- Where: [Windows Clock binding](source/library/std/build.omg#L104), [Windows TimeHost sleep](source/library/std/targets/windows_x86_64/time_host_impl.omg#L119), [POSIX split of large waits](source/library/std/targets/linux_x86_64/clock_impl.omg#L30), [sleep_for saturates](source/library/std/time.omg#L1065)
- Saw: [three layers](wiki/spec/build/platform_services.md) requires a portable service to mean the same thing on every target. The Linux and Darwin providers split a count above `i32::MAX` into finite waits, so `sleep(4294967295)` returns after about 49.7 days. Windows binds `Clock::sleep` and `WindowsTimeRead::sleep` straight to `Sleep(DWORD)`, where 0xFFFFFFFF is `INFINITE`. `sleep_for` saturates any duration of 49.7 days or more to exactly that value, and its comment says the saturated request "is RETURNED so the caller can observe it".
- Cost: A long sleep hangs forever on Windows only. Give Windows a checked adapter that never passes 0xFFFFFFFF, for example by splitting the wait as the POSIX providers do.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/platform_services.md

### POSIX and Windows filesystem adapters give five portable calls different meanings
- Category: One idea, several implementations
- Where: [Windows open_with](source/library/std/targets/windows_x86_64/filesystem/handles.omg#L30), [POSIX open_with](source/library/std/posix_filesystem_adapter.omg#L1191), [POSIX Default permissions](source/library/std/posix_filesystem_adapter.omg#L1009), [Windows Default permissions](source/library/std/targets/windows_x86_64/filesystem/metadata.omg#L161), [POSIX removal budget](source/library/std/posix_filesystem_adapter.omg#L1477), [POSIX removal stack](source/library/std/posix_filesystem_adapter.omg#L422), [POSIX create_dir_all prefix](source/library/std/posix_filesystem_adapter.omg#L919), [Windows read](source/library/std/targets/windows_x86_64/filesystem/handles.omg#L110), [Linux read clamp](source/library/std/targets/linux_x86_64/posix_filesystem_impl.omg#L148)
- Saw: [three layers](wiki/spec/build/platform_services.md) says the portable service "is identical on every target" and all translation to the OS shape happens in the adapter. Each family decides these cases itself:
  - `open_with` with `truncate` and no write access (or with `append`): Windows refuses with `InvalidInput`; POSIX encodes `O_RDONLY|O_TRUNC` and empties the file.
  - `set_permissions(p, Permissions::Default)`: Windows only clears read-only; POSIX calls `chmod(p, 0o666)`, which strips execute, set-ID and sticky bits (a 0755 script stops running, a directory loses its search bit), unlike its own `Readonly { value: false }` path.
  - `remove_dir_all`: POSIX stops after 4096 steps or 16 levels, after deleting part of the tree, with `Other`; Windows has no budget.
  - `create_dir_all`: POSIX skips any parent prefix ending at byte 255 or later and ignores every intermediate `mkdir` failure; Windows creates every prefix and stops at the first real error.
  - `read`, `read_all` and `copy` with a count larger than the buffer: Windows refuses with `InvalidInput`; POSIX clamps to the buffer.
- Cost: The same call is harmless on one target and destroys data or half-finishes on another. Decide each rule once in the portable layer and make both families follow it.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/platform_services.md

### POSIX std reads `errno` by `memcpy` from a raw address and declares it cannot crash
- Category: A verdict with nothing behind it
- Where: [copy_error](source/library/std/posix_filesystem_adapter.omg#L1813), [posix_canonical_error](source/library/std/posix_filesystem_adapter.omg#L1875), [c_copy_from_address](source/library/std/targets/macos_arm64/posix_filesystem_impl.omg#L156), [darwin_failure_i32](source/library/std/targets/macos_arm64/posix_filesystem_impl.omg#L161), [memcpy bindings](source/library/std/build.omg#L359)
- Saw: [inert addresses and marshaling](wiki/spec/build/foreign_storage.md) says `addr` is numerical data that ordinary code cannot dereference, and a binding materializes a pointer "only from established storage claims"; thread-local storage must be copied or mediated by such a claim. std declares `copy_error(destination, source: u64, count)` and `c_copy_from_address` as `reaches none crashes none`, binds both to `memcpy`, and passes the bare `u64` from `__errno_location`/`___error` as the source. That is a general read of any address, and `source = 0` faults. The two copies also decode differently: one assembles four bytes, the other keeps only `code[0]`.
- Cost: Reviewers see a crash-free boundary that is really an unchecked dereference. Provide one shared errno requirement whose provider returns the `int`, or an admitted view of the thread-local slot, for both callers.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/foreign_storage.md

### The Omega-written compiler lets a case pattern with a `field: value` test cover the whole case
- Category: A verdict with nothing behind it
- Where: [case_pattern](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/coverage.omg#L206), [valued field typing](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/statements.omg#L484)
- Saw: [patterns.md](wiki/spec/language/patterns.md#L30) says "`field: value` is ordinary projected equality", so `Reading::Value { level: 0 }` matches only some `Value` readings. `case_pattern` checks only that the arm names the case and ignores the pattern's fields, so `transition r { Reading::Value { level: 0 } -> 0  Reading::Missing -> 1 }` passes coverage and `Value { level: 5 }` matches nothing.
- Cost: The coverage verdict has nothing behind it. Only an arm whose fields are all binders or `as _` should close its case.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/patterns.md

### Omega-written typing re-scans integer literals from source bytes
- Category: Re-deriving
- Where: [literal_value](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/literals.omg#L15), [suffix_carrier](source/psi/pipeline/03_symbol-resolved-trees-to-typed-trees/literals.omg#L180), [lexer](source/psi/pipeline/00_source_files_to_tokens.omg#L944), [token kind](source/psi/representations/tokens.omg#L109)
- Saw: AGENTS.md says stages consume their declared input representation and must not bypass the pipeline with "source re-analysis". The lexer already records each integer literal's base, digit run and suffix presence in `TokenKind::Integer`; stage 03's `literal_value` reopens the source bytes at the span, detects the base again, re-scans digits and separators, and matches the suffix against eight spellings.
- Cost: Two scanners must agree on literal grammar, so a change to one (such as the spec's `u13` suffixes) can split them silently. Decode value and carrier once in the lexer or parser and carry them forward.
- Reported: 2026-11-16, Claude Code, while auditing code against omega-rust/pipeline.md

### The compiler has no `InternalFailure` outcome, so a defect it detects in itself exits as "source rejected"
- Category: A verdict with nothing behind it
- Where: [outcome enum](omega-rust/omega/compilation/products/src/compile_outcomes.rs#L109), [non-diagnostic errors become Reject](omega-rust/omega/build-system/orchestration/src/lib.rs#L233), [validator failure called a compiler defect](omega-rust/omega/build-system/orchestration/src/lib.rs#L215), [exit status](omega-rust/omega/application/src/commands/compile.rs#L244), [reserved-codes comment](omega-rust/omega/application/src/arguments/mod.rs#L33)
- Saw: [outcomes](wiki/spec/build/compiler_request.md#L249) says compilation "preserves four outcome distinctions", including `InternalFailure` for "a compiler/checker malfunction or violated implementation invariant", with halt value 3; [independent stage validators](wiki/spec/build/command_line.md) says to "distinguish compiler invariant failures from invalid authored source". `CompilerRequestOutcome` has only `Complete`, `Reject` and `Incomplete`, and `BuildError::outcome` maps every non-diagnostic error (`StageValidation`, `Publication`, `BuildDirectory`, `CheckWorkspace`) to `Reject`. A failing `omega --validator TargetOperationCorrespondence main.omg` prints "this is a compiler defect, not an error in the source" and exits 1. The Omega-written compiler has the case (`source/omega/application/ocreq/outcome.omg`).
- Cost: Scripts reading `$?` cannot tell a compiler bug from a rejected program. Add `InternalFailure` (exit 3), classify compiler-invariant errors as it, and exit with it.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/compiler_request.md

### `x == S::C` on a payload-bearing sum becomes a bare tag test when Psi 03 cannot type `x`
- Category: One idea, several implementations
- Where: [untyped comparison left in place](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_preparation.rs#L217), [no type for a receiver call](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/typing_queries.rs#L1425), [read as a case test](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/expressions.rs#L500), [case test without payload check](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/places.rs#L1412)
- Saw: conformances.md says "Records and payload-bearing sums require an explicit named conformance", and data_and_literals.md forbids "silently changing tag equality into field equality". Psi 03 enforces that only when `DeclaredExpressionTypes` types both operands; otherwise it `continue`s, and the query returns `None` for receiver calls, calls with machine arguments and `match` expressions. Psi 05 then reads any `==` whose right side names a case as `IsCase`, without checking for payload cases or a selected `Equatable`. With `data Cmd { Stop; Move { dx: i32; } }`, `Codecs::decode(&bytes) == Cmd::Stop` compiles as a tag test while `let c: Cmd = Codecs::decode(&bytes); c == Cmd::Stop` is refused (or calls the custom `Equatable<Cmd>`).
- Cost: The compiler accepts an equality the spec refuses, and `==` means different things depending on how the operand is spelled. Let Psi 05 read `==` as a case test only for payload-free sums, or have Psi 03 refuse a comparison it cannot type.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/language/conformances.md

### The test runner cannot run, because Build admission refuses every test group
- Category: Zombie machinery
- Where: [registration refused](omega-rust/psi/compilation/build-time-evaluation/src/product/product_declarations.rs#L48), [groups always empty](omega-rust/psi/compilation/build-time-evaluation/src/product/product_declarations.rs#L65), [test discovery](omega-rust/omega/compilation/compiler/src/checked/test_roots.rs#L74), [native verdict](omega-rust/omega/build-system/orchestration/src/compile/test_execution.rs#L260), [std slot found by package name](omega-rust/omega/compilation/compiler/src/checked/test_roots.rs#L40)
- Saw: `product_declarations::admit` refuses any Build that uses `TestGroupRegistration` or `TestGroupService`, and otherwise returns `test_groups: Vec::new()`. So `select_test_roots`, `test_execution::run_tests` and the private-filesystem runner that [TEST-GROUP-FILESYSTEM-TARGETS](TASKS.md#L308) describes as working cannot be reached; no fixture registers a test group. The unreachable code also departs from [testing.md](wiki/spec/build/testing.md): `execute_native` passes any test whose process exits 0, although the outcome table says a process exit instead of returning to the runner is "Not a pass"; and `group_filesystem` recognizes the std `FilesystemService` slot by the package name `omega_language_std`, although [declarations.md](wiki/spec/build/declarations.md#L151) says a package named `std` cannot confer compiler custody.
- Cost: Readers of the runner and TASKS.md assume test groups run; admitting registration later makes both latent bugs live. Deliver group admission with both bugs fixed, or say on the TASKS item that registration is refused and own both bugs there.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/testing.md

### Default compilation re-checks the image it just wrote, which one spec page requires and two forbid
- Category: The documents disagree
- Where: [default image emission](omega-rust/omega/compilation/backend/images/image-emission/src/image.rs#L1851), [validate_terminal_image](omega-rust/omega/compilation/backend/images/image-emission/src/image.rs#L803), [machine-state evidence](wiki/spec/build/machine_state_evidence.md#L24), [verification placement](omega-rust/pipeline.md#L65), [optimizations](wiki/spec/build/optimizations.md#L59)
- Saw: machine_state_evidence.md says "Final-artifact validation is mandatory". pipeline.md says a compile "does not revalidate, replay or rebuild output the same compile just produced", and optimizations.md repeats it. Every ordinary native build calls `validate_terminal_image`, which re-checks import and relocation counts, region inventories and literal-pool bytes, and decodes x86 and AArch64 address fields from the final bytes to compare them with the object plan, without any `--validator` flag.
- Cost: Agents can cite one rule to delete this work and another to extend it, and every native build pays for it. Decide whether final-artifact validation is an exception to verification placement; then move the checks behind `--validator` or write the exception into pipeline.md and optimizations.md.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/machine_state_evidence.md

### `Build.freestanding` is one untargeted mutable flag that two spec pages forbid and a third relies on
- Category: The documents disagree
- Where: [Build declaration](omega-rust/omega/packages/build-declarations/src/prelude.rs#L269), [settings field](omega-rust/omega/packages/package-compilation/src/build_execution/build_value/configuration.rs#L22), [assembly authority](omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs#L243), [configuration.md](wiki/spec/build/configuration.md#L53), [uefi_entry.md](wiki/spec/build/uefi_entry.md#L3), [permissions.md](wiki/spec/build/permissions.md#L233)
- Saw: configuration.md says the hosted/freestanding environment belongs to the target profile, "not duplicated mutable Build flags", and that grants are rows keyed by target; uefi_entry.md calls freestanding "an immutable selected target/provider property". permissions.md instead names "the evaluated `Build.freestanding` selection" as machine-owner admission, and the code follows it: source checking turns the single untargeted bool into `AsmAuthorityAdmission::MACHINE_OWNER` for the whole program. With `--target uefi_x86_64 --target linux_x86_64`, machine-control assembly is admitted for the hosted Linux realization too. The field comment ("no ambient host packages or import thunks") describes behavior the flag does not have.
- Cost: Agents following different pages delete or extend the field. Settle the page, then make the grant a per-target row or rewrite configuration.md and uefi_entry.md, and correct the comment.
- Reported: 2026-11-16, Claude Code, while auditing code against wiki/spec/build/configuration.md

### Legalization replaces its failure causes with a bare custody error
- Category: Verifying costs more than writing
- Where: [scalar graph](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/native-preparation/src/legalization_scalar_graph.rs#L1084) and four sibling `legalization_*.rs` files in `native-preparation`
- Saw: 39 calls to `map_err(|_| LegalizationError::custody())` drop the underlying error. The result records the Rust call site and nothing about which argument, storage or result failed.
- Cost: Each new legalization refusal needs a diagnostic build to find its cause. Keep the cause in the error; this changes no accepted program.
- Reported: 2026-11-16, Claude Code, while checking a backend review against the spec

### Callable analysis hides its storage-flow refusal
- Category: Verifying costs more than writing
- Where: [callable projection](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/callable_values/terminal.rs#L124)
- Saw: `terminal_callable_values` replaces the reference graph's error with `callable provenance cannot reconstruct current Terminal storage flow`. The unchanged [indexed-case program](tests/fixtures/run/calls/indexed_case_discriminator/main.omg) produces that refusal before native compilation on both current-form source `70dfe3f` and clean landed base `d3deb345`; the underlying graph error is lost.
- Cost: A caller cannot name the failed operation or storage relationship without another diagnostic build. Preserve the graph's reason through the callable query and its callers; keep the program and its oracle unchanged.
- Reported: 2026-11-16, Codex, while qualifying current-form frame publication

### Changing storage analysis rebuilds the native compiler
- Category: Verifying costs more than writing
- Where: [semantic analysis package](omega-rust/psi/compilation/representations/terminal-psi/semantics/Cargo.toml), [declaration lookup in optimization](omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/structural_places.rs#L619), [target descriptor inputs](omega-rust/omega/compilation/representations/target-operations/target-vocabulary/src/descriptor_input.rs)
- Saw: Changing only the private descriptor replay cache and its error reporting rebuilt 64 packages and took 14m 17s before the original Console test could run. Some native consumers need only declaration or structural-path lookups from the same package; ownership analysis and explicit image validators genuinely need the full analysis. `target-operations` also declares an unused direct dependency, although removing it alone leaves a transitive dependency. Source and build evidence remains in claim `18e85a23de177c780000000018e83683`, under `post-3776-original-binding-control-49/native-terminal-semantics-coupling.json`.
- Cost: Edits to the analysis invalidate native consumers of small lookup functions. Move the declaration and path queries out of the full analysis crate, keep each judgment implemented once, and remove unused direct dependencies. Measure the same Console edit/build/run loop before claiming a build-time improvement.
- Reported: 2026-11-16, Codex, while carrying the original Console program through checking and execution

### Source correspondence repeats signature searches
- Category: Verifying costs more than writing
- Where: [source correspondence](omega-rust/psi/compilation/representations/typed-trees/src/service_declarations.rs#L497)
- Saw: For each projected method, correspondence validation scans every inherited signature and derives its callable identity before interning it. Finding one match still exhausts the scan. The original Console binding replay43 took 329.112s before its next refusal. A brief sample caught this work, but captured only two samples; its share of total time is unknown. Raw evidence remains in claim `18e85a23de177c780000000018e83683`.
- Cost: Repeated identity normalization grows with both the method and signature counts. Prepare the exact callable/package identities once per inherited declaration set. Preserve source authentication, complete applications, and refusals for missing or ambiguous requirements. Validate with the unchanged Console program and refusal controls; measure the identity derivations before claiming a speedup.
- Reported: 2026-11-16, Codex, while carrying the original Console binding and stored-machine programs through checking and execution

### Every native caller evaluates constants between two driver calls
- Category: one idea, several implementations
- Where: [native completion](omega-rust/omega/build-system/orchestration/src/compile/native_completion.rs#L123), [scalar completion](omega-rust/omega/compilation/native-compiler/src/scalar_completion.rs), [Build's completer](omega-rust/omega/build-system/build-settings/src/activation/scalar_completion.rs#L20)
- Saw: Omega 00 now runs whole inside `native_compiler::compile`, but callers still call `native_compiler::prepare`, run `build_settings::scalar_completion::complete` on every `NativeScalarCompletion`, then call `compile`. Preparation reads only the retained Terminal product and its native proposal (`TerminalNativeRealizationProposal::prepare_native_execution`, `PreparedNativeExecution::scalar_application_demand`). Product checking separately completes the same artifact's conditions with an empty demand (`ProducedTerminalProduct::check_conditions`).
- Cost: Orchestration, the review example and compiler tests each repeat the prepare, evaluate, compile sequence, and conditions are evaluated twice. Select the execution and complete its demand once where the retained native product is formed, pass the completed scalars in `NativeCompileRequest`, and delete `NativeScalarCompletion` and the split entry.
- Reported: 2026-11-16, pi, while checking that only the native driver calls Omega stages.

### Emission libraries keep earlier stage outputs beside the final layout
- Category: knowledge crossing a boundary
- Where: [fragment source](omega-rust/omega/compilation/backend/machine-emission/src/fragment_emission.rs#L211), [driver join](omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L227), [object accessors](omega-rust/omega/compilation/backend/object-file/src/fragment_container.rs#L204), [runtime-only route](omega-rust/omega/compilation/native-compiler/src/realization.rs#L172), [stage-call exceptions](tools/xtask/src/boundary.rs)
- Saw: `StagedOptimizedFunctionFragmentEmissionSource::new` joins the stage 09 realization with stage 02's whole output, and frame staging between stages 06 and 07 runs in machine-emission on types defined inside stage crates. The source keeps that `Arc<ValidatedOptimizedTargetOperations>` on every compile; image and object consumers can still reach earlier programs through `optimized_target()`, `lowering_facts()` and `source().source().source()`. Function-realization replay is now opt-in (`retain_replay_for_validation`); this field is not. [pipeline.md](omega-rust/pipeline.md#portable-materialization-and-consumer-ownership) keeps producer history "off the path ordinary consumers walk to get current data". `admit_terminal_runtime` runs stage 01, then calls stage 02's `capture_runtime_borrowed_storage_layouts` on the side. `native-artifact` reads Omega 00's `VerifiedNativeArtifactInput` for runtime provisions.
- Cost: Emission can read anything an earlier stage knew, so a missing fact never has to be carried forward. Move the stage output types emission names into `compilation/representations`, carry the rows emission needs in each stage's output, and delete each `STAGE_CALL_ALLOWANCES` row as its edge goes.
- Reported: 2026-11-16, pi, while checking that only the native driver calls Omega stages.

### Borrowed field recasts lose the target carrier layout of the enclosing record
- Category: re-deriving
- Where: [reference view geometry](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references/representation_geometry.rs#L147), [sum geometry](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs#L4450)
- Saw: A recast checks the enclosing record's alignment, but its geometry query has no carrier layout for a sibling sum. The checker supplies the retained target's record resolver without its case-tag layout. An exact `PosixFilesystem` reduction refuses a whole aligned 144-byte field. Changing only the sibling `ErrorKind` field to `u32` yields native and Psi exit 0 on macOS arm64.
- Cost: The standard-library POSIX stat decoders still refuse the whole buffer. Pass the existing target carrier layout to projected storage checks, preserve the short and misaligned refusals, and replay the unchanged [note vault](https://github.com/CathedralOS/Omega/pull/3722).
- Reported: 2026-11-15, Codex, while validating shared recasts of an aligned record field.

### Specialization failures discard the reason they stopped
- Category: Verifying costs more than writing
- Where: [specialization](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs)
- Saw: many specialization checks still return `Stop::Unserved(None)`. The Console binding replay reached an anonymous nested-parameter mismatch; adding the missing reason required another optimized compiler build and replay. The current repair gives that mismatch a reason, but adjacent shape, lookup and argument checks still discard theirs.
- Cost: the next failure names only the unavailable application, hiding the failed check. Carry each check's reason through the existing refusal path so one replay identifies the repair, without changing which programs are accepted.
- Reported: 2026-11-15, Codex, while restoring Console preparation for stored-machine execution

### Focused compiler controls spend twenty-two minutes in code generation
- Category: Verifying costs more than writing
- Where: [compiler test targets](omega-rust/omega/compilation/compiler/Cargo.toml#L110), [compiler feature alignment](omega-rust/omega/compilation/compiler/Cargo.toml#L102), and the `coherent-native-controls-24-01` receipt in continuation claim `18e81ab82af520780000000018e96239`
- Saw: On macOS arm64 at `a827cf2c269`, a fixed-source nextest selection of 24 controls across 12 binaries spent 22m17s building before 176.905s of test execution. The command used the retained CLI feature union, optimization level 1, two codegen units and three jobs; 1,219 tests were not selected. Actual compiler processes remained active. After integrating main `a2684962ce882`, 13 selected unit controls across six crates spent 15m49s building and 0.414s running on the same host and budget (`merge-a268-direct-controls-01`, with exact source patch and input records retained in the active claim). The source of this build cost has not been established.
- Cost: The recorded twelve-binary replay spent twenty-two minutes building before exposing the next semantic refusal. A later three-binary replay at c16df641c12 built in 2m51s; that smaller selection does not identify the earlier cost. Measure the actual invalidation and compiled-target work, then repair the responsible build or test ownership while retaining the selected controls and exact-source optimized producer checks.
- Reported: 2026-11-14, Codex, while carrying stored-machine programs through Console and native-binding preparation

### Callback and interrupt records lose the machine that owns their operation
- Category: Re-deriving
- Where: [terminal-psi/src/lib.rs:352](omega-rust/psi/compilation/representations/terminal-psi/src/lib.rs#L352), [executable preparation](omega-rust/omega/compilation/compiler/src/executable.rs), and [codec/src/foundation_validation.rs:2261](omega-rust/psi/compilation/representations/terminal-psi/codec/src/foundation_validation.rs#L2261)
- Saw: each lowered machine allocates operation IDs from its own fresh `MachineSources`, but callback and interrupt records retain only `OperationId`. Executable preparation searches every reached machine for that number, while codec interrupt validation collects all machines into a map keyed by the same bare ID. Stage05 currently emits neither roster; the existing constructors are fixtures and decoding, and no failing customer execution was observed.
- Cost: equal operation numbers in different machines can select an unrelated reached use or overwrite an installation site. Preserve the owning machine with the operation through the record, encoding and consumers so each registration names its actual call.
- Reported: 2026-11-14, Codex, while reviewing stored-machine executable preparation

### Selected-body activation copies retained side records
- Category: re-deriving
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L669), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1295), [owned arena clone](omega-rust/psi/foundation/arena/src/arena.rs#L748)
- Saw: Replacing symbols on a shared typed tree can copy side records, normalized identities and owned containers even with no bodies selected. The [call and machine record repair](https://github.com/CathedralOS/Omega/pull/3747) measured empty activation and two nonempty batches on both Windows x86-64 routes. Empty activation requested 2,182,456 bytes in 12,456 allocations and cloned 36 arenas per route on fixed `570d707` with the repair and observer (opt-level 1). See the [data-child repair](https://github.com/CathedralOS/Omega/pull/3720) for the earlier baseline.
- Cost: Share immutable storage where measurements still show copies. Limit writes to changed records or new batches. Preserve source application preparation, projection, authority checks, and signature and inferred-summary dependencies. Re-measure empty activation and nonempty completion on both [scalar and borrowed-root workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary), counting copied storage and new output separately. See the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.
