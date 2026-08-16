# Packaging

`riso` is one binary plus a manpage and a licence. It calls `git` and `curl`
at run time to install themes and plugins; everything else it needs is
compiled in.

## Arch

```bash
cd packaging && makepkg -si
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
