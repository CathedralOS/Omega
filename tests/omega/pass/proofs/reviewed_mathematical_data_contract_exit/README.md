# Mathematical predicates in package contracts

A package can publish a bounded value whose invariant calls a mathematical
function. Its consumer computes `100 - value` from that bound and checks the
result. The same package also publishes a function whose contract uses a type
argument, a partly applied predicate, and a function returned by another call.

This is a focused regression for Cathedral's allocator dependency. With producer
`ea1ae6ac61a03750d56c7e96499924d70903898f`, the program checks and runs, but
ordinary `omega update --project <this directory> --target macos_arm64` rejects
its dependency: public data `Bounded` calls a target for which package review
finds no parameter declaration. Cathedral's `Bump` fails at the same operation.

Validate package update through the public CLI as well as `omega run --both`;
the corpus runner alone does not exercise the complete package-review route.
The [invalid control](../../../fail/proofs/reviewed_mathematical_data_contract_rejected/main.omg)
changes only the constructed value from 73 to 101 and must reject for violating
the data invariant. It does not remove or weaken the mathematical contract.
