{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    nixpkgs-750fc50b.url = "github:NixOS/nixpkgs/750fc50bfd132a44972aa15bb21937ae26303bc4";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      nixpkgs-750fc50b,
      rust-overlay,
      crane,
    }:
    let
      overlays = [
        rust-overlay.overlays.default
        (final: prev: {
          rustToolchain = prev.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-analyzer"
              "rust-src"
              "rustfmt"
            ];
          };
        })
      ];
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f rec {
            pkgs-750fc50b = import nixpkgs-750fc50b { inherit system; };
            pkgs = import nixpkgs {
              inherit system;
              overlays = overlays ++ [
                (final: prev: {
                  meilisearch = pkgs-750fc50b.meilisearch.override (old: {
                    rustPlatform = old.rustPlatform // {
                      buildRustPackage =
                        args:
                        old.rustPlatform.buildRustPackage (
                          args
                          // {
                            version = "v1.2.1";

                            src = pkgs-750fc50b.fetchFromGitHub {
                              owner = "meilisearch";
                              repo = "MeiliSearch";
                              rev = "refs/tags/v1.2.1";
                              hash = "sha256-snoC6ZnKJscwoXdw4TcZsjoygxAjpsBW1qlhoksCguY=";
                            };

                            cargoLock = {
                              lockFile = pkgs-750fc50b.fetchurl {
                                url = "https://github.com/meilisearch/meilisearch/raw/v1.2.1/Cargo.lock";
                                hash = "sha256-ZHHjJK83jOezmXBnbknx8zXSplxmqETUesXcSLr6FqE=";
                              };
                              outputHashes = {
                                "actix-web-static-files-3.0.5" = "sha256-2BN0RzLhdykvN3ceRLkaKwSZtel2DBqZ+uz4Qut+nII=";
                                "heed-0.12.5" = "sha256-WOdpgc3sDNKBSYWB102xTxmY1SWljH9Q1+6xmj4Rb8Q=";
                                "lmdb-rkv-sys-0.15.1" = "sha256-zLHTprwF7aa+2jaD7dGYmOZpJYFijMTb4I3ODflNUII=";
                                "nelson-0.1.0" = "sha256-eF672quU576wmZSisk7oDR7QiDafuKlSg0BTQkXnzqY=";
                              };
                            };
                          }
                        );
                    };
                  });
                })
              ];
            };
            bin = pkgs.lib.makeOverridable (import ./nix/bin.nix) {
              inherit
                system
                pkgs
                nixpkgs
                rust-overlay
                crane
                ;
            };
            dockerImage = pkgs.lib.makeOverridable (import ./nix/docker-image.nix) { inherit pkgs bin; };
          }
        );
    in
    {
      devShells = forEachSupportedSystem (
        { pkgs, bin, ... }:
        {
          default = pkgs.mkShell {
            inputsFrom = [ bin ];

            packages = with pkgs; [
              rustToolchain
              lldb_15

              commitizen
              pre-commit

              meilisearch
            ];
          };
        }
      );

      packages = forEachSupportedSystem (
        {
          pkgs,
          bin,
          dockerImage,
          ...
        }:
        rec {
          inherit bin dockerImage;
          default = dockerImage;

          bin-x86_64 = bin.override { target = "x86_64-unknown-linux-musl"; };
          bin-aarch64 = bin.override { target = "aarch64-unknown-linux-musl"; };

          dockerImage-x86_64 = dockerImage.override {
            pkgs = pkgs.pkgsCross.musl64;
            bin = bin-x86_64;
          };
          dockerImage-aarch64 = dockerImage.override {
            pkgs = pkgs.pkgsCross.aarch64-multiplatform-musl;
            bin = bin-aarch64;
          };
        }
      );
    };
}
