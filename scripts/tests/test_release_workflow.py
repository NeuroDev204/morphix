from __future__ import annotations

import unittest
from pathlib import Path

import yaml


ROOT: Path = Path(__file__).resolve().parents[2]
WORKFLOW: Path = ROOT / ".github/workflows/release.yml"


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


class DebChecksumTests(unittest.TestCase):
    def test_release_publishes_the_deb_checksum(self) -> None:
        release = (ROOT / ".github" / "workflows" / "release.yml").read_text(encoding="utf-8")
        self.assertIn("${{ runner.temp }}/dist/*.deb.sha256", release)
        self.assertIn("test -f dist/morphix_*_amd64.deb.sha256", release)

    def test_deb_checksum_names_the_file_relatively(self) -> None:
        # WHY: `sha256sum -c` trên máy người dùng chỉ chạy được khi file ghi tên tương đối, không có đường dẫn máy build.
        script = (ROOT / "scripts" / "build-deb.sh").read_text(encoding="utf-8")
        self.assertNotIn('sha256sum "$DEB" > "$DEB.sha256"', script)
        self.assertIn('(cd "$OUT" && sha256sum "$(basename "$DEB")" > "$(basename "$DEB").sha256")', script)


class WindowsSmokeTests(unittest.TestCase):
    def test_release_publishes_only_after_the_installer_passes_on_real_windows(self) -> None:
        jobs = yaml.safe_load(WORKFLOW.read_text())["jobs"]
        smoke = jobs["smoke-windows"]
        self.assertEqual(smoke["needs"], "windows")
        self.assertEqual(smoke["runs-on"], "windows-latest")
        runs = " ".join(str(step.get("run", "")) for step in smoke["steps"])
        self.assertIn("scripts/ci/smoke-windows-installer.ps1 -Setup dist/MorphixSetup.exe", runs)
        self.assertIn("smoke-windows", jobs["publish"]["needs"])

    def test_smoke_evidence_is_kept_even_when_it_fails(self) -> None:
        for path in (WORKFLOW, ROOT / ".github/workflows/smoke-windows.yml"):
            jobs = yaml.safe_load(path.read_text())["jobs"]
            uploads = [step for step in jobs["smoke-windows"]["steps"] if "upload-artifact" in str(step.get("uses", ""))]
            self.assertTrue(uploads, path.name)
            self.assertEqual(uploads[0].get("if"), "always()", path.name)

    def test_manual_smoke_can_check_any_published_release(self) -> None:
        workflow = yaml.safe_load((ROOT / ".github/workflows/smoke-windows.yml").read_text())
        triggers = workflow[True]
        self.assertIn("tag", triggers["workflow_dispatch"]["inputs"])
        self.assertIn("scripts/ci/smoke-windows-installer.ps1", triggers["pull_request"]["paths"])
        self.assertIn("installer/**", triggers["pull_request"]["paths"])

    def test_pull_requests_smoke_the_installer_built_from_their_own_source(self) -> None:
        workflow = yaml.safe_load((ROOT / ".github/workflows/smoke-windows.yml").read_text())
        steps = workflow["jobs"]["smoke-windows"]["steps"]
        build = [step for step in steps if "package-windows.ps1" in str(step.get("run", ""))]
        self.assertTrue(build)
        self.assertEqual(build[0].get("if"), "inputs.tag == ''")
        cargo = " ".join(str(step.get("run", "")) for step in steps)
        self.assertIn("--features morphix-gui/custom-protocol", cargo)


class CiWorkflowTests(unittest.TestCase):
    def test_ci_runs_clippy_and_tests_on_windows(self) -> None:
        jobs = yaml.safe_load((WORKFLOW.parent / "ci.yml").read_text())["jobs"]
        job = jobs["test-windows"]
        self.assertEqual(job["runs-on"], "windows-latest")
        text = "\n".join(str(step.get("run", "")) for step in job["steps"])
        self.assertIn("cargo clippy --workspace --all-targets -- -D warnings", text)
        self.assertIn("cargo test --workspace", text)
        self.assertIn("setup-windows-deps.ps1", text)


class ScriptModeTests(unittest.TestCase):
    def test_every_script_the_workflows_run_directly_is_executable_in_git(self) -> None:
        # WHY: `run: scripts/x.sh` cần bit thực thi; thiếu thì job báo "Permission denied" (exit 126) chỉ khi đã gắn tag.
        import re
        import subprocess
        root = WORKFLOW.parents[2]
        modes = dict(
            line.split()[::3][:2][::-1]
            for line in subprocess.run(["git", "ls-files", "-s"], cwd=root, capture_output=True, text=True, check=True).stdout.splitlines()
        )
        for name in ("ci.yml", "release.yml"):
            for command in re.findall(r"run:\s+(scripts/[\w./-]+\.(?:sh|py))", (WORKFLOW.parent / name).read_text()):
                self.assertEqual(modes.get(command), "100755", f"{name}: {command} không có bit thực thi trong git")


class SupplyChainTests(unittest.TestCase):
    FILES = ("ci.yml", "release.yml", "ffmpeg.yml")

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


class FfmpegWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.doc = yaml.safe_load((WORKFLOW.parent / "ffmpeg.yml").read_text())

    def test_is_manual_and_does_not_publish_unless_asked(self) -> None:
        # PyYAML đọc khoá `on` thành True.
        trigger = self.doc.get("on", self.doc.get(True))
        self.assertEqual(list(trigger), ["workflow_dispatch"])
        inputs = trigger["workflow_dispatch"]["inputs"]
        self.assertEqual(inputs["publish"]["default"], False)
        self.assertEqual(self.doc["jobs"]["publish"]["if"], "${{ inputs.publish }}")

    def test_builds_both_platforms_with_the_repo_scripts(self) -> None:
        text = yaml.safe_dump(self.doc)
        self.assertIn("scripts/build-ffmpeg-slim.sh", text)
        self.assertIn("scripts/build-ffmpeg-slim-windows.sh", text)
        self.assertIn("ubuntu:22.04", text)
        self.assertIn("debian:13", text)

    def test_only_the_publish_job_can_write(self) -> None:
        for name, job in self.doc["jobs"].items():
            expected = {"contents": "write"} if name == "publish" else None
            self.assertEqual(job.get("permissions"), expected, name)


if __name__ == "__main__":
    unittest.main()
