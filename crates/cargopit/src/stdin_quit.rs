//! Play-session stdin quit-key poll. Matches C `init_stdin_quit_poll`.

const RAW_MIN_BYTES: libc::cc_t = 1;
const RAW_TIMEOUT: libc::cc_t = 0;
const KEY_READ_COUNT: usize = 1;

#[derive(Debug)]
pub struct StdinQuit {
    original: libc::termios,
}

impl Drop for StdinQuit {
    fn drop(&mut self) {
        restore(&self.original);
    }
}

#[derive(Debug)]
pub enum StdinQuitStart {
    NotTty,
    Ready(StdinQuit),
    ReadSettingsFailed,
    RawModeFailed,
    PollFailed,
}

pub fn start() -> StdinQuitStart {
    if !is_tty() {
        return StdinQuitStart::NotTty;
    }
    let Some(original) = read_settings() else {
        return StdinQuitStart::ReadSettingsFailed;
    };
    if !set_raw(&original) {
        return StdinQuitStart::RawModeFailed;
    }
    if !set_nonblocking() {
        restore(&original);
        return StdinQuitStart::PollFailed;
    }
    StdinQuitStart::Ready(StdinQuit { original })
}

pub fn read_key() -> Option<u8> {
    let mut byte = [0u8; KEY_READ_COUNT];
    let n = unsafe {
        libc::read(
            libc::STDIN_FILENO,
            byte.as_mut_ptr().cast::<libc::c_void>(),
            KEY_READ_COUNT,
        )
    };
    if n == KEY_READ_COUNT as isize {
        Some(byte[0])
    } else {
        None
    }
}

fn is_tty() -> bool {
    unsafe { libc::isatty(libc::STDIN_FILENO) != 0 }
}

fn read_settings() -> Option<libc::termios> {
    let mut settings = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut settings) };
    if rc != 0 {
        return None;
    }
    Some(settings)
}

fn set_raw(original: &libc::termios) -> bool {
    let mut raw = *original;
    raw.c_lflag &= !(libc::ICANON | libc::ECHO);
    raw.c_cc[libc::VMIN] = RAW_MIN_BYTES;
    raw.c_cc[libc::VTIME] = RAW_TIMEOUT;
    unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) == 0 }
}

fn set_nonblocking() -> bool {
    let flags = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFL) };
    if flags < 0 {
        return false;
    }
    unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_SETFL, flags | libc::O_NONBLOCK) >= 0 }
}

fn restore(original: &libc::termios) {
    unsafe {
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, original);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_skips_when_stdin_is_not_a_tty() {
        if is_tty() {
            return;
        }
        assert!(matches!(start(), StdinQuitStart::NotTty));
    }

    #[test]
    fn read_key_returns_none_when_stdin_is_not_a_tty() {
        if is_tty() {
            return;
        }
        assert_eq!(read_key(), None);
    }
}
