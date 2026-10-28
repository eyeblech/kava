use std::io::{self, Read, Write};
use std::process::Command;

#[cfg(unix)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

#[cfg(unix)]
#[repr(C)]
#[derive(Clone, Copy)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

#[cfg(target_os = "linux")]
const TIOCGWINSZ: usize = 0x5413;
#[cfg(target_os = "macos")]
const TIOCGWINSZ: usize = 0x40087468;
#[cfg(all(unix, not(target_os = "linux"), not(target_os = "macos")))]
const TIOCGWINSZ: usize = 0x40087468;

#[cfg(unix)]
const POLLIN: i16 = 0x0001;

#[cfg(unix)]
extern "C" {
    fn ioctl(fd: i32, request: usize, ...) -> i32;
    fn poll(fds: *mut PollFd, nfds: usize, timeout: i32) -> i32;
}

pub fn terminal_size() -> (u16, u16) {
    #[cfg(unix)]
    {
        let mut ws = Winsize::default();
        let res = unsafe { ioctl(1, TIOCGWINSZ, &mut ws as *mut Winsize) };
        if res == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
            return (ws.ws_col, ws.ws_row);
        }
    }
    if let (Ok(c), Ok(r)) = (std::env::var("COLUMNS"), std::env::var("LINES")) {
        if let (Ok(cols), Ok(rows)) = (c.parse::<u16>(), r.parse::<u16>()) {
            if cols > 0 && rows > 0 {
                return (cols, rows);
            }
        }
    }
    (100, 34)
}

pub struct RawTerminalGuard;

impl RawTerminalGuard {
    pub fn enter() -> io::Result<Self> {
        #[cfg(unix)]
        {
            let _ = Command::new("stty")
                .args(["-icanon", "-echo", "-isig", "min", "0", "time", "0"])
                .status();
        }
        let mut stdout = io::stdout();
        stdout.write_all(b"\x1b[?1049h\x1b[?25l\x1b[2J")?;
        stdout.flush()?;
        Ok(Self)
    }
}

impl Drop for RawTerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = stdout.write_all(b"\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = stdout.flush();
        #[cfg(unix)]
        {
            let _ = Command::new("stty").arg("sane").status();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKey {
    Char(char),
    Up,
    Down,
    Left,
    Right,
    Tab,
    Esc,
    CtrlC,
}

/// Non-blocking poll and read of keyboard events from stdin
pub fn poll_key() -> Option<InputKey> {
    #[cfg(unix)]
    {
        let mut pfd = PollFd {
            fd: 0,
            events: POLLIN,
            revents: 0,
        };
        let ret = unsafe { poll(&mut pfd as *mut PollFd, 1, 0) };
        if ret <= 0 || (pfd.revents & POLLIN) == 0 {
            return None;
        }

        let mut buf = [0u8; 8];
        let n = io::stdin().read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }

        match buf[0] {
            3 => Some(InputKey::CtrlC),
            9 => Some(InputKey::Tab),
            27 => {
                if n == 1 {
                    Some(InputKey::Esc)
                } else if n >= 3 && buf[1] == b'[' {
                    match buf[2] {
                        b'A' => Some(InputKey::Up),
                        b'B' => Some(InputKey::Down),
                        b'C' => Some(InputKey::Right),
                        b'D' => Some(InputKey::Left),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            b => Some(InputKey::Char(b as char)),
        }
    }
    #[cfg(not(unix))]
    {
        None
    }
}
