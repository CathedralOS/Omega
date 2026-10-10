# case domain contract

Compile-scope probe. No standard-library dependency.

- `machine make() -> Choice ensures result in Choice::Cell` — a case
  identity is a domain the callee can guarantee.
- `observe(&Choice)` / `take(Choice)` `requires choice in
  Choice::Cell`; a saved `let choice = make()` result discharges all
  three call sites (by reference and by value).

## Gaps found while authoring (filed)

- The identical calls inside a carrier entry `Main::main(main: &mut
  Main)` are REJECTED — 'cannot prove requires contract: choice ==
  Choice::Cell' at every call site. A free `machine main()` caller
  proves them. Saved-result case guarantees don't reach carrier-entry
  callers.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/case_domain_contract/main.omg
mbx run -p omega -- update --project samples/cli/probes/case_domain_contract
```

Green on Linux x86-64 with the free-machine caller.
