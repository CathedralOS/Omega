# Array Domain Field

Compile-scope probe for a domain-qualified three-byte fixed-array field.
Qualification preserves the ordinary array's layout and length.

- `domain [u8; 3]::Utf8(bytes) requires valid_utf8(bytes)` states membership.
- `main.label = "abc"` replaces exactly three bytes.
- `main.label[0] = 66` preserves membership; a later state verifies length 3
  and bytes `Bbc`. Success records `hits = 1`.

Compile-checked only, with no ProcessExit provider:

```sh
mbx run -p omega -- --check samples/cli/probes/array_domain_field/main.omg
mbx run -p omega -- update --project samples/cli/probes/array_domain_field
```

Project acceptance is local; `omega.lock` is ignored.
