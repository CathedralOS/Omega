"""Summarize repeatable work counts and explicitly diagnostic nested durations."""
from collections import Counter
import json
from pathlib import Path

folder = Path('build/resolution-measurement')
observations = json.loads((folder/'observations.json').read_text())
public_resolvers = {
    'resolver.resolve', 'resolver.resolve_deferred', 'resolver.resolve_deferred_numeric_probe',
    'resolver.resolve_const_argument_selection', 'resolver.resolve_numeric_probe',
    'resolver.resolve_deferred_initializer_probe', 'resolver.prepare_const_initializer_selection',
    'resolver.resolve_extension', 'resolver.complete_bodies',
}
summary = {}
for subject in dict.fromkeys(row['subject'] for row in observations['records']):
    rows = [row for row in observations['records'] if row['subject']==subject and not row['warmup']]
    item = {'configurations': {}}
    for producer, traced in [('baseline',False),('instrumented',False),('instrumented',True)]:
        samples = [row for row in rows if row['producer']==producer and row['traced']==traced]
        item['configurations'][producer+('-trace' if traced else '')] = {
            'wall_ms': [row['wall_ms'] for row in samples],
            'user_cpu_ms': [row['user_cpu_ms'] for row in samples],
            'system_cpu_ms': [row['system_cpu_ms'] for row in samples],
            'peak_rss_bytes': [row['peak_rss_bytes'] for row in samples],
            'timings_ms': [row['timings_ms'] for row in samples],
        }
    captures = [row['trace']['events'] for row in rows if row['traced']]
    item['diagnostic'] = []
    for events in captures:
        capture = {}
        for domain in ('pre_resolution','later'):
            selected = [event for event in events if
                        ('pre_resolution' in event['parents']) == (domain=='pre_resolution')]
            resolver_events = [event for event in selected if event['label'] in public_resolvers]
            outer_resolvers = [event for event in resolver_events if not
                              any(parent in public_resolvers for parent in event['parents'])]
            capture[domain] = dict(
                resolver_calls=dict(Counter(event['label'] for event in resolver_events)),
                outer_resolver_ms=sum(event['elapsed_ns'] for event in outer_resolvers)/1e6,
                lower_items_calls=sum(event['label']=='resolver.lower_items' for event in selected),
                root_visits=sum(event['fields']['roots'] for event in selected if event['label']=='resolver.lower_items'),
                syntax_clone_calls=sum(event['label']=='syntax.clone' for event in selected),
                expression_table_clone_calls=sum(event['label']=='resolved.expressions.clone' for event in selected),
                live_expression_rows_cloned=sum(event['fields']['expressions'] for event in selected if event['label']=='resolved.expressions.clone'),
                expression_table_clone_ms=sum(event['elapsed_ns'] for event in selected if event['label']=='resolved.expressions.clone')/1e6,
            )
        capture['pre_resolution_ms'] = sum(event['elapsed_ns'] for event in events if event['label']=='pre_resolution')/1e6
        capture['driver_prepare_ms'] = sum(event['elapsed_ns'] for event in events if event['label']=='driver.prepare')/1e6
        capture['clone_contexts'] = dict(Counter('/'.join(event['parents']) for event in events if event['label']=='resolved.expressions.clone'))
        item['diagnostic'].append(capture)
    summary[subject] = item
(folder/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
