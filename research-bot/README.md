import unittest
from recoverlink import compare

class MatchingSafetyTests(unittest.TestCase):
    def item(self, **kwargs):
        return dict(id='ITEM', brand='Demo', model='Same model', description='Blue camera', **kwargs)
    def listing(self, **kwargs):
        return dict(id='LIST', source='synthetic://test', brand='Demo', model='Same model', description='Blue camera', **kwargs)
    def test_conflicting_serial_overrides_identical_description(self):
        self.assertEqual(compare(self.item(serial='AAA'),self.listing(serial='BBB'))['status'], 'REJECTED: conflicting serials')
    def test_same_model_does_not_establish_item_match(self):
        self.assertEqual(compare(self.item(serial=''),self.listing(serial=''))['status'], 'UNRESOLVED: similar description only')
    def test_normalized_exact_serial_is_lead_not_verified_ownership(self):
        result=compare(self.item(serial='DEMO-123'),self.listing(serial='demo 123'))
        self.assertEqual(result['status'],'EXACT SERIAL LEAD')
        self.assertEqual(result['human_decision'],'NOT REVIEWED')
    def test_serial_o_and_zero_are_not_conflated(self):
        self.assertEqual(compare(self.item(serial='AO1'),self.listing(serial='A01'))['status'],'REJECTED: conflicting serials')
    def test_shared_marking_remains_review_candidate(self):
        result=compare(self.item(serial='',markings=['Crescent sticker']),self.listing(serial='',markings=['crescent sticker']))
        self.assertEqual(result['status'],'REVIEW CANDIDATE: shared marking')
    def test_missing_serial_never_exact_match(self):
        self.assertNotEqual(compare(self.item(serial=''),self.listing(serial=''))['status'],'EXACT SERIAL LEAD')

if __name__=='__main__':unittest.main()
