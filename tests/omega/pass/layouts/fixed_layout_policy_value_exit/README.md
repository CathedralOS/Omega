A table validator can inspect ordinary fields stored with an explicit layout and prove that an accepted value satisfies its policy. The compiler used to reject the layout wrapper as opaque, even though its semantic fields remain ordinary values.

The program checks an accepted value, a different refused value, an updated value, and both nonzero neighboring fields. The wrong-validator control always returns true and must fail its guarantee. The borrowed-content control puts a reference inside the laid-out record; a layout must not make that reference a repeatable proof value.

This is the general layout-policy repair required by Cathedral's timer table validation. It does not establish timer installation or hardware interrupt delivery.
