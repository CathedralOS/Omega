Canonical extent geometry forms a specification term
===================================================

The canonical `no_wrap(base, length)` predicate must retain its transparent
mathematical denotation when used in an ordinary machine's `requires` clause.
The predicate embeds the address and independent count into unbounded proof
integers; this fixture supplies no runtime provider or executable-call promise.

Check from the repository root:

```sh
omega --check tests/omega/pass/math/extent_no_wrap_requires_denotation/main.omg
```

The source is the unchanged controlled regression probe. With the same compiler
and library inputs, changing only `core/extent.omg` from its transparent formula
to a bodyless boundary machine rejects the specification call for lacking a
value denotation. Restoring the formula checks 14 source files. Full compiler,
source, and library identities are retained in the milestone evidence.
