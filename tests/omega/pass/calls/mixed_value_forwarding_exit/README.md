# Mixed-value forwarding

Two ordinary owned calls forward a value containing both a common field and a
selected case payload. Both must survive unchanged, including the case tag.

After ordinary local package review:

```sh
omega run --both tests/omega/pass/calls/mixed_value_forwarding_exit/main.omg
```

Native and interpreted execution must both exit 70.
