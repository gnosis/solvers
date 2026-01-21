![pull request](https://github.com/gnosis/solvers/workflows/pull%20request/badge.svg) ![deploy](https://github.com/gnosis/solvers/workflows/deploy/badge.svg)
# SOLVERS

## Description

This project is a solver engine that interfaces with several Decentralized Exchanges (DEXes), including Balancer, 0x, OneInch, and ParaSwap. 
The specific DEX solver that is instantiated depends on the command line argument provided when initiating the project.

## Pre-requisites

To build and run this project, you will need:

- **[Rust](https://www.rust-lang.org/tools/install)**. It's recommended to use the stable branch.

### Usage

1. Build the project by running:
   ```bash
   cargo build --release
   ```
   This command will create an executable `solvers` in the `./target/release` directory.

2. Run the project with the following command:
   ```bash
   solvers <solver_name> --config <config_path>
   ```
   
   Replace `<solver_name>` with the necessary solver you want to run:
    - `zeroex`
    - `balancer`
    - `oneinch`
    - `okx`
    - `paraswap`

   `<config_path>` is the path to the corresponding solver's config. Examples for each solver can be found in the `./config` directory.
