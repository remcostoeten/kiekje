{
  description = "Kiekje screenshot tray app";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "kiekje";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
            src = ./.;
            cargoLock = {
              lockFile = ./Cargo.lock;
              allowBuiltinFetchGit = true;
            };

            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [
              pkgs.cairo
              pkgs.fontconfig
              pkgs.glib
              pkgs.gtk4
              pkgs.gtk4-layer-shell
              pkgs.libxkbcommon
              pkgs.pango
              pkgs.wayland
            ];

            postInstall = ''
              install -Dm644 packaging/dev.remcostoeten.Kiekje.desktop \
                $out/share/applications/dev.remcostoeten.Kiekje.desktop
              substituteInPlace $out/share/applications/dev.remcostoeten.Kiekje.desktop \
                --replace-fail /usr/bin/kiekje $out/bin/kiekje
              for icon in resources/icons/hicolor/*/apps/*; do
                install -Dm644 "$icon" \
                  "$out/share/icons/hicolor/''${icon#resources/icons/hicolor/}"
              done
              install -Dm644 resources/fonts/Geist-Variable.ttf \
                $out/share/kiekje/fonts/Geist-Variable.ttf
              install -Dm644 resources/fonts/GeistMono-Variable.ttf \
                $out/share/kiekje/fonts/GeistMono-Variable.ttf
              install -Dm644 LICENSE $out/share/licenses/kiekje/LICENSE
              install -Dm644 resources/fonts/OFL.txt \
                $out/share/licenses/kiekje/OFL-fonts.txt
            '';

            meta = {
              description = "Capture, mark up and keep screenshots";
              homepage = "https://github.com/remcostoeten/kiekje";
              license = [ pkgs.lib.licenses.mit pkgs.lib.licenses.ofl ];
              mainProgram = "kiekje";
              platforms = nixpkgs.lib.platforms.linux;
            };
          };
        });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/kiekje";
        };
      });

      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.mkShell {
            packages = [ pkgs.cargo pkgs.pkg-config pkgs.rustc ];
            buildInputs = [ pkgs.gtk4 pkgs.gtk4-layer-shell pkgs.wayland ];
          };
        });
    };
}
