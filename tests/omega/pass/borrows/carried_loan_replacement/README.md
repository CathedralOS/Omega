# carried_loan_replacement

`Carrier<'storage>` holds a `&'storage Cell` member; replacing the whole
`holder.carrier` record swaps its referent while `holder.sibling` keeps
the earlier source — each carrier row retains its own referent.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
