#[macro_use]
extern crate thiserror;

mod config;
mod image;
mod info;
mod runtime;
mod toolchain;

use bollard::Docker;

use self::{
    config::{Config, Error as ConfigError},
    image::{Image, ImageBuf, ImageSource, ImageSourceBuf, TagVariants},
    runtime::Runtime,
};

use std::{env::args, error::Error as _, io, process::exit};

#[derive(Debug, Error)]
pub enum Error {
    #[error("configuration error")]
    Configure(#[source] ConfigError),
    #[error("docker error")]
    Docker(#[from] runtime::Error),
    #[error("the --name flag requires a name as an argument")]
    NameFlag,
    #[error("subcommand requires at least one argument")]
    RequiresArgument,
    #[error("a name for the new image must be given")]
    SaveAsArgument,
    #[error("subcommand failed")]
    Subcommand(#[source] io::Error),
    #[error("missing subcommand argument")]
    SubcommandRequired,
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

    let subcommand = subcommand.take().ok_or(Error::SubcommandRequired)?;

    let mut subcommand_args = Vec::new();

    let mut as_root = false;
    let mut flagged_variants = TagVariants::empty();
    let mut name = None;
    let mut docker_func: fn() -> Result<Docker, failure::Error> =
        Docker::connect_with_local_defaults;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => help(),
            "--" => break,
            "--gpu" => flagged_variants |= TagVariants::GPU,
            "--https" => docker_func = Docker::connect_with_tls_defaults,
            "--jupyter" => flagged_variants |= TagVariants::JUPYTER,
            "--name" => {
                name = Some(arguments.next().ok_or(Error::NameFlag)?.as_str());
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

    let mut runtime = Runtime::new(docker_func)?;

    match subcommand {
        "default" => {
            let tag = argument_required(&subcommand_args)?;
            let variants = subcommand_args.into_iter().skip(1).collect::<TagVariants>();

            let new_config = Config {
                image: Some(ImageBuf { variants, source: ImageSourceBuf::Tensorflow(tag.into()) }),
            };

            new_config.write().map_err(Error::Configure)?;
        }
        "list" => {
            runtime.list()?;
        }
        "pull" => {
            if let Some(tag) = subcommand_args.get(0) {
                image.source = ImageSource::Tensorflow(tag);
                image.variants = flagged_variants;
            }

            image.pull().map_err(Error::Subcommand)?;
        }
        "remove" => {
            let image = argument_required(&subcommand_args)?;
            runtime.remove(image)?;
        }
        "run" => {
            let cmd = argument_required(&subcommand_args)?;
            let args: Vec<&str> = subcommand_args.into_iter().skip(1).collect();

            let args: Option<&[&str]> = if args.is_empty() { None } else { Some(&args) };
            runtime.run(&image, cmd, name, as_root, args)?;
        }
        "save" => {
            let container = argument_required(&subcommand_args)?;
            let save_as: &str = subcommand_args.get(1).ok_or(Error::SaveAsArgument)?;
            runtime.save(container, save_as)?;
        }
        "show" => {
            if subcommand_args.is_empty() {
                println!("{}", image);
            } else {
                unimplemented!()
            }
        }
        _ => help(),
    }

    Ok(())
}

fn argument_required<'a>(args: &[&'a str]) -> Result<&'a str, Error> {
    if args.is_empty() {
        Err(Error::RequiresArgument)
    } else {
        Ok(args[0])
    }
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
    --gpu
        Uses an image which supports GPU compute

    --jupyter
        Usages an image which has Jupyter preinstalled

    --https
        Connect to Docker via HTTPS (defined in DOCKER_HOST env variable)

    --name NAME
        Gives NAME to the container when it is launched.

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
        match why {
            Error::SubcommandRequired => help(),
            _ => {
                eprintln!("tensorman: {}", why);
                let mut source = why.source();
                while let Some(why) = source {
                    eprintln!("    caused by: {}", why);
                    source = why.source();
                }

                exit(1)
            }
        }
    }
}
