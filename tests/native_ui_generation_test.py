"""Finite native package generation/refusal contracts; no deploy or account I/O."""
from pathlib import Path
import importlib.util
import tempfile
import hashlib
import json
import shutil
import unittest

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('native_ui_generation',ROOT/'scripts/native_ui_generation.py')
generator=importlib.util.module_from_spec(spec)
spec.loader.exec_module(generator)


class NativeUiGeneration(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='telebar-ui-generation-')
        self.root=Path(self.temp.name)
        self.source=self.root/'native'
        self.source.mkdir()
        manifest={'id':'io.github.pavellizunov.telebar','keepLoaded':True,
                  'entryPoints':{'service':'shell/TelebarService.qml',
                                 'barWidget':'shell/BarWidget.qml','overlay':'shell/TelebarQuickReply.qml'}}
        self.files={'manifest.json':(json.dumps(manifest)+'\n').encode(),
                    'bin/telebar-runtime':b'\x7fELFsynthetic-runtime',
                    'bin/omagram':b'#!/usr/bin/bash -p\n',
                    'app/RingMark.qml':b'import QtQuick\nItem {}\n',
                    'shell/TelebarService.qml':b'Item { property string binDir: String(Qt.resolvedUrl("../bin/")) }\n',
                    'shell/BarWidget.qml':b'import QtQuick\nItem {}\n',
                    'shell/TelebarQuickReply.qml':b'import QtQuick\nItem {}\n',
                    'assets/omagram.svg':b'<svg/>\n'}
        for name,data in self.files.items():
            path=self.source/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
        self.saveManifest()
        (self.source/'README.md').write_text('Synthetic native package\n')
    def tearDown(self): self.temp.cleanup()
    def saveManifest(self):
        metadata={'runtime':'rust','cutoverReady':True,'sourceHashes':{
            name:hashlib.sha256((self.source/name).read_bytes()).hexdigest() for name in self.files}}
        (self.source/'runtime-candidate.json').write_text(json.dumps(metadata)+'\n')
    def test_exact_content_bound_generation_and_service_root(self):
        output=self.root/'out'
        row=generator.prepare(self.source,output)
        self.assertTrue(row['elfUnchanged'])
        self.assertTrue(row['serviceLauncherRootPreserved'])
        generation=row['uiGeneration']
        self.assertEqual((output/'bin/telebar-runtime').read_bytes(),self.files['bin/telebar-runtime'])
        self.assertEqual((output/generation/'app/RingMark.qml').read_bytes(),self.files['app/RingMark.qml'])
        text=(output/generation/'shell/TelebarService.qml').read_text()
        self.assertIn('String(Qt.resolvedUrl("../../bin/"))',text)
        self.assertEqual((output/'shell/TelebarService.qml').read_bytes(),self.files['shell/TelebarService.qml'])
        changed=json.loads((output/'manifest.json').read_text())
        self.assertEqual(changed['entryPoints']['service'],generation+'/shell/TelebarService.qml')
        self.assertTrue(changed['keepLoaded'])
    def test_same_source_identity_and_changed_ui_identity(self):
        first=generator.prepare(self.source,self.root/'one')
        second=generator.prepare(self.source,self.root/'two')
        self.assertEqual(first['uiGeneration'],second['uiGeneration'])
        (self.source/'app/RingMark.qml').write_text('import QtQuick\nItem { width: 16 }\n')
        self.saveManifest()
        changed=generator.prepare(self.source,self.root/'three')
        self.assertNotEqual(first['uiGeneration'],changed['uiGeneration'])
    def test_refuses_corruption_unmanifested_python_and_links_before_output(self):
        for kind in ('corrupt','extra','python','link'):
            source=self.root/kind;shutil.copytree(self.source,source)
            if kind=='corrupt':(source/'app/RingMark.qml').write_text('corrupt')
            if kind=='extra':(source/'unreviewed.txt').write_text('extra')
            if kind=='python':(source/'application.py').write_text('raise RuntimeError')
            if kind=='link':(source/'assets/escape.svg').symlink_to('/etc/passwd')
            output=self.root/(kind+'-out')
            with self.assertRaises(ValueError):generator.prepare(source,output)
            self.assertFalse(output.exists())


if __name__=='__main__':unittest.main()
