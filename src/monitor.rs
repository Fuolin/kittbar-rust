use crate::get_current_ws;
use std::io::{stdout, Write};
use std::mem;
use std::ffi::CStr;
use std::time::{SystemTime, UNIX_EPOCH};
use std::os::unix::io::AsRawFd;
use std::process::{Command, Stdio};
use crossterm::{queue,execute, terminal::{size, Clear, ClearType}, cursor::{Hide, Show, MoveTo}};

use libc::{
    time_t, tm, localtime_r, strftime, poll, pollfd, POLLIN,
    inotify_init, inotify_add_watch, IN_MODIFY,
};

fn ignore_signals() {
    unsafe {
        libc::signal(2, libc::SIG_IGN);
        libc::signal(15, libc::SIG_IGN);
    }
}

struct Cleanup;
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = execute!(stdout(), Show, MoveTo(4, 0), Clear(ClearType::FromCursorDown));
    }
}

fn now_datetime() -> String {
    unsafe {
        // 获取系统时间戳
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as time_t;

        // 转为本地时间
        let mut tm: tm = mem::zeroed();
        localtime_r(&ts, &mut tm);

        // 格式化：严格 YYYY-MM-DD HH:MM:SS
        let mut buf = [0i8; 20];
        strftime(
            buf.as_mut_ptr(),
            20,
            b"%Y-%m-%d %H:%M:%S\0".as_ptr() as *const i8,
            &tm
        );

        // 返回字符串
        CStr::from_ptr(buf.as_ptr())
            .to_str()
            .unwrap_or_default()
            .to_string()
    }
}

fn print_backgruand(cols:&u16) -> (usize,usize) {
        let content_width:usize = cols.saturating_sub(3) as usize;
            let border: String = "─".repeat(content_width);
            let right:usize = content_width.saturating_sub(19);
            let right1: usize = right / 2;
            let right2: usize = right - right1;

            let total_space: usize = content_width.saturating_sub(51) as usize;
            let pad_left: usize = total_space / 2;
            let pad_right: usize = total_space - pad_left;
    //print
    let _ = execute!(stdout(), MoveTo(0, 0));
    let _ = writeln!(stdout(), "╭{border}╮");
    let _ = writeln!(stdout(), "│ 󰕾     󰃠     󰂯 {:<right1$} 󰖩 {:<right2$} │","","");
    let _ = writeln!(stdout(), "│ 󰙅 workspace:   {:pad_left$}Niloufer abi{:pad_right$} 󰃰                     │","","");
    let _ = writeln!(stdout(), "╰{border}╯");

    (right1,right2)
}

fn print_time(cols:&u16,time:&String) {
    queue!(stdout(), MoveTo(14, 2)).unwrap();
    print!("{:3}",get_current_ws());
    queue!(stdout(), MoveTo(cols - 22, 2)).unwrap();
    print!("{}",time);
    stdout().flush().unwrap(); 
}

fn print_part(x: u16, width: usize, text: &str) {
    queue!(stdout(), MoveTo(x, 1)).unwrap();
    print!("{:<width$}", text);
    stdout().flush().unwrap();
}

/// 创建inotify监听文件修改
fn create_inotify_fd(path: &str) -> i32 {
    unsafe {
        let fd = inotify_init();
        inotify_add_watch(fd, path.as_ptr() as *const i8, IN_MODIFY);
        fd
    }
}

/// 执行命令并返回stdout管道FD
fn create_cmd_fd(cmd: &str, args: &[&str]) -> i32 {
    let child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdout.unwrap().as_raw_fd()
}

/// 读取文件内容
fn read_file(path: &str) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// 解析蓝牙设备名
fn parse_bt(data: &str) -> String {
    data.lines()
        .last()
        .and_then(|l| l.split_whitespace().last())
        .unwrap_or("disconnected")
        .to_string()
}

pub fn run() {
    ignore_signals();
    let _clean: Cleanup = Cleanup;
    let _ = execute!(stdout(), Hide);

    let uid = std::env::var("UID").unwrap_or_default();
    let volume_fd = create_inotify_fd(&format!("/run/user/{uid}/pipewire-0"));
    let brightness_fd = create_inotify_fd("/sys/class/backlight/nvidia_0/brightness");
    let nm_fd = create_cmd_fd("nmcli", &["monitor"]);
    let bt_fd = create_cmd_fd("bluetoothctl", &[]);

    let mut fds = [
        pollfd { fd: volume_fd, events: POLLIN, revents: 0 },
        pollfd { fd: brightness_fd, events: POLLIN, revents: 0 },
        pollfd { fd: nm_fd, events: POLLIN, revents: 0 },
        pollfd { fd: bt_fd, events: POLLIN, revents: 0 },
    ];

    let mut old_cols: u16 = 0;
    let (mut right1,mut right2)= (0,0);
    let mut buf = [0u8; 1024];

    let mut old_time: String = String::new();
    loop {
        unsafe { poll(fds.as_mut_ptr(), fds.len() as u64, -1) };//添加到下一秒的毫秒级timeout

        let cols= match size() {
            Ok((_cols,_)) => _cols,
            Err(_)=>old_cols
        };

        if cols != old_cols {
            let _ = execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0));
            old_cols = cols;
            (right1,right2) = print_backgruand(&cols);
        }

        if fds[0].revents & POLLIN != 0 {
            let vol = read_file(&format!("/run/user/{uid}/pipewire-0"));
            print_part(4, 3, &vol);
        }

        if fds[1].revents & POLLIN != 0 {
            let bright = read_file("/sys/class/backlight/nvidia_0/brightness");
            print_part(10, 3, &bright);
        }

        //nmcli monitor 
        if fds[2].revents & POLLIN != 0 {
            let n = unsafe { libc::read(nm_fd, buf.as_mut_ptr() as _, buf.len()) };
            if n > 0 {
                let wifi = String::from_utf8_lossy(&buf[..n as usize]).trim().to_string();
                print_part((19 + right1) as u16, right2, &wifi);
            }
        }

        //bluetoothctl | grep --line-buffered -E "NEW Device|CHG Device|Connected:"
        if fds[3].revents & POLLIN != 0 {
            let n = unsafe { libc::read(bt_fd, buf.as_mut_ptr() as _, buf.len()) };
            if n > 0 {
                let bt = parse_bt(&String::from_utf8_lossy(&buf[..n as usize]));
                print_part(16, right1, &bt);
            }
        }

        //时间刷新
        let time: String = now_datetime();
        if time != old_time {
            print_time(&cols,&time);
            old_time = time;
        }
std::thread::sleep(std::time::Duration::from_millis(2000));//测试用，会删掉
    }
}