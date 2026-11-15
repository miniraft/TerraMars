# Compatibility patch

This is `xattr` 0.2.3 from crates.io, with its original MIT and Apache-2.0 licenses.
GPUI 0.2.2 brings this version through its archive dependencies.

The only source change is in `src/sys/mod.rs`: Linux and Android use
`libc::ENODATA` for a missing extended attribute. Those platforms no longer expose
the BSD `libc::ENOATTR` alias in current libc versions. macOS and BSD keep their
original constant. The public xattr API and error semantics stay the same.

The application pins this local fix through `[patch.crates-io]`. Remove it when
the upstream GPUI dependency graph no longer requires this old xattr version.
