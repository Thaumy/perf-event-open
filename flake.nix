{
  inputs = {
    pkgs.url = "github:NixOS/nixpkgs/7b033cac491de078d36d15b07f0036ab197a3180"; # 26-9-16
    rust-overlay = {
      url = "github:oxalica/rust-overlay/35ca0490d13a3d38c4602d0eb9600a30fa63a367"; # 26-9-16
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

      rustfmt = pkgs.rust-bin.nightly."2026-09-16".rustfmt;
      rust-toolchain = pkgs.rust-bin.stable."1.98.1".complete.override {
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
        RUSTFMT = "${rustfmt}/bin/rustfmt";

        nativeBuildInputs = [
          rust-toolchain
        ];

        checkPhase = "./check.sh";
      };
    });
}
