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

Revision `117f2abc9c` (origin/main, 2026-09-22). Host: Windows x86-64. The
suites ran through Cargo in the dev profile: the pass umbrella in 1,310 s, the
fail umbrella in 56 s.

```text
python tools/progress.py --pass-log <pass.log> --fail-log <fail.log> --samples-log <samples.log>
```

## The number

**60 of 66 core and typical language-spec sections are natively established:
at least one fixture exercising the section compiles to a native artifact and
passes.** Across all 120 sections it is 92.

That is breadth. Depth, counting fixtures rather than sections: of the distinct
native-tier fixtures in groups that map to core or typical sections, 801 of
944 pass. The two numbers together say the compiler reaches nearly every rule
a real program needs, and fails about one fixture in seven along the way.

| Measurement | Value | Reads as |
| --- | --- | --- |
| core+typical spec sections natively established | 60/66 | breadth over what real programs use |
| all spec sections natively established | 92/120 | breadth over the whole language |
| core+typical native fixtures passing | 801/944 | depth: distinct fixtures, not sections |
| spec sections exercised by any pass fixture | 98/120 | the corpus's own coverage of the spec |
| rostered pass fixtures that pass their tier | 1,558/1,755 | the umbrella suite's health |
| fixtures that compile to a native artifact | 881/1,071 | the native tiers alone |
| fail fixtures rejecting with their expected diagnostic | 1,156/1,156 | the compiler refuses what it should |
| pass fixtures some roster runs | 2,029/2,059 | how much of the corpus is measured at all |
| construct pairs that matter, covered by a fixture | 31/31 | combinations real samples spell |

"Natively established" is deliberately weak: one passing fixture establishes a
section. A section whose only native fixture passes reads the same as one
where fifty do. The depth row is the corrective. Neither says a real program
combining those sections compiles; that is the samples' job, below.

## Where it fails

198 rostered pass fixtures fail. Grouped by the family of their first
diagnostic, five families own 83% of them:

| Fixtures | Share | Family | Owner |
| --- | --- | --- | --- |
| 85 | 43% | `ProgramEntry establishment rejoins 0 Terminal attachment identities`: the machine's Unit plan was omitted at a structural field store | STATE-LOCAL-VALUE-FRONTIER |
| 30 | 15% | `selected compiler intrinsic ... has no closed native catalog identity` | FLOAT-PROVIDERS, ARITHMETIC-POLICY-REALIZATION |
| 18 | 9% | `Lowering(InvalidUnitMachinePlan ...)` from native Terminal production | GENERAL-CYCLIC-EXECUTION |
| 17 | 9% | `native-artifact production requires one exact selected program entry` | corpus: nine take parameters and cannot be a program root; eight are EFI programs on the host tier |
| 15 | 8% | `ProgramEntry Service field requires a selected Fused provider` | library: no provider exists for the boundary |

The first family is one mechanism at six sites, not eighty-five gaps. The
Unit builder constructs a machine's plan site by site, and each site admits a
fixed set of source shapes; when a statement's shape is outside its site's
set, the plan is omitted and ProgramEntry cannot establish. The diagnostic
names the site:

| Fixtures | Omitted at |
| --- | --- |
| 21 | structural field store: record literal field |
| 20 | statement sequence: local data: structural call binding |
| 16 | state graph: state signature: parameter signature: attached data shape |
| 10 | statement sequence: call: call operation |
| 4 | statement sequence: local data: scalar local: pure initializer |
| 2 | statement sequence: assignment: call source result type |

The field-store site (`structural_scalar_store/mod.rs`) refuses a floating
computation, a payloadless sum case, a string literal, an indexed byte store
or a call result stored into a field; `float` (9) and `recast` (5) hit it
most. The class is not host-specific by construction rather than by probe:
every site is in `typed-trees-to-checked-trees`, the Psi stage, which the
ownership firewall keeps target-neutral, so the plan is omitted before any
target is chosen. A direct off-host compile could not confirm it — a full
compile of one of these fixtures exceeds 400 s on this host and was cut off.
Each site is a recognizer for the arrangements a
prior fixture happened to need, where ordinary statement sequencing would
admit them all; the product-compiler board item already flags the gate as
advancing one statement shape per slice, and this is its measured size.

Family four is roster placement, repaired at this revision: nine fixtures
whose `Main::main` takes parameters were on the native tier, where no hosted
entry can bind them, and now check; eight EFI programs authored in July sat on
the host tier from before the cross-target tier existed, and now compile for
`uefi_x86_64` beside their siblings.

Family five is a library gap on the settled carrier. The compiler-known
runtime carrier is `Binding<R>` ([entry roots](../../spec/build/entry_roots.md));
the library and corpus still spell it `Service<R>`, and `Binding<...>` still
names the unrelated foreign locator, which is the migration
BINDING-CARRIER-NAME owes. A direct entry-receiver field of that carrier
demands a selected Fused provider for `R`, and `source/library/std` declares
exactly three: `ConsoleNativeProvider`, `ProcessExitNativeProvider` and
`UefiOsHandoffNativeProvider`. The fifteen fixtures declare the carrier over
`FilesystemHost` (9), `TimeHost` (2), `Gui` (2) and `Input` (1) directly on
`Main`, for which no provider exists, so the compiler's refusal is correct.
The passing filesystem fixtures reach the host through std's `Filesystem`
wrapper, whose own carrier field is established by the library rather than
selected at the entry; a program that takes the raw host directly is blocked
until a provider is authored.

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
