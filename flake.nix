{
  inputs = {
    pkgs.url = "github:NixOS/nixpkgs/295c3f1c2ac1a55504373727cd6cafb26fb6b047"; # 26-5-23
    rust-overlay = {
      url = "github:oxalica/rust-overlay/40b0a3a193e0840c76174b4a322874c8f6dd0a63"; # 26-5-29
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

      rustfmt = pkgs.rust-bin.nightly."2025-12-08".rustfmt;
      rust-toolchain = pkgs.rust-bin.stable."1.80.1".complete.override {
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
