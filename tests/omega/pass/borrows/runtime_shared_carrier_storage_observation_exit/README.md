# Shared reads of borrowed carrier storage

After package preparation:

```sh
omega run --both tests/omega/pass/borrows/runtime_shared_carrier_storage_observation_exit/main.omg
```

A reference-bearing carrier already has a shared view. The helper reads
through both a direct shared carrier argument and the stored view, observing
the same original payload. Exit 0 requires both readings to be 73.
Shared loans freeze mutation, not shared observation of carrier storage.
