# Applied ancestor binding with forwarded value operands

`omega run --both main.omg` must exit 0. A selected Child implementation serves
Parent<i32> through a borrowed Binding helper. The outer type occurs in the
requirement's parameter and result, so retaining only a phantom identity or
closing only direct entry-field calls does not establish this behavior.
