# Sibling exits retain their incoming source frontier

The first emitted exit consumes `first` through a scalar call. The other exit
selects `first` or `second` from its own incoming ownership frontier. All three
runtime paths return their authored value; an operation emitted in one sibling
cannot remove a source from another sibling.

The pre-repair producer (`fcfab3f716e7991155bb58e614e27e7810c2eabee330f758808c8dccc5268101`)
refused this valid source with `MissingSelectionSources` for `choose::entry`.

The reject twin is
[`sibling_exit_same_arm_reuses_owned_source`](../../../fail/drops/sibling_exit_same_arm_reuses_owned_source/main.omg):
it uses `first` in both operands of one comparison in the first exit.
Sibling independence never permits consuming a source twice on one path.
