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


if __name__ == "__main__":
    unittest.main()
