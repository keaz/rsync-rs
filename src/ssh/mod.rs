use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

use console::style;
use ssh2::Session;

use crate::io::SourceFile;
use crate::CONNECTING;

pub fn create_session(
    source: &str,
    destination: &str,
    ssh_conn: &str,
) -> Result<Session, Box<dyn std::error::Error>> {
    // Split SSH connection string into user and host
    let parts: Vec<&str> = ssh_conn.split('@').collect();
    if parts.len() != 2 {
        return Err("Invalid SSH connection format. Expected user@host.".into());
    }
    let user = parts[0];
    let host = parts[1];

    println!(
        "{} {} Connectig to server {} ...",
        style("[1/5]").bold().dim(),
        CONNECTING,
        host
    );

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

    Ok(sess)
}

pub fn check_folders_exists(
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
        let cmd = format!("stat {}", folder);
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

pub fn create_folders(
    sess: &Session,
    folders: &Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    for folder in folders {
        let mut channel = sess.channel_session()?;
        let cmd = format!("mkdir -p {}", folder);
        channel.exec(&cmd)?;

        let mut result = String::new();
        channel.read_to_string(&mut result)?;
        channel.wait_close()?;
    }

    Ok(())
}

pub fn scp_upload_file(
    sess: &Session,
    source: SourceFile,
    destination: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_size = source.size;

    // Open SCP channel
    let mut remote_file = sess.scp_send(Path::new(destination), 0o644, file_size, None)?;
    let five_mb = 5 * 1024 * 1024;
    if file_size > five_mb {
        let mut buffer = vec![0; five_mb as usize]; // 5MB buffer
        let mut file = File::open(source.file_path)?;

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            remote_file.write_all(&buffer[..n])?;
        }
        return Ok(());
    } else {
        let mut buffer = vec![0; file_size as usize];
        let mut file = File::open(source.file_path)?;

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            remote_file.write_all(&buffer[..n])?;
        }
    }

    Ok(())
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
