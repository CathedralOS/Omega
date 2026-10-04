# Dependency implementation closure

The consumer imports only `value.omg` and observes its authored result, 7.
The dependency Build queries the signature of Product entry `launch` through
a helper. Its unrelated `unused.omg` body calls `abs` with two arguments and
initializes `[u8; 0]` with one byte. The former rejects during runtime expression
normalization and the latter during typing, before the later Build projection.
The test cannot pass merely by postponing proof checks until after typing.
The same module declares a computed array type. Its detached type-arena node
must not cause Build evaluation to execute a deferred Product helper.
It also declares a computed constant and a generic call in an unused signature.
Their preparation must demand only their helper closure, without making the
unrelated malformed implementation executable or resolving its statements.
An unused machine also declares a structural fixed-array equation. Preparing
that declaration must not demand the module's unrelated implementations.
Its placed-view signature reflects a schema with a transitive computed array
extent and a nested plan-laid field. The probe must retain both declarations
and demand their helpers and policy, without demanding an unrelated invalid
layout policy discovered in the same forest. A sibling file defines an invalid
helper with the same leaf name as the selected array helper: exact source-owned
call identity, not a global spelling lookup, must select the valid declaration.

Run `omega --check tests/omega/pass/build/dependency_implementation_closure/main.omg`
to exercise dependency Build preparation through package orchestration. The
corpus native route exercises the imported module and must exit 0, but does
not run dependency Builds and cannot establish the source-closure acceptance.

This intentionally invalid full dependency is a frontend selection probe,
not an accepted-package fixture: full review must reject the malformed builtin,
and adding `use closure_dependency::unused` to the consumer must reject too.
The fixture does not claim that invalid source can receive package acceptance
or that an unaccepted project can execute through the CLI.
