# Parser fixture expectations

`main.epsilon` calls `OmegaParser::parse_view` through ordinary
Epsilon execution. The harness must include all eight whole manifested D source
members, followed by this entrypoint. There are sixteen parser invocations on
one receiver. Success is `Exit(0)` with stdout `A`; the private tagged execution
observation is `000000000041`. This fixture does not establish full Omega
acceptance: `Complete` owns the retained syntax rows described by D's README,
while `Incomplete` leaves those rows unavailable to consumers.

The first data input keeps `b: u32` and requires `b <= 9` on its data
subject. The unsigned carrier supplies the lower zero bound; the proposition
supplies the upper nine bound. The other three fields retain their fixed-array,
shared-reference and exact named-domain qualifications. The field roots are
1, 2, 4 and 6, with seven type-reference rows and one domain constraint.
The requirement owns the ordinary `LessEqual` expression with its exact path,
literal and operator spans. The final invocation repeats this same input after
failures and requires the same live rows.

A separate input retains three repeated predicates on `[copy]` data, followed by
a declaration with no requirements and one with a bare Boolean requirement.
Its thirteen expression rows preserve grouping and arithmetic/logical precedence;
four ordered requirement roots belong to data spans 0..3, 3..3 and 3..4.
The first predicate uses the logical `self.b` subject; the second retains
named membership followed by the third clause. Operator identities and the
logical subject's exact path spans remain observable. This checks syntax
ownership and composition, not semantic proof of the requirements.

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

The six incomplete inputs require these exact half-open source spans. Their
offsets are authored input coordinates, not Epsilon customer coordinates.

| Input feature | Span |
| --- | --- |
| External binding without a preceding clause | `12..15` (`via`) |
| Hexadecimal assignment value | `18..21` (`0x1`) |
| Hexadecimal array length | `17..20` (`0x2`) |
| Hexadecimal transition pattern | `29..32` (`0x1`) |
| Struct initializer after `self` rather than a named path | `23..24` (`{`) |
| Outer reference in a cast target | `27..28` (`&`) |

The two former hexadecimal scalar-range inputs now require `SyntaxFailure`
with `UnexpectedTypeSuffix` at `16..17` (`[`). They no longer reach an endpoint
parser. Independent hexadecimal array, expression and transition controls remain.
Three malformed `requires` inputs require `ExpectedDataRequirement`: an empty
predicate at `22..23` (`{`), a missing predicate at EOF `21..21`, and a missing
comparison operand at EOF `30..30`. No failed invocation exposes usable syntax
rows. The following complete invocation must reset all clause and expression
counts.

All observations and row checks run in the authored Epsilon customer. The host
frames whole manifested source units and compares bytes; it does not parse or
lower the inputs or copy the parser guards. These observations do not close the
Rust suffix retirement or the parent capability's remaining source-to-native
acceptance.
