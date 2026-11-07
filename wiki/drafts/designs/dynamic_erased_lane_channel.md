# Proof arguments through runtime callable selection

Status: both the instance-bearing descriptor and whole-trait runtime selector
are superseded by [machine values and some packages](machine_values_some.md).
Implementation remains with the existing descriptor migration on the
[execution board](../../../TASKS.md).

An indirect call selects a machine value under its callable contract and supplies
ordinary arguments. Proof-only actuals remain associated with their declared
formal positions and exact callable application. A `some` opening additionally
retains dependent field identity; no proof lane is turned into an implicit
receiver, runtime type test or automatic cross-operation law.

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
