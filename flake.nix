{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    rust-overlay,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [rust-overlay.overlays.default];
      };

      osxDependencies = with pkgs;
        lib.optionals stdenv.isDarwin
        [
          darwin.apple_sdk.frameworks.Security
          darwin.apple_sdk.frameworks.CoreServices
          darwin.apple_sdk.frameworks.SystemConfiguration
        ];

      cargoTomlContents = builtins.readFile ./Cargo.toml;

      version = (builtins.fromTOML cargoTomlContents).workspace.package.version;
      rustVersion = "1.88.0";

      rustToolchain = pkgs.rust-bin.stable.${rustVersion}.default;

      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };

      iaiken = rustPlatform.buildRustPackage {
        inherit version;

        name = "iaiken";

        buildInputs = with pkgs; [openssl] ++ osxDependencies;
        nativeBuildInputs = with pkgs; [pkg-config openssl.dev];

        src = pkgs.lib.cleanSourceWith {src = self;};
        doCheck = false; # don't run cargo test
        CARGO_BUILD_TESTS = "false"; # don't even compile test binaries

        cargoLock = {
          lockFile = ./Cargo.lock;
          outputHashes = {
            "aiken-lang-1.1.19" = "sha256-PZ4AbgMmUKyfeQXph0StWdBBPb1uuDStuA2dUHMSX/g=";
          };
        };

        meta = with pkgs.lib; {
          description = "Jupyter kernel for Aiken smart contract language";
          homepage = "https://github.com/rober-m/iaiken";
          license = licenses.asl20;
          mainProgram = "iaiken";
        };
      };

      aiken-repl = rustPlatform.buildRustPackage {
        inherit version;

        name = "aiken-repl";

        buildInputs = with pkgs; [openssl] ++ osxDependencies;
        nativeBuildInputs = with pkgs; [pkg-config openssl.dev];

        src = pkgs.lib.cleanSourceWith {src = self;};
        doCheck = false; # don't run cargo test
        CARGO_BUILD_TESTS = "false"; # don't even compile test binaries

        cargoLock = {
          lockFile = ./Cargo.lock;
          outputHashes = {
            "aiken-lang-1.1.19" = "sha256-PZ4AbgMmUKyfeQXph0StWdBBPb1uuDStuA2dUHMSX/g=";
          };
        };

        meta = with pkgs.lib; {
          description = "REPL for the Aiken smart contract language";
          homepage = "https://github.com/rober-m/iaiken";
          license = licenses.asl20;
          mainProgram = "aiken-repl";
        };
      };

      packages = {
        iaiken = iaiken;
        aiken-repl = aiken-repl;
        default = packages.iaiken;
      };

      overlays.default = final: prev: {iaiken = packages.iaiken;};

      gitRev =
        if (builtins.hasAttr "rev" self)
        then self.rev
        else "dirty";
    in {
      inherit packages overlays;

      devShell = pkgs.mkShell {
        buildInputs = with pkgs;
          [
            pkg-config
            openssl
            cargo-insta
            (rustToolchain.override {
              extensions = ["rust-src" "clippy" "rustfmt" "rust-analyzer"];
            })
          ]
          ++ osxDependencies;

        shellHook = ''
          export GIT_REVISION=${gitRev}
        '';
      };
    });
}
