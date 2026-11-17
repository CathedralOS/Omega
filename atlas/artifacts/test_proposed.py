"""Keep the planned route honest and its arrows attached to the right boxes."""
import copy
import unittest

from .overview import layout_model, validate
from .proposed import proposed_model
from .test_overview import GeometryContracts


class ProposedContracts(GeometryContracts, unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.model=proposed_model()
        cls.layout=layout_model(cls.model)

    def test_proposal_cannot_claim_implementation_evidence_or_completion(self):
        for mutation in ('status','source'):
            model=copy.deepcopy(self.model)
            if mutation=='status':
                model['status']='implemented'
            else:
                model['edges'][0]['source']=model['edges'][0]['source'].replace('atlas/plans/foundation-and-backend.md','omega-rust/pipeline.md')
            with self.assertRaisesRegex(ValueError,'unverified and cite the recorded plan'):
                validate(model)

    def test_main_program_moves_forward_without_an_earlier_form_side_input(self):
        spine=self.model['spine']
        positions={key:i for i,key in enumerate(spine)}
        self.assertEqual(spine[0],'build')
        self.assertEqual(spine[-1],'native-run')
        for edge in self.model['edges']:
            if edge['start'] in positions and edge['end'] in positions:
                self.assertEqual(positions[edge['end']],positions[edge['start']]+1)
        nodes={n['id']:n for n in self.layout['nodes']}
        for a,b in zip(spine,spine[1:]):
            self.assertLess(nodes[a]['y']+nodes[a]['height'],nodes[b]['y'])
            self.assertEqual(nodes[a]['x'],nodes[b]['x'])

    def test_execution_uses_one_engine_and_preserves_the_portable_runtime_route(self):
        self.assertEqual(sum(n['id']=='engine' for n in self.model['nodes']),1)
        callers={e['start'] for e in self.model['edges'] if e['end']=='engine' and e['kind']=='request'}
        self.assertEqual(callers,{'executor','runtime-host'})
        self.assertIn(('portable','runtime-host'),{(e['start'],e['end']) for e in self.model['edges']})
        self.assertNotIn('runtime-host',self.model['spine'])
        self.assertTrue(all(e['end'] in ('executor','runtime-host') for e in self.model['edges'] if e['start']=='engine'))

    def test_open_contracts_remain_explicit_without_invented_stage_numbers(self):
        nodes={n['id']:n for n in self.model['nodes']}
        for key in ('source-settlement','native-planning','physical-planning','binding','encoding','runtime-provisions'):
            self.assertTrue(nodes[key]['gate'])
        self.assertFalse(any(e['kind']=='check' for e in self.model['edges']))
        self.assertFalse(any(n['title'][:2].isdigit() for n in self.model['nodes']))


if __name__=='__main__':
    unittest.main()
