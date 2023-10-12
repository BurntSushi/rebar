# Wanted

It would be great to add more regex engines to this barometer. I am thinking
of at least the following, but I'm generally open to any regex engine that
has a reasonable build process with stable tooling:

* Ruby's regex engine, or perhaps just [Onigmo](https://github.com/k-takata/Onigmo)
directly.
* [`nim-regex`](https://github.com/nitely/nim-regex)
* [CTRE](https://github.com/hanickadot/compile-time-regular-expressions). (This
one may prove tricky since "compile a regex" probably means "compile a C++
program." The rebar tool supports this, but it will be annoying. If you want
to add this, please file an issue to discuss an implementation path.)
* A POSIX regex engine.
* [`NSRegularExpression`](https://developer.apple.com/documentation/foundation/nsregularexpression), perhaps through Swift?
* Lisp's [CL-PPCRE](https://github.com/edicl/cl-ppcre/).
* A selected subset of the [mess that is regex libraries for
Haskell](https://wiki.haskell.org/Regular_expressions).

Here are some other regex engines I'm aware of, but I have reservations about
including them:

* PHP's `preg` functions. This "just" uses PCRE2, which is already included
in this benchmark, so it's not clear whether it's also worth measuring here
too. But maybe it is. Maybe PHP introduces some interesting performance
characteristics that meaningfully alter the picture presented by using PCRE2
directly.
* Julia's standard regex engine, which last I checked was also PCRE2. So a
similar reasoning as for PHP applies here.
* C++'s `std::regex` or Boost's regex engine. These are known to be horribly
slow. Maybe we should still include them for completeness.
* [re2c](http://re2c.org/) does regex matching through code generation, so this
would likely work similarly to CTRE if it were to be added. It serves a very
different use case than most regex engines, so I'm not sure if it fits here,
but it could be interesting.
* Regex engines embedded in grep tools like GNU grep. These may be a little
tricky to correctly benchmark given the methodology here, but I think it
should be possible to satisfy at least some of the models. The idea here would
be to actually call the `grep` program and not try to rip the regex engine
out of it.
* Tcl's regex library is something I've benchmarked in the past and I recall
it also being extraordinarily slow. So I'm not sure if it's worth it? Also,
does anyone still choosing Tcl for new projects in production?

I think the main criteria for inclusion are:

* Someone has to actually be using the regex engine for something. It's not
scalable to include every regex engine someone threw up on GitHub five years
ago that isn't being maintained and nobody is using.
* The build process for the regex engine needs to be somewhat reasonable,
or it needs to somehow be maintained by someone else. For example, we don't
build Python's regex engine or ICU. Instead, we just require that Python or
ICU be installed by some other means, with Python being made available via the
`python` binary, and ICU being made available via `pkg-config`.
