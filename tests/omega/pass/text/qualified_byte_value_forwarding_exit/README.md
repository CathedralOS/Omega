# Qualified byte-value returns

After package review, run:

```sh
omega run --both tests/omega/pass/text/qualified_byte_value_forwarding_exit/main.omg
```

Both engines must exit 70.
An owned UTF-8 value returns from an entry parameter and from an owned state
arrival, preserving all six bytes and its live length. This capacity uses the
host's indirect aggregate-result ABI as well as owned value transport.
