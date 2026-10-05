# Proof arguments through dynamic implementation selection

Status: the former instance-bearing descriptor proposal is superseded by the
[named-implementation decision](../../spec/language/conformances.md#data-and-code-are-independent)
and [dynamic dispatch contract](../../spec/terminal-psi/dynamic_dispatch.md).
Implementation work belongs to `DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION`
on the [execution board](../../../TASKS.md).

A dynamic call selects an implementation and supplies the requirement's
ordinary arguments. Proof-only actuals remain associated with their declared
formal positions and the exact selected requirement. They do not travel as a
hidden instance field, an implicit receiver, or an extra per-object channel.

The migration must preserve the same proof substitutions, witness identity and
relevance checks as ordinary calls through forwarding, rebinding and storage.
Validate the actuals against every possible selected implementation's public
contract. Private implementation facts cannot strengthen a caller's guarantees.
Static argument selection and runtime data selection remain separate.

The existing `fail/relevance/dynamic_erased_formal_lane` refusal records a
missing implementation route, not permission to revive the old descriptor.
Replace it with positive execution coverage when transport works, while retaining
negative coverage for omitted, mismatched or misbound proof actuals. Preserve
complete row identity and the selected call's actual argument relationships.
