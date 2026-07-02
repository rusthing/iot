const SEQ_MAX_SIZE: u16 = 32768;

pub struct Window {
    /// 窗口的左边界(包含)
    left: u16,
    /// 窗口的右边界(不包含)
    right: u16,
    /// 窗口的最大大小
    max_size: usize,
}

impl Window {
    fn new(max_size: usize) -> Self {
        Self {
            left: 0,
            right: 0,
            max_size,
        }
    }

    pub fn inc(&mut self) {
        self.right = (self.right + 1) % SEQ_MAX_SIZE;
    }

    pub fn is_empty(&self) -> bool {
        self.left == self.right
    }

    pub fn current(&self) -> u16 {
        self.right
    }

    pub fn current_size(&self) -> usize {
        let left = self.left as usize;
        let right = self.right as usize;
        if right >= left {
            right - left
        } else {
            right + SEQ_MAX_SIZE as usize - left
        }
    }

    pub fn is_full(&self) -> bool {
        self.current_size() >= self.max_size
    }
}

pub struct SendWindow {
    pub window: Window,
}

impl SendWindow {
    pub fn new(max_size: usize) -> Self {
        Self {
            window: Window::new(max_size),
        }
    }

    pub fn confirm(&mut self, n: u16) {
        self.window.left = n;
    }
}

pub struct RecvWindow {
    pub window: Window,
}

impl RecvWindow {
    pub fn new(max_size: usize) -> Self {
        Self {
            window: Window::new(max_size),
        }
    }

    /// 清空窗口
    /// 窗口的左边界设置为右边界
    pub fn clear(&mut self) {
        self.window.left = self.window.right;
    }
}
