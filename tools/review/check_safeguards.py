"""Real HTTP smoke test. Uses an isolated synthetic root; never touches repo audit data.
Run after cargo build: python3 tools/review/check_safeguards.py
"""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

repo = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix='casefile-http-') as tmp:
    root = Path(tmp)
    shutil.copytree(repo / 'data/seed', root / 'data/seed')
    for folder in ['ui', 'agents']:
        (root / folder).symlink_to(repo / folder, target_is_directory=True)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = dict(os.environ, CASEFILE_ROOT=str(root), CASEFILE_PUBLIC='0')
    proc = subprocess.Popen([str(repo / 'target/debug/casefile'), 'serve', '--port', str(port)], env=env,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    def request(path, payload=None):
        req = urllib.request.Request(f'http://127.0.0.1:{port}/api/{path}',
              data=None if payload is None else json.dumps(payload).encode(),
              headers={'Content-Type': 'application/json'})
        try:
            with urllib.request.urlopen(req, timeout=5) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            return error.code, json.load(error)
    try:
        for _ in range(100):
            try:
                _, queue = request('queue')
                break
            except (OSError, urllib.error.URLError):
                if proc.poll() is not None:
                    raise RuntimeError('server exited')
                time.sleep(.05)
        else:
            raise RuntimeError('server startup timeout')
        case = queue['cases'][0]['id']
        step = request('case/' + case)[1]['next_steps'][0]
        real_detail = {'step': step['step'], 'source': step['source']}
        def decision(action, subject, detail):
            return {'actor': 'det.demo', 'action': action, 'subject': subject, 'detail': detail}
        for payload in [decision('approve_series_alert', 'H-DECOY', {'verdict': 'pass'}),
                        decision('accept_step', 'UNKNOWN', {}),
                        decision('invented', case, {}),
                        decision('reject_step', case, {}),
                        decision('reject_step', case, {'reason': 'Other', 'note': ' '})]:
            assert request('decision', payload)[0] == 400
            assert request('audit')[1]['chain']['entries'] == 0
        assert request('decision', decision('reject_step', case, dict(real_detail, reason='Not lawful')))[0] == 200
        assert request('audit')[1]['entries'][0]['detail']['recommendation'] == step
        assert request('audit')[1]['chain'] == {'intact': True, 'entries': 1}
        audit = root / 'data/audit.jsonl'
        audit.write_text(audit.read_text() + 'BROKEN\n')
        before = audit.read_bytes()
        assert request('audit')[1]['chain']['intact'] is False
        assert request('decision', decision('accept_step', case, real_detail))[0] == 400
        assert audit.read_bytes() == before
        print('PASS: HTTP bypasses rejected; valid decision recorded; corrupt audit blocks writes unchanged.')
    finally:
        proc.terminate()
        proc.wait(timeout=10)
