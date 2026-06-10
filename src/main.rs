use gl_rdr::descriptor;

fn main() {
    println!("{}", descriptor::pool().services().count());
}
