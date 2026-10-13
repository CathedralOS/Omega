# Intended invalid quadrature

Check `../../model/intended_invalid.omg` with `omega --check`. It imports the application’s actual quadrature module from its sibling path. The integration call supplies 321 panels,
violating the source contract `panels <= 320`. It must be rejected before runtime.
