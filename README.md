# stax

A terminal dashboard for GitHub pull requests. Stacked PRs show as trees you can fold.

```
My PRs (6)
  main
  └─ #41505 move the driver to client/server
     └─ #41506 add a non-blocking enqueue
        └─ #41507 build core and event crates
```

## Install

```sh
cargo install --path .
```

The binary is `stax`. The crate is `stax-tui`, because `stax` is taken on crates.io.

## Auth

stax reads a token from `gh auth token` at startup. Log in with `gh auth login` first.

## Credits

Inspired by [gh-dash](https://github.com/dlvhdr/gh-dash) by Dolev Hadar. stax is a separate Rust program and shares no code with it.

## License

MIT
