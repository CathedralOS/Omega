This program puts the issued-ticket obligation on Envelope.ticket itself.
An earlier constructor child captures the issued ticket before the later child
replaces its source. Both execution engines must observe the saved 41/9 ticket
and the caller's replacement 0/0 ticket.

After package preparation, the acceptance command is:

```text
omega run --both tests/omega/pending/calls/runtime_constructor_child_declared_membership_exit/main.omg
```

The child-capture candidate passes source checking and checked interpretation
(exit 0), but native production stops at Main::main's constructor call. Record
construction does not yet transport a structural child's routed qualification.

Retain each completed child's root and projected qualifications under its exact
destination Field path. The existing StructuralPathQualification representation
and constructor_argument_signature provide the transport and independent source
join. ShapeCollector must register nested domain theories; S4 result paths must
descend record fields; S5 record emission and Terminal record validation must
retain and check the resulting rows. A field declaration is an obligation, not
issuer provenance. Broadening storage classification alone would lose evidence.
