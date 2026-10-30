# Case-dependent default domain

The case-dependent default domain admits only `Boolean` on `Value<bool>` and
establishes `lo <= hi` with `hi <= 32` on `Range`, so the typed extractor
consumes the Boolean payload and `hi - lo` is exact — the same `<=` bound
predicate re-admitted as the named generic domain `u64::AtMost<32>` on the
guarded `lo`.

Parent: `DOMAIN-DEFAULT-PREDICATE-UNIFICATION` in
[TASKS.md](../../../../../TASKS.md). This row executes the generic typed
extractor over `Integer`/`Boolean` and the guarded `Range` payload consumer
through native and Terminal Psi, exercising the same predicate machinery as a
data default domain and a named generic domain.

`Value<T>` carries its requires disjunction as the default domain; on the
closed `Value<bool>` instance the `Integer` case is refuted by `T == i32`, so
`pick` matches only `Boolean`. `Interval` carries `(self in Empty) ||
((self in Range) && self.lo <= self.hi && self.hi <= 32)`; matching `Range`
establishes `lo <= hi` for the exact `hi - lo` consumer and `hi <= 32` for
`width_range`, whose `lo as u64 in AtMost<32>` establishment spells the same
`<=` bound through the named generic domain.

Each of `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body` transports the
same `Value<bool>`/`Interval` pair to the same `pick`/`width` consumers. All
projects bind the four hosted targets and require `omega run --both` native
and Terminal agreement on exit 0. The runtime oracle requires `pick` to
observe `true`, `width` to observe `4` (`21 - 17`), and the `match_join`
place additionally drives the `Boolean { value: false }` and `Empty` arms to
distinct outcomes.

Each `invalid/main.omg` drops only the `hi <= 32` conjunct from
`width_range`'s requires, so `lo as u64 in AtMost<32>` can no longer prove
the named-domain predicate and must reject. Positive and negative
counterparts are pinned as
`tests/omega/{pass,fail}/facts/case_dependent_default_domain_through_<place>_{exit,rejected}`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Governing contracts: [domains](../../../../../wiki/spec/language/domains.md),
[data and literals](../../../../../wiki/spec/language/data_and_literals.md),
[generics](../../../../../wiki/spec/language/generics.md),
[dependent values](../../../../../wiki/spec/language/dependent_values.md).
