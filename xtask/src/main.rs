use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::{
    env,
    process::{Command, Output, Stdio},
};

fn main() -> Result<()> {
    let task = env::args().nth(1);
    match task.as_ref().map(|it| it.as_str()) {
        Some("ruby-ci") => test_ruby_ci()?,
        Some("ruby-bundle") => test_ruby_bundle()?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:
ruby-bundle	Installs all rubygems' bundles
ruby-ci		Runs the CI test suite
"
    )
}

fn test_ruby_bundle() -> Result<()> {
    run("bundle install", "./gems/kleinhirn_loader")?;
    Ok(())
}

fn test_ruby_ci() -> Result<()> {
    run("bundle exec srb tc", "./gems/kleinhirn_loader")?;
    run("bundle exec rubocop", "./gems/kleinhirn_loader")?;
    Ok(())
}

pub fn run(cmdline: &str, dir: &str) -> Result<()> {
    do_run(cmdline, dir, |c| {
        c.stdout(Stdio::inherit());
    })
    .map(|_| ())
}

fn do_run<F>(cmdline: &str, dir: &str, mut f: F) -> Result<Output>
where
    F: FnMut(&mut Command),
{
    eprintln!("\nwill run: {}", cmdline);
    let proj_dir = project_root().join(dir);
    let mut args = cmdline.split_whitespace();
    let exec = args.next().unwrap();
    let mut cmd = Command::new(exec);
    f(cmd
        .args(args)
        .current_dir(proj_dir)
        .stderr(Stdio::inherit()));
    let output = cmd
        .output()
        .with_context(|| format!("running `{}`", cmdline))?;
    if !output.status.success() {
        anyhow::bail!("`{}` exited with {}", cmdline, output.status);
    }
    Ok(output)
}

pub fn project_root() -> PathBuf {
    Path::new(
        &env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned()),
    )
    .ancestors()
    .nth(1)
    .unwrap()
    .to_path_buf()
}
