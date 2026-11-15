#!/usr/bin/env python3
"""Package a compiled TerraMars binary using the Python standard library."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import platform
import plistlib
import shutil
import stat
import subprocess
import tarfile
import tempfile
import zipfile


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
    suffix = ".zip" if target == "windows" else ".tar.gz"
    archive = output / f"TerraMars-{target}-{arch}{suffix}"

    with tempfile.TemporaryDirectory(prefix="terramars-package-") as directory:
        staging = Path(directory)
        bundle = staging / ("TerraMars.app" if target == "macos" else "TerraMars")
        if target == "macos":
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
        else:
            bundle.mkdir(parents=True)
            executable_dir = resources = bundle
            if target == "linux":
                shutil.copy2(root / "assets/icons/app_logo/logo_small.png", resources / "org.terramars.app.png")

        destination = executable_dir / binary.name
        shutil.copy2(binary, destination)
        if target != "windows":
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
        if target == "macos" and platform.system() == "Darwin":
            subprocess.run(["codesign", "--force", "--deep", "--sign", "-", os.fspath(bundle)], check=True)
        if target == "windows":
            # crates.io source files can carry Unix-epoch mtimes; ZIP starts at 1980.
            with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, strict_timestamps=False) as stream:
                for path in sorted(bundle.rglob("*")):
                    if path.is_file():
                        stream.write(path, path.relative_to(staging))
        else:
            with tarfile.open(archive, "w:gz") as stream:
                stream.add(bundle, arcname=bundle.name)
    return archive


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=("linux", "macos", "windows"), required=True)
    parser.add_argument("--binary-dir", type=Path, default=Path("target/release"))
    parser.add_argument("--output", type=Path, default=Path("dist"))
    args = parser.parse_args()
    print(package(args.platform, args.binary_dir.resolve(), args.output.resolve()))
