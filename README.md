# check-updates

A Nagios/NRPE plugin for monitoring system updates using PackageKit, written in Rust.

[![Rust](https://github.com/liberodark/check-updates/actions/workflows/rust.yml/badge.svg)](https://github.com/liberodark/check-updates/actions/workflows/rust.yml)
[![Security Audit](https://github.com/liberodark/check-updates/actions/workflows/security-audit.yml/badge.svg)](https://github.com/liberodark/check-updates/actions/workflows/security-audit.yml)

## Features

- Check for available system updates via PackageKit D-Bus API
- Identify security updates (CVE-based detection)
- Apply updates automatically (optional)
- Lock file support to prevent concurrent execution
- Cron-like scheduling support
- Compatible with Nagios performance data format
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
sudo cp target/release/check_updates /usr/local/nagios/libexec/
sudo chmod +x /usr/local/nagios/libexec/check_updates
```

### Create symlink for command-line usage
```bash
sudo ln -s /usr/local/nagios/libexec/check_updates /usr/bin/check_updates
```

### Precompiled binaries
Precompiled binaries are available in the [Releases](https://github.com/liberodark/check-updates/releases) section.

## Usage

### Basic usage

Check for updates without applying them:
```bash
check_updates
```

### With warning and critical thresholds
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
check_updates --lock /tmp/check_updates.lock -w 10 -c 20
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
  -w, --warning <N>        Return warning if more than N security updates are available [default: 10]
  -c, --critical <N>       Return critical if more than N security updates are available [default: 20]
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

The plugin follows Nagios plugin standards:

```
UPDATE OK - Security-Update = 0 | 'Total Update' = 5 'Security Update' = 0
UPDATE WARNING - Security-Update = 12 | 'Total Update' = 25 'Security Update' = 12
UPDATE CRITICAL - Security-Update = 25 | 'Total Update' = 40 'Security Update' = 25
```

## Exit Codes

- 0: OK - Updates below thresholds
- 1: WARNING - Security updates exceed warning threshold
- 2: CRITICAL - Security updates exceed critical threshold or error occurred
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

## Differences from C Version

This Rust implementation offers several improvements over the original C version:

- **Memory safety**: No buffer overflows or memory leaks
- **Better error handling**: Comprehensive error messages with context
- **Native D-Bus**: Direct PackageKit API communication instead of shell commands
- **Async operations**: Non-blocking I/O with tokio
- **Type safety**: Compile-time guarantees with Rust's type system

## License

This project is distributed under the [GPL-3.0](LICENSE) license.
