"""Reject misleading endpoint and evidence mutations before publishing the map."""
import copy
from pathlib import Path
import unittest

from .overview import overview_model, validate
from .whole_map import architecture


class OverviewContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.architecture = architecture(Path(__file__).resolve().parents[1])
        cls.model = overview_model(cls.architecture)

    def changed(self):
        return copy.deepcopy(self.model)

    def test_every_drawn_endpoint_has_a_cited_architecture_owner(self):
        owners = {n['id'] for n in self.architecture['nodes']}
        self.assertTrue(all(n['owner'] in owners for n in self.model['nodes']))
        validate(self.model)

    def test_anonymous_frame_endpoint_is_refused(self):
        model = self.changed()
        model['edges'][0]['end'] = 'psi-frame-border'
        with self.assertRaisesRegex(ValueError, 'Unknown overview endpoint'):
            validate(model)

    def test_moving_an_operation_input_to_its_output_is_refused(self):
        model = self.changed()
        edge = next(e for e in model['edges'] if e['kind']=='auxiliary')
        edge['end'] = 'instructions'
        with self.assertRaisesRegex(ValueError, 'must end at the operation'):
            validate(model)

    def test_unpinned_evidence_is_refused(self):
        model = self.changed()
        model['edges'][0]['source'] = model['edges'][0]['source'].replace(
            '3f9ce43f0c9d179048911d18faa91ba93f60176d', 'main')
        with self.assertRaisesRegex(ValueError, 'pinned file-and-line'):
            validate(model)

    def test_engine_cannot_be_drawn_as_source_compiler_caller(self):
        model = self.changed()
        edge = next(e for e in model['edges'] if e['start']=='source-engine')
        edge['end'] = 'source-caller'
        with self.assertRaisesRegex(ValueError, 'does not invoke the source compiler'):
            validate(model)

    def test_provider_closure_does_not_claim_source_recompilation(self):
        nodes = {n['id']: n for n in self.architecture['nodes']}
        for edge in self.architecture['edges']:
            if edge['start'] in ('providers','policy-inputs','policy'):
                self.assertNotEqual(nodes[edge['end']]['id'], 'source')

    def test_paths_do_not_pass_through_unrelated_boxes(self):
        nodes = {n['id']: n for n in self.model['nodes']}

        def port(node, name):
            x,y,w,h = (node[k] for k in ('x','y','width','height'))
            return {'left':(x,y+h/2),'right':(x+w,y+h/2),
                    'top':(x+w/2,y),'bottom':(x+w/2,y+h)}[name]

        def enters(a,b,node):
            # Clip the segment against the box interior. Border contacts are
            # allowed; traversing another box implies an unintended handoff.
            low,high = 0,1
            for dimension, extent in ((0,'width'),(1,'height')):
                start = node['x' if dimension==0 else 'y']+1
                stop = start+node[extent]-2
                delta = b[dimension]-a[dimension]
                if delta==0:
                    if not start<a[dimension]<stop:
                        return False
                else:
                    t0,t1=sorted(((start-a[dimension])/delta,(stop-a[dimension])/delta))
                    low,high=max(low,t0),min(high,t1)
                    if low>=high:
                        return False
            return True

        for edge in self.model['edges']:
            points=[port(nodes[edge['start']],edge['start_port']),*edge['via'],
                    port(nodes[edge['end']],edge['end_port'])]
            for node in nodes.values():
                if node['role']=='transform' or node['id'] in (edge['start'],edge['end']):
                    continue
                for a,b in zip(points,points[1:]):
                    self.assertFalse(enters(a,b,node), f'{edge["start"]} → {edge["end"]} crosses {node["id"]}')


if __name__ == '__main__':
    unittest.main()
