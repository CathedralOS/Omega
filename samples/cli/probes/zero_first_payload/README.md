# zero first payload

Compile-scope probe. No standard-library dependency.

- Payload-bearing first case: `let token: Token;` zero-init selects
  `Token::Number` with a zeroed, established payload — transition
  reads `value == 0` on the Number arm.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/zero_first_payload/main.omg
mbx run -p omega -- update --project samples/cli/probes/zero_first_payload
```
