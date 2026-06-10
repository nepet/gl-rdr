mod descriptor;

fn main() {
    println!("{}", descriptor::pool().services().count());
}
