use std::{
    env,
    fs::File,
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
};

use clap::Parser;
use cmd::CmdArgs;
use io::{read_file_metadata, FileReader};
use ssh2::{MethodType, Session};

mod cmd;
mod io;

fn main() {
    let cmds = CmdArgs::parse_from(env::args_os());

    if let Err(e) = copy_files_over_ssh(&cmds.source, &cmds.destination, &cmds.ssh) {
        eprintln!("Error copying files: {}", e);
    }
}

fn copy_files_over_ssh(
    source: &str,
    destination: &str,
    ssh_conn: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Split SSH connection string into user and host
    let parts: Vec<&str> = ssh_conn.split('@').collect();
    if parts.len() != 2 {
        return Err("Invalid SSH connection format. Expected user@host.".into());
    }
    let user = parts[0];
    let host = parts[1];

    // Open TCP connection to SSH server
    let tcp = TcpStream::connect(format!("{}:22", host))?;
    let mut sess = Session::new().unwrap();

    sess.set_tcp_stream(tcp);
    sess.handshake().unwrap();

    sess.userauth_agent(user)?;

    // Check if authentication succeeded
    if !sess.authenticated() {
        return Err("SSH authentication failed.".into());
    }

    // Start SCP session
    scp_upload(&sess, source, destination)?;

    Ok(())
}

fn scp_upload(
    sess: &Session,
    source: &str,
    destination: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(source);

    if path.is_dir() {
        let file_reader = FileReader::new(source.to_string());
        let mut file_data = vec![];
        let mut folders = vec![];
        read_file_metadata(file_reader, source, &mut file_data, &mut folders);

        let folders_to_create = check_folders_exists(sess, &folders, destination)?;
        for folder in folders_to_create.clone() {
            println!("Creating folder: {}", folder);
        }
        create_folders(sess, &folders_to_create, destination)?;

        for file in file_data {
            let remote_file = format!(
                "{}/{}",
                destination,
                get_reative_path(&file.file_path, source)
            );
            let remote_size = get_remote_file_size(sess, &remote_file).unwrap_or(0);
            if remote_size != file.size {
                println!("Copying file: {}", remote_file);
                scp_upload_file(sess, &file.file_path, &remote_file)?;
            }
        }
    } else {
        scp_upload_file(sess, path, destination)?;
    }

    Ok(())
}

fn check_folders_exists(
    sess: &Session,
    folders: &Vec<String>,
    destination: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut folders_to_create = vec![];

    let mut channel = sess.channel_session()?;
    let cmd = format!("stat {}", destination);
    channel.exec(&cmd)?;

    let mut result = String::new();
    channel.read_to_string(&mut result)?;
    channel.wait_close()?;

    if result.is_empty() {
        folders_to_create.push(destination.to_string());
        folders_to_create.append(&mut folders.clone());
        return Ok(folders_to_create);
    }

    for folder in folders {
        let mut channel = sess.channel_session()?;
        let cmd = format!("stat {}/{}", destination, folder);
        channel.exec(&cmd)?;

        let mut result = String::new();
        channel.read_to_string(&mut result)?;
        channel.wait_close()?;

        if result.is_empty() {
            folders_to_create.push(folder.clone());
        }
    }

    Ok(folders_to_create)
}

fn create_folders(
    sess: &Session,
    folders: &Vec<String>,
    destination: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for folder in folders {
        let mut channel = sess.channel_session()?;
        let cmd = format!("mkdir -p {}/{}", destination, folder);
        channel.exec(&cmd)?;

        let mut result = String::new();
        channel.read_to_string(&mut result)?;
        channel.wait_close()?;
    }

    Ok(())
}

fn scp_upload_file(
    sess: &Session,
    source: &Path,
    destination: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(source)?;
    let file_size = file.metadata()?.len();

    // Open SCP channel
    let mut remote_file = sess.scp_send(Path::new(destination), 0o644, file_size, None)?;
    let mut buffer = vec![0; 1024 * 64]; // 64KB buffer

    // Transfer file in chunks
    let mut file = File::open(source)?;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        remote_file.write_all(&buffer[..n])?;
    }

    Ok(())
}

pub fn get_reative_path(file: &PathBuf, source: &str) -> String {
    let source = PathBuf::from(source);
    let relative_path = file.strip_prefix(source).unwrap();
    let relative_path = format!("{:?}", relative_path);
    relative_path.replace('\"', "")
}

fn get_remote_file_size(
    sess: &Session,
    remote_path: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    let mut channel = sess.channel_session()?;
    let cmd = format!("stat -c%s {}", remote_path);
    channel.exec(&cmd)?;

    let mut result = String::new();
    channel.read_to_string(&mut result)?;
    channel.wait_close()?;

    // Parse the file size from the result
    let remote_size: u64 = result.trim().parse()?;
    Ok(remote_size)
}
