# Packaging

`riso` is one binary plus a manpage and a licence. It calls `git` and `curl`
at run time to install themes and plugins; everything else it needs is
compiled in.

## Arch

```bash
cd packaging && makepkg -si
```

## Debian and RPM

Both are built from the same crate metadata with the standard cargo helpers,
which are not vendored here:

```bash
cargo install cargo-deb cargo-generate-rpm

cargo deb                 # target/debian/riso_0.1.0_amd64.deb
cargo build --release && cargo generate-rpm
```

The manpage and licence are declared in `Cargo.toml` under
`package.metadata.deb` and `package.metadata.generate-rpm`, so both packages
carry them without a spec file.

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
