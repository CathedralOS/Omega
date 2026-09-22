# Checking-side contract-exit-fact cost — hotspot record

Status: measurement record, not a design or implementation task. Every number
below was taken on this host (macOS arm64, Apple M4) at `31634c31a1` with a
`dev`-profile `omega` binary, by sampling the running process rather than by
reading the code.

This records a **checking**-stage cost centre. It is distinct from
[the C2L conjunct-lowering cliff](c2l_conjunct_lowering_cliff.md), which is a
lowering-stage record; `proof/src/checker` is not involved in either.

## The profile

`omega --check` on `samples/cli/basics/print_number`, sampled for 8s with
`sample(1)`. Counts are samples out of 4698 on the `omega-compile` thread:

| frame | samples |
| --- | --- |
| `typed_trees_to_checked_trees::checking::check_program` | 4698 |
| ` facts::build_check_facts` | 3752 |
| `  proof::build_proof_facts_with_operators` | 3650 |
| `   proof::contracts::calls::build_contract_exit_facts` | 2675 |
| `    flow::calls::call_result_qualification_identities` | 1286 |
| `    facts::field_domain::declared_owned_field_domain_identities` | 1286 |
| `     ::visit` (recursive, nine levels observed) | 1270, 985, 943, 879, 813, 753, 434, 183, 82 |

`visit` walks the owned-field tree of a type reference, and every field
segment it pushes goes through `flow::place::canonicalization::push_field_place_segments`
-> `facts::fact_plan::places::resolution::payload_variant_for_field`, which
scans **every data definition and every member in the program** to answer
"which variant owns this field symbol".

## What was fixed

`build_contract_exit_facts` evaluated `return_field_obligations` once per
STATE, although the computation reads only
`program.machine_states(machine).first()` — the entry state, identical on
every iteration of the state loop. It is now hoisted to the machine loop.
`referent_field_obligations` genuinely reads `state_parameters(state)` and
stays per-state.

Measured on an idle host, same input, the two binaries built from adjacent
commits — `omega --check --target linux_x86_64` on a one-line program that
pulls in eight standard-library sources:

| | run 1 | run 2 |
| --- | --- | --- |
| before | 11.88s | 11.69s |
| after | 9.01s | 8.67s |

About 25%. `cargo nextest run -p typed-trees-to-checked-trees` is 5198/5198
across the change, so the hoist alters no judgment.

## What is still open

The hoist removes a repeat, not the walk. After it, self time is spread thin
across the same recursion — `payload_variant_for_field` (~54/4660),
`push_field_place_segments` (~52), `storage_index_from_arena_index` (~56) —
so there is no single frame left to cut. The two candidates, in order of
expected value:

1. **Memoize `declared_owned_field_domain_identities` by type reference.** It
   is a pure function of `(program, reference)` and is called repeatedly with
   the same reference from several sites. There is currently nowhere to hang
   the memo: it is a free function over `&TypedTrees`.
2. **Answer `payload_variant_for_field` from the symbol parent.** The
   scoped symbol tree already holds the answer the scan reconstructs: a
   payload field's symbol is a CHILD of its variant symbol.
   `syntax-trees-to-symbol-resolved-trees/src/symbols/lookup.rs:234-238`
   states it and relies on it — "a case payload has its own lexical fields,
   then inherits its data's common fields", reached through
   `symbols.get(parent).parent` when `parent`'s kind is `Variant` — and
   `lookup.rs:197-205` tests a case symbol's children for `SymbolKind::Field`.
   So the whole-program scan can become: take the field symbol's parent, and
   return it when its kind is `Variant`.

   **Do not land this on the reading alone.** The failure mode is silent and
   it is a custody failure, not a crash: if any construction path (generic
   case synthesis is the one to check — `preparation/generic_data/substitution.rs`
   appends payload fields directly) binds a payload field's symbol somewhere
   other than under its variant, the fast path returns `None` where the scan
   returned `Some`, and `push_field_place_segments` silently drops the
   `PlaceSegment::Case`. Establish equivalence first: put a `debug_assert_eq!`
   between the two answers and run the canary corpus, then remove the scan.

   The expected win is small — `payload_variant_for_field` was ~54 of 4660
   samples of self time after the hoist — so this is worth doing for the
   asymptotics, not for the measured centre.

## Scale, for whoever picks this up

This is not a micro-cost. In a `dev` build on this host, `omega --check` on
`samples/cli/basics/print_number` takes **731s** and ends with a real
diagnostic (`self.out requires [u8; N]::Utf8`), not a hang — an earlier
25-minute run with no output was simply below the finish line. Individual
`samples_compile` tests run 600-2400s each.


## Aside: the samples_compile gate is red at base, from two causes

Recorded here because the profiling above was reached through this gate, and
the reds are not on the board.

**Cause 1, repaired at `cef9613f9c`.** windows_x86_64 is the FIRST entry in
`HOSTED_SAMPLE_TARGETS`, and the harness had no accepted Windows entry
binding, so every cohort test failed there first. Four members were measured
red-then-green across that fix: proof_samples (15s FAIL -> 414s PASS),
caesar_cipher (670s), format_number (857s), print_squares (868s).

**The rest is not one cause.** It is a lane. Census at `cef9613f9c`, from
`--no-capture` runs of the two surviving cohort members:

`interpreter_samples_compile_from_authored_program_entry_bindings` -- 20
failures, all of this shape, on EVERY target (not only Windows):

```
calculator_rpn/{windows_x86_64,linux_x86_64,...}:
  "selected ProgramEntry establishment rejoins 0 Terminal attachment
   identities; expected one; the machine's unit plan was omitted at an
   unavailable callee (`Main::add`)"
```

`algorithm_samples_compile_from_authored_program_entry_bindings` -- 29
failures over at least SIX distinct causes (each count of 4 is one sample
across the four hosted targets):

| count | cause |
| --- | --- |
| 4 | local construction at `structural field store: record literal field` (state 4) |
| 4 | local construction at `structural field store: record literal field` (state 0) |
| 4 | local construction at `statement sequence: local data: scalar local: pure initializer` (state 2, statement 1) |
| 4 | local construction at `state graph: terminator: conditional successors: guard expression` (state 1) |
| - | `authored Operator declaration selection occurrence 108 remained unresolved after successful checking (CheckedOperator)` |
| - | `cannot prove default-domain field requirement for call render_col from Main::main::...` |

Remember that a local-construction phase is a breadcrumb, not a cause: the
trace records the LAST phase entered, there are 123 distinct phase strings
across 23 families, and it never changes admission. Four distinct phase
strings are not proof of four distinct causes, nor of fewer.

## The `UnavailableCallee` chain, and why it misleads

**The reported stage names the CALLER's reason.** `Main::main` is omitted
*because* `Main::add` has no plan; why `Main::add` was omitted is a separate
ledger row the message never printed. Chasing `Main::add`'s body is wasted
work -- verified by reduction below.

`service_custody/root.rs` now follows that chain and reports the callee's own
recorded reason (bounded, and it says `(dependency cycle)` rather than
looping). On the reduction it turns

```
... omitted at an unavailable callee (`Main::add`)
```

into

```
... omitted at an unavailable callee (`Main::add`), which was itself omitted
at local construction at `statement sequence: local data: scalar local: pure
initializer` (state 1, statement 0)
```

State 1 is `add_sp2` and statement 0 is
`let a: i32 in Saturating = self.stack.slots[0];` -- **a scalar local with a
ranged type, initialised from an indexed read of a nested field.** That is the
actual defect site.

**This corrected a hypothesis taken from reading the code, which is why the
chain was worth building.** Two routes in
`typed-trees-to-checked-trees/src/execution/unit/candidate_closure/mod.rs`
drop a callee: `require_entry` (:61-76) when the machine is absent from
`entries`, and the roster construction (:36-44), which groups by machine
symbol, retains **only groups of exactly one entry**, and drops any machine
with competing entries as `CompetingCandidates`. The second looked like the
answer. It is not: the ledger says `Main::add` left at local construction, so
it was admitted to the roster and failed later. Do not repeat that inference.

Note the phase string is one of the four the algorithm cohort reports, so the
interpreter family and part of that cohort may share this cause -- may, not
do. Confirm per leg with the chain, now that it prints.

The remaining puzzle for whoever continues: the SAME statement compiles
clean in the std-free reduction (17s, first row below). Adding
`console: Service<Console>` as `Main`'s first field is what breaks it, which
shifts `stack` off field index 0 as well as pulling in the standard library.
Separate those two before theorising.

## Reduction log, for whoever continues

`calculator_rpn` reproduces identically through the CLI
(`omega --check --target linux_x86_64 main.omg`, 287s) as through the gate
(963s), so the CLI is a valid and cheaper harness FOR THIS FAILURE. It is not
in general: `print_number` fails via the CLI with a domain error
(`self.out requires [u8; N]::Utf8`) while its gate cohort passes, because the
`samples_compile` harness supplies an accepted Console binding
(`console_acceptance::candidate_console_exit_binding`) and the CLI supplies
none. Check the confound before trusting any CLI reduction.

Reductions run, all with `Main::add` as the named callee:

| fixture | result |
| --- | --- |
| multi-state `add` + indexed writes, NO standard library | **compiles, 17s** |
| the same + `console: Service<Console>` on `Main` | fails, identical diagnostic, 252s |
| + `reaches Console` on the callee that never uses it | still fails |
| + callee moved to `Stack` so its receiver carries no service | still fails, names `Stack::add` |
| service program, helper call REMOVED | `omega-language-std` itself fails: `routed service field 'Filesystem::host' has no exact Fused selected-provider-plan join` |
| service program, helper body reduced to one assignment | same `Filesystem::host` failure |

So the machine's shape is not the trigger, and neither is where the service
carrier sits. The last two rows also show the bisection perturbing the
standard library's own closure -- the `Filesystem::host` join error is real
and reproducible, just not on `calculator_rpn`. Treat single-variable
reasoning here with suspicion: changing the root program changes what std is
asked to supply.
