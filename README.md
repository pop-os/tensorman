# Tensorflow Container Manager

Packaging Tensorflow for Linux distributions is notoriously difficult, if not impossible. Every release of Tensorflow is accommodated by a myriad of possible build configurations, which requires building many variants of Tensorflow for each Tensorflow release. To make matters worse, each new version of Tensorflow will depend on a wide number of shared dependencies which may not be supported on older versions of a Linux distribution that is still actively supported by the distribution maintainers.

To solve this problem, the Tensorflow project provides official Docker container builds, which allows Tensorflow to operate in an isolated environment that is contained from the rest of the system. This virtual environment can operate independent of the base system, allowing you to use any version of Tensorflow on any version of a Linux distribution that supports the Docker runtime.

However, configuring and managing Docker containers for Tensorflow using the `docker` command line is currently tedious, and managing multiple versions for different projects is even moreso. To solve this problem for our users, we have developed `tensorman` as a convenient tool to manage the installation and execution of Tensorflow Docker containers. It condenses the command-line soup into a set of simple commands that are easy to memorize.

## Comparison to Docker Command

Take the following Docker invocation as an example:

```
docker run -u $UID:$UID -v $PWD:/project -w /project \
    --runtime=nvidia --it --rm tensorflow/tensorflow:latest-gpu \
    python ./script.py
```

This designates for the latest version of Tensorflow with GPU support to be used, mounting the working directory to `/project`, launching the container with the current user account, and and executing `script.py` with the Python binary in the container. With `tensorman`, we can achieve the same with:

```
tensorman run --gpu python -- ./script.py
```

Which defaults to the latest version, and whose version and tag variants can be set as defaults per-run, per-project, or user-wide.

## Installing/Updating Containers

By default, docker will automatically install a container when running a container that it is not already installed. However, if you would like to install a container beforehand, you may do so using the `pull` subcommand.

```
tensorman pull 1.14.0
tensorman pull latest
```

## Running commands in containers

The `run` subcommand allows you to execute a command from within the container. This could be the `bash` shell, for an interactive session inside the container, or the program / compiler which you wish to run.

```
# Default container version with Bash prompt
tensorman run bash

# Default container version with Python script
tensorman run python -- script.py

# Default container version with GPU support
tensorman run --gpu bash

# With GPU, Python3, and Juypyter support
tensorman run --gpu --python3 --jupyter bash
```

## Setting the container version

Taking inspiration from [rustup], there are methods to set the container version per-run, per-project, and per-user. The per-run version always takes priority over a per-project definition, which takes priority over the per-user configuration.

[rustup]: https://rustup.rs

### Setting per-run

If a version is specified following a `+` argument, `tensorman` will prefer this version.

```
tensorman +1.14.0 run --python3 --gpu bash
```

### Setting per-project

If the `tensorflow-toolchain` file is found in the working directory, the release tag and tag variants defined within it will override the user-wide default version. This is useful for setting the tensorflow project per-project.

```
# cat tensor-toolchain
1.14.0 gpu python3
```

### Setting per-user

you can set a default version user-wide using the `default` subcommand. This version of Tensorflow will be launched whenever you use the `tensorman run` command.

```
tensorman default 1.14.0
tensorman default latest gpu python3
tensorman default nightly
```

> By default, `tensorman` will use `latest` as the default per-user version tag.

## Showing the active container version

If you would like to know which container will be used when launched from the current working directory, you can use the `show` command.

```
tensorman show
```

## Removing containers

Having many containers installed simultaneously on the same system can quickly use a lot of disk storage. If you find yourself in need of culling the containers installed on your system, you may do so with the `remove` command.

```
tensorman remove 1.14.0
tensorman remove latest
tensorman remove 481cb7ea88260404
```

## Listing installed containers

To aid in discovering what containers are installed on the system, the `list` subcommand is available.

```
tensorman list
```


## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.