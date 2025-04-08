{
  inputs = {
    pkgs.url = "github:NixOS/nixpkgs/eb6cf22e8ee7d1307705d8cac7c1f81b8135c2ca"; # 25-4-3
    rust-overlay = {
      url = "github:oxalica/rust-overlay/9d00c6b69408dd40d067603012938d9fbe95cfcd"; # 24-4-6
      inputs.nixpkgs.follows = "pkgs";
    };
    flake-utils.url = "github:numtide/flake-utils/11707dc2f618dd54ca8739b309ec4fc024de578b"; # 24-11-14
  };

  outputs = inputs@{ ... }: inputs.flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import inputs.pkgs {
        inherit system;
        overlays = [ (import inputs.rust-overlay) ];
      };

      rust-toolchain = channel: version:
        pkgs.rust-bin."${channel}"."${version}".complete.override {
          extensions = [ "rust-src" ];
          targets = [
            "x86_64-unknown-linux-gnu"
            "x86_64-unknown-linux-musl"
          ];
        };
    in
    {
      devShells.default = pkgs.mkShell {
        name = "perf-event-open";

        # Use nightly fmt for better style
        RUSTFMT = "${rust-toolchain "nightly" "2025-04-03"}/bin/rustfmt";

        nativeBuildInputs = [
          (rust-toolchain "stable" "1.80.0")
        ];
      };
    });
}
