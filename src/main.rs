use std::{
    env,
    fs::File,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    sync::{Arc, Mutex},
};

use clap::Parser;
use cmd::CmdArgs;
use console::{style, Emoji};
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use io::{get_reative_path, read_file_metadata, FileReader, SourceFile};
use ssh::create_session;
use ssh2::Session;

use self::{ssh::check_folders_exists, util::get_leaf_folders};

mod cmd;
mod io;
mod ssh;
mod util;

static TRUCK: Emoji<'_, '_> = Emoji("🚚  ", "");
static LOOKING_GLASS: Emoji<'_, '_> = Emoji("🔍  ", "");
static CONNECTING: Emoji<'_, '_> = Emoji("🔗  ", "");
static UPLOADING: Emoji<'_, '_> = Emoji("📤  ", "");
static FOLDER: Emoji<'_, '_> = Emoji("📁  ", "");

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
    let sess = create_session(source, destination, ssh_conn)?;
    // Start SCP session
    scp_upload(&sess, source, destination)?;

    Ok(())
}

fn scp_upload(
    sess: &Session,
    source: &str,
    destination: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let spinner_style = ProgressStyle::with_template("{.bold.dim} {spinner} {wide_msg}")
        .unwrap()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

    let multi_progress = MultiProgress::new();

    let path = Path::new(source);

    if path.is_dir() {
        let file_reader = FileReader::new(source.to_string());
        let mut file_data = vec![];
        let mut folders = vec![];

        println!(
            "{} {} Collecting files...",
            style("[2/5]").bold().dim(),
            LOOKING_GLASS,
        );

        read_file_metadata(
            file_reader,
            source,
            &mut file_data,
            &mut folders,
            destination,
        );

        println!(
            "{} {} Check {} Folder exists...",
            style("[3/5]").bold().dim(),
            TRUCK,
            folders.len()
        );
        let folders_to_create = check_folders_exists(sess, &folders, destination)?;
        let is_root_does_not_exists = folders_to_create.len() == folders.len() + 1;
        let leaf_folders = get_leaf_folders(folders_to_create.iter().map(|f| f.as_str()).collect());
        println!(
            "{} {} Creating {} Folders...",
            style("[4/5]").bold().dim(),
            FOLDER,
            leaf_folders.len()
        );
        create_folders(sess, &leaf_folders)?;

        let total_size: u64 = file_data.iter().map(|file_data| file_data.size).sum();
        let total_size_pb = Arc::new(create_total_progressbar(&multi_progress, total_size));
        let current_file = create_progress_bars(&multi_progress);
        current_file.set_style(spinner_style);
        let current_file = Arc::new(current_file);

        println!("{} {} Copying...", style("[5/5]").bold().dim(), UPLOADING);

        let mut handlers = vec![];
        let file_data = Arc::new(Mutex::new(file_data));
        for _ in 0..3 {
            let file_data = file_data.clone();
            let sess = sess.clone();
            let current_file = current_file.clone();
            let total_size_pb = total_size_pb.clone();
            let destination = destination.to_string();
            let source = source.to_string();

            let handle = std::thread::spawn(move || {
                loop {
                    let mut file_data = file_data.lock().unwrap();
                    let file = match file_data.pop() {
                        Some(file) => file,
                        None => break,
                    };
                    drop(file_data);
                    let remote_file = format!(
                        "{}/{}",
                        destination,
                        get_reative_path(&file.file_path, &source)
                    );
                    let size = file.size;
                    current_file.set_message(format!("Copying file: {:?}", remote_file));
                    // Ignore file size check if the root folder desn't exists, that means we should copy all files
                    if is_root_does_not_exists {
                        scp_upload_file(&sess, file, &remote_file).unwrap();
                        total_size_pb.inc(size);
                    } else {
                        let remote_size = get_remote_file_size(&sess, &remote_file).unwrap_or(0);
                        if remote_size != size {
                            scp_upload_file(&sess, file, &remote_file).unwrap();
                            total_size_pb.inc(size);
                        } else {
                            total_size_pb.inc(size);
                        }
                    }
                }
            });
            handlers.push(handle);
        }

        for handler in handlers {
            handler.join().unwrap();
        }

        // for file in file_data {
        //     let remote_file = format!(
        //         "{}/{}",
        //         destination,
        //         get_reative_path(&file.file_path, source)
        //     );
        //     let size = file.size;
        //     current_file.set_message(format!("Copying file: {:?}", remote_file));
        //     // Ignore file size check if the root folder desn't exists, that means we should copy all files
        //     if is_root_does_not_exists {
        //         scp_upload_file(sess, file, &remote_file)?;
        //         total_size_pb.inc(size);
        //     } else {
        //         let remote_size = get_remote_file_size(sess, &remote_file).unwrap_or(0);
        //         if remote_size != size {
        //             scp_upload_file(sess, file, &remote_file)?;
        //             total_size_pb.inc(size);
        //         } else {
        //             total_size_pb.inc(size);
        //         }
        //     }
        // }
        // total_size_pb.finish();
    } else {
        //scp_upload_file(sess, path, destination)?;
    }

    Ok(())
}

fn create_folders(sess: &Session, folders: &Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
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

fn scp_upload_file(
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

pub fn create_total_progressbar(multi_progress: &MultiProgress, total_files: u64) -> ProgressBar {
    let total_size_pb = multi_progress.add(ProgressBar::new(total_files));
    let sty = ProgressStyle::with_template(
        "[{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({elapsed_precise})",
    )
    .unwrap()
    .with_key(
        "eta",
        |state: &ProgressState, w: &mut dyn std::fmt::Write| {
            write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
        },
    )
    .progress_chars("#>-");
    total_size_pb.set_style(sty);
    total_size_pb
}

pub fn create_progress_bars(multi_progress: &MultiProgress) -> ProgressBar {
    multi_progress.add(ProgressBar::new_spinner())
}
