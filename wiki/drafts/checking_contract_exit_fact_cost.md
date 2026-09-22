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
