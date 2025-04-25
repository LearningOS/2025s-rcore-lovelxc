use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};
/// Virtual filesystem layer over easy-fs
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }
    /// Call a function over a disk inode to read it
    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }
    /// Call a function over a disk inode to modify it
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }
    /// Find inode under a disk inode by name
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(dirent.inode_id() as u32);
            }
        }
        None
    }
    /// Find inode under current inode by name
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }
    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }
    /// Create inode under current inode by name
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.read_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block
        let new_inode_id = fs.alloc_inode();
        // initialize inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_disk_inode(|disk_inode| {
            // append file in the dirent
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, disk_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(name, new_inode_id);
            disk_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
        // release efs lock automatically by compiler
    }
    /// List inodes under current inode
    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }
    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }
    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }
    /// Clear the data in current inode
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }
    /// Link a file to current inode
    pub fn link(&self, old_path: &str, new_path: &str) -> bool {
        let mut fs = self.fs.lock();
        // 先获取旧的目录项内容
        if let Some(old_inode) =
            self.read_disk_inode(|disk_inode| self.find_inode_id(old_path, disk_inode))
        {
            // 一开始是创建了一个新的inode，但在写unlink的时候，发现不用
            // 在目录项那边新加一个就行
            let (old_inode_block_id, old_inode_block_offset) = fs.get_disk_inode_pos(old_inode);
            get_block_cache(old_inode_block_id as usize, Arc::clone(&self.block_device))
                .lock()
                .modify(old_inode_block_offset, |old_inode: &mut DiskInode| {
                    // 增加一个硬链接数
                    old_inode.links_count += 1;
                });
            // 读取当前目录项的磁盘索引节点
            self.modify_disk_inode(|disk_inode| {
                // 先增加目录项的大小
                let file_count = (disk_inode.size as usize) / DIRENT_SZ;
                let new_size = (file_count + 1) * DIRENT_SZ;
                // 增加目录项的大小
                self.increase_size(new_size as u32, disk_inode, &mut fs);
                let new_dirent = DirEntry::new(new_path, old_inode);
                // 写入新的目录项
                disk_inode.write_at(
                    file_count * DIRENT_SZ,
                    new_dirent.as_bytes(),
                    &self.block_device,
                );
            });
            block_cache_sync_all();
            return true;
        } else {
            return false;
        }
    }
    /// Unlink a file to current inode
    pub fn unlink(&self, path: &str) -> bool {
        let mut fs = self.fs.lock();
        // 先获取旧的目录项内容
        if let Some(inode) = self.read_disk_inode(|disk_inode| self.find_inode_id(path, disk_inode))
        {
            let (block_id, block_offset) = fs.get_disk_inode_pos(inode);
            get_block_cache(block_id as usize, Arc::clone(&self.block_device))
                .lock()
                .modify(block_offset, |disk_inode: &mut DiskInode| {
                    // clear the data block if it's last link
                    if disk_inode.links_count == 1 {
                        // 感觉性能很差。。，因为还要find，完全没必要其实hh
                        self.find(path).unwrap().clear();
                        return;
                    }
                    disk_inode.links_count -= 1;
                });
            // 删掉这个目录项，并用最后一个目录项来填充他，这样就不用全部移动了
            self.modify_disk_inode(|disk_inode| {
                // delete file in the dirent
                let file_count = (disk_inode.size as usize) / DIRENT_SZ;
                let mut dirent = DirEntry::empty();
                let idx = (0..file_count).find(|i| {
                    assert_eq!(
                        disk_inode.read_at(
                            DIRENT_SZ * i,
                            dirent.as_bytes_mut(),
                            &self.block_device,
                        ),
                        DIRENT_SZ,
                    );
                    dirent.name() == path
                });
                // 肯定能找到，找不到说明磁盘盘有问题了
                assert!(idx.is_some());
                let idx = idx.unwrap();
                // 读取最后一个目录项
                disk_inode.read_at(
                    DIRENT_SZ * (file_count - 1),
                    dirent.as_bytes_mut(),
                    &self.block_device,
                );
                // 写到要删除的地方
                disk_inode.write_at(DIRENT_SZ * idx, dirent.as_bytes(), &self.block_device);
                disk_inode.size -= DIRENT_SZ as u32;
            });
            block_cache_sync_all();
            return true;
        } else {
            return false;
        }
    }
    /// get stat of current inode
    pub fn get_stat(&self) -> (u64, u32, u32) {
        let fs = self.fs.lock();
        let inode_id = fs.get_inode_id(self.block_id, self.block_offset);
        self.read_disk_inode(|disk_inode| {
            let mut inode_type: u32 = 0;
            if disk_inode.is_dir() {
                inode_type = 1;
            } else if disk_inode.is_file() {
                inode_type = 2;
            }
            (inode_id as u64, inode_type, disk_inode.links_count)
        })
    }
}
