# Descriptor scalar-loan writeback

This program binds only Windows x86-64. It reads `AB\n` through exact admitted
MSVCRT imports into a caller field, a local array element passed through an
ordinary helper, and a record-owned array element. Zero-length reads and EOF
leave storage unchanged; short reads preserve the untouched bytes of each i32.
The author expects exit 70 and output `A!\n`, with foreign and Console writes
sharing one ordered output observation.

On Windows, run the ordinary prepared product with:

```text
omega run --both main.omg < input.txt
```

The exit oracle is Windows-specific: building its PE image on another host is
not native execution. There is no portable stdout oracle claiming such a
foreign-target build ran. The Terminal leg can execute the Windows-selected
artifact on another supported host with explicit runtime supplies; that does
not qualify the actual CRT descriptor mode or native input behavior.
