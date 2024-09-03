use crate::sensors::{Sensor, SensorKind};
use fuser::{FileAttr, FileType, Filesystem, KernelConfig, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, Request, FUSE_ROOT_ID};
use libc::{c_int, ENOENT};
use tracing::{debug, error, info, instrument, trace, warn};
use nvml_wrapper::Nvml;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::time::{Duration, SystemTime};

const TTL: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct NvSensorFs<'nvml> {
    nvml: &'nvml Nvml,
    root_dir: FileAttr,
    files: HashMap<u64, SensorFile>,
    file_names: HashMap<OsString, u64>, // TODO readdir() expects this to only contain regular files for efficiency reasons. generify this later
}

#[derive(Copy, Clone, Debug)]
struct SensorFile {
    sensor: Sensor,
    file_attr: FileAttr,
}

impl<'nvml> NvSensorFs<'nvml> {
    pub fn new(nvml: &'nvml Nvml) -> NvSensorFs {
        let root_dir = create_root_dir();
        let mut files = HashMap::new(); // TODO add capacity
        let mut file_names = HashMap::new(); // TODO add capacity

        // TODO adding/removing sensors should happen dynamically, not in new()
        let mut last_ino = FUSE_ROOT_ID;
        for device_index in 0..nvml.device_count().unwrap_or(0) {
            last_ino += 1;

            // TODO iterate over all sensor types
            let (file_name, file) = create_file(last_ino, device_index, SensorKind::Temperature);
            files.insert(last_ino, file);
            file_names.insert(file_name, last_ino);
        }

        NvSensorFs {
            nvml,
            root_dir,
            files,
            file_names,
        }
    }
}

impl Filesystem for NvSensorFs<'_> {

    #[instrument(skip(self, _req, _config))]
    fn init(&mut self, _req: &Request<'_>, _config: &mut KernelConfig) -> Result<(), c_int> {
        info!("initializing FUSE file system");
        Ok(())
    }

    #[instrument(skip(self))]
    fn destroy(&mut self) {
        info!("destroying FUSE file system");
    }

    #[instrument(skip(self, _req, reply))]
    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent != FUSE_ROOT_ID {
            warn!("unknown parent id");
            reply.error(ENOENT);
            return;
        }

        let Some(ino) = self.file_names.get(name) else {
            warn!("unknown file name");
            reply.error(ENOENT);
            return;
        };

        let Some(file) = self.files.get(ino) else {
            error!("file could not be found for ino {ino} - this is likely a bug");
            reply.error(ENOENT);
            return;
        };

        trace!("returning file entry");
        reply.entry(&TTL, &file.file_attr, 0);
    }

    #[instrument(skip(self, _req, reply))]
    fn getattr(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyAttr) {
        if ino == FUSE_ROOT_ID {
            trace!("returning root dir {:?}", &self.root_dir);
            reply.attr(&TTL, &self.root_dir);
            return;
        }

        let Some(file) = self.files.get(&ino) else {
            warn!("file could not be found");
            reply.error(ENOENT);
            return;
        };

        trace!("returning file attr {:?}", &file.file_attr);
        reply.attr(&TTL, &file.file_attr);
    }

    #[instrument(skip(self, _req, reply))]
    fn open(&mut self, _req: &Request<'_>, _ino: u64, _flags: i32, reply: ReplyOpen) {
        reply.opened(0, 0);
    }

    // TODO getting sensor value should happen in open() once and transferred over to read() in the file handle
    // TODO read() should use the cached value in the file handle because of the possibility of partial reads
    #[instrument(skip(self, _req, reply))]
    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        _size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let Some(file) = self.files.get(&ino) else {
            warn!("file could not be found");
            reply.error(ENOENT);
            return;
        };

        let mut value = file
            .sensor
            .get_value(self.nvml)
            .unwrap_or_else(|err| {
                warn!("could not get value for sensor {:?}: {err}", &file.sensor);
                "N/A".to_string()
            });
        value.push('\n');

        trace!("returning value {:?}", &value);
        reply.data(&value.as_bytes()[offset as usize..]);
    }

    #[instrument(skip(self, _req, reply))]
    fn release(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _flags: i32, _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
        reply.ok();
    }

    #[instrument(skip(self, _req, reply))]
    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        if ino != FUSE_ROOT_ID {
            warn!("unknown directory ino");
            reply.error(ENOENT);
            return;
        }

        let mut file_names = self.file_names.iter().collect::<Vec<_>>();
        file_names.sort_by_key(|v| v.1);

        for (index, (file_name, ino)) in file_names.iter().enumerate().skip(offset as usize) {
            if reply.add(**ino, (index + 1) as i64, FileType::RegularFile, file_name) {
                break;
            }
        }

        trace!("returning files {:?}", &reply);
        reply.ok();
    }
}

#[instrument]
fn create_file(ino: u64, device_index: u32, kind: SensorKind) -> (OsString, SensorFile) {
    let now = SystemTime::now();

    let file_attr = FileAttr {
        ino,
        size: 4096,
        blocks: 0,
        atime: now,
        mtime: now,
        ctime: now,
        crtime: now,
        kind: FileType::RegularFile,
        perm: 0o444,
        nlink: 1,
        uid: 0,
        gid: 0,
        rdev: 0,
        blksize: 4096,
        flags: 0,
    };

    let file_name = match kind {
        SensorKind::Temperature => format!("temp{}_input", device_index + 1),
    };

    let sensor_file = SensorFile {
        file_attr,
        sensor: Sensor::new(device_index, kind),
    };

    debug!("file created: {file_name}");
    (file_name.into(), sensor_file)
}

fn create_root_dir() -> FileAttr {
    let now = SystemTime::now();
    FileAttr {
        ino: FUSE_ROOT_ID,
        size: 0,
        blocks: 0,
        atime: now,
        mtime: now,
        ctime: now,
        crtime: now,
        kind: FileType::Directory,
        perm: 0o755,
        nlink: 1,
        uid: 0,
        gid: 0,
        rdev: 0,
        blksize: 4096,
        flags: 0,
    }
}
