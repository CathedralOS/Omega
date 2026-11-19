# bounded_case_result

A two-byte fixed-array local lends a `&[u8; 2] in Utf8` view through states
to a `Leading` case-result machine. Its fixed length is 2; the first byte
is returned as `Leading::Byte { value }`. Qualification changes no geometry.

Compile-checked only; this probe binds no native program entry.
