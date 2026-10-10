# qualified match calls

Compile-scope probe. No standard-library dependency.

- Match arms calling `mark(..)` produce `i64 in Coordinate<7>`; the
  match result feeds `relay(value: i64 in Coordinate<7>)` — domain
  typing flows out of arm expressions.
- `relay(match ..) as i64` — `as` erasure on a whole match result.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/qualified_match_calls/main.omg
mbx run -p omega -- update --project samples/cli/probes/qualified_match_calls
```
