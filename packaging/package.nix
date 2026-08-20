# The one Nix derivation for riso: the flake builds it via callPackage, and
# the nixpkgs submission is this file with src/cargoHash swapped for
# fetchFromGitHub, so the two can never drift apart.
{
  lib,
  rustPlatform,
  installShellFiles,
  git,
}:

rustPlatform.buildRustPackage {
  pname = "riso";
  # Cargo.toml is the one place the version is written.
  version = (lib.importTOML ../Cargo.toml).workspace.package.version;

  src = lib.cleanSource ../.;
  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [ installShellFiles ];
  # The catalog tests drive a real git; the sandbox has none.
  nativeCheckInputs = [ git ];

  postInstall = ''
    installManPage docs/riso.1
    install -Dm644 NOTICE $out/share/doc/riso/NOTICE
  '';

  meta = {
    description = "Modular ricing framework";
    homepage = "https://github.com/eldios/riso";
    changelog = "https://github.com/eldios/riso/releases";
    mainProgram = "riso";
    license = lib.licenses.mit;
    platforms = lib.platforms.unix;
  };
}
