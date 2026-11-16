#!/usr/bin/env python3
"""Produce a Linux AppImage, a Windows ZIP, or a macOS DMG containing the app."""

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
import tomllib
import zipfile

# Committed next to the source logo so every macOS bundle has an icon even when
# the host lacks sips/iconutil. Regenerate with tools/make_macos_icon.py.
ICON_NAME = "TerraMars.icns"


def project_version(root: Path) -> str:
    """Read the bundle version from Cargo.toml so it cannot drift."""
    package = tomllib.loads((root / "Cargo.toml").read_text())["package"]
    return str(package["version"])


def macos_minimum_version(root: Path, arch: str) -> str:
    """Use Cargo's deployment target for the bundle, including local builds."""
    config = tomllib.loads((root / ".cargo/config.toml").read_text())
    version = os.environ.get(
        "MACOSX_DEPLOYMENT_TARGET", config["env"]["MACOSX_DEPLOYMENT_TARGET"]
    )
    # Apple Silicon has never supported macOS versions before Big Sur.
    if arch == "arm64" and tuple(map(int, version.split("."))) < (11, 0):
        return "11.0"
    return version


def macos_icon(root: Path, destination: Path) -> None:
    """Build the bundle icon from logo.png, falling back to the committed icns.

    macOS hosts have sips and iconutil, so packaging regenerates a native ICNS
    from ``assets/icons/app_logo/logo.png`` every time the logo changes. Hosts
    without those tools copy the committed file produced by
    ``tools/make_macos_icon.py``.
    """
    logo = root / "assets/icons/app_logo/logo.png"
    if logo.is_file() and shutil.which("sips") and shutil.which("iconutil"):
        macos_icon_from_source(logo, destination)
        return
    committed = root / "assets/icons/app_logo" / ICON_NAME
    if not committed.is_file():
        raise SystemExit(f"Missing {committed}; run tools/make_macos_icon.py")
    shutil.copy2(committed, destination)


def macos_icon_from_source(source: Path, destination: Path) -> None:
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


def package_appimage(root: Path, binary: Path, artifact: Path, arch: str) -> Path:
    """Build a portable AppDir and compress it with appimagetool when available."""
    tool = os.environ.get("APPIMAGETOOL")
    if not tool or not Path(tool).is_file():
        # A raw binary is not an AppImage, so refuse to publish one under that
        # name. CI downloads appimagetool before this step.
        raise SystemExit(
            "appimagetool is required to build the Linux AppImage; "
            "set APPIMAGETOOL to its path"
        )
    with tempfile.TemporaryDirectory(prefix="terramars-appimage-") as directory:
        appdir = Path(directory) / "TerraMars.AppDir"
        bin_dir = appdir / "usr/bin"
        icon_dir = appdir / "usr/share/icons/hicolor/256x256/apps"
        desktop_dir = appdir / "usr/share/applications"
        for path in (bin_dir, icon_dir, desktop_dir):
            path.mkdir(parents=True)
        destination = bin_dir / "terramars"
        shutil.copy2(binary, destination)
        destination.chmod(destination.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        icon = appdir / "terramars.png"
        shutil.copy2(root / "assets/icons/app_logo/logo.png", icon)
        shutil.copy2(icon, icon_dir / "terramars.png")
        desktop = (
            "[Desktop Entry]\n"
            "Type=Application\n"
            "Name=TerraMars\n"
            "Comment=Mars surface explorer and route planner\n"
            "Exec=terramars\n"
            "Icon=terramars\n"
            "Terminal=false\n"
            "Categories=Science;Education;\n"
        )
        (appdir / "TerraMars.desktop").write_text(desktop)
        (desktop_dir / "terramars.desktop").write_text(desktop)
        app_run = appdir / "AppRun"
        app_run.write_text("#!/bin/sh\nexec \"$APPDIR/usr/bin/terramars\" \"$@\"\n")
        app_run.chmod(0o755)
        environment = os.environ.copy()
        environment["ARCH"] = "aarch64" if arch == "arm64" else "x86_64"
        subprocess.run([tool, os.fspath(appdir), os.fspath(artifact)], check=True, env=environment)
    if not artifact.is_file() or artifact.stat().st_size == 0:
        raise SystemExit(f"appimagetool did not produce {artifact}")
    return artifact


def macos_metadata(root: Path, arch: str, version: str) -> dict:
    """Load the checked-in Info.plist and fill in values the build knows.

    macos/Info.plist is the source of truth for the bundle identity. The
    packager only overrides the fields that depend on the binary being bundled:
    versions, icon name, and the deployment minimum actually compiled into that
    binary.
    """
    metadata = plistlib.loads((root / "macos/Info.plist").read_bytes())
    metadata["CFBundleExecutable"] = "terramars"
    metadata["CFBundleIconFile"] = ICON_NAME
    metadata["CFBundleShortVersionString"] = version
    metadata["CFBundleVersion"] = version
    metadata["LSMinimumSystemVersion"] = macos_minimum_version(root, arch)
    return metadata


def package(
    target: str,
    binary_dir: Path,
    output: Path,
    arch: str | None = None,
    version: str | None = None,
    sign_identity: str | None = None,
) -> Path:
    root = Path(__file__).resolve().parents[1]
    binary = binary_dir / ("terramars.exe" if target == "windows" else "terramars")
    if not binary.is_file():
        raise SystemExit(f"Missing {binary}; run cargo build --locked --release first.")
    output.mkdir(parents=True, exist_ok=True)
    arch = (arch or platform.machine()).lower()
    arch = {"amd64": "x86_64", "aarch64": "arm64"}.get(arch, arch)
    version = version or project_version(root)
    suffix = {"linux": ".AppImage", "windows": ".zip", "macos": ".dmg"}[target]
    artifact = output / f"TerraMars-{target}-{arch}{suffix}"

    if target == "windows":
        # Folder-style distribution, as in the first release: one TerraMars/
        # directory holding the executable, documentation, licences and the
        # provenance snapshots people expect beside it.
        with tempfile.TemporaryDirectory(prefix="terramars-package-") as directory:
            staging = Path(directory)
            bundle = staging / "TerraMars"
            bundle.mkdir(parents=True)
            shutil.copy2(binary, bundle / "TerraMars.exe")
            for name in (
                "README.md",
                "docs",
                "vendor/GPUI_PATCHES.md",
            ):
                source = root / name
                if source.is_dir():
                    shutil.copytree(source, bundle / name)
                elif source.is_file():
                    (bundle / name).parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(source, bundle / name)
            licenses = bundle / "font-licenses"
            licenses.mkdir()
            for license_file in (root / "assets/fonts").glob("*.txt"):
                shutil.copy2(license_file, licenses / license_file.name)
            for license_file in (root / "assets/licenses").glob("*.txt"):
                shutil.copy2(license_file, licenses / license_file.name)
            for name in ("LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING"):
                application_license = root / name
                if application_license.is_file():
                    shutil.copy2(application_license, bundle / name)
            shutil.copytree(root / "assets/data", bundle / "assets/data")
            map_manifests = bundle / "assets/maps"
            map_manifests.mkdir(parents=True)
            for manifest in (root / "assets/maps").glob("*.json"):
                shutil.copy2(manifest, map_manifests / manifest.name)
            # crates.io source files can carry Unix-epoch mtimes; ZIP starts at 1980.
            with zipfile.ZipFile(
                artifact,
                "w",
                compression=zipfile.ZIP_DEFLATED,
                strict_timestamps=False,
            ) as stream:
                for path in sorted(bundle.rglob("*")):
                    if path.is_file():
                        stream.write(path, path.relative_to(staging).as_posix())
        return artifact

    if target == "linux":
        return package_appimage(root, binary, artifact, arch)

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
        macos_icon(root, resources / ICON_NAME)
        metadata = macos_metadata(root, arch, version)
        with (contents / "Info.plist").open("wb") as stream:
            plistlib.dump(metadata, stream)
        (contents / "PkgInfo").write_bytes(b"APPL????")
        destination = executable_dir / binary.name
        shutil.copy2(binary, destination)
        destination.chmod(destination.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        for name in ("README.md", "docs", "vendor/GPUI_PATCHES.md", "vendor/rotation.patch"):
            source = root / name
            dest = resources / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            if source.is_dir():
                shutil.copytree(source, dest)
            else:
                shutil.copy2(source, dest)
        licenses = resources / "font-licenses"
        licenses.mkdir()
        for pattern in ("*.txt", "Orbitron/OFL.txt", "DepartureMono-1.500/LICENSE"):
            for license_file in (root / "assets/fonts").glob(pattern):
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
        # With an identity the bundle is Developer ID signed with the hardened
        # runtime and the microphone entitlement, ready for notarization. Without
        # one, an ad-hoc signature keeps the local bundle structurally valid on
        # Apple Silicon; that does not replace signing or notarization.
        if sign_identity:
            subprocess.run(
                [
                    "codesign", "--force", "--options", "runtime", "--timestamp",
                    "--entitlements", os.fspath(root / "macos/entitlements.plist"),
                    "--sign", sign_identity, os.fspath(bundle),
                ],
                check=True,
            )
            subprocess.run(
                ["codesign", "--verify", "--strict", "--verbose=2", os.fspath(bundle)],
                check=True,
            )
        else:
            subprocess.run(
                ["codesign", "--force", "--sign", "-", os.fspath(bundle)], check=True
            )
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
    # Explicit arch keeps cross-compiled CI runners honest; a universal macOS
    # build reports "universal" and uses the deployment floor from the target.
    parser.add_argument("--arch", default=None, help="x86_64, arm64 or universal")
    parser.add_argument("--version", default=None, help="CFBundleShortVersionString")
    parser.add_argument(
        "--sign-identity",
        default=None,
        help="Developer ID identity for the macOS bundle; omit for an ad-hoc signature",
    )
    args = parser.parse_args()
    print(
        package(
            args.platform,
            args.binary_dir.resolve(),
            args.output.resolve(),
            arch=args.arch,
            version=args.version,
            sign_identity=args.sign_identity,
        )
    )
