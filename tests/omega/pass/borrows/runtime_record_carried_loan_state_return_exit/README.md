# Record return loans cross ordinary states

A record carrying a shared reference is transformed through an ordinary
state edge, then returned unchanged through the other edge. Its marker changes
from 41 to 42 while both returned references read the original backing value 42.
This challenges the result-origin join independently of borrowed-storage windows.
The states put scalar parameters on different sides of the carrier, so authored
positions cannot stand in for dense structural ingress positions. A second
carrier stores a shared primitive reference and passes it to a reader; that
descriptor must not be normalized into an integer field initializer.

After package review, `omega run --both main.omg` must return 42 natively and in
Terminal. The checked-tree interpreter is a separate control.
