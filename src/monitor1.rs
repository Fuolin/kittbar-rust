use crate::get_current_ws;
use crossterm::{queue, execute, terminal::{size, Clear, ClearType}, cursor::{Hide, Show, MoveTo}};
use std::io::{stdout, Write};
use std::process::{Command, Stdio};
use std::time::{SystemTime, Duration};
use std::thread;

// 忽略信号
fn ignore_signals() {
    unsafe {
        libc::signal(2, libc::SIG_IGN);
        libc::signal(15, libc::SIG_IGN);
    }
}

// 终端清理
struct Cleanup;
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = execute!(stdout(), Show, MoveTo(4, 0), Clear(ClearType::FromCursorDown));
    }
}

// 状态缓存
#[derive(Default)]
struct StateCache {
    old_cols: u16,
    last_sec: String,
    ws: String,
    vol: String,
    bright: String,
    wifi: String,
    bt: String,
}

// ============== 修复：精准对齐下一秒（无漂移、不跳秒） ==============
fn sleep_to_next_second() {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    // 计算距离下一个整秒的剩余时间
    let sleep_ns = 1_000_000_000 - now.subsec_nanos() as u64;
    thread::sleep(Duration::from_nanos(sleep_ns));
}

// ============== 修复：安全高效的时间格式化 ==============
fn format_time() -> String {
    let now = SystemTime::now();
    let local_time = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let ts = local_time.as_secs() as libc::time_t;

    unsafe {
        let mut tm = std::mem::zeroed::<libc::tm>();
        // 线程安全的本地时间转换
        libc::localtime_r(&ts, &mut tm);

        let mut buf = [0u8; 32];
        libc::strftime(
            buf.as_mut_ptr() as *mut i8,
            32,
            b"%Y-%m-%d %H:%M:%S\0".as_ptr() as *const i8,
            &tm,
        );
        String::from_utf8_lossy(&buf).trim().to_string()
    }
}

// ============== 核心修复：纯标准库实现「命令超时」（解决编译错误）==============
/// 执行系统命令，带超时保护，防止阻塞导致跳秒
fn run_cmd(cmd: &str, args: &[&str], timeout_ms: u64) -> Option<String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let timeout = Duration::from_millis(timeout_ms);
    let start = SystemTime::now();

    // 轮询检查命令是否完成（纯标准库，无第三方依赖）
    loop {
        match child.try_wait() {
            // 命令执行完成
            Ok(Some(_)) => {
                let output = child.wait_with_output().ok()?;
                return Some(String::from_utf8_lossy(&output.stdout).to_string());
            }
            // 未完成，检查是否超时
            Ok(None) => {
                if start.elapsed().unwrap() > timeout {
                    let _ = child.kill(); // 超时杀死进程
                    return None;
                }
                thread::sleep(Duration::from_millis(5)); // 轻量轮询
            }
            // 执行失败
            Err(_) => return None,
        }
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

fn print_time(cols:&u16,time:&String,ws:&String) {
    queue!(stdout(), MoveTo(14, 2)).unwrap();
    print!("{:3}",ws);
    queue!(stdout(), MoveTo(cols - 22, 2)).unwrap();
    print!("{}",time);
}

fn print_part(x: u16, width: usize, text: &str) {
    queue!(stdout(), MoveTo(x, 1)).unwrap();
    print!("{:<width$}", text);
}


// ============== 系统状态更新（使用修复后的超时命令） ==============
fn update_system_state() -> (String, String, String, String, String) {
    // 所有命令 100ms 超时，绝不卡住主循环
    let ws = get_current_ws();

    let vol = run_cmd("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"], 100)
        .and_then(|s| s.split_whitespace().nth(1)
            .map(|x| format!("{}%", x.replace('.', "").trim_start_matches('0'))))
        .unwrap_or_default();

    let bright = run_cmd("brightnessctl", &["i"], 100)
        .and_then(|s| s.lines().nth(1)
            .and_then(|l| l.split_whitespace().nth(2))
            .map(|x| format!("{}%", x)))
        .unwrap_or_default();

    let wifi = run_cmd("nmcli", &["connection", "show", "--active"], 100)
        .and_then(|s| s.lines().find(|l| l.contains("wifi"))
            .and_then(|l| l.split_whitespace().next())
            .map(|x| x.to_string()))
        .unwrap_or_else(|| "none".into());

    let bt = run_cmd("bluetoothctl", &["devices"], 100)
        .and_then(|s| s.lines().next()
            .map(|l| l.chars().skip(11).collect::<String>().trim().to_string()))
        .unwrap_or_else(|| "none".into());

    (ws, vol, bright, wifi, bt)
}

pub fn run() {
    ignore_signals();
    let _clean = Cleanup;
    let mut cache = StateCache::default();
    let _ = execute!(stdout(), Hide);
    let (mut right1,mut right2) = (0,0);

    loop {
        let (cols, _) = size().unwrap_or((80, 24));
        if cols != cache.old_cols {
            let _ = execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0));
            cache.old_cols = cols;
            (right1,right2) = print_backgruand(&cols);
        }

        let time_str = format_time();
        let current_sec = &time_str.split_whitespace().last().unwrap_or("00")[6..8];

        // 偶数秒更新系统信息
        if current_sec.parse::<i32>().unwrap() % 2 == 0 || cache.last_sec.is_empty() {
            let (ws, vol, bright, wifi, bt) = update_system_state();
            cache.ws = ws;
            cache.vol = vol;
            cache.bright = bright;
            cache.wifi = wifi;
            cache.bt = bt;
        }

        // 每秒刷新一次UI
        if current_sec != cache.last_sec {
            print_part(4, 3, &cache.vol);
            print_part(10, 3, &cache.bright);
            print_part(16, right1, &cache.bt);
            print_part((19 + right1) as u16, right2, &cache.wifi);
            print_time(&cols,&time_str,&cache.ws);
            
            cache.last_sec = current_sec.to_string();
        }

        let _ = stdout().flush();
        sleep_to_next_second();
    }
}