# Exclusive atomic host storage

On macOS ARM64, run:

```sh
omega run --both main.omg < input.txt
```

A helper lends the original nested packet exclusively to `_read`, not a plain
scalar reference or a copied record. A zero read leaves input untouched, a short
read replaces two bytes of the atomic resident, and EOF preserves the remainder.
The subsequent atomic fetch returns the instruction-observed resident. `_write`
uses another exclusive packet loan and emits exactly the authored bytes. Both
engines must exit zero; high resident bytes, an adjacent field and enclosing
siblings must remain unchanged.

A second record lends only its ordinary prefix for shared host output. Its
atomic tail stays outside the demanded byte interval and is subsequently
observed with an atomic load; merely containing an atomic cell cannot refuse
an otherwise backed ordinary prefix. The combined stdout oracle has two lines.

This exercises exclusive custody in one activation, not concurrent atomic
visibility or a license to snapshot an atomic cell through shared host access.
