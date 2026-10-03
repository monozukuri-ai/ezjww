# Preparing and publishing a release

This checkout prepares **0.4.0**. Creating or merging its preparation PR does not
publish packages. The [changelog](../CHANGELOG.md) and [migration guide](MIGRATING_0_4.md)
describe the new writer and Python API changes.

## Local qualification

Use Rust, Python 3.11+ with `maturin`/`pip`, Node.js 22.12+ and pnpm 9.15.1.
The distributed Python package continues to support Python 3.9+; only the release
metadata checker requires `tomllib` from Python 3.11+.

```sh
python scripts/check-release.py
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo fmt --all --check
python scripts/jwc/check_corpus.py
python scripts/jww/check_fixtures.py
maturin build --release --locked --out dist
python scripts/check-wheel.py --wheel-dir dist --expected-version 0.4.0 --report wheel-verification.json
maturin sdist --out sdist
python scripts/check-sdist.py --archive sdist/ezjww-0.4.0.tar.gz --expected-version 0.4.0 --report sdist-verification.json
```

Run package tests/typechecks, browser build, web-WASM and cross-language checks
as in [CI](../.github/workflows/ci.yml). From `packages/ezjww`, pack the npm archive
with `pnpm pack --pack-destination /tmp`; from the repository root verify it:

```sh
node scripts/check-npm-package.mjs --archive /tmp/ezjww-0.4.0.tgz --expected-version 0.4.0 --report npm-verification.json
```

Use fresh output directories so a previous artifact cannot be selected. The
wheel/npm probes install offline into new temporary directories and exercise
JWW creation/readback, exact mixed-drawing bytes, existing JWW/JWC reading and DXF
conversion. The sdist check validates its payload and embedded template, extracts
outside the checkout, builds with the archived lockfile and runs the archived
wheel probe in another clean environment. Rust/build tools and their dependency
caches are still required; this is not a claim of a fully offline source build.

The sdist uses Maturin's [Git generator](https://www.maturin.rs/config.html), so
create it from a Git checkout with the intended files tracked. This preserves
both Cargo workspace members and the shared lockfile. The default Cargo generator
omitted the WASM member but retained its lock entries in this project, causing
`--locked` builds from the archive to fail. The sdist gate checks the complete
workspace and builds without rewriting the lockfile.

## Release workflow

The [release workflow](../.github/workflows/release.yml) has two modes:

- **Run workflow / `workflow_dispatch`:** build and qualify all artifacts, upload
  them and their JSON reports, and skip both publish jobs. Use this on the
  preparation branch to inspect Linux/Windows/macOS wheels, sdist and npm output.
- **Push `vX.Y.Z`:** require the tag to match Python, all Cargo packages, npm and
  lockfile versions; build and qualify every distribution; then publish the exact
  archives from that run. Both registries wait for all build jobs to pass.

The publish jobs verify every archive's SHA-256 against its qualification report.
npm publishes the tested `.tgz` with lifecycle scripts disabled, so `prepack`
cannot create a different WASM binary during publication. Python publishes the
tested three platform wheels and sdist. OIDC permission is limited to the two
publish jobs. The existing PyPI token configuration and npm Trusted Publisher
account configuration must be available when publishing; branch verification
does not validate registry credentials. Registry uploads are separate operations,
so failure at one registry does not roll back the other.

Before a release tag, finalize the changelog date, update all version fields and
locks together, and pass the branch workflow. The final tag push is the publishing
action. Afterward, check both registries and installed-package behavior separately;
successful builds or a merged PR alone do not establish publication.

CI's Windows reader/package checks do not run Jw_cad. Retain the Wine/Windows
distinction and known native limitations in [the compatibility report](JWW_COMPATIBILITY.md).
