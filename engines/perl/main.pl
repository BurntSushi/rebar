use strict;
use Time::HiRes qw(CLOCK_MONOTONIC clock_gettime);

sub parseklv {
    my $raw = shift;
    my @pieces = split /:/, $raw, 3;
    if (@pieces < 3) {
        die "invalid KLV item: not enough pieces";
    }
    my $key = $pieces[0];
    my $valuelen = int($pieces[1]);
    my $rest = $pieces[2];
    if (length $rest < $valuelen) {
        die "not enough bytes remaining for length $valuelen for key '$key'";
    }
    my $value = substr $rest, 0, $valuelen;
    $rest = substr $rest, $valuelen, (length $rest - $valuelen);
    if (length $rest == 0 || substr($rest, 0, 1) ne "\n") {
        die "did not find \\n after value for key '$key'";
    }
    my $nread =
        length($pieces[0]) + 1 + length($pieces[1]) + 1 + length($value) + 1;
    return ($key, $value, $nread);
}

sub compilepat {
    my %config = %{shift()};
    my $pattern = shift;

    my $flags = "";
    if ($config{"unicode"}) {
        $flags .= "u";
    } else {
        $flags .= "a";
    }
    if ($config{"casei"}) {
        $flags .= "i";
    }
    my $p = "(?$flags)$pattern";
    return qr/$p/;
}

sub benchmark {
    sub now {
        return clock_gettime(CLOCK_MONOTONIC);
    }

    sub elapsednanos {
        my $t = shift;
        return int(1000000000 * (now() - $t));
    }

    my %config = %{shift()};
    my $count = shift; # closure that accepts result of $bench and returns int
    my $bench = shift; # closure that runs the thing we are measuring.

    my $warmupstart = now();
    for (my $i = 0; $i < $config{"maxwarmupiters"}; $i++) {
        my $result = &$bench();
        &$count($result);
        if (elapsednanos($warmupstart) >= $config{"maxwarmuptime"}) {
            last;
        }
    }

    # In other runner programs, I usually represent the
    # samples as a single array of pairs. I guess in Perl
    # the most natural thing would be a multi-dimensional
    # array. I looked up how to do that because the obvious
    # thing didn't work for me, and decided to just skip
    # out on that and use corresponding flat arrays.
    #
    # PRs are welcome to simplify this code.
    my @durations = ();
    my @counts = ();
    my $runstart = now();
    for (my $i = 0; $i < $config{"maxiters"}; $i++) {
        my $benchstart = now();
        my $result = &$bench();
        my $elapsed = elapsednanos($benchstart);
        my $n = &$count($result);
        $durations[++$#durations] = $elapsed;
        $counts[++$#counts] = $n;
        if (elapsednanos($runstart) >= $config{"maxtime"}) {
            last;
        }
    }
    return (\@durations, \@counts);
}

sub modelcompile {
    my %config = %{shift()};
    my $count = sub {
        my $re = shift;
        my $count = 0;
        while ($config{"haystack"} =~ /$re/g) {
            $count++;
        }
        return $count;
    };
    # My suspicion is that this is caching regex
    # compilation, thus making this measurement
    # bunk. I could not find a way to clear the
    # regex compilation cache sadly. Ideas for
    # how to resolve this (assuming I'm even right
    # about this getting cached) are very welcome.
    my $bench = sub { compilepat \%config, $config{"pattern"} };
    return benchmark \%config, $count, $bench;
}

sub modelcount {
    my %config = %{shift()};
    my $re = compilepat \%config, $config{"pattern"};
    my $count = sub { my $n = shift; $n };
    my $bench = sub {
        my $count = 0;
        # Does appending the 'g' flag here cause the pattern to
        # get re-compiled? I kind of hope not. But I couldn't attach
        # 'g' to the pre-compiled regex (which makes sense, because 'g'
        # is about how the search executes). Anyway, patches welcome
        # to fix this.
        #
        # I will note that I can observe the first search taking longer
        # than subsequent searches... Which might suggest that the
        # pattern is being re-compiled here. But not necessarily.
        #
        # Of course, if this is the only way, and the regex has to get
        # re-compiled for every search just to find all non-overlapping
        # matches, then, well, that's just the way the cookie crumbles.
        while ($config{"haystack"} =~ /$re/g) {
            $count++;
        }
        return $count;
    };
    return benchmark \%config, $count, $bench;
}

sub modelcountspans {
    my %config = %{shift()};
    my $re = compilepat \%config, $config{"pattern"};
    my $count = sub { my $n = shift; $n };
    my $bench = sub {
        my $sum = 0;
        while ($config{"haystack"} =~ /$re/g) {
            $sum += $+[0] - $-[0];
        }
        return $sum;
    };
    return benchmark \%config, $count, $bench;
}

sub modelcountcaptures {
    my %config = %{shift()};
    my $re = compilepat \%config, $config{"pattern"};
    my $count = sub { my $n = shift; $n };
    my $bench = sub {
        my $count = 0;
        while ($config{"haystack"} =~ /$re/g) {
            # ^CAPTURE only includes the explicit groups,
            # but rebar wants the count to include the
            # overall implicit matching group too.
            $count++;
            foreach my $cap (@{^CAPTURE}) {
                if (defined($cap)) {
                    $count++;
                }
            }
        }
        return $count;
    };
    return benchmark \%config, $count, $bench;
}

sub modelgrep {
    my %config = %{shift()};
    my $re = compilepat \%config, $config{"pattern"};
    my $count = sub { my $n = shift; $n };
    my $bench = sub {
        my $count = 0;
        # It's a little weird to iterate over lines using
        # regex, when we are trying to measure regex search
        # time. But this model is about idiomatically iterating
        # over lines, and this appears to be the standard
        # approach for a string that is already in memory.
        foreach my $line (split /\r?\n/, $config{"haystack"}) {
            if ($line =~ $re) {
                $count++;
            }
        }
        return $count;
    };
    return benchmark \%config, $count, $bench;
}

sub modelgrepcaptures {
    my %config = %{shift()};
    my $re = compilepat \%config, $config{"pattern"};
    my $count = sub { my $n = shift; $n };
    my $bench = sub {
        my $count = 0;
        # It's a little weird to iterate over lines using
        # regex, when we are trying to measure regex search
        # time. But this model is about idiomatically iterating
        # over lines, and this appears to be the standard
        # approach for a string that is already in memory.
        foreach my $line (split /\r?\n/, $config{"haystack"}) {
            while ($line =~ /$re/g) {
                # ^CAPTURE only includes the explicit groups,
                # but rebar wants the count to include the
                # overall implicit matching group too.
                $count++;
                foreach my $cap (@{^CAPTURE}) {
                    if (defined($cap)) {
                        $count++;
                    }
                }
            }
        }
        return $count;
    };
    return benchmark \%config, $count, $bench;
}

sub modelregexredux {
    my %config = %{shift()};
    my $count = sub { my $n = shift; $n };
    my $bench = sub {
        my $expected = "agggtaaa|tttaccct 6
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
";

        my @out = ();
        my $seq = $config{"haystack"};
        my $ilen = length $seq;

        my $re = compilepat \%config, ">[^\n]*\n|\n";
        $seq =~ s/$re//g;
        my $clen = length $seq;

        my @variants = (
			"agggtaaa|tttaccct",
			"[cgt]gggtaaa|tttaccc[acg]",
			"a[act]ggtaaa|tttacc[agt]t",
			"ag[act]gtaaa|tttac[agt]ct",
			"agg[act]taaa|ttta[agt]cct",
			"aggg[acg]aaa|ttt[cgt]ccct",
			"agggt[cgt]aa|tt[acg]accct",
			"agggta[cgt]a|t[acg]taccct",
			"agggtaa[cgt]|[acg]ttaccct",
        );
        foreach my $variant (@variants) {
            my $re = compilepat \%config, $variant;
            my $count = 0;
            while ($seq =~ /$re/g) {
                $count++;
            }
            $out[++$#out] = sprintf "%s %d", $variant, $count;
        }

        my $re = compilepat \%config, "tHa[Nt]";
        $seq =~ s/$re/<4>/g;

        my $re = compilepat \%config, "aND|caN|Ha[DS]|WaS";
        $seq =~ s/$re/<3>/g;

        my $re = compilepat \%config, "a[NSt]|BY";
        $seq =~ s/$re/<2>/g;

        my $re = compilepat \%config, "<[^>]*>";
        $seq =~ s/$re/|/g;

        my $re = compilepat \%config, "\\|[^|][^|]*\\|";
        $seq =~ s/$re/-/g;

        $out[++$#out] = "";
        $out[++$#out] = $ilen;
        $out[++$#out] = $clen;
        $out[++$#out] = length $seq;
        my $result = join("\n", @out) . "\n";
        if ($result ne $expected) {
            die "result did not match expected";
        }
        return length $seq;
    };
    return benchmark \%config, $count, $bench;
}

sub main {
    # It's not clear whether this is actually necessary or not. But basically,
    # we really just want to treat stdin as a raw byte stream since we might
    # get haystacks that are invalid UTF-8. (We could defensibly reject such
    # things, but it looks like Perl can support it, so we do it.)
    binmode(STDIN);
    my %config = (
        name => undef,
        model => undef,
        pattern => undef,
        casei => 0,
        unicode => 0,
        haystack => undef,
        maxiters => 0,
        maxwarmupiters => 0,
        maxtime => 0,
        maxwarmuptime => 0,
    );
    # Yes, this is apparently how one is supposed to slurp up the contents of a
    # file handle into memory. Holy moses.
    my $raw = do { local $/ = undef; <STDIN> };
    while (length($raw) != 0) {
        my ($key, $value, $nread) = parseklv $raw;
        $raw = substr $raw, $nread, length($raw) - $nread;

        if ($key eq "name") {
            $config{"name"} = $value;
        } elsif ($key eq "model") {
            $config{"model"} = $value;
        } elsif ($key eq "pattern") {
            $config{"pattern"} = $value;
        } elsif ($key eq "case-insensitive") {
            $config{"casei"} = $value eq "true";
        } elsif ($key eq "unicode") {
            $config{"unicode"} = $value eq "true";
        } elsif ($key eq "haystack") {
            $config{"haystack"} = $value;
        } elsif ($key eq "max-iters") {
            $config{"maxiters"} = int($value);
        } elsif ($key eq "max-warmup-iters") {
            $config{"maxwarmupiters"} = int($value);
        } elsif ($key eq "max-time") {
            $config{"maxtime"} = int($value);
        } elsif ($key eq "max-warmup-time") {
            $config{"maxwarmuptime"} = int($value);
        }
    }
    # This is apparently necessary for Unicode semantics to
    # apply in regexes. Guess how many times 'utf8::decode'
    # is mentioned in 'perlre' or 'perlunicode'. Guess. Just
    # guess.
    #
    # ZERO.
    #
    # N.B. 'use utf8;' can be used to what appears to achieve
    # the same thing, but only if your regex patterns are
    # literals. In this program, our regex patterns (and
    # haystacks) are inputs.
    if ($config{"unicode"}) {
        utf8::decode($config{"pattern"});
        utf8::decode($config{"haystack"});
    }

    my $durations;
    my $counts;
    if ($config{"model"} eq "compile") {
        ($durations, $counts) = modelcompile(\%config);
    } elsif ($config{"model"} eq "count") {
        ($durations, $counts) = modelcount(\%config);
    } elsif ($config{"model"} eq "count-spans") {
        ($durations, $counts) = modelcountspans(\%config);
    } elsif ($config{"model"} eq "count-captures") {
        ($durations, $counts) = modelcountcaptures(\%config);
    } elsif ($config{"model"} eq "grep") {
        ($durations, $counts) = modelgrep(\%config);
    } elsif ($config{"model"} eq "grep-captures") {
        ($durations, $counts) = modelgrepcaptures(\%config);
    } elsif ($config{"model"} eq "regex-redux") {
        ($durations, $counts) = modelregexredux(\%config);
    } else {
        die "unrecognized model '$config{model}'";
    }
    for (my $i = 0; $i < @{$durations}; $i++) {
        my $dur = @{$durations}[$i];
        my $count = @{$counts}[$i];
        printf "%s,%s\n", $dur, $count;
    }
}

main
