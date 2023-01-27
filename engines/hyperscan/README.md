This directory contains a Rust runner program for benchmarking [Hyperscan].
Hyperscan was originally built for deep packet inspection (DPI), is currently
developed by Intel and uses finite automata to implement a subset of the PCRE
regex engine's functionality.

The Hyperscan runner program makes the following decisions:

* None of the "captures" models are implemented because Hyperscan does not
support reporting the match locations of capture groups. This is perhaps
possible to do by using the Chimera regex engine provided by Hyperscan, but
it's not clear it's worth the effort to do so.
* The `count` model does _not_ ask for the start of a match (SOM), which
usually causes Hyperscan to run quite a bit faster and also permits Hyperscan
to compile bigger regexes than it otherwise would.
* Similarly, the `count-spans` model _does_ ask for SOM.
* Hyperscan's notable streaming mode is not benchmarked at all. A streaming
regex benchmark is surely useful, but rebar is not currently the place to do
it. (There are very few regex engines that support streaming mode.)
* Hyperscan has a Unicode mode (analogous to PCRE2's `UCP` and `UTF8` modes)
that we enable when `unicode` is enabled in the benchmark definition.
However, Hyperscan's API decrees that it is _undefined behavior_ to run a
regex compiled in Unicode mode on a haystack that is invalid UTF-8. Therefore,
this runner program will report an error when the haystack is not valid UTF-8
and Unicode mode is enabled.

## Match counts

Hyperscan differs from most other regex engines in that it reports the
locations of all possible matches, including overlapping matches. For example,
using Hyperscan to run the regex `[a-z]+` on the haystack `abc` will report
`[0, 1]`, `[0, 2]` and `[0, 3]` as matches.

For this reason, the benchmark definitions in rebar will sometimes use a
different count result for Hyperscan. In general, we do try to avoid doing
this, as different counts usually imply different amounts of work. However, at
a certain point, the fact that Hyperscan does it differently than most other
regex engines is going to come up as a problem that you will likely need to
work-around in some manner. Sometimes that means adjusting the regex so that
it works the same as other regex engines (which we sometimes do in rebar) and
sometimes that means just being okay with more matches than you might otherwise
expect (which we also sometimes do in rebar).

Either way, benchmarks that specify a different match count for Hyperscan (or
any regex engine) make it very clear that they do so.

## Third party binding

Unlike the runner programs for PCRE2 and RE2 (also written in Rust), this
runner program makes use of a [third party Rust binding for
Hyperscan][binding]. We ususally try to avoid this because it introduces a
risk for benchmarking to be skewed in some way by indiosyncracies in the
binding.

However, in the course of constructing this benchmark, I kind of ran out of
energy to re-roll my own Hyperscan bindings. Firstly, its build is quite a bit
more complex than either RE2 or PCRE2. Secondly, the Hyperscan API is a bit
more complicated to bind, in part because of its use of internal iteration.

With that said, if the third party binding ends up not working out or if there
is some serious problem with it that's discovered, then we should probably
endeavor to cut out the middle man. It might be good to do that anyway, and in
particular, I would prefer if `cargo build` also compiled Hyperscan itself.
Otherwise, it seems really difficult to get good debug symbols, which makes
profiling Hyperscan more difficult than necessary. Building Hyperscan ourselves
also gives us more control over which version is being benchmarked, instead of
relying on it being installed in your system.

I did do a quick audit of the binding library and things look okayish
internally, although it has lots of layers and uses even more dependencies
just to help define the binding, which I would personally prefer to avoid. The
API is also quite expansive with lots of "ref" types and absolutely overloaded
with traits. I found it pretty difficult to navigate personally.

I was briefly tempted to use the "regex crate compatible" sub-module, but a
quick look at its implementation reveals precisely why I have a fear of third
party bindings: it allocates new Hyperscan scratch space for every search.
Owch. Also, its [own benchmarks][binding-benchmarks] are highly suspect.

[Hyperscan]: https://github.com/intel/hyperscan
[binding]: https://github.com/flier/rust-hyperscan
[binding-benchmarks]: https://github.com/flier/rust-hyperscan/issues/30
