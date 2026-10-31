# Completed integer carrier bounds

A completed u8 value keeps 0..255 when widened to i32 and placed in a bounded record.

The same receiving predicate remains `0 <= self.value <= 255` in every place.
Each positive exercises zero and 255 and traps if the constructor result changes.
The match place selects distinct arms; the other-target place uses an ordinary
named implementation selected by Build for each of the four hosted entry slots.

Reject twins change only the producing carrier from u8 to u16 and exercise 256.
The unchanged receiving predicate must reject that missing upper bound. A receiving
constructor annotation is not evidence for its own predicate. The runtime oracle
remains zero/255, so incorrectly accepting the twin cannot silently pass execution.

Places: [local](local/main.omg), [parameter](parameter/main.omg), [call_result](call_result/main.omg), [other_machine](other_machine/main.omg), [other_state](other_state/main.omg), [field](field/main.omg), [match_join](match_join/main.omg), [const_name](const_name/main.omg), [target_body](target_body/main.omg).
Each place has an `invalid/main.omg` twin and pinned positive/reject fixtures under
`tests/omega/{pass,fail}/facts/completed_integer_carrier_bounds_through_*`.

This row covers the source-bound propagation prerequisite discovered by the
dungeon compilation performance investigation. It does not consolidate integer
bounds engines or change numeric conversion representability. Fixed carrier
endpoints and denotation-preserving casts follow
[the numeric contract](../../../../wiki/spec/language/numeric_values.md);
[constructor predicates](../../../../wiki/spec/language/domains.md) remain
independent formation obligations. Runtime observations are recorded with the
exact producer in the performance evidence.

Verified on Linux x86_64 with optimized producer SHA256
`bcdded47390ca9b5afb3ccd5dc68ed4b4a219aa8ccf962eef7e8e11c43fe9ecc`:
all nine places exit zero in native and Terminal, and all nine twins reject the
unchanged receiving domain. Other hosted bindings are declared, not runtime
qualification of those hosts. The pristine producer rejects the parameter place.
