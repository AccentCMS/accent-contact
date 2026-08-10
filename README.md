# accent-contact

A contact form plugin for [Accent CMS](https://accentcms.dev). It validates and
rate-limits submissions, protects them with a CSRF token, and delivers the
result by SMTP through the host's mail capability.

It also contributes two island components, `word-count` and `reading-time`,
which is what makes it the worked example for plugin-provided islands.

## Install

```
accent plugin install accent-contact
```

## What it does

| | |
|---|---|
| Route | `POST /contact-submit` |
| Hook | `on_render` |
| Host capability | `mail` |
| Islands | `word-count`, `reading-time` |

Mail goes through the typed host-services capability, so the plugin needs no
outbound-HTTP allowlist and opens no network connections of its own.

## Building

This is a WebAssembly Component-Model component. Plain `cargo build` will not
work: the bindings under `src/bindings.rs` are generated from `wit/` by
`cargo-component`.

```
rustup target add wasm32-wasip1
cargo install cargo-component --locked --version 0.21.1
cargo component build --release
```

The artifact is `plugin.toml`, `plugin.wasm` and `assets/` together. Package it
with:

```
accent package <staged-directory> --author accentx
```

## Licence

MIT. See [LICENSE](LICENSE).
