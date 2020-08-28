use std::env;
use std::io::Write;
use std::process::exit;

use getopts::Options;

use yt_chanvids::{FetchLinksError, ParseJsonDataError, RequestError, YtUploadsCrawler};

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = &args[0];

    let mut opts = Options::new();
    opts.optflag("h", "help", "Print this help menu.");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };

    let usage = format!("Usage: {} [OPTIONS] [--] CHANNEL-ID|USERNAME", program);

    let print_help = || {
        write!(&mut std::io::stdout(), "{}", opts.usage(&usage)).unwrap();
    };

    let print_usage_mistake = |mistake| {
        write!(&mut std::io::stderr(), "{}\n\n", mistake).unwrap();
        write!(&mut std::io::stderr(), "{}", opts.usage(&usage)).unwrap();
    };

    if matches.opt_present("h") {
        print_help();
        exit(0);
    }

    match matches.free.len() {
        0 => {
            print_usage_mistake("Channel id or username not provided.");
            exit(1);
        }
        1 => {
            let channel = &matches.free[0];
            let mut crawler = YtUploadsCrawler::channel(channel);

            while let Some(link) = crawler.next() {
                println!("{}", link.url);
            }

            if let &Some(ref error) = crawler.error() {
                match error {
                    &FetchLinksError::RequestError(ref err) => match err {
                        &RequestError::NotFound => {
                            print_error("Channel does not seem to be reachable.");
                            exit(1);
                        }
                        &RequestError::ParseJsonDataError(ParseJsonDataError::Html(ref html)) => {
                            let message = "A JSON request returned html:\n".to_string() + &html;
                            print_error(&message);
                            exit(1);
                        }
                        _ => panic!(format!("{:?}", err)),
                    },
                    &FetchLinksError::MissingUploadsPage => {
                        print_error("This channel does not have an Uploads page.");
                        exit(1);
                    }
                }
            }

            exit(0);
        }
        _ => {
            print_usage_mistake("Unexpected number of arguments.");
            exit(1);
        }
    }

    fn print_error(message: &str) {
        write!(&mut std::io::stderr(), "{}\n", message).unwrap();
    };
}
