# Decimal state contract

Run `omega run samples/cli/probes/decimal_state_contract/main.omg` from the
repository root. Expected exit: 0.

The program subtracts hundreds and then tens from 999, counting decimal digits. Its state
contract relates the actual Trapping integer payloads through proof-only
`embed` projections. The final acyclic state checks both ASCII digit bounds
without repeating the loop's invariant. The program traps if the resulting
last two digits are not nine and nine.

The isolated customer exercises simultaneous state argument substitution,
normal-return arithmetic facts, integer rounding in implication, and ordinary
fact transport across an acyclic state. Wrapping arithmetic retains its own
payload meaning; this program does not grant its unbounded integer equation.
