# Compound index guard

Owning tasks: BUILD-OWNED-TARGET-SELECTION and the remaining compound-index
acceptance under INDEX-INDUCTIVE-BOUNDS. This row preserves one fact and operation:
`0 <= b * 20 + a < 200` permits storing `7` at `grid[b * 20 + a]`.
The index remains a two-variable `i32 in Wrapping` computation and the grid retains
200 elements. The authored runtime coordinates remain `a = 15`, `b = 3`.

The [root](main.omg) and nine places carry the same sentence and operation:
[local](local/main.omg), [field](field/main.omg), [parameter](parameter/main.omg),
[call result](call_result/main.omg), [match join](match_join/main.omg),
[const name](const_name/main.omg), [nested machine](other_state/main.omg),
[another machine](other_machine/main.omg), and
[another target body](target_body/main.omg). Each place has its adjacent
`invalid/main.omg` and corresponding pass/fail pin under `tests/omega/facts`.
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
Whole-row qualification, counterfactuals, affected-current-compiler applicability,
scoped goldens and final review are pending; the broader parent tasks remain open.

Commands and frozen inputs are retained in the existing ignored qualification
directory `build/compound-index-build-selection-20261102`.

Governing contracts: [Build selection](../../../../wiki/spec/build/provider_selection.md),
[index bounds](../../../../wiki/spec/language/counts_and_addresses.md),
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md), and
[fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md).
