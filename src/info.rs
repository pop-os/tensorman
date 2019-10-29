use bollard::image::APIImages;
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct Info {
    pub repo:     Box<str>,
    pub tag:      Box<str>,
    pub image_id: Box<str>,
    pub created:  DateTime<Utc>,
    pub size:     u64,
}

impl Info {
    /// Check if any of the string fields matches the `needle`.
    pub fn field_matches(&self, needle: &str) -> bool {
        match self.repo.as_ref() {
            "tensorman" => self.tag.as_ref() == needle,
            "tensorflow/tensorflow" => {
                self.tag.as_ref() == needle || self.image_id.starts_with(needle)
            }
            _ => false,
        }
    }
}

pub fn iterate_image_info(images: Vec<APIImages>) -> impl Iterator<Item = Info> {
    fn valid_tag(tag: &str) -> bool {
        tag.starts_with("tensorflow/tensorflow:") || tag.starts_with("tensorman:")
    }

    images
        .into_iter()
        .filter(|image| image.repo_tags.as_ref().map_or(false, |tags| valid_tag(&*tags[0])))
        .flat_map(|image| {
            let mut image_tags = image.repo_tags.unwrap();

            let mut tags = Vec::new();
            std::mem::swap(&mut tags, &mut image_tags);

            let APIImages { created, size, id, .. } = image;

            tags.into_iter().map(move |tag| {
                let mut fields = tag.split(':');
                let repo = fields.next().expect("image without a repo").to_owned();
                let tag = fields.next().expect("image without a tag").to_owned();
                let id = &id[7..];

                Info { repo: repo.into(), tag: tag.into(), image_id: Box::from(id), created, size }
            })
        })
}
