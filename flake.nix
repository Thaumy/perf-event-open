{
  inputs = {
    pkgs.url = "github:NixOS/nixpkgs/eb6cf22e8ee7d1307705d8cac7c1f81b8135c2ca"; # 25-4-3
    rust-overlay = {
      url = "github:oxalica/rust-overlay/7ed7e8c74be95906275805db68201e74e9904f07"; # 25-12-8
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
            "x86_64-unknown-freebsd"
          ];
        };
    in
    {
      devShells.default = pkgs.mkShell {
        name = "perf-event-open";

        # Use nightly fmt for better style
        RUSTFMT = "${rust-toolchain "nightly" "2025-12-08"}/bin/rustfmt";

        nativeBuildInputs = [
          (rust-toolchain "stable" "1.80.0")
        ];

        checkPhase = "./check.sh";
      };
    });
}
