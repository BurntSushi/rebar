use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process,
    sync::Arc,
};

use {
    anyhow::Context,
    bstr::{BString, ByteSlice},
    once_cell::sync::Lazy,
};

use crate::{
    args::{Filter, Filters},
    util,
};

#[derive(Clone, Debug)]
pub struct Benchmarks {
    pub engines: Engines,
    pub defs: Vec<Definition>,
    pub analysis: BTreeMap<String, String>,
}

impl Benchmarks {
    pub fn from_dir<P: AsRef<Path>>(
        dir: P,
        filters: &Filters,
    ) -> anyhow::Result<Benchmarks> {
        let dir = dir.as_ref();
        let mut wire = WireDefinitions::new();
        wire.load_dir(dir)?;
        wire.check_duplicates()?;
        wire.filter_by_name(&filters.name);
        wire.filter_by_model(&filters.model);
        wire.filter_by_engine(&filters.engine);
        // Now that we've filtered out our benchmarks, we now collect our
        // engines. We are careful to only collect engines that both pass our
        // engine filter and have an actual explicit reference in a benchmark
        // that has passed our filters. We would otherwise wind up getting the
        // version info for every regex engine in some cases even when we don't
        // need to.
        let enginerefs = wire.engine_references(&filters.engine);
        let engines =
            Engines::from_file(dir, |e| enginerefs.contains(&e.name))?;
        let res = Regexes::new(dir, &wire)?;
        let hays = Haystacks::new(dir, &wire)?;
        let mut defs = vec![];
        for wire_def in wire.definitions.iter() {
            let def =
                wire_def.to_definition(filters, &engines, &res, &hays)?;
            defs.push(def);
        }
        Ok(Benchmarks { engines, defs, analysis: wire.all_analysis })
    }

    pub fn find_one<P: AsRef<Path>>(
        dir: P,
        name: &str,
    ) -> anyhow::Result<Definition> {
        // This is a little cumbersome, but we go to war with the army we have.
        let pattern = format!("^(?:{})$", regex_syntax::escape(&name));
        let filters = Filters {
            name: Filter::from_pattern(&pattern)?,
            ..Filters::default()
        };
        let mut defs = Benchmarks::from_dir(dir, &filters)?;
        anyhow::ensure!(
            defs.defs.len() == 1,
            "expected to match 1 benchmark definition but matched {}",
            defs.defs.len(),
        );
        Ok(defs.defs.pop().unwrap())
    }

    #[cfg(test)]
    pub fn from_slice<B: AsRef<[u8]>>(
        engines: &Engines,
        filters: &Filters,
        group: &str,
        data: B,
    ) -> anyhow::Result<Benchmarks> {
        let mut wire = WireDefinitions::new();
        wire.load_slice(group, data.as_ref())?;
        wire.check_duplicates()?;
        wire.filter_by_name(&filters.name);
        wire.filter_by_model(&filters.model);
        wire.filter_by_engine(&filters.engine);
        let res = Regexes::new(Path::new("dummy"), &wire)?;
        let hays = Haystacks::new(Path::new("dummy"), &wire)?;
        let mut defs = vec![];
        for wire_def in wire.definitions.iter() {
            let def =
                wire_def.to_definition(filters, &engines, &res, &hays)?;
            defs.push(def);
        }
        Ok(Benchmarks {
            engines: Engines::default(),
            defs,
            analysis: wire.all_analysis,
        })
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct Engines {
    #[serde(skip)]
    pub by_name: BTreeMap<String, Engine>,
    #[serde(rename = "engine")]
    #[serde(default)] // allows empty TOML files
    pub list: Vec<Engine>,
}

impl Engines {
    #[cfg(test)]
    fn from_list(list: Vec<Engine>) -> Engines {
        let mut engines = Engines { by_name: BTreeMap::new(), list };
        for e in engines.list.iter() {
            engines.by_name.insert(e.name.clone(), e.clone());
        }
        engines
    }

    pub fn from_file(
        parent_dir: &Path,
        mut include: impl FnMut(&Engine) -> bool,
    ) -> anyhow::Result<Engines> {
        let Some(parent) = parent_dir.to_str() else {
            anyhow::bail!(
                "parent directory '{}' of engines.toml contains \
                 invalid UTF-8",
                parent_dir.display(),
            );
        };
        let path = parent_dir.join("engines.toml");
        let data = std::fs::read(&path).with_context(|| {
            format!("failed to read engines from {}", path.display())
        })?;
        let data = std::str::from_utf8(&data).with_context(|| {
            format!("data in {} is not valid UTF-8", path.display())
        })?;
        let mut engines: Engines =
            toml::from_str(&data).with_context(|| {
                format!("error decoding TOML for {}", path.display())
            })?;
        engines.list.retain(|e| include(e));
        for e in engines.list.iter_mut() {
            // Note that validate can modify parts of the engine, e.g.,
            // to populate empty bin names with the path to the current
            // executable.
            e.validate(&parent).with_context(|| {
                format!("validation for engine '{}' failed", e.name)
            })?;
            anyhow::ensure!(
                !engines.by_name.contains_key(&e.name),
                "found duplicate regex engine '{}'",
                e.name,
            );
            engines.by_name.insert(e.name.clone(), e.clone());
        }
        Ok(engines)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct Engine {
    pub name: String,
    pub cwd: Option<String>,
    pub run: Command,
    #[serde(rename = "version")]
    pub version_config: VersionConfig,
    #[serde(skip)]
    pub version: String,
    #[serde(default)]
    pub dependency: Vec<Dependency>,
    #[serde(default)]
    pub build: Vec<Command>,
    #[serde(default)]
    pub clean: Vec<Command>,
}

impl Engine {
    /// Returns true if this engine is missing version information. This
    /// occurs when running the engine's version command fails.
    pub fn is_missing_version(&self) -> bool {
        // It's bush league to be checking this by examining the version string
        // itself, but it's almost certain that 'ERROR' is never a real version
        // string for any regex engine and it's just convenient to do it this
        // way.
        self.version == "ERROR"
    }

    fn validate(&mut self, bench_dir: &str) -> anyhow::Result<()> {
        static RE_ENGINE: Lazy<regex::Regex> = Lazy::new(|| {
            regex::Regex::new(r"^[-A-Za-z0-9]+(/[-A-Za-z0-9]+)*$").unwrap()
        });

        anyhow::ensure!(
            RE_ENGINE.is_match(&self.name),
            "engine name '{}' does not match format '{}'",
            self.name,
            RE_ENGINE.as_str(),
        );
        self.cwd = {
            let cwd = match self.cwd.take() {
                None => Path::new(bench_dir).to_path_buf(),
                Some(cwd) => Path::new(bench_dir).join(cwd),
            };
            // OK because we know bench_dir and the original cwd are valid
            // UTF-8, and joining two valid UTF-8 strings together will always
            // result in valid UTF-8. (Becuase the join delimiter is always
            // ASCII in any reasonable context.)
            Some(cwd.into_os_string().into_string().unwrap())
        };
        let cwd = self.cwd.as_deref();
        self.run.validate(cwd)?;
        if let Some(ref mut run) = self.version_config.run {
            run.validate(cwd)?;
        }
        for cmd in self.build.iter_mut() {
            cmd.validate(cwd)?;
        }
        for cmd in self.clean.iter_mut() {
            cmd.validate(cwd)?;
        }
        self.version = match self.version_config.get() {
            Ok(version) => version,
            Err(err) => {
                log::debug!(
                    "extracted version for engine '{}' failed: {:#}",
                    self.name,
                    err
                );
                "ERROR".to_string()
            }
        };
        Ok(())
    }
}

/// Represents the configuration required to attain the version of a regex
/// engine. This generally follows the process model and requires that the
/// version string is accessible by running a sub-process. It does also permit
/// specifying a regex that can be used to search and extract a substring of
/// the output.
///
/// Alternatively, the version string can just be stored in a flat file.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct VersionConfig {
    pub regex: Option<Regex>,
    pub file: Option<String>,
    #[serde(flatten)]
    pub run: Option<Command>,
}

impl VersionConfig {
    /// Executes the command in this `Version` to get a version string for
    /// a specific regex engine. If 'regex' is present, then it is applied
    /// to the output and the value of the capturing group named 'version'
    /// is returned. Otherwise, the last line of output from the command is
    /// returned and trimmed.
    pub fn get(&self) -> anyhow::Result<String> {
        let out = if let Some(ref file) = self.file {
            BString::from(std::fs::read(file).with_context(|| {
                format!("failed to read version from {}", file)
            })?)
        } else if let Some(ref run) = self.run {
            run.output().context("failed to get version")?
        } else {
            anyhow::bail!("must set either 'file' or 'run' for version config")
        };
        log::trace!("version command output: {:?}", out.as_bstr());
        let re = match self.regex {
            Some(ref re) => re,
            None => {
                let last = match out.lines().last() {
                    None => anyhow::bail!("version stdout was empty"),
                    Some(last) => last,
                };
                return Ok(last.to_str()?.trim().to_string());
            }
        };
        anyhow::ensure!(
            re.capture_names().filter_map(|n| n).any(|n| n == "version"),
            "version regex {:?} does not contain a 'version' capture group",
            re.as_str(),
        );
        let caps = match re.captures(&out) {
            Some(caps) => caps,
            None => anyhow::bail!(
                "version regex {:?} did not match output",
                re.as_str()
            ),
        };
        let m = match caps.name("version") {
            Some(m) => m,
            None => anyhow::bail!(
                "version regex {:?} matched, but 'version' capture did not",
                re.as_str(),
            ),
        };
        let version = m.as_bytes().to_str()?.to_string();
        anyhow::ensure!(
            !version.contains('\n'),
            "version regex {:?} matched a version with a \\n",
            re.as_str(),
        );
        Ok(version)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct Dependency {
    pub regex: Option<Regex>,
    #[serde(flatten)]
    pub run: Command,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct Command {
    pub cwd: Option<String>,
    pub bin: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub envs: Vec<CommandEnv>,
}

impl Command {
    /// This builds and runs this command synchronously. If there was a problem
    /// running the command, then stderr is inspected and its last line is used
    /// to construct the error message returned. (The entire stderr is logged
    /// at debug level however.)
    pub fn output(&self) -> anyhow::Result<BString> {
        util::output(&mut self.command()?)
    }

    /// Builds a standard library 'Command' with the binary name, arguments,
    /// current working directory and environment variables preloaded. This
    /// also handles the case of ensuring that the binary name is not a
    /// relative path when the current working directory is set.
    ///
    /// This is useful when you need fine grained control over how the command
    /// runs. If you just need to synchronously run a command and get a nice
    /// error message out of it, use the 'output' method.
    pub fn command(&self) -> anyhow::Result<process::Command> {
        let bin = self.bin()?;
        let mut cmd = process::Command::new(&bin);
        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }
        cmd.args(self.args.iter());
        cmd.envs(self.envs.iter().map(|e| (&e.name, &e.value)));
        Ok(cmd)
    }

    /// Returns the path to the program that should be invoked to run this
    /// command.
    pub fn bin(&self) -> anyhow::Result<PathBuf> {
        // std says that if we set 'cmd.current_dir', then the behavior is
        // not specified when the binary name is a relative path. It suggests
        // canonicalizing the path instead. But in this context, we don't
        // really know the intent of the user. If they used, say, a command
        // like 'cargo', do we really want to join that with the CWD given and
        // canonicalize it? No, we probably just want to have the normal PATH
        // lookup logic happen.
        //
        // So... for better or worse, we assume that if the 'cwd' is set AND
        // if the binary name contains any path separators, then we join the
        // 'cwd' with the binary name and canonicalize it. Otherwise, we leave
        // it be.
        Ok(match self.cwd {
            None => PathBuf::from(&self.bin),
            Some(ref config_cwd) => {
                if !self.bin.chars().any(std::path::is_separator) {
                    log::trace!(
                        "cwd is set to {:?}, but since binary name {:?} \
                         contains no separators, we're using it as is",
                        config_cwd,
                        self.bin,
                    );
                    PathBuf::from(&self.bin)
                } else {
                    let rebar_cwd = std::env::current_dir()
                        .context("failed to get current directory")?;
                    let bin = rebar_cwd.join(config_cwd).join(&self.bin);
                    log::trace!(
                        "cwd is set to {:?} and rebar is running in {:?}, \
                         since binary name {:?} contains a separator, we \
                         canonicalized it to {:?}",
                        config_cwd,
                        rebar_cwd,
                        self.bin,
                        bin.display(),
                    );
                    bin
                }
            }
        })
    }

    fn validate(&mut self, cwd: Option<&str>) -> anyhow::Result<()> {
        if self.cwd.is_none() {
            self.cwd = cwd.map(|s| s.to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct CommandEnv {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Eq, PartialEq)]
pub struct Definition {
    pub model: String,
    pub name: DefinitionName,
    pub regexes: Arc<[String]>,
    pub regex_path: Option<String>,
    pub options: DefinitionOptions,
    pub haystack: Arc<[u8]>,
    pub haystack_path: Option<String>,
    pub count: Vec<CountEngine>,
    pub engines: Vec<Engine>,
    pub analysis: Option<String>,
}

impl Definition {
    pub fn count(&self, engine: &str) -> anyhow::Result<u64> {
        for ce in self.count.iter() {
            if ce.re.is_match(engine.as_bytes()) {
                return Ok(ce.count);
            }
        }
        anyhow::bail!("no count available for engine '{}'", engine)
    }
}

// We hand-roll our own Debug impl so that the 'haystack' field doesn't vomit
// a huge string (since most haystacks are quite large).
impl std::fmt::Debug for Definition {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let linecount = self.haystack.lines().count();
        let haystack = if linecount <= 1 {
            self.haystack.to_vec()
        } else {
            let mut hay = self.haystack.lines().next().unwrap().to_vec();
            hay.extend_from_slice("[... snip ...]".as_bytes());
            hay
        };
        f.debug_struct("Definition")
            .field("model", &self.model)
            .field("name", &self.name)
            .field("regexes", &self.regexes)
            .field("regex_path", &self.regex_path)
            .field("options", &self.options)
            .field("haystack", &haystack.as_bstr())
            .field("haystack_path", &self.haystack_path)
            .field("count", &self.count)
            .field("engines", &self.engines)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionName {
    pub full: String,
    pub group: String,
    pub local: String,
}

impl DefinitionName {
    pub fn as_str(&self) -> &str {
        &self.full
    }
}

impl std::fmt::Display for DefinitionName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.full.fmt(f)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CountEngine {
    pub re: Regex,
    pub engine: String,
    pub count: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DefinitionOptions {
    #[serde(default)]
    pub case_insensitive: bool,
    #[serde(default)]
    pub unicode: bool,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDefinitions {
    #[serde(rename = "bench")]
    #[serde(default)] // allows empty TOML files
    definitions: Vec<WireDefinition>,
    analysis: Option<String>,
    #[serde(skip)]
    all_analysis: BTreeMap<String, String>,
}

impl WireDefinitions {
    /// Create an empty set of benchmark definitions.
    fn new() -> WireDefinitions {
        WireDefinitions {
            definitions: vec![],
            analysis: None,
            all_analysis: BTreeMap::new(),
        }
    }

    /// Load all benchmark definitions from the given directory recursively.
    /// Any file with a 'toml' extension is read and deserialized. The
    /// top-level 'haystacks' and 'regexes' directories are skipped.
    fn load_dir(&mut self, dir: &Path) -> anyhow::Result<()> {
        let dir = dir.join("definitions");
        for result in walkdir::WalkDir::new(&dir).sort_by_file_name() {
            let dent = result?;
            if !dent.file_type().is_file() {
                continue;
            }
            let ext = match dent.path().extension() {
                None => continue,
                Some(ext) => ext,
            };
            if ext != "toml" {
                continue;
            }
            self.load_file(&dir, dent.path())?;
        }
        Ok(())
    }

    /// Load the benchmark definitions from the TOML file at the given path.
    fn load_file(&mut self, dir: &Path, path: &Path) -> anyhow::Result<()> {
        let suffix = path.strip_prefix(dir).with_context(|| {
            format!(
                "failed to strip prefix from {} with base {}",
                path.display(),
                dir.display(),
            )
        })?;
        let group = suffix
            .with_extension("")
            .to_str()
            .with_context(|| {
                format!("invalid UTF-8 found in {}", path.display())
            })?
            // If we're on Windows and get \ path separators,
            // change them to /.
            .replace("\\", "/")
            .to_string();
        let data = std::fs::read(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        self.load_slice(&group, &data)
            .with_context(|| format!("error loading {}", path.display()))?;
        Ok(())
    }

    /// Load the benchmark definitions from the TOML data. The group given is
    /// assigned to every benchmark definition. Typically the group name is the
    /// stem of the file name.
    fn load_slice(&mut self, group: &str, data: &[u8]) -> anyhow::Result<()> {
        let data = std::str::from_utf8(data)?;
        let top: WireDefinitions = toml::from_str(data)
            .with_context(|| format!("error decoding TOML for '{}'", group))?;
        for mut def in top.definitions {
            def.group = group.to_string();
            def.name = format!("{}/{}", def.group, def.local);
            self.definitions.push(def);
        }
        if let Some(ref analysis) = top.analysis {
            self.all_analysis.insert(group.to_string(), analysis.to_string());
        }
        Ok(())
    }

    /// Looks for benchmarks with duplicate names, and if one exists, returns
    /// an error.
    ///
    /// This should only be called after all benchmarks have been loaded.
    fn check_duplicates(&self) -> anyhow::Result<()> {
        let mut seen = BTreeSet::new();
        for def in self.definitions.iter() {
            anyhow::ensure!(
                !seen.contains(&def.name),
                "found at least two benchmarks with the same name '{}'",
                def.name,
            );
            seen.insert(def.name.clone());
        }
        Ok(())
    }

    /// Retain only the definitions that pass the given filter applied to the
    /// name of each definition.
    fn filter_by_name(&mut self, filter: &Filter) {
        self.definitions.retain(|def| filter.include(&def.name));
    }

    /// Retain only the definitions that pass the given filter applied to the
    /// model of each definition.
    fn filter_by_model(&mut self, filter: &Filter) {
        self.definitions.retain(|def| filter.include(&def.model));
    }

    /// Retain only the definitions that pass the given filter applied to the
    /// engines of each definition. A definition is kept only when it has at
    /// least one engine that matches the given filter.
    fn filter_by_engine(&mut self, filter: &Filter) {
        self.definitions.retain(|def| {
            // This is kind of a weird case where a benchmark has no engines
            // given. We let it pass through here purely because it will
            // provoke an error at a higher abstraction layer, which we want to
            // happen instead of silently ignoring the benchmark.
            if def.engines.is_empty() {
                return true;
            }
            for engine in def.engines.iter() {
                if filter.include(engine) {
                    return true;
                }
            }
            false
        });
    }

    /// Returns a set of all engines that both pass the given filter and
    /// have an explicit reference in these benchmarks.
    fn engine_references(&self, filter: &Filter) -> BTreeSet<String> {
        let mut set = BTreeSet::new();
        for def in self.definitions.iter() {
            for engine in def.engines.iter() {
                if !filter.include(engine) {
                    continue;
                }
                set.insert(engine.clone());
            }
        }
        set
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct WireDefinition {
    model: String,
    #[serde(skip)]
    name: String,
    #[serde(skip)]
    group: String,
    #[serde(rename = "name")]
    local: String,
    regex: WireRegex,
    #[serde(flatten)]
    options: DefinitionOptions,
    haystack: WireHaystack,
    count: WireCount,
    engines: Vec<String>,
    analysis: Option<String>,
}

impl WireDefinition {
    fn to_definition(
        &self,
        filters: &Filters,
        engines: &Engines,
        res: &Regexes,
        hays: &Haystacks,
    ) -> anyhow::Result<Definition> {
        let def = Definition {
            model: self.model.clone(),
            name: self.name()?,
            regexes: self.regexes(res)?,
            regex_path: self.regex_path(),
            options: self.options.clone(),
            haystack: self.haystack(hays)?,
            haystack_path: self.haystack_path(),
            count: self.count()?,
            engines: self.engines(filters, engines)?,
            analysis: self.analysis.clone(),
        };
        Ok(def)
    }

    fn name(&self) -> anyhow::Result<DefinitionName> {
        static RE_GROUP: Lazy<regex::Regex> =
            Lazy::new(|| regex::Regex::new(r"^[-A-Za-z0-9]+$").unwrap());
        static RE_NAME: Lazy<regex::Regex> =
            Lazy::new(|| regex::Regex::new(r"^[-A-Za-z0-9]+$").unwrap());

        for piece in self.group.split("/") {
            anyhow::ensure!(
                RE_GROUP.is_match(piece),
                "part '{}' from group name '{}' does not match format '{}' \
                 (group name is usually derived from TOML file name)",
                piece,
                self.group,
                RE_GROUP.as_str(),
            );
        }
        anyhow::ensure!(
            RE_NAME.is_match(&self.local),
            "benchmark name '{}' does not match format '{}'",
            self.name,
            RE_NAME.as_str(),
        );
        Ok(DefinitionName {
            full: self.name.clone(),
            group: self.group.clone(),
            local: self.local.clone(),
        })
    }

    fn engines(
        &self,
        filters: &Filters,
        engines: &Engines,
    ) -> anyhow::Result<Vec<Engine>> {
        let mut resolved = vec![];
        for name in self.engines.iter() {
            if !filters.engine.include(name) {
                continue;
            }
            let e = match engines.by_name.get(name) {
                Some(e) => e.clone(),
                None => anyhow::bail!(
                    "could not find regex engine '{}' for benchmark '{}'",
                    name,
                    self.name,
                ),
            };
            if filters.ignore_missing_engines && e.is_missing_version() {
                continue;
            }
            resolved.push(e);
        }
        Ok(resolved)
    }

    fn regexes(&self, res: &Regexes) -> anyhow::Result<Arc<[String]>> {
        let patterns: Arc<[String]> = match self.regex {
            WireRegex::Inline(ref inline) => Arc::from(inline.patterns()),
            WireRegex::Full(ref full) => {
                if let Some(key) = RegexKey::from_wire(full) {
                    anyhow::ensure!(
                        full.patterns.is_none(),
                        "benchmark '{}' defines both 'patterns' and 'path'",
                        self.name,
                    );
                    // Every "full" definition that can have a key constructed
                    // is guaranteed to be in our 'res' map, and if it isn't,
                    // there's a bug somewhere in this module.
                    return Ok(res.map.get(&key).unwrap().clone());
                }
                // There's a key if and only if the actual regex is in a file.
                assert!(full.path.is_none());
                let patterns = match full.patterns {
                    None => anyhow::bail!(
                        "missing regex patterns for benchmark '{}'",
                        self.name
                    ),
                    Some(ref inline) => inline.patterns(),
                };
                Arc::from(full.options.transform_from_inline(patterns))
            }
        };
        Ok(patterns)
    }

    fn regex_path(&self) -> Option<String> {
        match self.regex {
            WireRegex::Inline(_) => None,
            WireRegex::Full(ref full) => full.path.clone(),
        }
    }

    fn haystack(&self, hays: &Haystacks) -> anyhow::Result<Arc<[u8]>> {
        match self.haystack {
            WireHaystack::Inline(ref haystack) => {
                Ok(Arc::from(haystack.as_bytes()))
            }
            WireHaystack::Full(ref full) => {
                if let Some(key) = HaystackKey::from_wire(full) {
                    anyhow::ensure!(
                        full.contents.is_none(),
                        "benchmark '{}' defines both 'contents' and 'path'",
                        self.name,
                    );
                    // Every "full" definition that can have a key constructed
                    // is guaranteed to be in our 'hays' map, and if it isn't,
                    // there's a bug somewhere in this module.
                    return Ok(hays.map.get(&key).unwrap().clone());
                }
                // There's a key if and only if the actual haystack is in a
                // file.
                assert!(full.path.is_none());
                let haystack = match full.contents {
                    None => anyhow::bail!(
                        "missing haystack for benchmark '{}'",
                        self.name
                    ),
                    Some(ref haystack) => haystack,
                };
                Ok(Arc::from(full.options.transform(haystack.as_bytes())))
            }
        }
    }

    fn haystack_path(&self) -> Option<String> {
        match self.haystack {
            WireHaystack::Inline(_) => None,
            WireHaystack::Full(ref full) => full.path.clone(),
        }
    }

    fn count(&self) -> anyhow::Result<Vec<CountEngine>> {
        match self.count {
            WireCount::Engines(ref engine_counts) => {
                let mut counts = vec![];
                for wire in engine_counts.iter() {
                    let pat = format!("^(?:{})$", wire.engine);
                    let re = regex::bytes::Regex::new(&pat).context(
                        "failed to parse engine count name as regex",
                    )?;
                    counts.push(CountEngine {
                        re: Regex(re),
                        engine: wire.engine.clone(),
                        count: wire.count,
                    });
                }
                Ok(counts)
            }
            WireCount::All(count) => Ok(vec![CountEngine {
                re: Regex(regex::bytes::Regex::new(r"^.*$").unwrap()),
                engine: r".*".to_string(),
                count,
            }]),
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
enum WireCount {
    Engines(Vec<WireCountEngine>),
    All(u64),
}

#[derive(Clone, Debug, serde::Deserialize)]
struct WireCountEngine {
    engine: String,
    count: u64,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
enum WireRegex {
    Inline(WireRegexInline),
    Full(WireRegexFull),
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
enum WireRegexInline {
    One(String),
    Many(Vec<String>),
}

impl WireRegexInline {
    fn patterns(&self) -> &[String] {
        match *self {
            WireRegexInline::One(ref p) => std::slice::from_ref(p),
            WireRegexInline::Many(ref ps) => ps,
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
struct WireRegexFull {
    patterns: Option<WireRegexInline>,
    path: Option<String>,
    #[serde(flatten)]
    options: WireRegexOptions,
}

#[derive(
    Clone, Debug, Default, Eq, PartialEq, PartialOrd, Ord, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
struct WireRegexOptions {
    #[serde(default)]
    literal: bool,
    #[serde(default)]
    per_line: WireRegexOptionPerLine,
    prepend: Option<String>,
    append: Option<String>,
}

impl WireRegexOptions {
    fn transform_from_file(&self, raw: &str) -> Vec<String> {
        match self.per_line {
            WireRegexOptionPerLine::None => {
                self.transform(vec![raw.trim().to_string()])
            }
            WireRegexOptionPerLine::Alternate => {
                let mut pats = raw.lines().map(|p| p.to_string()).collect();
                pats = self.transform(pats);
                pats =
                    pats.into_iter().map(|p| format!("(?:{})", p)).collect();
                vec![pats.join("|")]
            }
            WireRegexOptionPerLine::Pattern => {
                self.transform(raw.lines().map(|x| x.to_string()).collect())
            }
        }
    }

    fn transform_from_inline(&self, patterns: &[String]) -> Vec<String> {
        self.transform(patterns.to_vec())
    }

    fn transform(&self, mut pats: Vec<String>) -> Vec<String> {
        if self.literal {
            for p in pats.iter_mut() {
                *p = regex_syntax::escape(p);
            }
        }
        if let Some(ref prepend) = self.prepend {
            for p in pats.iter_mut() {
                *p = format!("{}{}", prepend, p);
            }
        }
        if let Some(ref append) = self.append {
            for p in pats.iter_mut() {
                p.push_str(append);
            }
        }
        pats
    }
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireRegexOptionPerLine {
    None,
    Alternate,
    Pattern,
}

impl Default for WireRegexOptionPerLine {
    fn default() -> WireRegexOptionPerLine {
        WireRegexOptionPerLine::None
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
enum WireHaystack {
    Inline(String),
    Full(WireHaystackFull),
}

#[derive(Clone, Debug, serde::Deserialize)]
struct WireHaystackFull {
    contents: Option<String>,
    path: Option<String>,
    #[serde(flatten)]
    options: WireHaystackOptions,
}

#[derive(
    Clone, Debug, Default, Eq, PartialEq, PartialOrd, Ord, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
struct WireHaystackOptions {
    #[serde(default)]
    utf8_lossy: bool,
    #[serde(default)]
    trim: bool,
    line_start: Option<usize>,
    line_end: Option<usize>,
    repeat: Option<usize>,
    prepend: Option<String>,
    append: Option<String>,
}

impl WireHaystackOptions {
    fn transform(&self, raw: &[u8]) -> Vec<u8> {
        let mut raw = raw.to_vec();
        if self.utf8_lossy {
            raw = String::from_utf8_lossy(&raw).into_owned().into_bytes();
        }
        if self.trim {
            raw = raw.trim_with(|c| c.is_whitespace()).to_vec();
        }
        match (self.line_start, self.line_end) {
            (None, None) => {}
            (Some(s), None) => {
                raw = bstr::concat(raw.lines_with_terminator().skip(s));
            }
            (None, Some(e)) => {
                raw = bstr::concat(raw.lines_with_terminator().take(e));
            }
            (Some(s), Some(e)) => {
                raw =
                    bstr::concat(raw.lines_with_terminator().take(e).skip(s));
            }
        }
        if let Some(n) = self.repeat {
            raw = raw.repeat(n);
        }
        if let Some(ref prepend) = self.prepend {
            raw.splice(0..0, prepend.as_bytes().iter().copied());
        }
        if let Some(ref append) = self.append {
            raw.extend_from_slice(append.as_bytes());
        }
        raw
    }
}

#[derive(Clone, Debug)]
struct Regexes {
    dir: PathBuf,
    map: BTreeMap<RegexKey, Arc<[String]>>,
}

impl Regexes {
    fn new(
        bench_dir: &Path,
        defs: &WireDefinitions,
    ) -> anyhow::Result<Regexes> {
        let mut res =
            Regexes { dir: bench_dir.join("regexes"), map: BTreeMap::new() };
        for def in defs.definitions.iter() {
            if let WireRegex::Full(ref full) = def.regex {
                res.add(full).with_context(|| {
                    format!(
                        "failed to add regex from benchmark '{}'",
                        def.name
                    )
                })?;
            }
        }
        Ok(res)
    }

    fn add(&mut self, full: &WireRegexFull) -> anyhow::Result<()> {
        // We don't put inline regexes into this map because they are already
        // stored inline to the benchmark definition and are generally assumed
        // to be small enough that reuse doesn't matter. Moreover, there
        // isn't any sensible way to create a key for an inline regex that is
        // independent from the benchmark itself.
        let key = match RegexKey::from_wire(full) {
            None => return Ok(()),
            Some(key) => key,
        };
        if self.map.contains_key(&key) {
            return Ok(());
        }
        let path = self.dir.join(&key.path);
        let raw = std::fs::read_to_string(&path).with_context(|| {
            format!("failed to read regex at {}", path.display())
        })?;
        let patterns = full.options.transform_from_file(&raw);
        self.map.insert(key, Arc::from(patterns));
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct Haystacks {
    dir: PathBuf,
    map: BTreeMap<HaystackKey, Arc<[u8]>>,
}

impl Haystacks {
    fn new(
        bench_dir: &Path,
        defs: &WireDefinitions,
    ) -> anyhow::Result<Haystacks> {
        let mut hays = Haystacks {
            dir: bench_dir.join("haystacks"),
            map: BTreeMap::new(),
        };
        for def in defs.definitions.iter() {
            if let WireHaystack::Full(ref full) = def.haystack {
                hays.add(full).with_context(|| {
                    format!(
                        "failed to add haystack from benchmark '{}'",
                        def.name
                    )
                })?;
            }
        }
        Ok(hays)
    }

    fn add(&mut self, full: &WireHaystackFull) -> anyhow::Result<()> {
        // We don't put inline haystacks into this map because they are already
        // stored inline to the benchmark definition and are generally assumed
        // to be small enough that reuse doesn't matter. Moreover, there isn't
        // any sensible way to create a key for an inline haystack that is
        // independent from the benchmark itself.
        let key = match HaystackKey::from_wire(full) {
            None => return Ok(()),
            Some(key) => key,
        };
        if self.map.contains_key(&key) {
            return Ok(());
        }
        let path = self.dir.join(&key.path);
        let raw = std::fs::read(&path).with_context(|| {
            format!("failed to read haystack at {}", path.display())
        })?;
        let haystack = full.options.transform(&raw);
        self.map.insert(key, Arc::from(haystack));
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct RegexKey {
    path: String,
    options: WireRegexOptions,
}

impl RegexKey {
    fn from_wire(full: &WireRegexFull) -> Option<RegexKey> {
        Some(RegexKey {
            path: full.path.clone()?,
            options: full.options.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct HaystackKey {
    path: String,
    options: WireHaystackOptions,
}

impl HaystackKey {
    fn from_wire(full: &WireHaystackFull) -> Option<HaystackKey> {
        Some(HaystackKey {
            path: full.path.clone()?,
            options: full.options.clone(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Regex(regex::bytes::Regex);

impl Eq for Regex {}

impl PartialEq for Regex {
    fn eq(&self, other: &Regex) -> bool {
        self.as_str() == other.as_str()
    }
}

impl std::ops::Deref for Regex {
    type Target = regex::bytes::Regex;
    fn deref(&self) -> &regex::bytes::Regex {
        &self.0
    }
}

impl<'de> serde::Deserialize<'de> for Regex {
    fn deserialize<D>(de: D) -> Result<Regex, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Error, Visitor};

        struct RegexVisitor;

        impl<'de> Visitor<'de> for RegexVisitor {
            type Value = Regex;

            fn expecting(
                &self,
                f: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                f.write_str("a regular expression pattern")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Regex, E> {
                regex::bytes::Regex::new(v)
                    .map(Regex)
                    .map_err(|err| E::custom(err.to_string()))
            }
        }

        de.deserialize_str(RegexVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(group: &str, local: &str) -> DefinitionName {
        DefinitionName {
            full: format!("{group}/{local}"),
            group: group.to_string(),
            local: local.to_string(),
        }
    }

    fn regexes(
        patterns: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Arc<[String]> {
        patterns.into_iter().map(|p| p.as_ref().to_string()).collect()
    }

    fn haystack(haystack: impl AsRef<[u8]>) -> Arc<[u8]> {
        Arc::from(haystack.as_ref())
    }

    fn engines(
        names: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Vec<Engine> {
        names
            .into_iter()
            .map(|n| Engine {
                name: n.as_ref().to_string(),
                cwd: None,
                run: Command {
                    cwd: None,
                    bin: "rebar".to_string(),
                    args: vec![],
                    envs: vec![],
                },
                version: "0.0.0".to_string(),
                version_config: VersionConfig {
                    regex: None,
                    file: None,
                    run: None,
                },
                dependency: vec![],
                build: vec![],
                clean: vec![],
            })
            .collect()
    }

    fn count_all(count: u64) -> Vec<CountEngine> {
        vec![CountEngine {
            re: Regex(regex::bytes::Regex::new(r"^.*$").unwrap()),
            engine: r".*".to_string(),
            count,
        }]
    }

    #[test]
    fn basic() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = 'foo'
case-insensitive = true
unicode = true
haystack = "quuxfoo"
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: regexes(["foo"]),
            regex_path: None,
            options: DefinitionOptions {
                case_insensitive: true,
                unicode: true,
            },
            haystack: haystack("quuxfoo"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn regex_empty() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = []
haystack = "quuxfoo"
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: Arc::from([]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack("quuxfoo"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn regex_many() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = ['foo', 'bar']
haystack = "quuxfoo"
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: regexes(["foo", "bar"]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack("quuxfoo"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn regex_full_inline() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = { patterns = "foo" }
haystack = "quuxfoo"
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: regexes(["foo"]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack("quuxfoo"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn regex_full_inline_empty() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = { patterns = [] }
haystack = "quuxfoo"
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: Arc::from([]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack("quuxfoo"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn regex_full_inline_many() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = { patterns = ["foo", "bar"] }
haystack = "quuxfoo"
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: regexes(["foo", "bar"]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack("quuxfoo"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn regex_full_inline_literal() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = { patterns = "f*oo", literal = true }
haystack = "quuxfoo"
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: regexes([r"f\*oo"]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack("quuxfoo"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn regex_full_inline_literal_many() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = { patterns = ["f*oo", "b*ar"], literal = true }
haystack = "quuxfoo"
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: regexes([r"f\*oo", r"b\*ar"]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack("quuxfoo"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn haystack_full_inline() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = 'foo'
haystack = { contents = "quuxfoo" }
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: regexes(["foo"]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack("quuxfoo"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn haystack_full_inline_trim() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = 'foo'
haystack = { contents = " quuxfoo\n", trim = true }
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: regexes(["foo"]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack("quuxfoo"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn haystack_full_inline_prepend() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = 'foo'
haystack = { contents = "quuxfoo", prepend = "bar" }
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: regexes(["foo"]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack("barquuxfoo"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn haystack_full_inline_append() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = 'foo'
haystack = { contents = "quuxfoo", append = "bar" }
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: regexes(["foo"]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack("quuxfoobar"),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn haystack_full_inline_trim_comes_first() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = 'foo'
haystack = { contents = "quuxfoo", trim = true, prepend = " ", append = " " }
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(engines(["regex/api"]));
        let filters = Filters::default();
        let benches =
            Benchmarks::from_slice(&es, &filters, "group", raw).unwrap();
        assert_eq!(1, benches.defs.len());
        let got = &benches.defs[0];
        let expected = Definition {
            model: "count".to_string(),
            name: name("group", "test"),
            regexes: regexes(["foo"]),
            regex_path: None,
            options: DefinitionOptions::default(),
            haystack: haystack(" quuxfoo "),
            haystack_path: None,
            count: count_all(1),
            engines: engines(["regex/api"]),
            analysis: None,
        };
        assert_eq!(expected, *got);
    }

    #[test]
    fn error_empty_engines() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
regex = 'foo'
haystack = "quuxfoo"
engines = []
count = 1
"#;
        let es = Engines::from_list(vec![]);
        let filters = Filters::default();
        assert!(Benchmarks::from_slice(&es, &filters, "group", raw).is_err());
    }

    #[test]
    fn error_no_regex() {
        let raw = r#"
[[bench]]
model = "count"
name = "test"
haystack = "quuxfoo"
engines = ["regex/api"]
count = 1
"#;
        let es = Engines::from_list(vec![]);
        let filters = Filters::default();
        assert!(Benchmarks::from_slice(&es, &filters, "group", raw).is_err());
    }

    #[test]
    fn error_regex_redux() {
        // regex-redux requires no 'regex' field as it hard-codes its own.
        let raw = r#"
[[bench]]
model = "regex-redux"
name = "test"
regex = ["foo"]
haystack = "quuxfoo"
engines = ["regex/api"]
"#;
        let es = Engines::from_list(vec![]);
        let filters = Filters::default();
        assert!(Benchmarks::from_slice(&es, &filters, "group", raw).is_err());
    }
}
