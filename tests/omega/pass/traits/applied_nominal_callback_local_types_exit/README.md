The applied visitor has both a type binder and a callback binder. Each selected
callback must retain its Item/Other carrier and its invocation-local type. Both
calls return true; native and Terminal exits are 0.

Run: `omega run --both main.omg`.

The generic callbacks close independently from the applied invocation contract.
Their own type binders are not caller type arguments; the selected visitor still
refines the universal nominal schema, and every callback tuple owes its bounds.
