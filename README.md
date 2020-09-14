# Simple OSD daemons

A collection of simple yet configurable OSD daemons that use Freedesktop notifications to display various information about your system.

## Installation

### Nix

These commands will allow you to try the daemons without installing anything globally; Likely, you want to somehow install this on your system after this. How you do it depends on your setup and preferences.

`nix shell git+https://code.balsoft.ru/balsoft/simple-osd-daemons` to get all daemons. Add `#$DAEMON` to only get `$DAEMON`. If you don't use nix 3 yet, try `nix-shell https://code.balsoft.ru/balsoft/simple-osd-daemons/archive/master.tar.gz -A defaultPackage.x86_64-linux`

### Cargo

`cargo build`

You can also build individual daemons with `cargo build --bin simple-osd-$DAEMON` .

## Usage

Run the daemons you need. At this moment, none of them accept any arguments.

### Configuration

`simple-osd-daemons` follows XDG Basedir specification: configuration will be written to `$XDG_CONFIG_HOME/simple-osd/`, typically `~/.config/simple-osd/`. Each daemon has a separate configuration file in INI format, and there is also a `common` configuration file. On startup, the daemons will create their configuration files and populate them with default values if they don't exist.

## FIXME

1. Generate `Cargo.nix` on the fly
2. Bluetooth daemon doesn't actually work, since blurz doesn't allow to get the list of connected devices
3. Need to figure out icons

## License

This is free and unencumbered software released into the public domain.

If you want to read more, see [LICENSE](./LICENSE)

## Contributing

If you have any issues or suggestions, shoot an email at <issues@balsoft.ru> with `[simple-osd]` in subj, or [open an issue on GitHub](https://github.com/balsoft/simple-osd-daemons/issues/new)

If you have some concrete changes and would like to share them, submit them with `git send-email` or `git request-pull` to <patches@balsoft.ru> with `simple-osd` in subj or [open a pull request](https://github.com/balsoft/simple-osd-daemons/pulls). Please note that your work will become public domain too if you do this.

***

balsoft, 2020. No rights reserved.
