The consumer imports only the dependency's `units` module. `Distance::add`
is declared in that dependency's package root, while `Span::add` is declared
beside the domains. Both operators must be discovered through their exact
domain families.

The two lexical `raw_sum` helpers deliberately differ. The root operator
returns 7; the module operator returns 21. Moving semantic association into
the domain must not change which helper the body calls.

The Product test makes a private visibility twin by removing only `public`
from `Distance::add`; the external consumer must then reject its `+` use.
