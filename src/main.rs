extern crate hellopiston;

fn main() {
    hellopiston::run().unwrap_or_else(|err| {
       eprintln!("Error: {}", err);
    });
}
