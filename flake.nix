{
  description = "riso - modular ricing framework";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "riso";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          meta = {
            description = "Modular ricing framework";
            mainProgram = "riso";
            license = pkgs.lib.licenses.mit;
          };
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            # toolchain
            cargo
            rustc
            rust-analyzer

            # CI gates
            rustfmt
            clippy

            # conformance fixtures are regenerated from the upstream shell
            # pipeline, which is bash and awk
            bash
            gawk
            git

            # task runner
            just
          ];

          # The upstream scripts carry a /bin/bash shebang that does not exist
          # here, so the fixture script rewrites them before running.
          RUST_BACKTRACE = "1";
        };
      });
}
