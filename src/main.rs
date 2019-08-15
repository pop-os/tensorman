#[macro_use]
extern crate err_derive;
#[macro_use]
extern crate log;

mod image;

use self::image::{Image, TagVariants};
use rs_docker::Docker;
use std::{env::args, io, process::exit};

#[derive(Debug, Error)]
pub enum Error {
    #[error(display = "failed to establish a connection to the Docker service")]
    DockerConnection(#[error(cause)] io::Error),
    #[error(display = "failed to fetch list of docker containers")]
    DockerContainers(#[error(cause)] io::Error),
    #[error(display = "subcommand requires at least one argument")]
    RequiresArgument,
    #[error(display = "subcommand failed")]
    Subcommand(#[error(cause)] io::Error),
    #[error(display = "missing subcommand argument")]
    SubcommandRequired,
}

fn main_() -> Result<(), Error> {
    let mut docker =
        Docker::connect("unix:///var/run/docker.sock").map_err(Error::DockerConnection)?;

    let arguments: Vec<String> = args().skip(1).collect();
    let mut arguments = arguments.iter();
    let mut tag = "latest";

    // Allow the first argument, if it begins with `+`, to override the tag.
    let mut subcommand = arguments.next().and_then(|argument| {
        if argument.starts_with('+') {
            tag = &argument[1..];
            arguments.next().map(String::as_str)
        } else {
            Some(argument.as_str())
        }
    });

    let subcommand = subcommand.take().ok_or(Error::SubcommandRequired)?;

    let mut variants = TagVariants::empty();
    let mut subcommand_args = Vec::new();

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--" => break,
            "--gpu" => variants |= TagVariants::GPU,
            "--py3" => variants |= TagVariants::PY3,
            "--jupyter" => variants |= TagVariants::JUPYTER,
            argument => subcommand_args.push(argument),
        }
    }

    let image = Image { tag, variants };

    let result = match subcommand {
        "default" => unimplemented!(),
        "list" => {
            let containers = docker.get_containers(false).map_err(Error::DockerContainers)?;
            unimplemented!()
        }
        "pull" => image.pull(),
        "remove" => unimplemented!(),
        "run" => {
            if subcommand_args.len() != 1 {
                return Err(Error::RequiresArgument);
            }

            let cmd: &str = subcommand_args[0];
            let args: Vec<&str> = arguments.map(|x| x.as_str()).collect();

            image.run(cmd, if args.is_empty() { None } else { Some(&args) })
        }
        _ => panic!("unknown subcommand: {}", subcommand),
    };

    result.map_err(Error::Subcommand)
}

fn main() {
    if let Err(why) = main_() {
        eprintln!("{}", why);
        exit(1)
    }
}
