<p align="center">
  <img src="packaging/icons/256x256.png" width="160" alt="PAWS icon">
</p>

<h1 align="center">PAWS</h1>

<p align="center"><strong>PAWS Answers Web Sessions</strong>, a small static file
web server with optional PHP support, written in Rust.</p>

> **!! THIS IS A PROJECT MADE FOR LEARNING AND TOYING WITH, THIS SHOULD NOT BE
> USED IN A PRODUCTION ENVIRONMENT !!**

## Binaries

| Binary     | What it is                                                                        |
| ---------- | --------------------------------------------------------------------------------- |
| `paws-gui` | Desktop control panel. Two tabs: start/monitor/stop the server, and configure it. |
| `paws`     | The server itself, run from the terminal. Reads the config file and serves.       |

The GUI starts the server binary from its own directory, so both must be next
to each other.

## Configuration

`conf.toml` next to the binary (see `paws/src/config.rs` for defaults):

```toml
interface = "0.0.0.0"
port = "8888"
document_root = "htdocs"
php = true
```

All fields are optional and fall back to the defaults above.

When `php = true`, the server looks for the bundled PHP runtime first, then for
`php-fpm` (Linux, macOS) or `php-cgi.exe` (Windows) on `PATH`. If it finds
nothing, startup fails with an error instead of silently serving a PHP-less
server.

## PHP

PHP is bundled with the release packages:

- Linux and macOS: static `php-fpm`, built by
  [static-php-cli](https://github.com/static-php/static-php-cli) from `craft.yml`
- Windows: `php-cgi.exe` from the official Windows PHP build, because php-fpm
  does not exist on Windows

The pinned runtime version is `PHP_VERSION` in `.github/workflows/php.yml`, and
the extension list is in `craft.yml`.

## Packaging

Releases are produced by
[cargo-packager](https://github.com/crabnebula-dev/cargo-packager) through
`.github/workflows/release.yml`: deb and AppImage on Linux, `.dmg` on macOS,
NSIS installer on Windows. Pushing a `v<version>` tag builds all of them and
publishes a GitHub release; running the workflow manually builds the same
packages as artifacts without releasing.

The macOS app is ad-hoc signed, not notarized, so the first launch needs a
right-click and Open in Finder. Windows and Linux packages are unsigned.

## License

MIT, see [LICENSE](LICENSE).
