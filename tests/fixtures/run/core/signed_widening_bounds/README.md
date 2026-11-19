# Signed widening publishes its source range

Each of the six signed-to-signed core widening calls feeds Exact negation in its wider carrier. The wrappers take unconstrained source formals, so checking must use the widening contract rather than the constants chosen by the entry. Runtime checks cover MIN, MAX and zero for every width pair. The Unit entry has all four hosted bindings and expects native/Psi exit0.

The negative twin changes only the i32-to-i64 wrapper's result upper bound from2147483648 to2147483647. Its possible input MIN32 makes that stronger claim false; arithmetic and the runtime source remain identical. No caller range assumption or errno value guarantee is introduced.

The positive must run to Unit completion (exit0) in native and Psi. The negative must be refused for its false upper-bound guarantee after all six arithmetic operations are proved; an overflow, syntax or setup refusal does not establish that result.
