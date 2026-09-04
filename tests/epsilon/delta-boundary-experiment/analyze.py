#!/usr/bin/env python3
"""Measure repeated mechanisms in the Delta-authored Epsilon compiler."""

from __future__ import annotations

import re
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Form:
    start_line: int
    end_line: int
    text: str
    tree: list

    @property
    def lines(self) -> int:
        return self.end_line - self.start_line + 1


def tokens(source: str):
    index = 0
    line = 1
    while index < len(source):
        byte = source[index]
        if byte in " \t\r":
            index += 1
        elif byte == "\n":
            line += 1
            index += 1
        elif byte == ";":
            end = source.find("\n", index)
            index = len(source) if end < 0 else end
        elif byte in "()":
            yield byte, line
            index += 1
        elif byte in "'\"":
            quote = byte
            start = index
            index += 1
            while index < len(source):
                if source[index] == "\\":
                    index += 2
                elif source[index] == quote:
                    index += 1
                    break
                else:
                    index += 1
            yield source[start:index], line
        else:
            start = index
            while index < len(source) and source[index] not in " \t\r\n();":
                index += 1
            yield source[start:index], line


def parse_forms(source: str) -> list[Form]:
    stream = list(tokens(source))
    forms = []
    index = 0

    def parse(position: int):
        token, start_line = stream[position]
        if token != "(":
            return token, position + 1, start_line
        result = []
        position += 1
        end_line = start_line
        while position < len(stream) and stream[position][0] != ")":
            child, position, end_line = parse(position)
            result.append(child)
        if position >= len(stream):
            raise SystemExit(f"unterminated form at line {start_line}")
        end_line = stream[position][1]
        return result, position + 1, end_line

    offsets = source.splitlines(keepends=True)
    line_starts = [0]
    for value in offsets:
        line_starts.append(line_starts[-1] + len(value))

    while index < len(stream):
        start_line = stream[index][1]
        tree, index, end_line = parse(index)
        text = "".join(offsets[start_line - 1 : end_line])
        forms.append(Form(start_line, end_line, text, tree))
    return forms


def is_data(form: Form) -> bool:
    return len(form.tree) >= 2 and form.tree[0] == "data"


def is_def(form: Form) -> bool:
    return len(form.tree) >= 2 and form.tree[0] == "def"


def constructors(form: Form):
    return form.tree[2:] if is_data(form) else []


def recursive_list(form: Form) -> bool:
    if not is_data(form) or len(constructors(form)) != 2:
        return False
    name = form.tree[1]
    first, second = constructors(form)
    return (
        isinstance(first, list)
        and isinstance(second, list)
        and len(first) == 1
        and len(second) >= 2
        and second[-1] == name
    )


def optional(form: Form) -> bool:
    return is_data(form) and str(form.tree[1]).startswith("EpsilonOptional")


def parse_outcome(form: Form) -> bool:
    if not is_data(form) or not str(form.tree[1]).endswith("ParseOutcome"):
        return False
    items = constructors(form)
    return len(items) == 2 and all(isinstance(item, list) for item in items)


def lookup(form: Form) -> bool:
    return is_data(form) and str(form.tree[1]).endswith("Lookup")


def candidate(form: Form) -> bool:
    return is_data(form) and "Candidate" in str(form.tree[1])


def function_name(form: Form) -> str:
    return str(form.tree[1]) if is_def(form) else ""


def report_group(name: str, forms: list[Form]) -> None:
    print(f"{name}_forms={len(forms)}")
    print(f"{name}_lines={sum(form.lines for form in forms)}")
    print(f"{name}_names=" + ",".join(str(form.tree[1]) for form in forms))


def constructor_shape(form: Form) -> str:
    return "/".join(
        str(len(item) - 1) for item in constructors(form) if isinstance(item, list)
    )


def report_shapes(name: str, forms: list[Form]) -> None:
    counts = Counter(constructor_shape(form) for form in forms)
    print(
        f"{name}_shapes="
        + ",".join(f"{shape}:{count}" for shape, count in sorted(counts.items()))
    )


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: analyze.py EPSILON_COMPILER.delta")
    source = Path(sys.argv[1]).read_text()
    forms = parse_forms(source)
    data = [form for form in forms if is_data(form)]
    definitions = [form for form in forms if is_def(form)]

    optionals = [form for form in data if optional(form)]
    outcomes = [form for form in data if parse_outcome(form)]
    lists = [form for form in data if recursive_list(form)]
    ordinary_lists = [form for form in lists if constructor_shape(form) == "0/2"]
    lookups = [form for form in data if lookup(form)]
    catalog_lookups = [
        form for form in lookups
        if str(form.tree[1]).startswith("EpsilonCatalog")
        or form.tree[1] == "EpsilonDataDeclarationLookup"
    ]
    candidates = [form for form in data if candidate(form)]

    reverse_functions = [
        form for form in definitions if function_name(form).startswith("epsilon_reverse_")
    ]
    template_reverse_functions = [
        form for form in reverse_functions
        if function_name(form) != "epsilon_reverse_control_references"
    ]
    count_functions = [
        form for form in definitions if function_name(form).endswith("_count")
    ]
    list_count_functions = [
        form for form in count_functions
        if function_name(form) in {
            "epsilon_expression_list_count",
            "epsilon_parameter_list_count",
            "epsilon_name_list_count",
        }
    ]
    catalog_functions = [
        form for form in definitions if function_name(form).startswith("epsilon_catalog_find_")
    ]
    span_functions = [
        form for form in definitions
        if function_name(form).endswith(("_start", "_end"))
        and not function_name(form).startswith("is_")
        and not function_name(form).startswith(("epsilon_token_", "epsilon_identifier_", "epsilon_number_", "epsilon_string_", "epsilon_character_", "epsilon_operator_"))
    ]
    minimum_functions = [
        form for form in definitions
        if function_name(form) in {
            "epsilon_census_min_present",
            "epsilon_census_min",
            "epsilon_type_formation_candidate_min_present",
            "epsilon_type_formation_candidate_min",
            "epsilon_final_candidate_add",
            "epsilon_final_candidate_merge",
        }
    ]

    print(f"source_lines={len(source.splitlines())}")
    print(f"top_level_forms={len(forms)}")
    print(f"data_forms={len(data)}")
    print(f"definition_forms={len(definitions)}")
    report_group("optional", optionals)
    report_shapes("optional", optionals)
    report_group("parse_outcome", outcomes)
    report_shapes("parse_outcome", outcomes)
    report_group("recursive_list", lists)
    report_shapes("recursive_list", lists)
    report_group("ordinary_list", ordinary_lists)
    report_group("lookup", lookups)
    report_group("catalog_lookup", catalog_lookups)
    report_shapes("catalog_lookup", catalog_lookups)
    report_group("candidate", candidates)
    report_group("reverse_function", reverse_functions)
    report_group("template_reverse_function", template_reverse_functions)
    report_group("count_function", count_functions)
    report_group("list_count_function", list_count_functions)
    report_group("catalog_function", catalog_functions)
    report_group("span_function", span_functions)
    report_group("minimum_function", minimum_functions)
    print(f"generic_sum_gross_ceiling_lines={sum(form.lines for form in optionals + outcomes)}")
    print(
        "generic_list_gross_ceiling_lines="
        + str(sum(form.lines for form in lists + reverse_functions + list_count_functions))
    )
    exact_list_family = ordinary_lists + template_reverse_functions + list_count_functions
    print(f"exact_list_family_forms={len(exact_list_family)}")
    print(f"exact_list_family_lines={sum(form.lines for form in exact_list_family)}")
    print(f"exact_list_family_bytes={sum(len(form.text.encode()) for form in exact_list_family)}")
    print(
        "catalog_gross_ceiling_lines="
        + str(sum(form.lines for form in catalog_lookups + catalog_functions))
    )
    print(f"span_gross_ceiling_lines={sum(form.lines for form in span_functions)}")
    print(
        "candidate_gross_ceiling_lines="
        + str(sum(form.lines for form in candidates + minimum_functions))
    )
    total_ceiling = (
        sum(form.lines for form in optionals + outcomes)
        + sum(form.lines for form in lists + reverse_functions + list_count_functions)
        + sum(form.lines for form in catalog_lookups + catalog_functions)
        + sum(form.lines for form in span_functions)
        + sum(form.lines for form in candidates + minimum_functions)
    )
    print(f"combined_gross_ceiling_lines={total_ceiling}")
    print(f"combined_gross_ceiling_per_mille={(total_ceiling * 1000) // len(source.splitlines())}")


if __name__ == "__main__":
    main()
