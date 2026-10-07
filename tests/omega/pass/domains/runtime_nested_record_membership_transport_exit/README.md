An issued copyable ticket is captured inside two record constructors before a
later child replaces its source. The wrapper crosses a machine call, its
envelope is copied into another call, and its ticket reaches an Issued-qualified
consumer that reads both fields. The caller independently observes replacement.

```text
omega run --both tests/omega/pass/domains/runtime_nested_record_membership_transport_exit/main.omg
```

Both engines must exit 0: the consumer sees 41/9 and the caller sees 0/0.
Membership follows completed child occurrences and exact projected paths, not
the receiving annotation, a sibling field or the source's later epoch.
