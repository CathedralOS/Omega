"""Claim-first experiment boundary; not a correctness or publication gate.

The producer's claims ARE the answer. Keep query completeness and semantic claim
granularity under review; JSON shape cannot establish either. Supply the existing
verifier prompt/API runner separately. Run this file for offline regression checks.
"""


def claim_cases(answers, tasks):
    """Validate the answer boundary and return one semantic case per cited claim.

    tasks maps IDs to question/documents (path/excerpt). A missing or nonliteral
    citation is a local warning, never an empty-evidence semantic acceptance.
    """
    if not isinstance(answers, list):
        raise ValueError('Expected an answer list')
    identities = [answer.get('id') for answer in answers if isinstance(answer, dict)]
    if len(identities) != len(answers) or len(set(identities)) != len(identities) or set(identities) != set(tasks):
        raise ValueError('Expected exactly one answer for every task')
    cases, warnings = {}, []
    for answer_index, answer in enumerate(answers):
        if set(answer) != {'id', 'claims'} or not isinstance(answer['claims'], list):
            raise ValueError('Claims must be the only answer content')
        task = tasks[answer['id']]
        documents = {item['path']: item['excerpt'] for item in task['documents']}
        if not answer['claims']:
            warnings.append(dict(answer=answer['id'], claim=None, reason='abstention'))
        for claim_index, claim in enumerate(answer['claims']):
            if (not isinstance(claim, dict) or set(claim) != {'text', 'evidence'}
                    or not isinstance(claim['text'], str) or not claim['text'].strip()
                    or not isinstance(claim['evidence'], list)):
                raise ValueError('Invalid claim structure')
            valid = bool(claim['evidence'])
            for citation in claim['evidence']:
                if (not isinstance(citation, dict) or set(citation) != {'path', 'quote'}
                        or not isinstance(citation['path'], str) or not isinstance(citation['quote'], str)):
                    raise ValueError('Invalid citation structure')
                valid = valid and citation['path'] in documents and bool(citation['quote'].strip())
                if valid:
                    valid = ' '.join(citation['quote'].split()) in ' '.join(documents[citation['path']].split())
            if not valid:
                warnings.append(dict(answer=answer['id'], claim=claim_index, reason='missing_or_nonliteral_citation'))
                continue
            identity = f'answer_{answer_index}_claim_{claim_index}'
            cases[identity] = dict(query=task['question'], claim=claim['text'], citations=claim['evidence'])
    return cases, warnings


def unsupported_claims(cases, judgments):
    """Keep every warning; do not average support probabilities across claims."""
    if set(judgments) != set(cases):
        raise ValueError('Missing or unexpected verifier judgments')
    if any(judgment.get('choice') not in ('supported', 'unsupported') for judgment in judgments.values()):
        raise ValueError('Unknown verifier choice')
    return [identity for identity in cases if judgments[identity]['choice'] == 'unsupported']


def self_check():
    task = {'one': dict(question='Who publishes?', documents=[dict(path='guide.md', excerpt='The publisher publishes.')])}
    answer = dict(id='one', claims=[dict(text='The publisher publishes.', evidence=[dict(path='guide.md', quote='The publisher publishes.')])])
    cases, warnings = claim_cases([answer], task)
    assert len(cases) == 1 and not warnings
    identity = next(iter(cases))
    assert unsupported_claims(cases, {identity: {'choice': 'unsupported'}}) == [identity]
    assert unsupported_claims(cases, {identity: {'choice': 'supported'}}) == []
    assert unsupported_claims({'first': {}, 'second': {}}, {
        'first': {'choice': 'supported'}, 'second': {'choice': 'unsupported'}}) == ['second']
    for evidence in ([], [dict(path='wrong.md', quote='The publisher publishes.')], [dict(path='guide.md', quote='Invented.')]):
        assert claim_cases([dict(id='one', claims=[dict(text='Claim', evidence=evidence)])], task)[1]
    assert claim_cases([dict(id='one', claims=[])], task)[1][0]['reason'] == 'abstention'
    for invalid in ([dict(answer, answer='Unverified extra prose')], [], [answer, answer]):
        try:
            claim_cases(invalid, task)
        except ValueError:
            pass
        else:
            raise AssertionError('Invalid answer was accepted')
    for invalid in ({}, {identity: {'choice': 'maybe'}}):
        try:
            unsupported_claims(cases, invalid)
        except ValueError:
            pass
        else:
            raise AssertionError('Invalid verdict was accepted')


if __name__ == '__main__':
    self_check()
    print('Claim boundary checks passed')
