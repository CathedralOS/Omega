An earlier constructor child captures an issued ticket before a later child
replaces its source. Both engines must observe the saved 41/9 ticket and the
caller's replacement 0/0 ticket.

```text
omega run --both tests/omega/pass/calls/runtime_constructor_child_declared_membership_exit/main.omg
```

The checker retains membership at the exact child occurrence. Copying carries
that decision, and record construction rebases the completed child's root and
projected memberships beneath its destination field. The field declaration
requires membership; it cannot supply issuer provenance.
