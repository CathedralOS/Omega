# Body-bearing match equality

Equal `f64 in Marked` carriers. The selected `==` body returns false, so the
direct comparison is false and the match takes the wildcard. Builtin equality
would do the opposite.

Owning task: **OPERATOR-MACHINE-SUPPLY**. This folder is the std-free witness
for a body-bearing spelled comparison. A bodyless floating comparison of `f64`
stays on the comparison route in `float_comparison_route`.

Positive root: [`main.omg`](main.omg), the local shape. The same sentence is
copied through [`field`](field/main.omg), [`parameter`](parameter/main.omg),
[`call_result`](call_result/main.omg), [`match_join`](match_join/main.omg),
[`const_name`](const_name/main.omg), [`other_state`](other_state/main.omg),
[`other_machine`](other_machine/main.omg), and
[`target_body`](target_body/main.omg). Each binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` with no
`Build::depend` line. `Expected exit: 0`. On this Windows host, `omega run
--both` is native exit 0 and interpreter exit 0 for every place. Each
`invalid/` twin changes only the selected body from `false` to `true` and
rejects `direct == false`.

The comparison records the selected body's boolean result. The match and the
direct use read that result. A domain on an `f64` constant keeps the `f64`
encoding.

The [`returned_boolean`](returned_boolean/main.omg) rung keeps a result constant
through agreeing literal returns, a named state and a terminal call; a separate
conditional result still executes both true and false. Its
[`twin`](returned_boolean/invalid/main.omg) changes only the named state's
returned value. Body-result equations join every reachable normal return and
selected target body. An unknown or disagreeing return supplies no constant;
an earlier expression statement is not the invocation's final result.

On `55898d082fc8`, an unrelated conditional body returning true or false wrongly
checks as always true, and a mixed named-state body wrongly checks as always
false. The agreeing named-state/call rung instead refuses its valid `demand`
contract. Those frozen baseline observations and the actual host execution
records are retained in the owning qualification/claim evidence. The focused
fixtures under `tests/omega/{pass,fail}/proofs/` pin the shared return join;
they do not close the rest of **OPERATOR-MACHINE-SUPPLY**.
