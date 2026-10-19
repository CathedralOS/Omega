# Runtime index application

A runtime limit from 9 to 10 binds Index<limit>, retaining payload 8 below the exact captured limit.

The operation is `let indexed: Index<limit> = Index{value:8}` under
`limit >= 9 && limit <= 10`. Construction must prove `value < Limit`.
The bound belongs to the runtime subject; it does not select a const
specialization. Every invalid twin changes only the constructed payload from
8 to 10, retaining the bound, consuming operation and runtime oracle.

The original root is preserved. Its baseline at
`fd367363d34c9c1e082bd51eafad81c57311a006` rejected at the construction with
“data `Index` literal cannot prove the default domain”. The const-binder and
ordinary-record controls checked; supplying the descriptor explicitly did not
repair the runtime-indexed construction. All nine places reproduced this refusal.
The same baseline incorrectly accepted payload 10 when a global constant named
`Limit` supplied 100 after ambiguous local lookup. The repaired scope rejects
that construction against its actual captured runtime limit.

| Place | Positive | Reject twin | Establishment carried to construction |
| --- | --- | --- | --- |
| local | [main](local/main.omg) | [payload 10](local/invalid/main.omg) | local copy of the admitted runtime subject |
| field | [main](field/main.omg) | [payload 10](field/invalid/main.omg) | `Bounds` field domain, read inside a separate machine |
| parameter | [main](parameter/main.omg) | [payload 10](parameter/invalid/main.omg) | runtime parameter's entry requirement |
| call result | [main](call_result/main.omg) | [payload 10](call_result/invalid/main.omg) | completed call's result guarantees |
| match join | [main](match_join/main.omg) | [payload 10](match_join/invalid/main.omg) | either bounded input; both arms execute |
| const name | [main](const_name/main.omg) | [payload 10](const_name/invalid/main.omg) | named bounds 9 and 10 on a runtime parameter |
| another state | [main](other_state/main.omg) | [payload 10](other_state/invalid/main.omg) | checked arrival and state entry requirement |
| another machine | [main](other_machine/main.omg) | [payload 10](other_machine/invalid/main.omg) | helper call substitutes its exact actual |
| target body | [main](target_body/main.omg) | [payload 10](target_body/invalid/main.omg) | each of four target bodies proves construction |

Mirrored corpus pairs live at
`tests/omega/pass/facts/runtime_index_application_through_<place>_exit` and
`tests/omega/fail/facts/runtime_index_application_through_<place>_payload_outside_limit`.
Their sources and builds match the sample pairs byte for byte. Every positive
and root binds all four hosted `ProgramEntry` targets and has an exit-0 oracle.
The final `first == 8 && second == 8` check observes the result; it supplies no
premise to the earlier construction. No caller guard repairs a missing fact.

## Requirement accounting

Owning task: **RUNTIME-VALUE-GENERICS**. This row establishes default-domain
construction of fixed-representation data using a directly named runtime scalar
index, transported through the nine places.

- **Establishment:** the admitted lower bound 9 proves payload 8 below the
  captured limit. Payload 10 violates the same predicate for every admitted
  limit. The constructor's domain never serves as its own premise.
- **Identity and invalidation:** the generic binder retains its declaration
  identity in type positions. In a data predicate its generated leading
  descriptor field supplies the captured value. Resolution uses that producer's
  telescope order, not a search for an arbitrary same-named field. It preserves
  genuine duplicate-field ambiguity, const/type binders and case-payload
  shadowing. Existing scalar snapshots still prevent reassignment from retagging
  an old indexed value; descriptor equality and effect checks remain required.
- **Composition:** locals, field domains, parameter/arrival contracts, exact
  call guarantees and both incoming match results justify the same constructor.
  The const-name place names the bounds; its subject remains a runtime parameter.
- **Soundness:** the repair removes the duplicate binder/descriptor entry in
  declaration fact resolution. It grants no constructor, proof, borrowing,
  linear custody, static-layout or authority exception. Runtime fields still owe
  equality to their captured application subjects. A global constant named
  `Limit` cannot replace the selected descriptor.
- **Terminal Psi and carrier lowering:** the existing elaborator supplies the
  captured scalar as an ordinary leading descriptor field. The fixed record
  retains its payload; no boxing, runtime proof object, per-index code generation
  or maximum-range stack reservation is introduced.
- **Native realization:** these fixed scalar carriers use the ordinary
  structural storage, call, frame and image route. The scalar result and exit
  oracle exercise source checking through Terminal and native execution. This
  row does not assert coverage of indirect aggregate returns, dispatch tables or
  byte-pool relocations, which it does not use.

Focused controls additionally challenge a second runtime descriptor and a
same-named global constant:
`tests/omega/pass/generics/runtime_index_predicate_second_descriptor_exit` and
`tests/omega/fail/generics/runtime_index_predicate_global_shadow`.
Existing `runtime_index_carrier_*` controls retain stale/mismatched indices,
metadata writes and argument/payload-effect refusals. The existing runtime
value-generic and forwarded-contract canaries retain their own oracles.

Remaining parent acceptance includes other runtime argument forms, general
parameter/result/field qualification forwarding through reassignment and guards,
static-only rejection, proof-only erasure and linear custody coverage beyond
these fixed scalar constructors. Runtime inline-array extents remain static-only;
finite dispatch is owned separately. Foreign-host execution requires those hosts;
source checking and bindings do not establish it.

Canonical contracts:
[Value binders and const requirements](../../../../wiki/spec/language/generics.md#value-binders-and-const-requirements),
[runtime index identity](../../../../wiki/spec/language/generics.md#runtime-index-identity-and-storage),
[establishment and witness loans](../../../../wiki/spec/language/dependent_values.md#establishment-and-witness-loans),
and [default domains](../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization).
The governing generics blob is `e3d20c0c1f44d36ac6221811e46446975bea57e8`.

```sh
omega update --project samples/cli/facts/runtime_index_application --target linux_x86_64
omega run --both samples/cli/facts/runtime_index_application/main.omg
python3 tools/sample_native_gate.py --filter cli/facts/runtime_index_application --cli <omega> --jobs 2
python3 tools/corpus_gate.py --filter runtime_index_application_through_,runtime_index_predicate_
python3 tools/corpus_gate.py --both --filter runtime_index_application_through_,runtime_index_predicate_
```

Matching-host execution records native and Terminal exits separately. Linux
x86-64 observations do not establish Windows, macOS or Linux ARM64 execution.

The existing 54-fixture runtime-index/value-generic regression selection has
three pre-existing golden regressions at series base `336cafc17e1a`: the
`runtime_index_carrier_descriptor_whole_preserve` positive rejects, and the
`borrow_mismatch` and `descriptor_whole_write` controls reject before their
authored reason. All 54 status, diagnostic, expected-fragment and execution-fact
signatures match that exact base (0 lost, 0 gained). These broader borrow-call
qualification gaps remain outside this construction repair; their expectations
and historical observations are preserved.
