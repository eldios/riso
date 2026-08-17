# Packaging

`riso` is one binary plus a manpage and a licence. It calls `git` and `curl`
at run time to install themes and plugins; everything else it needs is
compiled in.

## Arch

From a release tag:

```bash
cd packaging && makepkg -si
```

From the repository tip (and the only path while the repository is private,
over ssh):

```bash
cd packaging && makepkg -sip PKGBUILD-git
```

## Debian, Ubuntu, Mint

```bash
cargo build --release
cargo deb -p riso-cli --no-build
```

Reads `package.metadata.deb` from `crates/riso-cli/Cargo.toml`, so the manpage,
the licence and the runtime dependencies come along without a control file.

## Fedora, RHEL, openSUSE

```bash
cargo build --release
rpmbuild --define "_topdir $PWD/target/rpm" -bb packaging/riso.spec
```

The spec installs an already-built binary, so every package ships the same
artifact. Copy `target/release/riso`, `docs/riso.1`, `LICENSE`, `NOTICE` and
`README.md` into `target/rpm/SOURCES` first.

On a distribution whose rpm is not installed under `/usr`, add
`--define "_prefix /usr"`.

## Release artifacts

Every `v*` tag builds the `.deb`, the `.rpm` and a binary tarball and
attaches them to the GitHub release. On a machine with `gh`:

```bash
gh release download --repo eldios/riso --pattern '*.deb'
sudo dpkg -i riso_*.deb
```

```bash
gh release download --repo eldios/riso --pattern '*.rpm'
sudo rpm -i riso-*.rpm
```

## Nix

The flake exposes the package and an overlay:

```nix
inputs.riso.url = "github:eldios/riso";
# then either
environment.systemPackages = [ inputs.riso.packages.${system}.default ];
# or
nixpkgs.overlays = [ inputs.riso.overlays.default ];
```

## Verifying a build

```bash
just ci            # format, lint, tests
just conformance   # byte-for-byte against Omarchy's own renderer
```
