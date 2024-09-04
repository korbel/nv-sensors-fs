# nv-sensor-fs

**[WORK IN PROGRESS]**

Creates a read-only FUSE file system with direct access to the NVIDIA dGPU sensors.

## Requirements

- Install fuse3 (most systems have it already installed)
- Proprietary or the open kernel module NVIDIA driver must be loaded.

## Development

You will need the rust toolchain to have installed. If you are new to Rust, follow these instructions: <https://rustup.rs/>

### Dependencies

#### Fedora

```shell
sudo dnf install fuse3 fuse3-devel pkgconfig
```

#### Ubuntu/Debian

```shell
sudo apt-get install fuse3 libfuse3-dev pkg-config
```

### Build

```shell
cargo build --release
```

### Debug logs

Run the service with the `RUST_LOG=trace` environment variable set.

```shell
export RUST_LOG=trace
sudo --preserve-env=RUST_LOG ./nv-sensors-fs
```

## Options

```shell
$ nv-sensors-fs --help

Creates a read-only FUSE file system with direct access to the NVIDIA dGPU sensors

Usage: nv-sensors-fs [OPTIONS]

Options:
  -m, --mount-point <MOUNT_POINT>  The mount point where the file system will be mounted to [default: /var/lib/nv-sensor-fs]
  -h, --help                       Print help
  -V, --version                    Print version
```

## Example

```shell
$ cat /var/lib/nv-sensor-fs/temp1_input 
45000
```

## TODO

- Add github workflow to build and test
- Add systemd service
- Create rpm and deb packages
- Add new sensor types, including:
  - Power usage
  - Fan speed
  - Max temperature
  - Memory stats
  - Labels
- Handle partial reads
- Move devices into directories
