"""Independent mathematical oracle; no compiler output or runtime extraction."""
from dataclasses import dataclass
import json


@dataclass(frozen=True)
class Representative:
    value: int


def equivalent(left, right):
    return left == right


def representative(value):
    return value


def representative_value(value):
    return value.value


def table():
    inputs = [("local", 11), ("field", 13), ("parameter", 17), ("call-result", 29)]
    rows = []
    for path, value in inputs:
        chosen = Representative(value)
        result = representative_value(representative(chosen))
        rows.append({"path": path, "representative": value, "result": result})
    # Check both selected result laws over a finite independent cross-product.
    # This computation illustrates the laws; the compiler must prove them for
    # every representative using the authored theorem bodies.
    domain = [Representative(n) for n in range(-32, 33)]
    for left in domain:
        for right in domain:
            if equivalent(left, right):
                assert equivalent(representative(left), representative(right))
                assert representative_value(left) == representative_value(right)
    return {"relation": "exact Representative equality", "rows": rows,
            "expected_exit": sum(row["result"] for row in rows)}


if __name__ == "__main__":
    print(json.dumps(table(), indent=2, sort_keys=True))
