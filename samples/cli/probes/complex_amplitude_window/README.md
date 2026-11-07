# Complex amplitude window

A four-cell complex amplitude row lends its left or right half as a mutable
window. The window is stored in a record field, scaled by a helper, and the
scaled first cell is observed through a local, that field, a parameter, and a
call result. The other half is unchanged.

`omega run` on this program exits 0 when those observations match the scaled
row. A wrong scale crashes. `controls/past_window` is a separate source
project: index 2 in the length-2 window must be rejected.

```sh
omega run samples/cli/independent/complex_amplitude_window/main.omg < /dev/null
```
