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
        self.assertEqual(spine[0],'admission')
        self.assertEqual(spine[-1],'published')
        self.assertEqual([n['id'] for n in self.model['nodes']],spine)
        self.assertEqual([(e['start'],e['end']) for e in self.model['edges']],list(zip(spine,spine[1:])))
        for edge in self.model['edges']:
            if edge['start'] in positions and edge['end'] in positions:
                self.assertEqual(positions[edge['end']],positions[edge['start']]+1)
        nodes={n['id']:n for n in self.layout['nodes']}
        for a,b in zip(spine,spine[1:]):
            self.assertLess(nodes[a]['y']+nodes[a]['height'],nodes[b]['y'])
            self.assertEqual(nodes[a]['x'],nodes[b]['x'])

    def test_side_services_and_extra_dependencies_are_rejected(self):
        for mutation in ('node','edge','return','skip'):
            model=copy.deepcopy(self.model)
            if mutation=='node':
                node=copy.deepcopy(model['nodes'][0])
                node.update(id='executor',owner='executor')
                model['nodes'].append(node)
            elif mutation=='edge':
                model['edges'].append(copy.deepcopy(model['edges'][0]))
            elif mutation=='return':
                model['edges'][0]['start'],model['edges'][0]['end']=model['edges'][0]['end'],model['edges'][0]['start']
            else:
                model['edges'][0]['end']=model['spine'][2]
            with self.assertRaisesRegex(ValueError,'one forward chain'):
                validate(model)

    def test_numbered_stages_keep_open_contracts_explicit(self):
        nodes={n['id']:n for n in self.model['nodes']}
        for key in ('admission','source-settlement','seal','native-planning','physical-planning','binding','encoding'):
            self.assertTrue(nodes[key]['gate'])
        stages=[n for n in self.model['nodes'] if n['role']=='owner']
        self.assertEqual([n['number'] for n in stages],list(range(11)))
        self.assertEqual([n['title'][:2] for n in stages],[f'{i:02d}' for i in range(11)])
        model=copy.deepcopy(self.model)
        next(n for n in model['nodes'] if n['id']=='source-settlement')['title']='Compile source'
        with self.assertRaisesRegex(ValueError,'numbered 00–10'):
            validate(model)

    def test_source_operations_and_evaluation_are_visible_in_order(self):
        operations=[n for n in self.model['nodes'] if n['role']=='operation' and n['owner']=='source-settlement']
        self.assertEqual([n['id'].removeprefix('source-settlement-') for n in operations],
                         ['lex','parse','resolve','type','prepare','lower','check','optimize','recheck'])
        self.assertIn('prepare → lower → check',operations[4]['notes'])
        self.assertIn('admit → execute → value',operations[4]['notes'])
        self.assertIn('selection only',operations[-1]['notes'])
        self.assertEqual(len(self.model['phases']),11)
        for stage in (n for n in self.model['nodes'] if n['role']=='owner'):
            self.assertTrue(any(n['role']=='operation' and n['owner']==stage['id'] for n in self.model['nodes']))


if __name__=='__main__':
    unittest.main()
