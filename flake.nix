{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-26.05";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }@inputs:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };
      name = "lari-web";
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        shellHook = ''
          PS1="\[\033[36m\][flake]\[\033[0m\] $PS1"
          alias build="cargo build -r -p lari-web --target wasm32-unknown-unknown && wasm-bindgen --no-typescript --target=web --out-dir target/web --out-name index ./target/wasm32-unknown-unknown/release/${name}.wasm"
        '';
        packages = with pkgs; [
          lld
          wasm-bindgen-cli_0_2_126

          (rust-bin.selectLatestNightlyWith (
            toolchain:
            toolchain.default.override {
              extensions = [ "rust-src" ];
              targets = [ "wasm32-unknown-unknown" ];
            }
          ))
        ];
      };
    };
}
