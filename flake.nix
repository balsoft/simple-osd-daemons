# This is free and unencumbered software released into the public domain.
# balsoft 2020

{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
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
      crate2nix,
    }:
    let
      forAllSystems = f: builtins.mapAttrs (_: f) nixpkgs.legacyPackages;
    in
    {
      packages = forAllSystems (
        pkgs:
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
      );
      apps = builtins.mapAttrs (
        _:
        builtins.mapAttrs (
          _: pkg: {
            type = "app";
            program = "${pkg}/bin/${pkg.pname}";
          }
        )
      ) self.packages;

      defaultPackage = builtins.mapAttrs (
        system: pkgs:
        pkgs.buildEnv {
          name = "simple-osd-daemons";
          paths = builtins.attrValues self.packages.${system};
        }
      ) nixpkgs.legacyPackages;

      devShell = builtins.mapAttrs (
        system: pkgs:
        pkgs.mkShell {
          inputsFrom = builtins.attrValues self.packages.${system};
          buildInputs = [
            pkgs.cargo
            pkgs.rust-analyzer
            pkgs.clippy
          ];
        }
      ) nixpkgs.legacyPackages;
    };
}
