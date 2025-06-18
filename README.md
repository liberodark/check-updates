# check-updates

A simple command-line tool for checking and applying system updates using PackageKit, written in Rust.

[![Rust](https://github.com/liberodark/check-updates/actions/workflows/rust.yml/badge.svg)](https://github.com/liberodark/check-updates/actions/workflows/rust.yml)
[![Security Audit](https://github.com/liberodark/check-updates/actions/workflows/security-audit.yml/badge.svg)](https://github.com/liberodark/check-updates/actions/workflows/security-audit.yml)

## Features

- Check for available system updates via PackageKit D-Bus API
- Identify security updates (CVE-based detection)
- Apply updates automatically (optional)
- Lock file support to prevent concurrent execution
- Cron-like scheduling support
- Multi-distribution support (any system with PackageKit)
- Native D-Bus communication for better performance

## Prerequisites

### Required System Dependencies

The following packages must be installed on your system:
- `packagekit` - System package management service
- `dbus` - D-Bus message bus system

### Installing Dependencies by Distribution

#### Debian/Ubuntu
```bash
sudo apt-get update
sudo apt-get install -y packagekit libdbus-1-3
```

#### Fedora/RHEL/CentOS/AlmaLinux/Rocky Linux
```bash
sudo dnf install -y PackageKit dbus
```

#### Arch Linux
```bash
sudo pacman -S packagekit dbus
```

#### openSUSE
```bash
sudo zypper install PackageKit dbus-1
```

## Installation

### From source
```bash
git clone https://github.com/liberodark/check-updates.git
cd check-updates
cargo build --release
sudo cp target/release/check_updates /usr/local/bin/
sudo chmod +x /usr/local/bin/check_updates
```

### Precompiled binaries
Precompiled binaries are available in the [Releases](https://github.com/liberodark/check-updates/releases) section.

## Usage

### Basic usage

Check for updates without applying them:
```bash
check_updates
```

### Apply all updates
```bash
sudo check_updates --update -y
```

### Apply only security updates
```bash
sudo check_updates --security-update -y
```

### With lock file (prevent concurrent execution)
```bash
check_updates --lock /tmp/check_updates.lock
```

### With cron scheduling
Run updates daily at midnight:
```bash
check_updates --lock /tmp/check_updates.lock --cron "@daily" --security-update -y
```

Run updates every 6 hours:
```bash
check_updates --lock /tmp/check_updates.lock --cron "0 */6 * *" --update -y
```

## Command-line Options

```
Options:
  --lock <FILE>            Lock file to prevent concurrent execution
  --cron <CRON_SPEC>       Abort execution if run before the end of the current period (requires --lock)
  --security-update        Apply security updates
  --update                 Apply all updates
  -y, --yes               Non-interactive mode (assume yes)
  -h, --help              Print help
  -V, --version           Print version
```

### Cron Specification

The `--cron` option accepts simplified cron expressions:

- `M H d m` - Standard cron format (minute hour day month)
- `@hourly` - Run once per hour (0 * * *)
- `@daily` or `@midnight` - Run once per day (0 0 * *)
- `@monthly` - Run once per month (0 0 1 *)
- `@annually` or `@yearly` - Run once per year (0 0 1 1)

Fields can be:
- A specific number
- `*` for any value
- `*/n` for steps (e.g., `*/5` for every 5 units)

## Output Format

The tool provides clear and simple output:

```
Available updates: 25 total (5 security)
------------------------------------------------------------
firefox 121.0-1 [SECURITY]
kernel 6.6.8-1
vim 9.0.2155-1
...

5 updates will be applied
Applying updates...
Updates applied successfully
```

## Exit Codes

- 0: Success
- 1: Error occurred

## Cron Examples

### System crontab entries

Daily security updates at 2 AM:
```cron
0 2 * * * root /usr/local/bin/check_updates --lock /var/lock/check_updates.lock --cron "@daily" --security-update -y
```

Weekly full updates on Sunday at 3 AM:
```cron
0 3 * * 0 root /usr/local/bin/check_updates --lock /var/lock/check_updates.lock --cron "@weekly" --update -y
```

## Development

### Building

```bash
cargo build
cargo test
cargo clippy -- -D warnings
```

### Using Nix Shell

```bash
nix-shell
cargo build
```

## Supported Distributions

Any Linux distribution with PackageKit support:
- Debian/Ubuntu (apt backend)
- RHEL/CentOS/AlmaLinux/Rocky Linux (yum/dnf backend)
- Fedora (dnf backend)
- openSUSE (zypper backend)
- Arch Linux (pacman backend)

## License

This project is distributed under the [GPL-3.0](LICENSE) license.
