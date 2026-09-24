# Compiler progress

How much of Omega the Rust reference compiler compiles and runs, as fractions
with named denominators. The instrument is `tools/progress.py`; the numbers
here are its output on the revision and host named below. Refresh by re-running
it and replacing the tables wholesale, not by appending a dated copy. Delete
this record once a generated report is published beside the release gates, or
once the corpus rosters carry per-fixture outcomes the boards can cite directly.

Driving a large sample until it stalls reports the first unsupported
construct and nothing past it. That is why Squalr and the other whole
applications have not answered this question: each stops once, at one wall.
The corpus of 2,059 small pass fixtures, each pinning one rule, reports a
coverage fraction per tier and per language surface instead.

Revision `e526ef3f54` (origin/main, 2026-09-23). Host: Windows x86-64. The
owner verdicts come from one release-profile run of the whole `canary_suite`
(1,511 tests, 1,518 s); the umbrella rows from the dev-profile pass and fail
umbrellas at `bf3640c39d` (1,310 s and 56 s), whose route the change between
the two revisions does not exercise.

```text
python tools/progress.py --pass-log <pass.log> --fail-log <fail.log> --samples-log <samples.log>
```

## The number

Every fixture and every spec section is reported at the strongest predicate
some run actually verified. The three predicates, in order:

- **runs**: a dedicated owner test compiled the fixture on the rooted native
  route, executed it on this host, and saw the expected exit status. This is
  the only verdict that means the feature works.
- **compiles**: the pass umbrella produced a native artifact for it on its
  tier's route; nothing executed it, and that route does not settle the rooted
  entry the way an application's build does.
- **checks**: it passes checked semantics (parse, resolve, type, check, proof)
  and nothing native was attempted or succeeded.

**53 of 66 core and typical language-spec sections have a fixture that runs
on this Windows host; 7 at best compile; 2 at best check; none is without a
verified fixture.** Across all
120 sections: 73 run, 13 compile, 10 check,
24 have none.

Of the 2,030 pass fixtures, 158 run, 94 compile, 692 check, 892 fail some run
and 194 are judged by neither route. At `e526ef3f54` on 2026-09-23 the same
numbers were ,, 154, ,, 92 and ,; the movement since is the three
repairs described below.

| Measurement | Value | Reads as |
| --- | --- | --- |
| core+typical sections with a fixture that runs natively | 53/66 | the feature works end to end |
| core+typical sections whose best fixture only compiles | 7/66 | a native artifact exists, never executed |
| core+typical sections whose best fixture only checks | 2/66 | the language rule is understood, not realized |
| all sections: runs / compiles / checks / none | 73 / 13 / 10 / 24 of 120 | breadth over the whole language |
| pass fixtures: runs / compiles / checks / fails / unmeasured | 158 / 94 / 692 / 892 / 194 of 2,030 | depth: distinct fixtures |
| elided fixtures judged by their dedicated owner: runs | 158/913 | the rooted native route with execution |
| owner failures by stage | compile 748, other 4, run 3 | what fails, fails before execution |
| spec sections exercised by any pass fixture | 98/120 | the corpus's own coverage of the spec |
| fail fixtures rejecting with their expected diagnostic | 1,158/1,158 | the compiler refuses what it should |
| pass fixtures some roster runs | 2,000/2,030 | how much of the corpus is rostered at all |
| construct pairs that matter, covered by a fixture | 31/31 | combinations real samples spell |

The distance, stated plainly: the compiler understands most of the language
(689 fixtures check; every core and typical section reaches at least
checking), and on the route a real application takes it runs 154 fixtures
end to end on Windows. Of the 913 fixtures the owners judged, 752 fail at
compile: 499 at the Unit-plan omission (the same mechanism the umbrella's own
failures show, below), 75 in Terminal production, 17 in physical staging and
legalization, 29 at exact-arithmetic overflow proofs, 14 at a boundary call
while a moved value is absent, 13 at translation validation of a structural
call argument, 7 at a non-copy transfer out of borrowed storage. Three fail
at run time, all interpreter-oracle filesystem fixtures on Windows
(`repeated_dir_walk_scan_exit`, `windows_fs_raw_breadth_exit`,
`windows_fs_wrapper_breadth_exit`) that exit 0 where 70 is expected.

On the rooted route the omission sites, by the phase the construction
trace names (622 owner failures name one, over 57 phases at `3b4e69c19b`),
group into families. Stores into a structural field own 230: no pure source
74, a case-typed leaf 60, a scalar-typed leaf 40, the destination parameter
40, the carrier path 11, a byte carrier 5. Then structural call binding of a
local 48, the state graph's result signature 35 (a multi-state machine that
returns a scalar has no route there), a guard expression 33, an unsupported
statement kind 31, a pure scalar initializer 28, write-frame agreement 18,
a bound prefix initializer 16. The 76 that stopped at "call operation" now
name their guard: a scalar result over structural operands with no
registered producer 14 (admitted to the closure since `71687a843b`), the
argument planner's parameter path 11, caller structural result 11, source
symbol 7, parameter access 4, alias and identity 3, parameter source 1;
projected operand support 8; boundary arguments 8 and boundary claim
transfers 7; scalar arguments 1; a linear structural result 1. Every site is
in `typed-trees-to-checked-trees`, so this wall is host-neutral.

The Windows wall that hid all of this is gone. Until 2026-09-23 every rooted
fixture that exited through `Console::exit_process` stopped at "selected
compiler intrinsic ... has no closed native catalog identity": std's Windows
console leaves were compiler intrinsics whose only hosted realization was a
kernel syscall, which Windows does not have. They now bind kernel32
`ExitProcess` by DLL linkage (`bec0f4a0c7`, `2268d68a02`). What remains
Windows-specific: `write_byte` and `read_byte` are still intrinsics without
a hosted realization, so the fixtures and samples that print stop there.

Three host-neutral repairs followed on 2026-09-24. A scalar-result call with
structural operands is planned and resolved by the candidate closure instead
of refused at construction (`71687a843b`); a single-state attached machine
with `&mut self` and a structural formal keeps its scalar graph, which a
same-day widening had excluded as a side effect (`75decc76bc`); and the
verifier treats a machine's shared `&[T]` input as established at entry
(`cf7b8baa2b`). Together they move 4 owner fixtures to running and the
remaining call-site failures onto their callees' own walls, which is why the
state graph's result signature grew from 35 to 40.

The next wall the same fixtures reach is in the Omega backend: the
legalization replay reports `Selection(SourceCustodyMismatch)` for a
multi-state caller that makes a scalar call with a slice operand into a
scalar graph (`runtime_looping_value_return_exit`,
`runtime_value_call_slice_len_guard_exit`); the console-exit-app item on
TASKS.md records the same rejection. Its site is inside the legalization's
scalar-graph input matching and is not yet attributed.

Provenance: the owner verdicts are one release-profile run of the whole
`canary_suite` at `e526ef3f54` (1,511 tests, 447 passed, 25 min on this
Windows x86-64 host); the run at `bf3640c39d` the day before passed 293,
and the run at `3b4e69c19b` that named the call-operation guards passed
the same 447 (1,633 s beside other builds).
Earlier drafts of this record quoted "sections natively established"; that
predicate counted an umbrella compile without execution and every elided
fixture as passing, and is retired.

## Where it fails

173 rostered pass fixtures fail at 2195bd5203. Grouped by the family of
their first diagnostic, five families own 87% of them:

| Fixtures | Share | Family | Owner |
| --- | --- | --- | --- |
| 72 | 42% | `ProgramEntry establishment rejoins 0 Terminal attachment identities`: the machine's Unit plan was omitted at local construction | STATE-LOCAL-VALUE-FRONTIER |
| 29 | 17% | `ProgramEntry Binding field requires a selected Fused provider` | library: no provider exists for the boundary; establishment now walks nested fields, so 15 became 29 |
| 29 | 17% | `selected compiler intrinsic ... has no closed native catalog identity` | FLOAT-PROVIDERS, ARITHMETIC-POLICY-REALIZATION |
| 16 | 9% | `Lowering(InvalidUnitMachinePlan ...)` from native Terminal production | GENERAL-CYCLIC-EXECUTION |
| 5 | 3% | `checked trapping conversion requires runtime policy realization` | ARITHMETIC-POLICY-REALIZATION |

The first family is one mechanism at a dozen sites, not seventy-two gaps. The
Unit builder constructs a machine's plan site by site, and each site admits a
fixed set of source shapes; when a statement's shape is outside its site's
set, the plan is omitted and ProgramEntry cannot establish. The diagnostic
names the site:

| Fixtures | Omitted at |
| --- | --- |
| 21 | statement sequence: local data: structural call binding |
| 12 | structural field store: pure source |
| 12 | statement sequence: call: call operation |
| 7 | structural field store: destination parameter |
| 4 | statement sequence: local data: scalar local: pure initializer |
| 4 | state graph: state signature: parameter signature: attached data shape |
| 3 | state graph: terminator: conditional successors: guard expression |
| 2 | structural field store: scalar field type |
| 2 | state graph: terminator: jump successor: parameter transfer |
| 5 | five sites with one fixture each |

Until this revision the field-store sites were one 21-fixture bucket labelled
`record literal field`, a trace artifact: the construction trace kept the last
store route's entry label, and no fixture in the bucket had a record-literal
field store. With the trace naming the route that got furthest, the bucket
reads as its causes: no pure scalar source for the store (12: float
arithmetic and float field reads, which the checked scalar vocabulary lacks;
Trapping arithmetic; float-to-integer policy casts), a destination that is a
local rather than a parameter (7: stores through recast reference locals), and
a reference-typed leaf (2: a byte-slice literal into a `&[u8] in Utf8` field).
The class is not host-specific by construction rather than by probe:
every site is in `typed-trees-to-checked-trees`, the Psi stage, which the
ownership firewall keeps target-neutral, so the plan is omitted before any
target is chosen. A direct off-host compile could not confirm it — a full
compile of one of these fixtures exceeds 400 s on this host and was cut off.
Each site is a recognizer for the arrangements a
prior fixture happened to need, where ordinary statement sequencing would
admit them all; the product-compiler board item already flags the gate as
advancing one statement shape per slice, and this is its measured size.

A roster-placement family of 17 at 117f2abc9c was repaired at 398b73a4f6:
nine fixtures whose `Main::main` takes parameters were on the native tier,
where no hosted entry can bind them, and now check; eight EFI programs
authored in July sat on the host tier from before the cross-target tier
existed, and now compile for `uefi_x86_64` beside their siblings.

The second family is a library gap on the settled carrier. The compiler-known
runtime carrier is `Binding<R>` ([entry roots](../../spec/build/entry_roots.md));
the library, corpus, samples and the compiler's recognizers spell it so, and
`ForeignBinding<...>` names the unrelated foreign locator. A field of that
carrier at any depth under the entry receiver demands a selected Fused provider
for `R` (establishment walks nested records since 5cc8422932), and
`source/library/std` declares exactly three: `ConsoleNativeProvider`,
`ProcessExitNativeProvider` and `UefiOsHandoffNativeProvider`. The twenty-nine
fixtures reach `FilesystemHost`, `TimeHost`, `Gui` or `Input` either directly
on `Main` or through std's `Filesystem` wrapper, whose own `host` field the
walk now demands (filesystem 19, host 3, time 3, capabilities 2, two more);
no provider exists for any of them, so the compiler's refusal is correct and
the fixtures stall on the library, not the compiler.

## Spec gaps

Six core or typical sections are not natively established:

| Section | Class | State |
| --- | --- | --- |
| data_and_literals: Lexical profile | core | exercised only by checked-only fixtures; no native fixture exists |
| modules: Public structural data | core | exercised only by checked-only fixtures; no native fixture exists |
| counts_and_addresses: Nominal index qualifications | typical | no pass fixture; two fail fixtures pin the live-bound half |
| ownership: Borrowed-storage invariant windows | typical | no pass fixture; seven fail fixtures; no fixture ever closes a window |
| ownership: Construction and disposal order | typical | no fixture of either kind |
| process_exit: Graceful completion and cleanup | typical | no fixture of either kind |

Twenty-two sections have no pass fixture at all. Twelve are the whole of
reflection: `reflect::schema`, `type_key`, `visit_runtime_fields` and
`metadata` appear nowhere in the corpus. The rest are concurrency protocol
proofs, evaluation result caching, three generics sections (reflection
boundary, invocation-lifetime families, publication and assumptions), the two
ownership sections above, and two process-exit sections. The mapping that
produces this list is `tools/progress/spec_groups.tsv`; each row names the
fixtures read to justify it, and `tools/tests/test_progress.py` holds it to
the current spec headings.

## What real programs use

The 147 maintained samples under `samples/` spell these surfaces, beyond the
structural ones every program has:

| Surface | Samples | | Surface | Samples |
| --- | --- | --- | --- | --- |
| `in <Domain>` qualification | 120 | | `pub` | 17 |
| `domain` declaration | 64 | | `trait` | 17 |
| `requires` | 45 | | shared `&` borrow | 16 |
| `boundary` | 31 | | `f32`/`f64` | 14 |
| `case` | 28 | | `match` | 11 |

Of the 82 surface pairs any sample spells, 31 appear in five or more samples.
Every one of the 31 is spelled by at least twelve pass fixtures, so the corpus
already contains the combinations real programs make; whether those fixtures
pass is the per-tier number above. The surface vocabulary is a keyword's
presence, not a parse, so this is evidence a fixture spells both constructs,
not that it exercises their interaction.

## What is not measured here

- **Samples.** No samples number is reported, and the reason is the
  instrument, not the language. `compile_sample_to_checked` builds a fresh
  `CheckedCompileRequest` per sample, so every sample re-checks the standard
  library from scratch; the std check alone is recorded at 945 s on Linux in
  [std check duration](std_check_duration_linux_x86_64.md). On this host in
  the dev profile the checked-trees test was still inside its first sample
  after 38 minutes — one build directory, holding only its package-evidence
  staging and no compiled file — and was stopped; the
  per-cohort native tests had produced nothing after 25 minutes. This is the
  mechanism behind whole samples "stalling": the harness spends hours before
  it can say anything. A samples number needs the harness to check std once
  and reuse it, or a release-profile run; until then the corpus numbers above
  are the measurement.
- **Other hosts.** These are Windows x86-64 results. Nine rooted-target and
  one Windows-host failure are host-specific by construction; the dominant
  family is target-neutral by construction, as above, but a macOS or Linux
  run of the same suites is the only evidence for the rest.
- **The 30 dark fixtures.** Eighteen under `objc/` need a macOS target no roster
  supplies; six `terminal_psi/` and four `filesystem/` fixtures were unrostered
  when measured. Six that check were rostered as checked-only at this revision.
- **Runtime correctness.** A fixture that compiles natively and exits as
  expected is counted as passing; the `run/` corpus of eleven differential
  members, which compare native output against the interpreter, is not folded
  in here.
- **Elided fixtures.** The pass umbrella does not compile every rostered
  active fixture: a fixture with a dedicated exact-native owner among the
  suite's `*_canary_runs` tests is elided from `pass_canaries_compile` and
  judged only by that owner. At this revision the umbrella elided 844 of the
  1,761 rostered fixtures (802 rooted, 4 direct, 35 cross-target, 3 rooted-
  target) and compiled 917 itself, so the rostered row above is a ceiling and
  the umbrella's own row is what this run verified. The umbrella writes its
  owner index under `OMEGA_PASS_CANARY_OWNER_INDEX=<file>` and prints the
  counts under `OMEGA_PASS_CANARY_REPORT_COUNTS=1`; `tools/progress.py
  --owner-index <file> --suite-log <log>` joins a full `cargo test -p compiler
  --test canary_suite` log to that index and gives each elided fixture its
  owner's verdict, which the numbers above now include (release profile, 30
  min; the dev profile runs about three tests a minute here).

## The same wall, measured over samples instead of fixtures

`mbx nextest run -p compiler --test samples_compile --no-fail-fast` at
`8b319c7ddf` (2026-09-23, macOS aarch64, cargo): **33 tests run, 11 passed,
22 failed**, 9,338 s. Nearly every failure is the Unit-plan omission family
above, reported through authored-entry selection:

    selected ProgramEntry establishment rejoins 0 Terminal attachment
    identities; expected one; the machine's unit plan was omitted at local
    construction at `<site>`

It is not target-specific: 75 to 79 occurrences on each of `linux_x86_64`,
`linux_arm64`, `macos_arm64` and `windows_x86_64`, because the stop is in
`typed-trees-to-checked-trees`, before a target is chosen.

**The sites that block samples are not the sites that block fixtures**, so
the fixture table above is the wrong guide for prioritizing this work.
Deduplicated by sample (each fails on about four targets), 57 distinct
samples stop at:

| Samples | Omitted at |
| --- | --- |
| 15 | `state graph: operation custody: unit call arguments` |
| 13 | `structural field store: scalar field type` |
| 9 | `structural field store: pure source` |
| 8 | `state graph: terminator: conditional successors: guard expression` |
| 2 each | `statement sequence: call: call operation`, `state graph: state signature: parameter signature: attached data shape`, `state graph: result signature`, `statement sequence: local data: structural call binding`, `structural field store: byte sequence carrier` |
| 1 each | `conditional successors: parameter transfer`, `prefix initializers: bound expression`, `local data: scalar local: pure initializer` |

Read against the 72-fixture table above, the ranking inverts. The fixture
corpus's largest bucket, `local data: structural call binding` at 21
fixtures, blocks **2** samples. The samples' largest,
`state graph: operation custody: unit call arguments` at 15 samples, does
not appear in the fixture table at all, and neither does
`structural field store: byte sequence carrier` or
`state graph: result signature`. `structural field store: scalar field type`
is 2 fixtures and 13 samples.

That is what a corpus of one-rule fixtures cannot tell you: each fixture
pins the rule someone was adding, so the corpus records which arrangements
have been NEEDED, while the samples record which arrangements real programs
actually write. Both are true; only the second predicts whether an
application compiles.

The four leading sites live in
`typed-trees-to-checked-trees/src/execution/terminal_unit/`, in
`state_graph/mod.rs` and `structural_scalar_store/mod.rs`. This entry is a
reading, not an attribution: no site is bisected to a culprit here, and the
diagnostic names the route that got furthest rather than a proven cause.
