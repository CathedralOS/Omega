# Core integer endpoint

The ordinary integer query preserves the exact u32 maximum through transport,
permitting subtraction of that endpoint.

Parent: `CORE-INTEGER-FLOAT-APIS` in [TASKS.md](../../../../TASKS.md).
The governing [numeric API contract](../../../../wiki/spec/language/numeric_values.md#core-integer-and-float-apis)
requires ordinary checked declarations, exact unqualified carrier identity and
semantic widths independent of physical layouts. No caller equality guard
establishes the endpoint.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [twin](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [twin](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [twin](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [twin](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [twin](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [twin](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [twin](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [twin](target_body/invalid/main.omg) |

The exact type predicate admits the eight unqualified integer carriers. Each
branch and its conditional postcondition are checked before specialization;
static type equations never become runtime data. The ordinary query's checked
result proves the unchanged `0xFFFF_FFFFu32` subtraction requirement. Every twin
changes only the producer to `max<u16>() as u32`, so the receiving obligation
rejects `65535 == 4294967295`. The match place executes both alternatives.
Every place binds all four hosted entries; the target place has four ordinary
named implementations selected by explicit target-keyed Build rows. Checking
visits every supplied implementation independently of realized target.

The const place retains its source-issued Call selection independently of its
qualified diagnostic span. Static application predicates are discharged against
the supplied argument before formal substitution. [Controls](controls/) reject
unsupported and qualified carriers, false pending type arguments, and a
same-spelling user declaration. Exact endpoint, width and signedness
[vectors](vectors/main.omg) cover all eight carriers with independent literal
oracles; they do not derive expected values from the API under test. Existing
reference-shaped equations and public refined contract transport have separate
pinned regression controls.

These are scalar values without loans, cleanup or external authority. Ordinary
checked contracts, arithmetic proofs, Terminal calls and native returns retain
the endpoint. No layout or provider supplies its numeric value, no query name
is an intrinsic, and no Terminal wire variant is added. Public review records
retain symbolic type equality rather than inventing a runtime comparison.

The initial fixed-base producer refused the absent module and the general
refinement probes. Current integer execution evidence, exact producer inputs,
raw streams and intended diagnostics are retained under the owning claim.
The nine positives and twins are pinned as `core_integer_endpoint_through_*`
in `tests/omega/{pass,fail}/facts`. Floating metadata, bit wrappers and their
representation row remain separate open acceptance of the same parent task.
