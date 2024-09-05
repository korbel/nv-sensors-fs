# nv-sensors-fs

**[WORK IN PROGRESS]**

Creates a read-only FUSE file system with direct access to the NVIDIA dGPU sensors.

## Requirements

- Proprietary or the open kernel module NVIDIA driver must be loaded.
- Enable the service after install to start it automatically:
  ```shell
    sudo systemctl enable --now nv-sensors-fs.service
  ```

## Development

You will need the rust toolchain to have installed. If you are new to Rust, follow these instructions: <https://rustup.rs/>

### Dependencies

#### Fedora

```shell
sudo dnf install fuse3 fuse3-devel pkgconfig gcc
```

#### Ubuntu/Debian

```shell
sudo apt-get install fuse3 libfuse3-dev pkg-config build-essential
```

### Build

```shell
cargo build --release
```

### Package

#### rpm

```shell
cargo install cargo-generate-rpm
cargo build --release
strip -s target/release/nv-sensors-fs
cargo generate-rpm
```

#### deb

```shell
cargo install cargo-deb
cargo build --release
cargo deb
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
  -m, --mount-point <MOUNT_POINT>  The mount point where the file system will be mounted to [default: /var/lib/nv-sensors-fs]
  -h, --help                       Print help
  -V, --version                    Print version
```

## Example

```shell
$ find /var/lib/nv-sensors-fs/0/ -type f | xargs -n1 sh -c 'echo $0 - `cat $0`'
/var/lib/nv-sensors-fs/0/bar1_memory_free - 263913472
/var/lib/nv-sensors-fs/0/bar1_memory_total - 268435456
/var/lib/nv-sensors-fs/0/bar1_memory_used - 4521984
/var/lib/nv-sensors-fs/0/clock_graphics - 300
/var/lib/nv-sensors-fs/0/clock_memory - 405
/var/lib/nv-sensors-fs/0/clock_streaming_multiprocessor - 300
/var/lib/nv-sensors-fs/0/clock_video - 540
/var/lib/nv-sensors-fs/0/decoder_utilization - 0
/var/lib/nv-sensors-fs/0/decoder_utilization_sampling_period - 1000000
/var/lib/nv-sensors-fs/0/encoder_utilization - 0
/var/lib/nv-sensors-fs/0/encoder_utilization_sampling_period - 1000000
/var/lib/nv-sensors-fs/0/enforced_power_limit - 80000000
/var/lib/nv-sensors-fs/0/memory_free - 6027083776
/var/lib/nv-sensors-fs/0/memory_total - 6442450944
/var/lib/nv-sensors-fs/0/memory_used - 415367168
/var/lib/nv-sensors-fs/0/name - NVIDIA GeForce GTX 1660 Ti
/var/lib/nv-sensors-fs/0/pcie_throughput_receive - 0
/var/lib/nv-sensors-fs/0/pcie_throughput_send - 0
/var/lib/nv-sensors-fs/0/performance_state - 8
/var/lib/nv-sensors-fs/0/power_source - 0
/var/lib/nv-sensors-fs/0/power_usage - 6314000
/var/lib/nv-sensors-fs/0/temperature - 45000
/var/lib/nv-sensors-fs/0/temperature_threshold - 102000
/var/lib/nv-sensors-fs/0/total_energy_consumption - 162250719
/var/lib/nv-sensors-fs/0/utilization_rate_gpu - 0
/var/lib/nv-sensors-fs/0/utilization_rate_memory - 0
```

## TODO

- Add github workflow to build and test
- Add test coverage
