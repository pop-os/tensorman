use crate::image::{ImageBuf, TagVariants};

pub fn toolchain_override() -> Option<ImageBuf> {
    std::fs::read_to_string("tensorflow-toolchain").ok().and_then(|info| {
        let mut iterator = info.trim().split_ascii_whitespace();
        let tag = iterator.next()?;

        Some(ImageBuf { tag: tag.into(), variants: iterator.collect::<TagVariants>() })
    })
}
