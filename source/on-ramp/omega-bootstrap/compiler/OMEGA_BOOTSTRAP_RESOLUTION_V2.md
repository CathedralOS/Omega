# Omega-bootstrap normalized resolution handoff, schema major 2

[`OMGRSW1`](OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGLOW5 to CKIR4`](OMEGA_BOOTSTRAP_RESOLVED_TO_CKIR4_V2.md) |
[`CKIR4`](OMEGA_BOOTSTRAP_CHECKED_IR_V4.md)

`OMGRSW2` is the minimal versioned resolution successor for direct nominal
field-receiver calls. Except for the rules below, every OMGRSW1 header field,
table, row width, ordering rule, source-custody relation, ceiling, status, and
publication rule remains normative.

The eight-byte magic is `OMGRSW2\0`; schema major is 2 and minor is 0. A
canonical OMGRSW2 contains at least one admitted field-receiver call. A source
without one remains OMGRSW1, so changing only the magic never creates a second
canonical encoding. OMGRSW1 and OMGRSW2 consumers reject the other identity.

## Direct field-receiver role-3 binding

In addition to inherited `self.machine(arguments)`, OMGRSW2 admits exactly:

```omega
self.field.machine(arguments)
```

The resolver:

1. resolves `field` uniquely on the caller machine's nominal owner;
2. requires the field to have an exact nominal record type;
3. requires that record and its attached callee to belong to the caller's same
   package and logical module for this first tranche;
4. resolves `machine` uniquely on that field record; and
5. publishes the ordinary 28-byte role-3 binding whose source span is only the
   exact authored `machine` token and whose target is that machine declaration.

No receiver kind, field ID, field offset, path, layout, or call operation is
added to OMGRSW2. Exact source, the existing field/type/declaration tables, and
the role-3 target determine the relation. The implementation may retain the
field-token span privately until resolution, but it may not publish or trust
that scratch as witness evidence.

Unknown, scalar, array, computed, indexed, parameter, parenthesized, or chained
receivers reject. Distinct-module, imported, and cross-package attached calls
remain outside this tranche pending the separate visibility ruling. Receiver
mutability, arguments, results, and call-cycle checks remain lowerer-owned.

One version-dispatching resolver implementation may emit the least required
canonical relation: byte-identical OMGRSW1 for inherited sources and OMGRSW2
only when the direct field form occurs. Sharing an implementation does not make
the two carrier identities interchangeable.
