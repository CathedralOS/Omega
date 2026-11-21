"""Runtime origins at the public L5 helper boundary. Not a constructor or cause inventory."""
from collections import Counter, defaultdict
import json
import re

from diagnostic_fields import readable

SOURCE = "layers/05_prepared-trees-to-terminal-psi/src/"
MODULE = re.compile(re.escape(SOURCE) + r"(?:[A-Za-z0-9_-]+/)*[A-Za-z0-9_-]+\.rs")
POSITIVE = re.compile(r"[1-9][0-9]*")


def decode(fields):
    path, line, column = fields
    if not MODULE.fullmatch(path) or not all(POSITIVE.fullmatch(n) for n in (line, column)):
        raise ValueError("expected a workspace-relative production L5 .rs path and positive Rust line and column")
    # Do not open a receipt-supplied path or compare an archive's coordinates to today's tree.
    return {"path": path, "line": int(line), "column": int(column)}


def finish(report, fixtures, statuses, captures, invalid, malformed, required):
    """Annotate observations only after the emitting test has a unique named result."""
    dispositions, sites = Counter(), set()
    # Two real helper calls can print indistinguishable rows (including the same source span).
    # Keep the raw occurrences for registry reconciliation. Neither is independent origin evidence.
    def identity(f):
        return json.dumps(dict(f, target=f['target'].replace('\\', '/')), sort_keys=True)

    keys = [(f, identity(f)) for f in fixtures if f['outcome'].startswith('refused: ')]
    multiplicity = Counter(key for _, key in keys)
    for fixture, key in keys:
        emitter = fixture['target'].replace('\\', '/'), fixture['test']
        site = fixture.get('reported_compiler_site')
        outcomes = statuses.get(emitter, [])
        if emitter in invalid:
            state = 'malformed capture'
        elif site is None:
            state = 'no origin reported'
            if required:
                report['measurement_errors'].append(f"L5 {emitter[0]}::{emitter[1]}: feature-enabled refusal missing compiler origin")
        elif outcomes != ['ok'] or captures[emitter] != 1:
            state = 'not a unique passing capture'
            if outcomes not in (['FAILED'], ['ignored']):
                report['measurement_errors'].append(f"L5 {emitter[0]}::{emitter[1]}: origin lacks a unique passing capture")
        elif multiplicity[key] != 1:
            state = 'indistinguishable repeated receipt'
        else:
            state = 'observed'
            sites.add((site['path'], site['line'], site['column']))
        if site is not None:
            fixture['origin_status'] = state
        dispositions[state] += 1
    report['lowering_origins'] = {
        'origins_required_by_reader': required, 'helper_refusals': len(keys), 'dispositions': dict(dispositions),
        'distinct_observed_sites': len(sites), 'compiler_site_denominator': None, 'malformed_fixture_records': malformed,
    }


def summary(report):
    ledger = report.get('lowering_origins')
    if ledger is None:
        return []
    counts = ledger['dispositions']
    lines = [
        '', '## Native L5 refusal origins', '',
        f"{ledger['helper_refusals']} fixture calls returned a lowering refusal. "
        f"{counts.get('observed', 0)} observations in passing tests identified "
        f"{ledger['distinct_observed_sites']} Rust construction sites. The total number of refusal decisions is unknown.",
        ', '.join(f'{n} {state}' for state, n in sorted(counts.items())) or 'No fixture call returned a refusal.',
        f"The reader found {ledger['malformed_fixture_records']} malformed fixture records. "
        "Each entry in `package_fixtures` in `evidence.json` records the program location and, when observed, "
        "the Rust location where lowering constructed the returned error.",
    ]
    witnesses = defaultdict(set)
    for fixture in report.get('package_fixtures', ()):
        if fixture.get('origin_status') == 'observed':
            site = fixture['reported_compiler_site']
            key = (site['path'], site['line'], site['column'])
            target = fixture['target'].replace('\\', '/')
            witnesses[key].add(f"{target}::{fixture['test']} · {fixture['outcome']}")
    for (path, line, column), rows in sorted(witnesses.items()):
        lines.extend(['', f'- `{path}:{line}:{column}`', *[f'  - {readable(row)}' for row in sorted(rows)]])
    return lines
