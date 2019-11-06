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
use bollard::Docker;

use self::{
    config::Config,
    image::{Image, ImageBuf, ImageSource, ImageSourceBuf, TagVariants},
    runtime::Runtime,
};

use std::{env::args, error::Error as _, process::exit};

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid command-line usage")]
    ArgumentUsage(#[source] anyhow::Error),
    #[error("configuration error")]
    Configure(#[source] anyhow::Error),
    #[error("an error with docker was encountered")]
    Docker(#[source] anyhow::Error),
}

fn main_() -> Result<(), Error> {
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

    let arguments: Vec<String> = args().skip(1).collect();
    let mut arguments = arguments.iter();

    // Allow the first argument, if it begins with `+`, to override the tag.
    let mut subcommand = arguments.next().and_then(|argument| {
        if argument.starts_with('+') {
            tag = &argument[1..];
            arguments.next().map(String::as_str)
        } else if argument.starts_with('=') {
            specific_container = Some(&argument[1..]);
            arguments.next().map(String::as_str)
        } else {
            Some(argument.as_str())
        }
    });

    let subcommand = subcommand
        .take()
        .context("tensorman must be given a subcommand to execute")
        .map_err(Error::ArgumentUsage)?;

    let mut subcommand_args = Vec::new();

    let mut as_root = false;
    let mut force = false;

    let mut flagged_variants = TagVariants::empty();

    let mut name = None;
    let mut port = None;

    let mut docker_func: fn() -> Result<Docker, failure::Error> =
        Docker::connect_with_local_defaults;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => help(),
            "--" => break,
            "-f" | "--force" => force = true,
            "--gpu" => flagged_variants |= TagVariants::GPU,
            "--https" => docker_func = Docker::connect_with_tls_defaults,
            "--jupyter" => flagged_variants |= TagVariants::JUPYTER,
            "--name" => {
                name = Some(
                    arguments
                        .next()
                        .context("the --name flag requires a name as an argument")
                        .map_err(Error::ArgumentUsage)?
                        .as_str(),
                );
            }
            "-p" | "--port" => {
                port = Some(
                    arguments
                        .next()
                        .context("the --port flag requires an argument")
                        .map_err(Error::ArgumentUsage)?
                        .as_str(),
                );
            }
            "--python3" => flagged_variants |= TagVariants::PY3,
            "--root" => as_root = true,
            argument => {
                if argument.starts_with('-') {
                    eprintln!("unknown argument to tensorman: {}", argument);
                    help();
                }

                subcommand_args.push(argument)
            }
        }
    }

    subcommand_args.extend(arguments.map(|x| x.as_str()));
    let mut subcommand_args = subcommand_args.into_iter();

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

    let mut runtime = Runtime::new(docker_func).map_err(Error::Docker)?;

    match subcommand {
        "default" => {
            let tag = subcommand_args
                .next()
                .context("a tag must be provided for the default subcommand")
                .map_err(Error::ArgumentUsage)?;

            let source = if tag.starts_with('=') {
                ImageSourceBuf::Container(tag[1..].into())
            } else {
                ImageSourceBuf::Tensorflow(tag.into())
            };

            let variants = subcommand_args.collect::<TagVariants>();

            let new_config = Config { image: Some(ImageBuf { variants, source }) };

            new_config.write().map_err(Error::Configure)?;
        }
        "list" => {
            runtime.list().map_err(Error::Docker)?;
        }
        "pull" => {
            if let Some(tag) = subcommand_args.next() {
                image.source = ImageSource::Tensorflow(tag);
                image.variants = flagged_variants;
            }

            image.pull().context("failed to pull image").map_err(Error::Docker)?;
        }
        "remove" => {
            if subcommand_args.len() == 0 {
                return Err(Error::ArgumentUsage(anyhow!(
                    "an image must be provided for the remove subcommand"
                )));
            }

            for image in subcommand_args {
                runtime
                    .remove(image, force)
                    .with_context(|| format!("failed to remove container '{}'", image))
                    .map_err(Error::Docker)?;
            }
        }
        "run" => {
            let cmd = subcommand_args
                .next()
                .context("run subcommand requires a command argument")
                .map_err(Error::ArgumentUsage)?;

            let args: Vec<&str> = subcommand_args.collect();
            let args: Option<&[&str]> = if args.is_empty() { None } else { Some(&args) };

            runtime
                .run(&image, cmd, name, port, as_root, args)
                .context("failed to run container")
                .map_err(Error::Docker)?;
        }
        "save" => {
            let container = subcommand_args
                .next()
                .context("save subcommand requires a container name as a source")
                .map_err(Error::ArgumentUsage)?;

            let image = subcommand_args
                .next()
                .context("save subcommand requires an image name as the destination")
                .map_err(Error::ArgumentUsage)?;

            runtime
                .save(container, image)
                .with_context(|| format!("failed to save container '{}' as '{}'", container, image))
                .map_err(Error::Docker)?;
        }
        "show" => {
            if subcommand_args.len() == 0 {
                println!("{}", image);
            } else {
                unimplemented!()
            }
        }
        _ => help(),
    }

    Ok(())
}

const HELP: &str = "tensorman
    Tensorflow Docker image manager

USAGE:
    tensorman [+TAG | =CONTAINER] SUBCOMMAND [FLAGS...]

SUBCOMMANDS:
    default TAG [VARIANTS...]
        Defines the default tensorflow image to use when not specified

    list
        List tensorflow images installed on the system

    pull [TAG]
        Fetches and updates tensorflow images

    remove ID
        Removes an image either by its sha sum, or tag

    run COMMAND [-- ARGS...]
        Mounts an image and executes the given command.

        Use a shell as the COMMAND interactive sessions.

    save CONTAINER NAME
        Saves an active container with the name of CONTAINER to a new image
        which shall be named NAME.

    show
        Show the active image that will be run

FLAGS:
    -f, --force
        Apply the subcommand by force (ie: force removal)

    --gpu
        Uses an image which supports GPU compute
    
    --https
        Connect to Docker via HTTPS (defined in DOCKER_HOST env variable)

    --jupyter
        Usages an image which has Jupyter preinstalled

    --name NAME
        Gives NAME to the container when it is launched

    -p, --port
        Specifies a port mapping for the container and host

    --python3
        Uses an image which supports Python3

    --root
        Run the docker container as root

    -h, --help
        Display this information";

fn help() -> ! {
    println!("{}", HELP);
    exit(0);
}

fn main() {
    if let Err(why) = main_() {
        eprintln!("tensorman: {}", why);
        let mut source = why.source();
        while let Some(why) = source {
            eprintln!("    caused by: {}", why);
            source = why.source();
        }

        if let Error::ArgumentUsage(_) = why {
            help()
        }

        exit(1);
    }
}
