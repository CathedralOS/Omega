#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT

command -v python3 >/dev/null 2>&1 || {
    echo "OCREQ tables: skipped (python3 absent)"
    exit 0
}

# The standalone request contract's closed tables must agree with the
# projections embedded in the D members before either implementation can
# claim the V1 boundary. This gate compares the markdown records in
# wiki/spec/build/compiler_request.md against the constant tables compiled
# into bootstrap/5_omega and the copies asserted by the omega-outcome gate:
# a table that drifts from any record refuses here without executing a seed.
python3 -B - "$OMEGA_REPO_ROOT" <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
spec = (root / "wiki/spec/build/compiler_request.md").read_text(encoding="utf-8")
omega5 = root / "bootstrap/5_omega"
request_src = (omega5 / "request_and_utf8.epsilon").read_text(encoding="utf-8")
outcome_src = (omega5 / "outcome.epsilon").read_text(encoding="utf-8")
scalar_src = (omega5 / "scalar_compilation.epsilon").read_text(encoding="utf-8")
outcome_gate = (
    root / "tests/bootstrap/omega-outcome/main.epsilon"
).read_text(encoding="utf-8")

failures = []
checks = 0


def agree(what, actual, expected):
    global checks
    checks += 1
    if actual != expected:
        failures.append(f"{what}: embedded {actual!r}, contract {expected!r}")


def machine_body(source, name):
    marker = f"machine {name}"
    start = source.index(marker)
    tail = source[start:]
    end = tail.find("\nmachine ", len(marker))
    return tail[:end] if end != -1 else tail


def int_arms(body):
    return {
        int(code): int(value)
        for code, value in re.findall(r"(\d+) -> return (-?\d+)", body)
    }


def table_row(cells):
    return [cell.strip() for cell in cells.split("|") if cell.strip() != ""]


# Contract identity rows: the request frame's eight identity bytes and the
# failure frame's eight identity bytes, in order.
spec_ocreq = [
    int(token, 16)
    for token in re.search(
        r"Eight identity bytes: `([0-9A-F ]+)`", spec
    ).group(1).split()
]
spec_ocout = [
    int(token, 16)
    for token in re.search(
        r"failure frame's eight-byte identity is `([0-9A-F ]+)`", spec
    ).group(1).split()
]

ocreq_machine = machine_body(request_src, "omega_ocreq_identity_byte")
ocreq_arms = int_arms(ocreq_machine)
ocreq_default = int(re.search(r"_ -> return (-?\d+)", ocreq_machine).group(1))
embedded_ocreq = [
    ocreq_arms.get(index, ocreq_default) for index in range(8)
]
agree("OCREQ identity bytes", embedded_ocreq, spec_ocreq)

identity_state = outcome_src.split("state write_identity() {", 1)[1]
identity_state = identity_state.split("\n    state ", 1)[0]
embedded_ocout = [
    value
    for _, value in sorted(
        (int(index), int(value))
        for index, value in re.findall(
            r"self\.bytes\[(\d)\] = (\d+);", identity_state
        )
    )
]
agree("OCOUT identity bytes", embedded_ocout, spec_ocout)

# Common-header layout: the spec's offset table assigns tag, space, reserved
# zero pair, and three little-endian fields; the encoder's writes must land
# on exactly those offsets.
def spec_table(header, after=None):
    """Rows of the markdown table whose header line equals `header`,
    optionally the first such table following `after` text."""
    body = spec.split(after, 1)[1] if after else spec
    lines = body.splitlines()
    start = lines.index(header)
    rows = []
    for line in lines[start + 1:]:
        if not line.startswith("|"):
            if rows:
                break
            continue
        cells = table_row(line[1:-1])
        if cells and all(re.fullmatch(r"-+", cell) for cell in cells):
            continue
        rows.append(cells)
    return rows


layout = [
    int(cells[0])
    for cells in spec_table(
        "| Offset | Encoding |", after="assigned common-header layout"
    )
    if re.fullmatch(r"\d+", cells[0])
]
agree("OCOUT header offsets", sorted(layout), [0, 8, 9, 10, 12, 16, 24, 32])
encoder_region = outcome_src.split("state write_identity() {", 1)[1]
write_offsets = sorted(
    int(offset)
    for offset in re.findall(r"self\.write_u(?:32|64)\((\d+), ", encoder_region)
)
agree("OCOUT field write offsets", write_offsets, [12, 16, 24, 32, 40, 44])
agree(
    "OCOUT reserved zero pair",
    sorted(set(
        int(index)
        for index in re.findall(r"self\.bytes\[(\d+)\] = 0;", identity_state)
    )),
    [7, 10, 11],
)
agree(
    "OCOUT frame extents",
    sorted(set(int(n) for n in re.findall(r"finish\((\d+)\)", encoder_region))),
    [40, 48],
)

# Scalar-resource table: every projected code's limit and coordinate space
# agree across the contract and both embedded records. The contract assigns
# coverage provisions at codes 15-25 and the request staging capacity at 26;
# the gate pins that every contract-assigned code is projected, so a newly
# assigned row that is not embedded trips here instead of diverging silently.
resources = {}
for cells in spec_table("| Code | Resource | Limit | Space | Coordinate |"):
    if (
        len(cells) >= 4
        and re.fullmatch(r"\d+", cells[0])
        and re.fullmatch(r"`[a-z_0-9]+`", cells[1])
        and re.fullmatch(r"[\d,]+", cells[2])
        and re.fullmatch(r"\d+ [a-z ]+", cells[3])
    ):
        resources[int(cells[0])] = (
            int(cells[2].replace(",", "")), int(cells[3].split()[0])
        )

limit_machine = machine_body(outcome_src, "omega_compiler_resource_limit")
space_machine = machine_body(outcome_src, "omega_compiler_resource_space")
embedded_limits = int_arms(limit_machine)
embedded_spaces = int_arms(space_machine)
limit_default = int(re.search(r"_ -> return (-?\d+)", limit_machine).group(1))
space_default = int(re.search(r"_ -> return (-?\d+)", space_machine).group(1))
projected = sorted(embedded_limits)
agree(
    "resource limit/space machines share one code set",
    projected,
    sorted(embedded_spaces),
)
agree(
    "projected resource codes are contract-assigned",
    set(projected) <= set(resources),
    True,
)
for code in projected:
    limit, space = resources[code]
    agree(f"resource {code} limit", embedded_limits[code], limit)
    agree(f"resource {code} space", embedded_spaces[code], space)
agree("resource limit default arm", limit_default, 0)
agree("resource space default arm", space_default, -1)
unprojected = sorted(set(resources) - set(projected))
agree(
    "pending resource provisions",
    unprojected,
    [],
)

gate_limits = dict(
    (int(code), int(limit))
    for code, limit in re.findall(
        r"omega_compiler_resource_limit\((\d+)\) == (\d+)", outcome_gate
    )
)
gate_spaces = dict(
    (int(code), int(space))
    for code, space in re.findall(
        r"omega_compiler_resource_space\((\d+)\) == (-?\d+)", outcome_gate
    )
)
for code, limit in sorted(gate_limits.items()):
    agree(
        f"gate resource {code} limit",
        limit, embedded_limits.get(code, limit_default),
    )
for code, space in sorted(gate_spaces.items()):
    agree(
        f"gate resource {code} space",
        space, embedded_spaces.get(code, space_default),
    )
# The gate copies pin the unassigned markers: resource limit has no arm
# boundary assert beyond the assigned inventory, so code 0 stands in as the
# unassigned limit sentinel while code 27 is the first space-unassigned code
# above the assigned 1-26 range the gate asserts.
agree("gate unassigned resource limit", gate_limits.get(0), 0)
agree("gate unassigned resource space", gate_spaces.get(27), -1)

# Assigned Reject codes: the contract's lexical inventory maps by name; the
# embedded projection returns the same wire codes.
reject_rows = {}
for cells in spec_table(
    "| Tag | Code | Name | Space | Coordinate | Limit/requested |"
):
    if len(cells) >= 4 and re.fullmatch(r"`[a-z_0-9]+`", cells[2] or ""):
        reject_rows[cells[2][1:-1]] = (int(cells[1]), int(cells[3].split()[0]))

# Reject coordinate-space table: the embedded projection carries exactly the
# contract's assigned Reject codes, each at its assigned space. An assigned
# row missing from the projection, a drifted space, or a code the contract
# never assigned all fail here.
spec_reject_spaces = {
    int(cells[1]): int(cells[3].split()[0])
    for cells in spec_table(
        "| Tag | Code | Name | Space | Coordinate | Limit/requested |"
    )
    if cells[0] == "1 `Reject`"
}
reject_space_machine = machine_body(outcome_src, "omega_reject_code_space")
embedded_reject_spaces = int_arms(reject_space_machine)
reject_space_default = int(
    re.search(r"_ -> return (-?\d+)", reject_space_machine).group(1)
)
agree(
    "reject space code set",
    sorted(embedded_reject_spaces),
    sorted(spec_reject_spaces),
)
for code, space in sorted(spec_reject_spaces.items()):
    agree(
        f"reject code {code} space",
        embedded_reject_spaces.get(code),
        space,
    )
agree("reject space default arm", reject_space_default, -1)

lexical_names = {
    "invalid_utf8": "InvalidUtf8",
    "outside_lexical_profile": "OutsideLexicalProfile",
    "unterminated_block_comment": "UnterminatedBlockComment",
    "unterminated_string_literal": "UnterminatedStringLiteral",
    "unterminated_string_escape": "UnterminatedStringEscape",
    "unsupported_escape": "UnsupportedEscape",
    "unterminated_hex_escape": "UnterminatedHexEscape",
    "invalid_hex_escape_digit": "InvalidHexEscapeDigit",
}
embedded_lexical = {
    name: int(code)
    for name, code in re.findall(
        r"OmegaLexDiagnosticCode::(\w+) -> return (\d+)",
        machine_body(outcome_src, "omega_lexical_reject_code"),
    )
}
for spec_name, case in lexical_names.items():
    code, space = reject_rows[spec_name]
    agree(f"lexical {spec_name} code", embedded_lexical.get(case), code)
    agree(f"lexical {spec_name} space", 4, space)

# Non-lexical assigned codes publish as literal reject tuples; each tuple
# must carry the coordinate space the contract's row assigns. Lexical codes
# reach the frame through the named projection checked above, not literals.
embedded_rejects = set()
for source in (scalar_src, request_src):
    for code, space in re.findall(
        r"(?:record_)?reject\((\d+), (\d+),", source
    ):
        embedded_rejects.add((int(code), int(space)))
refuse_blocks = re.findall(
    r"state refuse(?:_envelope)?\(\) \{(.*?)\}", request_src, re.S
)
for block in refuse_blocks:
    space = re.search(r"self\.outcome_space = (\d+)", block)
    code = re.search(r"self\.outcome_code = (-?\d+)", block)
    if space and code:
        embedded_rejects.add((int(code.group(1)), int(space.group(1))))
agree(
    "assigned reject (code, space) tuples",
    sorted(embedded_rejects),
    sorted(set(
        (code, space)
        for name, (code, space) in reject_rows.items()
        if name in {
            "malformed_request", "duplicate_name", "missing_entry",
            "integer_literal_out_of_range",
        }
    )),
)

# InternalFailure: one assigned code anchored in the internal-row space.
internal_machine = machine_body(outcome_src, "omega_internal_code_space")
internal_space = int_arms(internal_machine)
spec_internal = reject_rows.get("invariant_violation")
internal_default = int(
    re.search(r"_ -> return (-?\d+)", internal_machine).group(1)
)
agree(
    "internal failure code 1 space",
    internal_space.get(1),
    spec_internal[1] if spec_internal else None,
)
agree("internal code default arm", internal_default, -1)
agree("explicit internal code arms", sorted(internal_space), [1])

# Invocation product tags: the request walker admits exactly the contract's
# assigned range.
product_match = re.search(
    r"let product: i32 = self\.take_tag\(request, (\d+), (\d+)\)",
    request_src,
)
spec_products = re.search(
    r"`product` \| `u32` tag: ((?:\d+ `[a-z_0-9]+`(?:, )?)+)", spec
).group(1)
spec_product_tags = sorted(
    int(tag) for tag in re.findall(r"(\d+) `", spec_products)
)
agree(
    "invocation product tag range",
    (int(product_match.group(1)), int(product_match.group(2))),
    (min(spec_product_tags), max(spec_product_tags)),
)
agree("invocation product count", len(spec_product_tags), 4)

if failures:
    for failure in failures:
        print(f"OCREQ tables: {failure}", file=sys.stderr)
    sys.exit(1)
print(f"OCREQ tables: {checks} contract/embedded records agree")
PY
