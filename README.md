CSGO demo file parser 
=====================

This project parses CS:GO demo files, currently reporting various stats from
the files. Eventually maybe supports ranking teams based on games, right now it doesn't.

See also:
- [csgo-demoinfo, Valve's CSGO demo parser](https://github.com/ValveSoftware/csgo-demoinfo/)
- [demoinfogo-linux, same but a Linux port](https://github.com/kaimallea/demoinfogo-linux)
- [demofile, node.js port](https://github.com/saul/demofile)
- [demoinfogo, earlier Rust port](https://github.com/miedzinski/demoinfogo)

Building
--------

Ensure that you have `cargo` and `sqlite3` installed.

```
cargo build
```

Running
-------

```
cargo run -- path/to/demo/file.dem
```

Running tests
-------------

```
cargo test
```
