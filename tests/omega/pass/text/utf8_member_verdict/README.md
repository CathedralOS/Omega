# utf8_member_verdict

`#1 label: &[u8] in Utf8` — a wire member carrying the locally declared
`domain [u8]::Utf8(bytes) requires valid_utf8(bytes)`. Decode validates
UTF-8 at the boundary: the honest three-byte euro run (E2 82 AC) decodes
`Sound`; a hand-crafted overlong run (E0 80 80) decodes `Invalid`.
`decoded.label = ""` — a string literal establishes the domain.

## Learnings

- `state` declarations live at machine level only — chained through
  transition arms; a state nested inside another state's body is a
  syntax error.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
