use crate::image::{ImageBuf, TagVariants};

pub fn toolchain_override() -> Option<ImageBuf> {
    std::fs::read_to_string("tensorflow-toolchain").ok().and_then(|info| {
        let mut iterator = info.trim().split_ascii_whitespace();
        let tag = iterator.next()?;

        let mut variants = TagVariants::empty();
        for variant in iterator {
            match variant {
                "gpu" => variants |= TagVariants::GPU,
                "py3" => variants |= TagVariants::PY3,
                "jupyter" => variants |= TagVariants::JUPYTER,
                _ => (),
            }
        }

        Some(ImageBuf { tag: tag.into(), variants })
    })
}
