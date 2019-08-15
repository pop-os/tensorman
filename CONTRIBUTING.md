# Contributor Guidelines

The three rules that contributors MUST follow:

- Discuss a feature before implementing it
- `cargo +nightly fmt` should always be run on code before creating a commit
- Commits should follow the [Conventional Commit] guidelines
    - A small change in one commit is exempt from this rule

Things that contributors SHOULD NOT be worried about:

- Code style: `cargo fmt` renders this a nonissue
- Code acceptance: in most circumstances, safe Rust code will be accepted on the spot
- Commit messages: maintainers can revise them before or after merging

Things that contributors SHOULD be aware of:

- `rustup` should be the preferred tool for Rust developers
- `cargo clippy` can point out the majority common mistakes in Rust code
- `sbuild` can be used to verified that debian packages build correctly
- `unsafe` is explicitly disallowed, unless otherwise permitted by a maintainer

[conventional commit]: https://www.conventionalcommits.org/en/v1.0.0-beta.4/
