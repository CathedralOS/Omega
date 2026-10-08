# Snapshot through a borrowed local record

After ordinary package review, run:

```sh
omega run --both tests/omega/pass/borrows/local_record_byte_field_snapshot_exit/main.omg
```

Both engines must exit 70. A constructed owned record lends its original local
home to a helper returning an independent UTF-8 field snapshot. Subsequent field
replacement changes neither the captured bytes nor the record's scalar sibling.
The caller checks both byte contents and live lengths after the helper returns.
