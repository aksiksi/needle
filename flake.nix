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
          cargoSha256 = "sha256-aUKzSbyniuk2+UZmrUZjMv+yhcjn+eilnZtCMxzJLZo=";
          # nativeBuildInputs: used only in build phase
          nativeBuildInputs = [
            pkgs.chromaprint
            pkgs.cmake
            pkgs.ffmpeg-full
            pkgs.llvmPackages.clang
            pkgs.pkg-config
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.fftw ];
          # buildInputs: used only at runtime (i.e., linked against)
          # https://nixos.org/manual/nixpkgs/stable/#ssec-stdenv-dependencies-overview
          buildInputs = [ pkgs.ffmpeg-full ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.fftw ];
          # Required to allow build to "see" libclang (used by bindgen I think)
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
          packages = [
            pkgs.cargo
            pkgs.chromaprint
            pkgs.cmake
            pkgs.ffmpeg-full
            pkgs.llvmPackages.clang
            pkgs.pkg-config
            pkgs.rustc
            pkgs.rust-analyzer
            pkgs.rustfmt
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.fftw ];
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        };
      }
    );
  };
}

