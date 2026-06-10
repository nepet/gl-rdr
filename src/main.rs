mod descriptor;
mod params;

fn main() {
    println!("{}", descriptor::pool().services().count());
}
