use std::{
    fs::{self, File, Metadata},
    path::{Path, PathBuf},
    time::SystemTime,
};

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
}

impl FileReader {
    pub fn new(path: String) -> Self {
        let path_buf = PathBuf::new().join(&path);
        if !path_buf.exists() {
            println!("File does not exists {:?}", path);
        }
        FileReader {
            file: File::open(path_buf).unwrap(),
        }
    }
}

impl FileReader {
    pub fn is_folder(&self) -> bool {
        self.file.metadata().unwrap().is_dir()
    }
}

pub fn read_file_metadata(
    file_reader: FileReader,
    source: &str,
    file_data: &mut Vec<SourceFile>,
    folders: &mut Vec<String>,
    destination: &str,
) {
    if file_reader.is_folder() {
        let path = PathBuf::from(source);
        let reads = fs::read_dir(path.clone());
        match reads {
            Err(er) => {
                eprintln!("Error reading folder  path {}", er);
            }
            Ok(entries) => {
                walk_dir(entries, file_data, folders, source, destination);
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

fn walk_dir(
    mut entries: fs::ReadDir,
    file_data: &mut Vec<SourceFile>,
    folders: &mut Vec<String>,
    source: &str,
    destination: &str,
) {
    while let Some(Ok(dir_entry)) = entries.next() {
        let path = dir_entry.path();
        if path.is_dir() {
            let reads = fs::read_dir(path.clone());
            match reads {
                Err(er) => {
                    eprintln!("Error reading folder  path {}", er);
                }
                Ok(entries) => {
                    let remote_file =
                        format!("{}/{}", destination, get_reative_path(&path, source));
                    folders.push(remote_file);
                    walk_dir(entries, file_data, folders, source, destination);
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

pub fn get_reative_path(file: &Path, source: &str) -> String {
    let source = PathBuf::from(source);
    let relative_path = file.strip_prefix(source).unwrap();
    let relative_path = format!("{:?}", relative_path);
    relative_path.replace('\"', "")
}
