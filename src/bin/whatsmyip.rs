extern crate whatsmyip;
extern crate env_logger;

use whatsmyip::WhatsMyIp;

fn main() {
    env_logger::init().unwrap();
    let addrs = WhatsMyIp::new()
                    .http_limit(Some(1))
                    .find().unwrap();
    for addr in addrs {
        println!("{}", &addr);
    }
}
