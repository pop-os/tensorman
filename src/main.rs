#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate thiserror;

mod config;
mod image;
mod info;
mod misc;
mod runtime;
mod toolchain;

use anyhow::Context;
use clap::{App, AppSettings, Arg, ArgSettings, SubCommand};

use self::{
    config::Config,
    image::{Image, ImageBuf, ImageSource, ImageSourceBuf, TagVariants},
    runtime::Runtime,
};

use std::{error::Error as _, process::exit};

#[derive(Debug, Error)]
pub enum Error {
    #[error("configuration error")]
    Configure(#[source] anyhow::Error),
    #[error("an error with docker was encountered")]
    Docker(#[source] anyhow::Error),
}

fn main_() -> Result<(), Error> {
    let matches = App::new("Tensorman")
        .about("Tensorflow Docker image manager")
        .setting(AppSettings::SubcommandRequired)
        .setting(AppSettings::DisableVersion)
        .setting(AppSettings::VersionlessSubcommands)
        .arg(Arg::with_name("tag-or-container")
            .help("+TAG_NAME or =CONTAINER_NAME")
            .required(false)
            .validator(|s|
                if s.starts_with("+") || s.starts_with("=") {
                    Ok(())
                } else {
                    Err(format!("'{}' doesn't start with + or =", s))
                }
            )
        )
        .subcommand(SubCommand::with_name("default")
            .about("Defines the default tensorflow image to use when not specified")
            .arg(Arg::with_name("tag")
                .help("Tag")
                .required(true)
            )
            .arg(Arg::with_name("variant")
                .help("Variant(s)")
                .possible_values(&["gpu", "python3", "jupyter"])
                .multiple(true)
            )
        )
        .subcommand(SubCommand::with_name("list")
            .about("List tensorflow images installed on the system")
        )
        .subcommand(SubCommand::with_name("pull")
            .about("Fetches and updates tensorflow images")
            .arg(Arg::with_name("tag")
            )
        )
        .subcommand(SubCommand::with_name("remove")
            .about("Removes an image either by its sha sum, or tag")
            .arg(Arg::with_name("image")
                .help("Image(s) to remove")
                .multiple(true)
                .required(true)
            )
        )
        .subcommand(SubCommand::with_name("run")
            .about("Mounts an image and executes the given command.")
            .after_help("Use a shell as the <cmd> for interactive sessions.")
            .arg(Arg::with_name("cmd")
                .help("Command to run in container")
                .required(true)
            )
            .arg(Arg::with_name("arg")
                .help("Argument(s) to command")
                .multiple(true)
            )
        )
        .subcommand(SubCommand::with_name("save")
            .about("Saves an active container to a new image.")
            .arg(Arg::with_name("container")
                .help("Container to save")
                .required(true)
            )
            .arg(Arg::with_name("image")
                .help("Name of new image")
                .required(true)
            )
        )
        .subcommand(SubCommand::with_name("show")
            .about("Show the active image that will be run")
        )
        .arg(Arg::with_name("force")
            .set(ArgSettings::Global)
            .help("Apply the subcommand by force (ie: force removal)")
            .short("f")
            .long("force")
        )
        .arg(Arg::with_name("gpu")
            .set(ArgSettings::Global)
            .help("Uses an image which supports GPU compute")
            .long("gpu")
        )
        .arg(Arg::with_name("docker-cmd")
            .set(ArgSettings::Global)
            .help("Call COMMAND when invoking docker")
            .long("docker-cmd")
            .takes_value(true)
        )
        .arg(Arg::with_name("jupyter")
            .set(ArgSettings::Global)
            .help("Usages an image which has Jupyter preinstalled")
            .long("jupyter")
        )
        .arg(Arg::with_name("name")
            .set(ArgSettings::Global)
            .help("Gives NAME to the container when it is launched")
            .long("name")
            .takes_value(true)
        )
        .arg(Arg::with_name("port")
            .set(ArgSettings::Global)
            .help("Specifies a port mapping for the container and host")
            .short("p")
            .long("port")
            .takes_value(true)
        )
        .arg(Arg::with_name("python3")
            .set(ArgSettings::Global)
            .help("Uses an image which supports Python3")
            .long("python3")
        )
        .arg(Arg::with_name("root")
            .set(ArgSettings::Global)
            .help("Run the docker container as root")
            .long("root")
        )
        .get_matches();

    let config = Config::read().map_err(Error::Configure)?;
    let toolchain_override = toolchain::toolchain_override();

    let (mut specific_container, mut tag, mut variants) =
        toolchain_override.as_ref().or_else(|| config.image.as_ref()).map_or_else(
            || (None, "latest", TagVariants::empty()),
            |image| {
                let (container, tag) = match &image.source {
                    ImageSourceBuf::Container(container) => (Some(container.as_ref()), None),
                    ImageSourceBuf::Tensorflow(tag) => (None, Some(tag.as_ref())),
                };

                (container, tag.unwrap_or("latest"), image.variants)
            },
        );

    if let Some(tag_or_container) = matches.value_of("tag-or-container") {
        if tag_or_container.starts_with('+') {
            tag = &tag_or_container[1..];
        } else if tag_or_container.starts_with('=') {
            specific_container = Some(&tag_or_container[1..]);
        } else {
            unreachable!();
        }
    }

    let force = matches.is_present("force");
    let as_root = matches.is_present("root");

    let docker_cmd = matches.value_of("docker-cmd").unwrap_or("docker");
    let name = matches.value_of("name");
    let port = matches.value_of("port");

    let mut flagged_variants = TagVariants::empty();
    if matches.is_present("gpu") {
        flagged_variants |= TagVariants::GPU;
    }
    if matches.is_present("jupyter") {
        flagged_variants |= TagVariants::JUPYTER;
    }
    if matches.is_present("python3") {
        flagged_variants |= TagVariants::PY3;
    }
    if !flagged_variants.is_empty() {
        variants = flagged_variants;
    }

    let mut image = Image {
        variants,
        source: match specific_container {
            Some(container) => ImageSource::Container(container),
            None => ImageSource::Tensorflow(tag),
        },
    };

    let mut runtime = Runtime::new(docker_cmd).map_err(Error::Docker)?;

    if let Some(sub_m) = matches.subcommand_matches("default") {
        let tag = sub_m.value_of("tag").unwrap();

        let source = if tag.starts_with('=') {
            ImageSourceBuf::Container(tag[1..].into())
        } else {
            ImageSourceBuf::Tensorflow(tag.into())
        };

        let variants = if let Some(variants) = sub_m.values_of("variant") {
            variants.collect()
        } else {
            TagVariants::empty()
        };

        let new_config = Config { docker_flags: None, image: Some(ImageBuf { variants, source }) };

        new_config.write().map_err(Error::Configure)?;
    } else if let Some(_sub_m) = matches.subcommand_matches("list") {
        runtime.list().map_err(Error::Docker)?;
    } else if let Some(sub_m) = matches.subcommand_matches("pull") {
        if let Some(tag) = sub_m.value_of("tag") {
            image.source = ImageSource::Tensorflow(tag);
            image.variants = flagged_variants;
        }

        image.pull(docker_cmd).context("failed to pull image").map_err(Error::Docker)?;
    } else if let Some(sub_m) = matches.subcommand_matches("remove") {
        for image in sub_m.values_of("image").unwrap() {
            runtime
                .remove(image, force)
                .with_context(|| format!("failed to remove container '{}'", image))
                .map_err(Error::Docker)?;
        }
    } else if let Some(sub_m) = matches.subcommand_matches("run") {
        let cmd = sub_m.value_of("cmd").unwrap();
        let args = sub_m.values_of("arg").map(|x| x.collect::<Vec<_>>());
        let dflags = config.docker_flags.as_ref().map(Vec::as_slice);

        runtime
            .run(&image, cmd, name, port, as_root, args.as_ref().map(|x| x.as_ref()), dflags)
            .context("failed to run container")
            .map_err(Error::Docker)?;
    } else if let Some(sub_m) = matches.subcommand_matches("save") {
        let container = sub_m.value_of("container").unwrap();
        let image = sub_m.value_of("image").unwrap();

        runtime
            .save(container, image)
            .with_context(|| format!("failed to save container '{}' as '{}'", container, image))
            .map_err(Error::Docker)?;
    } else if let Some(_sub_m) = matches.subcommand_matches("show") {
        println!("{}", image);
    } else {
        unreachable!();
    }

    Ok(())
}

fn main() {
    if let Err(why) = main_() {
        eprintln!("tensorman: {}", why);
        let mut source = why.source();
        while let Some(why) = source {
            eprintln!("    caused by: {}", why);
            source = why.source();
        }

        exit(1);
    }
}
