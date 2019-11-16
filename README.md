# photo-captioner

This is a command line application to aid in the creation and editing of captions for a gallery of images.

Command Line Options:
```
USAGE:
    photo-captioner [OPTIONS] [gallery-dir]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -n, --output-name <output-name>    The name of the output file (if there is one). Will be "captions.csv" by default
                                       for the "csv" output-type.
    -t, --output-type <output-type>    The type of output, available options: "csv" [default: csv]

ARGS:
    <gallery-dir>    Directory of the gallery to generate captions for
```

