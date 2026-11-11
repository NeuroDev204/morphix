from __future__ import annotations

import unittest
from pathlib import Path

import yaml


WORKFLOW: Path = Path(__file__).resolve().parents[2] / ".github/workflows/release.yml"


class ReleaseWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.jobs: dict = yaml.safe_load(WORKFLOW.read_text())["jobs"]

    def test_uploads_fail_when_nothing_was_built(self) -> None:
        # WHY: thiếu file mà upload chỉ cảnh báo thì Release vẫn phát hành install.sh trỏ tới tarball 404.
        for name in ("tarball", "deb"):
            uploads = [s for s in self.jobs[name]["steps"] if str(s.get("uses", "")).startswith("actions/upload-artifact")]
            self.assertTrue(uploads, name)
            for step in uploads:
                self.assertEqual(step["with"].get("if-no-files-found"), "error", name)

    def test_container_job_avoids_runner_temp_expression(self) -> None:
        # WHY: trong job container, ${{ runner.temp }} là đường dẫn của máy chủ, khác $RUNNER_TEMP bên trong container.
        text = yaml.safe_dump(self.jobs["tarball"])
        self.assertNotIn("runner.temp", text)
        self.assertNotIn("RUNNER_TEMP", text)

    # ---- Windows: exe tải về và gói msix cho Store ----

    def steps_text(self, job: dict) -> str:
        return "\n".join(str(step.get("run", "")) + " " + str(step.get("uses", "")) for step in job["steps"])

    def test_windows_job_builds_the_setup_exe_and_the_store_msix(self) -> None:
        job = self.jobs["windows"]
        self.assertEqual(job["runs-on"], "windows-latest")
        self.assertIn("test", job["needs"] if isinstance(job["needs"], list) else [job["needs"]])
        text = self.steps_text(job)
        self.assertIn("setup-windows-deps.ps1", text)
        self.assertIn("package-windows.ps1", text)
        self.assertIn("custom-protocol", text)
        names = [step["with"]["name"] for step in job["steps"] if str(step.get("uses", "")).startswith("actions/upload-artifact")]
        self.assertEqual(sorted(names), ["store-msix", "windows-setup"])
        for step in job["steps"]:
            if str(step.get("uses", "")).startswith("actions/upload-artifact"):
                self.assertEqual(step["with"].get("if-no-files-found"), "error")

    def test_windows_job_refuses_a_tag_that_does_not_match_the_workspace_version(self) -> None:
        self.assertIn("check_version.py", self.steps_text(self.jobs["windows"]))
        self.assertIn("GITHUB_REF_NAME", self.steps_text(self.jobs["windows"]))

    def test_release_attaches_the_exe_but_never_the_store_msix(self) -> None:
        publish = self.jobs["publish"]
        self.assertIn("windows", publish["needs"])
        downloaded = [step["with"]["name"] for step in publish["steps"] if str(step.get("uses", "")).startswith("actions/download-artifact")]
        self.assertEqual(sorted(downloaded), ["deb", "tarball", "windows-setup"])
        text = self.steps_text(publish)
        self.assertNotIn("msix", text.lower())
        self.assertIn("MorphixSetup.exe", text)


class CiWorkflowTests(unittest.TestCase):
    def test_ci_runs_clippy_and_tests_on_windows(self) -> None:
        jobs = yaml.safe_load((WORKFLOW.parent / "ci.yml").read_text())["jobs"]
        job = jobs["test-windows"]
        self.assertEqual(job["runs-on"], "windows-latest")
        text = "\n".join(str(step.get("run", "")) for step in job["steps"])
        self.assertIn("cargo clippy --workspace --all-targets -- -D warnings", text)
        self.assertIn("cargo test --workspace", text)
        self.assertIn("setup-windows-deps.ps1", text)


class SupplyChainTests(unittest.TestCase):
    FILES = ("ci.yml", "release.yml")

    def load(self, name: str) -> dict:
        return yaml.safe_load((WORKFLOW.parent / name).read_text())

    def test_every_third_party_action_is_pinned_to_a_commit_sha(self) -> None:
        # WHY SHA: tag như `v4` là con trỏ di động; chủ repo (hoặc kẻ chiếm tài khoản) đổi được mã chạy trong CI.
        for name in self.FILES:
            for job_name, job in self.load(name)["jobs"].items():
                for step in job.get("steps", []):
                    uses = str(step.get("uses", ""))
                    if uses and not uses.startswith("./"):
                        self.assertRegex(uses, r"@[0-9a-f]{40}(\s|$)", f"{name}:{job_name}: {uses}")

    def test_workflows_default_to_a_read_only_token(self) -> None:
        for name in self.FILES:
            self.assertEqual(self.load(name).get("permissions"), {"contents": "read"}, name)
        self.assertEqual(self.load("release.yml")["jobs"]["publish"]["permissions"], {"contents": "write"})

    def test_pinned_rust_toolchain_still_selects_stable(self) -> None:
        # dtolnay/rust-toolchain chọn toolchain theo tên ref; ghim SHA thì phải truyền `toolchain` tường minh.
        for name in self.FILES:
            for job in self.load(name)["jobs"].values():
                for step in job.get("steps", []):
                    if str(step.get("uses", "")).startswith("dtolnay/rust-toolchain@"):
                        self.assertEqual(step["with"]["toolchain"], "stable")

    def test_windows_tests_can_find_the_downloaded_ffmpeg(self) -> None:
        steps = self.load("ci.yml")["jobs"]["test-windows"]["steps"]
        text = "\n".join(str(step.get("run", "")) for step in steps)
        self.assertIn("GITHUB_PATH", text)
        self.assertIn(r"installer\bundle", text)

    def test_windows_release_checks_that_the_ui_is_embedded(self) -> None:
        steps = self.load("release.yml")["jobs"]["windows"]["steps"]
        text = "\n".join(str(step.get("run", "")) for step in steps)
        self.assertIn("check-gui-embeds-ui.ps1", text)
        script = (WORKFLOW.parents[2] / "scripts" / "ci" / "check-gui-embeds-ui.ps1").read_text(encoding="utf-8-sig")
        for needle in ("assets/khay-", "khay.html", "localhost:5173"):
            self.assertIn(needle, script)

    def test_inno_setup_fallback_install_is_pinned(self) -> None:
        script = (WORKFLOW.parents[2] / "scripts" / "ci" / "package-windows.ps1").read_text(encoding="utf-8-sig")
        self.assertRegex(script, r"choco install innosetup --version \d+\.\d+\.\d+")


if __name__ == "__main__":
    unittest.main()
