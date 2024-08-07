# Contributing

I'm eager to hear all your feedback and suggestions!
Just open a [GitHub issue](https://github.com/manforowicz/gday/issues)
and include as many details as you can.
For example, try running with `--verbosity debug` or `--verbosity trace`
and paste the log into your issue.

## Contributing code

Learn how to contribute code by following GitHub's
[contributing to a project](https://docs.github.com/en/get-started/exploring-projects-on-github/contributing-to-a-project)
guide.

Verify your code passes tests by running the cargo commands listed
in [/other/pre-push](/other/pre-push).

## Running a server

One of the strengths of gday is its decentralized nature.
Want to add your own server to the list of
[default servers](https://docs.rs/gday_hole_punch/latest/gday_hole_punch/server_connector/constant.DEFAULT_SERVERS.html)?
Read the instructions in [/gday_server/README.md](/gday_server/README.md).

## Technical

[release.yml](/.github/workflows/release.yml) is automatically generated by [cargo dist](https://crates.io/crates/cargo-dist).
