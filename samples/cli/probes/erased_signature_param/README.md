# erased_signature_param

`bound [erased]: i32` — a signature parameter may be erased exactly like
a data field: it stays in checking and semantic identity (the `requires`
cites it) but contributes no runtime storage, transfer, or read; the
call site discharges the instantiated requires from literal arguments.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
