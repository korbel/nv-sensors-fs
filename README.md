# nv-sensor-fs
**[WORK IN PROGRESS]**

Creates a read-only FUSE filesystem with direct access to the NVIDIA dGPU sensors.

## Dependencies

- Install fuse3 (most systems have it already installed)
- Proprietary or the open kernel module NVIDIA driver must be loaded.

## Build

Requires the RUST toolchain and `fuse3-devel` to build.

```shell
cargo build --release
```

## Options

```shell
$ nv-sensors-fs --help

Creates a read-only FUSE filesystem with direct access to the NVIDIA dGPU sensors

Usage: nv-sensors-fs [OPTIONS]

Options:
  -m, --mount-point <MOUNT_POINT>  The mount point where the filesystem will be mounted to [default: /var/lib/nv_sensor_fs]
  -h, --help                       Print help
  -V, --version                    Print version
```

## Example

```shell
$ cat /var/lib/nv_sensor_fs/temp1_input 
45000
```

## TODO

- Add github workflow to build and test
- Add systemd service
- Create rpm and deb packages
- Add debug and info logs
- Add new sensor types, including:
  - Power usage
  - Fan speed
  - Max temperature
  - Memory stats
  - Labels
- Add proper error handling
- Handle partial reads
