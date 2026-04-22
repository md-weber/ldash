{
  description = "Terminal dashboard TUI for hledger — crypto portfolio, net worth, monthly income/expenses";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        ldash = pkgs.rustPlatform.buildRustPackage {
          pname = "ldash";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          meta = with pkgs.lib; {
            description = "Terminal dashboard TUI for hledger";
            homepage = "https://codeberg.org/md-weber/ldash";
            license = licenses.gpl3Plus;
            maintainers = [ ];
            mainProgram = "ldash";
          };
        };

        # Shared rust toolchain used by checks and devShell
        rustToolchain = pkgs.rustPlatform.rust.rustc;

        checkSrc = pkgs.rustPlatform.buildRustPackage {
          pname = "ldash-clippy";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          buildPhase = ''
            cargo clippy -- -D warnings
          '';
          installPhase = "touch $out";
          doCheck = false;
        };

      in {
        # nix build
        packages.default = ldash;

        # nix run
        apps.default = flake-utils.lib.mkApp { drv = ldash; };

        # nix flake check
        checks = {
          build = ldash;

          fmt = pkgs.runCommand "ldash-fmt" {
            buildInputs = [ pkgs.rustfmt pkgs.cargo ];
            src = ./.;
          } ''
            cargo fmt --manifest-path $src/Cargo.toml -- --check
            touch $out
          '';

          clippy = pkgs.rustPlatform.buildRustPackage {
            pname = "ldash-clippy";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            buildPhase = "cargo clippy -- -D warnings";
            installPhase = "touch $out";
            doCheck = false;
          };

          test = pkgs.rustPlatform.buildRustPackage {
            pname = "ldash-test";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            buildPhase = "cargo test";
            installPhase = "touch $out";
            doCheck = false;
          };
        };

        # nix develop
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustc
            cargo
            rustfmt
            clippy
            rust-analyzer
            hledger
          ];

          RUST_BACKTRACE = 1;
          shellHook = ''
            echo "ldash dev shell — hledger $(hledger --version | head -1)"
          '';
        };
      });
}
