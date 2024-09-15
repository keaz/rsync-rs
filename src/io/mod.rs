use std::{
    fs::{self, File, Metadata},
    io::{Read, Seek, SeekFrom, Write},
    ops::ControlFlow,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use filetime::{set_file_mtime, FileTime};

#[derive(Debug)]
struct Folder {
    path_buff: PathBuf,
}

#[derive(Clone)]
pub struct SourceFile {
    pub file_path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug)]
pub enum FileError {
    CannotCreate(String),
    FileNotCreate(String),
}

pub struct FileReader {
    file: File,
    file_name: String,
}

impl FileReader {
    pub fn new(path: String) -> Self {
        let path_buf = PathBuf::new().join(&path);
        if !path_buf.exists() {
            println!("File does not exists {:?}", path);
        }
        let cl = path_buf.clone();
        let file_name = cl.file_name().unwrap().to_str().unwrap();
        FileReader {
            file: File::open(path_buf).unwrap(),
            file_name: String::from(file_name),
        }
    }

    pub fn from(path_buf: PathBuf) -> Self {
        if !path_buf.exists() {
            println!("File does not exists {:?}", path_buf);
        }
        let cl = path_buf.clone();
        let file_name = cl.file_name().unwrap().to_str().unwrap();
        let file = File::open(path_buf);
        if let Err(er) = file {
            eprintln!("Error opening file {}", er);
            panic!()
        }
        FileReader {
            file: file.unwrap(),
            file_name: String::from(file_name),
        }
    }
}

impl FileReader {
    pub fn name(&self) -> String {
        self.file_name.clone()
    }

    pub fn size(&self) -> u64 {
        self.file.metadata().unwrap().len()
    }

    pub fn is_folder(&self) -> bool {
        self.file.metadata().unwrap().is_dir()
    }

    pub fn read_random(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, FileError> {
        self.file.seek(SeekFrom::Start(offset)).unwrap();
        let read_data = self.file.read(buf).unwrap();

        Ok(read_data)
    }
}

pub fn read_file_metadata(
    file_reader: FileReader,
    source: &str,
    file_data: &mut Vec<SourceFile>,
    folders: &mut Vec<String>,
) {
    if file_reader.is_folder() {
        let path = PathBuf::from(source);
        let reads = fs::read_dir(path.clone());
        match reads {
            Err(er) => {
                eprintln!("Error reading folder  path {}", er);
                return;
            }
            Ok(entries) => {
                walk_dir(entries, file_data, folders);
            }
        }
    } else {
        let path_buff = PathBuf::new().join(source);
        let metadata = fs::metadata(&path_buff).unwrap();
        let modified = match metadata.modified() {
            Ok(modified) => Some(modified),
            Err(_) => None,
        };

        file_data.push(SourceFile {
            file_path: path_buff,
            size: metadata.len(),
            modified,
        });
    }
}

fn walk_dir(mut entries: fs::ReadDir, file_data: &mut Vec<SourceFile>, folders: &mut Vec<String>) {
    while let Some(Ok(dir_entry)) = entries.next() {
        let path = dir_entry.path();
        if path.is_dir() {
            let reads = fs::read_dir(path.clone());
            match reads {
                Err(er) => {
                    eprintln!("Error reading folder  path {}", er);
                }
                Ok(entries) => {
                    folders.push(path.to_str().unwrap().to_string());
                    walk_dir(entries, file_data, folders)
                }
            }
        } else if path.is_file() {
            match fs::metadata(&path) {
                Err(er) => {
                    eprintln!("Error reading metadata {:?} error {:?}", path, er);
                }
                Ok(metadata) => {
                    extract_detail_and_walk(metadata, path, file_data);
                }
            }
        }
    }
}

fn extract_detail_and_walk(metadata: Metadata, path: PathBuf, file_data: &mut Vec<SourceFile>) {
    if metadata.is_file() {
        let modified = match metadata.modified() {
            Ok(modified) => Some(modified),
            Err(_) => None,
        };
        file_data.push(SourceFile {
            file_path: path,
            size: metadata.len(),
            modified,
        });
    }
}
