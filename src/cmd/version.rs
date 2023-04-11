use std::io::Write;

use lexopt::Arg;

use crate::util;

fn usage() -> String {
    format!(
        "\
Print the version of this rebar command.

USAGE:
    rebar version

",
    )
    .trim()
    .to_string()
}

pub fn run(p: &mut lexopt::Parser) -> anyhow::Result<()> {
    while let Some(arg) = p.next()? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                anyhow::bail!("{}", usage())
            }
            _ => return Err(arg.unexpected().into()),
        }
    }

    let mut wtr = std::io::stdout();
    writeln!(wtr, "{}", util::version())?;
    Ok(())
}
