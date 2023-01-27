These benchmarks test the execution and compilation of regexes derived from
very large dictionaries. These cases are almost impossible for a backtracker to
handle unless they special case them (although few do). But they are also quite
difficult to handle even for automata oriented engines because the size tends
to overwhelm them.
