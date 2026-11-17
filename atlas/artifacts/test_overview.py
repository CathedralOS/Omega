"""Reject misleading endpoint and evidence mutations before publishing the map."""
import copy
from pathlib import Path
import unittest

from .overview import overview_model, layout_model, validate
from .whole_map import architecture


class GeometryContracts:
    def test_paths_do_not_pass_through_unrelated_boxes(self):
        nodes = {n['id']: n for n in self.layout['nodes']}

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

        for edge in self.layout['edges']:
            points=[(p['x'],p['y']) for section in edge['sections'] for p in section['points']]
            for node in nodes.values():
                if node['role']=='transform' or node['id'] in (edge['start'],edge['end']):
                    continue
                for a,b in zip(points,points[1:]):
                    self.assertFalse(enters(a,b,node), f'{edge["start"]} → {edge["end"]} crosses {node["id"]}')

    def test_routes_touch_the_named_endpoint_boxes(self):
        nodes={n['id']:n for n in self.layout['nodes']}
        for edge in self.layout['edges']:
            points=[p for section in edge['sections'] for p in section['points']]
            for key,point in ((edge['start'],points[0]),(edge['end'],points[-1])):
                node=nodes[key]
                x,y=point['x'],point['y']
                self.assertTrue(node['x']-1.2<=x<=node['x']+node['width']+1.2,edge['id'])
                self.assertTrue(node['y']-1.2<=y<=node['y']+node['height']+1.2,edge['id'])
                distance=min(abs(x-node['x']),abs(x-node['x']-node['width']),
                             abs(y-node['y']),abs(y-node['y']-node['height']))
                self.assertLess(distance,1.2,edge['id'])


class OverviewContracts(GeometryContracts, unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.architecture = architecture(Path(__file__).resolve().parents[1])
        cls.model = overview_model(cls.architecture)
        cls.layout = layout_model(cls.model)

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
        edge = next(e for e in model['edges'] if e['start']=='engine')
        edge['end'] = 'compiler'
        with self.assertRaisesRegex(ValueError, 'does not invoke the source compiler'):
            validate(model)

    def test_provider_closure_does_not_claim_source_recompilation(self):
        nodes = {n['id']: n for n in self.architecture['nodes']}
        for edge in self.architecture['edges']:
            if edge['start'] in ('providers','policy-inputs','policy'):
                self.assertNotEqual(nodes[edge['end']]['id'], 'source')


    def test_duplicate_component_copy_is_refused(self):
        model = self.changed()
        node = copy.deepcopy(next(n for n in model['nodes'] if n['id']=='engine'))
        node['id']='another-interpreter'
        model['nodes'].append(node)
        with self.assertRaisesRegex(ValueError,'one visible node'):
            validate(model)

    def test_all_three_interpreter_callers_share_one_node(self):
        self.assertEqual(sum(n['owner']=='engine' for n in self.model['nodes']),1)
        self.assertEqual(sum(n['role']=='owner' and n['owner']=='source' for n in self.model['nodes']),1)
        callers={e['start'] for e in self.model['edges'] if e['end']=='engine' and e['kind']=='request'}
        self.assertEqual(callers,{'source-eval','policy-eval','runtime-host'})

    def test_the_visible_graph_is_one_connected_component(self):
        neighbors={n['id']:set() for n in self.model['nodes']}
        for edge in self.model['edges']:
            neighbors[edge['start']].add(edge['end']);neighbors[edge['end']].add(edge['start'])
        reached=set();pending=['compiler']
        while pending:
            node=pending.pop()
            if node not in reached:
                reached.add(node);pending.extend(neighbors[node]-reached)
        self.assertEqual(reached,set(neighbors))

    def test_the_main_route_starts_at_build_and_runs_down_one_column(self):
        self.assertEqual(self.model['spine'][0],'build')
        self.assertEqual(self.model['spine'][-1],'native-run')
        nodes={n['id']:n for n in self.layout['nodes']}
        route=[nodes[key] for key in self.model['spine']]
        centers=[n['x']+n['width']/2 for n in route]
        self.assertLess(max(centers)-min(centers),0.1)
        for a,b in zip(route,route[1:]):
            self.assertLess(a['y']+a['height'],b['y'])

    def test_both_numbered_pipelines_are_complete_and_in_order(self):
        for prefix,count in (('psi-',7),('native-',10)):
            expected=[f'{prefix}{i}' for i in range(count)]
            self.assertEqual([key for key in self.model['spine'] if key in expected],expected)
        self.assertNotIn('engine',self.model['spine'])
        self.assertNotIn('policy-eval',self.model['spine'])

    def test_compact_canvas_bounds(self):
        self.assertLess(self.layout['width'],1800)
        self.assertLess(self.layout['height'],4000)



if __name__ == '__main__':
    unittest.main()
