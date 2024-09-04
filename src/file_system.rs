use crate::sensors::{Sensor, SensorKind};
use fuser::{
    FileAttr, FileType, Filesystem, KernelConfig, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, Request, FUSE_ROOT_ID,
};
use libc::{c_int, ENOENT};
use nvml_wrapper::error::NvmlError;
use nvml_wrapper::Nvml;
use std::cmp::min;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::time::{Duration, SystemTime};
use tracing::{debug, error, info, instrument, trace, warn};

type INodeId = u64;
type FileHandle = u64;

const TTL: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct NvSensorFs<'nvml> {
    nvml: &'nvml Nvml,
    last_inode_id: INodeId,
    last_file_handle: FileHandle,
    device_count: u32,
    inodes: HashMap<INodeId, INode>,
    name_lookup: HashMap<(INodeId, OsString), INodeId>, // parent inode id + name -> inode
    file_content: HashMap<FileHandle, Vec<u8>>,
}

#[derive(Clone, Debug)]
struct INode {
    name: String,
    attr: FileAttr,
    kind: INodeKind,
}

#[derive(Clone, Debug)]
enum INodeKind {
    SensorFile(Sensor),
    Directory(Vec<INodeId>),
}

impl<'nvml> NvSensorFs<'nvml> {
    pub fn new(nvml: &'nvml Nvml) -> NvSensorFs {
        NvSensorFs {
            nvml,
            last_inode_id: FUSE_ROOT_ID,
            last_file_handle: 0,
            device_count: 0,
            inodes: HashMap::new(),
            name_lookup: HashMap::new(),
            file_content: HashMap::new(),
        }
    }

    fn update_sensors(&mut self) {
        let current_device_count = self.device_count;
        let new_device_count = self.nvml.device_count().unwrap_or(0);

        if current_device_count == new_device_count {
            return;
        }

        info!(
            "number of known devices changed ({} -> {}), rebuilding sensor list",
            current_device_count, new_device_count
        );

        self.inodes.clear();
        self.name_lookup.clear();

        let mut device_nodes = Vec::new();

        for device_index in 0..new_device_count {
            // reserve device inode id
            self.last_inode_id += 1;
            let device_node_id = self.last_inode_id;
            let mut device_sensors = Vec::new();
            device_nodes.push(device_node_id);

            // create sensors
            self.last_inode_id += 1;
            let sensor_node_id = self.last_inode_id;
            let sensor_file = create_sensor_file(
                sensor_node_id,
                device_index,
                SensorKind::Temperature,
            );
            self.name_lookup.insert(
                (device_node_id, sensor_file.name.clone().into()),
                sensor_node_id,
            );
            self.inodes.insert(sensor_node_id, sensor_file);
            device_sensors.push(sensor_node_id);

            // create device
            let device_dir = create_device_dir(device_node_id, device_index, device_sensors);
            self.name_lookup.insert(
                (FUSE_ROOT_ID, device_dir.name.clone().into()),
                device_node_id,
            );
            self.inodes.insert(device_node_id, device_dir);
        }

        // add root directory
        self.inodes.insert(
            FUSE_ROOT_ID,
            INode {
                name: "".to_string(),
                attr: create_dir_attr(FUSE_ROOT_ID),
                kind: INodeKind::Directory(device_nodes),
            },
        );

        self.device_count = new_device_count;
    }
}

impl Filesystem for NvSensorFs<'_> {
    #[instrument(skip(self, _req, _config))]
    fn init(&mut self, _req: &Request<'_>, _config: &mut KernelConfig) -> Result<(), c_int> {
        info!("initializing FUSE file system");
        self.update_sensors();
        Ok(())
    }

    #[instrument(skip(self))]
    fn destroy(&mut self) {
        info!("destroying FUSE file system");
    }

    #[instrument(skip(self, _req, reply))]
    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        self.update_sensors();

        let Some(ino) = self.name_lookup.get(&(parent, name.to_os_string())) else {
            warn!("file does not exist");
            reply.error(ENOENT);
            return;
        };

        let Some(node) = self.inodes.get(ino) else {
            error!("file could not be found for ino {ino} - this is likely a bug");
            reply.error(ENOENT);
            return;
        };

        trace!("returning file entry");
        reply.entry(&TTL, &node.attr, 0);
    }

    #[instrument(skip(self, _req, reply))]
    fn getattr(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyAttr) {
        let Some(node) = self.inodes.get(&ino) else {
            warn!("inode could not be found");
            reply.error(ENOENT);
            return;
        };

        trace!("returning attr");
        reply.attr(&TTL, &node.attr);
    }

    #[instrument(skip(self, _req, reply))]
    fn open(&mut self, _req: &Request<'_>, ino: u64, _flags: i32, reply: ReplyOpen) {
        let Some(node) = self.inodes.get(&ino) else {
            warn!("node could not be found");
            reply.error(ENOENT);
            return;
        };

        let INodeKind::SensorFile(sensor) = node.kind else {
            error!("trying to open a file that's not of a sensor type - this is likely a bug");
            reply.error(ENOENT);
            return;
        };

        let value = sensor.get_value(self.nvml);
        let value = match value {
            Ok(mut v) => {
                debug!("value: {v}");
                v.push('\n');
                v
            }
            Err(NvmlError::NotSupported) => {
                warn!("unsupported sensor type {:?}", &sensor);
                "N/A\n".to_string()
            }
            Err(err) => {
                warn!(
                    "error while trying to retrieve the value of sensor {:?}: {err}",
                    &sensor
                );
                self.update_sensors();
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
        self.update_sensors();

        let Some(node) = self.inodes.get(&ino) else {
            warn!("unknown ino");
            reply.error(ENOENT);
            return;
        };

        let INodeKind::Directory(children) = &node.kind else {
            error!("is not a directory - likely a bug");
            reply.error(ENOENT);
            return;
        };

        for (index, ino) in children.iter().enumerate().skip(offset as usize) {
            let Some(child_node) = self.inodes.get(ino) else {
                error!("unknown child node {ino} - this is likely a bug");
                reply.error(ENOENT);
                return;
            };

            trace!("returning file {index}: {:?}", &node.name);
            let file_name: OsString = child_node.name.clone().into();

            if reply.add(*ino, (index + 1) as i64, FileType::RegularFile, file_name) {
                trace!("reply buffer filled");
                break;
            }
        }

        reply.ok();
    }
}

#[instrument]
fn create_sensor_file(ino: INodeId, device_index: u32, kind: SensorKind) -> INode {
    let file_attr = create_file_attr(ino);

    let file_name = match kind {
        SensorKind::Temperature => "temperature".to_string(),
    };

    let sensor = Sensor::new(device_index, kind);

    debug!("file created: {file_name}");
    INode {
        name: file_name,
        attr: file_attr,
        kind: INodeKind::SensorFile(sensor),
    }
}

#[instrument]
fn create_device_dir(ino: INodeId, device_index: u32, sensors: Vec<INodeId>) -> INode {
    let dir_attr = create_dir_attr(ino);

    debug!("device directory created: {device_index}");
    INode {
        name: device_index.to_string(),
        attr: dir_attr,
        kind: INodeKind::Directory(sensors),
    }
}

fn create_dir_attr(ino: INodeId) -> FileAttr {
    let now = SystemTime::now();
    FileAttr {
        ino,
        size: 0,
        blocks: 0,
        atime: now,
        mtime: now,
        ctime: now,
        crtime: now,
        kind: FileType::Directory,
        perm: 0o755,
        nlink: 2,
        uid: 0,
        gid: 0,
        rdev: 0,
        blksize: 4096,
        flags: 0,
    }
}

fn create_file_attr(ino: INodeId) -> FileAttr {
    let now = SystemTime::now();
    FileAttr {
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
    }
}
