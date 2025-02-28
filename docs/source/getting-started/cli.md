# Using the compiler

Inko's compiler is available through the `inko` command.

## Compiling and running

To compile and then run a file, use the `inko run` command:

```bash
inko run hello.inko
```

This command is useful when running simple scripts or during the development of
your project. If your program defines any command line flags, specify them
_after_ the file to run:

```bash
inko run hello.inko --foo=bar
```

Any flags specified _before_ the file to run are treated as flags for the `run`
command.

## Compiling without running

The `inko run` command requires your source code to be available, and compiles
it from scratch every time. To avoid this, we can build a standalone executable
using the `inko build` command:

```bash
inko build hello.inko
```

The resulting executable is located at `./build/hello`. By default the compiler
enables a reasonable number of optimisations, without sacrificing compile times.
You can either disable optimisations entirely, or enable more aggressive
optimisations at the cost of compile times increasing:

```bash
inko build --opt none hello.inko       # No optimisations
inko build --opt aggressive hello.inko # Aggressive optimisations
```

For `--opt none` the executable is placed in `./build/none/hello`, and
`./build/aggressive/hello` for `--opt aggressive`.

!!! tip
    Only use `--opt aggressive` if you have determined the increase in compile
    times is worth the increase in runtime performance. Most users will want to
    avoid this option entirely.

You can specify an alternative output path using the `-o` option:

```bash
inko build -o /tmp/hello hello.inko
```

For more information, run `inko --help`.
