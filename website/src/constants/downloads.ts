export const MORPHIX_VERSION = "0.1.6";

export const DOWNLOADS = {
  version: MORPHIX_VERSION,
  githubRepo: "https://github.com/NeuroDev204/morphix",
  releasesPage: "https://github.com/NeuroDev204/morphix/releases",
  windows: {
    exe: "https://github.com/NeuroDev204/morphix/releases/download/v0.1.6/MorphixSetup.exe",
    name: "MorphixSetup.exe",
    size: "24.1 MB",
    os: "Windows 10 / 11 (64-bit)",
  },
  linux: {
    curlCommand: "curl -fsSL https://github.com/NeuroDev204/morphix/releases/latest/download/install.sh | sh",
    os: "Ubuntu 22.04+, Debian 12+, Fedora, Arch, openSUSE (x86_64)",
  },
} as const;
