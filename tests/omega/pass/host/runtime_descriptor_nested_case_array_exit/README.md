# Retag a nested mixed value, then lend its byte array

```sh
omega run --both tests/omega/pass/host/runtime_descriptor_nested_case_array_exit/main.omg < tests/omega/pass/host/runtime_descriptor_nested_case_array_exit/input.txt
```

On macOS ARM64 the input replaces only the four-byte native tag. The new Right
payload retains valid incoming scalar/array bytes, including the original
ordinary common field in its separate namespace. A later byte-view import emits
ABC/newline from the new active array's real backing. The containing record's
prefix/suffix stay unchanged; both engines exit zero.

The input contains the tag's four binary bytes followed by an unused newline.
