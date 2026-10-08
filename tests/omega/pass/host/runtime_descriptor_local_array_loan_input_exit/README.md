On macOS AArch64:

```sh
omega run --both main.omg < input.txt
```

Both engines return 0 and print `abc` followed by LF. A local `[u16; 3]`
passes through a mutable helper and a state transition before an ordinary
helper forwards its whole-array loan to exact Darwin `read`. A shared loan
feeds `write`. The short read and EOF retain the third word, and the caller
observes the original first two words after helper return.

Borrowed arrays retain their owner's activation through the existing collection
home; only owned array arguments copy their payload. Native placement supplies
the byte extent without changing the fixed-array language type into a byte view.
The fixture needs no external package.
