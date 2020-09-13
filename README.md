# Simple OSD daemons

A collection of simple yet configurable OSD daemons that use Freedesktop notifications to display various information about your system.

## Installation

### Nix

These commands will allow you to try the daemons without installing anything globally; Likely, you want to somehow install this on your system after this. How you do it depends on your setup and preferences.

`nix shell git+https://code.balsoft.ru/balsoft/simple-osd-daemons` to get all daemons. Add `#$DAEMON` to only get `$DAEMON`. If you don't use nix 3 yet, try `nix-shell https://code.balsoft.ru/balsoft/simple-osd-daemons/archive/master.tar.gz -A defaultPackage.x86_64-linux`

### Cargo

`cargo build`

## Usage

Run the daemons you need. At this moment, none of them accept any arguments.

### Configuration

`simple-osd-daemons` follows XDG Basedir specification: configuration will be written to `$XDG_CONFIG_HOME/simple-osd/`, typically `~/.config/simple-osd/`. Each daemon has a separate configuration file in INI format, and there is also a `common` configuration file. On startup, the daemons will create their configuration files and populate them with default values if they don't exist.
