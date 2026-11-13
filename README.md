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

## Use

```sh
stax        # the dashboard
stax list   # every section's stacks as text
```

Each section is a tab of open PRs from a GitHub search. A PR whose base branch is another PR's branch, in the same repo, sits under it.

| Key | Action |
| --- | --- |
| `j` `k` | move |
| `g` `G` | first, last |
| `h` `l` | previous, next section |
| `space` `enter` | fold or unfold a stack |
| `p` | toggle the preview |
| `ctrl+d` `ctrl+u` | scroll the preview |
| `o` | open in the browser |
| `y` | copy the URL |
| `d` | diff in `less` |
| `C` | check out in the repo's local clone |
| `v` | approve, after `y` to confirm |
| `m` | merge with `gh pr merge` |
| `r` `R` | refresh this section, every section |
| `q` | quit |

## Config

stax reads `~/.config/stax/config.toml`: sections, your own keybindings, and where repos are checked out. See [`config.example.toml`](config.example.toml).

## Credits

Inspired by [gh-dash](https://github.com/dlvhdr/gh-dash) by Dolev Hadar. stax is a separate Rust program and shares no code with it.

## License

MIT
