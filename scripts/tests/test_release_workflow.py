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


if __name__ == "__main__":
    unittest.main()
