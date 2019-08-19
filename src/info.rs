use crate::Error;
use rs_docker::image::Image;

#[derive(Debug)]
pub struct Info {
    pub repo:     Box<str>,
    pub tag:      Box<str>,
    pub image_id: Box<str>,
    pub created:  u64,
    pub size:     u64,
}

impl Info {
    /// Check if any of the string fields matches the `needle`.
    pub fn field_matches(&self, needle: &str) -> bool {
        self.tag.as_ref() == needle || self.image_id.starts_with(needle)
    }
}

pub fn iterate_image_info(mut images: Vec<Image>) -> impl Iterator<Item = Info> {
    fn valid_tag(tag: &str) -> bool {
        tag.starts_with("nvidia/") || tag.starts_with("tensorflow/tensorflow:")
    }

    images
        .into_iter()
        .filter(|image| !image.RepoTags.is_empty() && valid_tag(&*image.RepoTags[0]))
        .flat_map(|mut image| {
            let mut tags = Vec::new();
            std::mem::swap(&mut tags, &mut image.RepoTags);
            let rs_docker::image::Image { Created, Id, Size, .. } = image;

            tags.into_iter().map(move |tag| {
                let mut fields = tag.split(':');
                let repo = fields.next().expect("image without a repo").to_owned();
                let tag = fields.next().expect("image without a tag").to_owned();
                let id = &Id[7..];

                Info {
                    repo:     repo.into(),
                    tag:      tag.into(),
                    image_id: id.into(),
                    created:  Created.into(),
                    size:     Size,
                }
            })
        })
}
