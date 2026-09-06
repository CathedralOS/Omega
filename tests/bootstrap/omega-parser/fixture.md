# Parser fixture expectations

`main.epsilon` calls the unchanged `OmegaParser::parse_view` through ordinary
Epsilon execution. The harness must include all six whole manifested D source
members, followed by this entrypoint. There are twelve parser invocations on
one receiver. Success is `Exit(0)` with stdout `A`; the private tagged execution
observation is `000000000041`. This fixture does not establish full Omega
acceptance: `Complete` owns the retained syntax rows described by D's README,
while `Incomplete` leaves those rows unavailable to consumers.

The first data input retains four fields. Each contributes a named base and one
wrapper: fixed array, inclusive-range constraint, reference, and domain
constraint. `finish_retain_type_base`, `finish_retain_type_array_node`,
`retain_type_constraint_node_bounded`, and `finish_retain_type_reference`
therefore produce eight type-reference rows, two constraint rows, and field
roots 1, 3, 5, and 7. The final invocation repeats this input after all failures
and requires the same live rows.

The machine input retains three assignment targets and their values: two
expressions for `x = 7`, three for the named struct assignment, and three for
the cast assignment. The transition subject adds the ninth expression;
`retain_integer_transition_guard` retains only the pattern's span. Three
assignments and one transition arm give four statements. The implicit entry
state and explicit `done` state give two state rows. The cast's `in Trapping`
belongs to its cast row, not to a type-constraint row.

The bodyless external input retains one machine, one implicit state, and one
`Satisfies` clause with `CompilerIntrinsic` binding. The next input has `via`
but no clause, specifically checking the count guard after a previous complete
invocation left a clause in backing storage.

The eight incomplete inputs require these exact half-open source spans. Their
offsets are authored input coordinates, not Epsilon customer coordinates.

| Input feature | Span |
| --- | --- |
| External binding without a preceding clause | `12..15` (`via`) |
| Hexadecimal assignment value | `18..21` (`0x1`) |
| Hexadecimal array length | `17..20` (`0x2`) |
| Hexadecimal range minimum | `17..20` (`0x0`) |
| Hexadecimal range maximum | `21..24` (`0x9`) |
| Hexadecimal transition pattern | `29..32` (`0x1`) |
| Struct initializer after `self` rather than a named path | `23..24` (`{`) |
| Outer reference in a cast target | `27..28` (`&`) |

These are current D parser implementation boundaries, not claims that Omega
rejects the corresponding language forms. All observations and row checks are
performed by the authored Epsilon customer; the host does not parse or lower
the inputs or copy the parser guards.
