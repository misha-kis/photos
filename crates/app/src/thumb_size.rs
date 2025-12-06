#[derive(Debug, Clone, Copy)]
pub enum ThumbSize {
    T32 = 32,
    T64 = 64,
    T128 = 128,
    T256 = 256,
}

impl ThumbSize {
    pub fn prev(self) -> ThumbSize {
        match self {
            ThumbSize::T32 => ThumbSize::T32,
            ThumbSize::T64 => ThumbSize::T32,
            ThumbSize::T128 => ThumbSize::T64,
            ThumbSize::T256 => ThumbSize::T128,
        }
    }

    pub fn next(self) -> ThumbSize {
        match self {
            ThumbSize::T32 => ThumbSize::T64,
            ThumbSize::T64 => ThumbSize::T128,
            ThumbSize::T128 => ThumbSize::T256,
            ThumbSize::T256 => ThumbSize::T256,
        }
    }
}
