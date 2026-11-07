# Selected window replacement

A helper replaces one descriptor in a borrowed collection of scan windows.
The selected element is proved equal to zero. It arrives as a local, a field,
a parameter, and a call result. The caller writes the displaced backing and
keeps the neighboring window. Expected exit: 0.

```sh
omega run samples/cli/collections/selected_window_replacement/main.omg
```

An element that is only known to lie in `0..2` does not identify the displaced
backing. That refusal is pinned by
`tests/omega/fail/borrows/unproved_window_index_keeps_backing`.
