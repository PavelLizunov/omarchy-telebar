#!/usr/bin/python3
"""Build a new immutable UI generation from a checked native package; no deploy.

keepLoaded service references can pin old same-URL QML in the shared engine.
Give all QML owners a content-bound URL while preserving the native service owner,
normal app/window paths, executable, accounts and host recovery mechanism.
This is a development packaging tool, excluded from the installed application.
"""
from pathlib import Path
import hashlib
import json
import sys

MAX_FILES = 256
MAX_BYTES = 16 * 1024 * 1024
MAX_PACKAGE_BYTES = 96 * 1024 * 1024


def sha(data):
    return hashlib.sha256(data).hexdigest()


def prepare(source, output):
    if not source.is_absolute() or not output.is_absolute() or source.is_symlink():
        raise ValueError('Explicit ordinary package and new absolute output required')
    if output.exists() or output.is_symlink() or output.is_relative_to(source):
        raise ValueError('Output must be new and outside input')
    files = {}
    for path in sorted(source.rglob('*')):
        if path.is_symlink() or not (path.is_dir() or path.is_file()):
            raise ValueError('Native package contains an unsafe entry')
        if path.is_file():
            if len(files) >= MAX_FILES or path.stat().st_size > 64 * 1024 * 1024:
                raise ValueError('Native package input budget exceeded')
            relative = path.relative_to(source).as_posix()
            if path.suffix in ('.py', '.pyc', '.pyo'):
                raise ValueError('Application Python is forbidden')
            files[relative] = path.read_bytes()
            if sum(map(len,files.values())) > MAX_PACKAGE_BYTES:
                raise ValueError('Native package byte budget exceeded')
    metadata = json.loads(files['runtime-candidate.json'])
    if metadata['runtime'] != 'rust' or metadata['cutoverReady'] is not True:
        raise ValueError('Checked launch-enabled native package required')
    if set(files) != set(metadata['sourceHashes']) | {'runtime-candidate.json','README.md'}:
        raise ValueError('Native manifest does not cover the exact package inventory')
    if not files['bin/telebar-runtime'].startswith(b'\x7fELF'):
        raise ValueError('Native ELF runtime required')
    for name, digest in metadata['sourceHashes'].items():
        if sha(files[name]) != digest:
            raise ValueError('Native input hash differs: '+name)
    manifest = json.loads(files['manifest.json'])
    if manifest['id'] != 'io.github.pavellizunov.telebar' or manifest.get('keepLoaded') is not True:
        raise ValueError('Native service identity/retention contract changed')
    if manifest['entryPoints'] != {'service':'shell/TelebarService.qml',
                                  'barWidget':'shell/BarWidget.qml',
                                  'overlay':'shell/TelebarQuickReply.qml'}:
        raise ValueError('Unexpected native source entry points')
    visual = {name:data for name,data in files.items() if name.startswith(('app/','shell/','assets/'))}
    if sum(map(len,visual.values())) > MAX_BYTES:
        raise ValueError('UI generation byte budget exceeded')
    identity = sha(b''.join(name.encode()+b'\0'+bytes.fromhex(sha(data))
                            for name,data in sorted(visual.items())))
    generation = 'ui-'+identity[:16]
    manifest['entryPoints']['service'] = generation+'/shell/TelebarService.qml'
    manifest['entryPoints']['barWidget'] = generation+'/shell/BarWidget.qml'
    manifest['entryPoints']['overlay'] = generation+'/shell/TelebarQuickReply.qml'
    files['manifest.json'] = (json.dumps(manifest,indent=2)+'\n').encode()
    generated = {generation+'/'+name:data for name,data in visual.items()}
    servicePath = generation+'/shell/TelebarService.qml'
    service = generated[servicePath].decode()
    old = 'String(Qt.resolvedUrl("../bin/"))'
    if service.count(old) != 1:
        raise ValueError('Service launcher root contract changed')
    generated[servicePath] = service.replace(old,'String(Qt.resolvedUrl("../../bin/"))').encode()
    files.update(generated)
    metadata['sourceHashes'] = {name:sha(data) for name,data in files.items()
                               if name not in ('runtime-candidate.json','README.md')}
    metadata['uiGeneration'] = generation
    metadata['uiGenerationSha256'] = identity
    metadata['sourceHashes'] = dict(sorted(metadata['sourceHashes'].items()))
    files['runtime-candidate.json'] = (json.dumps(metadata,indent=2)+'\n').encode()
    # All source bytes/manifests are admitted before any output is created.
    output.mkdir(mode=0o700)
    for name,data in files.items():
        path = output/name
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(data)
        path.chmod((source/name).stat().st_mode & 0o777 if (source/name).is_file() else 0o644)
    return {'uiGeneration':generation,'uiGenerationSha256':identity,'files':len(files),
            'visualFiles':len(visual),'uiBytes':sum(map(len,visual.values())),
            'elfUnchanged':files['bin/telebar-runtime']==(source/'bin/telebar-runtime').read_bytes(),
            'serviceEntryGeneration':manifest['entryPoints']['service'],
            'serviceLauncherRootPreserved':True,
            'keepLoadedRequiresExplicitRecreateForOldInstance':True,
            'normalWindowPathsUnchanged':True,'applicationPython':False,'installsOrReloads':False}


if __name__ == '__main__':
    if len(sys.argv) != 3:
        raise SystemExit('Usage: native_ui_generation.py <checked-native-package> <new-native-output>')
    print(json.dumps(prepare(Path(sys.argv[1]),Path(sys.argv[2])),indent=2))
