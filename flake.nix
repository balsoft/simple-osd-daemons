# This is free and unencumbered software released into the public domain.
# balsoft 2020

{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    crate2nix = {
      url = "github:balsoft/crate2nix/tools-nix-version-comparison";
      flake = false;
    };
  };

  description = "A collection of simple on-screen-display daemons";

  outputs = { self, nixpkgs, crate2nix }:
    let forAllSystems = f: builtins.mapAttrs (_: f) nixpkgs.legacyPackages;
    in {
      packages = forAllSystems (pkgs:
        let
          commonDeps = with pkgs; [ libnotify gdk_pixbuf glib ];

          inherit (import "${crate2nix}/tools.nix" { inherit pkgs; })
            generatedCargoNix;

          project = pkgs.callPackage (generatedCargoNix {
            name = "simple-osd-daemons";
            src = ./.;
          }) {
            defaultCrateOverrides = pkgs.defaultCrateOverrides // {
              simple-osd-battery = oa: { buildInputs = commonDeps; };
              simple-osd-common = oa: {
                buildInputs = commonDeps
                  ++ [ pkgs.dbus_tools.lib pkgs.dbus_tools.dev ];
                nativeBuildInputs = [ pkgs.pkg-config ];
              };
              simple-osd-brightness = oa: { buildInputs = commonDeps; };
              # See https://github.com/kolloch/crate2nix/issues/149
              libpulse-binding = oa: {
                preBuild = "sed s/pulse::libpulse.so.0/pulse/ -i target/*link*";
              };
              simple-osd-pulseaudio = oa: {
                buildInputs = commonDeps ++ [ pkgs.libpulseaudio ];
                nativeBuildInputs = [ pkgs.pkg-config ];
                preBuild = "sed s/pulse::libpulse.so.0/pulse/ -i target/*link*";
              };
              simple-osd-mpris = oa: {
                buildInputs = commonDeps ++ [ pkgs.libpulseaudio ];
                preBuild = "sed s/pulse::libpulse.so.0/pulse/ -i target/*link*";
              };
            };
          };
          membersList = builtins.attrValues (builtins.mapAttrs (name: member: {
            name = pkgs.lib.removePrefix "simple-osd-" name;
            value = member.build;
          }) project.workspaceMembers);
        in builtins.listToAttrs membersList);
      apps = builtins.mapAttrs (_:
        builtins.mapAttrs (_: pkg: {
          type = "app";
          program = "${pkg}/bin/${pkg.pname}";
        })) self.packages;

      defaultPackage = builtins.mapAttrs (system: pkgs:
        pkgs.buildEnv {
          name = "simple-osd-daemons";
          paths = builtins.attrValues self.packages.${system};
        }) nixpkgs.legacyPackages;

      devShell = builtins.mapAttrs (system: pkgs:
        pkgs.mkShell {
          inputsFrom = builtins.attrValues self.packages.${system};
          buildInputs = [ pkgs.cargo pkgs.rust-analyzer pkgs.clippy ];
        }) nixpkgs.legacyPackages;
    };
}
