mod monitor;
mod notepad;

use std::fs;
use std::env;
use std::io::{BufRead, BufReader,Read};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::path::PathBuf;
use libc::{flock, LOCK_EX, LOCK_NB, mkfifo, dup2};
use serde::Deserialize;
use toml;
use kitty_rc::{Kitty,LaunchCommand};
use tokio::runtime::Runtime;

#[derive(Deserialize)]
pub struct Config {
    pub bottom: u32,
    pub left: u32,
}

fn lock() -> fs::File {
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open("/tmp/kitbar.lock")
        .unwrap();
    if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } != 0 {
        std::process::exit(0);
    }
    file
}

fn redirect_stdio_to_null() {
    if let Ok(null) = fs::File::open("/dev/null") {
        let fd = null.as_raw_fd();
        unsafe {
            dup2(fd, 1);
            dup2(fd, 2);
        }
    }
}

fn get_config_dir() -> PathBuf {
    let home = env::var("HOME").expect("无法获取 HOME 目录");
    let path = format!("{}/.config/kitbar", home);
    let path = PathBuf::from(path);
    fs::create_dir_all(&path).ok();
    path
}

fn load_config() -> Config {
    let mut path = get_config_dir();
    path.push("config.toml");

    if !path.exists() {
        let default = r#"
bottom=90
left=40
        "#;
        fs::write(&path, default).unwrap();
    }

    let content = fs::read_to_string(&path).unwrap();
    toml::from_str(&content).unwrap()
}

fn get_kitty_config() -> PathBuf {
    let mut path = get_config_dir();
    path.push("kitbar.conf");
    if !path.exists() {
        let default = r#"
include ~/.config/kitty/kitty.conf
allow_remote_control socket-only
listen_on unix:/tmp/kitbar
background_opacity 0.6
allow_multiple_windows yes
clear_all_shortcuts yes
map ctrl+shift+c copy_to_clipboard
map ctrl+shift+v paste_from_clipboard
        "#;
        fs::write(&path, default).unwrap();
    }
    path
}

fn get_notepad() -> PathBuf {
    let mut path = get_config_dir();
    path.push("notepad");
    if !path.exists() {
        fs::write(&path, "notepad").unwrap();
    }
    path
}

fn get_hypr_socket_path() -> PathBuf {
    let xdg = env::var("XDG_RUNTIME_DIR").unwrap();
    let instance = env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap();
    let mut path = PathBuf::from(xdg);
    path.push("hypr");
    path.push(instance);
    path.push(".socket2.sock");
    path
}

fn hyptmove(win_id: &str, to: &str) {
    let target = format!("{},address:0x{}", to, win_id);
    let _ = Command::new("hyprctl").arg("dispatch").arg("movetoworkspacesilent").arg(&target).status();
}

fn get_current_ws() -> String {
    let output = Command::new("hyprctl").arg("activeworkspace").output();
    let output = match output {
        Ok(o) => o,
        Err(_) => return "1".into(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.split_whitespace().nth(2).unwrap_or("1").to_string()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1) == Some(&"--monitor".to_string()) {
        monitor::run();
        return;
    }
    if args.get(1) == Some(&"--notepad".to_string()) {
        notepad::run();
        return;
    }

    //redirect_stdio_to_null();
    let _lock: fs::File = lock();

    let stream: UnixStream = UnixStream::connect(get_hypr_socket_path()).expect("stream err");
    let mut reader: BufReader<UnixStream> = BufReader::new(stream);
    let mut buf = String::with_capacity(128);

    open_kitbar();
    let mut kitbar_id: String = String::new();
    loop {
        buf.clear();

        let mut prefix: [u8; 12] = [0u8; 12];

        let n: usize = match reader.read(&mut prefix) {
            Ok(n) => n,
            Err(_) => continue,
        };

        let is_useful: bool = matches!(&prefix[..n],b"movewindowv2" | b"openwindow>>" | b"closewindow>" );
        if !is_useful {
            let _ = reader.read_line(&mut buf);
            continue;
        }
        buf.push_str(std::str::from_utf8(&prefix[..n]).unwrap_or_default());
        let _ = reader.read_line(&mut buf);
        handle_stream(&buf,&mut kitbar_id);

        if kitbar_id.is_empty(){
            open_kitbar();
        }
    }
}

fn open_kitbar() {
    let fifo_path: String = format!("/tmp/kitbar-fifo-{}", std::process::id());
    let _ = fs::remove_file(&fifo_path);
    unsafe { mkfifo(fifo_path.as_ptr() as *const i8, 0o644); }

    let kitty_config: PathBuf = get_kitty_config();
    let sh_cmd: String = format!("echo \"$KITTY_LISTEN_ON\" > '{}'; exec bash", fifo_path);

    let _ = Command::new("kitty")
        .args(["--class", "kitbar"]).arg("--config").arg(kitty_config).arg("bash").arg("-c").arg(sh_cmd)
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("kitky start err");

    std::thread::sleep(std::time::Duration::from_millis(300));
    let socket_raw: String = fs::read_to_string(&fifo_path).unwrap_or_default().trim().to_string();
    let _ = fs::remove_file(&fifo_path);

    let socket: String = socket_raw.replace("unix:", "");

    let rt: Runtime = Runtime::new().unwrap();

    rt.block_on(async {
        let kitty_result  = Kitty::builder().socket_path(&socket).connect().await;
        let mut kitty = match kitty_result {
            Ok(kitty)=>kitty,
            Err(_)=>{
               return;
            }
        };
        
        let config: Config = load_config();
        let hsplit: kitty_rc::KittyMessage = LaunchCommand::builder()
            .location("hsplit".into()).bias(config.bottom as i32).build().to_message().unwrap();
        let _ = kitty.send_command(hsplit).await;

        let vsplit: kitty_rc::KittyMessage = LaunchCommand::builder()
            .location("vsplit".into()).bias(config.left as i32).build().to_message().unwrap();
        let _ = kitty.send_command(vsplit).await;

        /* 
        //command
        let bin: String = env::current_exe().unwrap().to_string_lossy().to_string();
        let cmd1 = format!("{} --monitor", bin);
        let send1 = SendTextCommand::builder().data(cmd1) 
            .match_spec("id:1".to_string()).build().to_message().unwrap();
        let r = kitty.send_command(send1).await;
        match r {
            Ok(_)=>println!("ok!!"),
            Err(e)=>println!("{}",e)
        }

        let cmd2 = format!("{} --notepad\r", bin);
        let send2 = SendTextCommand::builder().data(cmd2) 
            .match_spec("id:1".to_string()).build().to_message().unwrap();
        let _ = kitty.send_command(send2).await;

        let cmd3 = format!("shopt -s expand_aliases\n alias notepad='nano ~/.config/kitbar/notepad'\n cd\n clear\n");
        let send3 = SendTextCommand::builder().data(cmd3) 
            .match_spec("id:1".to_string()).build().to_message().unwrap();
        let _ = kitty.send_command(send3).await;*/
    });

    let bin: String = env::current_exe().unwrap().to_string_lossy().to_string();
    let cmd1: String = format!(" {} --monitor\n", bin);
    let cmd2: String = format!(" {} --notepad\n", bin);
    let cmd3: String = format!(" shopt -s expand_aliases\n alias notepad='nano {}'\n cd\n clear\n", get_notepad().to_str().unwrap());

    let _ = Command::new("kitten").args(["@", "--to", &socket_raw, "send-text", "--match", "id:1", &cmd1]).status();
    let _ = Command::new("kitten").args(["@", "--to", &socket_raw, "send-text", "--match", "id:2", &cmd2]).status();
    let _ = Command::new("kitten").args(["@", "--to", &socket_raw, "send-text", "--match", "id:3", &cmd3]).status();

}

fn handle_stream(line: &str, kitbar_id: &mut String) {
    let Some((part1, part2)) = line.split_once(">>") else { return };
    match part1 {
        "movewindowv2" => {
            let Some((win_id, rest)) = part2.split_once(",") else { return };
            let Some((_, to_raw)) = rest.split_once(",") else { return };
            let to: &str = to_raw.trim();
            if win_id == kitbar_id {
                if to != "special:kitbar" {hyptmove(win_id, "special:kitbar");}
            } else if to == "special:kitbar" {
                hyptmove(win_id, &get_current_ws());
            }
        }
        "openwindow" => {
            let Some((win_id, rest)) = part2.split_once(",") else { return };
            let Some((ws, rest)) = rest.split_once(",") else { return };
            let Some((cls, _)) = rest.split_once(",") else { return };
            if cls == "kitbar" {
                if kitbar_id.is_empty() { 
                    *kitbar_id = win_id.into();
                }
                if ws != "special:kitbar" { hyptmove(kitbar_id, "special:kitbar"); }
            } else if ws == "special:kitbar" {
                hyptmove(win_id, &get_current_ws());
            }
        }
        "closewindow" => {            
            if part2.trim() == kitbar_id {
                *kitbar_id = String::new();
            }
        }
        _ => {}
    }
}
