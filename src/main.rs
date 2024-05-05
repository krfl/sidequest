use chrono::serde::ts_seconds;
use chrono::{DateTime, DurationRound, Local, NaiveDateTime, TimeDelta, Utc};
use clap::{Parser, Subcommand};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(arg_required_else_help(true))]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    input: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    Add {},
    List {
        #[arg(short, long)]
        from: Option<String>,
        #[arg(short, long)]
        to: Option<String>,
    },
    Summary {
        #[arg(short, long)]
        from: Option<String>,
        #[arg(short, long, requires = "from")]
        to: Option<String>,
    },
    Export {
        // Print all stored ajour entries in a given format
        #[arg(short, long)]
        format: String,
        #[arg(short, long)]
        from: Option<String>,
        #[arg(short, long, requires = "from")]
        to: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub(crate) struct Entry {
    #[serde(with = "ts_seconds")]
    timestamp: DateTime<Utc>,
    message: String,
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

impl Entry {
    fn to_daily(val: &Entry) -> Self {
        Self {
            timestamp: val
                .timestamp
                .duration_trunc(TimeDelta::try_days(1).unwrap())
                .unwrap(),
            message: val.message.to_owned(),
        }
    }
    fn merge(&mut self, entry: &Entry) {
        let mut msg = capitalize(&self.message);
        msg.push_str(". ");
        msg.push_str(&capitalize(&entry.message));
        self.message = msg;
    }
}

fn get_ajour_file(clear: bool) -> File {
    let mut path = config_dir().expect("Unable to find ajour file");
    path.push("ajour");
    path.push("ajour.json");
    let path_str = path.clone();
    let error_message = format!("Unable to open file: {:?}", path_str.as_os_str());
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(clear)
        .open(path)
        .expect(&error_message)
}

fn parse_date(date: Option<String>) -> Option<DateTime<Utc>> {
    match date {
        Some(date) => {
            let naive_date_time = NaiveDateTime::parse_from_str(date.as_str(), "%Y-%m-%d %H:%M");
            let naive_date = NaiveDateTime::parse_from_str(
                format!("{} 0:0", date.as_str()).as_str(),
                "%Y-%m-%d %H:%M",
            );
            let date_time = naive_date_time.or(naive_date).ok()?;
            let timezone = Local::now().timezone();
            match date_time.and_local_timezone(timezone) {
                chrono::offset::LocalResult::Single(dt) => Some(dt.to_utc()),
                chrono::offset::LocalResult::Ambiguous(dt, dt2) => {
                    eprintln!("Ambigous date `{}` got {dt:?} and {dt2:?}", date);
                    // TODO: return Some(dt.to_utc()) instead?
                    None
                }
                chrono::offset::LocalResult::None => None,
            }
        }
        None => None,
    }
}

fn main() {
    let cli = Cli::parse();

    let mut entries: Vec<Entry>;

    let file = get_ajour_file(false);
    let reader = BufReader::new(file);
    entries = match serde_json::from_reader(reader) {
        Ok(entries) => entries,
        Err(_) => vec![],
    };
    match &cli.command {
        Some(Commands::Add {}) | None => {
            if !cli.input.is_empty() {
                entries.push(Entry {
                    timestamp: Utc::now(),
                    message: cli.input.join(" "),
                });
                let file = get_ajour_file(true);
                let writer = BufWriter::new(file);
                let res = serde_json::to_writer(writer, &entries);
                if res.is_ok() {
                    // Do nothing
                } else {
                    println!("Not ok")
                }
            }
        }
        Some(Commands::List { from, to }) => {
            let mut filtered_entries: Vec<Entry> = entries.clone();

            if from.is_some() {
                filtered_entries.retain(|e| {
                    e.timestamp >= parse_date(from.to_owned()).expect("Invalid datetime supplied")
                });
            }

            if to.is_some() {
                filtered_entries.retain(|e| {
                    e.timestamp <= parse_date(to.to_owned()).expect("Invalid datetime supplied")
                });
            }

            for entry in filtered_entries {
                let local_time: DateTime<Local> = DateTime::from(entry.timestamp);
                println!("{}: {}", local_time, entry.message);
            }
        }
        Some(Commands::Summary { from, to }) => {
            let mut filtered_entries: Vec<Entry> = entries.clone();

            if from.is_some() {
                filtered_entries.retain(|e| {
                    e.timestamp >= parse_date(from.to_owned()).expect("Invalid datetime supplied")
                });
            }

            if to.is_some() {
                filtered_entries.retain(|e| {
                    e.timestamp <= parse_date(to.to_owned()).expect("Invalid datetime supplied")
                });
            }

            let mut dailies = HashMap::<DateTime<Utc>, Entry>::new();

            filtered_entries.iter().map(Entry::to_daily).for_each(|e| {
                if let Some(daily) = dailies.get_mut(&e.timestamp) {
                    daily.merge(&e);
                } else {
                    dailies.insert(e.timestamp, e);
                }
            });

            let mut sorted: Vec<_> = dailies.iter().collect();
            sorted.sort_by_key(|a| a.0);

            for (key, value) in sorted.iter() {
                let local_time: DateTime<Local> = DateTime::from(**key);
                println!("{}: {}", local_time.format("%Y-%m-%d"), value.message);
            }
        }
        Some(Commands::Export { .. }) => {
            todo!();
        }
    }
}
