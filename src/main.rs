mod file_system;
mod sensors;

use anyhow::{bail, Context};
use clap::Parser;
use file_system::NvSensorFs;
use fuser::{mount2, MountOption};
use is_root::is_root;
use nvml_wrapper::Nvml;
use std::fs;
use std::path::PathBuf;

/// Creates a read-only FUSE file system with direct access to the NVIDIA dGPU sensors
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The mount point where the file system will be mounted to
    #[arg(short, long, default_value=get_default_mount_point().into_os_string())]
    mount_point: PathBuf,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    if !is_root() {
        bail!("This command has to be run with superuser privileges.");
    }

    if !args.mount_point.is_dir() {
        if args.mount_point == get_default_mount_point() {
            fs::create_dir(&args.mount_point).context("Failed to create default mount point")?;
        } else {
            bail!("The specified mount directory does not exist");
        }
    }

    let nvml = Nvml::init().context("Failed to initialize NVIDIA Management Library")?;

    let file_system = NvSensorFs::new(&nvml);
    let mount_point = args.mount_point;
    let options = vec![
        MountOption::FSName("nv-sensors-fs".to_string()),
        MountOption::RO,
        MountOption::AllowOther,
        MountOption::AutoUnmount,
    ];

    mount2(file_system, mount_point, &options).context("Failed to mount the file system")?;

    Ok(())
}

fn get_default_mount_point() -> PathBuf {
    PathBuf::from("/var/lib/nv-sensors-fs")
}
