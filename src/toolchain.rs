use crate::{
    image::{ImageBuf, ImageSourceBuf, TagVariants},
    misc::walk_parent_directories,
};

pub fn toolchain_override() -> Option<ImageBuf> {
    let current_dir = std::env::current_dir().ok()?;

    let path = walk_parent_directories(&current_dir, "tensorflow-toolchain")?;
    let info = std::fs::read_to_string(&path).ok()?;
    
    let mut iterator = info.trim().split_ascii_whitespace();
    let tag = iterator.next()?;

    Some(ImageBuf {
        variants: iterator.collect::<TagVariants>(),
        source:   if tag.starts_with('=') {
            ImageSourceBuf::Container(tag[1..].into())
        } else {
            ImageSourceBuf::Tensorflow(tag.into())
        },
    })
}
