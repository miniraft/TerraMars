#!/usr/bin/env python3
"""Produce one executable for Linux/Windows, or a macOS DMG containing the app."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import platform
import plistlib
import shutil
import stat
import subprocess
import tempfile


def macos_icon(source: Path, destination: Path) -> None:
    """Create the native icon with macOS's built-in image tools."""
    with tempfile.TemporaryDirectory(prefix="terramars-icon-") as directory:
        iconset = Path(directory) / "TerraMars.iconset"
        iconset.mkdir()
        for size in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                pixels = str(size * scale)
                suffix = "@2x" if scale == 2 else ""
                subprocess.run(
                    ["sips", "-z", pixels, pixels, os.fspath(source), "--out",
                     os.fspath(iconset / f"icon_{size}x{size}{suffix}.png")],
                    check=True, stdout=subprocess.DEVNULL,
                )
        subprocess.run(
            ["iconutil", "-c", "icns", os.fspath(iconset), "-o", os.fspath(destination)],
            check=True,
        )


def package(target: str, binary_dir: Path, output: Path) -> Path:
    root = Path(__file__).resolve().parents[1]
    binary = binary_dir / ("terramars.exe" if target == "windows" else "terramars")
    if not binary.is_file():
        raise SystemExit(f"Missing {binary}; run cargo build --locked --release first.")
    output.mkdir(parents=True, exist_ok=True)
    arch = platform.machine().lower()
    arch = {"amd64": "x86_64", "aarch64": "arm64"}.get(arch, arch)
    suffix = {"linux": "", "windows": ".exe", "macos": ".dmg"}[target]
    artifact = output / f"TerraMars-{target}-{arch}{suffix}"

    # Application assets, shaders and license notices are embedded in the binary.
    if target != "macos":
        shutil.copy2(binary, artifact)
        if target == "linux":
            artifact.chmod(artifact.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        return artifact
    if platform.system() != "Darwin":
        raise SystemExit("macOS DMG packaging requires a macOS host with hdiutil.")

    with tempfile.TemporaryDirectory(prefix="terramars-package-") as directory:
        staging = Path(directory)
        bundle = staging / "TerraMars.app"
        contents = bundle / "Contents"
        executable_dir = contents / "MacOS"
        resources = contents / "Resources"
        executable_dir.mkdir(parents=True)
        resources.mkdir(parents=True)
        macos_icon(root / "assets/icons/app_logo/logo.png", resources / "TerraMars.icns")
        metadata = {
            "CFBundleName": "TerraMars",
            "CFBundleDisplayName": "TerraMars",
            "CFBundleExecutable": "terramars",
            "CFBundleIdentifier": "org.terramars.app",
            "CFBundlePackageType": "APPL",
            "CFBundleIconFile": "TerraMars.icns",
            "CFBundleShortVersionString": "0.2.0",
            "CFBundleVersion": "1",
            "NSHighResolutionCapable": True,
            "LSMinimumSystemVersion": "11.0",
        }
        with (contents / "Info.plist").open("wb") as stream:
            plistlib.dump(metadata, stream)
        (contents / "PkgInfo").write_bytes(b"APPL????")
        destination = executable_dir / binary.name
        shutil.copy2(binary, destination)
        destination.chmod(destination.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        for name in ("README.md", "docs", "tools/voice_bridge.py", "tools/voice-requirements.txt", "vendor/GPUI_PATCHES.md", "vendor/rotation.patch"):
            source = root / name
            dest = resources / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            if source.is_dir():
                shutil.copytree(source, dest)
            else:
                shutil.copy2(source, dest)
        licenses = resources / "font-licenses"
        licenses.mkdir()
        for license_file in (root / "assets/fonts").glob("*.txt"):
            shutil.copy2(license_file, licenses / license_file.name)
        for license_file in (root / "assets/licenses").glob("*.txt"):
            shutil.copy2(license_file, licenses / license_file.name)
        for name in ("LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING"):
            application_license = root / name
            if application_license.is_file():
                shutil.copy2(application_license, resources / name)
        # Keep the small provenance snapshots readable beside the documentation.
        shutil.copytree(root / "assets/data", resources / "assets/data")
        map_manifests = resources / "assets/maps"
        map_manifests.mkdir(parents=True)
        for manifest in (root / "assets/maps").glob("*.json"):
            shutil.copy2(manifest, map_manifests / manifest.name)

        # GPUI's compiled shaders and application assets are embedded in the binary.
        # Ad-hoc signing makes the local bundle structurally valid on Apple Silicon;
        # this does not replace Developer ID signing or notarization.
        subprocess.run(["codesign", "--force", "--deep", "--sign", "-", os.fspath(bundle)], check=True)
        (staging / "Applications").symlink_to("/Applications", target_is_directory=True)
        subprocess.run(
            ["hdiutil", "create", "-volname", "TerraMars", "-srcfolder",
             os.fspath(staging), "-format", "UDZO", "-ov", os.fspath(artifact)],
            check=True,
        )
    return artifact


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=("linux", "macos", "windows"), required=True)
    parser.add_argument("--binary-dir", type=Path, default=Path("target/release"))
    parser.add_argument("--output", type=Path, default=Path("dist"))
    args = parser.parse_args()
    print(package(args.platform, args.binary_dir.resolve(), args.output.resolve()))
