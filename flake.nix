{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;

          overlays = [ (import rust-overlay) ];
        };

        runtimeInputs = with pkgs; [
          rust-bin.stable.latest.minimal
          stdenv.cc
        ];

        nightlyRuntimeInputs = with pkgs; [
          (rust-bin.selectLatestNightlyWith (toolchain: toolchain.minimal))
          stdenv.cc
        ];

        exportCommands = ''
          export CARGO_BUILD_JOBS=1
          export OPENSSL_INCLUDE_DIR="${pkgs.openssl.dev}/include";
          export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib";
        '';

        buildCommand = ''
          cargo fetch
          time cargo build --release
        '';
        
        threads = "-Zthreads=8";
      in
      {
        packages = {
          codegenunits-1 = pkgs.writeShellApplication {
            inherit runtimeInputs;

            name = "codegenunits-1";

            text = ''
              ${exportCommands}
              export RUSTFLAGS="-Ccodegen-units=1"

              ${buildCommand}
            '';
          };

          codegenunits-1-nightly = pkgs.writeShellApplication {
            name = "codegenunits-1-nightly";

            runtimeInputs = nightlyRuntimeInputs;

            text = ''
              ${exportCommands}
              export RUSTFLAGS="-Ccodegen-units=1 ${threads}"

              ${buildCommand}
            '';
          };

          codegenunits-10 = pkgs.writeShellApplication {
            inherit runtimeInputs;

            name = "codegenunits-10";

            text = ''
              ${exportCommands}
              export RUSTFLAGS="-Ccodegen-units=10"

              ${buildCommand}
            '';
          };

          codegenunits-10-nightly = pkgs.writeShellApplication {
            name = "codegenunits-10-nightly";

            runtimeInputs = nightlyRuntimeInputs;

            text = ''
              ${exportCommands}
              export RUSTFLAGS="-Ccodegen-units=10 ${threads}"

              ${buildCommand}
            '';
          };

          codegenunits-16 = pkgs.writeShellApplication {
            inherit runtimeInputs;

            name = "codegenunits-16";

            text = ''
              ${exportCommands}
              export RUSTFLAGS="-Ccodegen-units=16"

              ${buildCommand}
            '';
          };

          codegenunits-16-nightly = pkgs.writeShellApplication {
            name = "codegenunits-16-nightly";

            runtimeInputs = nightlyRuntimeInputs;

            text = ''
              ${exportCommands}
              export RUSTFLAGS="-Ccodegen-units=16 ${threads}"

              ${buildCommand}
            '';
          };
        };
      }
    );
}
