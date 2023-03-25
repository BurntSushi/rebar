import collections
import io
import sys
import time


class Config(collections.namedtuple('Config', [
    'name',
    'model',
    'patterns',
    'case_insensitive',
    'unicode',
    'haystack',
    'max_iters',
    'max_warmup_iters',
    'max_time',
    'max_warmup_time',
])):
    '''
    The configuration of a benchmark. This describes the regexes, their
    options, the haystack and several parameters for how to actually
    execute the benchmark.
    '''

    @staticmethod
    def parse():
        '''
        Parses stdin in KLV format to get the benchmark configuration.
        This raises an exception if the format is invalid.
        '''
        c = Config(
            name='',
            model='',
            patterns=[],
            case_insensitive=False,
            unicode=False,
            haystack='',
            max_iters=0,
            max_warmup_iters=0,
            max_time=0,
            max_warmup_time=0,
        )
        raw = sys.stdin.buffer.read()
        while len(raw) > 0:
            klv, nread = OneKLV.parse(raw)
            raw = raw[nread:]
            if klv.key == 'name':
                c = c._replace(name=klv.value.decode('utf-8'))
            elif klv.key == 'model':
                c = c._replace(model=klv.value.decode('utf-8'))
            elif klv.key == 'pattern':
                c.patterns.append(klv.value.decode('utf-8'))
            elif klv.key == 'case-insensitive':
                c = c._replace(case_insensitive=klv.value == b'true')
            elif klv.key == 'unicode':
                c = c._replace(unicode=klv.value == b'true')
            elif klv.key == 'haystack':
                c = c._replace(haystack=klv.value)
            elif klv.key == 'max-iters':
                c = c._replace(max_iters=int(klv.value))
            elif klv.key == 'max-warmup-iters':
                c = c._replace(max_warmup_iters=int(klv.value))
            elif klv.key == 'max-time':
                c = c._replace(max_time=int(klv.value))
            elif klv.key == 'max-warmup-time':
                c = c._replace(max_warmup_time=int(klv.value))
            else:
                raise ValueError(f"unrecognized KLV item key '{klv.key}'")
        return c

    def get_haystack(self):
        '''
        Returns either a 'str' haystack when Unicode mode is enabled,
        or a 'bytes' haystack when it is disabled.
        '''
        # We always read the haystack as binary data. When unicode mode is
        # enabled, we decode this as UTF-8 outside of measurement. This does
        # mean that Python Unicode regexes can't be run on arbitrary bytes
        # (unlike RE2, PCRE2 and the regex crate), but this appears to be a
        # real limitation of the module itself and not an artifact of our
        # methodology here. Namely, you get an error when you try to search a
        # 'bytes' haystack with a Unicode regex. Similarly, you can't run a
        # 'bytes' regex on a Unicode string.
        if self.unicode:
            return self.haystack.decode('utf-8')
        else:
            return self.haystack

    def get_one_pattern(self):
        '''
        Returns a single pattern that is a 'str' when Unicode mode is
        enabled, or a 'bytes' pattern when it is disabled.

        If this benchmark has anything but one pattern, then this
        raises a 'ValueError' exception.
        '''
        if len(self.patterns) != 1:
            raise ValueError(
                f'expected 1 pattern, but got {len(self.patterns)}',
            )
        p = self.patterns[0]
        if self.unicode:
            return p
        return p.encode('utf-8')

    def get_one_regex(self):
        '''
        Returns a single regex object that is Unicode-aware when
        Unicode mode is enabled, or a 8-bit byte oriented pattern when
        it is disabled.

        If this benchmark has anything but one pattern, then this
        raises a 'ValueError' exception.
        '''
        return re.compile(self.get_one_pattern(), self.get_re_flags())

    def get_re_flags(self):
        '''
        Return flags suitable for use with 're.compile' based on this
        benchmark's configuration.
        '''
        flags = 0  # should be re.NOFLAG if we required Python 3.11
        if self.case_insensitive:
            flags |= re.IGNORECASE
        if self.unicode:
            flags |= re.UNICODE
        else:
            flags |= re.ASCII
        return flags

    def maybe_bytes(self, s):
        '''
        When 's' is a Unicode string and Unicode is disabled for this
        benchmark, then return it as a UTF-8 encoded byte string.
        Otherwise return 's' unchanged.

        This is useful for letting us just use regular string literals
        below, and then this function will convert it to a byte string
        if needed.
        '''
        if not self.unicode and isinstance(s, str):
            return s.encode('utf-8')
        return s


class OneKLV(collections.namedtuple('OneKLV', ['key', 'value'])):
    @staticmethod
    def parse(raw):
        assert isinstance(raw, bytes)

        pieces = raw.split(b':', 2)
        if len(pieces) < 3:
            raise ValueError("invalid KLV item: not enough pieces")
        key = pieces[0].decode('utf-8')
        value_len = int(pieces[1])
        rest = pieces[2]
        if len(rest) < value_len:
            raise ValueError(
                f"not enough bytes remaining for length "
                f"{value_len} for key '{key}'",
            )
        value = rest[:value_len]
        rest = rest[value_len:]
        if len(rest) == 0 or rest[0:1] != b'\n':
            raise ValueError(f"did not find \\n after value for key '{key}'")
        nread = len(pieces[0]) + 1 + len(pieces[1]) + 1 + len(value) + 1
        return OneKLV(key=key, value=value), nread


def model_compile(c):
    '''Implements the 'compile' rebar benchmark model.'''
    p = c.get_one_pattern()
    flags = c.get_re_flags()
    h = c.get_haystack()
    def bench():
        return re.compile(p, flags)
    def count(r):
        return sum(1 for _ in r.finditer(h))
    return run_and_count(c, count, bench)


def model_count(c):
    '''Implements the 'count' rebar benchmark model.'''
    r = c.get_one_regex()
    h = c.get_haystack()
    return run(c, lambda: sum(1 for _ in r.finditer(h)))


def model_count_spans(c):
    '''Implements the 'count-spans' rebar benchmark model.'''
    r = c.get_one_regex()
    h = c.get_haystack()
    def bench():
        if c.unicode:
            return sum(len(m.group(0).encode('utf-8')) for m in r.finditer(h))
        else:
            return sum(len(m.group(0)) for m in r.finditer(h))
    return run(c, bench)


def model_count_captures(c):
    '''Implements the 'count-captures' rebar benchmark model.'''
    r = c.get_one_regex()
    h = c.get_haystack()
    def bench():
        count = 0
        for m in r.finditer(h):
            # Add 1 to account for implicit capture group.
            count += 1 + sum(1 for g in m.groups() if g is not None)
        return count
    return run(c, bench)


def model_grep(c):
    '''Implements the 'grep' rebar benchmark model.'''
    r = c.get_one_regex()
    h = c.get_haystack()
    def bench():
        hay = h
        count = 0
        # N.B. I tried using io.StringIO here to avoid loading all of the
        # lines into memory first, but it doesn't seem to make a difference.
        #
        # Also, handle the case where the haystack ends with a '\n'. That's
        # the last line and we don't want an empty one after it.
        #
        # We don't use 'splitlines' here because it splits on more than just
        # LF and CRLF.
        lines = h.split(c.maybe_bytes('\n'))
        if len(lines) > 0 and len(lines[len(lines)-1]) == 0:
            lines = lines[:len(lines)-1]
        for line in lines:
            if line.endswith(c.maybe_bytes('\r')):
                line = line[0:len(line)-1]
            if r.search(line):
                count += 1
        return count
    return run(c, bench)


def model_grep_captures(c):
    '''Implements the 'grep-captures' rebar benchmark model.'''
    r = c.get_one_regex()
    h = c.get_haystack()
    def bench():
        count = 0
        lines = h.split(c.maybe_bytes('\n'))
        if len(lines) > 0 and len(lines[len(lines)-1]) == 0:
            lines = lines[:len(lines)-1]
        for line in lines:
            if line.endswith(c.maybe_bytes('\r')):
                line = line[0:len(line)-1]
            for m in r.finditer(line):
                # Add 1 to account for implicit capture group.
                count += 1 + sum(1 for g in m.groups() if g is not None)
        return count
    return run(c, bench)


def model_regex_redux(c):
    '''Implements the 'regex-redux' rebar benchmark model.'''
    def maybe_bytes(s):
        '''
        When 's' is a Unicode string and Unicode is disabled for this
        benchmark, then return it as a UTF-8 encoded byte string.
        Otherwise return 's' unchanged.

        This is useful for letting us just use regular string literals
        below, and then this function will convert it to a byte string
        if needed.
        '''
        if not c.unicode and isinstance(s, str):
            return s.encode('utf-8')
        return s

    def verify(output):
        '''Raise an exception if 'output' is incorrect.'''
        expected = maybe_bytes('''
agggtaaa|tttaccct 6
[cgt]gggtaaa|tttaccc[acg] 26
a[act]ggtaaa|tttacc[agt]t 86
ag[act]gtaaa|tttac[agt]ct 58
agg[act]taaa|ttta[agt]cct 113
aggg[acg]aaa|ttt[cgt]ccct 31
agggt[cgt]aa|tt[acg]accct 31
agggta[cgt]a|t[acg]taccct 32
agggtaa[cgt]|[acg]ttaccct 43

1016745
1000000
547899
'''.lstrip())
        if expected != output:
            raise ValueError('output did not match what was expected')
        return output

    def regex(pattern):
        '''Compile the given regex pattern.'''
        return re.compile(maybe_bytes(pattern), c.get_re_flags())

    def bench():
        '''Run a single iteration of the regex-redux benchmark.'''
        if c.unicode:
            out = io.StringIO()
        else:
            out = io.BytesIO()
        seq = c.get_haystack()
        ilen = len(seq)
        seq = regex(r">[^\n]*\n|\n").sub(maybe_bytes(''), seq)
        clen = len(seq)

        variants = [
            r"agggtaaa|tttaccct",
            r"[cgt]gggtaaa|tttaccc[acg]",
            r"a[act]ggtaaa|tttacc[agt]t",
            r"ag[act]gtaaa|tttac[agt]ct",
            r"agg[act]taaa|ttta[agt]cct",
            r"aggg[acg]aaa|ttt[cgt]ccct",
            r"agggt[cgt]aa|tt[acg]accct",
            r"agggta[cgt]a|t[acg]taccct",
            r"agggtaa[cgt]|[acg]ttaccct",
        ]
        for variant in variants:
            count = sum(1 for _ in regex(variant).finditer(seq))
            out.write(maybe_bytes(f'{variant} {count}\n'))

        substs = [
            (regex(r"tHa[Nt]"), "<4>"),
            (regex(r"aND|caN|Ha[DS]|WaS"), "<3>"),
            (regex(r"a[NSt]|BY"), "<2>"),
            (regex(r"<[^>]*>"), "|"),
            (regex(r"\|[^|][^|]*\|"), "-"),
        ]
        for (r, replacement) in substs:
            seq = r.sub(maybe_bytes(replacement), seq)
        out.write(maybe_bytes(f'\n{ilen}\n{clen}\n{len(seq)}\n'))
        verify(out.getvalue())
        return len(seq)

    return run(c, bench)


def run(c, bench):
    '''
    Given a 'Config' and a function that accepts no arguments and runs
    a single iteration of a benchmark, this will execute possibly many
    iterations of that benchmark and return a list of samples. Each
    sample is a pair of duration and count returned.

    The 'bench' function must return a count of the number of regex
    matches.
    '''
    return run_and_count(c, lambda count: count, bench)


def run_and_count(c, count, bench):
    '''
    Like 'run', but also accepts a 'count' function that accepts the
    return value of 'bench' and must return a count of the number
    of times the regex matches the haystack. This is useful for the
    'compile' model, where the 'bench' function should return a regex
    object, and 'count' should execute the regex on a haystack.

    The purpose of this setup is so that 'count' (which is used to
    verify the benchmark) is separate from 'bench' (which is what is
    actually measured).
    '''
    warmup_start = time.time_ns()
    for _ in range(c.max_warmup_iters):
        # See comment below for why we do this.
        re.purge()
        result = bench()
        _count = count(result)
        if (time.time_ns() - warmup_start) >= c.max_warmup_time:
            break

    results = []
    run_start = time.time_ns()
    for _ in range(c.max_iters):
        # Purge's the re module's regex cache, otherwise we wind up just
        # measuring how long it takes to fetch a regex from its internal cache.
        # Technically, this is only necessary for the 'compile' model, but it's
        # easier to just do it here so that our 'compile' model doesn't wind up
        # measuring the time it takes to clear the cache and compile the regex.
        #
        # I've tried search-only benchmarks with and without this purge step
        # and I can't see any measureable difference. In theory, this *could*
        # result in a difference if the cache contains more than just a static
        # compile regex object, but that depends on the implementation of
        # which I am not familiar. If that is indeed the case, we'll want to
        # re-orient how this program is structured so that purging only happens
        # in the 'compile' model implementation.
        re.purge()
        bench_start = time.time_ns()
        result = bench()
        elapsed = time.time_ns() - bench_start
        results.append((elapsed, count(result)))
        if (time.time_ns() - run_start) >= c.max_time:
            break
    return results


if __name__ == '__main__':
    engine = sys.argv[1]
    if engine == 're':
        import re
    elif engine == 'regex':
        # Sometimes Python is nice. A cheap trick to reuse all of the code
        # above, written for the 're' module, but actually use 'regex'.
        import regex as re
        # Opt into the new flavor. One wonders whether this should be treated
        # as an entire different engine, but I don't think it's worth it.
        re.DEFAULT_VERSION = re.VERSION1
    else:
        raise ValueError(f"unrecognized engine '{engine}'")

    config = Config.parse()
    if config.model == 'compile':
        results = model_compile(config)
    elif config.model == 'count':
        results = model_count(config)
    elif config.model == 'count-spans':
        results = model_count_spans(config)
    elif config.model == 'count-captures':
        results = model_count_captures(config)
    elif config.model == 'grep':
        results = model_grep(config)
    elif config.model == 'grep-captures':
        results = model_grep_captures(config)
    elif config.model == 'regex-redux':
        results = model_regex_redux(config)
    else:
        raise ValueError(f"unrecognized benchmark model '{config.model}'")
    for (duration, count) in results:
        print(f'{duration},{count}')
