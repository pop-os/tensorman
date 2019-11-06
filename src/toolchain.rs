use crate::image::{ImageBuf, ImageSourceBuf, TagVariants};

pub fn toolchain_override() -> Option<ImageBuf> {
    std::fs::read_to_string("tensorflow-toolchain").ok().and_then(|info| {
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
    })
}
