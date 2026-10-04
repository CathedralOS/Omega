# Build and Product proof ownership

Build and Product declare separate domains and qualified callable contracts.
Build checking must traverse its own contracts without treating the Product
facts retained in backing arenas as Build obligations. Product checking must
then check the authored Product contracts normally.

Run `omega --check tests/omega/pass/build/declaration_domain_context/main.omg`.
The `helper_contract_context` negative fixture keeps an invalid Build helper
contract in scope, so excluding Product facts cannot excuse Build obligations.
