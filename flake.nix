{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    fenix,
  } @ inputs: let
    system = "x86_64-linux";
    overlays = [(fenix.overlays.default)];
    pkgs = import nixpkgs {inherit system overlays;};
    lib = pkgs.lib;

    rustToolchain = pkgs.fenix.stable.toolchain;

    hsmDeps = with pkgs; [pkg-config alsa-lib];
  in {
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = with pkgs; [alejandra rustToolchain] ++ hsmDeps;
      RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/src";
      LD_LIBRARY_PATH = lib.makeLibraryPath hsmDeps;
    };

    packages.${system}.homeslashmusic = pkgs.callPackage ./nix/homeslashmusic.nix {};

    defaultPackage.${system} = self.packages.${system}.homeslashmusic;

    overlays.default = final: prev: {
      n16-shell = self.packages.${system}.homeslashmusic;
    };
  };
}
