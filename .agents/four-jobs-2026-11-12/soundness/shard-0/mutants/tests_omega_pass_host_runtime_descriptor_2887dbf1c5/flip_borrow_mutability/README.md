On macOS AArch64:

```sh
omega run --both main.omg < input.txt
```

Both engines return 0 and print `abc` followed by LF. A helper forwards a
mutable borrow of a record field to exact Darwin `read`; a readonly whole-record
borrow feeds `write`. A zero-count read, a short read and EOF retain the unread
second word and the containing record's prefix/suffix. No byte view or copied
record substitutes for the original loan. The fixture needs no external package.

Native lowering supplies scalar-leaf placement to the runtime adapter; Terminal
captures original fields and restrictions. Unmodeled padding, pointers and
atomic storage are not synthesized as bytes. The nested-record fixture challenges
the same protocol with an array and different-width fields.
