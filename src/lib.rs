//!
//! Find out your external IP address, using
//!
//! 1. Internet Gateway Device protocol
//! 2. Public HTTP Services for address information
//!
//! ## Usage
//!
//! ```no_run
//! use whatsmyip::whatsmyip;
//! let addr = whatsmyip().unwrap();
//! ```
//!
//! If you want to specify additional options check
//! `WhatsMyIp`, e.g. to disable the use of IGD
//!
//! ```no_run
//! use whatsmyip::WhatsMyIp;;
//! let addrs = WhatsMyIp::new()
//!                         .igd(false)
//!                         .find().unwrap();
//! ```
//!

extern crate hyper;
#[macro_use] extern crate log;
extern crate rand;
extern crate igd;

use hyper::Client;
use hyper::status::StatusCode;
use std::io::Read;
use rand::{StdRng, Rng};
use std::str::FromStr;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::fmt;
use std::time::Duration;
use std::cmp::min;


// TODO: Get ip from local interfaces
// TODO: PCP
// TODO: NAT-PMP

fn ip_from_str(ip_s: &str) -> Result<MyIp, String> {
    // FIXME: check for private addresses and other
    // erroneous cases
    let ip_trimmed = ip_s.trim();
    if let Ok(ip) = Ipv4Addr::from_str(ip_trimmed) {
        return Ok(MyIp::V4(ip));
    }
    if let Ok(ip) = Ipv6Addr::from_str(ip_trimmed) {
        return Ok(MyIp::V6(ip));
    }
    Err(format!("Invalid IP address {}", ip_s))
}

fn http_ip_txt(opts: &WhatsMyIp, url: &str) -> Result<MyIp,String> {
    let mut cli = Client::new();
    cli.set_read_timeout(opts.http_timeout);
    cli.set_write_timeout(opts.http_timeout);
    let mut res = try!(cli.get(url)
                    .send()
                    .map_err(|err| format!("{}", err)));
    if res.status != StatusCode::Ok {
        return Err(format!("{}", res.status))
    }

    let mut s = String::new();
    try!(res.read_to_string(&mut s)
        .map_err(|err| format!("{}", err)));

    debug!("{} => {}", &url, &s);
    ip_from_str(&s)
}

fn igd_ip() -> Option<MyIp> {
    match igd::search_gateway() {
        Ok(gw) => match gw.get_external_ip() {
            Ok(ip) => {
                // FIXME: check for private IP addresses
                debug!("IGD => {}", ip);
                return Some(MyIp::V4(ip))
            },
            Err(_) => info!("Unable to find IGD gateway"),
        },
        Err(err) => info!("Unable to find gateway: {}", err),
    }
    None
}

// TODO: ip-api.com/json 
type Provider = (&'static str, fn(&WhatsMyIp, &str) -> Result<MyIp, String>);
const HTTP_PROVIDERS: &'static [Provider] = &[
    ("http://icanhazip.com", http_ip_txt),
    ("http://myip.dnsomatic.com", http_ip_txt),
    ("http://bot.whatismyipaddress.com/", http_ip_txt),
    ("https://api.ipify.org?format=text", http_ip_txt),
    ];

#[derive(PartialEq)]
pub enum MyIp {
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}

impl fmt::Display for MyIp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &MyIp::V4(ip) => ip.fmt(f),
            &MyIp::V6(ip) => ip.fmt(f),
        }
    }
}

pub struct WhatsMyIp {
    igd: bool,
    fast: bool,
    http: usize,
    http_timeout: Option<Duration>,
}

impl WhatsMyIp {
    pub fn new() -> WhatsMyIp {
        WhatsMyIp {
            igd: true,
            fast: false,
            http: HTTP_PROVIDERS.len(),
            http_timeout: None,
        }
    }

    /// Enable/Disable the use of the Internet Gateway Device 
    /// (defaults to **true**)
    pub fn igd(&mut self, enabled: bool) -> &mut Self {
        self.igd = enabled;
        self
    }

    /// If true, `find()` will return as soon as
    /// it gets one IP address. If false it will try all available
    /// methods before returning.
    /// (defaults to **false**)
    pub fn fast(&mut self, enabled: bool) -> &mut Self {
        self.fast = enabled;
        self
    }

    /// Limit the number of HTTP requests we can make
    /// (defaults to **None** i.e. no limit)
    pub fn http_limit(&mut self, count: Option<usize>) -> &mut Self {
        self.http = match count {
            Some(num) => min(num, HTTP_PROVIDERS.len()),
            None => HTTP_PROVIDERS.len(),
        };
        self
    }

    /// Enforce HTTP request timeout for HTTP services (per service)
    pub fn http_timeout(&mut self, t: Option<Duration>) -> &mut Self {
        self.http_timeout = t;
        self
    }

    /// Returns a list of IP addresses, with no repeated entries.
    ///
    /// IP addresses are determined from various sources,
    /// in this order:
    ///
    /// 1. Internet Gateway Device protocol
    /// 2. external HTTP services (see the source for a full list)
    ///
    /// In general you can expect this method to be slow.
    /// even if `fast(true)`.
    pub fn find(&self) -> Result<Vec<MyIp>, String> {
        let mut results = Vec::new();

        if let Some(ip) = igd_ip() {
            results.push(ip);
            if self.fast {
                return Ok(results);
            }
        }

        if self.http > 0 {
            // Shuffle HTTP_PROVIDERS just in case
            let mut providers = Vec::new();
            for p in HTTP_PROVIDERS {
                providers.push(p.clone());
            };
            if let Ok(mut rng) = StdRng::new() {
                rng.shuffle(&mut providers);
            }

            for idx in 0..self.http {
                let &(url, fun) = providers[idx];
                let ip = match fun(self, url) {
                    Ok(ip) => ip,
                    Err(err) => {
                        info!("{} => {}", &url, err);
                        continue;
                    },
                };

                if !results.contains(&ip) {
                    results.push(ip);
                }
                if self.fast {
                    return Ok(results);
                }
            }
        }

        if results.is_empty() {
            Err("Unable to find any IP address".to_owned())
        } else {
            Ok(results)
        }
    }
}

/// Returns the first IP address we can find
pub fn whatsmyip() -> Result<MyIp, String> {
    let mut addrs = try!(WhatsMyIp::new()
                        .fast(true)
                        .find());
    addrs.pop()
        .ok_or("Unable to find any IP address".to_owned())
}

#[test]
fn test_http_providers() {
    let w = WhatsMyIp::new();
    for &(url, f) in HTTP_PROVIDERS {
        assert!(f(&w, url).is_ok());
    }
}

#[ignore]
#[test]
fn test_igd() {
    assert!(igd_ip().is_some())
}
