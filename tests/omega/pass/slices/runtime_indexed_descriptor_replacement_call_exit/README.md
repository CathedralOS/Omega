# Indexed descriptor replacement across a call

Replace a mutable view field through an indexed record view, then return the
updated descriptor through an ordinary helper. Both execution engines must
observe the new count and full-width record fields while old backing stays
unchanged. This differs from direct observation: native call-result backing
analysis must account for the indexed store instead of losing all contents.
The collection has two distinct borrowed backing arrays, so an analysis that
erases selector coordinates must retain every candidate rather than treating
the destination as a singleton. The untouched array is checked as well.
