# check-updates

A tool for checking and applying system updates using PackageKit, written in Rust.
It is built either as a plain command-line tool (default) or as a Nagios/NRPE plugin (`nagios` feature).

[![Rust](https://github.com/liberodark/check-updates/actions/workflows/rust.yml/badge.svg)](https://github.com/liberodark/check-updates/actions/workflows/rust.yml)
[![Security Audit](https://github.com/liberodark/check-updates/actions/workflows/security-audit.yml/badge.svg)](https://github.com/liberodark/check-updates/actions/workflows/security-audit.yml)

## Features

- Check for available system updates via PackageKit D-Bus API
- Identify security updates (CVE-based detection)
- Apply updates automatically (optional)
- Lock file support to prevent concurrent execution
- Cron-like scheduling support
- Optional Nagios/NRPE plugin mode with performance data (`nagios` build feature)
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

Command-line tool (default build):
```bash
git clone https://github.com/liberodark/check-updates.git
cd check-updates
cargo build --release
sudo install -m 755 target/release/check_updates /usr/local/bin/
```

Nagios/NRPE plugin:
```bash
cargo build --release --features nagios
sudo install -m 755 target/release/check_updates /usr/local/nagios/libexec/
```

A binary that runs on any x86_64 Linux distribution regardless of its glibc version
(e.g. built on a recent distribution and deployed on Debian 12) can be produced as a static musl build:
```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl [--features nagios]
# Binary: target/x86_64-unknown-linux-musl/release/check_updates
```

### Precompiled binaries
Precompiled static binaries are available in the [Releases](https://github.com/liberodark/check-updates/releases) section:
- `check_updates-<version>-linux-x86_64`: command-line tool
- `check_updates-nagios-<version>-linux-x86_64`: Nagios/NRPE plugin

## Usage

### Basic usage

Check for updates without applying them:
```bash
check_updates
```

### With warning and critical thresholds (Nagios build)
```bash
check_updates -w 10 -c 20
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
  --lock <FILE>            Avoid concurrent execution by locking specified file
  --cron <CRON_SPEC>       Abort execution if run before the end of the current period (requires --lock)
  -w, --warning <N>        Return warning if at least N security updates are available [default: 10] (Nagios build only)
  -c, --critical <N>       Return critical if at least N security updates are available [default: 20] (Nagios build only)
  --security-update        Apply security updates instead of just showing them
  --update                 Apply all updates instead of just showing them
  -y, --yes               Disable interactive mode (assume yes)
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

## Nagios/NRPE Configuration

Requires the Nagios build (`--features nagios` or the `check_updates-nagios-*` release binary).

Add to your NRPE configuration (`/usr/local/nagios/etc/nrpe.cfg`):

```bash
# Basic check
command[check_updates]=/usr/local/nagios/libexec/check_updates --lock /tmp/check_updates.lock -w $ARG1$ -c $ARG2$

# With automatic security updates
command[check_updates_auto]=sudo /usr/local/nagios/libexec/check_updates --lock /tmp/check_updates.lock --security-update -y -w $ARG1$ -c $ARG2$
```

### Sudo Configuration

For automatic updates, add to `/etc/sudoers.d/nagios`:
```
nagios ALL=(ALL) NOPASSWD: /usr/local/nagios/libexec/check_updates
```

## Output Format

### Command-line build

```
Available updates: 25 total (5 security)
------------------------------------------------------------
firefox 121.0-1 [SECURITY]
kernel 6.6.8-1
vim 9.0.2155-1
...
```

### Nagios build

The plugin follows Nagios plugin standards (security updates listed as long output):

```
UPDATE Warning - Security-Update = 12 | 'Total Update' = 25
Security updates:
firefox 121.0-1 (SECURITY)
...
 | 'Total Update'=25 'Security Update'=12
```

## Exit Codes

### Command-line build

- 0: Success
- 1: Error occurred

### Nagios build

- 0: OK - Security updates below thresholds
- 1: WARNING - Security updates reach the warning threshold, or lock file already held
- 2: CRITICAL - Security updates reach the critical threshold, operation cancelled or error occurred
- 3: UNKNOWN - Unable to determine update status

## Cron Examples

### System crontab entries

Daily security updates at 2 AM:
```cron
0 2 * * * root /usr/bin/check_updates --lock /var/lock/check_updates.lock --cron "@daily" --security-update -y
```

Weekly full updates on Sunday at 3 AM:
```cron
0 3 * * 0 root /usr/bin/check_updates --lock /var/lock/check_updates.lock --cron "@weekly" --update -y
```

## Development

### Building

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features nagios -- -D warnings
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

## Differences from C Version

This Rust implementation offers several improvements over the original C version:

- **Memory safety**: No buffer overflows or memory leaks
- **Better error handling**: Comprehensive error messages with context
- **Native D-Bus**: Direct PackageKit API communication instead of shell commands
- **Async operations**: Non-blocking I/O with tokio
- **Type safety**: Compile-time guarantees with Rust's type system

## License

This project is distributed under the [GPL-3.0](LICENSE) license.
