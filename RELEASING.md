# Releasing Cargopit

The release version is stored in `version.txt`. The CMake build, CLI, TUI,
package workflow, and Flatpak metadata use that value. A release tag must
match it exactly; both `0.4.0` and `v0.4.0` are accepted, but use `0.4.0` for
consistency with the existing tags.

## Pre-release checks

Run these commands from a checkout with the simapi submodule initialized:

```bash
git submodule sync --recursive
git submodule update --init --recursive
cmake -B build -DENABLE_TESTS=ON -DCMAKE_BUILD_TYPE=Debug
cmake --build build
ctest --test-dir build --output-on-failure --timeout 30
cargo test --manifest-path tui/Cargo.toml
bash tools/static-analysis.sh --ci --build-dir build/static-analysis
```

Run **Make Packages** from the Actions tab once before tagging. A manual
workflow run builds artifacts without publishing a release, so each package
can be inspected in a clean environment. Confirm the package metadata, the
`cargopit` icon, `cargopit --version`, `cargopit-tui --version`, configuration
creation, and uninstall behavior.

## Publishing

After the pre-release workflow succeeds and the release notes in
`CHANGELOG.md` are ready:

```bash
git tag -a 0.4.0 -m "Release 0.4.0"
git push origin 0.4.0
```

The tag workflow validates `version.txt`, builds the Debian/Ubuntu, Fedora,
AppImage, and Flatpak artifacts, generates SHA-256 files, and uploads them to
the GitHub release. The Flatpak job is allowed to fail independently because
real hardware access inside the sandbox still needs separate validation.

After publication, verify the release assets, checksums, generated notes, and
the install commands in `README.md` from a clean machine.
