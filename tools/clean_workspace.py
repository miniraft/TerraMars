#!/usr/bin/env python3
"""Remove reproducible development files, retaining the release app and packages.

Run after building/packaging, with no Cargo builds in progress. --toolchains also
removes downloaded project-local Rust toolchains and registries; future builds
download them again. Source, bundled assets, and mission storage are untouched.
"""
import argparse
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def remove(path: Path) -> None:
    if path.is_symlink() or path.is_file():
        path.unlink()
    elif path.is_dir():
        shutil.rmtree(path)


def clean(toolchains: bool) -> None:
    target = ROOT / "target"
    for child in target.iterdir() if target.is_dir() else []:
        if child.name != "release":
            remove(child)
    release = target / "release"
    for child in release.iterdir() if release.is_dir() else []:
        if child.name not in {"terramars", "terramars.exe"}:
            remove(child)
    remove(ROOT / "build/previews")
    for directory in (ROOT / "tools").rglob("__pycache__"):
        remove(directory)
    if toolchains:
        for relative in (
            ".tools/cargo/registry", ".tools/cargo/git",
            ".tools/rustup/toolchains", ".tools/rustup/downloads", ".tools/rustup/tmp",
            ".tools/fonttools",
        ):
            remove(ROOT / relative)
    print("Removed development caches. Release app, packages, source and mission log retained.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchains", action="store_true", help="also discard downloaded local Rust tools/dependencies")
    clean(parser.parse_args().toolchains)
