"""Native mark vector and bar ownership checks; no application or account I/O."""
from pathlib import Path
import re
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]


class IconContract(unittest.TestCase):
    def test_qml_and_desktop_share_one_bounded_stepped_contour(self):
        mark = (ROOT/'app/RingMark.qml').read_text()
        svg = ET.fromstring((ROOT/'assets/omagram.svg').read_bytes())
        self.assertEqual(svg.attrib['viewBox'],'0 0 16 16')
        paths = svg.findall('{http://www.w3.org/2000/svg}path')
        self.assertEqual(len(paths),1)
        vector = paths[0].attrib['d']
        self.assertEqual(re.search(r'path: "([^"]+)"',mark).group(1),vector)
        self.assertRegex(vector,r'^[MHVZ0-9 ]+$')
        self.assertTrue(all(0 <= int(value) <= 16 for value in re.findall(r'\d+',vector)))
        self.assertEqual(paths[0].attrib['fill-rule'],'evenodd')
        self.assertIn('ShapePath.OddEvenFill',mark)
        self.assertIn('scale: mark.size / 16',mark)
        self.assertNotIn('Rectangle',mark)
        self.assertNotIn('Animation',mark)
        self.assertEqual(svg.findall('{http://www.w3.org/2000/svg}rect'),[])

    def test_optical_canvas_does_not_replace_native_hit_and_status_owner(self):
        bar = (ROOT/'shell/BarWidget.qml').read_text()
        self.assertIn('size: button.opticalSize',bar)
        self.assertIn('BarIconButton {',bar)
        self.assertIn('root.ready && root.unread > 0',bar)
        self.assertIn('color: Commons.Color.accent',bar)
        self.assertIn('root.togglePanel()',bar)
        self.assertIn('if (b === Qt.RightButton)',bar)
        self.assertNotIn('MouseArea',bar)
        self.assertNotIn('ShaderEffect',bar)

    def test_desktop_ink_has_nontext_contrast_on_reviewed_light_and_dark(self):
        svg = ET.fromstring((ROOT/'assets/omagram.svg').read_bytes())
        ink = svg.find('{http://www.w3.org/2000/svg}path').attrib['fill']
        def luminance(value):
            rgb = [int(value[i:i+2],16)/255 for i in (1,3,5)]
            rgb = [v/12.92 if v <= .04045 else ((v+.055)/1.055)**2.4 for v in rgb]
            return sum(a*b for a,b in zip(rgb,[.2126,.7152,.0722]))
        for background in ['#101315','#edf0f2']:
            low,high = sorted([luminance(ink),luminance(background)])
            self.assertGreaterEqual((high+.05)/(low+.05),3,background)


if __name__ == '__main__':
    unittest.main()
