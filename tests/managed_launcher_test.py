#!/usr/bin/python3
"""Managed launch regressions; fixed fake runner, never a live user manager."""
import importlib.machinery
import importlib.util
import pathlib
import sys
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parent.parent
loader = importlib.machinery.SourceFileLoader("managed_launcher", str(ROOT / "bin/omagram"))
spec = importlib.util.spec_from_loader(loader.name, loader)
launcher = importlib.util.module_from_spec(spec)
loader.exec_module(launcher)

class ManagedLauncher(unittest.TestCase):
    def test_backend_uses_manager_not_namespace_inheriting_fork(self):
        result = mock.Mock(ok=True)
        with mock.patch.object(launcher.safe, "run", return_value=result) as run:
            launcher.managed_start("telebar-backend", ROOT / "bin/omagramd")
        argv = run.call_args.args[0]
        self.assertEqual(argv[:3], ["/usr/bin/systemd-run", "--user", "--quiet"])
        self.assertIn("--unit=telebar-backend", argv)
        self.assertIn("--property=KillMode=control-group", argv)
        self.assertIn("LD_PRELOAD", argv)
        self.assertEqual(argv[-3:], ["/usr/bin/python3", "-I", str(ROOT / "bin/omagramd")])
        self.assertEqual(run.call_args.kwargs["timeout"], 5)
        self.assertNotIn("env", run.call_args.kwargs)

    def test_namespaced_entry_delegates_before_importing_or_opening_accounts(self):
        import runpy
        with mock.patch("os.stat", return_value=mock.Mock(st_uid=65534)), mock.patch("os.execv", side_effect=SystemExit(0)) as execute:
            with self.assertRaises(SystemExit):
                runpy.run_path(str(ROOT / "bin/omagramd"), run_name="__main__")
        self.assertEqual(execute.call_args.args[0], "/usr/bin/python3")
        self.assertEqual(execute.call_args.args[1][-1], "--service")
        self.assertNotIn("omagram_backend", sys.modules)

    def test_running_owner_is_not_restarted(self):
        with mock.patch.object(launcher, "service_running", return_value=True), mock.patch.object(launcher, "managed_start") as start:
            self.assertTrue(launcher.start_service())
            start.assert_not_called()

    def test_window_is_host_managed_without_opening_from_the_shell(self):
        with mock.patch.object(launcher, "managed_start", return_value=mock.Mock(ok=True)) as start, mock.patch.object(launcher, "start_service") as service, mock.patch.object(launcher, "prepare_root") as prepare:
            self.assertEqual(launcher.main(["omagram", "--chat", "42"]), 0)
            self.assertEqual(start.call_args.args[2], ["--host-window", "--chat", "42"])
            service.assert_not_called()
            prepare.assert_not_called()

if __name__ == "__main__":
    unittest.main()
