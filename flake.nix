# This is free and unencumbered software released into the public domain.
# balsoft 2020

{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crate2nix = {
      url = "github:nix-community/crate2nix";
      flake = false;
    };
  };

  description = "A collection of simple on-screen-display daemons";

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      crate2nix,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages =
          let
            commonDeps = with pkgs; [
              libnotify
              gdk-pixbuf
              glib
            ];

            inherit (import "${crate2nix}/tools.nix" { inherit pkgs; })
              generatedCargoNix
              ;

            project =
              pkgs.callPackage
                (generatedCargoNix {
                  name = "simple-osd-daemons";
                  src = ./.;
                })
                {
                  defaultCrateOverrides = pkgs.defaultCrateOverrides // {
                    simple-osd-battery = oa: { buildInputs = commonDeps; };
                    simple-osd-common = oa: {
                      buildInputs = commonDeps ++ [
                        pkgs.dbus.lib
                        pkgs.dbus.dev
                      ];
                      nativeBuildInputs = [ pkgs.pkg-config ];
                    };
                    simple-osd-brightness = oa: { buildInputs = commonDeps; };
                    simple-osd-pulseaudio = oa: {
                      buildInputs = commonDeps ++ [ pkgs.libpulseaudio ];
                      postInstall = "patchelf --add-rpath ${pkgs.libpulseaudio}/lib $out/bin/*";
                    };
                    simple-osd-mpris = oa: {
                      buildInputs = commonDeps ++ [ pkgs.libpulseaudio ];
                      postInstall = "patchelf --add-rpath ${pkgs.libpulseaudio}/lib $out/bin/*";
                    };
                  };
                };
            membersList = builtins.attrValues (
              builtins.mapAttrs (name: member: {
                name = pkgs.lib.removePrefix "simple-osd-" name;
                value = member.build;
              }) project.workspaceMembers
            );
          in
          builtins.listToAttrs membersList
          // {
            default = pkgs.buildEnv {
              name = "simple-osd-daemons";
              paths = map (x: x.value) membersList;
            };
          };

        devShells.default = pkgs.mkShell {
          inputsFrom = builtins.attrValues self.packages.${system};
          buildInputs = [
            pkgs.cargo
            pkgs.rust-analyzer
            pkgs.clippy
          ];
        };
      }
    );
}
