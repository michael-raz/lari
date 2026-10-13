{
	inputs = {
		nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-26.05";
	};

	outputs = { self, nixpkgs }@inputs:
		let
			system = "x86_64-linux";
			pkgs = import nixpkgs { inherit system; };
			name = "lari-web";
		in
		{
			devShells.${system}.default = pkgs.mkShell {
				shellHook = ''
					PS1="\[\033[36m\][flake]\[\033[0m\] $PS1"
					alias build="cargo build -r --target wasm32-unknown-unknown && wasm-bindgen --no-typescript --target=web --out-dir target/web --out-name index ./target/wasm32-unknown-unknown/release/${name}.wasm"
				'';
				packages = with pkgs; [
					cargo
					lld
					wasm-bindgen-cli_0_2_126
				];
			};
		};
}
