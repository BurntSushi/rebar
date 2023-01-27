use std::io::Write;

use {
    anyhow::Context,
    lexopt::{Arg, ValueExt},
    regex_automata::nfa::thompson::{pikevm::PikeVM, Compiler},
    regex_syntax::{
        ast::{parse::ParserBuilder, Ast},
        hir::translate::TranslatorBuilder,
    },
};

fn main() -> anyhow::Result<()> {
    let mut p = lexopt::Parser::from_env();
    let engine = match p.next()? {
        None => anyhow::bail!("missing engine name"),
        Some(Arg::Value(v)) => v.string().context("<engine>")?,
        Some(arg) => {
            return Err(
                anyhow::Error::from(arg.unexpected()).context("<engine>")
            );
        }
    };
    anyhow::ensure!(
        engine == "ast" || engine == "hir",
        "unrecognized engine '{}'",
        engine,
    );
    let (mut quiet, mut version) = (false, false);
    while let Some(arg) = p.next()? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                anyhow::bail!("main [--version | --quiet]")
            }
            Arg::Short('q') | Arg::Long("quiet") => {
                quiet = true;
            }
            Arg::Long("version") => {
                version = true;
            }
            _ => return Err(arg.unexpected().into()),
        }
    }
    if version {
        writeln!(std::io::stdout(), "{}", env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }
    let b = klv::Benchmark::read(std::io::stdin())
        .context("failed to read KLV data from <stdin>")?;
    anyhow::ensure!(
        b.model == "compile",
        "unsupported benchmark model '{}'",
        b.model,
    );
    let samples = match &*engine {
        "ast" => model_compile_ast(&b)?,
        "hir" => model_compile_hir(&b)?,
        _ => unreachable!(),
    };
    if !quiet {
        let mut stdout = std::io::stdout().lock();
        for s in samples.iter() {
            writeln!(stdout, "{},{}", s.duration.as_nanos(), s.count)?;
        }
    }
    Ok(())
}

fn model_compile_ast(
    b: &klv::Benchmark,
) -> anyhow::Result<Vec<timer::Sample>> {
    let pattern = b.regex.one()?;
    timer::run_and_count(
        b,
        |ast: Ast| {
            let hir = TranslatorBuilder::new()
                .utf8(false)
                .unicode(b.regex.unicode)
                .case_insensitive(b.regex.case_insensitive)
                .build()
                .translate(pattern, &ast)?;
            let nfa = Compiler::new().build_from_hir(&hir)?;
            let re = PikeVM::builder().build_from_nfa(nfa)?;
            let mut cache = re.create_cache();
            Ok(re.find_iter(&mut cache, &b.haystack).count())
        },
        || ParserBuilder::new().build().parse(pattern).map_err(|e| e.into()),
    )
}

fn model_compile_hir(
    b: &klv::Benchmark,
) -> anyhow::Result<Vec<timer::Sample>> {
    let pattern = b.regex.one()?;
    let ast = ParserBuilder::new().build().parse(&pattern)?;
    let mut translator = TranslatorBuilder::new()
        .utf8(false)
        .unicode(b.regex.unicode)
        .case_insensitive(b.regex.case_insensitive)
        .build();
    timer::run_and_count(
        b,
        |hir| {
            let nfa = Compiler::new().build_from_hir(&hir)?;
            let re = PikeVM::builder().build_from_nfa(nfa)?;
            let mut cache = re.create_cache();
            Ok(re.find_iter(&mut cache, &b.haystack).count())
        },
        || translator.translate(pattern, &ast).map_err(|e| e.into()),
    )
}
