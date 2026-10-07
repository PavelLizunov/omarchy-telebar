"""Retained recordings and explicit retry through inert real IPC/TDLib."""
import pathlib
import unittest
from unittest import mock
import daemon_test


class RecordingRetry(daemon_test.Harness):
    def setUp(self):
        super().setUp()
        self.conn = self.connect()
        self.sign_in(self.conn)
        def voice_argv(path):
            pathlib.Path(path).write_bytes(b'OggS')
            return [daemon_test.sys.executable, '-c', 'import time; time.sleep(30)']
        def video(source, target):
            pathlib.Path(target).write_bytes(b'synthetic mp4')
            return 7
        for name, value in [('voice_argv', voice_argv), ('prepare_voice', lambda p: (3, 'AAAA')), ('prepare_video_note', video)]:
            patch = mock.patch.object(self.d.media, name, value)
            patch.start()
            self.addCleanup(patch.stop)
    def prepare_send(self, kind, rid=50):
        before = self.sent_count('sendMessage')
        if kind == 'voice':
            self.assertTrue(self.request(self.conn, rid, 'voice.start', chatId=42)['ok'])
            path = pathlib.Path(self.daemon.recording['path'])
            self.send(self.conn, {'id': rid + 1, 'cmd': 'voice.stop', 'args': {'send': True, 'threadId': 55, 'replyToMessageId': 9}})
        else:
            source = self.d.media.rec_dir() / ('note-retry-' + str(rid) + '.mp4')
            source.write_bytes(b'synthetic recording')
            self.send(self.conn, {'id': rid + 1, 'cmd': 'videonote.send', 'args': {'chatId': 42, 'path': str(source), 'topicId': 2, 'replyToMessageId': 9}})
        query = self.next_query('sendMessage', before)
        content = query['input_message_content']
        field = 'voice_note' if kind == 'voice' else 'video_note'
        path = pathlib.Path(content[field][field]['path'])
        return query, path

    def test_explicit_rejection_keeps_recording_and_retry_reuses_exact_content(self):
        for index, kind in enumerate(('voice', 'video')):
            with self.subTest(kind=kind):
                rid = 50 + index * 20
                query, path = self.prepare_send(kind, rid)
                self.answer(query, {'@type': 'error', 'code': 400, 'message': 'synthetic rejected'})
                rejected = self.read(self.conn, lambda v: v.get('id') == rid + 1)
                self.assertFalse(rejected['ok'])
                record = self.daemon.active_session.recorded_send
                self.assertEqual(record['state'], 'rejected')
                self.assertTrue(path.exists())
                view = self.request(self.conn, rid + 2, 'hello')['result']['recordedSend']
                self.assertEqual(view['token'], record['token'])
                self.assertNotIn('path', view)
                before = self.sent_count('sendMessage')
                self.send(self.conn, {'id': rid + 3, 'cmd': 'recording.retry', 'args': {'token': record['token']}})
                retry = self.next_query('sendMessage', before)
                self.assertEqual({k:v for k,v in retry.items() if k != '@extra'}, {k:v for k,v in query.items() if k != '@extra'})
                self.assertFalse(self.request(self.conn, rid + 4, 'recording.retry', token=record['token'])['ok'])
                self.answer(retry, {'@type': 'message', 'id': 77, 'chat_id': 42, 'content': {'@type':'messageText', 'text': {'text': 'synthetic', 'entities': []}}})
                self.assertTrue(self.read(self.conn, lambda v: v.get('id') == rid + 3)['ok'])
                self.assertIsNone(self.daemon.active_session.recorded_send)
                self.assertTrue(path.exists(), 'TDLib keeps using accepted outgoing media')

    def test_preparation_failure_keeps_raw_file_and_retries_without_new_recording(self):
        for index, kind in enumerate(('voice', 'video')):
            with self.subTest(kind=kind):
                rid = 90 + index * 10
                before = self.sent_count('sendMessage')
                name = 'prepare_voice' if kind == 'voice' else 'prepare_video_note'
                with mock.patch.object(self.d.media, name, side_effect=RuntimeError('synthetic preparation failure')):
                    if kind == 'voice':
                        self.assertTrue(self.request(self.conn, rid, 'voice.start', chatId=42)['ok'])
                        path = pathlib.Path(self.daemon.recording['path'])
                        result = self.request(self.conn, rid + 1, 'voice.stop', send=True)
                    else:
                        path = self.d.media.rec_dir() / 'note-preparation-retry.mp4'
                        path.write_bytes(b'synthetic recording')
                        result = self.request(self.conn, rid + 1, 'videonote.send', chatId=42, path=str(path))
                self.assertFalse(result['ok'])
                record = self.daemon.active_session.recorded_send
                self.assertEqual(record['state'], 'prepareFailed')
                self.assertTrue(path.exists())
                self.assertEqual(self.sent_count('sendMessage'), before)
                if kind == 'video':
                    self.assertFalse(self.request(self.conn, rid + 2, 'videonote.discard', path=str(path))['ok'])
                self.send(self.conn, {'id': rid + 3, 'cmd': 'recording.retry', 'args': {'token': record['token']}})
                query = self.next_query('sendMessage', before)
                self.answer(query, {'@type':'error', 'code':400, 'message':'rejected'})
                self.read(self.conn, lambda v: v.get('id') == rid + 3)
                self.assertEqual(record['state'], 'rejected')
                self.assertTrue(self.request(self.conn, rid + 4, 'recording.discard', token=record['token'])['ok'])
                self.assertFalse(path.exists())

    def test_quick_video_stop_rejection_retains_prepared_file_for_retry(self):
        def video_argv(path, preview):
            pathlib.Path(path).write_bytes(b'synthetic recording')
            pathlib.Path(preview).write_bytes(b'synthetic preview')
            return [daemon_test.sys.executable, '-c', 'import time; time.sleep(30)']
        with mock.patch.object(self.d.media, 'video_capture_argv', video_argv):
            self.assertTrue(self.request(self.conn, 180, 'videonote.record', chatId=42)['ok'])
            raw = pathlib.Path(self.daemon.recording['path'])
            before = self.sent_count('sendMessage')
            self.send(self.conn, {'id':181, 'cmd':'videonote.stop', 'args': {'send':True, 'replyToMessageId':9}})
            query = self.next_query('sendMessage', before)
        self.answer(query, {'@type':'error', 'code':400})
        self.read(self.conn, lambda v: v.get('id') == 181)
        record = self.daemon.active_session.recorded_send
        self.assertEqual(record['state'], 'rejected')
        self.assertFalse(raw.exists())
        self.assertTrue(pathlib.Path(record['path']).exists())
        self.send(self.conn, {'id':182,'cmd':'recording.retry','args':{'token':record['token']}})
        retry = self.next_query('sendMessage', before + 1)
        self.assertEqual(retry['input_message_content'], query['input_message_content'])
        self.answer(retry, {'@type':'error', 'code':400})
        self.read(self.conn, lambda v: v.get('id') == 182)
        self.assertTrue(self.request(self.conn, 183, 'recording.discard', token=record['token'])['ok'])

    def test_account_generation_change_disables_retry_preserves_upload_and_snapshot(self):
        query, path = self.prepare_send('video')
        session = self.daemon.active_session
        session.client_id += 1
        self.answer(query, {'@type':'message','id':79,'chat_id':42,'content':{'@type':'messageUnsupported'}})
        self.read(self.conn, lambda v: v.get('id') == 51)
        record = session.recorded_send
        self.assertEqual(record['state'], 'unknown')
        snapshot = self.request(self.conn, 52, 'hello')['result']['recordedSend']
        self.assertEqual(snapshot['state'], 'unknown')
        self.assertFalse(self.request(self.conn, 53, 'recording.retry', token=record['token'])['ok'])
        self.assertTrue(path.exists())

    def test_main_video_admission_and_allocation_failure_retain_capture(self):
        for index, allocation in enumerate((False, True)):
            source = self.d.media.rec_dir() / ('note-admission-' + str(index) + '.mp4')
            source.write_bytes(b'synthetic recording')
            self.daemon.jobs = 0 if allocation else self.d.JOBS_MAX
            context = mock.patch.object(self.d.media, 'new_sent_path', side_effect=OSError('synthetic disk full')) if allocation else mock.patch.object(self.d.media, 'prepare_video_note')
            with context as operation:
                answer = self.request(self.conn, 150 + index * 10, 'videonote.send', chatId=42, path=str(source))
                self.assertFalse(answer['ok'])
                if not allocation: operation.assert_not_called()
            self.daemon.jobs = 0
            record = self.daemon.active_session.recorded_send
            self.assertEqual(record['state'], 'prepareFailed')
            self.assertTrue(source.exists())
            before = self.sent_count('sendMessage')
            self.send(self.conn, {'id':151 + index * 10, 'cmd':'recording.retry', 'args': {'token':record['token']}})
            query = self.next_query('sendMessage', before)
            self.answer(query, {'@type':'error', 'code':400})
            self.read(self.conn, lambda v: v.get('id') == 151 + index * 10)
            self.assertTrue(self.request(self.conn, 152 + index * 10, 'recording.discard', token=record['token'])['ok'])
            self.assertFalse(source.exists())

    def test_other_account_cleanup_does_not_remove_retained_raw_recording(self):
        source = self.d.media.rec_dir() / 'note-old-retained.mp4'
        source.write_bytes(b'synthetic recording')
        daemon_test.os.utime(source, (1, 1))
        other = self.d.AccountSession('work')
        self.daemon.accounts['work'] = other
        other.recorded_send = {'path': '/synthetic/not-a-file', 'source': source}
        self.daemon.clean_recordings()
        self.assertTrue(source.exists())

    def test_explicit_server_timeout_and_malformed_success_disable_retry(self):
        for index, event in enumerate(({'@type':'error', 'code':408}, {'@type':'error', 'code':500}, {'@type':'ok'},
                                       {'@type':'message', 'id':77, 'chat_id':99, 'content': {'@type':'messageUnsupported'}})):
            query, path = self.prepare_send('video', 120 + index * 10)
            self.answer(query, event)
            self.read(self.conn, lambda v: v.get('id') == 121 + index * 10)
            record = self.daemon.active_session.recorded_send
            self.assertEqual(record['state'], 'unknown')
            self.assertFalse(self.request(self.conn, 125 + index * 10, 'recording.retry', token=record['token'])['ok'])
            self.assertTrue(self.request(self.conn, 126 + index * 10, 'recording.discard', token=record['token'])['ok'])
            self.assertTrue(path.exists())

    def test_discard_confirmed_rejection_removes_only_owned_file(self):
        query, path = self.prepare_send('video')
        self.answer(query, {'@type':'error', 'code':400, 'message':'rejected'})
        self.read(self.conn, lambda v: v.get('id') == 51)
        record = self.daemon.active_session.recorded_send
        self.assertFalse(self.request(self.conn, 52, 'recording.discard', token='wrong')['ok'])
        self.assertTrue(path.exists())
        self.assertTrue(self.request(self.conn, 53, 'recording.discard', token=record['token'])['ok'])
        self.assertFalse(path.exists())
        self.assertIsNone(self.daemon.active_session.recorded_send)

    def test_unknown_transport_result_cannot_retry_or_delete_possible_upload(self):
        query, path = self.prepare_send('video')
        self.daemon.expire_requests(now=10**20)
        self.read(self.conn, lambda v: v.get('id') == 51)
        record = self.daemon.active_session.recorded_send
        self.assertEqual(record['state'], 'unknown')
        before = self.sent_count('sendMessage')
        self.assertFalse(self.request(self.conn, 52, 'recording.retry', token=record['token'])['ok'])
        self.assertEqual(self.sent_count('sendMessage'), before)
        self.assertTrue(self.request(self.conn, 53, 'recording.discard', token=record['token'])['ok'])
        self.assertTrue(path.exists(), 'unknown delivery may still use this file')
        self.answer(query, {'@type':'message', 'id': 88, 'chat_id':42, 'content': {'@type':'messageUnsupported'}})
        self.assertIsNone(self.daemon.active_session.recorded_send)

    def test_wrong_account_and_new_recording_cannot_replace_retained_message(self):
        query, path = self.prepare_send('video')
        self.answer(query, {'@type':'error', 'code':400, 'message':'rejected'})
        self.read(self.conn, lambda v: v.get('id') == 51)
        record = self.daemon.active_session.recorded_send
        other = self.d.AccountSession('work');other.auth = {'state':'ready'};other.client_id = 2
        self.daemon.accounts['work'] = other
        self.assertFalse(self.request(self.conn, 52, 'recording.retry', account='work', token=record['token'])['ok'])
        self.assertFalse(self.request(self.conn, 53, 'voice.start', chatId=42)['ok'])
        self.assertIs(self.daemon.active_session.recorded_send, record)
        self.assertTrue(path.exists())
        switched = self.request(self.conn, 54, 'account.switch', accountId='work')['result']
        self.assertIsNone(switched['recordedSend'])
        restored = self.request(self.conn, 55, 'account.switch', accountId='default')['result']
        self.assertEqual(restored['recordedSend']['token'], record['token'])


if __name__ == '__main__':
    unittest.main()
