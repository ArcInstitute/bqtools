use clap::Parser;

#[derive(Parser, Debug)]
/// Show information about a BINSEQ file.
pub struct InfoCommand {
    #[clap(num_args=1.., required=true)]
    pub input: Vec<String>,

    #[clap(flatten)]
    pub opts: InfoOpts,
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "INFO OPTIONS")]
pub struct InfoOpts {
    /// Only print the number of records in the file
    #[clap(short, long)]
    pub num: bool,

    /// Print the file in JSON format
    #[clap(short, long, conflicts_with_all=["show_index", "show_headers", "num"])]
    pub json: bool,

    /// Print the index of the file
    #[clap(long)]
    pub show_index: bool,

    /// Print the block headers of the file
    #[clap(long, conflicts_with = "show_index")]
    pub show_headers: bool,
}
