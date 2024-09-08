{
  description = "A CLI tool that finds a needle (opening/intro and ending/credits) in a haystack (TV or anime episode).";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs, ... }: let
    supportedSystems = [ "x86_64-linux" "x86_64-darwin" "aarch64-linux" "aarch64-darwin" ];
    forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    pname = "needle";
    owner = "aksiksi";
    version = "0.1.5";
  in {
    packages = forAllSystems (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        default = pkgs.rustPlatform.buildRustPackage {
          inherit pname;
          inherit version;
          src = ./needle;
          cargoSha256 = "sha256-CUcjt7BLvTSaiiYCuzVilEf0y1zN08Bo8YtvKFVTSiM=";
          # nativeBuildInputs: used only in build phase
          nativeBuildInputs = [
            pkgs.cmake
            pkgs.llvmPackages.clang
            pkgs.pkg-config
          ];
          # buildInputs: used only at runtime (i.e., linked against)
          # https://nixos.org/manual/nixpkgs/stable/#ssec-stdenv-dependencies-overview
          buildInputs = [
            pkgs.ffmpeg-full
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            pkgs.fftw
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Accelerate
            pkgs.darwin.apple_sdk.frameworks.AVFoundation
          ];
          # Required to allow build to "see" libclang (used by bindgen I think).
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          meta = {
            description = "A CLI tool that finds a needle (opening/intro and ending/credits) in a haystack (TV or anime episode).";
            homepage = "https://github.com/aksiksi/needle";
            license = [ pkgs.lib.licenses.mit pkgs.lib.licenses.lgpl21 ];
            maintainers = [];
          };
        };
      }
    );

    # Development shell
    # nix develop
    devShells = forAllSystems (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        default = pkgs.mkShell {
          buildInputs = [
            pkgs.ffmpeg-full
            pkgs.libiconv    # required by rust-ffmpeg build script
            pkgs.pkg-config
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            pkgs.fftw
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Accelerate
            pkgs.darwin.apple_sdk.frameworks.AVFoundation
          ];
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          packages = [
            pkgs.cargo
            pkgs.cmake
            pkgs.llvmPackages.clang
            pkgs.pkg-config
            pkgs.rustc
            pkgs.rust-analyzer
            pkgs.rustfmt
          ];
        };
      }
    );
  };
}

