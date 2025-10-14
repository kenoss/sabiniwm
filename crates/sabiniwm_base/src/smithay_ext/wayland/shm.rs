use smithay::utils::{Buffer as BufferCoord, Size};
use smithay::wayland::shm::BufferData;

pub trait BufferDataExt {
    fn size(&self) -> Size<i32, BufferCoord>;
}

impl BufferDataExt for BufferData {
    fn size(&self) -> Size<i32, BufferCoord> {
        (self.width, self.height).into()
    }
}
