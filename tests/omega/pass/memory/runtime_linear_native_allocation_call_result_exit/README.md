# Ordinary helper allocation custody

The same real malloc/free journey forwards each issued allocation through an
ordinary call result and consumes it through another machine parameter after a
state transition. It preserves the non-null/distinct checks and exit-0 oracle of
the direct allocation fixture.
