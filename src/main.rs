extern crate stacked_worlds;

fn main() {
    stacked_worlds::run().unwrap_or_else(|err| {
       eprintln!("Error: {}", err);
    });
}
