# const_alias_identity

Cross-module const aliases keep nominal identity through the alias
chain: `ALIAS`→`SELECTED`→`AGAIN` all denote
`foreign::Token { value: 7 }`, so `Indexed<AGAIN>` and
`Indexed<SELECTED>` are the same type.

## Learnings

- A record literal on a generic data writes the UNAPPLIED head
  (`Indexed { value: 1 }`) — `Indexed<AGAIN> { ... }` is a parse error
  in expression position.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
