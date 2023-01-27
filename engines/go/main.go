package main

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"os"
	"regexp"
	"runtime"
	"strconv"
	"strings"
	"time"
)

type config struct {
	Name            string
	Model           string
	Pattern         string
	Regexp          *regexp.Regexp
	CaseInsensitive bool
	Unicode         bool
	Haystack        []byte
	MaxIters        int
	MaxWarmupIters  int
	MaxTime         time.Duration
	MaxWarmupTime   time.Duration
}

func parseConfig(rdr io.Reader) (*config, error) {
	c := &config{}
	raw, err := io.ReadAll(rdr)
	if err != nil {
		return nil, errors.New("failed to read KLV data from reader")
	}
	patterns := []string{}
	for len(raw) > 0 {
		klv, nread, err := parseOneKLV(raw)
		if err != nil {
			return nil, err
		}
		raw = raw[nread:]
		switch klv.Key {
		case "name":
			c.Name = string(klv.Value)
		case "model":
			c.Model = string(klv.Value)
		case "pattern":
			patterns = append(patterns, string(klv.Value))
		case "case-insensitive":
			c.CaseInsensitive = string(klv.Value) == "true"
		case "unicode":
			c.Unicode = string(klv.Value) == "true"
		case "haystack":
			c.Haystack = klv.Value
		case "max-iters":
			n, err := strconv.Atoi(string(klv.Value))
			if err != nil {
				return nil, fmt.Errorf(
					"failed to parse 'max-iters': %w",
					err,
				)
			}
			c.MaxIters = n
		case "max-warmup-iters":
			n, err := strconv.Atoi(string(klv.Value))
			if err != nil {
				return nil, fmt.Errorf(
					"failed to parse 'max-warmup-iters': %w",
					err,
				)
			}
			c.MaxWarmupIters = n
		case "max-time":
			n, err := strconv.Atoi(string(klv.Value))
			if err != nil {
				return nil, fmt.Errorf(
					"failed to parse 'max-time': %w",
					err,
				)
			}
			c.MaxTime = time.Duration(int64(n))
		case "max-warmup-time":
			n, err := strconv.Atoi(string(klv.Value))
			if err != nil {
				return nil, fmt.Errorf(
					"failed to parse 'max-warmup-time': %w",
					err,
				)
			}
			c.MaxWarmupTime = time.Duration(int64(n))
		default:
			return nil, fmt.Errorf(
				"unrecognized KLV item key '%s'",
				klv.Key,
			)
		}
	}
	if c.Model != "regex-redux" {
		if len(patterns) != 1 {
			return nil, errors.New("number of patterns must be 1")
		}
		c.Pattern = patterns[0]
		c.Regexp, err = regexp.Compile(c.pattern())
		if err != nil {
			return nil, fmt.Errorf(
				"failed to compile regexp: %w",
				err,
			)
		}
	}
	return c, nil
}

type oneKLV struct {
	Key   string
	Value []byte
}

func parseOneKLV(raw []byte) (*oneKLV, int, error) {
	pieces := bytes.SplitN(raw, []byte(":"), 3)
	if len(pieces) < 3 {
		return nil, 0, errors.New("invalid KLV item: not enough pieces")
	}
	key := string(pieces[0])
	valueLen, err := strconv.Atoi(string(pieces[1]))
	if err != nil {
		return nil, 0, fmt.Errorf("failed to parse value length: %w", err)
	}
	rest := pieces[2]
	if len(rest) < valueLen {
		return nil, 0, fmt.Errorf(
			"not enough bytes remaining for length %d for key '%s'",
			valueLen,
			key,
		)
	}
	value := rest[:valueLen]
	rest = rest[valueLen:]
	if len(rest) == 0 || rest[0] != '\n' {
		return nil, 0, fmt.Errorf(
			"did not find \\n after value for key '%s'",
			key,
		)
	}
	nread := len(pieces[0]) + 1 + len(pieces[1]) + 1 + len(value) + 1
	return &oneKLV{Key: key, Value: value}, nread, nil
}

func (c *config) pattern() string {
	// OK because config parsing fails if number of patterns != 1.
	if c.CaseInsensitive {
		c.Pattern = "(?i:" + c.Pattern + ")"
	}
	// Go doesn't have a "Unicode" mode. It is always enabled.
	// But note that \w, \d and \s are *not* Unicode aware and
	// there is no way to make them Unicode aware.
	return c.Pattern
}

type sample struct {
	Duration time.Duration
	Count    int
}

func modelCompile(c *config) ([]sample, error) {
	// Config parsing already compiles the pattern
	// for convenience, but we obviously ignore that
	// here because we want to measure compilation.
	p := c.pattern()
	bench := func() (*regexp.Regexp, error) {
		return regexp.Compile(p)
	}
	count := func(re *regexp.Regexp) (int, error) {
		return len(re.FindAllIndex(c.Haystack, -1)), nil
	}
	return runAndCount(c, count, bench)
}

func modelCount(c *config) ([]sample, error) {
	return run(c, func() (int, error) {
		return len(c.Regexp.FindAllIndex(c.Haystack, -1)), nil
	})
}

func modelCountSpans(c *config) ([]sample, error) {
	return run(c, func() (int, error) {
		sum := 0
		for _, m := range c.Regexp.FindAllIndex(c.Haystack, -1) {
			sum += m[1] - m[0]
		}
		return sum, nil
	})
}

func modelCountCaptures(c *config) ([]sample, error) {
	return run(c, func() (int, error) {
		count := 0
		matches := c.Regexp.FindAllSubmatchIndex(c.Haystack, -1)
		for _, match := range matches {
			for i := 0; i < len(match); i += 2 {
				if match[i] > -1 {
					count += 1
				}
			}
		}
		return count, nil
	})
}

func modelGrep(c *config) ([]sample, error) {
	return run(c, func() (int, error) {
		count := 0
		lines := bytes.Split(c.Haystack, []byte{'\n'})
		// Get rid of the empty line when haystack ends with \n.
		if len(lines) > 0 && len(lines[len(lines)-1]) == 0 {
			lines = lines[:len(lines)-1]
		}
		for _, line := range lines {
			if len(line) > 0 && line[len(line)-1] == '\r' {
				line = line[:len(line)-1]
			}
			if c.Regexp.Match(line) {
				count += 1
			}
		}
		return count, nil
	})
}

func modelGrepCaptures(c *config) ([]sample, error) {
	return run(c, func() (int, error) {
		count := 0
		lines := bytes.Split(c.Haystack, []byte{'\n'})
		// Get rid of the empty line when haystack ends with \n.
		if len(lines) > 0 && len(lines[len(lines)-1]) == 0 {
			lines = lines[:len(lines)-1]
		}
		for _, line := range lines {
			if len(line) > 0 && line[len(line)-1] == '\r' {
				line = line[:len(line)-1]
			}
			matches := c.Regexp.FindAllSubmatchIndex(line, -1)
			for _, match := range matches {
				for i := 0; i < len(match); i += 2 {
					if match[i] > -1 {
						count += 1
					}
				}
			}
		}
		return count, nil
	})
}

func modelRegexRedux(c *config) ([]sample, error) {
	verify := func(output string) error {
		expected := `
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
`[1:]
		if expected != output {
			return errors.New(
				"output did not match what was expected",
			)
		}
		return nil
	}
	compile := func(pattern string) *regexp.Regexp {
		if c.CaseInsensitive {
			pattern = "(?i:" + pattern + ")"
		}
		// This is okay, because all regexes in this
		// benchmark model are known statically and
		// we know they are valid.
		return regexp.MustCompile(pattern)
	}
	bench := func() (int, error) {
		out := new(strings.Builder)
		seq := string(c.Haystack)
		ilen := len(seq)
		seq = compile(`>[^\n]*\n|\n`).ReplaceAllString(seq, "")
		clen := len(seq)

		variants := []string{
			`agggtaaa|tttaccct`,
			`[cgt]gggtaaa|tttaccc[acg]`,
			`a[act]ggtaaa|tttacc[agt]t`,
			`ag[act]gtaaa|tttac[agt]ct`,
			`agg[act]taaa|ttta[agt]cct`,
			`aggg[acg]aaa|ttt[cgt]ccct`,
			`agggt[cgt]aa|tt[acg]accct`,
			`agggta[cgt]a|t[acg]taccct`,
			`agggtaa[cgt]|[acg]ttaccct`,
		}
		for _, variant := range variants {
			re := compile(variant)
			count := len(re.FindAllStringIndex(seq, -1))
			fmt.Fprintf(out, "%s %d\n", variant, count)
		}

		type subst struct {
			re   *regexp.Regexp
			repl string
		}
		substs := []subst{
			subst{compile(`tHa[Nt]`), "<4>"},
			subst{compile(`aND|caN|Ha[DS]|WaS`), "<3>"},
			subst{compile(`a[NSt]|BY`), "<2>"},
			subst{compile(`<[^>]*>`), "|"},
			subst{compile(`\|[^|][^|]*\|`), "-"},
		}
		for _, s := range substs {
			seq = s.re.ReplaceAllString(seq, s.repl)
		}

		fmt.Fprintf(out, "\n%d\n%d\n%d\n", ilen, clen, len(seq))
		return len(seq), verify(out.String())
	}
	return run(c, bench)
}

func run(c *config, bench func() (int, error)) ([]sample, error) {
	count := func(n int) (int, error) { return n, nil }
	return runAndCount(c, count, bench)
}

func runAndCount[T any](
	c *config,
	count func(T) (int, error),
	bench func() (T, error),
) ([]sample, error) {
	warmupStart := time.Now()
	for i := 0; i < c.MaxWarmupIters; i++ {
		result, err := bench()
		if err != nil {
			return nil, err
		}
		_, err = count(result)
		if err != nil {
			return nil, err
		}
		if time.Since(warmupStart) >= c.MaxWarmupTime {
			break
		}
	}

	results := []sample{}
	runStart := time.Now()
	for i := 0; i < c.MaxIters; i++ {
		benchStart := time.Now()
		result, err := bench()
		elapsed := time.Since(benchStart)
		if err != nil {
			return nil, err
		}
		n, err := count(result)
		if err != nil {
			return nil, err
		}
		results = append(results, sample{
			Duration: elapsed,
			Count:    n,
		})
		if time.Since(runStart) >= c.MaxTime {
			break
		}
	}
	return results, nil
}

func main() {
	if err := tryMain(); err != nil {
		fmt.Fprintf(os.Stderr, "%s\n", err)
		os.Exit(1)
	}
}

func tryMain() error {
	if len(os.Args) == 2 && os.Args[1] == "version" {
		fmt.Println(runtime.Version())
		return nil
	}
	quiet := len(os.Args) == 2 && os.Args[1] == "--quiet"
	c, err := parseConfig(os.Stdin)
	if err != nil {
		return fmt.Errorf("failed to read stdin: %w", err)
	}
	var results []sample
	switch c.Model {
	case "compile":
		results, err = modelCompile(c)
		if err != nil {
			return err
		}
	case "count":
		results, err = modelCount(c)
		if err != nil {
			return err
		}
	case "count-spans":
		results, err = modelCountSpans(c)
		if err != nil {
			return err
		}
	case "count-captures":
		results, err = modelCountCaptures(c)
		if err != nil {
			return err
		}
	case "grep":
		results, err = modelGrep(c)
		if err != nil {
			return err
		}
	case "grep-captures":
		results, err = modelGrepCaptures(c)
		if err != nil {
			return err
		}
	case "regex-redux":
		results, err = modelRegexRedux(c)
		if err != nil {
			return err
		}
	default:
		return fmt.Errorf("unrecognized benchmark model '%s'", c.Model)
	}
	if !quiet {
		for _, sample := range results {
			fmt.Printf("%d,%d\n", int64(sample.Duration), sample.Count)
		}
	}
	return nil
}
