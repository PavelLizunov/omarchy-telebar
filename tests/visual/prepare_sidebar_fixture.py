#!/usr/bin/python3
"""Prepare complete Main with explicit inert I/O and native unshown surfaces."""
from pathlib import Path
import importlib.util
import hashlib
import json
import shutil
import sys
import tempfile

SOURCE = Path(__file__).resolve().parents[2]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def prepare(output):
    if not output.is_absolute() or output.exists() or output.is_symlink():
        raise ValueError('Fixture output must be a new absolute directory')
    preview = module('preview_io', SOURCE/'rust/tests/preview_io.py')
    icons = module('icon_fixture', SOURCE/'tests/visual/prepare_icon_fixture.py')
    # Reuse the existing explicit Process/FileView overlay and native Commons
    # admission. Neither helper starts a process, socket or compositor.
    with tempfile.TemporaryDirectory(prefix='telebar-sidebar-imports-') as directory:
        imports = Path(directory)/'icon'
        icons.prepare(imports)
        inputs = preview.assemble(SOURCE, output)
        shutil.copytree(imports/'imports/qs', output/'imports/qs')
        shutil.copyfile(imports/'fixture-inputs.json', output/'native-import-inputs.json')
    target = output/'tests/visual'
    target.mkdir(parents=True)
    hashes = {}
    for name in ('MainSidebarConsumer.qml', 'FixtureApp.qml', 'sample-photo.svg'):
        data = icons.read(SOURCE/'tests/visual'/name)
        (target/name).write_bytes(data)
        hashes[name] = hashlib.sha256(data).hexdigest()
    # Main's socket/autoStart overrides are applied before it is constructed.
    # TelebarClient's separate default is additionally blanked by preview_io.
    consumer = (target/'MainSidebarConsumer.qml').read_text()
    if 'autoStart: false' not in consumer or 'socketPath: ""' not in consumer:
        raise ValueError('Actual Main consumer I/O contract changed')
    (output/'sidebar-inputs.json').write_text(json.dumps({
        'sourceHashes': inputs, 'fixtureHashes': hashes,
        'nativeWindowsUnshown': True, 'autoStart': False, 'socketPath': '',
        'actualContentReparented': True,
        'limits': ['No compositor focus/placement, production drafts or Telegram acceptance']
    }, indent=2)+'\n')


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('Usage: prepare_sidebar_fixture.py <new-absolute-fixture>')
    prepare(Path(sys.argv[1]))
