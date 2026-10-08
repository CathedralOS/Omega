# Physical view of an encoded record

```sh
mbx run -p omega -- run --both tests/omega/pass/layouts/runtime_encoded_physical_view_exit/main.omg
```

A helper re-views an incoming encoded record reference as a canonical `i8`
record, reads -128 and writes 127. The caller observes 127 through the original
semantic `i64` view, byte 127 in the original backing and neighboring byte 85.
This pins reference formation from an incoming reference and re-viewing an
already captured byte region without substituting a temporary record.
