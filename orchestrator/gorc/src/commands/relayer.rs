mod start;

use abscissa_core::{clap::Parser, Command, Runnable};

/// Management commands for the relayer
#[derive(Command, Debug, Parser, Runnable)]
pub enum RelayerCmd {
    Start(start::StartCommand),
}
