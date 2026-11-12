Run the authored program natively and in the checked interpreter:

```sh
mbx run -p omega -- run --both tests/omega/pass/collections/runtime_composed_element_view_reads_exit/main.omg
```

Both executions must exit 0. The corpus `--both` gate also checks the shared
Terminal interpreter against the native result.
