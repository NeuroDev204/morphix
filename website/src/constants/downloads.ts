export const MORPHIX_VERSION = "0.1.4";

export const DOWNLOADS = {
  version: MORPHIX_VERSION,
  githubRepo: "https://github.com/NeuroDev204/morphix",
  releasesPage: "https://github.com/NeuroDev204/morphix/releases",
  windows: {
    exe: "https://github.com/NeuroDev204/morphix/releases/download/v0.1.4/MorphixSetup.exe",
    name: "MorphixSetup.exe",
    size: "24.1 MB",
    os: "Windows 10 / 11 (64-bit)",
  },
  linux: {
    deb: "https://github.com/NeuroDev204/morphix/releases/download/v0.1.4/morphix_0.1.4-1_amd64.deb",
    tarball: "https://github.com/NeuroDev204/morphix/releases/download/v0.1.4/morphix-x86_64-linux.tar.gz",
    curlCommand: "curl -fsSL https://github.com/NeuroDev204/morphix/releases/latest/download/install.sh | sh",
    nameDeb: "morphix_0.1.4-1_amd64.deb",
    sizeDeb: "12.4 MB",
    nameTarball: "morphix-x86_64-linux.tar.gz",
    sizeTarball: "11.8 MB",
    os: "Ubuntu 20.04+, Debian, Fedora, Arch (x86_64)",
  },
} as const;
