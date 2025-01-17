{
  system,
  pkgs,
  nixpkgs,
  rust-overlay,
  crane,
  target ? "",
}:
let
  isStatic = (builtins.stringLength target > 0);
  crossPkgs = import nixpkgs (
    {
      localSystem = system;
      overlays = [ rust-overlay.overlays.default ];
    }
    // pkgs.lib.optionalAttrs isStatic { crossSystem.config = target; }
  );
  craneLib = (crane.mkLib crossPkgs).overrideToolchain (
    p:
    p.rust-bin.stable.latest.default.override (
      pkgs.lib.optionalAttrs isStatic { targets = [ target ]; }
    )
  );

  nativeBuildInputs = with crossPkgs.pkgsBuildHost; [ pkg-config ];
  buildInputs =
    with (if isStatic then crossPkgs.pkgsHostHost.pkgsStatic else crossPkgs.pkgsHostHost); [ openssl ];

  src = craneLib.cleanCargoSource ../.;
  commonArgs = {
    inherit src nativeBuildInputs buildInputs;
    strictDeps = true;
  };
in
craneLib.buildPackage (
  commonArgs
  // pkgs.lib.optionalAttrs isStatic {
    CARGO_BUILD_TARGET = target;
    CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";

    "CARGO_TARGET_${
      pkgs.lib.toUpper (builtins.replaceStrings [ "-" ] [ "_" ] target)
    }_LINKER" = "${crossPkgs.stdenv.cc.targetPrefix}cc";
  }
)
