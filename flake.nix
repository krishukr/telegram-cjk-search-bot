{
  description = "A Nix-flake-based Rust development environment";

  inputs =
    {
      nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
      rust-overlay = {
        url = "github:oxalica/rust-overlay";
        inputs.nixpkgs.follows = "nixpkgs";
      };
    };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      overlays = [
        rust-overlay.overlays.default
        (final: prev: {
          rustToolchain = prev.rust-bin.stable.latest.default.override {
            extensions = [ "rust-analyzer" "rust-src" "rustfmt" ];
          };
        })
        (final: prev: {
          meilisearch = prev.meilisearch.override (old: {
            rustPlatform = old.rustPlatform // {
              buildRustPackage = args: old.rustPlatform.buildRustPackage (args // {
                version = "v1.2.1";

                src = prev.fetchFromGitHub {
                  owner = "meilisearch";
                  repo = "MeiliSearch";
                  rev = "refs/tags/v1.2.1";
                  hash = "sha256-snoC6ZnKJscwoXdw4TcZsjoygxAjpsBW1qlhoksCguY=";
                };

                cargoLock = {
                  lockFile = prev.fetchurl {
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
              });
            };
          });
        }
        )
      ];
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forEachSupportedSystem = f: nixpkgs.lib.genAttrs supportedSystems (system: f {
        pkgs = import nixpkgs { inherit overlays system; };
      });
    in
    {
      devShells = forEachSupportedSystem ({ pkgs }: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain
            openssl
            pkg-config
            # rust-analyzer
            lldb_15

            meilisearch
          ];
        };
      });
    };
}
