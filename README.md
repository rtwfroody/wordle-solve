# Introduction

wordle-solve finds the best(?) wordle guess. It should work with any unicode
list of words, so should work for any language where wordle makes sense.

# Usage

If you run it with no arguments it will compute the best first guess. This
will take some time, and I can save you the trouble. The answer this algorithm
comes up with is "raise."

Be sure to run a release build, because the debug build is 23 times slower.

Subsequent runs, where you specify what the wordle game returns, are plenty
fast. Each row in the wordle game is represented by a single command line
argument. That argument consists of 5 letters separated by spaces. Each gray
letter has a `-` in front of it, each yellow letter has a `~` in front of it,
and green letters don't have a prefix.

Sample session:
```
$ wordle-solve -- "-r -a ~i -s -e"
107/2310 words remaining
Best guess: hotly
$ wordle-solve -- "-r -a ~i -s -e" "-h -o ~t -l y"
2/2310 words remaining
  fifty
  minty
Guess: fifty
```
Note that you need to surround each constraint with `"` to make the shell pass
them as a single argument. In addition here you also need the extra `--`
argument to prevent the option parser from thinking you're trying to pass an
option that starts with `-r`.

# Algorithm

```
for each guess in the word list:
    for each word in the word list:
        constraints = wordle result you get from guess against word
        increase the guess score by the number of words this constraint lets you
                eliminate from the word list
```

# Disclaimers

This was my first rust project, so there are probably many things that could be
done better. I also haven't done any research on how to most efficiently solve
wordle. I'm sure someone has a better solution than this one.
