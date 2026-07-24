import std/parseopt
import std/monotimes
import std/strutils
import pkg/regex

type
    Sample = object
        duration: int64
        count: uint64

    StopWatch = object
        startTicks: int64

proc start(watch: var StopWatch) =
    watch.startTicks = getMonoTime().ticks

proc peek(watch: var StopWatch): int64 =
    return getMonoTime().ticks - watch.startTicks

var
    name: string
    model: string
    patterns: seq[string]
    case_insensitive: bool
    unicode: bool
    haystack: string
    max_iters: int64
    max_warmup_iters: int64
    max_time: int64
    max_warmup_time: int64

proc readUntilColon(): string =
    result = ""
    while not stdin.endOfFile():
        var ch = stdin.readChar()
        if ch == ':':
            break
        else:
            result.add(ch)

proc readValue(): string =
    var size = parseInt(readUntilColon())
    result = ""

    while (not stdin.endOfFile()) and size > 0:
        result.add(stdin.readChar())
        size -= 1

    if size > 0:
        raise newException(Exception, "Could not read full-length value!")

    if stdin.readChar() != '\n':
        raise newException(Exception, "Malformed KLV format!")

proc readBoolValue(): bool =
    var val = readValue()
    case val:
    of "true": return true
    of "false": return false
    else:
        raise newException(
            Exception,
            "Malformed KLV format: expected true or false but got: " & val
        )

type RunFunc[T] = proc(): T
type CountFunc[T] = proc(t: var T): uint64

proc runAndCount[T](bench: RunFunc[T], count: CountFunc[T]) =
    var warmupTimer: StopWatch
    warmupTimer.start()
    for i in 1..max_warmup_iters:
        var result = bench()
        discard count(result)
        if warmupTimer.peek() >= max_warmup_time:
            break

    var samples = newSeqOfCap[Sample](cap = max_iters)

    var runTimer: StopWatch
    runTimer.start()
    for i in 1..max_iters:
        var benchTimer: StopWatch
        benchTimer.start()
        var result = bench()
        var duration = benchTimer.peek()
        var cnt = count(result)
        samples.add(Sample(
            duration: duration,
            count: cnt,
        ))
        if runTimer.peek() >= max_time:
            break

    for sample in samples:
        echo sample.duration, ",", sample.count

proc identityCount(i: var uint64): uint64 =
    return i

template run(bench: RunFunc[uint64]) =
    runAndCount(bench, identityCount)

proc compile(): auto =
    assert(patterns.len == 1)
    var flags: RegexFlags

    if case_insensitive:
        flags.incl regexCaseless

    if not unicode:
        flags.incl regexAscii

    return re2(patterns[0], flags)

iterator lines(inp: string): string =
    var i: int64
    while i < inp.len:
        var j: int64 = inp.find('\n', i)
        if j < 0:
            yield inp[i .. ^1]
            i = inp.len
        else:
            yield inp[i .. j-1]
            i = j + 1

# -- Main:

for kind, key, val in getopt():
    case kind
    of cmdEnd: break
    of cmdShortOption, cmdLongOption:
        case key
        of "version":
            echo NimVersion
            quit(0)
    of cmdArgument:
        discard

while not stdin.endOfFile:
    var key = readUntilColon()
    case key
    of "name":
        name = readValue()
    of "model":
        model = readValue()
    of "pattern":
        patterns.add(readValue())
    of "case-insensitive":
        case_insensitive = readBoolValue()
    of "unicode":
        unicode = readBoolValue()
    of "haystack":
        haystack = readValue()
    of "max-iters":
        max_iters = parseInt(readValue())
    of "max-warmup-iters":
        max_warmup_iters = parseInt(readValue())
    of "max-time":
        max_time = parseBiggestInt(readValue())
    of "max-warmup-time":
        max_warmup_time = parseBiggestInt(readValue())
    else:
        raise newException(Exception, "Unknown key: " & key)

# echo "name: ", name
# echo "model: ", model
# echo "patterns: ", patterns
# echo "case_insensitive: ", case_insensitive
# echo "unicode: ", unicode
# echo "haystack: ", haystack
# echo "max_iters: ", max_iters
# echo "max_warmup_iters: ", max_warmup_iters
# echo "max_time: ", max_time
# echo "max_warmup_time: ", max_warmup_time

case model
of "compile":
    runAndCount(
        proc (): auto =
            return compile()
        ,
        proc (re: var Regex2): uint64 =
            return cast[uint64]( haystack.findAll(re).len )
    )

of "count":
    var regex = compile()
    run(
        proc (): uint64 =
            return cast[uint64]( haystack.findAll(regex).len )
    )

of "count-spans":
    var regex = compile()
    run(
        proc (): uint64 =
            var sum: uint64 = 0
            for m in haystack.findAll(regex):
                sum += cast[uint64]( m.boundaries.len )
            return sum
    )

of "count-captures":
    var regex = compile()
    run(
        proc (): uint64 =
            var count: uint64 = 0
            for m in haystack.findAll(regex):
                # count one for the overall match,
                # as nim-regex dosn't put the entire match
                # as it's own group/capture into m.captures.
                count += 1
                for cap in m.captures:
                    if cap != reNonCapture:
                        count += 1
            return count
    )

of "grep":
    var regex = compile()
    run(
        proc (): uint64 =
            var count: uint64 = 0
            for line in haystack.lines:
                if line.match(regex):
                    count += 1
            return count
    )

of "grep-captures":
    var regex = compile()
    run(
        proc (): uint64 =
            var count: uint64 = 0
            for line in haystack.lines:
                for m in line.findAll(regex):
                    # same as in count-captures
                    count += 1
                    for cap in m.captures:
                        if cap != reNonCapture:
                            count += 1
            return count
    )

else:
    raise newException(Exception, "Unrecognized benchmark model: " & model)
