extern crate j;
extern crate env_logger;

use j::JBot;

fn main() {
    env_logger::init();
    JBot::run("j.toml")
}
