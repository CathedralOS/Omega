# Indexed scan-window exchange

Expected exit: 0. Exchange both positions of a borrowed window collection through
one runtime-indexed helper. Each returned window keeps the displaced backing;
the installed window keeps its new extent and full-width tags. A write through
the returned view must reach the original array, and an untouched neighbor must
retain its contents.

After ordinary package review:

```sh
omega run --both samples/cli/collections/indexed_scan_windows/main.omg
```

The program requires native and Terminal exit 0. The companion corpus fixtures
exercise helper forwarding and implicit reborrowing of a padded descriptor with
a nonzero backing offset. Indexed writes preserve possible previous and new
backing origins across calls; that provenance does not grant access to vacant
storage or erase the obligation to restore a moved-out element.

This is a compiler capability needed by scan-window manipulation, not acceptance
of the complete Squalr application.
