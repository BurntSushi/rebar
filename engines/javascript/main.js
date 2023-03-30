const buffer = require('buffer')
const fs = require('fs')

function main() {
  // Beware, do not use 'process.stdin.fd'!
  // See: https://github.com/nodejs/node/issues/7439
  //
  // I was using it, but was getting *transient* errors:
  //
  //     Error: EAGAIN: resource temporarily unavailable, read
  //
  // And it *is* part of the public API and mentions
  // nothing about this failure mode:
  // https://nodejs.org/api/process.html#process_process_stdin_fd
  //
  // 💩💩💩¯\_(ツ)_/¯💩💩💩
  const config = parseConfig(fs.readFileSync(0));
  let samples = [];
  if (config.model == "compile") {
    samples = modelCompile(config);
  } else if (config.model == "count") {
    samples = modelCount(config);
  } else if (config.model == "count-spans") {
    samples = modelCountSpans(config);
  } else if (config.model == "count-captures") {
    samples = modelCountCaptures(config);
  } else if (config.model == "grep") {
    samples = modelGrep(config);
  } else if (config.model == "grep-captures") {
    samples = modelGrepCaptures(config);
  } else if (config.model == "regex-redux") {
    samples = modelRegexRedux(config);
  } else {
    throw new Error(`unrecognized benchmark model '${config.model}'`);
  }
  for (const s of samples) {
    process.stdout.write(`${s.duration},${s.count}\n`);
  }
}

function modelCompile(config) {
  return runAndCount(
    config,
    re => regexCount(re, config.haystack),
    () => compileRegex(config),
  );
}

function modelCount(config) {
  const re = compileRegex(config);
  return run(config, () => regexCount(re, config.haystack));
}

function modelCountSpans(config) {
  const re = compileRegex(config);
  return run(config, () => {
    let sum = 0;
    let last = 0;
    let m;
    while ((m = re.exec(config.haystack)) != null) {
      sum += m[0].length;
      // Oh my goodness, the whole lastIndex business
      // doesn't account for zero-width matches.
      if (last == re.lastIndex) {
        re.lastIndex++;
      }
      last = re.lastIndex;
    }
    return sum;
  });
}

function modelCountCaptures(config) {
  const re = compileRegex(config);
  return run(config, () => {
    let count = 0;
    let last = 0;
    let m;
    while ((m = re.exec(config.haystack)) != null) {
      for (const group of m) {
        if (typeof group != 'undefined') {
          count++;
        }
      }
      // Oh my goodness, the whole lastIndex business
      // doesn't account for zero-width matches.
      if (last == re.lastIndex) {
        re.lastIndex++;
      }
      last = re.lastIndex;
    }
    return count;
  });
}

function modelGrep(config) {
  const re = compileRegex(config);
  return run(config, () => {
    let count = 0;
    const lines = config.haystack.split('\n');
    for (let line of lines) {
      if (line.endsWith('\r')) {
        line = line.slice(0, line.length - 1);
      }
      re.lastIndex = 0;
      if (re.test(line)) {
        count++;
      }
    }
    return count;
  });
}

function modelGrepCaptures(config) {
  const re = compileRegex(config);
  return run(config, () => {
    let count = 0;
    const lines = config.haystack.split('\n');
    for (let line of lines) {
      if (line.endsWith('\r')) {
        line = line.slice(0, line.length - 1);
      }
      let m;
      while ((m = re.exec(line)) != null) {
        for (const group of m) {
          if (typeof group != 'undefined') {
            count++;
          }
        }
        // N.B. The grep benchmark models
        // permit assuming that the regex
        // will never match the empty string.
      }
    }
    return count;
  });
}

function modelRegexRedux(config) {
  return run(config, () => {
    const expected = `
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
`
    const result = [];
    let seq = config.haystack;
    const ilen = seq.length;
    seq = seq.replaceAll(compilePattern(config, '>[^\n]*\n|\n'), '');
    const clen = seq.length;

    const variants = [
      "agggtaaa|tttaccct",
      "[cgt]gggtaaa|tttaccc[acg]",
      "a[act]ggtaaa|tttacc[agt]t",
      "ag[act]gtaaa|tttac[agt]ct",
      "agg[act]taaa|ttta[agt]cct",
      "aggg[acg]aaa|ttt[cgt]ccct",
      "agggt[cgt]aa|tt[acg]accct",
      "agggta[cgt]a|t[acg]taccct",
      "agggtaa[cgt]|[acg]ttaccct",
    ];
    for (const variant of variants) {
      const re = compilePattern(config, variant);
      const count = regexCount(re, seq);
      result.push(`${variant} ${count}`);
    }

    seq = seq.replaceAll(compilePattern(config, 'tHa[Nt]'), '<4>');
    seq = seq.replaceAll(compilePattern(config, 'aND|caN|Ha[DS]|WaS'), '<3>');
    seq = seq.replaceAll(compilePattern(config, 'a[NSt]|BY'), '<2>');
    seq = seq.replaceAll(compilePattern(config, '<[^>]*>'), '|');
    seq = seq.replaceAll(compilePattern(config, '\\|[^|][^|]*\\|'), '-');

    result.push('');
    result.push(`${ilen}`);
    result.push(`${clen}`);
    result.push(`${seq.length}`);
    if (expected.trim() != result.join('\n').trim()) {
      throw new Error(`result did not match what was expected`);
    }
    return seq.length;
  });
}

// Repeatedly runs the given 'bench' function according to the
// 'config' object given. The 'bench' function should return a count
// corresponding to the verification number for this benchmark.
function run(config, bench) {
  return runAndCount(config, n => n, bench);
}

// Repeatedly runs the given 'bench' function according to the 'config'
// object given. The result of 'bench' is given to 'count', which
// should return a verification number for the given benchmark. Only
// the 'bench' function is measured.
function runAndCount(config, count, bench) {
  const warmupStart = process.hrtime.bigint();
  for (let i = 0; i < config.maxWarmupIters; i++) {
    count(bench());
    if ((process.hrtime.bigint() - warmupStart) >= config.maxWarmupTime) {
      break;
    }
  }

  const samples = [];
  const runStart = process.hrtime.bigint();
  for (let i = 0; i < config.maxIters; i++) {
    const benchStart = process.hrtime.bigint();
    const result = bench();
    const elapsed = process.hrtime.bigint() - benchStart;
    const n = count(result);
    samples.push({duration: elapsed, count: n});
    if ((process.hrtime.bigint() - runStart) >= config.maxTime) {
      break;
    }
  }
  return samples;
}

// Parses a sequence of KLV items into a single config object.
// 'raw' should be a Buffer corresponding to the benchmark KLV
// data.
function parseConfig(raw) {
  const config = {
    name: null,
    model: null,
    pattern: null,
    caseInsensitive: false,
    unicode: false,
    haystack: null,
    maxIters: 0,
    maxWarmupIters: 0,
    maxTime: 0,
    maxWarmupTime: 0,
  };
  while (raw.length > 0) {
    const klv = parseOneKLV(raw);
    raw = raw.subarray(klv.length);
    if (klv.key == "name") {
      config.name = klv.value;
    } else if (klv.key == "model") {
      config.model = klv.value;
    } else if (klv.key == "pattern") {
      if (config.pattern != null) {
        throw new Error(`only one pattern is supported`);
      }
      config.pattern = klv.value;
    } else if (klv.key == "case-insensitive") {
      config.caseInsensitive = klv.value == "true";
    } else if (klv.key == "unicode") {
      config.unicode = klv.value == "true";
    } else if (klv.key == "haystack") {
      config.haystack = klv.value;
    } else if (klv.key == "max-iters") {
      config.maxIters = parseInt(klv.value, 10);
    } else if (klv.key == "max-warmup-iters") {
      config.maxWarmupIters = parseInt(klv.value, 10);
    } else if (klv.key == "max-time") {
      config.maxTime = BigInt(klv.value);
    } else if (klv.key == "max-warmup-time") {
      config.maxWarmupTime = BigInt(klv.value);
    } else {
      throw new Error(`unrecognized KLV key '${klv.key}'`);
    }
  }
  return config;
}

// Parses a single KLV item, and returns an object with the following
// keys: 'key', 'value' and 'length'. 'length' is the number of *bytes*
// read from the given 'raw' Buffer in order to parse the KLV item.
function parseOneKLV(raw) {
  const klv = {length: 0};
  const keyEnd = raw.indexOf(':');
  if (keyEnd == -1) {
    throw new Error(`invalid KLV item: could not find first ':'`);
  }
  klv.key = decode(raw.subarray(0, keyEnd));
  klv.length += keyEnd + 1;
  raw = raw.subarray(keyEnd + 1);

  const valueLenEnd = raw.indexOf(':');
  if (valueLenEnd == -1) {
    throw new Error(`invalid KLV item: could not find second ':'`);
  }
  const valueLen = parseInt(decode(raw.subarray(0, valueLenEnd)), 10);
  klv.length += valueLenEnd + 1;
  raw = raw.subarray(valueLenEnd + 1);

  if (raw[valueLen] != 0x0A) {
    throw new Error(`invalid KLV item: no line terminator`);
  }
  klv.value = decode(raw.subarray(0, valueLen));
  klv.length += valueLen + 1;
  return klv;
}

// Compiles the pattern in the given config object.
// (Where the config object is given by 'parseConfig'.)
function compileRegex(config) {
  return compilePattern(config, config.pattern);
}

// Compiles the given pattern according to the given config
// object. (Where the config object is given by 'parseConfig'.)
function compilePattern(config, pattern) {
  let flags = "g";
  if (config.caseInsensitive) {
    flags += "i";
  }
  if (config.unicode) {
    flags += "u";
  }
  return new RegExp(pattern, flags);
}

// Counts the total number of times 're' matches 'haystack'.
function regexCount(re, haystack) {
  let count = 0;
  let last = 0;
  while (re.test(haystack)) {
    count++;
    // Oh my goodness, the whole lastIndex business
    // doesn't account for zero-width matches.
    if (last == re.lastIndex) {
      re.lastIndex++;
    }
    last = re.lastIndex;
  }
  return count;
}

// Decodes the given 'Buffer' into a string, assuming that
// the buffer given is UTF-8 encoded bytes. If it's invalid
// UTF-8, then this throws an exception.
function decode(buf) {
  // All of the transcoding APIs seem to do lossy decoding,
  // but we want to actually fail if we get invalid UTF-8.
  // Otherwise, we might end up searching a subtly different
  // haystack than what other regex engines do for the same
  // benchmark definition.
  if (!buffer.isUtf8(buf)) {
    throw new Error(`buffer contains invalid UTF-8: ${buf}`)
  }
  return buf.toString();
}

main()
