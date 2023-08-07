# Developer setup

In all cases a "full" (i.e. kernel + GNU userspace + systemd) Linux system is assumed.
WSL2 should probably work, but has not been extensively tested.

## Pipeline software

1. Install [Nix](https://nixos.org/) using the [Determinate Installer](https://github.com/DeterminateSystems/nix-installer#usage).
2. Run `cargo build`.

## Kafka

### Development broker

1. Follow the instruction to [deploy Redpanda for development](https://docs.redpanda.com/docs/deploy/deployment-option/self-hosted/manual/production/dev-deployment/).
    - `<private-ip>` and `<seed-node-ips>` should both be set to the IP of the external interface on the system the broker is deployed on.

### Users and topics for data pipeline

1. TODO: users
2. TODO: topics
