"""Associate the fixed sample's compiler refusal with its observed outcome.

`OMEGA_SAMPLE_REFUSAL` receipts are printed by `Refusal::marker` in
layers/05_prepared-trees-to-terminal-psi/tests/sample_baseline/pipeline.rs:

    OMEGA_SAMPLE_REFUSAL label stage package file start end line column occurrence reason

separated by tabs. `-` marks an unavailable line, column or occurrence. The reason
escapes `%`, tab, carriage return and line feed as `%25`, `%09`, `%0D` and `%0A`.
The occurrence escapes the same characters, writes each byte of an invalid UTF-8
sequence as `%XX` and writes a lone literal `-` as `%2D`. A receipt is accepted
only when its fields are exactly what that emitter writes for the bytes they denote.
"""
import json
import re
from diagnostic_fields import readable, unescape

from frontend_diagnostics import compiler_stop, location

TAG = "OMEGA_SAMPLE_REFUSAL"
ROOT = "root"
UNAVAILABLE = "-"
STAGES = {"L0", "L1", "L2", "L3", "L3-resumed", "L4", "L5"}
REFUSED = {"compatibility blocked", "intended rejection verified"}
# A refused configuration that broke its expectation; an unexpected product is `unexpected Psi product`.
INSPECTION_FAILED = "inspection failed"
NUMBER = re.compile(r"0|[1-9][0-9]*")
TEXT_ESCAPES = {"%": "%25", "\t": "%09", "\r": "%0D", "\n": "%0A"}


def escape(text):
    """The emitter's `escape`: `%` first, then tab, carriage return and line feed."""
    return "".join(TEXT_ESCAPES.get(character, character) for character in text)


def escape_occurrence(data):
    """The emitter's occurrence field for source bytes, or for None when they are unavailable."""
    if data is None:
        return UNAVAILABLE
    if data == b"-":
        return "%2D"
    # surrogateescape maps exactly the bytes outside valid UTF-8 sequences to U+DC80..U+DCFF.
    return "".join(f"%{ord(character) - 0xDC00:02X}" if "\udc80" <= character <= "\udcff" else escape(character)
                   for character in data.decode("utf-8", errors="surrogateescape"))


def decode_reason(field):
    if not field:
        raise ValueError("missing compiler reason")
    try:
        text = unescape(field).decode("utf-8")
    except UnicodeDecodeError:
        raise ValueError("compiler reason is not UTF-8 text") from None
    if escape(text) != field:
        raise ValueError("compiler reason is not escaped as the emitter escapes it")
    return text


def decode_occurrence(field):
    """The occurrence bytes, or None when the field marks them unavailable."""
    if field == UNAVAILABLE:
        return None
    data = unescape(field)
    if escape_occurrence(data) != field:
        raise ValueError("source occurrence is not escaped as the emitter escapes it")
    return data


def number(field, name):
    if not NUMBER.fullmatch(field):
        raise ValueError(f"{name} is not a decimal byte count: {field!r}")
    return int(field)


def occurrence_record(data):
    """Readable text when the bytes are UTF-8; hexadecimal bytes only when they are not."""
    if data is None:
        return {"occurrence_form": "unavailable", "occurrence": None}
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return {"occurrence_form": "invalid UTF-8", "occurrence": None, "occurrence_hex": data.hex()}
    return {"occurrence_form": "text" if text else "empty", "occurrence": text}


def parse(line):
    """Return (label, receipt). A receipt naming its label but failing validation is returned
    as {"invalid": why, "line": line}, so `attach` can tell a present but invalid receipt from
    a missing one. Lines without a label raise ValueError."""
    fields = line.split("\t")
    if fields[0] != TAG or len(fields) < 2 or not fields[1]:
        raise ValueError("missing sample refusal tag or sample label")
    label = fields[1]
    try:
        return label, decode(fields)
    except ValueError as error:
        return label, {"invalid": str(error), "line": line}


def decode(fields):
    if len(fields) != 11:
        raise ValueError(f"expected 11 tab-separated fields, found {len(fields)}")
    _, _, stage, package, file, start, end, row, column, occurrence, reason = fields
    if stage not in STAGES:
        raise ValueError(f"not a compiler stage: {stage!r}")
    if not package or not file:
        raise ValueError("missing package or file")
    start, end = number(start, "start"), number(end, "end")
    if end < start:
        raise ValueError(f"byte range {start}..{end} ends before it starts")
    data = decode_occurrence(occurrence)
    reason = decode_reason(reason)
    if (row == UNAVAILABLE) != (column == UNAVAILABLE):
        raise ValueError("incomplete line and byte column")
    if row == UNAVAILABLE:
        # Without the package's bytes at the span start there is neither a position nor an occurrence.
        if data is not None:
            raise ValueError("source text without a source position")
        site = location(file, "", "", start, end)
    else:
        line, byte_column = number(row, "line"), number(column, "byte column")
        if line < 1 or byte_column < 1:
            raise ValueError("source positions start at one")
        # Each earlier line ends in a line feed, and the column counts bytes from the line start.
        if (line - 1) + (byte_column - 1) > start or (line == 1 and byte_column != start + 1):
            raise ValueError(f"line {line}, byte column {byte_column} cannot start at byte {start}")
        if data is None and start == end:
            raise ValueError("an empty span inside the file has an empty occurrence, not an unavailable one")
        if data is not None and len(data) != end - start:
            raise ValueError(f"source occurrence has {len(data)} bytes but the range {start}..{end} has {end - start}")
        site = location(file, row, column, start, end)
    return {**site, **compiler_stop(stage), "package": package, **occurrence_record(data), "reason": reason}


def raw_stage(record):
    return "L3-resumed" if record.get("after_static_evaluation", False) else record["stage"]


def rendered(site):
    """The baseline row after its stage, as the sample test flattens it into a case detail."""
    reason = re.sub(r"[\t\r\n]", " ", site["reason"])
    if "line" in site:
        owner = "" if site["package"] == ROOT else f"<{site['package']}>/"
        return f"{owner}{site['file']}:{site['line']}:{site['byte_column']} {reason}"
    return f"<{site['package']}>/{site['file']} (bytes {site['byte_start']}..{site['byte_end']}) {reason}"


def needs_refusal(case):
    """Whether the sample test prints a compiler refusal for this observed outcome."""
    return case["status"] in REFUSED | {INSPECTION_FAILED}


def disagreement(case, site):
    if case is None:
        return "compiler refusal for a sample without a case result"
    if case["status"] not in REFUSED | {INSPECTION_FAILED}:
        return f"compiler refusal for a case reported as {case['status']}"
    if (case["stage"], case.get("after_static_evaluation", False)) != (site["stage"], site["after_static_evaluation"]):
        return f"compiler stopped at {raw_stage(site)} but the case reports {raw_stage(case)}"
    detail, row = case["detail"], rendered(site)
    if case["status"] in REFUSED:
        if detail != row:
            return f"case detail {detail!r} disagrees with the compiler refusal {row!r}"
    else:
        # `expected Psi, got <refusal>` or `expected <other refusal>, got <refusal>`.
        got = f", got {raw_stage(site)} {row}"
        if not (detail.startswith("expected ") and detail.endswith(got) and len(detail) > len("expected ") + len(got)):
            return f"inspection failure {detail!r} does not end with the compiler refusal{got!r}"
    return None


def attach(cases, observed, errors):
    for label, site in observed.items():
        key = "sample/" + label
        if "invalid" in site:
            errors.append(f"L5: invalid sample refusal: {site['invalid']}: {site['line']}")
            continue
        case = cases.get(key)
        if problem := disagreement(case, site):
            errors.append(f"{key}: {problem}")
            continue
        case["compiler_refusal"] = site
        case["later_layers_not_reached"] = site["later_layers_not_reached"]
    for key, case in cases.items():
        # A receipt that was printed but rejected above is already reported.
        if case.get("cohort") != "sample" or "compiler_refusal" in case or key.removeprefix("sample/") in observed:
            continue
        if needs_refusal(case):
            errors.append(f"{key}: missing the compiler's original refusal")


def snippet(site):
    form = site["occurrence_form"]
    if form == "text":
        return json.dumps(site["occurrence"], ensure_ascii=False)
    if form == "empty":
        return "empty span"
    if form == "invalid UTF-8":
        return "invalid UTF-8 bytes " + " ".join(f"{byte:02X}" for byte in bytes.fromhex(site["occurrence_hex"]))
    return "source bytes unavailable"


def summary(cases):
    samples = [(key.removeprefix("sample/"), case) for key, case in cases.items() if case.get("cohort") == "sample"]
    lines = []
    for label, case in samples:
        site = case.get("compiler_refusal")
        if not site:
            if needs_refusal(case):
                lines.append(f"- {label} · {case['status']} · {raw_stage(case)} · compiler refusal not recorded")
            continue
        address = f"<{site['package']}>/{site['file']}"
        if "line" in site:
            address += f":{site['line']}:{site['byte_column']}"
        lines.append(f"- {label} · {case['status']} · stopped at {raw_stage(site)} · {address} · "
                     f"bytes {site['byte_start']}..{site['byte_end']} · {snippet(site)}")
        lines.extend("  " + readable(part) for part in site["reason"].split("\n"))
    return ["", "### First compiler refusal in the fixed samples", "", *lines] if lines else []
