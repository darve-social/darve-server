# Darve server

Social platform for joining on user challenge requests, accepting and delivering on them with video content and receiving proposed rewards.

## Development

This project uses [just](https://github.com/casey/just) for common development tasks.

### Prerequisites

- [just](https://github.com/casey/just):  
  Install with Homebrew:
  ```sh
  brew install just
  ```
- [Docker](https://www.docker.com/) and [Docker Compose](https://docs.docker.com/compose/)

### Usage

Start the development server (with infrastructure):

```sh
just dev
```

Run tests:

```sh
just test
```

Build and run in release mode:

```sh
just release
```

Start or stop local infrastructure only:

```sh
just infra_start
just infra_stop
```

> **Note:**  
> The `just` commands automatically load environment variables from `.env`.
