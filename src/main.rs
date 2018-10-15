/* Based heavily off of ddcset by arcnmx, https://github.com/arcnmx/ddcset-rs/ */
extern crate clap;
extern crate ddc_hi;
extern crate env_logger;

#[macro_use]
extern crate conv;
#[macro_use]
extern crate enum_derive;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
#[macro_use]
extern crate macro_attr;

use clap::{App, AppSettings, Arg, SubCommand};
use conv::TryFrom;
use ddc_hi::{Backend, Ddc, DdcHost, Display, Query};
use failure::Error;
use std::str::FromStr;

const INPUT_SELECT: u8 = 0x60;

macro_attr! {
    #[derive(Clone, Copy, Debug, PartialEq, EnumDisplay!, EnumFromStr!, IterVariantNames!(InputSourceVariantNames), TryFrom!(u16))]
    #[repr(u8)]
    enum InputSource {
          Vga1 = 0x01,
          Vga2 = 0x02,
          Dvi1 = 0x03,
          Dvi2 = 0x04,
          CompositeVideo1 = 0x05,
          CompositeVideo2 = 0x06,
          SVideo1 = 0x07,
          SVideo2 = 0x08,
          Tuner1 = 0x09,
          Tuner2 = 0x0a,
          Tuner3 = 0x0b,
          ComponentVideo1 = 0x0c,
          ComponentVideo2 = 0x0d,
          ComponentVideo3 = 0x0e,
          DisplayPort1 = 0x0f,
          DisplayPort2 = 0x10,
          Hdmi1 = 0x11,
          Hdmi2 = 0x12,
    }
}

#[derive(Default)]
struct DisplaySleep(Vec<Display>);

impl DisplaySleep {
    fn add(&mut self, display: Display) {
        self.0.push(display)
    }
}

impl Drop for DisplaySleep {
    fn drop(&mut self) {
        info!("Waiting for display communication delays before exit");
        for display in &mut self.0 {
            display.handle.sleep()
        }
    }
}

fn displays(query: (Query, bool)) -> Result<Vec<Display>, Error> {
    let needs_caps = query.1;
    let query = query.0;
    Display::enumerate()
        .into_iter()
        .map(|mut d| {
            if needs_caps && d.info.backend == Backend::WinApi {
                d.update_capabilities().map(|_| d)
            } else {
                Ok(d)
            }
        }).filter(|d| {
            if let Ok(ref d) = *d {
                query.matches(&d.info)
            } else {
                true
            }
        }).collect()
}

fn set_input_source(display: &mut Display, input_source: InputSource) -> Result<(), Error> {
    if let Some(feature) = display.info.mccs_database.get(INPUT_SELECT) {
        display
            .handle
            .set_vcp_feature(feature.code, input_source as u16)
    } else {
        Err(format_err!("Could not access input source feature"))
    }
}

fn get_input_source(display: &mut Display) -> Result<InputSource, Error> {
    if let Some(feature) = display.info.mccs_database.get(INPUT_SELECT) {
        InputSource::try_from(display.handle.get_vcp_feature(feature.code)?.value())
            .map_err(Error::from)
    } else {
        Err(format_err!("Could not access input source feature"))
    }
}

fn main() -> Result<(), Error> {
    env_logger::init();

    let backend_values: Vec<_> = Backend::values().iter().map(|v| v.to_string()).collect();
    let backend_values: Vec<_> = backend_values.iter().map(|v| &v[..]).collect();

    let input_source_values: Vec<_> = InputSource::iter_variant_names().collect();

    let app = App::new("monitor-switch")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Maxwell Koo <mjkoo90@gmail.com>")
        .about("DDC/CI monitor switch, based off of ddcset by arcnmx (https://github.com/arcnmx/ddcset-rs)")
        .arg(Arg::with_name("backend")
            .short("b")
            .long("backend")
            .value_name("BACKEND")
            .takes_value(true)
            .multiple(true)
            .number_of_values(1)
            .possible_values(&backend_values)
            .help("Backend driver whitelist")
        ).arg(Arg::with_name("id")
            .short("i")
            .long("id")
            .value_name("ID")
            .takes_value(true)
            .help("Filter by matching backend ID")
        ).arg(Arg::with_name("manufacturer")
            .short("g")
            .long("mfg")
            .value_name("MANUFACTURER")
            .takes_value(true)
            .help("Filter by matching manufacturer ID")
        ).arg(Arg::with_name("model")
            .short("l")
            .long("model")
            .value_name("MODEL NAME")
            .takes_value(true)
            .help("Filter by matching model")
        ).arg(Arg::with_name("serial")
            .short("n")
            .long("sn")
            .value_name("SERIAL")
            .takes_value(true)
            .help("Filter by matching serial number")
            // TODO: filter by index? winapi makes things difficult, nothing is identifying...
        ).subcommand(SubCommand::with_name("set")
            .about("Set input source to specified value")
            .arg(Arg::with_name("INPUT")
                 .required(true)
                 .possible_values(&input_source_values)
                 .index(1))
        ).subcommand(SubCommand::with_name("toggle")
            .about("Toggle input source between two values")
            .arg(Arg::with_name("INPUT1")
                 .required(true)
                 .possible_values(&input_source_values)
                 .index(1))
            .arg(Arg::with_name("INPUT2")
                 .required(true)
                 .possible_values(&input_source_values)
                 .index(2))
        ).setting(AppSettings::SubcommandRequiredElseHelp);

    let matches = app.get_matches();

    let mut query = Query::Any;
    let mut needs_caps = false;
    if let Some(backends) = matches
        .values_of("backend")
        .map(|v| v.map(Backend::from_str))
    {
        let backends = backends
            .map(|b| b.map(Query::Backend))
            .collect::<Result<_, _>>()
            .unwrap();
        query = Query::And(vec![query, Query::Or(backends)])
    }
    if let Some(id) = matches.value_of("id") {
        query = Query::And(vec![query, Query::Id(id.into())])
    }
    if let Some(manufacturer) = matches.value_of("manufacturer") {
        query = Query::And(vec![query, Query::ManufacturerId(manufacturer.into())])
    }
    if let Some(model) = matches.value_of("model") {
        query = Query::And(vec![query, Query::ModelName(model.into())]);
        needs_caps = true;
    }
    if let Some(serial) = matches.value_of("serial") {
        query = Query::And(vec![query, Query::SerialNumber(serial.into())])
    }

    let query = (query, needs_caps);

    let mut sleep = DisplaySleep::default();

    match matches.subcommand() {
        ("set", Some(matches)) => {
            let input_source: InputSource = matches
                .value_of("INPUT")
                .map(InputSource::from_str)
                .unwrap()?;

            for mut display in displays(query)? {
                display.update_capabilities()?;
                // This sometimes fails but the switch still succeeded, ignore the Err for now
                if let Err(e) = set_input_source(&mut display, input_source) {
                    warn!("Error while setting input: {}", e)
                }
                sleep.add(display);
            }
        }
        ("toggle", Some(matches)) => {
            let input_source_1: InputSource = matches
                .value_of("INPUT1")
                .map(InputSource::from_str)
                .unwrap()?;
            let input_source_2: InputSource = matches
                .value_of("INPUT2")
                .map(InputSource::from_str)
                .unwrap()?;

            let mut target: Option<InputSource> = None;
            for mut display in displays(query)? {
                display.update_capabilities()?;

                if target.is_none() {
                    let current = get_input_source(&mut display)?;

                    target = if current == input_source_1 {
                        Some(input_source_2)
                    } else if current == input_source_2 {
                        Some(input_source_1)
                    } else {
                        bail!(format_err!("Current input source is not a toggle option"))
                    }
                }

                if let Some(input_source) = target {
                    // This sometimes fails but the switch still succeeded, ignore the Err for now
                    if let Err(e) = set_input_source(&mut display, input_source) {
                        warn!("Error while setting input: {}", e)
                    }
                }

                sleep.add(display);
            }
        }
        _ => unreachable!("Invalid subcommand"),
    }

    Ok(())
}
