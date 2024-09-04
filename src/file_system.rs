use std::cmp::min;
use crate::sensors::{Sensor, SensorKind};
use fuser::{FileAttr, FileType, Filesystem, KernelConfig, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, Request, FUSE_ROOT_ID};
use libc::{c_int, ENOENT};
use nvml_wrapper::Nvml;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::time::{Duration, SystemTime};
use nvml_wrapper::error::NvmlError;
use tracing::{debug, error, info, instrument, trace, warn};

type INode = u64;
type FileHandle = u64;

const TTL: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct NvSensorFs<'nvml> {
    nvml: &'nvml Nvml,
    last_ino: INode,
    last_file_handle: FileHandle,
    number_of_devices: u32,
    root_dir: FileAttr,
    files: HashMap<INode, SensorFile>,
    file_names: HashMap<OsString, INode>,
    file_content: HashMap<FileHandle, Vec<u8>>,
}

#[derive(Copy, Clone, Debug)]
struct SensorFile {
    sensor: Sensor,
    file_attr: FileAttr,
}

impl<'nvml> NvSensorFs<'nvml> {
    pub fn new(nvml: &'nvml Nvml) -> NvSensorFs {
        NvSensorFs {
            nvml,
            last_ino: FUSE_ROOT_ID,
            last_file_handle: 0,
            number_of_devices: 0,
            root_dir: create_root_dir(),
            files: HashMap::new(),
            file_names: HashMap::new(),
            file_content: HashMap::new()
        }
    }
    
    fn update_file_list(&mut self) {
        let device_count = self.nvml.device_count().unwrap_or(0);
        
        if self.number_of_devices == device_count {
            return;
        }
        
        info!("number of known devices changed ({} -> {}), refreshing file list", self.number_of_devices, device_count);
        
        self.files.clear();
        self.file_names.clear();
        
        for device_index in 0..device_count {
            self.last_ino += 1;

            // TODO iterate over all sensor types
            let (file_name, file) = create_file(self.last_ino, device_index, SensorKind::Temperature);
            self.files.insert(self.last_ino, file);
            self.file_names.insert(file_name, self.last_ino);
        }

        self.number_of_devices = device_count;
    }
}

impl Filesystem for NvSensorFs<'_> {
    #[instrument(skip(self, _req, _config))]
    fn init(&mut self, _req: &Request<'_>, _config: &mut KernelConfig) -> Result<(), c_int> {
        info!("initializing FUSE file system");
        self.update_file_list();
        Ok(())
    }

    #[instrument(skip(self))]
    fn destroy(&mut self) {
        info!("destroying FUSE file system");
    }

    #[instrument(skip(self, _req, reply))]
    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        self.update_file_list();
        
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
            trace!("returning root dir");
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
    fn open(&mut self, _req: &Request<'_>, ino: u64, _flags: i32, reply: ReplyOpen) {
        let Some(file) = self.files.get(&ino) else {
            warn!("file could not be found");
            reply.error(ENOENT);
            return;
        };

        let value = file.sensor.get_value(self.nvml);
        let value = match value {
            Ok(mut v) => {
                debug!("value: {v}");
                v.push('\n');
                v
            },
            Err(NvmlError::NotSupported) => {
                warn!("unsupported sensor type {:?}", &file.sensor);
                "N/A\n".to_string()
            }
            Err(err) => {
                warn!("error while trying to retrieve the value of sensor {:?}: {err}", &file.sensor);
                self.update_file_list();
                "N/A\n".to_string()
            }
        };
        
        let content = value.as_bytes().to_vec();
        let file_handle = self.last_file_handle + 1;
        self.file_content.insert(file_handle, content);
        self.last_file_handle = file_handle;
        
        trace!("file handle {file_handle} open");
        reply.opened(file_handle, 0);
    }
    
    #[instrument(skip(self, _req, reply))]
    fn read(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let Some(content) = self.file_content.get(&fh) else {
            error!("file content could not be found");
            reply.error(ENOENT);
            return;
        };
        
        let start_offset = min(offset as usize, content.len());
        let end_offset = min(start_offset + size as usize, content.len());

        trace!("returning value from offset {start_offset} to {end_offset}");
        reply.data(&content[offset as usize..]);
    }

    #[instrument(skip(self, _req, reply))]
    fn release(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        if self.file_content.remove(&fh).is_none() {
            warn!("trying to release file handle {fh}, but content could not be found");
        }
        trace!("file closed");
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
        self.update_file_list();
        
        if ino != FUSE_ROOT_ID {
            warn!("unknown directory ino");
            reply.error(ENOENT);
            return;
        }

        let mut file_names = self.file_names.iter().collect::<Vec<_>>();
        file_names.sort_by_key(|v| v.1);

        for (index, (file_name, ino)) in file_names.iter().enumerate().skip(offset as usize) {
            trace!("returning file {index}: {file_name:?}");
            if reply.add(**ino, (index + 1) as i64, FileType::RegularFile, file_name) {
                trace!("reply buffer filled");
                break;
            }
        }
        
        reply.ok();
    }
}

#[instrument]
fn create_file(ino: INode, device_index: u32, kind: SensorKind) -> (OsString, SensorFile) {
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
