# Compound index guard

Owning task: BUILD-OWNED-TARGET-SELECTION, retiring this row's remaining Source
target family while preserving its compound-index requirement. This row preserves one fact and operation:
`0 <= b * 20 + a < 200` permits storing `7` at `grid[b * 20 + a]`.
The index remains a two-variable `i32 in Wrapping` computation and the grid retains
200 elements. The authored runtime coordinates remain `a = 15`, `b = 3`.

The [root](main.omg) and nine places carry the same sentence and operation:
[local](local/main.omg), [field](field/main.omg), [parameter](parameter/main.omg),
[call result](call_result/main.omg), [match join](match_join/main.omg),
[const name](const_name/main.omg), [nested machine](other_state/main.omg),
[another machine](other_machine/main.omg), and
[another target body](target_body/main.omg). Each place has its adjacent
`invalid/main.omg` and corresponding pass/fail pin under `tests/omega/{pass,fail}/facts`.
All positive Build files bind `ProgramEntry` for the four hosted targets and
all positive Sources declare exit zero.

An ordinary `run_main` wrapper preserves the original `hits = 0`, `a = 15`,
`b = 3` setup, in that order, before calling the guarded code. That code has an
unrestricted `&mut Main` parameter and retains the original guard, indexed store,
transport place and failure route. At the target-body place it is an ordinary
named implementation selected by four explicit Build rows. Its public requirement
keeps empty service reach and `Trap`, with no added precondition or postcondition.
Other places use an ordinary guarded helper. Joining the common setup and guarded
body preserves the original complete helper's behavior for every original input.

This separation matters to the witness. When the setup was inside the guarded
body, the checker could legitimately derive index 75 without its upper guard.
A positive clone with only that conjunct removed accepted on retained producer
`f6c2d20ae37b918d6fae1ec256b128b8b8bdc9b8`; this is a valid constant proof, not a
compiler defect. The historical reject Sources also omitted the coordinate
assignments and runtime scaffolding. Those original files and results are
retained. The corrected twins copy the new positive and remove only the strict
upper fact, including its corresponding parameter requirement where that place
already carries it. The independently checked guarded body must justify the
store for arbitrary `Main`, rather than assuming the wrapper's literal 75.

The entry owns fully initialized storage and checks the result after the call.
It observes `hits == 1`, `grid[75] == 7`, and untouched neighboring cells
`grid[74] == 101`, `grid[76] == 107`. The target cell starts at `0x12345667`.
A callee operating on a staged copy, omitting its store, or corrupting those
neighbors cannot satisfy the caller's observations. These runtime checks do not
supply a proof premise for the indexed store.

The match place executes both Boolean routes and writes distinct 17/23 markers
to a separate caller-owned scalar. `mark_coordinate` promises only the same
returned coordinate. The first route and stored value are captured before the
second match/store, so the second execution cannot mask an incorrect first arm
or missing first write. The original match tested only one input with identical
arm results; its old result is historical evidence, not a branch-discrimination
claim.

The [wrong-actual control](controls/call_result_wrong_actual/main.omg) retains the
full guard and changes only the coordinate call's actual from `main.a` to `main.b`.
Its guarded body must refuse: bounds on `b * 20 + a` do not bound `b * 20 + b`.

Applicable requirements:

- **Establishment:** the complete lower and upper conjunction establishes the
  exact wrapping compound index's bounds. The independently checked body has no
  literal-coordinate entry assumption. Removing its upper fact must reject at
  the store; no caller equality guard repairs the missing premise.
- **Identity and lifetime:** the guard and store retain the same scalar/storage
  versions. Calls preserve captured coordinates and expire only affected storage.
  The exclusive `Main` borrow denotes original caller storage; marker writes use
  an independent scalar, and its first snapshot survives the second write.
- **Composition:** fields, parameters, returned values, joins, named constants,
  nested states and ordinary/selected machines transport the same bound into the
  same store. Existing helper requirements retain their complete bound.
- **Soundness:** wrapping arithmetic retains its declared meaning. Array accesses
  still owe bounds and mutable storage authority. Build selects checked code and
  supplies no hidden coordinate premise or permission.
- **Terminal and native realization:** reference identity, array layout, scaled
  indexing, state arrivals and borrowed calls must survive canonical Terminal
  lowering and native execution. Positive acceptance requires actual host
  native/Terminal exits zero and agreement. Foreign artifact production is
  separate from execution; no foreign-host runtime result is inferred.

The fixed warm Source base is
`fffbe8d9f2b3561f922a0156d51985a617bdbe89`, after verified PR #1307 publication.
The original 121 files, 19 public projects and 18 pins were frozen before edits.
On immutable optimized producer `f6c2d20ae37b918d6fae1ec256b128b8b8bdc9b8` and its
own 189-file SDK snapshot, the original pins are nine checked / nine intended
refusals, and all nine runnable pins have native zero / Terminal zero / agreement.
The original root also passes normal public preparation and both engines.
These observations keep their actual producer identity; they are not executions
of the newer Main compiler and do not establish the strengthened runtime oracle.

The migrated target-body prototype with the setup split freshly passed normal
preparation and both engines, and its upperless twin refused the intended index
bound on that same producer. The earlier prototype's missing explicit boundary
`reaches` was an authored setup error, retained separately from compiler evidence.
Whole-row Source qualification on that retained producer checked seven positive
places. The valid call-result and strengthened match places refused:
`cannot prove index main.b * 20 + a is within length 200 in put::entry`.
The direct-copy control checked, while direct-call and saved-input call variants
refused. The frozen trace retained the public result/input equation and a
complete empty write frame.

An actual Source-to-Check test through the compiler fixture facade established
that both guard comparisons, the public scalar equation, and every admitted
wrapping operation were present at the failed private arrival. The numeric
substitution owner published the same equation twice: after `x := y`, its raw
reverse orientation installed `y := x`, leaving equal observations with different
normal forms under bounded expansion. It now normalizes both sides through
established substitutions before choosing an orientation. The existing overflow,
formation and expansion limits remain. This is not a claim of global acyclicity
for arbitrary substitution graphs.

On exact candidate `7331854657d4a7d551695ca1137f8e65141b8262`, all four focused
optimized units pass, both unchanged Source positives check, and both upperless
twins plus the wrong-actual control refuse the intended index bound. These are
Source checking observations, not native execution claims. The first match
attempt's Rust-style dereference assignment was an authored error; its corrected
transparent `route = tag` syntax and later valid-program refusal are retained
separately.

Five baseline preservation Sources are proper (one positive / four refusals),
including exact actuals, stale field snapshots and distinct repeated calls; the
one runnable snapshot control has native/Terminal zero and agreement on the
retained producer. The first full qualification producer is optimized Linux x86-64
`b3f00fd60e7f7abf613fde219da040e8e19fcf42`, integrating published Main
`8d22dd8afbe602cb94cb7087373928c5d4080c97` with the repair. Its 7,948 frozen
compiler/Cargo/SDK inputs include the actual 176-file SDK, independently retained
with hashes. Fresh acceptance establishes:

- All 19 owned pins: nine checked positives and ten intended refusals, including
  the wrong-actual control; all nine runnable pins have native zero / Terminal
  zero / agreement.
- All 20 public projects through ordinary preparation: root plus nine positive
  places have native/Terminal zero and agreement; nine twins and the wrong-actual
  control reject for the index bound.
- The five focused preservation Source tuples and the runnable snapshot's full
  outcome tuple match their recorded baseline. Baseline and fresh producer/host
  identities remain distinct.
- Changing only the selected store from 7 to 8, or only the first match route
  from 17 to 19, produces actual native signal 4 and agreeing Terminal `Trap`.
- Four explicit target-body artifact requests produce PE, two ELF images and
  Mach-O with retained hashes. These artifact observations claim no foreign-host
  execution.

The hosted run is [41390167139](https://github.com/CathedralOS/Omega/actions/runs/41390167139).
Producer preparation took 1,218.899 seconds; Source checking took 64.510 seconds,
pinned execution 37.382 seconds and public execution 148.325 seconds. These are
actual run timings, not a performance improvement claim. Scoped Clippy completed
with pre-existing Main warnings retained and no warnings in the changed Rust
files. The independent Source/oracle review is accepted. A separate raw replay on that
exact b3 CLI and SDK, run [41394395926](https://github.com/CathedralOS/Omega/actions/runs/41394395926),
confirms all ten public positive programs explicitly exit zero in both engines;
a successful preparation process alone is not counted as runtime acceptance.

Actual conflicts required integrating Main
`39a69f97dd6e8b06e0f5da8f01ae826b2b28289d`, whose changed normal-completion,
predicate and layout consumers warranted a fresh affected-closure replay.
Optimized producer `68a9db4f4b0741530272ebd070b385c0e0f2c4e7`, hosted run
[41395511792](https://github.com/CathedralOS/Omega/actions/runs/41395511792),
freshly passes all 23 Source cases (10 checked / 13 intended refusals), all ten
pinned native/Terminal zero agreements, and ordinary public root, target-body
and match-join executions with explicit native/Terminal zero. All 23 checking
and ten runtime full outcome tuples equal the first full qualification. The
two counterfactuals again produce native signal 4 and agreeing Terminal Trap;
four fresh target artifacts retain their actual hashes and formats, with no
foreign execution claim. At that checkpoint, all 7,957 frozen compiler/Cargo/SDK inputs matched the
compiler bytes, including its actual 176 SDK files. Temporary hosted probe
orchestration is removed before landing. Normal CI and exact final semantic
review remain publication gates.

A later actual queue conflict required integrating Main
`e4a61cb4484f0f4bf37c2cde98555018da931aab`. Its Boolean-premise admission and
selected-conjunction consumers warranted another bounded proof check; moved
interpreter frame/storage routes warranted four affected runtime observations.
Actual optimized CLI `08b7c22717e58f422e770c848287927ac9099f64`, run
[41399107774](https://github.com/CathedralOS/Omega/actions/runs/41399107774),
freshly checks all 23 frozen Sources through ordinary review and explicit
checking products: ten positives and thirteen authored refusals. Call-result,
match, selected-target and snapshot preservation pins each explicitly exit zero
in both engines. The same two mutants again produce native signal 4 and agreeing
Terminal Trap. This is a CLI-only consumer check, with no new corpus-runner build
or restamping of historical corpus records. All 7,964 current compiler/Cargo/SDK
inputs are retained; its 176-file SDK includes three allocator/extent changes
whose APIs this row does not consume. Earlier 68a runtime/public results keep
their actual producer identities. The native backend did not change in this
join, so the four 68a artifact receipts remain applicable.

A separate replay on that exact retained CLI and source snapshot, run
[41401034384](https://github.com/CathedralOS/Omega/actions/runs/41401034384),
retains the generated acceptance locks/admissions beside each command's raw
checking and engine output. It performs no Rust build. These historical locks
are replay inputs, not authority to install into a new checkout.

Only this fact's target-body frontier is removed. The remaining places are
discoverable through the reproducible inventory query in `TASKS.md`; its
handwritten aggregate count was removed after repeated actual merge conflicts. Both Build-owned
parent capabilities, broader target-family retirement, other-host runtime, and
mutable retained-completion footprint closure remain open.

Commands and frozen inputs are retained in the existing ignored qualification
directory `build/compound-index-build-selection-20261102`.

Governing contracts: [Build selection](../../../../wiki/spec/build/provider_selection.md),
[index bounds](../../../../wiki/spec/language/counts_and_addresses.md),
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md), and
[fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md).
